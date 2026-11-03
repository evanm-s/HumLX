//! The stage plan: marks on the floor, and where each mover hangs, worked out
//! by pointing it at those marks. Once a mover is calibrated, a spot on the
//! plan can be turned back into the pan and tilt that hits it.
//!
//! Coordinates are in metres with the origin at the front edge of the acting
//! area, in the middle: x runs to the right as seen from the desk, y runs
//! away from the desk (up stage) and z is height above the floor.
//!
//! The model of a mover: pan turns the beam round the fixture's own axis and
//! tilt swings it away from that axis, with 50% tilt pointing straight along
//! the axis. Its position, which way that axis points, and how far its pan and
//! tilt travel are all fitted to the calibration readings, so nothing has to
//! be measured except where the marks are.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Readings needed before a mover can be worked out.
pub const MIN_READINGS: usize = 4;

/// What has been measured of a fixture's position: across, up stage and
/// height, each in metres or `None` when not measured.
pub type Known = [Option<f32>; 3];

/// Readings needed to work a mover out: four with nothing measured, three
/// with part of its position (such as the grid height), two with all of it.
pub fn min_readings(known: &Known) -> usize {
    match known.iter().filter(|k| k.is_some()).count() {
        0 => MIN_READINGS,
        3 => 2,
        _ => 3,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Stage {
    /// Size of the acting area drawn on the plan, in metres.
    pub width: f32,
    pub depth: f32,
    /// Studio floor shown round the stage on the plan, in metres on each side.
    pub margin: Margins,
    /// How far the stage floor sits above the studio floor. Heights are
    /// measured from the stage floor, so the studio floor is at minus this.
    pub riser: f32,
    /// Tape marks on the floor that movers are calibrated against.
    pub marks: Vec<Mark>,
    /// Calibration of each mover, by fixture number.
    pub movers: Vec<Calibration>,
    /// Where fixtures hang, measured by hand. A mover with a known position
    /// only needs its heading worked out, which suits wide beams.
    pub positions: Vec<Placed>,
    /// Things on the stage drawn for reference, such as an LED wall.
    pub scenery: Vec<Scenery>,
}

/// Studio floor shown round the stage, in metres on each side. Shows saved
/// before each side had its own read a single number for all four.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Margins {
    pub left: f32,
    pub right: f32,
    pub front: f32,
    pub back: f32,
}

impl Margins {
    pub fn even(m: f32) -> Self {
        Self {
            left: m,
            right: m,
            front: m,
            back: m,
        }
    }

    fn sides(&self) -> [f32; 4] {
        [self.left, self.right, self.front, self.back]
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self::even(2.0)
    }
}

impl<'de> Deserialize<'de> for Margins {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Sides {
            left: f32,
            right: f32,
            front: f32,
            back: f32,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Even(f32),
            Sides(Sides),
        }
        Ok(match Either::deserialize(d)? {
            Either::Even(m) => Self::even(m),
            Either::Sides(s) => Self {
                left: s.left,
                right: s.right,
                front: s.front,
                back: s.back,
            },
        })
    }
}

/// A piece of scenery drawn on the plan as a line through its points, such
/// as an LED wall. Only for reference: it doesn't change how anything aims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenery {
    pub name: String,
    /// Points across and up stage, in metres.
    pub points: Vec<[f32; 2]>,
    /// How tall it stands above the floor, for the side view.
    pub height: f32,
}

/// A fixture's measured position in metres; any part can be left unmeasured.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placed {
    pub fixture: u32,
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    #[serde(default)]
    pub z: Option<f32>,
}

impl Default for Stage {
    fn default() -> Self {
        let (width, depth) = (8.0, 6.0);
        Self {
            width,
            depth,
            margin: Margins::default(),
            riser: 0.0,
            marks: default_marks(width, depth),
            movers: Vec::new(),
            positions: Vec::new(),
            scenery: Vec::new(),
        }
    }
}

/// The four corners of the acting area and its centre.
pub fn default_marks(width: f32, depth: f32) -> Vec<Mark> {
    let (l, r) = (-width / 2.0, width / 2.0);
    [
        ("Front left", l, 0.0),
        ("Front right", r, 0.0),
        ("Back left", l, depth),
        ("Back right", r, depth),
        ("Centre", 0.0, depth / 2.0),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, x, y))| Mark {
        id: i as u32 + 1,
        name: name.into(),
        x,
        y,
        z: 0.0,
    })
    .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mark {
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    /// Height above the floor; 0 for a tape mark.
    #[serde(default)]
    pub z: f32,
}

impl Mark {
    fn point(&self) -> V3 {
        [f64::from(self.x), f64::from(self.y), f64::from(self.z)]
    }
}

/// One mover's readings and, once there are enough, where it hangs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub fixture: u32,
    pub readings: Vec<Reading>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solution: Option<Solution>,
}

impl Calibration {
    /// Pan and tilt travel from the last fit, to start the next one from.
    fn ranges(&self) -> Option<(f32, f32)> {
        self.solution.as_ref().map(|s| (s.pan_range, s.tilt_range))
    }
}

/// Pan and tilt, as sent to the fixture (after its patch orientation), that
/// put the beam on a mark.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub mark: u32,
    pub pan: f32,
    pub tilt: f32,
}

/// Where a mover hangs and how it moves, fitted to its readings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Position of the mover in metres.
    pub position: [f32; 3],
    /// Turns directions from the mover's own frame into stage directions;
    /// column 3 is where 50% tilt points.
    pub rotation: [[f32; 3]; 3],
    /// Pan and tilt travel in degrees.
    pub pan_range: f32,
    pub tilt_range: f32,
    /// How far from each mark the fitted mover would land, in metres.
    pub errors: Vec<MarkError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkError {
    pub mark: u32,
    pub metres: f32,
}

/// Pan and tilt (as sent to the fixture) that aim a mover at a spot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aim {
    pub pan: f32,
    pub tilt: f32,
    /// False when the spot is outside the mover's travel and the nearest
    /// reachable position was used.
    pub reachable: bool,
}

/// The parts of the stage set up by hand: its size and marks.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub width: f32,
    pub depth: f32,
    pub margin: Margins,
    pub riser: f32,
    pub marks: Vec<Mark>,
    #[serde(default)]
    pub scenery: Vec<Scenery>,
}

impl Stage {
    /// Height of the floor at a spot: the stage floor on the stage, the
    /// studio floor round it.
    pub fn floor_at(&self, x: f32, y: f32) -> f32 {
        let on_stage = x.abs() <= self.width / 2.0 && (0.0..=self.depth).contains(&y);
        if on_stage {
            0.0
        } else {
            -self.riser
        }
    }

    pub fn calibration(&self, fixture: u32) -> Option<&Calibration> {
        self.movers.iter().find(|c| c.fixture == fixture)
    }

    /// What has been measured of a fixture's position.
    pub fn position(&self, fixture: u32) -> Known {
        self.positions
            .iter()
            .find(|p| p.fixture == fixture)
            .map_or([None; 3], |p| [p.x, p.y, p.z])
    }

    /// Sets a fixture's measured position (any part `None` to leave it to the
    /// calibration), and works its calibration out again to suit.
    pub fn set_position(&mut self, fixture: u32, position: Known) -> Result<()> {
        if !position.iter().flatten().all(|v| v.is_finite() && v.abs() <= 500.0) {
            return Err(Error::InvalidStage("that position is out of range".into()));
        }
        self.positions.retain(|p| p.fixture != fixture);
        if position.iter().any(Option::is_some) {
            let [x, y, z] = position;
            self.positions.push(Placed { fixture, x, y, z });
            self.positions.sort_by_key(|p| p.fixture);
        }
        let marks = self.marks.clone();
        if let Some(calibration) = self.movers.iter_mut().find(|c| c.fixture == fixture) {
            let ranges = calibration.ranges();
            calibration.solution = solve(&marks, &calibration.readings, ranges, position);
        }
        Ok(())
    }

    pub fn solution(&self, fixture: u32) -> Option<&Solution> {
        self.calibration(fixture).and_then(|c| c.solution.as_ref())
    }

    /// Checks the plan and replaces its size and marks, keeping calibrations.
    /// Readings against marks that no longer exist are dropped and movers are
    /// worked out again, as a moved mark changes the answer.
    pub fn set_plan(&mut self, plan: Plan) -> Result<()> {
        let Plan {
            width,
            depth,
            margin,
            riser,
            marks,
            scenery,
        } = plan;
        for piece in &scenery {
            if piece.points.len() < 2 {
                return Err(Error::InvalidStage(format!("{} needs at least two points", piece.name)));
            }
            let finite = piece.points.iter().flatten().all(|v| v.is_finite() && v.abs() <= 500.0);
            if !finite || !(0.0..=50.0).contains(&piece.height) {
                return Err(Error::InvalidStage(format!("{} is out of range", piece.name)));
            }
        }
        if !(width > 0.0 && depth > 0.0 && width <= 200.0 && depth <= 200.0) {
            return Err(Error::InvalidStage(
                "the stage must be between 0 and 200 m each way".into(),
            ));
        }
        if !(margin.sides().iter().all(|m| (0.0..=50.0).contains(m)) && (0.0..=10.0).contains(&riser)) {
            return Err(Error::InvalidStage(
                "the studio margin must be 0 to 50 m and the stage height 0 to 10 m".into(),
            ));
        }
        let mut ids: Vec<u32> = marks.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != marks.len() {
            return Err(Error::InvalidStage("two marks have the same number".into()));
        }
        if marks
            .iter()
            .any(|m| !(m.x.is_finite() && m.y.is_finite() && m.z.is_finite()))
        {
            return Err(Error::InvalidStage("a mark has no position".into()));
        }
        let moved = marks != self.marks;
        self.width = width;
        self.depth = depth;
        self.margin = margin;
        self.riser = riser;
        self.marks = marks;
        self.scenery = scenery;
        if moved {
            let marks = self.marks.clone();
            let positions = self.positions.clone();
            for calibration in &mut self.movers {
                calibration.readings.retain(|r| marks.iter().any(|m| m.id == r.mark));
                let ranges = calibration.ranges();
                let known = positions
                    .iter()
                    .find(|p| p.fixture == calibration.fixture)
                    .map_or([None; 3], |p| [p.x, p.y, p.z]);
                calibration.solution = solve(&marks, &calibration.readings, ranges, known);
            }
        }
        Ok(())
    }

    /// Records that `fixture` hits `mark` at this pan and tilt, replacing any
    /// earlier reading for that mark, and works the mover out again.
    pub fn record(&mut self, fixture: u32, mark: u32, pan: f32, tilt: f32, ranges: (f32, f32)) -> Result<&Calibration> {
        if !self.marks.iter().any(|m| m.id == mark) {
            return Err(Error::InvalidStage(format!("there is no mark {mark}")));
        }
        let index = match self.movers.iter().position(|c| c.fixture == fixture) {
            Some(index) => index,
            None => {
                self.movers.push(Calibration {
                    fixture,
                    readings: Vec::new(),
                    solution: None,
                });
                self.movers.len() - 1
            }
        };
        let known = self.position(fixture);
        let calibration = &mut self.movers[index];
        calibration.readings.retain(|r| r.mark != mark);
        calibration.readings.push(Reading { mark, pan, tilt });
        calibration.readings.sort_by_key(|r| r.mark);
        calibration.solution = solve(&self.marks, &calibration.readings, Some(ranges), known);
        Ok(&self.movers[index])
    }

    /// Forgets one reading, or the whole calibration when `mark` is `None`.
    pub fn forget(&mut self, fixture: u32, mark: Option<u32>) {
        match mark {
            None => self.movers.retain(|c| c.fixture != fixture),
            Some(mark) => {
                let marks = self.marks.clone();
                let known = self.position(fixture);
                if let Some(calibration) = self.movers.iter_mut().find(|c| c.fixture == fixture) {
                    calibration.readings.retain(|r| r.mark != mark);
                    let ranges = calibration.ranges();
                    calibration.solution = solve(&marks, &calibration.readings, ranges, known);
                }
            }
        }
    }
}

// --- Maths -----------------------------------------------------------------

type V3 = [f64; 3];
type M3 = [[f64; 3]; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn mul(m: &M3, v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}
fn mul_t(m: &M3, v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}
fn matmul(a: &M3, b: &M3) -> M3 {
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

/// Rotation by the vector's length (radians) round its direction.
fn rotation(v: V3) -> M3 {
    let angle = norm(v);
    if angle < 1e-12 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let [x, y, z] = scale(v, 1.0 / angle);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
    ]
}

/// Beam direction in the mover's own frame for pan and tilt angles.
fn local_direction(pan: f64, tilt: f64) -> V3 {
    [tilt.sin() * pan.cos(), tilt.sin() * pan.sin(), tilt.cos()]
}

/// A mover being fitted: a starting orientation plus the adjustments the
/// solver works on.
#[derive(Clone, Copy)]
struct Model {
    base: M3,
    base_ranges: (f64, f64),
    /// Measured parts of its position, which the fit holds the mover to.
    known: [Option<f64>; 3],
}

/// Parameters: position (3), rotation tweak (3), log scale of pan and tilt range (2).
type Params = [f64; 8];

impl Model {
    fn pose(&self, q: &Params) -> (V3, M3, f64, f64) {
        let rotation = matmul(&self.base, &rotation([q[3], q[4], q[5]]));
        let pan_range = self.base_ranges.0 * q[6].exp();
        let tilt_range = self.base_ranges.1 * q[7].exp();
        ([q[0], q[1], q[2]], rotation, pan_range, tilt_range)
    }

    /// Misses in metres for each reading, then a gentle pull towards the
    /// expected pan and tilt ranges so few readings still give a sensible answer.
    fn residuals(&self, q: &Params, points: &[(V3, f64, f64)], out: &mut Vec<f64>) {
        out.clear();
        let (position, rotation, pan_range, tilt_range) = self.pose(q);
        for &(target, pan, tilt) in points {
            let beam = mul(
                &rotation,
                local_direction((pan - 0.5) * pan_range, (tilt - 0.5) * tilt_range),
            );
            let to = sub(target, position);
            let distance = norm(to).max(1e-6);
            let miss = sub(beam, scale(to, 1.0 / distance));
            out.extend(miss.iter().map(|m| m * distance));
        }
        out.push(q[6] * 0.05);
        out.push(q[7] * 0.05);
        out.extend((0..3).map(|i| self.known[i].map_or(0.0, |k| (q[i] - k) * 100.0)));
    }
}

fn cost(r: &[f64]) -> f64 {
    r.iter().map(|v| v * v).sum()
}

/// Solves `a x = b` for a small square system, or `None` when it is singular.
#[allow(clippy::needless_range_loop)]
fn solve_linear<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let pivot = (col..N).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..N {
            let f = a[row][col] / a[col][col];
            for k in col..N {
                a[row][k] -= f * a[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let s: f64 = (row + 1..N).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - s) / a[row][row];
    }
    Some(x)
}

/// Levenberg-Marquardt from `q`, returning the fitted parameters and cost.
fn fit(model: &Model, points: &[(V3, f64, f64)], mut q: Params) -> (Params, f64) {
    let mut r = Vec::new();
    let mut trial = Vec::new();
    model.residuals(&q, points, &mut r);
    let mut current = cost(&r);
    let mut lambda = 1e-3;
    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); 8];
    for _ in 0..200 {
        // Jacobian by central differences.
        for (j, column) in columns.iter_mut().enumerate() {
            let h = 1e-6;
            let (mut up, mut down) = (q, q);
            up[j] += h;
            down[j] -= h;
            model.residuals(&up, points, &mut trial);
            let plus = trial.clone();
            model.residuals(&down, points, &mut trial);
            column.clear();
            column.extend(plus.iter().zip(&trial).map(|(a, b)| (a - b) / (2.0 * h)));
        }
        let mut jtj = [[0.0; 8]; 8];
        let mut jtr = [0.0; 8];
        for i in 0..8 {
            for k in 0..8 {
                jtj[i][k] = columns[i].iter().zip(&columns[k]).map(|(a, b)| a * b).sum();
            }
            jtr[i] = -columns[i].iter().zip(&r).map(|(a, b)| a * b).sum::<f64>();
        }
        let mut improved = false;
        for _ in 0..10 {
            let mut damped = jtj;
            for (i, row) in damped.iter_mut().enumerate() {
                row[i] += lambda * (jtj[i][i] + 1e-9);
            }
            let Some(step) = solve_linear(damped, jtr) else {
                lambda *= 10.0;
                continue;
            };
            let mut next = q;
            for (n, s) in next.iter_mut().zip(step) {
                *n += s;
            }
            model.residuals(&next, points, &mut trial);
            let c = cost(&trial);
            if c < current {
                let gain = current - c;
                q = next;
                std::mem::swap(&mut r, &mut trial);
                current = c;
                lambda = (lambda / 3.0).max(1e-12);
                improved = gain > 1e-14;
                break;
            }
            lambda *= 4.0;
        }
        if !improved {
            break;
        }
    }
    (q, current)
}

/// Works out where a mover hangs from its readings. `ranges` are the expected
/// pan and tilt travel in degrees; `None` keeps whatever an earlier fit found
/// (or the usual 540 by 270). Whatever is `known` of its position is held,
/// so fewer and less exact readings are needed; with all of it, only which
/// way it faces is worked out.
pub fn solve(marks: &[Mark], readings: &[Reading], ranges: Option<(f32, f32)>, known: Known) -> Option<Solution> {
    let needed = min_readings(&known);
    let known = known.map(|k| k.map(f64::from));
    let fully_known = known.iter().all(Option::is_some);
    let points: Vec<(V3, f64, f64)> = readings
        .iter()
        .filter_map(|r| {
            let mark = marks.iter().find(|m| m.id == r.mark)?;
            Some((mark.point(), f64::from(r.pan), f64::from(r.tilt)))
        })
        .collect();
    if points.len() < needed {
        return None;
    }
    let (pan_range, tilt_range) = ranges.unwrap_or((540.0, 270.0));
    let base_ranges = (f64::from(pan_range).to_radians(), f64::from(tilt_range).to_radians());

    // The marks' spread gives a sense of where to look for the mover.
    let (mut min, mut max) = ([f64::MAX; 3], [f64::MIN; 3]);
    for (p, _, _) in &points {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    let middle = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
    let reach = ((max[0] - min[0]).max(max[1] - min[1]) / 2.0).max(1.0);

    // Try hung (axis down) and standing (axis up) movers from a few places
    // and headings, then keep the best fit.
    let down: M3 = [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]];
    let up: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut best: Option<(Model, Params, f64)> = None;
    for (base, height) in [(down, max[2] + 4.0), (up, min[2] + 0.5)] {
        for heading in 0..8 {
            let yaw = rotation([0.0, 0.0, f64::from(heading) * std::f64::consts::FRAC_PI_4]);
            let model = Model {
                base: matmul(&yaw, &base),
                base_ranges,
                known,
            };
            // A known position needs only the one start for each heading.
            let offsets: &[(f64, f64)] = if fully_known {
                &[(0.0, 0.0)]
            } else {
                &[
                    (-1.0, -1.5),
                    (-1.0, 0.0),
                    (-1.0, 1.5),
                    (0.0, -1.5),
                    (0.0, 0.0),
                    (0.0, 1.5),
                    (1.0, -1.5),
                    (1.0, 0.0),
                    (1.0, 1.5),
                ]
            };
            for &(dx, dy) in offsets {
                let guess = [middle[0] + dx * reach, middle[1] + dy * reach, height];
                let [x, y, z] = [0, 1, 2].map(|i| known[i].unwrap_or(guess[i]));
                let start = [x, y, z, 0.0, 0.0, 0.0, 0.0, 0.0];
                let (q, c) = fit(&model, &points, start);
                if best.as_ref().is_none_or(|(_, _, b)| c < *b) {
                    best = Some((model, q, c));
                }
            }
        }
    }
    let (model, q, _) = best?;
    let (position, rotation, pan_range, tilt_range) = model.pose(&q);
    let errors = readings
        .iter()
        .filter_map(|r| {
            let mark = marks.iter().find(|m| m.id == r.mark)?;
            let beam = mul(
                &rotation,
                local_direction(
                    (f64::from(r.pan) - 0.5) * pan_range,
                    (f64::from(r.tilt) - 0.5) * tilt_range,
                ),
            );
            // Distance from the mark to the beam's line.
            let to = sub(mark.point(), position);
            let along = dot(to, beam);
            let miss = if along > 0.0 { norm(cross(to, beam)) } else { norm(to) };
            Some(MarkError {
                mark: r.mark,
                metres: miss as f32,
            })
        })
        .collect();
    let f = |v: f64| v as f32;
    Some(Solution {
        position: position.map(f),
        rotation: rotation.map(|row| row.map(f)),
        pan_range: f(pan_range.to_degrees()),
        tilt_range: f(tilt_range.to_degrees()),
        errors,
    })
}

impl Solution {
    fn matrix(&self) -> M3 {
        self.rotation.map(|row| row.map(f64::from))
    }

    /// Where the beam lands on the floor (or at `height`) for this pan and
    /// tilt, or `None` when it points up and away.
    pub fn landing(&self, pan: f32, tilt: f32, height: f32) -> Option<[f32; 2]> {
        let beam = mul(
            &self.matrix(),
            local_direction(
                (f64::from(pan) - 0.5) * f64::from(self.pan_range).to_radians(),
                (f64::from(tilt) - 0.5) * f64::from(self.tilt_range).to_radians(),
            ),
        );
        let position = self.position.map(f64::from);
        let drop = f64::from(height) - position[2];
        if beam[2].abs() < 1e-9 || drop / beam[2] <= 0.0 {
            return None;
        }
        let t = drop / beam[2];
        Some([(position[0] + beam[0] * t) as f32, (position[1] + beam[1] * t) as f32])
    }

    /// Pan and tilt that point at `target`, picking whichever way round is
    /// the shortest move from `current` so a mover never swings all the way
    /// round when it doesn't need to.
    pub fn aim(&self, target: [f32; 3], current: (f32, f32)) -> Aim {
        let pan_range = f64::from(self.pan_range).to_radians();
        let tilt_range = f64::from(self.tilt_range).to_radians();
        let (cur_pan, cur_tilt) = (f64::from(current.0), f64::from(current.1));
        let to = sub(target.map(f64::from), self.position.map(f64::from));
        let distance = norm(to);
        if distance < 1e-6 {
            return Aim {
                pan: current.0,
                tilt: current.1,
                reachable: false,
            };
        }
        let local = mul_t(&self.matrix(), scale(to, 1.0 / distance));
        let tilt = local[2].clamp(-1.0, 1.0).acos();
        // Straight along the axis, pan makes no difference: leave it be.
        let pan = if tilt.sin() < 1e-6 {
            (cur_pan - 0.5) * pan_range
        } else {
            local[1].atan2(local[0])
        };
        let turn = std::f64::consts::TAU;
        let mut best: Option<(f64, f64, f64, bool)> = None;
        for (pan, tilt) in [(pan, tilt), (pan + std::f64::consts::PI, -tilt)] {
            for k in -3..=3 {
                let p = 0.5 + (pan + f64::from(k) * turn) / pan_range;
                let t = 0.5 + tilt / tilt_range;
                let inside = (0.0..=1.0).contains(&p) && (0.0..=1.0).contains(&t);
                let (pc, tc) = (p.clamp(0.0, 1.0), t.clamp(0.0, 1.0));
                // Unreachable candidates are judged by how far off they are.
                let outside = ((p - pc) * pan_range).powi(2) + ((t - tc) * tilt_range).powi(2);
                let travel = ((pc - cur_pan) * pan_range).powi(2) + ((tc - cur_tilt) * tilt_range).powi(2);
                let score = if inside { travel } else { 1e6 + outside };
                if best.is_none_or(|(s, ..)| score < s) {
                    best = Some((score, pc, tc, inside));
                }
            }
        }
        let (_, pan, tilt, reachable) = best.unwrap_or((0.0, cur_pan, cur_tilt, false));
        Aim {
            pan: pan as f32,
            tilt: tilt as f32,
            reachable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn margin_reads_one_number_or_each_side() {
        let old: Stage = serde_json::from_str(r#"{"width":9.2,"depth":6.6,"margin":3}"#).unwrap();
        assert_eq!(old.margin, Margins::even(3.0));
        let new: Stage = serde_json::from_str(r#"{"margin":{"left":4.6,"right":0,"front":9.2,"back":0}}"#).unwrap();
        assert_eq!(new.margin.left, 4.6);
        assert_eq!(new.margin.back, 0.0);
        let again: Stage = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
        assert_eq!(again, new);
    }

    /// A made-up mover, used to fake readings at each mark.
    fn fake(position: V3, rotation: M3, ranges: (f64, f64)) -> Solution {
        let f = |v: f64| v as f32;
        Solution {
            position: position.map(f),
            rotation: rotation.map(|row| row.map(f)),
            pan_range: ranges.0 as f32,
            tilt_range: ranges.1 as f32,
            errors: Vec::new(),
        }
    }

    fn readings(mover: &Solution, marks: &[Mark]) -> Vec<Reading> {
        marks
            .iter()
            .map(|m| {
                let aim = mover.aim([m.x, m.y, m.z], (0.5, 0.5));
                assert!(aim.reachable);
                Reading {
                    mark: m.id,
                    pan: aim.pan,
                    tilt: aim.tilt,
                }
            })
            .collect()
    }

    fn hung(yaw_degrees: f64) -> M3 {
        let yaw = rotation([0.0, 0.0, yaw_degrees.to_radians()]);
        matmul(&yaw, &[[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]])
    }

    fn assert_close(a: [f32; 3], b: [f32; 3], within: f32) {
        let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        assert!(d < within, "{a:?} is {d} m from {b:?}");
    }

    #[test]
    fn finds_a_hung_mover_from_five_marks() {
        let marks = default_marks(6.0, 4.0);
        let truth = fake([1.5, -2.0, 4.5], hung(37.0), (540.0, 270.0));
        let solution = solve(&marks, &readings(&truth, &marks), Some((540.0, 270.0)), [None; 3]).unwrap();
        assert_close(solution.position, truth.position, 0.01);
        assert!(solution.errors.iter().all(|e| e.metres < 0.01), "{:?}", solution.errors);
        // And it aims where the real mover would.
        let aim = solution.aim([0.7, 2.2, 0.0], (0.5, 0.5));
        let landing = truth.landing(aim.pan, aim.tilt, 0.0).unwrap();
        assert!(
            (landing[0] - 0.7).abs() < 0.02 && (landing[1] - 2.2).abs() < 0.02,
            "{landing:?}"
        );
    }

    #[test]
    fn fits_an_unexpected_range_and_a_tilted_hang() {
        let marks = default_marks(8.0, 5.0);
        let tilted = matmul(&hung(-120.0), &rotation([0.25, -0.1, 0.0]));
        let truth = fake([-3.0, 6.0, 5.0], tilted, (630.0, 250.0));
        // Told to expect 540 by 270: the fit should still find the real ranges.
        let solution = solve(&marks, &readings(&truth, &marks), Some((540.0, 270.0)), [None; 3]).unwrap();
        assert_close(solution.position, truth.position, 0.05);
        assert!(solution.errors.iter().all(|e| e.metres < 0.03), "{:?}", solution.errors);
        let aim = solution.aim([1.0, 1.0, 0.0], (0.5, 0.5));
        let landing = truth.landing(aim.pan, aim.tilt, 0.0).unwrap();
        assert!(
            (landing[0] - 1.0).abs() < 0.05 && (landing[1] - 1.0).abs() < 0.05,
            "{landing:?}"
        );
    }

    #[test]
    fn finds_a_floor_standing_mover() {
        let marks = default_marks(6.0, 4.0);
        let standing = rotation([0.0, 0.0, 0.4]);
        // On the floor downstage, tilted back towards the stage.
        let truth = fake(
            [0.0, -3.0, 0.3],
            matmul(&standing, &rotation([-0.9, 0.0, 0.0])),
            (540.0, 270.0),
        );
        let solution = solve(&marks, &readings(&truth, &marks), Some((540.0, 270.0)), [None; 3]).unwrap();
        assert_close(solution.position, truth.position, 0.05);
    }

    #[test]
    fn a_bad_reading_shows_up_as_an_error() {
        let marks = default_marks(6.0, 4.0);
        let truth = fake([1.0, -1.0, 4.0], hung(10.0), (540.0, 270.0));
        let mut readings = readings(&truth, &marks);
        readings[4].pan += 0.03; // centre mark, about 16 degrees out
        let solution = solve(&marks, &readings, Some((540.0, 270.0)), [None; 3]).unwrap();
        let worst = solution
            .errors
            .iter()
            .max_by(|a, b| a.metres.total_cmp(&b.metres))
            .unwrap();
        assert!(worst.metres > 0.1, "{:?}", solution.errors);
    }

    #[test]
    fn needs_four_readings() {
        let marks = default_marks(6.0, 4.0);
        let truth = fake([1.0, -1.0, 4.0], hung(10.0), (540.0, 270.0));
        let readings = readings(&truth, &marks);
        assert!(solve(&marks, &readings[..3], None, [None; 3]).is_none());
        assert!(solve(&marks, &readings[..4], None, [None; 3]).is_some());
    }

    #[test]
    fn aims_the_short_way_round() {
        let truth = fake([0.0, 0.0, 4.0], hung(0.0), (540.0, 270.0));
        // Most spots can be reached a half or full turn of pan apart; starting
        // near the end of the range should pick a way round near that end.
        let near_middle = truth.aim([0.5, 1.0, 0.0], (0.5, 0.5));
        let near_end = truth.aim([0.5, 1.0, 0.0], (0.95, 0.5));
        assert!(near_middle.reachable && near_end.reachable);
        assert!(near_end.pan > near_middle.pan + 0.25, "{near_middle:?} {near_end:?}");
        // Both land in the same place.
        for aim in [near_middle, near_end] {
            let landing = truth.landing(aim.pan, aim.tilt, 0.0).unwrap();
            assert!((landing[0] - 0.5).abs() < 0.01 && (landing[1] - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn recording_keeps_one_reading_per_mark_and_moving_marks_resolves() {
        let mut stage = Stage::default();
        let truth = fake([1.0, -1.0, 4.0], hung(10.0), (540.0, 270.0));
        for r in readings(&truth, &stage.marks) {
            stage.record(7, r.mark, r.pan, r.tilt, (540.0, 270.0)).unwrap();
        }
        stage.record(7, 1, 0.1, 0.1, (540.0, 270.0)).unwrap();
        assert_eq!(stage.calibration(7).unwrap().readings.len(), 5);
        assert!(stage.record(7, 99, 0.5, 0.5, (540.0, 270.0)).is_err());

        let mut marks = stage.marks.clone();
        marks.retain(|m| m.id != 5);
        let plan = Plan {
            width: 8.0,
            depth: 6.0,
            margin: Margins::even(2.0),
            riser: 0.4,
            marks,
            scenery: vec![Scenery {
                name: "LED wall".into(),
                points: vec![[-3.8, 3.1], [-1.0, 5.6], [0.3, 5.6], [3.2, 3.1]],
                height: 3.0,
            }],
        };
        stage.set_plan(plan).unwrap();
        assert_eq!(stage.floor_at(0.0, 3.0), 0.0);
        assert_eq!(stage.floor_at(0.0, -1.0), -0.4);
        assert_eq!(stage.scenery.len(), 1);
        let mut bad = Plan {
            width: 8.0,
            depth: 6.0,
            margin: Margins::even(2.0),
            riser: 0.4,
            marks: stage.marks.clone(),
            scenery: vec![Scenery {
                name: "Flat".into(),
                points: vec![[0.0, 0.0]],
                height: 2.0,
            }],
        };
        assert!(stage.set_plan(bad.clone()).is_err());
        bad.scenery[0].points.push([1.0, 0.0]);
        assert!(stage.set_plan(bad).is_ok());
        assert_eq!(stage.calibration(7).unwrap().readings.len(), 4);
        stage.forget(7, Some(1));
        assert!(stage.solution(7).is_none());
        stage.forget(7, None);
        assert!(stage.calibration(7).is_none());
    }

    #[test]
    fn a_known_position_needs_only_two_rough_readings() {
        let marks = default_marks(8.0, 6.0);
        let truth = fake([2.0, -1.5, 4.0], hung(-50.0), (540.0, 270.0));
        let mut readings = readings(&truth, &marks);
        // A wide wash: the pool centred by eye, a degree or so out.
        for (i, r) in readings.iter_mut().enumerate() {
            r.pan += if i % 2 == 0 { 0.002 } else { -0.002 };
            r.tilt += if i % 3 == 0 { 0.003 } else { -0.002 };
        }
        let known = [Some(2.0), Some(-1.5), Some(4.0)];
        assert!(solve(&marks, &readings[..1], None, known).is_none());
        let solution = solve(&marks, &readings[..2], Some((540.0, 270.0)), known).unwrap();
        assert_close(solution.position, truth.position, 0.01);
        let solution = solve(&marks, &readings, Some((540.0, 270.0)), known).unwrap();
        let aim = solution.aim([0.0, 3.0, 0.0], (0.5, 0.5));
        let landing = truth.landing(aim.pan, aim.tilt, 0.0).unwrap();
        assert!(
            (landing[0]).abs() < 0.15 && (landing[1] - 3.0).abs() < 0.15,
            "{landing:?}"
        );
    }

    #[test]
    fn setting_a_position_refits() {
        let mut stage = Stage::default();
        let truth = fake([1.0, -1.0, 4.0], hung(10.0), (540.0, 270.0));
        for r in readings(&truth, &stage.marks).into_iter().take(2) {
            stage.record(3, r.mark, r.pan, r.tilt, (540.0, 270.0)).unwrap();
        }
        assert!(stage.solution(3).is_none());
        stage.set_position(3, [Some(1.0), Some(-1.0), Some(4.0)]).unwrap();
        assert!(stage.solution(3).is_some());
        assert_eq!(stage.position(3), [Some(1.0), Some(-1.0), Some(4.0)]);
        stage.set_position(3, [None; 3]).unwrap();
        assert!(stage.solution(3).is_none());
        assert!(stage.positions.is_empty());
        assert!(stage.set_position(3, [Some(f32::NAN), None, None]).is_err());
    }

    #[test]
    fn a_known_height_helps_with_rough_readings() {
        let marks = default_marks(8.0, 6.0);
        let truth = fake([-2.5, -1.0, 4.5], hung(25.0), (540.0, 270.0));
        let mut readings = readings(&truth, &marks);
        for (i, r) in readings.iter_mut().enumerate() {
            r.pan += if i % 2 == 0 { 0.002 } else { -0.002 };
            r.tilt += if i % 3 == 0 { 0.003 } else { -0.002 };
        }
        let height = [None, None, Some(4.5)];
        assert!(solve(&marks, &readings[..2], None, height).is_none());
        let solution = solve(&marks, &readings, Some((540.0, 270.0)), height).unwrap();
        assert!((solution.position[2] - 4.5).abs() < 0.01);
        // Aiming is what matters: it should land close to where it is sent.
        for spot in [[0.0, 3.0, 0.0], [2.0, 1.0, 0.0], [-3.0, 5.0, 1.6]] {
            let aim = solution.aim(spot, (0.5, 0.5));
            let landing = truth.landing(aim.pan, aim.tilt, spot[2]).unwrap();
            let miss = ((landing[0] - spot[0]).powi(2) + (landing[1] - spot[1]).powi(2)).sqrt();
            assert!(miss < 0.2, "{spot:?} missed by {miss}");
        }
    }
}
