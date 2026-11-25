//! The coast after a trackpad swipe where the platform gives none (Linux): Chromium's fling,
//! from its sources at 2026-11-17. The fingers' last frames set a velocity
//! (`runtime/input/throw.rs`), and the offset then travels a set distance on an ease-out cubic
//! Bézier whose start matches that velocity, as Chromium's `PhysicsBasedFlingCurve`
//! (`ui/events/gestures/physics_based_fling_curve.cc`, on by default on Linux and Windows
//! through `features::kExperimentalFlingAnimation`) does.
//!
//! The distance is how far a body at the release speed travels at a constant deceleration of
//! 2.7559e-5 inches per ms² at 96 points per inch (`kDefaultPixelsPerInch`), at most three
//! window heights (or widths) times the boost (`runtime/input/throw.rs`). The duration is the
//! release speed over 2,300 points per s², at most 2 s. All in the axis's offset space: positive
//! raises the offset.

/// Deceleration that sets the distance: Chromium's `kDefaultPhysicalDeceleration` (inches per
/// ms²) at `kDefaultPixelsPerInch`, in points per s².
const DECELERATION: f64 = 2.7559e-5 * 96.0 * 1e6;
/// Deceleration that sets the duration (`kDefaultPixelDeceleration`), points per s².
const DURATION_DECELERATION: f64 = 2300.0;
/// The longest a fling runs (`kMaxCurveDurationForFling`), seconds.
const MAX_DURATION: f64 = 2.0;
/// How many window extents a fling may travel at most, before any boost
/// (`kDefaultBoundsMultiplier`).
const BOUNDS: f64 = 3.0;
/// The curve's control points before they are fitted to the release speed (`kDefaultP1X` ...).
const P1: (f64, f64) = (0.2, 1.0);
const P2: (f64, f64) = (0.55, 1.0);
/// Chromium's fling controller starts the curve at the last scroll update, unless its first
/// step comes more than two frames after the fling began; then it starts one frame back
/// (`components/input/fling_controller.cc`, `kMaxMicrosecondsFromFlingTimestampToFirstProgress`).
const FRAME: f64 = 1.0 / 60.0;

/// A released scroll coasting on one axis: where it is on its curve and how far it has carried
/// the offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct Fling {
    /// How far the whole fling carries the offset, signed.
    distance: f64,
    /// Seconds it runs.
    duration: f64,
    ease: Bezier,
    /// Seconds into the curve, and the part of `distance` already applied.
    t: f64,
    at: f64,
    /// It has taken its first step (the step that may move its start, see `FRAME`).
    stepped: bool,
}

impl Fling {
    /// A fling released at `v` (offset points a second), in a window `extent` points along the
    /// axis, boosted `boost` times (1 unless a fling added to a running one), whose last scroll
    /// update came `lead` seconds ago. None when it wouldn't move.
    pub(in crate::runtime) fn new(v: f64, extent: f64, boost: f64, lead: f64) -> Option<Self> {
        let speed = v.abs();
        let bound = (extent * boost * BOUNDS).floor();
        if !(speed > 0.0 && bound > 0.0) { return None; }
        // Travel to a stop at a constant deceleration: v² / 2a, inside the bound.
        let duration_to_stop = speed / DECELERATION;
        let reach = ((speed - DECELERATION * duration_to_stop * 0.5) * duration_to_stop).min(bound);
        let duration = MAX_DURATION.min(speed / DURATION_DECELERATION);
        // Fit the first control point so the curve leaves at `speed`: raise its y to 1, then
        // move its x left.
        let slope = speed * duration / reach;
        let p1 = if slope * P1.0 < 1.0 { (P1.0, P1.0 * slope) } else { (P1.1 / slope, P1.1) };
        Some(Fling { distance: reach.copysign(v), duration, ease: Bezier::new(p1, P2), t: lead.max(0.0), at: 0.0, stepped: false })
    }

    /// Steps `dt` seconds: the offset change, and whether the fling runs on after it. Its last
    /// step lands it on `distance` exactly.
    pub(in crate::runtime) fn step(&mut self, dt: f64) -> (f64, bool) {
        self.t += dt;
        if !self.stepped {
            self.stepped = true;
            if self.t >= 2.0 * FRAME { self.t = FRAME; }
        }
        let running = self.t < self.duration;
        let to = if running { self.distance * self.ease.solve(self.t / self.duration) } else { self.distance };
        let delta = to - self.at;
        self.at = to;
        (delta, running)
    }

    /// Its speed now, in offset points a second (signed): the curve's slope.
    pub(in crate::runtime) fn velocity(&self) -> f64 {
        if self.t >= self.duration { return 0.0; }
        self.distance / self.duration * self.ease.slope(self.t / self.duration)
    }
}

/// A CSS-style cubic Bézier easing from (0, 0) to (1, 1), solved as Chromium's
/// `gfx::CubicBezier` solves it: Newton's method, then bisection.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bezier {
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
}

impl Bezier {
    fn new(p1: (f64, f64), p2: (f64, f64)) -> Self {
        let c = (3.0 * p1.0, 3.0 * p1.1);
        let b = (3.0 * (p2.0 - p1.0) - c.0, 3.0 * (p2.1 - p1.1) - c.1);
        Bezier { a: (1.0 - c.0 - b.0, 1.0 - c.1 - b.1), b, c }
    }

    fn x(&self, t: f64) -> f64 { ((self.a.0 * t + self.b.0) * t + self.c.0) * t }
    fn y(&self, t: f64) -> f64 { ((self.a.1 * t + self.b.1) * t + self.c.1) * t }
    fn dx(&self, t: f64) -> f64 { (3.0 * self.a.0 * t + 2.0 * self.b.0) * t + self.c.0 }
    fn dy(&self, t: f64) -> f64 { (3.0 * self.a.1 * t + 2.0 * self.b.1) * t + self.c.1 }

    /// The curve's parameter where its x is `x` (`CubicBezier::SolveCurveX`).
    fn t_at(&self, x: f64) -> f64 {
        const EPSILON: f64 = 1e-7;
        let mut t = x;
        for _ in 0..4 {
            let off = self.x(t) - x;
            if off.abs() < EPSILON { return t; }
            let d = self.dx(t);
            if d.abs() < EPSILON { break; }
            t -= off / d;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        t = x;
        while lo < hi {
            let at = self.x(t);
            if (at - x).abs() < EPSILON { return t; }
            if x > at { lo = t } else { hi = t }
            t = (lo + hi) * 0.5;
        }
        t
    }

    /// Progress at time fraction `x`.
    fn solve(&self, x: f64) -> f64 { self.y(self.t_at(x.clamp(0.0, 1.0))) }

    /// d(progress)/d(time fraction) at `x`.
    fn slope(&self, x: f64) -> f64 {
        let t = self.t_at(x.clamp(0.0, 1.0));
        let dx = self.dx(t);
        if dx.abs() < 1e-12 { 0.0 } else { self.dy(t) / dx }
    }
}

#[cfg(test)]
#[path = "fling_tests.rs"]
mod tests;
