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
    /// Seconds to fade into this cue: levels going up, and everything
    /// other than intensity.
    #[serde(default = "default_fade")]
    pub fade: f32,
    /// Seconds for levels going down, and for fading the list out when it's
    /// released or replaced. The fade in time when not set, as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<f32>,
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
            fade_out: None,
            mode: CueMode::Hold,
            follow: 0.0,
            values: Look::new(),
            effects: Vec::new(),
        }
    }
}

impl Cue {
    /// How long levels take to come up, and everything else to move.
    pub fn fade_in_time(&self) -> Duration {
        Duration::from_secs_f32(self.fade.max(0.0))
    }

    /// How long levels take to go down, and the list to fade out on release.
    pub fn fade_out_time(&self) -> Duration {
        Duration::from_secs_f32(self.fade_out.unwrap_or(self.fade).max(0.0))
    }

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
#[serde(rename_all = "camelCase")]
pub struct CueList {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub cues: Vec<Cue>,
    /// GO on this list fades out other running lists that have this on too,
    /// so looks replace each other rather than stacking, as Onyx's "Release
    /// on next go". Off for lists meant to layer on top.
    #[serde(default = "yes")]
    pub release_on_next_go: bool,
    /// GO on this list fades out every other running list.
    #[serde(default)]
    pub solo: bool,
}

fn yes() -> bool {
    true
}

impl Default for CueList {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            cues: Vec::new(),
            release_on_next_go: true,
            solo: false,
        }
    }
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
    blend_split(from, to, t, t)
}

/// As [`blend`], with intensity going down at `t_out` and everything else at
/// `t_in`, for split fade times.
pub fn blend_split(from: &Values, to: &Values, t_in: f32, t_out: f32) -> Values {
    let t = t_in.clamp(0.0, 1.0);
    let t_out = t_out.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let pair = |a: Option<f32>, b: Option<f32>| match (a, b) {
        (Some(a), Some(b)) => Some(lerp(a, b)),
        (a, b) => b.or(a),
    };
    let intensity = match (from.intensity, to.intensity) {
        (None, None) => None,
        (a, b) => {
            let (a, b) = (a.unwrap_or(0.0), b.unwrap_or(0.0));
            let t = if b < a { t_out } else { t };
            Some(a + (b - a) * t)
        }
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
        let value = match start {
            // Snapping attributes jump as the fade starts.
            Some(start) if attribute.snaps() && t <= 0.0 => start,
            _ if attribute.snaps() => *value,
            _ => pair(start, Some(*value)).unwrap_or(*value),
        };
        other.insert(*attribute, value);
    }
    Values {
        intensity,
        colour,
        pan: pair(from.pan, to.pan),
        tilt: pair(from.tilt, to.tilt),
        other,
    }
}

fn blend_looks(from: &Look, to: &Look, t_in: f32, t_out: f32) -> Look {
    let empty = Values::default();
    from.keys()
        .chain(to.keys())
        .map(|fixture| {
            let a = from.get(fixture).unwrap_or(&empty);
            let b = to.get(fixture).unwrap_or(&empty);
            (*fixture, blend_split(a, b, t_in, t_out))
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
    fade_in: Duration,
    fade_out: Duration,
    /// When the current cue was gone to, kept when the list is refreshed.
    entered: Instant,
    /// When the next cue follows, for a follow cue gone to with GO.
    follow_at: Option<Instant>,
    /// Fading out after being released; gone once the fade is done.
    pub releasing: bool,
}

impl Playback {
    fn progress_over(&self, fade: Duration, now: Instant) -> f32 {
        if fade.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.started).as_secs_f32() / fade.as_secs_f32()).min(1.0)
    }

    /// How far through the fade in, from 0 to 1.
    fn progress(&self, now: Instant) -> f32 {
        self.progress_over(self.fade_in, now)
    }

    /// How far through the fade out, from 0 to 1.
    fn progress_out(&self, now: Instant) -> f32 {
        self.progress_over(self.fade_out, now)
    }

    fn set_fades(&mut self, fade: Duration) {
        self.fade_in = fade;
        self.fade_out = fade;
    }

    /// The look this playback outputs right now, before its master.
    pub fn look(&self, now: Instant) -> Look {
        let (t_in, t_out) = (self.progress(now), self.progress_out(now));
        if t_in >= 1.0 && t_out >= 1.0 {
            self.to.clone()
        } else {
            blend_looks(&self.from, &self.to, t_in, t_out)
        }
    }

    /// The look with this playback's effects on it, their sizes scaled by
    /// `size`. Effects fade in and out with the cue fade; an effect in both
    /// cues keeps running.
    pub fn look_with_effects(&self, now: Instant, size: f32, beats: &Beats) -> Look {
        let mut look = self.look(now);
        let t = self.progress(now);
        let t_out = self.progress_out(now);
        let elapsed = now.saturating_duration_since(self.clock).as_secs_f32();
        if t_out < 1.0 {
            for effect in self.from_effects.iter().filter(|e| !self.to_effects.contains(e)) {
                effect.apply_timed(&mut look, elapsed, beats, size * (1.0 - t_out));
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
        self.progress(now) < 1.0 || self.progress_out(now) < 1.0
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
            fade_in: cue.fade_in_time(),
            fade_out: cue.fade_out_time(),
            entered: now,
            follow_at: cue.follow_time().map(|t| now + t),
            releasing: false,
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
        // A list fading out after a release starts again from the top.
        let next = match self.active.get(&list.id) {
            Some(p) if !p.releasing && p.current + 1 < list.cues.len() => p.current + 1,
            _ => 0,
        };
        self.go_to(list, next, now)
    }

    /// Goes to the next cue of every list whose follow time is up. The next
    /// cue starts when the follow was due, so timing holds between frames.
    /// A follow on the last cue loops round to the first, which makes a chase.
    pub fn follow(&mut self, lists: &[CueList], now: Instant) {
        self.active.retain(|_, p| !(p.releasing && !p.fading(now)));
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
        let Some(playback) = self.active.get(&list.id).filter(|p| !p.releasing) else {
            return Ok(());
        };
        let previous = playback.current.saturating_sub(1);
        self.go_to(list, previous, now)?;
        if let Some(p) = self.active.get_mut(&list.id) {
            p.set_fades(Duration::from_millis(500));
            p.follow_at = None;
        }
        Ok(())
    }

    pub fn release(&mut self, list: u32) {
        self.active.remove(&list);
    }

    /// Fades a running list out over `fade`, then releases it. Its
    /// intensities fade to zero; everything else holds until it's gone.
    pub fn fade_out(&mut self, list: u32, fade: Duration, now: Instant) {
        if fade.is_zero() {
            return self.release(list);
        }
        let Some(p) = self.active.get_mut(&list).filter(|p| !p.releasing) else {
            return;
        };
        p.from = p.look(now);
        p.from_effects = std::mem::take(&mut p.to_effects);
        p.to = Look::new();
        p.started = now;
        p.set_fades(fade);
        p.follow_at = None;
        p.releasing = true;
    }

    /// After GO on `list`: fades out the lists it replaces, over the fade out
    /// time of the cue it went to. A solo list replaces every other list; a list with
    /// release on next go replaces the others that have it too.
    pub fn release_others(&mut self, list: &CueList, lists: &[CueList], now: Instant) {
        if !list.solo && !list.release_on_next_go {
            return;
        }
        let Some(current) = self.active.get(&list.id).map(|p| p.current) else {
            return;
        };
        let fade = list.cues.get(current).map_or(Duration::ZERO, Cue::fade_out_time);
        let others: Vec<u32> = self
            .active
            .iter()
            .filter(|(id, p)| **id != list.id && !p.releasing)
            .filter(|(id, _)| list.solo || lists.iter().any(|l| l.id == **id && l.release_on_next_go))
            .map(|(id, _)| *id)
            .collect();
        for id in others {
            self.fade_out(id, fade, now);
        }
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
            .filter(|p| !p.releasing)
            .map(|p| (p.current, p.entered, p.follow_at.is_some()))
        {
            if index < list.cues.len() {
                let _ = self.go_to(list, index, now);
                if let Some(p) = self.active.get_mut(&list.id) {
                    p.set_fades(Duration::ZERO);
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
        self.combine(|p| p.look_with_effects(now, effect_size, beats))
    }

    /// The combined values of every running playback, leaving their effects
    /// out entirely, as Load puts them in the programmer.
    pub fn values(&self, now: Instant) -> Look {
        self.combine(|p| p.look(now))
    }

    fn combine(&self, look: impl Fn(&Playback) -> Look) -> Look {
        let mut playbacks: Vec<&Playback> = self.active.values().collect();
        playbacks.sort_by_key(|p| p.order);
        let mut out = Look::new();
        for playback in playbacks {
            for (fixture, mut values) in look(playback) {
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
                    fade_out: None,
                    mode: Default::default(),
                    follow: 0.0,
                    values: [(1, red)].into(),
                    effects: Vec::new(),
                },
                Cue {
                    name: "Half".into(),
                    fade: 2.0,
                    fade_out: None,
                    mode: Default::default(),
                    follow: 0.0,
                    values: [(1, values(0.5)), (2, values(1.0))].into(),
                    effects: Vec::new(),
                },
            ],
            ..Default::default()
        }
    }

    /// A one-cue list setting fixture 1 to `level` with a 2 second fade.
    fn look(id: u32, level: f32, release_on_next_go: bool, solo: bool) -> CueList {
        CueList {
            id,
            name: format!("Look {id}"),
            cues: vec![Cue {
                fade: 2.0,
                fade_out: None,
                values: [(1, values(level))].into(),
                ..Default::default()
            }],
            release_on_next_go,
            solo,
        }
    }

    #[test]
    fn looks_replace_each_other_with_a_fade() {
        let lists = [
            look(1, 1.0, true, false),
            look(2, 0.5, true, false),
            look(3, 0.8, false, false),
        ];
        let mut playbacks = Playbacks::default();
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        for list in [&lists[0], &lists[2]] {
            playbacks.go(list, start).unwrap();
            playbacks.release_others(list, &lists, start);
        }
        // Look 3 doesn't release on next go, so look 1 stays under it.
        assert!(playbacks.get(1).is_some_and(|p| !p.releasing));
        playbacks.go(&lists[1], at(3000)).unwrap();
        playbacks.release_others(&lists[1], &lists, at(3000));
        // Look 1 fades out over look 2's fade; look 3 layers on, untouched.
        assert!(playbacks.get(1).unwrap().releasing);
        assert!(!playbacks.get(3).unwrap().releasing);
        let level = playbacks.get(1).unwrap().look(at(4000))[&1].intensity.unwrap();
        assert!((level - 0.5).abs() < 1e-6, "{level}");
        playbacks.follow(&lists, at(5100));
        assert!(playbacks.get(1).is_none());
        assert!(playbacks.get(3).is_some());

        // GO on a list that's fading out starts it again.
        playbacks.go(&lists[1], at(6000)).unwrap();
        playbacks.fade_out(2, Duration::from_secs(2), at(6000));
        playbacks.go(&lists[1], at(6500)).unwrap();
        assert!(!playbacks.get(2).unwrap().releasing);
    }

    #[test]
    fn solo_releases_everything_else() {
        let lists = [
            look(1, 1.0, false, false),
            look(2, 0.5, true, false),
            look(3, 0.8, true, true),
        ];
        let mut playbacks = Playbacks::default();
        let now = Instant::now();
        for list in &lists {
            playbacks.go(list, now).unwrap();
            playbacks.release_others(list, &lists, now);
        }
        assert!(playbacks.get(1).unwrap().releasing);
        assert!(playbacks.get(2).unwrap().releasing);
        assert!(!playbacks.get(3).unwrap().releasing);
    }

    #[test]
    fn old_lists_release_on_next_go() {
        let list: CueList = serde_json::from_str(r#"{"id": 1, "name": "Main"}"#).unwrap();
        assert!(list.release_on_next_go && !list.solo);
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
    fn columns_snap_instead_of_fading() {
        let at = |column: f32| Values {
            other: [
                (crate::fixture::Attribute::Column, column),
                (crate::fixture::Attribute::Zoom, column),
            ]
            .into(),
            ..Default::default()
        };
        let (from, to) = (at(0.0), at(1.0));
        assert_eq!(blend(&from, &to, 0.0).other[&crate::fixture::Attribute::Column], 0.0);
        let mid = blend(&from, &to, 0.5);
        assert_eq!(mid.other[&crate::fixture::Attribute::Column], 1.0);
        assert_eq!(mid.other[&crate::fixture::Attribute::Zoom], 0.5);
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
    fn split_fade_times() {
        // Cue 2 fades fixture 1 down from 100% to 50% and fixture 2 up from
        // nothing, in over 2s and out over 4s.
        let mut list = list();
        list.cues[1].fade_out = Some(4.0);
        let mut playbacks = Playbacks::default();
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        playbacks.go(&list, start).unwrap();
        playbacks.go(&list, start).unwrap();
        let out = playbacks.output(at(1000));
        assert_eq!(out[&1].intensity, Some(0.875)); // a quarter of the way down
        assert_eq!(out[&2].intensity, Some(0.5)); // halfway up
        assert!(playbacks.get(1).unwrap().fading(at(3000)));
        assert_eq!(playbacks.output(at(3000))[&2].intensity, Some(1.0));
        assert!(!playbacks.get(1).unwrap().fading(at(4000)));

        // Releasing fades out over the cue's fade out time.
        let lists = [list.clone()];
        playbacks.fade_out(1, list.cues[1].fade_out_time(), at(5000));
        assert_eq!(playbacks.output(at(7000))[&2].intensity, Some(0.5));
        playbacks.follow(&lists, at(9100));
        assert!(playbacks.get(1).is_none());
    }

    #[test]
    fn fade_out_is_fade_in_unless_set() {
        let cue: Cue = serde_json::from_str(r#"{"name":"Old","fade":1.5}"#).unwrap();
        assert_eq!(cue.fade_out, None);
        assert_eq!(cue.fade_out_time(), Duration::from_secs_f32(1.5));
        assert!(!serde_json::to_string(&cue).unwrap().contains("fadeOut"));
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
                fade_out: None,
                mode: Default::default(),
                follow: 0.0,
                values: [(1, values(0.8))].into(),
                effects: Vec::new(),
            }],
            ..Default::default()
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
