//! A trackpad swipe's release where the platform gives no momentum of its own (Linux): the
//! velocity the fingers leave with, the fling it starts (`runtime/scroll/fling.rs`), a finger
//! landing that stops it, and a quick second swipe that adds to it. As Chromium's Wayland input
//! does, from its sources at 2026-11-17: `ui/ozone/platform/wayland/host/wayland_event_source.cc`
//! for the release and the stop, `ui/events/blink/fling_booster.cc` for the boost.
//!
//! Times are the platform's own event times where it gives them (`WheelEvent::time`), so
//! frames that wait behind a slow paint keep their spacing; else the runtime clock.
use super::*;
use crate::machine::Attempt;
use crate::runtime::scroll_fling::Fling;
use crate::runtime::scroll_x::WheelOwner;
use crate::{GesturePhase, Mods, WheelEvent};
use std::collections::VecDeque;

/// Platforms whose trackpads coast by themselves (macOS reports momentum as wheel deltas).
const THROWS: bool = !cfg!(target_os = "macos");
/// Scroll frames the release velocity is fitted to (`kPointerScrollDataSetMaxSize`).
const FRAMES: usize = 3;
/// A lift more than this long after the last frame throws nothing (`kFlingStartTimeoutMs`),
/// seconds.
const START_TIMEOUT: f64 = 0.2;
/// A second swipe adds to a fling when it begins within this of the stop, and keeps each of its
/// movements within this of the last (`kFlingBoostTimeoutDelay`), seconds.
const BOOST_WINDOW: f64 = 0.05;
/// Both flings move at least this fast to add up (`kMinBoostFlingSpeedSquare`), points a second.
const BOOST_FLING_SPEED: f64 = 350.0;
/// The second swipe moves at least this fast between its movements
/// (`kMinBoostTouchScrollSpeedSquare`), points a second.
const BOOST_SCROLL_SPEED: f64 = 150.0;

type V2 = (f64, f64);

fn dot(a: V2, b: V2) -> f64 { a.0 * b.0 + a.1 * b.1 }
fn len2(a: V2) -> f64 { dot(a, a) }

/// One frame of scroll deltas and the seconds since the frame before it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    d: V2,
    dt: f64,
    /// From fingers (a precise delta), not a wheel's steps.
    finger: bool,
}

/// What the trackpad's releases know between frames.
#[derive(Debug, Default)]
pub(in crate::runtime) struct Throw {
    /// The latest scroll frames, newest first.
    frames: VecDeque<Frame>,
    /// When the last scroll frame came, and the last that moved.
    last: Option<f64>,
    moved: f64,
    boost: Boost,
    /// What the running fling moves, while one may run.
    flung: Option<Flung>,
    /// A release whose fling starts when its gesture's queued deltas have run.
    release: Option<Release>,
}

/// The viewport a fling moves, on one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Flung {
    id: NodeId,
    horizontal: bool,
}

/// A release waiting for its gesture's turn: the velocity it left with (deltas a second), the
/// boost it took, and how long after the last movement it came.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Release {
    gesture: Attempt,
    v: V2,
    boost: f64,
    lead: f64,
}

/// Whether a fling can still be added to, as Chromium's `FlingBooster` decides.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Boost {
    #[default]
    Off,
    /// A fling runs that left at `start` (deltas a second) with `mods` held.
    Running { start: V2, mods: Mods },
    /// A fling stopped while moving at `now`: a release in `start`'s direction by `until` adds
    /// `start` to its own. `last` is when the swipe since last moved.
    Open { start: V2, now: V2, mods: Mods, until: f64, last: Option<f64> },
}

impl Boost {
    /// Past its window at `t`, it is off.
    fn at(self, t: f64) -> Self {
        match self {
            Boost::Open { until, .. } if t > until => Boost::Off,
            boost => boost,
        }
    }

    /// The running fling stopped at `t`, moving at `now` (a finger landed or a swipe began).
    fn stopped(self, t: f64, now: V2) -> Self {
        match self.at(t) {
            Boost::Running { start, mods } => Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last: None },
            Boost::Open { start, now, mods, .. } => Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last: None },
            Boost::Off => Boost::Off,
        }
    }

    /// A swipe began at `t`.
    fn began(self, t: f64) -> Self {
        match self.at(t) {
            Boost::Open { start, now, mods, last, .. } => Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last },
            boost => boost,
        }
    }

    /// The swipe moved `d` at `t`: it must head the fling's way, fast enough, and soon enough.
    fn moved(self, t: f64, d: V2) -> Self {
        let Boost::Open { start, now, mods, last, .. } = self.at(t) else { return self.at(t) };
        if dot(start, d) <= 0.0 { return Boost::Off; }
        if let Some(last) = last {
            let gap = t - last;
            if gap >= 0.001 && len2((d.0 / gap, d.1 / gap)) < BOOST_SCROLL_SPEED * BOOST_SCROLL_SPEED { return Boost::Off; }
        }
        Boost::Open { start, now, mods, until: t + BOOST_WINDOW, last: Some(t) }
    }

    /// A release at `t` with velocity `v`: the velocity the fling takes, and how many times the
    /// release's own that is. The fling runs from here.
    fn released(self, t: f64, v: V2, mods: Mods) -> (V2, f64, Self) {
        let min = BOOST_FLING_SPEED * BOOST_FLING_SPEED;
        let boosted = match self.at(t) {
            Boost::Open { start, now, mods: held, .. } if held == mods && dot(start, v) > 0.0 && len2(now) >= min && len2(v) >= min =>
                (v.0 + start.0, v.1 + start.1),
            _ => v,
        };
        let peak = |v: V2| v.0.abs().max(v.1.abs());
        let boost = if peak(v) > 0.0 { peak(boosted) / peak(v) } else { 1.0 };
        (boosted, boost, if len2(boosted) > 0.0 { Boost::Running { start: boosted, mods } } else { Boost::Off })
    }
}

impl Throw {
    /// A frame came at `t`: the seconds since the one before.
    fn gap(&mut self, t: f64) -> f64 {
        let dt = self.last.map_or(0.0, |last| (t - last).max(0.0));
        self.last = Some(t);
        dt
    }

    /// The fingers left at `t` after the frames so far: their velocity, fitted by least squares
    /// to the last three frames as Chromium (and ChromeOS's touchpad library) fits it.
    fn velocity(&mut self, stop: f64) -> V2 {
        let count = self.frames.len();
        if count == 0 || stop > START_TIMEOUT { return (0.0, 0.0); }
        if count == 1 {
            // Chromium's own: a product, not a quotient, so a single frame barely throws.
            let f = self.frames[0];
            let dt = f.dt + stop;
            return (f.d.0 * dt, f.d.1 * dt);
        }
        let (mut time, mut x, mut y) = (stop, 0.0, 0.0);
        let (mut tt, mut ts, mut tx, mut ty, mut xs, mut ys) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for f in &self.frames {
            if !f.finger { break; }
            time += f.dt;
            x += f.d.0;
            y += f.d.1;
            tt += time * time;
            ts += time;
            tx += time * x;
            ty += time * y;
            xs += x;
            ys += y;
        }
        self.frames.clear();
        let n = count as f64;
        let det = n * tt - ts * ts;
        if det == 0.0 {
            return if ts != 0.0 { (x / ts, y / ts) } else { (0.0, 0.0) };
        }
        ((n * tx - ts * xs) / det, (n * ty - ts * ys) / det)
    }
}

impl RuntimeCore {
    /// The running trackpad fling's velocity in deltas a second, while one runs.
    fn flinging(&self) -> Option<V2> {
        let flung = self.scroll.throw.flung?;
        let st = self.scroll.scrolls.get(&flung.id)?;
        if flung.horizontal {
            match st.x.motion {
                XMotion::Fling(f) => Some((st.x.toward(f.velocity()), 0.0)),
                _ => None,
            }
        } else {
            match st.mode {
                ScrollMode::Fling { fling, .. } => Some((0.0, -fling.velocity())),
                _ => None,
            }
        }
    }

    /// Stops the running trackpad fling at `t`, as a finger landing or a new swipe does, and
    /// opens the window in which a swipe the same way adds to it.
    fn stop_throw(&mut self, t: f64) {
        let Some(now) = self.flinging() else {
            self.scroll.throw.flung = None;
            if matches!(self.scroll.throw.boost, Boost::Running { .. }) { self.scroll.throw.boost = Boost::Off; }
            return;
        };
        let Some(flung) = self.scroll.throw.flung.take() else { return };
        if let Some(st) = self.scroll.scrolls.get_mut(&flung.id) {
            if flung.horizontal {
                if matches!(st.x.motion, XMotion::Fling(_)) { st.x.motion = XMotion::Rest; }
            } else {
                st.halt();
            }
        }
        self.scroll.throw.boost = self.scroll.throw.boost.stopped(t, now);
        log_event!(self, "fling stopped");
    }

    /// A trackpad's fingers landed (Wayland's hold gesture) at platform time `time`, or now:
    /// a running fling stops where it is, and nothing the fingers did before throws. The app
    /// hears it too (`on_touchpad_hold`), for flings of its own.
    pub(crate) fn touchpad_hold(&mut self, app: &mut dyn AppDriver, time: Option<f64>) {
        if !THROWS { return; }
        app.on_touchpad_hold();
        if self.flinging().is_none() { return; }
        let t = time.unwrap_or_else(|| self.now_clock());
        self.scroll.throw.frames.clear();
        self.stop_throw(t);
    }

    /// Records wheel event `ev` as it arrives: a frame of a swipe, or its release. A finger
    /// scroll stops a running fling first.
    pub(super) fn throw_arrived(&mut self, ev: &WheelEvent) {
        if !THROWS { return; }
        let t = ev.time.unwrap_or_else(|| self.now_clock());
        let moved = ev.dx != 0.0 || ev.dy != 0.0;
        let dt = self.scroll.throw.gap(t);
        if ev.phase == GesturePhase::Ended && !moved {
            let v = self.scroll.throw.velocity(dt);
            let (v, boost, state) = self.scroll.throw.boost.released(t, v, ev.mods);
            self.scroll.throw.boost = state;
            let gesture = self.scroll.wheel_gesture();
            self.scroll.throw.release = (len2(v) > 0.0).then_some(Release { gesture, v, boost, lead: t - self.scroll.throw.moved });
            return;
        }
        if ev.precise && self.flinging().is_some() { self.stop_throw(t); }
        if ev.phase == GesturePhase::Began { self.scroll.throw.boost = self.scroll.throw.boost.began(t); }
        let d = (f64::from(ev.dx), f64::from(ev.dy));
        if moved {
            self.scroll.throw.boost = self.scroll.throw.boost.moved(t, d);
            self.scroll.throw.moved = t;
        }
        if self.scroll.throw.frames.len() == FRAMES { self.scroll.throw.frames.pop_back(); }
        self.scroll.throw.frames.push_front(Frame { d, dt, finger: ev.precise });
    }

    /// Gesture `gesture`'s release reached scrolling, untaken by any `on_wheel` handler: its
    /// fling starts on the viewport the gesture scrolled, at (x, y) when vertical.
    pub(super) fn throw_released(&mut self, gesture: Attempt, x: f32, y: f32) {
        let Some(release) = self.scroll.throw.release.take_if(|r| r.gesture == gesture) else { return };
        let target = match self.scroll.wheel_owner_of(gesture) {
            Some(WheelOwner::Vertical) => self.scroll_at(x, y).map(|id| Flung { id, horizontal: false }),
            Some(WheelOwner::Horizontal(id)) => Some(Flung { id: *id, horizontal: true }),
            _ => None,
        };
        let Some(flung) = target else {
            self.scroll.throw.boost = Boost::Off;
            return;
        };
        let (w, h) = self.viewport;
        let (zoom, extent) = (f64::from(self.zoom()), if flung.horizontal { f64::from(w) } else { f64::from(h) });
        let Some(st) = self.scroll.scrolls.get_mut(&flung.id) else { return };
        let v = if flung.horizontal { st.x.toward(release.v.0) } else { -release.v.1 };
        let Some(fling) = Fling::new(v, extent / zoom, release.boost, release.lead) else {
            self.scroll.throw.boost = Boost::Off;
            return;
        };
        if flung.horizontal {
            st.x.motion = XMotion::Fling(fling);
        } else {
            st.mode = st.mode.to_fling(fling);
        }
        self.scroll.throw.flung = Some(flung);
        self.release_hover_hold();
        self.drop_scroll_holds();
        self.dismiss_help();
        self.pointer.scroll_cancels_press(&self.tree.nodes, flung.id);
        log_event!(self, "fling {v:.0}");
    }
}
