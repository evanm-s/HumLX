//! Drives Resolume (Arena or Avenue) over OSC from Resolume fixtures in the
//! patch. Each one is a fixture like any other: its intensity, after the
//! grand master and blackout, becomes the composition master, and its column
//! channel triggers a composition column, picked like a gobo.
//!
//! | Sent when                  | Address                             | Argument     |
//! | -------------------------- | ----------------------------------- | ------------ |
//! | intensity changes          | `/composition/master`               | 0.0 to 1.0   |
//! | column changes to `n`      | `/composition/columns/{n}/connect`  | 1            |
//! | column changes to Clear    | `/composition/disconnectall`        | 1            |
//!
//! A column is only triggered when it changes, so a cue that keeps the same
//! column never restarts it. The master is also sent again every second, so
//! Resolume picks it up after a restart.

use std::collections::HashMap;
use std::net::{SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use crate::dmx::Universe;
use crate::fixture::{Attribute, Library, CLEAR_COLUMN, RESOLUME};
use crate::osc::{encode, Arg};
use crate::patch::{mode_of, Patch};

/// How often the master is sent when it isn't changing.
const RESEND: Duration = Duration::from_secs(1);

/// What one Resolume fixture is putting out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Feed {
    pub fixture: u32,
    pub target: SocketAddrV4,
    /// Composition master, 0 to 1.
    pub master: f32,
    /// 0 for none, a column number, or [`CLEAR_COLUMN`].
    pub column: u8,
}

/// Reads every Resolume fixture's levels from the universe as it goes out.
pub fn feeds(patch: &Patch, library: &Library, universe: &Universe) -> Vec<Feed> {
    patch
        .fixtures()
        .iter()
        .filter(|f| f.profile == RESOLUME)
        .filter_map(|fixture| {
            let mode = mode_of(library, fixture)?;
            let level = |i: usize| universe.get(fixture.address + i as u16);
            let mut coarse = None;
            let mut fine = 0u8;
            let mut column = 0u8;
            for (i, channel) in mode.channels.iter().enumerate() {
                match channel.attribute {
                    Attribute::Intensity if channel.fine_of.is_some() => fine = level(i),
                    Attribute::Intensity => coarse = Some(level(i)),
                    Attribute::Column => column = level(i),
                    _ => {}
                }
            }
            let master = coarse.map_or(0.0, |c| f32::from(u16::from(c) << 8 | u16::from(fine)) / 65535.0);
            Some(Feed {
                fixture: fixture.id,
                target: SocketAddrV4::new(fixture.options.osc_host, fixture.options.osc_port),
                master,
                column,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct Sent {
    target: SocketAddrV4,
    master: f32,
    master_at: Instant,
    column: u8,
}

/// Remembers what each fixture last sent, so only changes go out.
#[derive(Debug, Default)]
pub struct Sender {
    sent: HashMap<u32, Sent>,
}

impl Sender {
    /// The OSC packets to send for this frame, with where each goes.
    pub fn packets(&mut self, feeds: &[Feed], now: Instant) -> Vec<(SocketAddr, Vec<u8>)> {
        self.sent.retain(|id, _| feeds.iter().any(|f| f.fixture == *id));
        let mut out = Vec::new();
        for feed in feeds {
            let previous = self.sent.get(&feed.fixture).filter(|s| s.target == feed.target);
            let to = SocketAddr::V4(feed.target);
            let master_due = previous.is_none_or(|s| {
                (s.master - feed.master).abs() > 1e-5 || now.saturating_duration_since(s.master_at) >= RESEND
            });
            if master_due {
                out.push((to, encode("/composition/master", &[Arg::Float(feed.master)])));
            }
            if previous.is_none_or(|s| s.column != feed.column) {
                match feed.column {
                    0 => {}
                    CLEAR_COLUMN => out.push((to, encode("/composition/disconnectall", &[Arg::Int(1)]))),
                    n => out.push((to, encode(&format!("/composition/columns/{n}/connect"), &[Arg::Int(1)]))),
                }
            }
            self.sent.insert(
                feed.fixture,
                Sent {
                    target: feed.target,
                    master: feed.master,
                    master_at: if master_due {
                        now
                    } else {
                        previous.map_or(now, |s| s.master_at)
                    },
                    column: feed.column,
                },
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::State;
    use crate::osc::{parse, Message};
    use crate::patch::NewFixtures;
    use crate::programmer::{SelectMode, Values};

    fn decoded(packets: &[(SocketAddr, Vec<u8>)]) -> Vec<String> {
        packets
            .iter()
            .flat_map(|(_, p)| parse(p).unwrap())
            .map(|m: Message| m.to_string())
            .collect()
    }

    #[test]
    fn sends_master_and_column_changes() {
        let mut state = State::default();
        state.library = Library::with_bundled();
        state
            .patch
            .add(
                &state.library,
                &NewFixtures {
                    profile: RESOLUME.into(),
                    mode: "OSC".into(),
                    count: 1,
                    first_id: Some(1),
                    address: Some(500),
                    name: None,
                },
            )
            .unwrap();
        state.programmer.select(&[1], SelectMode::Set);
        state.programmer.values.insert(
            1,
            Values {
                intensity: Some(0.5),
                other: [(Attribute::Column, 3.0 / 255.0)].into(),
                ..Default::default()
            },
        );
        state.compose();
        let feed = feeds(&state.patch, &state.library, state.universe());
        assert_eq!(feed.len(), 1);
        assert!((feed[0].master - 0.5).abs() < 0.001);
        assert_eq!(feed[0].column, 3);
        assert_eq!(feed[0].target.port(), 7000);

        let mut sender = Sender::default();
        let now = Instant::now();
        let first = decoded(&sender.packets(&feed, now));
        assert_eq!(first.len(), 2);
        assert!(first[0].starts_with("/composition/master 0.5"));
        assert_eq!(first[1], "/composition/columns/3/connect 1");
        // Nothing changed: nothing sent, until the master is due again.
        assert!(sender.packets(&feed, now).is_empty());
        assert_eq!(decoded(&sender.packets(&feed, now + RESEND)).len(), 1);

        // Grand master scales the master; Clear disconnects.
        state.grand_master.set(0.5);
        state
            .programmer
            .values
            .get_mut(&1)
            .unwrap()
            .other
            .insert(Attribute::Column, 1.0);
        state.compose();
        let feed = feeds(&state.patch, &state.library, state.universe());
        let sent = decoded(&sender.packets(&feed, now + RESEND));
        assert!(sent[0].starts_with("/composition/master 0.25"), "{sent:?}");
        assert_eq!(sent[1], "/composition/disconnectall 1");
    }
}
