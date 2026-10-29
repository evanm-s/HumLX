//! The live engine: holds the show, works out the DMX levels and sends them
//! over Art-Net at a steady rate on a background thread.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::artnet;
use crate::dmx::Universe;
use crate::error::{Error, Result};
use crate::fixture::{Attribute, Library};
use crate::patch::{mode_of, Patch};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OutputSettings {
    pub enabled: bool,
    /// Where to send Art-Net: a node's IP address, or a broadcast address.
    pub target: Ipv4Addr,
    /// The 15-bit Art-Net port-address of the universe.
    pub universe: u16,
    /// Packets per second.
    pub rate_hz: u8,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            target: Ipv4Addr::BROADCAST,
            universe: 0,
            rate_hz: 40,
        }
    }
}

impl OutputSettings {
    pub fn validate(&self) -> Result<()> {
        if self.universe > artnet::MAX_PORT_ADDRESS {
            return Err(Error::InvalidShow(format!(
                "universe must be between 0 and {}",
                artnet::MAX_PORT_ADDRESS
            )));
        }
        if !(1..=44).contains(&self.rate_hz) {
            return Err(Error::InvalidShow("output rate must be between 1 and 44 Hz".into()));
        }
        Ok(())
    }
}

/// Live output health, shown in the UI.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputStatus {
    pub packets_sent: u64,
    pub last_error: Option<String>,
}

/// Everything the output thread reads. Locked briefly by each tick and by
/// each UI command.
#[derive(Debug, Default)]
pub struct State {
    pub library: Library,
    pub patch: Patch,
    pub output: OutputSettings,
    pub status: OutputStatus,
    pub blackout: bool,
    /// Raw channel levels set from the channel test screen, by fixture
    /// number and channel index within the fixture.
    test_levels: HashMap<u32, HashMap<usize, u8>>,
    universe: Universe,
}

impl State {
    pub fn new(library: Library, patch: Patch, output: OutputSettings) -> Self {
        Self {
            library,
            patch,
            output,
            ..Default::default()
        }
    }

    pub fn set_test_level(&mut self, fixture: u32, channel: usize, value: u8) -> Result<()> {
        let patched = self.patch.get(fixture).ok_or(Error::UnknownFixture(fixture))?;
        let channels = mode_of(&self.library, patched).map_or(0, |m| m.channels.len());
        if channel >= channels {
            return Err(Error::InvalidShow(format!(
                "fixture {fixture} has no channel {}",
                channel + 1
            )));
        }
        self.test_levels.entry(fixture).or_default().insert(channel, value);
        Ok(())
    }

    /// Sets every channel of the fixture to its highlight value so it can be
    /// found in the room.
    pub fn locate(&mut self, fixture: u32) -> Result<()> {
        let patched = self.patch.get(fixture).ok_or(Error::UnknownFixture(fixture))?;
        let mode = mode_of(&self.library, patched).ok_or_else(|| Error::UnknownProfile(patched.profile.clone()))?;
        let levels = mode
            .channels
            .iter()
            .enumerate()
            .map(|(i, channel)| (i, channel.highlight_value()))
            .collect();
        self.test_levels.insert(fixture, levels);
        Ok(())
    }

    /// Releases test levels for one fixture, or for all when `None`.
    pub fn release(&mut self, fixture: Option<u32>) {
        match fixture {
            Some(id) => {
                self.test_levels.remove(&id);
            }
            None => self.test_levels.clear(),
        }
    }

    /// Current levels of a fixture's channels, as last output.
    pub fn fixture_levels(&self, fixture: u32) -> Result<Vec<u8>> {
        let patched = self.patch.get(fixture).ok_or(Error::UnknownFixture(fixture))?;
        let count = mode_of(&self.library, patched).map_or(0, |m| m.channels.len());
        Ok((0..count)
            .map(|i| self.universe.get(patched.address + i as u16))
            .collect())
    }

    /// Fixtures that currently have test levels applied.
    pub fn tested_fixtures(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.test_levels.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    pub fn universe(&self) -> &Universe {
        &self.universe
    }

    /// Works out the level of every channel and stores it as the current universe.
    pub fn compose(&mut self) -> &Universe {
        let mut universe = Universe::default();
        for fixture in self.patch.fixtures() {
            let Some(mode) = mode_of(&self.library, fixture) else {
                continue;
            };
            let tests = self.test_levels.get(&fixture.id);
            // Without a dimmer channel, blackout has to pull the colour mix down instead.
            let has_dimmer = mode.channels.iter().any(|c| c.attribute == Attribute::Intensity);
            for (i, channel) in mode.channels.iter().enumerate() {
                let mut value = tests.and_then(|t| t.get(&i)).copied().unwrap_or(channel.default);
                let dims = if has_dimmer {
                    channel.attribute == Attribute::Intensity
                } else {
                    channel.attribute.is_colour_mix()
                };
                if self.blackout && dims {
                    value = 0;
                }
                universe.set(fixture.address + i as u16, value);
            }
        }
        self.universe = universe;
        &self.universe
    }
}

/// Owns the output thread. Dropping it stops output.
pub struct Engine {
    state: Arc<Mutex<State>>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    pub fn start(state: State) -> std::io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        let state = Arc::new(Mutex::new(state));
        let running = Arc::new(AtomicBool::new(true));
        let thread = std::thread::Builder::new().name("artnet-output".into()).spawn({
            let state = Arc::clone(&state);
            let running = Arc::clone(&running);
            move || output_loop(&socket, &state, &running)
        })?;
        Ok(Self {
            state,
            running,
            thread: Some(thread),
        })
    }

    /// Locks the state for a command. Keep the work done under the lock short.
    pub fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    // A panic elsewhere must not stop the lights, so recover a poisoned lock.
    state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn output_loop(socket: &UdpSocket, state: &Mutex<State>, running: &AtomicBool) {
    let mut sequence: u8 = 0;
    let mut next_tick = Instant::now();
    while running.load(Ordering::Relaxed) {
        let (packet, target, interval) = {
            let mut state = lock(state);
            let settings = state.output.clone();
            let data = *state.compose().as_bytes();
            let interval = Duration::from_secs_f64(1.0 / f64::from(settings.rate_hz.clamp(1, 44)));
            if !settings.enabled {
                (None, None, interval)
            } else {
                sequence = if sequence == 255 { 1 } else { sequence + 1 };
                let packet = artnet::dmx_packet(settings.universe, sequence, &data);
                (
                    Some(packet),
                    Some(SocketAddr::from((settings.target, artnet::PORT))),
                    interval,
                )
            }
        };

        if let (Some(packet), Some(target)) = (packet, target) {
            let result = socket.send_to(&packet, target);
            let mut state = lock(state);
            match result {
                Ok(_) => {
                    state.status.packets_sent += 1;
                    state.status.last_error = None;
                }
                Err(error) => state.status.last_error = Some(error.to_string()),
            }
        }

        next_tick += interval;
        let now = Instant::now();
        if next_tick > now {
            std::thread::sleep(next_tick - now);
        } else {
            // Fell behind (e.g. the PC slept); restart the schedule rather than bursting.
            next_tick = now;
        }
    }
}

/// Broadcasts an `ArtPoll` and collects the nodes that reply within `wait`.
pub fn discover(wait: Duration) -> Result<Vec<artnet::Node>> {
    let socket = discovery_socket()?;
    socket.set_broadcast(true)?;
    let poll = artnet::poll_packet();
    for target in [Ipv4Addr::BROADCAST, Ipv4Addr::new(2, 255, 255, 255)] {
        // A missing route for one of these is not fatal.
        let _ = socket.send_to(&poll, (target, artnet::PORT));
    }

    let deadline = Instant::now() + wait;
    let mut nodes: Vec<artnet::Node> = Vec::new();
    let mut buffer = [0u8; 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        socket.set_read_timeout(Some(remaining))?;
        match socket.recv_from(&mut buffer) {
            Ok((len, _)) => {
                if let Some(node) = artnet::parse_poll_reply(&buffer[..len]) {
                    if !nodes.iter().any(|n| n.ip == node.ip && n.short_name == node.short_name) {
                        nodes.push(node);
                    }
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => break,
            Err(e) => return Err(e.into()),
        }
    }
    nodes.sort_by_key(|n| n.ip);
    Ok(nodes)
}

/// Nodes reply to port 6454, so listen there. Another Art-Net program on the
/// same PC may already hold the port, in which case discovery cannot work.
fn discovery_socket() -> Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, artnet::PORT)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            Error::Unsupported(
                "searching for nodes while another Art-Net program is running on this PC. \
                 Close it, or type the node's IP address instead"
                    .into(),
            )
        } else {
            e.into()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::NewFixtures;

    fn state_with(profile: &str, mode: &str, address: u16) -> State {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch
            .add(
                &library,
                &NewFixtures {
                    profile: profile.into(),
                    mode: mode.into(),
                    count: 1,
                    first_id: Some(1),
                    address: Some(address),
                    name: None,
                },
            )
            .unwrap();
        State::new(library, patch, OutputSettings::default())
    }

    #[test]
    fn defaults_then_test_levels() {
        let mut state = state_with("generic/moving-head-spot", "12ch", 101);
        state.compose();
        assert_eq!(state.universe().get(101), 128); // pan default
        assert_eq!(state.universe().get(107), 255); // shutter open by default

        state.set_test_level(1, 5, 200).unwrap();
        state.compose();
        assert_eq!(state.universe().get(106), 200);
        assert_eq!(state.fixture_levels(1).unwrap()[5], 200);
        assert!(state.set_test_level(1, 12, 1).is_err());

        state.release(Some(1));
        state.compose();
        assert_eq!(state.universe().get(106), 0);
    }

    #[test]
    fn locate_lights_fixture() {
        let mut state = state_with("generic/rgbw-par", "6ch DRGBW Strobe", 1);
        state.locate(1).unwrap();
        state.compose();
        let levels: Vec<u8> = (1..=6).map(|a| state.universe().get(a)).collect();
        assert_eq!(levels, vec![255, 255, 255, 255, 255, 0]);
        assert_eq!(state.tested_fixtures(), vec![1]);
    }

    #[test]
    fn blackout_uses_dimmer_or_colour() {
        let mut state = state_with("generic/rgbw-par", "6ch DRGBW Strobe", 1);
        state.locate(1).unwrap();
        state.blackout = true;
        state.compose();
        assert_eq!(state.universe().get(1), 0); // dimmer
        assert_eq!(state.universe().get(2), 255); // colour kept

        let mut state = state_with("generic/rgb-par", "3ch RGB", 1);
        state.locate(1).unwrap();
        state.blackout = true;
        state.compose();
        assert_eq!(
            (1..=3).map(|a| state.universe().get(a)).collect::<Vec<_>>(),
            vec![0, 0, 0]
        );
    }

    #[test]
    fn sends_artnet() {
        // The engine always sends to the Art-Net port, so point it at a
        // listener there only if it is free; otherwise just check it runs.
        let mut state = state_with("generic/dimmer", "1ch", 1);
        state.locate(1).unwrap();
        state.output.target = Ipv4Addr::LOCALHOST;
        let listener = UdpSocket::bind((Ipv4Addr::LOCALHOST, artnet::PORT)).ok();
        let engine = Engine::start(state).unwrap();
        if let Some(listener) = listener {
            listener.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut buffer = [0u8; 600];
            let (len, _) = listener.recv_from(&mut buffer).unwrap();
            assert_eq!(len, 530);
            assert_eq!(buffer[18], 255);
        }
        std::thread::sleep(Duration::from_millis(100));
        assert!(engine.state().status.packets_sent > 0);
    }
}
