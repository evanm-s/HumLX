//! Decoding for the Obsidian NX-K control surface (keypad and four encoders).
//!
//! The NX-K is a vendor-specific USB device (`11be:e102`) with one interrupt
//! endpoint that sends a packet whenever a key or encoder changes; no
//! handshake is needed. Worked out from captures of a real unit:
//!
//! | Packet                         | Meaning                                   |
//! | ------------------------------ | ----------------------------------------- |
//! | `01 a b c s`                   | Key `a b c` pressed (`s` = 1) or let go   |
//! | `02 42 e 59 lo hi 00`          | Encoder `e` turned by signed `hi lo`      |
//!
//! Encoders are `02`, `12`, `22` and `32` from left to right. Empty packets
//! arrive between real ones and mean nothing.

use std::time::{Duration, Instant};

use serde::Serialize;

pub const VENDOR_ID: u16 = 0x11be;
pub const PRODUCT_ID: u16 = 0xe102;
/// Interface 0 sends nothing until this alternate setting is chosen.
pub const ALT_SETTING: u8 = 1;
pub const ENDPOINT: u8 = 0x82;

/// Every key on the surface, by the code its packets carry.
const KEYS: &[([u8; 3], &str)] = &[
    ([0x02, 0x01, 0x54], "record"),
    ([0x02, 0x02, 0x54], "update"),
    ([0x02, 0x11, 0x54], "load"),
    ([0x02, 0x12, 0x54], "group"),
    ([0x02, 0x13, 0x54], "cue"),
    ([0x42, 0x13, 0x52], "enter"),
    ([0x42, 0x00, 0x52], "0"),
    ([0x42, 0x01, 0x52], "1"),
    ([0x42, 0x02, 0x52], "2"),
    ([0x42, 0x03, 0x52], "3"),
    ([0x42, 0x04, 0x52], "4"),
    ([0x42, 0x05, 0x52], "5"),
    ([0x42, 0x06, 0x52], "6"),
    ([0x42, 0x07, 0x52], "7"),
    ([0x42, 0x08, 0x52], "8"),
    ([0x42, 0x09, 0x52], "9"),
    ([0x42, 0x10, 0x52], "minus"),
    ([0x42, 0x11, 0x52], "plus"),
    ([0x42, 0x12, 0x52], "point"),
    ([0x42, 0x14, 0x52], "slash"),
    ([0x42, 0x16, 0x52], "at"),
    ([0x02, 0x15, 0x52], "backspace"),
    ([0x42, 0x01, 0x53], "full"),
    ([0x42, 0x02, 0x53], "thru"),
    ([0x02, 0x01, 0x60], "highlight"),
    ([0x02, 0x01, 0x51], "edit"),
    ([0x02, 0x02, 0x51], "undo"),
    ([0x02, 0x03, 0x51], "clear"),
    ([0x02, 0x04, 0x51], "copy"),
    ([0x02, 0x06, 0x51], "move"),
    ([0x02, 0x07, 0x51], "delete"),
    ([0x02, 0x01, 0x20], "macro"),
    ([0x02, 0x02, 0x20], "preview"),
    ([0x02, 0x03, 0x20], "menu"),
    ([0x02, 0x21, 0x43], "fade"),
    ([0x02, 0x22, 0x43], "delay"),
    ([0x02, 0x31, 0x43], "snapshot"),
    ([0x02, 0x32, 0x43], "bank"),
    ([0x02, 0x01, 0x64], "last"),
    ([0x02, 0x02, 0x64], "next"),
    ([0x02, 0x11, 0x64], "swap"),
    ([0x02, 0x08, 0x61], "link"),
    ([0x02, 0x01, 0x59], "encoder1"),
    ([0x02, 0x11, 0x59], "encoder2"),
    ([0x02, 0x21, 0x59], "encoder3"),
    ([0x02, 0x31, 0x59], "encoder4"),
];

const ENCODERS: [u8; 4] = [0x02, 0x12, 0x22, 0x32];

/// Something done on the surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Event {
    /// A key pressed (`down`) or let go. Unknown keys are named by their code,
    /// such as `"key-02-05-51"`.
    Key { key: String, down: bool },
    /// Encoder 1 to 4 turned; positive is clockwise.
    Encoder { index: u8, delta: i16 },
}

/// Reads one packet. Returns `None` for empty or unrecognised packets.
pub fn parse(packet: &[u8]) -> Option<Event> {
    match *packet {
        [0x01, a, b, c, state] => {
            let code = [a, b, c];
            let key = KEYS
                .iter()
                .find(|(k, _)| *k == code)
                .map(|(_, name)| (*name).to_string())
                .unwrap_or_else(|| format!("key-{a:02x}-{b:02x}-{c:02x}"));
            Some(Event::Key { key, down: state != 0 })
        }
        [0x02, 0x42, id, 0x59, lo, hi, _] => {
            let index = ENCODERS.iter().position(|e| *e == id)? as u8 + 1;
            let delta = i16::from_le_bytes([lo, hi]);
            (delta != 0).then_some(Event::Encoder { index, delta })
        }
        _ => None,
    }
}

/// Steadies encoder readings. Worn encoders flick back and forth by one count
/// while barely touched; counts are collected and only passed on once they
/// add up to [`Smoother::STEP`] in one direction, and leftovers are dropped
/// after a pause so stray flicks never add up.
#[derive(Debug, Default)]
pub struct Smoother {
    pending: [(i32, Option<Instant>); 4],
}

impl Smoother {
    pub const STEP: i32 = 2;
    const IDLE: Duration = Duration::from_millis(250);

    /// Takes an encoder's raw counts and returns the steps to act on, if any.
    pub fn feed(&mut self, index: u8, delta: i16, now: Instant) -> Option<i16> {
        let (sum, last) = self.pending.get_mut(usize::from(index).checked_sub(1)?)?;
        if last.is_some_and(|t| now.duration_since(t) > Self::IDLE) {
            *sum = 0;
        }
        *last = Some(now);
        *sum += i32::from(delta);
        let steps = *sum / Self::STEP;
        *sum -= steps * Self::STEP;
        (steps != 0).then_some(steps as i16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str, down: bool) -> Option<Event> {
        Some(Event::Key { key: name.into(), down })
    }

    #[test]
    fn reads_keys() {
        assert_eq!(parse(&[0x01, 0x02, 0x01, 0x54, 0x01]), key("record", true));
        assert_eq!(parse(&[0x01, 0x02, 0x01, 0x54, 0x00]), key("record", false));
        assert_eq!(parse(&[0x01, 0x42, 0x07, 0x52, 0x01]), key("7", true));
        assert_eq!(parse(&[0x01, 0x42, 0x02, 0x53, 0x01]), key("thru", true));
        assert_eq!(parse(&[0x01, 0x02, 0x11, 0x59, 0x01]), key("encoder2", true));
        assert_eq!(parse(&[0x01, 0x02, 0x05, 0x51, 0x01]), key("key-02-05-51", true));
    }

    #[test]
    fn reads_encoders() {
        let turn = |index, delta| Some(Event::Encoder { index, delta });
        assert_eq!(parse(&[0x02, 0x42, 0x02, 0x59, 0x04, 0x00, 0x00]), turn(1, 4));
        assert_eq!(parse(&[0x02, 0x42, 0x12, 0x59, 0xfd, 0xff, 0x00]), turn(2, -3));
        assert_eq!(parse(&[0x02, 0x42, 0x32, 0x59, 0x01, 0x00, 0x00]), turn(4, 1));
        assert_eq!(parse(&[0x02, 0x42, 0x22, 0x59, 0x00, 0x00, 0x00]), None);
    }

    #[test]
    fn ignores_noise() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0x03, 0x00]), None);
        assert_eq!(parse(&[0x02, 0x42, 0x09, 0x59, 0x01, 0x00, 0x00]), None);
    }

    #[test]
    fn smoother_drops_jitter_and_keeps_turns() {
        let mut s = Smoother::default();
        let t = Instant::now();
        let ms = |n| t + Duration::from_millis(n);
        // Flicking back and forth goes nowhere.
        for (i, d) in [1, -1, 1, -1, 1, -1].into_iter().enumerate() {
            assert_eq!(s.feed(2, d, ms(i as u64 * 8)), None);
        }
        // A real turn comes through at half the counts.
        assert_eq!(s.feed(2, 4, ms(60)), Some(2));
        assert_eq!(s.feed(2, 1, ms(68)), None);
        assert_eq!(s.feed(2, 1, ms(76)), Some(1));
        assert_eq!(s.feed(2, -6, ms(84)), Some(-3));
        // A stray count after a pause does not join an old leftover.
        assert_eq!(s.feed(1, 1, ms(0)), None);
        assert_eq!(s.feed(1, 1, ms(1000)), None);
        assert_eq!(s.feed(5, 10, ms(0)), None);
    }
}
