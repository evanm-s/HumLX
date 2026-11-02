//! Cue lists and their live playback with timed fades.
//!
//! Each cue stores programmer values and effects. Cue lists track: the look
//! at a cue is every earlier cue with that cue's changes laid on top, so a cue
//! only needs to hold what changes. An effect keeps running into later cues
//! until a later cue sets a value or another effect of its family on its
//! fixtures, as on grandMA3.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::programmer::{Rgb, Values};

use crate::effect::{families_in, lay_over, take_out, Beats};
pub use crate::effect::{Effect, Look, RunningEffects};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    pub name: String,
    /// Seconds to fade into this cue.
    #[serde(default = "default_fade")]
    pub fade: f32,
    /// What happens once the cue has been gone to.
    #[serde(default)]
    pub mode: CueMode,
    /// For a follow cue, seconds from when the cue is gone to until the list
    /// goes to the next cue by itself. The clock starts with the cue's fade,
    /// so a follow time shorter than the fade cuts into it.
    #[serde(default)]
    pub follow: f32,
    #[serde(default)]
    pub values: Look,
    /// Effects this cue starts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

impl Default for Cue {
    fn default() -> Self {
        Self {
            name: String::new(),
            fade: default_fade(),
            mode: CueMode::Hold,
            follow: 0.0,
            values: Look::new(),
            effects: Vec::new(),
        }
    }
}

impl Cue {
    /// How long after this cue is gone to that the next cue follows, if it does.
    pub fn follow_time(&self) -> Option<Duration> {
        (self.mode == CueMode::Follow).then(|| Duration::from_secs_f32(self.follow.max(0.0)))
    }
}

/// Whether a cue waits for GO or moves on by itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueMode {
    /// Stays until the next GO.
    #[default]
    Hold,
    /// Goes to the next cue by itself after the cue's follow time.
    Follow,
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

    /// The effects running at cue `index`, with earlier cues tracked through.
    pub fn effects_at(&self, index: usize) -> Vec<Effect> {
        let mut effects = Vec::new();
        for cue in self.cues.iter().take(index + 1) {
            for (fixture, values) in &cue.values {
                for family in families_in(values) {
                    take_out(&mut effects, family, &[*fixture]);
                }
            }
            for effect in &cue.effects {
                lay_over(&mut effects, effect.clone());
            }
        }
        effects
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
    /// Effects fading out and in, alongside the values.
    from_effects: Vec<Effect>,
    to_effects: Vec<Effect>,
    /// When the list started running, which effects keep time from.
    clock: Instant,
    started: Instant,
    fade: Duration,
    /// When the current cue was gone to, kept when the list is refreshed.
    entered: Instant,
    /// When the next cue follows, for a follow cue gone to with GO.
    follow_at: Option<Instant>,
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

    /// The look with this playback's effects on it, their sizes scaled by
    /// `size`. Effects fade in and out with the cue fade; an effect in both
    /// cues keeps running.
    pub fn look_with_effects(&self, now: Instant, size: f32, beats: &Beats) -> Look {
        let mut look = self.look(now);
        let t = self.progress(now);
        let elapsed = now.saturating_duration_since(self.clock).as_secs_f32();
        if t < 1.0 {
            for effect in self.from_effects.iter().filter(|e| !self.to_effects.contains(e)) {
                effect.apply_timed(&mut look, elapsed, beats, size * (1.0 - t));
            }
        }
        for effect in &self.to_effects {
            let k = if self.from_effects.contains(effect) { 1.0 } else { t };
            if k > 0.0 {
                effect.apply_timed(&mut look, elapsed, beats, size * k);
            }
        }
        look
    }

    pub fn effects(&self) -> &[Effect] {
        &self.to_effects
    }

    /// True while a fade is running.
    pub fn fading(&self, now: Instant) -> bool {
        self.progress(now) < 1.0
    }

    /// Seconds until the next cue follows, if one will.
    pub fn follow_in(&self, now: Instant) -> Option<f32> {
        self.follow_at.map(|at| at.saturating_duration_since(now).as_secs_f32())
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
        let previous = self.active.get(&list.id);
        let from = previous.map(|p| p.look(now)).unwrap_or_default();
        let from_effects = previous.map(|p| p.to_effects.clone()).unwrap_or_default();
        let clock = previous.map_or(now, |p| p.clock);
        self.counter += 1;
        let playback = Playback {
            current: index,
            master: self.master(list.id),
            order: self.counter,
            from,
            to: list.look_at(index),
            from_effects,
            to_effects: list.effects_at(index),
            clock,
            started: now,
            fade: Duration::from_secs_f32(cue.fade.max(0.0)),
            entered: now,
            follow_at: cue.follow_time().map(|t| now + t),
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

    /// Goes to the next cue of every list whose follow time is up. The next
    /// cue starts when the follow was due, so timing holds between frames.
    /// A follow on the last cue loops round to the first, which makes a chase.
    pub fn follow(&mut self, lists: &[CueList], now: Instant) {
        let due: Vec<(u32, Instant)> = self
            .active
            .iter()
            .filter_map(|(id, p)| p.follow_at.filter(|at| *at <= now).map(|at| (*id, at)))
            .collect();
        for (id, at) in due {
            let Some(list) = lists.iter().find(|l| l.id == id) else {
                continue;
            };
            if self.go(list, at).is_err() {
                if let Some(p) = self.active.get_mut(&id) {
                    p.follow_at = None;
                }
            }
        }
    }

    /// Goes back to the previous cue, quickly. Follows don't run after Back,
    /// so it stays on the cue it went back to.
    pub fn back(&mut self, list: &CueList, now: Instant) -> Result<()> {
        let Some(playback) = self.active.get(&list.id) else {
            return Ok(());
        };
        let previous = playback.current.saturating_sub(1);
        self.go_to(list, previous, now)?;
        if let Some(p) = self.active.get_mut(&list.id) {
            p.fade = Duration::from_millis(500);
            p.follow_at = None;
        }
        Ok(())
    }

    pub fn release(&mut self, list: u32) {
        self.active.remove(&list);
    }

    pub fn release_all(&mut self) {
        self.active.clear();
    }

    /// Re-reads a running list after its cues were edited. A pending follow
    /// keeps counting from when the cue was gone to, with the edited time.
    pub fn refresh(&mut self, list: &CueList, now: Instant) {
        if let Some((index, entered, armed)) = self
            .active
            .get(&list.id)
            .map(|p| (p.current, p.entered, p.follow_at.is_some()))
        {
            if index < list.cues.len() {
                let _ = self.go_to(list, index, now);
                if let Some(p) = self.active.get_mut(&list.id) {
                    p.fade = Duration::ZERO;
                    p.entered = entered;
                    p.follow_at = list.cues[index].follow_time().filter(|_| armed).map(|t| entered + t);
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

    /// The combined output of every running playback, without effects.
    pub fn output(&self, now: Instant) -> Look {
        self.output_with_effects(now, 0.0, &Beats::default())
    }

    /// The combined output of every running playback, with their effects at
    /// `effect_size`. Intensity takes the highest level (after each master);
    /// everything else comes from the most recently moved playback.
    pub fn output_with_effects(&self, now: Instant, effect_size: f32, beats: &Beats) -> Look {
        let mut playbacks: Vec<&Playback> = self.active.values().collect();
        playbacks.sort_by_key(|p| p.order);
        let mut out = Look::new();
        for playback in playbacks {
            for (fixture, mut values) in playback.look_with_effects(now, effect_size, beats) {
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
                    mode: Default::default(),
                    follow: 0.0,
                    values: [(1, red)].into(),
                    effects: Vec::new(),
                },
                Cue {
                    name: "Half".into(),
                    fade: 2.0,
                    mode: Default::default(),
                    follow: 0.0,
                    values: [(1, values(0.5)), (2, values(1.0))].into(),
                    effects: Vec::new(),
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
    fn follow_cues_move_on_by_themselves() {
        let mut list = list();
        list.cues[0].mode = CueMode::Follow;
        list.cues[0].follow = 1.0;
        let lists = [list.clone()];
        let mut playbacks = Playbacks::default();
        let start = Instant::now();
        playbacks.go(&list, start).unwrap();
        assert_eq!(playbacks.get(1).unwrap().follow_in(start), Some(1.0));

        playbacks.follow(&lists, start + Duration::from_millis(900));
        assert_eq!(playbacks.get(1).unwrap().current, 0);
        // The next cue's fade starts when the follow was due, not when it was noticed.
        playbacks.follow(&lists, start + Duration::from_secs(2));
        let p = playbacks.get(1).unwrap();
        assert_eq!(p.current, 1);
        assert_eq!(p.follow_in(start), None); // cue 2 holds
        assert_eq!(
            playbacks.output(start + Duration::from_secs(2))[&2].intensity,
            Some(0.5)
        );

        // Back lands on the follow cue without it moving on again.
        playbacks.back(&list, start + Duration::from_secs(3)).unwrap();
        playbacks.follow(&lists, start + Duration::from_secs(10));
        assert_eq!(playbacks.get(1).unwrap().current, 0);
    }

    #[test]
    fn follow_on_last_cue_loops() {
        let mut list = list();
        list.cues[1].mode = CueMode::Follow;
        list.cues[1].follow = 0.5;
        let lists = [list.clone()];
        let mut playbacks = Playbacks::default();
        let start = Instant::now();
        playbacks.go_to(&list, 1, start).unwrap();
        playbacks.follow(&lists, start + Duration::from_secs(1));
        assert_eq!(playbacks.get(1).unwrap().current, 0);
    }

    #[test]
    fn effects_track_until_replaced() {
        use crate::effect::Shape;
        let pulse = Effect::new(1, "Pulse".into(), Shape::Pulse, &[], vec![1, 2]);
        let circle = Effect::new(2, "Circle".into(), Shape::Circle, &[], vec![1]);
        let mut list = list();
        list.cues[0].effects = vec![pulse.clone(), circle.clone()];
        // Cue 2 sets intensity on fixtures 1 and 2, which stops the pulse.
        assert_eq!(list.effects_at(0), vec![pulse.clone(), circle.clone()]);
        assert_eq!(list.effects_at(1), vec![circle.clone()]);

        list.cues[1].values.clear();
        list.cues[1].effects = vec![Effect::new(3, "Chase".into(), Shape::Chase, &[], vec![2])];
        let at_two = list.effects_at(1);
        assert_eq!(at_two.len(), 3);
        assert_eq!(at_two[0].fixtures, vec![1]); // the pulse carries on on fixture 1 only
    }

    #[test]
    fn effects_fade_with_the_cue() {
        use crate::effect::Shape;
        let mut list = list();
        list.cues[1].values.clear();
        list.cues[1].effects = vec![Effect::new(1, "Chase".into(), Shape::Chase, &[], vec![1])];
        let mut playbacks = Playbacks::default();
        let start = Instant::now();
        playbacks.go(&list, start).unwrap();
        playbacks.go(&list, start).unwrap();
        let out = playbacks.output_with_effects(start + Duration::from_millis(250), 1.0, &Beats::default());
        assert!((out[&1].intensity.unwrap() - 1.0).abs() < 1e-6); // chase on
        let out = playbacks.output_with_effects(start + Duration::from_millis(1500), 1.0, &Beats::default());
        let level = out[&1].intensity.unwrap();
        assert!((level - 0.25).abs() < 1e-6, "{level}"); // 75% faded in, chase off
        assert_eq!(
            playbacks.output(start + Duration::from_millis(1500))[&1].intensity,
            Some(1.0)
        );
    }

    #[test]
    fn hold_is_the_default() {
        let cue: Cue = serde_json::from_str(r#"{"name":"Old","fade":1}"#).unwrap();
        assert_eq!(cue.mode, CueMode::Hold);
        assert_eq!(cue.follow_time(), None);
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
                mode: Default::default(),
                follow: 0.0,
                values: [(1, values(0.8))].into(),
                effects: Vec::new(),
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
}
