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
use crate::effect::{Effect, EffectSize, SpeedMasters};
use crate::error::{Error, Result};
use crate::fixture::{Attribute, Library, Mode};
use crate::layout::Layout;
use crate::osc::{OscSettings, OscStatus};
use crate::patch::{mode_of, Patch};
use crate::playback::{CueList, Playback, Playbacks, RunningEffects};
use crate::programmer::{next_id, Group, Preset, Programmer, Rgb, Values};

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

/// The grand master level, from 0 to 1. Full unless someone pulls it down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrandMaster(pub f32);

impl Default for GrandMaster {
    fn default() -> Self {
        Self(1.0)
    }
}

impl GrandMaster {
    pub fn set(&mut self, level: f32) {
        self.0 = if level.is_finite() { level.clamp(0.0, 1.0) } else { 1.0 };
    }
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
    /// Scales every fixture's intensity, from 0 to 1, after everything else.
    /// Colour, position and beam are left alone.
    pub grand_master: GrandMaster,
    /// Picks out the selected fixtures: full, in the highlight colour and
    /// tight, with every other fixture out. A temporary override that is never
    /// recorded.
    pub highlight: bool,
    /// Colour that highlighted fixtures turn, white unless changed in Settings.
    pub highlight_colour: Rgb,
    pub programmer: Programmer,
    pub groups: Vec<Group>,
    pub presets: Vec<Preset>,
    pub cue_lists: Vec<CueList>,
    /// Where cue lists sit on the Playback screen, and which is selected.
    pub layout: Layout,
    pub playbacks: Playbacks,
    /// When each programmer effect started, by its number.
    pub running_effects: RunningEffects,
    /// Scales the size of every effect, in cues and the programmer.
    pub effect_size: EffectSize,
    /// Tempos that effects can follow.
    pub speed_masters: SpeedMasters,
    pub osc_settings: OscSettings,
    pub osc: OscStatus,
    /// Raw channel levels set from the channel test screen, by fixture
    /// number and channel index within the fixture.
    test_levels: HashMap<u32, HashMap<usize, u8>>,
    universe: Universe,
}

impl State {
    fn cue_list(&self, id: u32) -> Result<CueList> {
        self.cue_lists
            .iter()
            .find(|l| l.id == id)
            .cloned()
            .ok_or(Error::UnknownCueList(id))
    }

    /// GO on a cue list, fading out the lists it replaces (see
    /// [`Playbacks::release_others`]).
    pub fn go(&mut self, list: u32, now: Instant) -> Result<()> {
        let list = self.cue_list(list)?;
        self.playbacks.go(&list, now)?;
        self.playbacks.release_others(&list, &self.cue_lists, now);
        Ok(())
    }

    /// Goes to cue `index` (from 0) of a cue list, fading out the lists it replaces.
    pub fn go_to(&mut self, list: u32, index: usize, now: Instant) -> Result<()> {
        let list = self.cue_list(list)?;
        self.playbacks.go_to(&list, index, now)?;
        self.playbacks.release_others(&list, &self.cue_lists, now);
        Ok(())
    }

    /// Fades a running list out over the fade of the cue it's on.
    pub fn fade_out(&mut self, list: u32, now: Instant) {
        let fade = self
            .playbacks
            .get(list)
            .and_then(|p| self.cue_lists.iter().find(|l| l.id == list)?.cues.get(p.current))
            .map_or(0.0, |c| c.fade.max(0.0));
        self.playbacks.fade_out(list, Duration::from_secs_f32(fade), now);
    }

    /// A tap of a grid button: selects its list, then fades a live one-cue
    /// look out, or else GOes.
    pub fn tap(&mut self, list: u32, now: Instant) -> Result<()> {
        let looks = self.cue_list(list)?.cues.len() == 1;
        self.layout.selected = Some(list);
        if looks && self.playbacks.get(list).is_some_and(|p| !p.releasing) {
            self.fade_out(list, now);
            Ok(())
        } else {
            self.go(list, now)
        }
    }

    /// Load: copies what running cue lists are putting out for `fixtures`
    /// (every fixture when `None`) into the programmer, so it can be tweaked
    /// and recorded. Values are as on stage right now, mid-fade included and
    /// intensity scaled by each list's master, but before the grand master
    /// and blackout. The cues' effects come along for those fixtures.
    /// Programmer values already set stay on top, as they're what's on stage.
    /// Returns how many fixtures got values.
    pub fn load_live(&mut self, fixtures: Option<&[u32]>, now: Instant) -> Result<usize> {
        let wanted = |id: u32| fixtures.is_none_or(|f| f.contains(&id)) && self.patch.get(id).is_some();
        let live: Vec<(u32, Values)> = self
            .playbacks
            .values(now)
            .into_iter()
            .filter(|(id, values)| wanted(*id) && !values.is_empty())
            .collect();
        let mut playbacks: Vec<&Playback> = self
            .playbacks
            .running()
            .map(|(_, p)| p)
            .filter(|p| !p.releasing)
            .collect();
        playbacks.sort_by_key(|p| p.order);
        let effects: Vec<Effect> = playbacks
            .iter()
            .flat_map(|p| p.effects())
            .filter_map(|effect| {
                let mut effect = effect.clone();
                effect.fixtures.retain(|id| wanted(*id));
                (!effect.fixtures.is_empty()).then_some(effect)
            })
            .collect();
        if live.is_empty() && effects.is_empty() {
            return Err(Error::NothingLive);
        }
        let count = live.len();
        for (id, values) in live {
            let entry = self.programmer.values.entry(id).or_default();
            let mut loaded = values;
            loaded.merge(entry);
            *entry = loaded;
        }
        for mut effect in effects {
            effect.id = next_id(self.programmer.effects.iter().map(|e| e.id));
            crate::effect::lay_over(&mut self.programmer.effects, effect);
        }
        Ok(count)
    }

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
        self.compose_at(Instant::now())
    }

    /// Works out the universe as it should be at `now`, which drives fades and effects.
    pub fn compose_at(&mut self, now: Instant) -> &Universe {
        // Follow cues move on first, so this frame shows the cue they went to.
        self.playbacks.follow(&self.cue_lists, now);
        // Playbacks with their effects, then programmer values on top (which
        // override a cue's effect on what they set), then programmer effects.
        let size = self.effect_size.0.clamp(0.0, 1.0);
        let beats = self.speed_masters.beats(now);
        let mut look = self.playbacks.output_with_effects(now, size, &beats);
        for (fixture, values) in &self.programmer.values {
            look.entry(*fixture).or_default().merge(values);
        }
        self.running_effects.start_new(&self.programmer.effects, now);
        self.running_effects
            .apply(&self.programmer.effects, &mut look, now, &beats, size);
        if self.highlight {
            for fixture in self.patch.fixtures() {
                let values = look.entry(fixture.id).or_default();
                if self.programmer.selection.contains(&fixture.id) {
                    let tight = mode_of(&self.library, fixture).map_or(0.0, tightest_zoom);
                    values.merge(&Values {
                        intensity: Some(1.0),
                        colour: Some(self.highlight_colour),
                        other: [(Attribute::Zoom, tight)].into(),
                        ..Default::default()
                    });
                } else {
                    values.intensity = Some(0.0);
                }
            }
        }

        let mut universe = Universe::default();
        for fixture in self.patch.fixtures() {
            let Some(mode) = mode_of(&self.library, fixture) else {
                continue;
            };
            let tests = self.test_levels.get(&fixture.id);
            let programmed = look.get(&fixture.id).map(|v| v.render(mode));
            // Without a dimmer channel, blackout has to pull the colour mix down instead.
            let has_dimmer = mode.channels.iter().any(|c| c.attribute == Attribute::Intensity);
            let highlighted = self.highlight && self.programmer.selection.contains(&fixture.id);
            for (i, channel) in mode.channels.iter().enumerate() {
                // Channel test levels win over the programmer and playbacks, which win over resting levels.
                let mut value = tests
                    .and_then(|t| t.get(&i))
                    .copied()
                    .or_else(|| programmed.as_ref().and_then(|p| p[i]))
                    .unwrap_or(channel.default);
                // A highlighted fixture's shutter opens, even if a cue left it strobing or closed.
                if highlighted && channel.attribute == Attribute::Shutter && tests.is_none_or(|t| !t.contains_key(&i)) {
                    value = channel.highlight_value();
                }
                let dims = if has_dimmer {
                    channel.attribute == Attribute::Intensity
                } else {
                    channel.attribute.is_colour_mix()
                };
                if dims {
                    if self.blackout {
                        value = 0;
                    } else if self.grand_master.0 < 1.0 {
                        value = (f32::from(value) * self.grand_master.0).round() as u8;
                    }
                }
                universe.set(fixture.address + i as u16, value);
            }
        }
        self.universe = universe;
        &self.universe
    }
}

/// The zoom level, from 0 to 1, that gives `mode` its narrowest beam. Most
/// fixtures zoom from narrow to wide, so 0 unless the profile says otherwise.
fn tightest_zoom(mode: &Mode) -> f32 {
    let reversed = mode
        .channels
        .iter()
        .filter(|c| c.attribute == Attribute::Zoom)
        .flat_map(|c| &c.capabilities)
        .any(|capability| {
            let label = capability.label.to_lowercase();
            label.contains("wide to narrow") || label.contains("big to small") || label.contains("large to small")
        });
    if reversed {
        1.0
    } else {
        0.0
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

    /// The shared state, for other threads such as the OSC listener.
    pub fn shared(&self) -> Arc<Mutex<State>> {
        Arc::clone(&self.state)
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

pub(crate) fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
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
    fn programmer_then_test_levels() {
        use crate::programmer::{Rgb, SelectMode};
        let mut state = state_with("generic/rgbw-par", "6ch DRGBW Strobe", 1);
        state.programmer.select(&[1], SelectMode::Set);
        state.programmer.update(|v| {
            v.intensity = Some(1.0);
            v.colour = Some(Rgb { r: 1.0, g: 0.0, b: 0.0 });
        });
        state.compose();
        assert_eq!(
            (1..=6).map(|a| state.universe().get(a)).collect::<Vec<_>>(),
            vec![255, 255, 0, 0, 0, 0]
        );

        state.set_test_level(1, 2, 77).unwrap();
        state.compose();
        assert_eq!(state.universe().get(3), 77);

        state.blackout = true;
        state.compose();
        assert_eq!(state.universe().get(1), 0);
    }

    #[test]
    fn load_pulls_live_values_into_programmer() {
        use crate::effect::{Effect, Shape};
        use crate::playback::{Cue, CueList};
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch
            .add(
                &library,
                &NewFixtures {
                    profile: "generic/dimmer".into(),
                    mode: "1ch".into(),
                    count: 3,
                    first_id: Some(1),
                    address: Some(1),
                    name: None,
                },
            )
            .unwrap();
        let mut state = State::new(library, patch, OutputSettings::default());
        let now = Instant::now();
        assert!(matches!(state.load_live(None, now), Err(Error::NothingLive)));

        let full = Values {
            intensity: Some(1.0),
            ..Default::default()
        };
        state.cue_lists.push(CueList {
            id: 1,
            name: "Main".into(),
            cues: vec![Cue {
                name: "Look".into(),
                fade: 0.0,
                values: [(1, full.clone()), (2, full.clone())].into(),
                effects: vec![Effect::new(1, "Chase".into(), Shape::Chase, &[], vec![1, 2, 3])],
                ..Default::default()
            }],
            ..Default::default()
        });
        state.go(1, now).unwrap();
        state.playbacks.set_master(1, 0.5);
        state.programmer.values.insert(
            2,
            Values {
                intensity: Some(0.1),
                ..Default::default()
            },
        );

        // Selected fixtures only: values as scaled by the master, and the
        // cue's effect on just those fixtures.
        assert_eq!(state.load_live(Some(&[1]), now).unwrap(), 1);
        assert_eq!(state.programmer.values[&1].intensity, Some(0.5));
        assert_eq!(state.programmer.values[&2].intensity, Some(0.1));
        assert_eq!(state.programmer.effects.len(), 1);
        assert_eq!(state.programmer.effects[0].fixtures, vec![1]);

        // Everything: programmer values already set stay on top, and the
        // effect now covers every fixture it runs on.
        assert_eq!(state.load_live(None, now).unwrap(), 2);
        assert_eq!(state.programmer.values[&2].intensity, Some(0.1));
        assert!(!state.programmer.values.contains_key(&3));
        assert_eq!(state.programmer.effects.len(), 1);
        assert_eq!(state.programmer.effects[0].fixtures, vec![1, 2, 3]);

        // Nothing live for an unlit fixture with no effect on it.
        state.playbacks.release_all();
        assert!(matches!(state.load_live(Some(&[3]), now), Err(Error::NothingLive)));
    }

    #[test]
    fn playback_under_programmer_and_effects() {
        use crate::effect::{Effect, EffectSize, Shape};
        use crate::playback::{Cue, CueList};
        use crate::programmer::{SelectMode, Values};
        let mut state = state_with("generic/dimmer", "1ch", 1);
        let chase = Effect::new(1, "Chase".into(), Shape::Chase, &[], vec![1]);
        let list = CueList {
            id: 1,
            name: "Main".into(),
            cues: vec![
                Cue {
                    name: "Warm".into(),
                    fade: 2.0,
                    values: [(
                        1,
                        Values {
                            intensity: Some(1.0),
                            ..Default::default()
                        },
                    )]
                    .into(),
                    ..Default::default()
                },
                Cue {
                    name: "Chase".into(),
                    fade: 0.0,
                    effects: vec![chase.clone()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        state.playbacks.go(&list, start).unwrap();
        state.compose_at(at(1000));
        assert_eq!(state.universe().get(1), 128); // halfway through the fade

        // The cue's effect keeps time from when the list started.
        state.playbacks.go(&list, at(2000)).unwrap();
        state.cue_lists.push(list);
        state.compose_at(at(2250));
        assert_eq!(state.universe().get(1), 255);
        state.compose_at(at(2750));
        assert_eq!(state.universe().get(1), 0); // in the off half of the chase

        state.effect_size = EffectSize(0.0);
        state.compose_at(at(2750));
        assert_eq!(state.universe().get(1), 255);
        state.effect_size = EffectSize(1.0);

        // A programmer value overrides the cue's effect.
        state.programmer.select(&[1], SelectMode::Set);
        state.programmer.update(|v| v.intensity = Some(0.25));
        state.compose_at(at(2750));
        assert_eq!(state.universe().get(1), 64);

        // A programmer effect runs on top.
        state.programmer.effects.push(chase);
        state.running_effects.start(1, start);
        state.compose_at(at(2750));
        assert_eq!(state.universe().get(1), 0);
    }

    #[test]
    fn highlight_picks_out_the_selection() {
        use crate::programmer::{Rgb, SelectMode};
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        for (profile, mode, id, address) in [
            ("generic/moving-head-wash", "12ch", 1, 1),
            ("generic/moving-head-wash", "12ch", 2, 13),
            ("generic/rgb-par", "3ch RGB", 3, 25),
        ] {
            patch
                .add(
                    &library,
                    &NewFixtures {
                        profile: profile.into(),
                        mode: mode.into(),
                        count: 1,
                        first_id: Some(id),
                        address: Some(address),
                        name: None,
                    },
                )
                .unwrap();
        }
        let mut state = State::new(library, patch, OutputSettings::default());
        state.programmer.select(&[1, 2, 3], SelectMode::Set);
        state.programmer.update(|v| {
            v.intensity = Some(0.5);
            v.colour = Some(Rgb { r: 1.0, g: 0.0, b: 0.0 });
            v.other.insert(Attribute::Shutter, 0.5); // strobing
        });
        state.programmer.select(&[1], SelectMode::Set);
        state.highlight = true;
        state.compose();
        let wash = |state: &State, address: u16| {
            (address..address + 12)
                .map(|a| state.universe().get(a))
                .collect::<Vec<_>>()
        };
        // Full, shutter open, white from the white emitter, zoom tight.
        assert_eq!(&wash(&state, 1)[5..], &[255, 255, 0, 0, 0, 255, 0]);
        // Everything else out, including a par with no dimmer.
        assert_eq!(wash(&state, 13)[5], 0);
        assert_eq!(
            (25..28).map(|a| state.universe().get(a)).collect::<Vec<_>>(),
            vec![0, 0, 0]
        );

        // Turning it off restores the programmer, which it never changed.
        state.highlight = false;
        state.compose();
        assert_eq!(&wash(&state, 1)[5..8], &[128, 128, 255]);
        assert_eq!(state.universe().get(25), 128);
        assert_eq!(state.programmer.values[&1].intensity, Some(0.5));

        // A highlight colour set in Settings replaces white.
        state.highlight_colour = Rgb { r: 1.0, g: 0.0, b: 1.0 };
        state.highlight = true;
        state.compose();
        assert_eq!(&wash(&state, 1)[5..], &[255, 255, 255, 0, 255, 0, 0]);
    }

    #[test]
    fn grand_master_scales_intensity_only() {
        let mut state = state_with("generic/rgbw-par", "6ch DRGBW Strobe", 1);
        state.locate(1).unwrap();
        state.grand_master.set(0.5);
        state.compose();
        assert_eq!(state.universe().get(1), 128); // dimmer scaled
        assert_eq!(state.universe().get(2), 255); // colour kept

        // Without a dimmer, the colour mix is what dims.
        let mut state = state_with("generic/rgb-par", "3ch RGB", 1);
        state.locate(1).unwrap();
        state.grand_master.set(0.0);
        state.compose();
        assert_eq!(
            (1..=3).map(|a| state.universe().get(a)).collect::<Vec<_>>(),
            vec![0, 0, 0]
        );

        state.grand_master.set(f32::NAN);
        assert_eq!(state.grand_master.0, 1.0);
        state.grand_master.set(3.0);
        assert_eq!(state.grand_master.0, 1.0);
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
