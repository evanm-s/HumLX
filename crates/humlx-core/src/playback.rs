//! Cue lists, their live playback with timed fades, and effects.
//!
//! Each cue stores programmer values. Cue lists track: the look at a cue is
//! every earlier cue with that cue's changes laid on top, so a cue only needs
//! to hold what changes.

use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::programmer::{Rgb, Values};

/// Fixture values by fixture number.
pub type Look = BTreeMap<u32, Values>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    pub name: String,
    /// Seconds to fade into this cue.
    #[serde(default = "default_fade")]
    pub fade: f32,
    #[serde(default)]
    pub values: Look,
}

fn default_fade() -> f32 {
    2.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueList {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub cues: Vec<Cue>,
}

impl CueList {
    /// The full look at cue `index`, with earlier cues tracked through.
    pub fn look_at(&self, index: usize) -> Look {
        let mut look = Look::new();
        for cue in self.cues.iter().take(index + 1) {
            for (fixture, values) in &cue.values {
                look.entry(*fixture).or_default().merge(values);
            }
        }
        look
    }
}

/// Mixes between two values: `t` is 0 at `from` and 1 at `to`. Intensity
/// fades from and to zero when one side has none; anything else only on one
/// side snaps to the side that has it.
pub fn blend(from: &Values, to: &Values, t: f32) -> Values {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let pair = |a: Option<f32>, b: Option<f32>| match (a, b) {
        (Some(a), Some(b)) => Some(lerp(a, b)),
        (a, b) => b.or(a),
    };
    let intensity = match (from.intensity, to.intensity) {
        (None, None) => None,
        (a, b) => Some(lerp(a.unwrap_or(0.0), b.unwrap_or(0.0))),
    };
    let colour = match (from.colour, to.colour) {
        (Some(a), Some(b)) => Some(Rgb {
            r: lerp(a.r, b.r),
            g: lerp(a.g, b.g),
            b: lerp(a.b, b.b),
        }),
        (a, b) => b.or(a),
    };
    let mut other = from.other.clone();
    for (attribute, value) in &to.other {
        let start = from.other.get(attribute).copied();
        other.insert(*attribute, pair(start, Some(*value)).unwrap_or(*value));
    }
    Values {
        intensity,
        colour,
        pan: pair(from.pan, to.pan),
        tilt: pair(from.tilt, to.tilt),
        other,
    }
}

fn blend_looks(from: &Look, to: &Look, t: f32) -> Look {
    let empty = Values::default();
    from.keys()
        .chain(to.keys())
        .map(|fixture| {
            let a = from.get(fixture).unwrap_or(&empty);
            let b = to.get(fixture).unwrap_or(&empty);
            (*fixture, blend(a, b, t))
        })
        .collect()
}

/// A cue list being played back.
#[derive(Debug, Clone)]
pub struct Playback {
    /// Index of the cue last gone to.
    pub current: usize,
    /// Level of the playback's fader, scaling its intensities.
    pub master: f32,
    /// When the playback was last started or moved, so the most recent wins
    /// for everything except intensity.
    pub order: u64,
    from: Look,
    to: Look,
    started: Instant,
    fade: Duration,
}

impl Playback {
    fn progress(&self, now: Instant) -> f32 {
        if self.fade.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.started).as_secs_f32() / self.fade.as_secs_f32()).min(1.0)
    }

    /// The look this playback outputs right now, before its master.
    pub fn look(&self, now: Instant) -> Look {
        let t = self.progress(now);
        if t >= 1.0 {
            self.to.clone()
        } else {
            blend_looks(&self.from, &self.to, t)
        }
    }

    /// True while a fade is running.
    pub fn fading(&self, now: Instant) -> bool {
        self.progress(now) < 1.0
    }
}

/// The live state of every cue list.
#[derive(Debug, Clone, Default)]
pub struct Playbacks {
    active: BTreeMap<u32, Playback>,
    /// Masters of cue lists, kept while they are released.
    masters: BTreeMap<u32, f32>,
    counter: u64,
}

impl Playbacks {
    pub fn get(&self, list: u32) -> Option<&Playback> {
        self.active.get(&list)
    }

    pub fn master(&self, list: u32) -> f32 {
        self.masters.get(&list).copied().unwrap_or(1.0)
    }

    pub fn set_master(&mut self, list: u32, level: f32) {
        let level = level.clamp(0.0, 1.0);
        self.masters.insert(list, level);
        if let Some(playback) = self.active.get_mut(&list) {
            playback.master = level;
        }
    }

    /// Fades the list to cue `index`, from wherever it is now.
    pub fn go_to(&mut self, list: &CueList, index: usize, now: Instant) -> Result<()> {
        let cue = list.cues.get(index).ok_or(Error::UnknownCue(index + 1))?;
        let from = self.active.get(&list.id).map(|p| p.look(now)).unwrap_or_default();
        self.counter += 1;
        let playback = Playback {
            current: index,
            master: self.master(list.id),
            order: self.counter,
            from,
            to: list.look_at(index),
            started: now,
            fade: Duration::from_secs_f32(cue.fade.max(0.0)),
        };
        self.active.insert(list.id, playback);
        Ok(())
    }

    /// Goes to the next cue, or the first when the list is not running. After
    /// the last cue it wraps round to the first.
    pub fn go(&mut self, list: &CueList, now: Instant) -> Result<()> {
        if list.cues.is_empty() {
            return Err(Error::UnknownCue(1));
        }
        let next = match self.active.get(&list.id) {
            Some(p) if p.current + 1 < list.cues.len() => p.current + 1,
            _ => 0,
        };
        self.go_to(list, next, now)
    }

    /// Goes back to the previous cue, quickly.
    pub fn back(&mut self, list: &CueList, now: Instant) -> Result<()> {
        let Some(playback) = self.active.get(&list.id) else {
            return Ok(());
        };
        let previous = playback.current.saturating_sub(1);
        self.go_to(list, previous, now)?;
        if let Some(p) = self.active.get_mut(&list.id) {
            p.fade = Duration::from_millis(500);
        }
        Ok(())
    }

    pub fn release(&mut self, list: u32) {
        self.active.remove(&list);
    }

    pub fn release_all(&mut self) {
        self.active.clear();
    }

    /// Re-reads a running list after its cues were edited.
    pub fn refresh(&mut self, list: &CueList, now: Instant) {
        if let Some(index) = self.active.get(&list.id).map(|p| p.current) {
            if index < list.cues.len() {
                let _ = self.go_to(list, index, now);
                if let Some(p) = self.active.get_mut(&list.id) {
                    p.fade = Duration::ZERO;
                }
            } else {
                self.release(list.id);
            }
        }
    }

    /// Keeps only playbacks of lists that still exist.
    pub fn retain(&mut self, exists: impl Fn(u32) -> bool) {
        self.active.retain(|id, _| exists(*id));
        self.masters.retain(|id, _| exists(*id));
    }

    /// The combined output of every running playback. Intensity takes the
    /// highest level (after each master); everything else comes from the
    /// most recently moved playback.
    pub fn output(&self, now: Instant) -> Look {
        let mut playbacks: Vec<&Playback> = self.active.values().collect();
        playbacks.sort_by_key(|p| p.order);
        let mut out = Look::new();
        for playback in playbacks {
            for (fixture, mut values) in playback.look(now) {
                values.intensity = values.intensity.map(|i| i * playback.master);
                let entry = out.entry(fixture).or_default();
                let highest = match (entry.intensity, values.intensity) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
                entry.merge(&values);
                entry.intensity = highest;
            }
        }
        out
    }

    pub fn running(&self) -> impl Iterator<Item = (&u32, &Playback)> {
        self.active.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    /// Intensity rises and falls.
    Dimmer,
    /// Colour cycles round the colour wheel.
    Rainbow,
    /// Moving heads trace a circle around their position.
    Circle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wave {
    Sine,
    Square,
    Saw,
}

impl Wave {
    /// Level from 0 to 1 at `phase` (in cycles).
    fn at(self, phase: f32) -> f32 {
        let p = phase.rem_euclid(1.0);
        match self {
            Wave::Sine => 0.5 - 0.5 * (p * TAU).cos(),
            Wave::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Wave::Saw => 1.0 - p,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub id: u32,
    pub name: String,
    pub kind: EffectKind,
    #[serde(default = "default_wave")]
    pub wave: Wave,
    /// Fixtures in order; spread offsets each one along this order.
    pub fixtures: Vec<u32>,
    /// Cycles per second.
    pub rate: f32,
    /// Depth of the effect, from 0 to 1.
    pub size: f32,
    /// How far apart the fixtures are, from 0 (all together) to 1 (spread
    /// evenly over one cycle).
    pub spread: f32,
}

fn default_wave() -> Wave {
    Wave::Sine
}

impl Effect {
    /// Changes `look` for this effect's fixtures at `elapsed` seconds since it started.
    pub fn apply(&self, look: &mut Look, elapsed: f32) {
        let count = self.fixtures.len().max(1) as f32;
        let size = self.size.clamp(0.0, 1.0);
        for (i, fixture) in self.fixtures.iter().enumerate() {
            let phase = elapsed * self.rate - self.spread * i as f32 / count;
            let values = look.entry(*fixture).or_default();
            match self.kind {
                EffectKind::Dimmer => {
                    let base = values.intensity.unwrap_or(1.0);
                    values.intensity = Some(base * (1.0 - size * (1.0 - self.wave.at(phase))));
                }
                EffectKind::Rainbow => {
                    let hue = phase.rem_euclid(1.0) * 6.0;
                    let f = |n: f32| {
                        let k = (n + hue) % 6.0;
                        1.0 - size.max(0.05) * k.min(4.0 - k).clamp(0.0, 1.0)
                    };
                    values.colour = Some(Rgb {
                        r: f(5.0),
                        g: f(3.0),
                        b: f(1.0),
                    });
                }
                EffectKind::Circle => {
                    let angle = phase * TAU;
                    let radius = size * 0.25;
                    values.pan = Some((values.pan.unwrap_or(0.5) + radius * angle.cos()).clamp(0.0, 1.0));
                    values.tilt = Some((values.tilt.unwrap_or(0.5) + radius * angle.sin()).clamp(0.0, 1.0));
                }
            }
        }
    }
}

/// Effects that are switched on, with when each started.
#[derive(Debug, Clone, Default)]
pub struct RunningEffects {
    started: BTreeMap<u32, Instant>,
}

impl RunningEffects {
    pub fn start(&mut self, id: u32, now: Instant) {
        self.started.insert(id, now);
    }

    pub fn stop(&mut self, id: u32) {
        self.started.remove(&id);
    }

    pub fn stop_all(&mut self) {
        self.started.clear();
    }

    pub fn is_running(&self, id: u32) -> bool {
        self.started.contains_key(&id)
    }

    pub fn ids(&self) -> Vec<u32> {
        self.started.keys().copied().collect()
    }

    /// Applies every running effect to `look`, in effect number order.
    pub fn apply(&self, effects: &[Effect], look: &mut Look, now: Instant) {
        for effect in effects {
            if let Some(started) = self.started.get(&effect.id) {
                effect.apply(look, now.saturating_duration_since(*started).as_secs_f32());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(intensity: f32) -> Values {
        Values {
            intensity: Some(intensity),
            ..Default::default()
        }
    }

    fn list() -> CueList {
        let mut red = Values {
            intensity: Some(1.0),
            colour: Some(Rgb { r: 1.0, g: 0.0, b: 0.0 }),
            ..Default::default()
        };
        red.pan = Some(0.2);
        CueList {
            id: 1,
            name: "Main".into(),
            cues: vec![
                Cue {
                    name: "Red".into(),
                    fade: 0.0,
                    values: [(1, red)].into(),
                },
                Cue {
                    name: "Half".into(),
                    fade: 2.0,
                    values: [(1, values(0.5)), (2, values(1.0))].into(),
                },
            ],
        }
    }

    #[test]
    fn cues_track() {
        let look = list().look_at(1);
        let one = &look[&1];
        assert_eq!(one.intensity, Some(0.5));
        assert_eq!(one.colour, Some(Rgb { r: 1.0, g: 0.0, b: 0.0 }));
        assert_eq!(look[&2].intensity, Some(1.0));
    }

    #[test]
    fn blend_rules() {
        let from = Values {
            intensity: Some(1.0),
            pan: Some(0.0),
            ..Default::default()
        };
        let to = Values {
            pan: Some(1.0),
            tilt: Some(0.3),
            ..Default::default()
        };
        let mid = blend(&from, &to, 0.25);
        assert_eq!(mid.intensity, Some(0.75)); // fading out to nothing
        assert_eq!(mid.pan, Some(0.25));
        assert_eq!(mid.tilt, Some(0.3)); // only on one side: snaps
    }

    #[test]
    fn go_fades_between_cues() {
        let list = list();
        let mut playbacks = Playbacks::default();
        let start = Instant::now();
        playbacks.go(&list, start).unwrap();
        assert_eq!(playbacks.output(start)[&1].intensity, Some(1.0)); // zero fade

        playbacks.go(&list, start).unwrap();
        let halfway = playbacks.output(start + Duration::from_secs(1));
        assert_eq!(halfway[&1].intensity, Some(0.75));
        assert_eq!(halfway[&2].intensity, Some(0.5));
        assert!(playbacks.get(1).unwrap().fading(start + Duration::from_secs(1)));
        let done = playbacks.output(start + Duration::from_secs(3));
        assert_eq!(done[&1].intensity, Some(0.5));

        // After the last cue, GO wraps to the first.
        playbacks.go(&list, start + Duration::from_secs(3)).unwrap();
        assert_eq!(playbacks.get(1).unwrap().current, 0);
    }

    #[test]
    fn back_and_release() {
        let list = list();
        let mut playbacks = Playbacks::default();
        let start = Instant::now();
        playbacks.go_to(&list, 1, start).unwrap();
        playbacks.back(&list, start).unwrap();
        assert_eq!(playbacks.get(1).unwrap().current, 0);
        assert!(playbacks.go_to(&list, 5, start).is_err());
        playbacks.release(1);
        assert!(playbacks.output(start).is_empty());
    }

    #[test]
    fn masters_and_htp() {
        let a = CueList {
            id: 1,
            name: "A".into(),
            cues: vec![Cue {
                name: "".into(),
                fade: 0.0,
                values: [(1, values(0.8))].into(),
            }],
        };
        let mut b = a.clone();
        b.id = 2;
        b.cues[0].values = [(
            1,
            Values {
                intensity: Some(0.6),
                pan: Some(0.9),
                ..Default::default()
            },
        )]
        .into();

        let mut playbacks = Playbacks::default();
        let now = Instant::now();
        playbacks.set_master(1, 0.5);
        playbacks.go(&a, now).unwrap();
        playbacks.go(&b, now).unwrap();
        let out = playbacks.output(now);
        assert_eq!(out[&1].intensity, Some(0.6)); // max(0.8 * 0.5, 0.6)
        assert_eq!(out[&1].pan, Some(0.9));
        playbacks.set_master(2, 0.0);
        assert_eq!(playbacks.output(now)[&1].intensity, Some(0.4));
    }

    #[test]
    fn dimmer_effect_spreads_over_fixtures() {
        let effect = Effect {
            id: 1,
            name: "Chase".into(),
            kind: EffectKind::Dimmer,
            wave: Wave::Square,
            fixtures: vec![1, 2],
            rate: 1.0,
            size: 1.0,
            spread: 1.0,
        };
        let mut look = Look::new();
        effect.apply(&mut look, 0.25);
        // Fixture 1 is in the first half of its cycle, fixture 2 half a cycle behind.
        assert_eq!(look[&1].intensity, Some(1.0));
        assert_eq!(look[&2].intensity, Some(0.0));
    }

    #[test]
    fn circle_and_rainbow() {
        let mut look: Look = [(
            1,
            Values {
                pan: Some(0.5),
                tilt: Some(0.5),
                ..Default::default()
            },
        )]
        .into();
        let circle = Effect {
            id: 1,
            name: "".into(),
            kind: EffectKind::Circle,
            wave: Wave::Sine,
            fixtures: vec![1],
            rate: 1.0,
            size: 1.0,
            spread: 0.0,
        };
        circle.apply(&mut look, 0.0);
        assert_eq!(look[&1].pan, Some(0.75));
        assert_eq!(look[&1].tilt, Some(0.5));

        let rainbow = Effect {
            kind: EffectKind::Rainbow,
            ..circle
        };
        rainbow.apply(&mut look, 0.0);
        let red = look[&1].colour.unwrap();
        assert!(
            (red.r - 1.0).abs() < 1e-6 && red.g.abs() < 1e-6 && red.b.abs() < 1e-6,
            "{red:?}"
        );
    }

    #[test]
    fn running_effects() {
        let mut running = RunningEffects::default();
        let now = Instant::now();
        running.start(3, now);
        assert!(running.is_running(3));
        assert_eq!(running.ids(), vec![3]);
        running.stop(3);
        assert!(!running.is_running(3));
    }
}
