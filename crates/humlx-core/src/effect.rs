//! Effects: values that move over time.
//!
//! As on grandMA3, an effect is a list of steps that the fixtures run
//! through, with settings that shape how they get from one step to the next.
//! People don't build steps by hand to start with: they pick a [`Shape`],
//! which writes the steps and sensible settings, and then change speed, size
//! and spread.

use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::fixture::Family;
use crate::programmer::{Rgb, Values};

/// Fixture values by fixture number.
pub type Look = BTreeMap<u32, Values>;

/// A ready-made effect. Picking one writes the effect's steps and settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Shape {
    /// Intensity rises and falls smoothly.
    Pulse,
    /// Intensity snaps on and off.
    Chase,
    /// Intensity fades up, then snaps back down.
    RampUp,
    /// Intensity snaps up, then fades down.
    RampDown,
    /// Intensity jumps around like a flame.
    Flicker,
    /// Colour cycles round the colour wheel.
    Rainbow,
    /// Colour snaps between two colours.
    Swap,
    /// Moving heads trace a circle around their position.
    Circle,
    /// Moving heads trace a figure of eight around their position.
    Figure8,
    /// Moving heads sweep side to side.
    PanSweep,
    /// Moving heads nod up and down.
    TiltNod,
}

impl Shape {
    pub fn family(self) -> Family {
        use Shape::*;
        match self {
            Pulse | Chase | RampUp | RampDown | Flicker => Family::Intensity,
            Rainbow | Swap => Family::Colour,
            Circle | Figure8 | PanSweep | TiltNod => Family::Position,
        }
    }

    /// The steps and settings this shape starts with. `colours` are the
    /// colours a [`Shape::Swap`] swaps between (red and blue if not given).
    pub fn recipe(self, colours: &[Rgb]) -> Recipe {
        use Shape::*;
        let level = |l: f32| Values {
            intensity: Some(l),
            ..Default::default()
        };
        let colour = |c: Rgb| Values {
            colour: Some(c),
            ..Default::default()
        };
        let at = |pan: Option<f32>, tilt: Option<f32>| Values {
            pan,
            tilt,
            ..Default::default()
        };
        // Curves are made of many straight steps, so the heads move at an even speed.
        let path = |f: &dyn Fn(f32) -> (f32, f32)| -> Vec<Values> {
            (0..PATH_STEPS)
                .map(|i| {
                    let (pan, tilt) = f(TAU * i as f32 / PATH_STEPS as f32);
                    at(Some(0.5 + 0.5 * pan), Some(0.5 + 0.5 * tilt))
                })
                .collect()
        };
        let base = Recipe {
            steps: Vec::new(),
            layering: Layering::OnTop,
            transition: 1.0,
            curve: Curve::Smooth,
            wrap: true,
        };
        match self {
            Pulse => Recipe {
                steps: vec![level(0.0), level(1.0)],
                ..base
            },
            Chase => Recipe {
                steps: vec![level(1.0), level(0.0)],
                transition: 0.0,
                ..base
            },
            RampUp | RampDown => Recipe {
                steps: if self == RampUp {
                    vec![level(0.0), level(1.0)]
                } else {
                    vec![level(1.0), level(0.0)]
                },
                curve: Curve::Linear,
                wrap: false,
                ..base
            },
            Flicker => Recipe {
                steps: [1.0, 0.35, 0.8, 0.15, 0.95, 0.5, 0.7, 0.25].map(level).to_vec(),
                transition: 0.4,
                curve: Curve::Linear,
                ..base
            },
            Rainbow => Recipe {
                // Straight lines between these six make the full colour wheel.
                steps: [
                    (1.0, 0.0, 0.0),
                    (1.0, 1.0, 0.0),
                    (0.0, 1.0, 0.0),
                    (0.0, 1.0, 1.0),
                    (0.0, 0.0, 1.0),
                    (1.0, 0.0, 1.0),
                ]
                .map(|(r, g, b)| colour(Rgb { r, g, b }))
                .to_vec(),
                layering: Layering::Replace,
                curve: Curve::Linear,
                ..base
            },
            Swap => Recipe {
                steps: vec![
                    colour(colours.first().copied().unwrap_or(Rgb { r: 1.0, g: 0.0, b: 0.0 })),
                    colour(colours.get(1).copied().unwrap_or(Rgb { r: 0.0, g: 0.0, b: 1.0 })),
                ],
                layering: Layering::Replace,
                transition: 0.0,
                ..base
            },
            Circle => Recipe {
                steps: path(&|a| (a.cos(), a.sin())),
                curve: Curve::Linear,
                ..base
            },
            Figure8 => Recipe {
                steps: path(&|a| (a.sin(), (2.0 * a).sin())),
                curve: Curve::Linear,
                ..base
            },
            PanSweep => Recipe {
                steps: vec![at(Some(0.0), None), at(Some(1.0), None)],
                ..base
            },
            TiltNod => Recipe {
                steps: vec![at(None, Some(0.0)), at(None, Some(1.0))],
                ..base
            },
        }
    }
}

/// Number of steps in a circle or figure of eight.
const PATH_STEPS: usize = 32;

/// The steps and settings a shape starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub steps: Vec<Values>,
    pub layering: Layering,
    pub transition: f32,
    pub curve: Curve,
    pub wrap: bool,
}

/// How an effect's values meet the look underneath.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Layering {
    /// Works from the look underneath: intensity is dimmed down from its
    /// level, position moves around it, colour is tinted from it.
    #[default]
    OnTop,
    /// Sets the values outright, whatever is underneath.
    Replace,
}

/// Which way an effect runs through its steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    #[default]
    Forward,
    Backward,
    /// Forward then backward.
    Bounce,
}

/// How the fixtures are spaced out in the effect, in the order they were picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pattern {
    /// One after another, first to last.
    #[default]
    InOrder,
    /// From both ends to the middle (or back out), so each side mirrors the other.
    Mirror,
    /// Two at a time.
    Pairs,
    /// Odd fixtures against even ones.
    OddEven,
}

impl Pattern {
    /// Which slot fixture `index` of `count` takes, and how many slots there are.
    /// Fixtures in the same slot move together.
    fn slot(self, index: usize, count: usize) -> (usize, usize) {
        let count = count.max(1);
        match self {
            Pattern::InOrder => (index, count),
            Pattern::Mirror => (index.min(count - 1 - index.min(count - 1)), count.div_ceil(2)),
            Pattern::Pairs => (index / 2, count.div_ceil(2)),
            Pattern::OddEven => (index % 2, count.min(2)),
        }
    }
}

/// How values move during the fade from one step to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Curve {
    /// Eases out of one step and into the next.
    #[default]
    Smooth,
    /// At an even speed.
    Linear,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "EffectFile")]
pub struct Effect {
    pub id: u32,
    pub name: String,
    /// The shape the effect was made from.
    pub shape: Shape,
    /// The values the fixtures run through, of the shape's family. For an
    /// effect laid on top, position steps are around 0.5 (no move).
    pub steps: Vec<Values>,
    pub layering: Layering,
    pub direction: Direction,
    pub pattern: Pattern,
    pub curve: Curve,
    /// Whether the last step fades back to the first. If not, the effect
    /// snaps back to the first step at the end of each cycle.
    pub wrap: bool,
    /// How much of each step is spent fading to the next, from 0 (snap) to 1
    /// (always moving).
    pub transition: f32,
    /// How much of each cycle the steps take, from 0 to 1. For the rest of
    /// the cycle the fixtures wait where the cycle ends.
    pub width: f32,
    /// Fixtures in order; spread offsets each one along this order.
    pub fixtures: Vec<u32>,
    /// Cycles per second, when the effect keeps its own speed.
    pub rate: f32,
    /// Where the effect takes its speed from.
    pub speed: Speed,
    /// Cycles per beat when following a speed master, such as 0.5 or 2.
    pub multiplier: f32,
    /// Depth of the effect, from 0 to 1.
    pub size: f32,
    /// How far apart the fixtures are, from 0 (all together) to 1 (spread
    /// evenly over one cycle).
    pub spread: f32,
}

/// Where an effect takes its speed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Speed {
    /// Its own rate, timed from when it started.
    #[default]
    Own,
    /// Speed master A: one cycle per beat, times the multiplier.
    A,
    /// Speed master B.
    B,
}

impl Effect {
    /// A new effect of `shape` on `fixtures`, with the shape's steps and settings.
    pub fn new(id: u32, name: String, shape: Shape, colours: &[Rgb], fixtures: Vec<u32>) -> Self {
        let recipe = shape.recipe(colours);
        Self {
            id,
            name,
            shape,
            steps: recipe.steps,
            layering: recipe.layering,
            direction: Direction::Forward,
            pattern: Pattern::InOrder,
            curve: recipe.curve,
            wrap: recipe.wrap,
            transition: recipe.transition,
            width: 1.0,
            fixtures,
            rate: 1.0,
            speed: Speed::Own,
            multiplier: 1.0,
            size: 1.0,
            spread: 1.0,
        }
    }

    /// Changes to `shape`, taking its steps and settings but keeping speed,
    /// size, spread, pattern, direction and width.
    pub fn reshape(&mut self, shape: Shape, colours: &[Rgb]) {
        let recipe = shape.recipe(colours);
        self.shape = shape;
        self.steps = recipe.steps;
        self.layering = recipe.layering;
        self.curve = recipe.curve;
        self.wrap = recipe.wrap;
        self.transition = recipe.transition;
    }

    pub fn family(&self) -> Family {
        self.shape.family()
    }

    /// The step values at `phase` cycles into the effect.
    pub fn sample(&self, phase: f32) -> Values {
        let n = self.steps.len();
        if n < 2 {
            return self.steps.first().cloned().unwrap_or_default();
        }
        let p = phase.rem_euclid(1.0);
        let p = match self.direction {
            Direction::Forward => p,
            Direction::Backward => (1.0 - p).rem_euclid(1.0),
            Direction::Bounce => 1.0 - (2.0 * p - 1.0).abs(),
        };
        let width = self.width.clamp(0.01, 1.0);
        // Past the width, wait where the cycle ends.
        let p = if p < width { p / width } else { 1.0 };
        let segments = if self.wrap { n } else { n - 1 };
        let x = p * segments as f32;
        let i = (x.floor() as usize).min(segments - 1);
        let f = x - i as f32;
        let t = self.transition.clamp(0.0, 1.0);
        let k = if t <= 0.0 || f < 1.0 - t {
            0.0
        } else {
            ((f - (1.0 - t)) / t).min(1.0)
        };
        let k = match self.curve {
            Curve::Smooth => 0.5 - 0.5 * (k * std::f32::consts::PI).cos(),
            Curve::Linear => k,
        };
        lerp_values(&self.steps[i], &self.steps[(i + 1) % n], k)
    }

    /// Changes `look` for this effect's fixtures at `elapsed` seconds since it started.
    pub fn apply(&self, look: &mut Look, elapsed: f32) {
        self.apply_scaled(look, elapsed, 1.0);
    }

    /// As [`Effect::apply`], with the effect's size scaled by `scale`.
    pub fn apply_scaled(&self, look: &mut Look, elapsed: f32, scale: f32) {
        self.apply_at(look, elapsed * self.rate, scale);
    }

    /// How many cycles the effect has run: from `elapsed` seconds at its own
    /// rate, or from the beats of the speed master it follows.
    pub fn cycles(&self, elapsed: f32, beats: &Beats) -> f32 {
        match self.speed {
            Speed::Own => elapsed * self.rate,
            Speed::A => beats.0[0] * self.multiplier,
            Speed::B => beats.0[1] * self.multiplier,
        }
    }

    /// Applies the effect `elapsed` seconds after it started, keeping time
    /// with `beats` when it follows a speed master.
    pub fn apply_timed(&self, look: &mut Look, elapsed: f32, beats: &Beats, scale: f32) {
        self.apply_at(look, self.cycles(elapsed, beats), scale);
    }

    /// Applies the effect `cycles` cycles in.
    pub fn apply_at(&self, look: &mut Look, cycles: f32, scale: f32) {
        let count = self.fixtures.len();
        let size = (self.size * scale).clamp(0.0, 1.0);
        let spread = self.spread.clamp(0.0, 1.0);
        let family = self.family();
        for (index, fixture) in self.fixtures.iter().enumerate() {
            let (slot, slots) = self.pattern.slot(index, count);
            let phase = cycles - spread * slot as f32 / slots as f32;
            let step = self.sample(phase);
            let values = look.entry(*fixture).or_default();
            let on_top = self.layering == Layering::OnTop;
            match family {
                Family::Intensity => {
                    let s = step.intensity.unwrap_or(1.0);
                    let base = if on_top { values.intensity.unwrap_or(1.0) } else { 1.0 };
                    values.intensity = Some(base * (1.0 - size * (1.0 - s)));
                }
                Family::Colour => {
                    if let Some(c) = step.colour {
                        let base = if on_top {
                            values.colour.unwrap_or(Rgb::WHITE)
                        } else {
                            Rgb::WHITE
                        };
                        values.colour = Some(lerp_rgb(base, c, size));
                    }
                }
                Family::Position => {
                    let mv = |base: Option<f32>, s: f32| {
                        if on_top {
                            // A full-size move goes a quarter of the range each way.
                            base.unwrap_or(0.5) + (s - 0.5) * size * 0.5
                        } else {
                            0.5 + (s - 0.5) * size
                        }
                        .clamp(0.0, 1.0)
                    };
                    if let Some(s) = step.pan {
                        values.pan = Some(mv(values.pan, s));
                    }
                    if let Some(s) = step.tilt {
                        values.tilt = Some(mv(values.tilt, s));
                    }
                }
                Family::Beam | Family::Control => {}
            }
        }
    }
}

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

fn lerp_rgb(a: Rgb, b: Rgb, k: f32) -> Rgb {
    Rgb {
        r: lerp(a.r, b.r, k),
        g: lerp(a.g, b.g, k),
        b: lerp(a.b, b.b, k),
    }
}

fn lerp_opt(a: Option<f32>, b: Option<f32>, k: f32) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(lerp(a, b, k)),
        _ => a.or(b),
    }
}

fn lerp_values(a: &Values, b: &Values, k: f32) -> Values {
    Values {
        intensity: lerp_opt(a.intensity, b.intensity, k),
        colour: match (a.colour, b.colour) {
            (Some(x), Some(y)) => Some(lerp_rgb(x, y, k)),
            (x, y) => x.or(y),
        },
        pan: lerp_opt(a.pan, b.pan, k),
        tilt: lerp_opt(a.tilt, b.tilt, k),
        other: BTreeMap::new(),
    }
}

/// An effect as stored in a show file, which may be from before effects had
/// steps (when they had a `kind` and a `wave`).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectFile {
    id: u32,
    #[serde(default)]
    name: String,
    shape: Option<Shape>,
    kind: Option<OldKind>,
    wave: Option<OldWave>,
    #[serde(default)]
    steps: Vec<Values>,
    layering: Option<Layering>,
    #[serde(default)]
    direction: Direction,
    #[serde(default)]
    pattern: Pattern,
    curve: Option<Curve>,
    wrap: Option<bool>,
    transition: Option<f32>,
    width: Option<f32>,
    #[serde(default)]
    speed: Speed,
    multiplier: Option<f32>,
    #[serde(default)]
    fixtures: Vec<u32>,
    rate: f32,
    size: f32,
    spread: f32,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum OldKind {
    Dimmer,
    Rainbow,
    Circle,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum OldWave {
    Sine,
    Square,
    Saw,
}

impl From<EffectFile> for Effect {
    fn from(f: EffectFile) -> Self {
        let shape = f.shape.unwrap_or(match (f.kind, f.wave) {
            (Some(OldKind::Rainbow), _) => Shape::Rainbow,
            (Some(OldKind::Circle), _) => Shape::Circle,
            (_, Some(OldWave::Square)) => Shape::Chase,
            (_, Some(OldWave::Saw)) => Shape::RampDown,
            _ => Shape::Pulse,
        });
        let recipe = shape.recipe(&[]);
        Effect {
            id: f.id,
            name: f.name,
            shape,
            steps: if f.steps.is_empty() { recipe.steps } else { f.steps },
            layering: f.layering.unwrap_or(recipe.layering),
            direction: f.direction,
            pattern: f.pattern,
            curve: f.curve.unwrap_or(recipe.curve),
            wrap: f.wrap.unwrap_or(recipe.wrap),
            transition: f.transition.unwrap_or(recipe.transition),
            width: f.width.unwrap_or(1.0),
            fixtures: f.fixtures,
            rate: f.rate,
            speed: f.speed,
            multiplier: f.multiplier.unwrap_or(1.0),
            size: f.size,
            spread: f.spread,
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

    /// Starts any of `effects` not yet running, from `now`.
    pub fn start_new(&mut self, effects: &[Effect], now: Instant) {
        for effect in effects {
            self.started.entry(effect.id).or_insert(now);
        }
    }

    /// Applies every running effect to `look`, in order, with sizes scaled by `scale`.
    pub fn apply(&self, effects: &[Effect], look: &mut Look, now: Instant, beats: &Beats, scale: f32) {
        for effect in effects {
            if let Some(started) = self.started.get(&effect.id) {
                let elapsed = now.saturating_duration_since(*started).as_secs_f32();
                effect.apply_timed(look, elapsed, beats, scale);
            }
        }
    }
}

/// How many beats each speed master has counted, A then B.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Beats(pub [f32; 2]);

/// The slowest and fastest a speed master runs, in beats per minute.
pub const BPM_RANGE: (f32, f32) = (10.0, 300.0);

/// A tempo that effects can follow, set by tapping or by typing a BPM.
#[derive(Debug, Clone)]
pub struct SpeedMaster {
    bpm: f32,
    /// When beat 0 was.
    origin: Instant,
    /// Recent taps, oldest first.
    taps: Vec<Instant>,
}

impl SpeedMaster {
    pub fn new(bpm: f32, now: Instant) -> Self {
        Self {
            bpm: bpm.clamp(BPM_RANGE.0, BPM_RANGE.1),
            origin: now,
            taps: Vec::new(),
        }
    }

    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Beats counted at `now`.
    pub fn beats(&self, now: Instant) -> f32 {
        now.saturating_duration_since(self.origin).as_secs_f32() * self.bpm / 60.0
    }

    /// Changes the tempo without a jump: effects carry on from where they are.
    pub fn set_bpm(&mut self, bpm: f32, now: Instant) {
        let beats = self.beats(now);
        self.bpm = bpm.clamp(BPM_RANGE.0, BPM_RANGE.1);
        let back = Duration::from_secs_f32(beats * 60.0 / self.bpm);
        self.origin = now
            .checked_sub(back)
            .or_else(|| now.checked_sub(Duration::from_secs_f32(beats.fract() * 60.0 / self.bpm)))
            .unwrap_or(now);
    }

    /// A tap of the beat. Two or more taps less than two seconds apart set
    /// the tempo from their average gap; every tap puts the beat on the tap.
    pub fn tap(&mut self, now: Instant) {
        if self
            .taps
            .last()
            .is_some_and(|last| now.saturating_duration_since(*last) > Duration::from_secs(2))
        {
            self.taps.clear();
        }
        self.taps.push(now);
        if self.taps.len() > 5 {
            self.taps.remove(0);
        }
        if let (Some(first), Some(last)) = (self.taps.first(), self.taps.last()) {
            let gaps = (self.taps.len() - 1) as f32;
            if gaps > 0.0 {
                let gap = last.saturating_duration_since(*first).as_secs_f32() / gaps;
                if gap > 0.0 {
                    self.bpm = (60.0 / gap).clamp(BPM_RANGE.0, BPM_RANGE.1);
                }
            }
        }
        self.origin = now;
    }
}

/// Speed masters A and B.
#[derive(Debug, Clone)]
pub struct SpeedMasters(pub [SpeedMaster; 2]);

impl Default for SpeedMasters {
    fn default() -> Self {
        let now = Instant::now();
        Self([SpeedMaster::new(120.0, now), SpeedMaster::new(120.0, now)])
    }
}

impl SpeedMasters {
    pub fn beats(&self, now: Instant) -> Beats {
        Beats([self.0[0].beats(now), self.0[1].beats(now)])
    }

    pub fn bpms(&self) -> [f32; 2] {
        [self.0[0].bpm(), self.0[1].bpm()]
    }
}

/// The master that scales the size of every effect, from 0 (no movement) to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectSize(pub f32);

impl Default for EffectSize {
    fn default() -> Self {
        Self(1.0)
    }
}

/// The families `values` sets.
pub fn families_in(values: &Values) -> Vec<Family> {
    let mut families = Vec::new();
    let mut add = |f: Family| {
        if !families.contains(&f) {
            families.push(f);
        }
    };
    if values.intensity.is_some() {
        add(Family::Intensity);
    }
    if values.colour.is_some() {
        add(Family::Colour);
    }
    if values.pan.is_some() || values.tilt.is_some() {
        add(Family::Position);
    }
    for attribute in values.other.keys() {
        add(attribute.family());
    }
    families
}

/// Takes `fixtures` out of every effect in `effects` that moves `family`,
/// dropping effects left with no fixtures. This is how a later cue's value
/// or effect stops an earlier effect on the same fixtures.
pub fn take_out(effects: &mut Vec<Effect>, family: Family, fixtures: &[u32]) {
    for effect in effects.iter_mut().filter(|e| e.family() == family) {
        effect.fixtures.retain(|f| !fixtures.contains(f));
    }
    effects.retain(|e| !e.fixtures.is_empty());
}

/// Adds `effect` to `effects`, replacing earlier effects of its family on its fixtures.
pub fn lay_over(effects: &mut Vec<Effect>, effect: Effect) {
    take_out(effects, effect.family(), &effect.fixtures);
    effects.push(effect);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn effect(shape: Shape, fixtures: Vec<u32>) -> Effect {
        Effect::new(1, String::new(), shape, &[], fixtures)
    }

    fn levels(e: &Effect, elapsed: f32) -> Vec<f32> {
        let mut look = Look::new();
        e.apply(&mut look, elapsed);
        e.fixtures.iter().map(|f| look[f].intensity.unwrap()).collect()
    }

    #[test]
    fn pulse_is_a_sine_wave() {
        let e = Effect {
            spread: 0.0,
            ..effect(Shape::Pulse, vec![1])
        };
        for p in [0.0, 0.1, 0.25, 0.4, 0.5, 0.8] {
            let want = 0.5 - 0.5 * (p * TAU).cos();
            assert!(close(levels(&e, p)[0], want), "at {p}");
        }
    }

    #[test]
    fn chase_spreads_over_fixtures() {
        let e = effect(Shape::Chase, vec![1, 2]);
        // Fixture 1 is in the first half of its cycle, fixture 2 half a cycle behind.
        assert_eq!(levels(&e, 0.25), vec![1.0, 0.0]);
    }

    #[test]
    fn ramp_down_is_a_saw() {
        let e = effect(Shape::RampDown, vec![1]);
        for p in [0.0, 0.25, 0.5, 0.9] {
            assert!(close(levels(&e, p)[0], 1.0 - p), "at {p}");
        }
    }

    #[test]
    fn width_makes_short_pulses() {
        let e = Effect {
            width: 0.25,
            spread: 0.0,
            ..effect(Shape::Chase, vec![1])
        };
        assert_eq!(levels(&e, 0.1), vec![1.0]);
        assert_eq!(levels(&e, 0.2), vec![0.0]);
        assert_eq!(levels(&e, 0.6), vec![0.0]); // waiting until the next cycle
    }

    #[test]
    fn direction() {
        let mut e = Effect {
            spread: 0.0,
            ..effect(Shape::RampUp, vec![1])
        };
        assert!(close(levels(&e, 0.25)[0], 0.25));
        e.direction = Direction::Backward;
        assert!(close(levels(&e, 0.25)[0], 0.75));
        e.direction = Direction::Bounce;
        assert!(close(levels(&e, 0.25)[0], 0.5));
        assert!(close(levels(&e, 0.75)[0], 0.5));
    }

    #[test]
    fn patterns() {
        let slots = |p: Pattern| (0..6).map(|i| p.slot(i, 6).0).collect::<Vec<_>>();
        assert_eq!(slots(Pattern::InOrder), vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(slots(Pattern::Mirror), vec![0, 1, 2, 2, 1, 0]);
        assert_eq!(slots(Pattern::Pairs), vec![0, 0, 1, 1, 2, 2]);
        assert_eq!(slots(Pattern::OddEven), vec![0, 1, 0, 1, 0, 1]);
        assert_eq!(Pattern::Mirror.slot(2, 5), (2, 3));

        let e = Effect {
            pattern: Pattern::OddEven,
            ..effect(Shape::Chase, vec![1, 2, 3, 4])
        };
        assert_eq!(levels(&e, 0.25), vec![1.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn on_top_dims_from_the_level_underneath() {
        let e = Effect {
            size: 0.5,
            ..effect(Shape::Chase, vec![1])
        };
        let mut look: Look = [(
            1,
            Values {
                intensity: Some(0.8),
                ..Default::default()
            },
        )]
        .into();
        e.apply(&mut look, 0.75);
        assert!(close(look[&1].intensity.unwrap(), 0.4));
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
        let circle = effect(Shape::Circle, vec![1]);
        circle.apply(&mut look, 0.0);
        assert!(close(look[&1].pan.unwrap(), 0.75));
        assert!(close(look[&1].tilt.unwrap(), 0.5));
        let mut quarter: Look = [(
            1,
            Values {
                pan: Some(0.5),
                tilt: Some(0.5),
                ..Default::default()
            },
        )]
        .into();
        circle.apply(&mut quarter, 0.25);
        assert!(close(quarter[&1].pan.unwrap(), 0.5));
        assert!(close(quarter[&1].tilt.unwrap(), 0.75));

        let rainbow = effect(Shape::Rainbow, vec![1]);
        rainbow.apply(&mut look, 0.0);
        assert_eq!(look[&1].colour, Some(Rgb { r: 1.0, g: 0.0, b: 0.0 }));
        rainbow.apply(&mut look, 0.25);
        let c = look[&1].colour.unwrap();
        // A quarter of the way round: between yellow and green.
        assert!(close(c.r, 0.5) && close(c.g, 1.0) && close(c.b, 0.0), "{c:?}");
    }

    #[test]
    fn swap_uses_given_colours() {
        let green = Rgb { r: 0.0, g: 1.0, b: 0.0 };
        let e = Effect::new(1, String::new(), Shape::Swap, &[green, Rgb::WHITE], vec![1]);
        let mut look = Look::new();
        e.apply(&mut look, 0.0);
        assert_eq!(look[&1].colour, Some(green));
    }

    #[test]
    fn loads_old_effects() {
        let old =
            r#"{"id":2,"name":"Old","kind":"dimmer","wave":"square","fixtures":[1,2],"rate":2,"size":0.5,"spread":1}"#;
        let e: Effect = serde_json::from_str(old).unwrap();
        assert_eq!(e.shape, Shape::Chase);
        assert_eq!(e.steps, Shape::Chase.recipe(&[]).steps);
        assert_eq!((e.rate, e.size, e.width), (2.0, 0.5, 1.0));

        let circle: Effect =
            serde_json::from_str(r#"{"id":1,"kind":"circle","fixtures":[],"rate":1,"size":1,"spread":0}"#).unwrap();
        assert_eq!(circle.shape, Shape::Circle);

        // And new ones round-trip.
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<Effect>(&json).unwrap(), e);
    }

    #[test]
    fn later_effects_replace_earlier_ones() {
        let mut effects = vec![effect(Shape::Pulse, vec![1, 2, 3]), effect(Shape::Circle, vec![1])];
        lay_over(&mut effects, effect(Shape::Chase, vec![2, 3]));
        assert_eq!(effects.len(), 3);
        assert_eq!(effects[0].fixtures, vec![1]);
        take_out(&mut effects, Family::Position, &[1]);
        assert_eq!(
            effects.iter().map(|e| e.shape).collect::<Vec<_>>(),
            vec![Shape::Pulse, Shape::Chase]
        );
        let values = Values {
            tilt: Some(0.2),
            ..Default::default()
        };
        assert_eq!(families_in(&values), vec![Family::Position]);
    }

    #[test]
    fn speed_masters() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut master = SpeedMaster::new(120.0, start);
        assert!(close(master.beats(at(1000)), 2.0));
        // Taps 0.6 s apart: 100 BPM, with the beat on the last tap.
        master.tap(at(1000));
        master.tap(at(1600));
        master.tap(at(2200));
        assert!(close(master.bpm(), 100.0));
        assert!(close(master.beats(at(2200)), 0.0));
        // A tap after a long gap starts counting again and keeps the tempo.
        master.tap(at(9000));
        assert!(close(master.bpm(), 100.0));
        // Changing the tempo carries on from the same beat.
        let before = master.beats(at(9300));
        master.set_bpm(200.0, at(9300));
        assert!(close(master.beats(at(9300)), before));
        assert!(close(master.beats(at(9600)), before + 1.0));
        master.set_bpm(1000.0, at(9600));
        assert_eq!(master.bpm(), BPM_RANGE.1);
    }

    #[test]
    fn effects_follow_speed_masters() {
        let e = Effect {
            speed: Speed::B,
            multiplier: 0.5,
            spread: 0.0,
            ..effect(Shape::Chase, vec![1])
        };
        let beats = Beats([0.0, 1.5]);
        // 1.5 beats at half a cycle per beat: three quarters through, the off half.
        assert!(close(e.cycles(99.0, &beats), 0.75));
        let mut look = Look::new();
        e.apply_timed(&mut look, 99.0, &beats, 1.0);
        assert_eq!(look[&1].intensity, Some(0.0));
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
