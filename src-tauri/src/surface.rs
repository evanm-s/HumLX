//! Reads an Obsidian NX-K control surface over USB and passes its keys and
//! encoders to the UI as `surface` events. Keeps looking for the surface in
//! the background, so it can be plugged in or out at any time.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use humlx_core::nxk::{self, Event, Smoother};
use rusb::UsbContext;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceStatus {
    pub connected: bool,
    /// Why the surface can't be used, when it is plugged in but not working.
    pub problem: Option<String>,
}

pub type Shared = Arc<Mutex<SurfaceStatus>>;

/// Starts watching for the surface.
pub fn start(handle: AppHandle) -> Shared {
    let status = Shared::default();
    let shared = Arc::clone(&status);
    thread::Builder::new()
        .name("nx-k".into())
        .spawn(move || watch(&handle, &shared))
        .expect("could not start the NX-K thread");
    status
}

fn watch(handle: &AppHandle, status: &Shared) {
    let set = |next: SurfaceStatus| {
        let mut current = status.lock().unwrap_or_else(|p| p.into_inner());
        if *current != next {
            *current = next.clone();
            let _ = handle.emit("surface-status", next);
        }
    };
    let Ok(context) = rusb::Context::new() else {
        set(SurfaceStatus {
            connected: false,
            problem: Some("USB is not available on this computer".into()),
        });
        return;
    };
    loop {
        match open(&context) {
            Ok(Some(device)) => {
                set(SurfaceStatus {
                    connected: true,
                    problem: None,
                });
                read(handle, &device);
                set(SurfaceStatus::default());
            }
            Ok(None) => set(SurfaceStatus::default()),
            Err(error) => set(SurfaceStatus {
                connected: false,
                problem: Some(explain(error)),
            }),
        }
        thread::sleep(Duration::from_secs(2));
    }
}

/// Opens the surface if it is plugged in.
fn open(context: &rusb::Context) -> rusb::Result<Option<rusb::DeviceHandle<rusb::Context>>> {
    let found = context.devices()?.iter().find(|d| {
        d.device_descriptor()
            .is_ok_and(|desc| desc.vendor_id() == nxk::VENDOR_ID && desc.product_id() == nxk::PRODUCT_ID)
    });
    let Some(device) = found else { return Ok(None) };
    let handle = device.open()?;
    // Not supported everywhere, and not needed where it isn't.
    let _ = handle.set_auto_detach_kernel_driver(true);
    handle.claim_interface(0)?;
    handle.set_alternate_setting(0, nxk::ALT_SETTING)?;
    Ok(Some(handle))
}

/// Passes on what the surface sends until it is unplugged.
fn read(handle: &AppHandle, device: &rusb::DeviceHandle<rusb::Context>) {
    let mut smoother = Smoother::default();
    let mut buf = [0u8; 64];
    loop {
        let len = match device.read_interrupt(nxk::ENDPOINT, &mut buf, Duration::from_millis(500)) {
            Ok(len) => len,
            Err(rusb::Error::Timeout) => continue,
            Err(_) => return,
        };
        let event = match nxk::parse(&buf[..len]) {
            Some(Event::Encoder { index, delta }) => match smoother.feed(index, delta, Instant::now()) {
                Some(delta) => Event::Encoder { index, delta },
                None => continue,
            },
            Some(event) => event,
            None => continue,
        };
        let _ = handle.emit("surface", event);
    }
}

fn explain(error: rusb::Error) -> String {
    match error {
        rusb::Error::Access if cfg!(target_os = "linux") => {
            "No permission to use the NX-K. Add the udev rule from the HumLX readme, then plug it in again.".into()
        }
        rusb::Error::Access | rusb::Error::Busy => {
            "The NX-K is in use by another program, such as Onyx. Close it and plug the NX-K in again.".into()
        }
        rusb::Error::NotSupported | rusb::Error::NotFound if cfg!(windows) => {
            "The NX-K needs the WinUSB driver to work with HumLX. See the HumLX readme.".into()
        }
        other => format!("The NX-K could not be opened ({other})."),
    }
}

#[tauri::command]
pub fn surface_status(status: State<'_, Shared>) -> SurfaceStatus {
    status.lock().unwrap_or_else(|p| p.into_inner()).clone()
}
