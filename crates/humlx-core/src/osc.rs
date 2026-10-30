//! OSC (Open Sound Control) input, so cue lists can be run from other
//! software or a phone: QLab, Bitfocus Companion, TouchOSC and so on.
//!
//! Messages arrive over UDP. The addresses understood are:
//!
//! | Address                          | Arguments        | Does                          |
//! | -------------------------------- | ---------------- | ----------------------------- |
//! | `/humlx/cuelist/{list}/go`       |                  | GO on the cue list            |
//! | `/humlx/cuelist/{list}/back`     |                  | Back one cue                  |
//! | `/humlx/cuelist/{list}/release`  |                  | Release the cue list          |
//! | `/humlx/cuelist/{list}/goto`     | cue number       | Go to a cue (numbered from 1) |
//! | `/humlx/cuelist/{list}/goto/{n}` |                  | Go to cue `n`                 |
//! | `/humlx/cuelist/{list}/master`   | level 0.0 to 1.0 | Set the cue list's master     |
//! | `/humlx/releaseall`              |                  | Release everything            |
//! | `/humlx/blackout`                | 1 or 0           | Blackout on or off            |
//!
//! `{list}` is the cue list's number or its name (any case, spaces allowed
//! as `_` or `-`). Buttons on most controllers send 1 when pressed and 0 when
//! let go, so a trigger whose first argument is 0 is ignored.

use std::net::{Ipv4Addr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::engine::{lock, State};
use crate::error::{Error, Result};
use crate::playback::CueList;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OscSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for OscSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8000,
        }
    }
}

/// What the listener is doing, for the UI.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OscStatus {
    pub listening: bool,
    pub error: Option<String>,
    pub received: u64,
    /// The last message, as text, and what came of it.
    pub last: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Int(i32),
    Float(f32),
    Str(String),
    Bool(bool),
}

impl Arg {
    fn number(&self) -> Option<f32> {
        match self {
            Arg::Int(i) => Some(*i as f32),
            Arg::Float(f) => Some(*f),
            Arg::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Arg::Str(s) => s.trim().parse().ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub address: String,
    pub args: Vec<Arg>,
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.address)?;
        for arg in &self.args {
            match arg {
                Arg::Int(i) => write!(f, " {i}")?,
                Arg::Float(v) => write!(f, " {v}")?,
                Arg::Str(s) => write!(f, " \"{s}\"")?,
                Arg::Bool(b) => write!(f, " {b}")?,
            }
        }
        Ok(())
    }
}

// --- Parsing ---------------------------------------------------------------

/// Parses an OSC packet: one message, or a bundle of them (nested bundles too).
pub fn parse(packet: &[u8]) -> Result<Vec<Message>> {
    let mut out = Vec::new();
    parse_into(packet, &mut out, 0)?;
    Ok(out)
}

fn parse_into(packet: &[u8], out: &mut Vec<Message>, depth: u8) -> Result<()> {
    if depth > 8 {
        return Err(bad("bundles nested too deeply"));
    }
    if packet.starts_with(b"#bundle\0") {
        // 8 bytes of name, 8 of time tag, then size-prefixed elements.
        let mut at = 16;
        while at < packet.len() {
            let size = read_i32(packet, at)? as usize;
            at += 4;
            let element = packet
                .get(at..at + size)
                .ok_or_else(|| bad("bundle element runs past the end"))?;
            parse_into(element, out, depth + 1)?;
            at += size;
        }
        return Ok(());
    }

    let (address, mut at) = read_str(packet, 0)?;
    if !address.starts_with('/') {
        return Err(bad("address must start with /"));
    }
    let mut args = Vec::new();
    if at < packet.len() {
        let (tags, next) = read_str(packet, at)?;
        at = next;
        for tag in tags.strip_prefix(',').ok_or_else(|| bad("missing type tags"))?.chars() {
            match tag {
                'i' => {
                    args.push(Arg::Int(read_i32(packet, at)?));
                    at += 4;
                }
                'f' => {
                    args.push(Arg::Float(f32::from_bits(read_i32(packet, at)? as u32)));
                    at += 4;
                }
                's' | 'S' => {
                    let (s, next) = read_str(packet, at)?;
                    args.push(Arg::Str(s));
                    at = next;
                }
                'T' => args.push(Arg::Bool(true)),
                'F' => args.push(Arg::Bool(false)),
                'N' | 'I' => {}
                'h' | 't' | 'd' => at += 8,
                'b' => {
                    let size = read_i32(packet, at)? as usize;
                    at += 4 + size.div_ceil(4) * 4;
                }
                'c' | 'r' | 'm' => at += 4,
                other => return Err(bad(format!("unsupported argument type '{other}'"))),
            }
        }
    }
    out.push(Message { address, args });
    Ok(())
}

fn read_i32(packet: &[u8], at: usize) -> Result<i32> {
    let bytes = packet.get(at..at + 4).ok_or_else(|| bad("message ends early"))?;
    Ok(i32::from_be_bytes(bytes.try_into().expect("four bytes")))
}

/// Reads a null-terminated string padded to four bytes; returns it and the offset after it.
fn read_str(packet: &[u8], at: usize) -> Result<(String, usize)> {
    let rest = packet.get(at..).ok_or_else(|| bad("message ends early"))?;
    let end = rest
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| bad("unterminated string"))?;
    let text = std::str::from_utf8(&rest[..end]).map_err(|_| bad("string is not UTF-8"))?;
    Ok((text.to_string(), at + (end + 4) / 4 * 4))
}

fn bad(message: impl Into<String>) -> Error {
    Error::InvalidOsc(message.into())
}

// --- Commands --------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Go(String),
    Back(String),
    Release(String),
    /// A cue number, counted from 1.
    GoTo(String, usize),
    Master(String, f32),
    ReleaseAll,
    Blackout(bool),
}

/// Works out what a message asks for. `Ok(None)` means the message is ours but
/// needs nothing doing, such as a button being let go.
pub fn command(message: &Message) -> Result<Option<Command>> {
    let parts: Vec<&str> = message.address.trim_matches('/').split('/').collect();
    let first = message.args.first().and_then(Arg::number);
    let pressed = first.is_none_or(|v| v != 0.0);
    let unknown = || Err(bad(format!("unknown address {}", message.address)));

    let command = match parts.as_slice() {
        ["humlx", "releaseall"] => pressed.then_some(Command::ReleaseAll),
        ["humlx", "blackout"] => Some(Command::Blackout(first.is_none_or(|v| v != 0.0))),
        ["humlx", "cuelist", list, action @ ..] => {
            let list = list.to_string();
            match action {
                ["go"] => pressed.then_some(Command::Go(list)),
                ["back"] => pressed.then_some(Command::Back(list)),
                ["release"] => pressed.then_some(Command::Release(list)),
                ["goto"] => {
                    let cue = first.ok_or_else(|| bad("goto needs a cue number"))?;
                    (cue >= 1.0).then(|| Command::GoTo(list, cue.round() as usize))
                }
                ["goto", n] => {
                    let cue: usize = n.parse().map_err(|_| bad(format!("\"{n}\" is not a cue number")))?;
                    (pressed && cue >= 1).then_some(Command::GoTo(list, cue))
                }
                ["master"] => {
                    let level = first.ok_or_else(|| bad("master needs a level from 0 to 1"))?;
                    Some(Command::Master(list, level.clamp(0.0, 1.0)))
                }
                _ => return unknown(),
            }
        }
        _ => return unknown(),
    };
    Ok(command)
}

/// Finds a cue list by number or by name, ignoring case and treating `_` and
/// `-` as spaces.
pub fn find_list<'a>(lists: &'a [CueList], key: &str) -> Option<&'a CueList> {
    if let Ok(id) = key.parse::<u32>() {
        if let Some(list) = lists.iter().find(|l| l.id == id) {
            return Some(list);
        }
    }
    let normal = |s: &str| s.trim().to_lowercase().replace(['_', '-'], " ");
    let key = normal(key);
    lists.iter().find(|l| normal(&l.name) == key)
}

pub fn apply(state: &mut State, command: &Command, now: Instant) -> Result<()> {
    let list = |key: &str| {
        find_list(&state.cue_lists, key)
            .cloned()
            .ok_or_else(|| bad(format!("no cue list \"{key}\"")))
    };
    match command {
        Command::Go(key) => {
            let l = list(key)?;
            state.playbacks.go(&l, now)
        }
        Command::Back(key) => {
            let l = list(key)?;
            state.playbacks.back(&l, now)
        }
        Command::Release(key) => {
            let l = list(key)?;
            state.playbacks.release(l.id);
            Ok(())
        }
        Command::GoTo(key, cue) => {
            let l = list(key)?;
            state.playbacks.go_to(&l, cue - 1, now)
        }
        Command::Master(key, level) => {
            let l = list(key)?;
            state.playbacks.set_master(l.id, *level);
            Ok(())
        }
        Command::ReleaseAll => {
            state.playbacks.release_all();
            state.running_effects.stop_all();
            Ok(())
        }
        Command::Blackout(on) => {
            state.blackout = *on;
            Ok(())
        }
    }
}

/// Handles one packet and records it in the status.
pub fn handle(state: &mut State, packet: &[u8], now: Instant) {
    let messages = match parse(packet) {
        Ok(messages) => messages,
        Err(error) => {
            state.osc.received += 1;
            state.osc.last = Some(format!("Unreadable message: {error}"));
            return;
        }
    };
    for message in messages {
        state.osc.received += 1;
        let result = command(&message).and_then(|c| match c {
            Some(c) => apply(state, &c, now),
            None => Ok(()),
        });
        state.osc.last = Some(match result {
            Ok(()) => message.to_string(),
            Err(error) => format!("{message} ({error})"),
        });
    }
}

// --- Listener --------------------------------------------------------------

/// Listens for OSC on a UDP port until dropped.
pub struct Listener {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// Starts listening, recording success or failure in the state's OSC status.
    pub fn start(state: Arc<Mutex<State>>, settings: &OscSettings) -> Option<Self> {
        let mut status = OscStatus::default();
        let result = if settings.enabled {
            UdpSocket::bind((Ipv4Addr::UNSPECIFIED, settings.port))
                .and_then(|s| s.set_read_timeout(Some(Duration::from_millis(200))).map(|_| s))
                .map(Some)
        } else {
            Ok(None)
        };
        let listener = match result {
            Ok(Some(socket)) => {
                status.listening = true;
                let running = Arc::new(AtomicBool::new(true));
                let thread = std::thread::Builder::new()
                    .name("osc-input".into())
                    .spawn({
                        let running = Arc::clone(&running);
                        let state = Arc::clone(&state);
                        move || receive_loop(&socket, &state, &running)
                    })
                    .ok();
                Some(Self { running, thread })
            }
            Ok(None) => None,
            Err(error) => {
                status.error = Some(format!("Couldn't listen on port {}: {error}", settings.port));
                None
            }
        };
        lock(&state).osc = status;
        listener
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn receive_loop(socket: &UdpSocket, state: &Mutex<State>, running: &AtomicBool) {
    let mut buffer = [0u8; 4096];
    while running.load(Ordering::Relaxed) {
        match socket.recv_from(&mut buffer) {
            Ok((size, _)) => handle(&mut lock(state), &buffer[..size], Instant::now()),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => {
                lock(state).osc.error = Some(error.to_string());
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::Library;
    use crate::playback::Cue;
    use crate::programmer::Values;

    /// Encodes a message the way OSC senders do.
    fn encode(address: &str, args: &[Arg]) -> Vec<u8> {
        fn pad_str(out: &mut Vec<u8>, s: &str) {
            out.extend_from_slice(s.as_bytes());
            out.push(0);
            while !out.len().is_multiple_of(4) {
                out.push(0);
            }
        }
        let mut out = Vec::new();
        pad_str(&mut out, address);
        let tags: String = std::iter::once(',')
            .chain(args.iter().map(|a| match a {
                Arg::Int(_) => 'i',
                Arg::Float(_) => 'f',
                Arg::Str(_) => 's',
                Arg::Bool(true) => 'T',
                Arg::Bool(false) => 'F',
            }))
            .collect();
        pad_str(&mut out, &tags);
        for a in args {
            match a {
                Arg::Int(i) => out.extend_from_slice(&i.to_be_bytes()),
                Arg::Float(f) => out.extend_from_slice(&f.to_bits().to_be_bytes()),
                Arg::Str(s) => pad_str(&mut out, s),
                Arg::Bool(_) => {}
            }
        }
        out
    }

    fn state() -> State {
        let mut state = State::new(Library::with_bundled(), Default::default(), Default::default());
        let cue = |name: &str| Cue {
            name: name.into(),
            fade: 0.0,
            mode: Default::default(),
            follow: 0.0,
            values: [(1, Values::default())].into(),
        };
        state.cue_lists.push(CueList {
            id: 3,
            name: "Walk In".into(),
            cues: vec![cue("A"), cue("B"), cue("C")],
        });
        state
    }

    #[test]
    fn parses_messages_and_bundles() {
        let message = encode(
            "/humlx/cuelist/1/goto",
            &[Arg::Int(2), Arg::Str("hi".into()), Arg::Float(0.5)],
        );
        let parsed = parse(&message).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].address, "/humlx/cuelist/1/goto");
        assert_eq!(
            parsed[0].args,
            vec![Arg::Int(2), Arg::Str("hi".into()), Arg::Float(0.5)]
        );

        let mut bundle = b"#bundle\0".to_vec();
        bundle.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]);
        for m in [encode("/a", &[]), encode("/b", &[Arg::Bool(true)])] {
            bundle.extend_from_slice(&(m.len() as i32).to_be_bytes());
            bundle.extend_from_slice(&m);
        }
        let parsed = parse(&bundle).unwrap();
        assert_eq!(
            parsed.iter().map(|m| m.address.as_str()).collect::<Vec<_>>(),
            vec!["/a", "/b"]
        );

        // An address with no type tags at all, as some senders do.
        assert_eq!(parse(b"/humlx/releaseall\0\0\0").unwrap()[0].args, vec![]);
        assert!(parse(b"/oops").is_err());
        assert!(parse(&encode("/x", &[Arg::Int(1)])[..10]).is_err());
    }

    #[test]
    fn reads_commands() {
        let msg = |address: &str, args: Vec<Arg>| Message {
            address: address.into(),
            args,
        };
        assert_eq!(
            command(&msg("/humlx/cuelist/1/go", vec![])).unwrap(),
            Some(Command::Go("1".into()))
        );
        // Button released: nothing to do.
        assert_eq!(
            command(&msg("/humlx/cuelist/1/go", vec![Arg::Float(0.0)])).unwrap(),
            None
        );
        assert_eq!(
            command(&msg("/humlx/cuelist/walk_in/goto", vec![Arg::Int(2)])).unwrap(),
            Some(Command::GoTo("walk_in".into(), 2))
        );
        assert_eq!(
            command(&msg("/humlx/cuelist/1/goto/3", vec![Arg::Float(1.0)])).unwrap(),
            Some(Command::GoTo("1".into(), 3))
        );
        assert_eq!(
            command(&msg("/humlx/cuelist/1/master", vec![Arg::Float(1.5)])).unwrap(),
            Some(Command::Master("1".into(), 1.0))
        );
        assert_eq!(
            command(&msg("/humlx/blackout", vec![Arg::Int(0)])).unwrap(),
            Some(Command::Blackout(false))
        );
        assert!(command(&msg("/humlx/cuelist/1/dance", vec![])).is_err());
        assert!(command(&msg("/other/thing", vec![])).is_err());
    }

    #[test]
    fn runs_cue_lists_by_number_or_name() {
        let mut state = state();
        let now = Instant::now();
        handle(&mut state, &encode("/humlx/cuelist/3/go", &[]), now);
        assert_eq!(state.playbacks.get(3).unwrap().current, 0);
        handle(
            &mut state,
            &encode("/humlx/cuelist/walk-in/go", &[Arg::Float(1.0)]),
            now,
        );
        assert_eq!(state.playbacks.get(3).unwrap().current, 1);
        handle(&mut state, &encode("/humlx/cuelist/WALK_IN/goto", &[Arg::Int(3)]), now);
        assert_eq!(state.playbacks.get(3).unwrap().current, 2);
        handle(&mut state, &encode("/humlx/cuelist/3/master", &[Arg::Float(0.25)]), now);
        assert_eq!(state.playbacks.master(3), 0.25);
        handle(&mut state, &encode("/humlx/cuelist/3/release", &[]), now);
        assert!(state.playbacks.get(3).is_none());
        assert_eq!(state.osc.received, 5);

        handle(&mut state, &encode("/humlx/cuelist/9/go", &[]), now);
        assert!(state.osc.last.as_deref().unwrap().contains("no cue list"));
    }

    #[test]
    fn listener_receives_over_udp() {
        let state = Arc::new(Mutex::new(state()));
        // Find a free port, then listen on it.
        let port = UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let settings = OscSettings { enabled: true, port };
        let listener = Listener::start(Arc::clone(&state), &settings).expect("listening");
        assert!(lock(&state).osc.listening);

        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender
            .send_to(&encode("/humlx/cuelist/3/go", &[]), ("127.0.0.1", port))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while lock(&state).osc.received == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(listener);
        let state = lock(&state);
        assert_eq!(state.osc.received, 1);
        assert!(state.playbacks.get(3).is_some());
    }
}
