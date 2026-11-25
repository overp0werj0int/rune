use super::*;

/// Steps `fling` to each time in `at` (seconds into its curve) and returns the offset it has
/// carried there.
fn carried(mut fling: Fling, at: &[f64]) -> Vec<f64> {
    let (mut t, mut total) = (0.0, 0.0);
    at.iter().map(|&to| {
        total += fling.step(to - t).0;
        t = to;
        (total * 1e4).round() / 1e4
    }).collect()
}

/// The reference numbers come from a line-by-line Python port of Chromium's
/// `PhysicsBasedFlingCurve` and `gfx::CubicBezier`, not from this code.
#[test]
fn a_fling_follows_chromiums_curve() {
    let fling = |v| Fling::new(v, 1000.0, 1.0, 0.0).unwrap();
    assert_eq!(carried(fling(3000.0), &[0.01, 0.05, 0.2, 0.5]).split_off(1), [144.749, 525.257, 1098.4855]);
    assert_eq!(carried(fling(-1200.0), &[0.01, 0.05, 0.2, 0.5]).split_off(1), [-55.0737, -175.7577, -271.7227]);
    assert_eq!(carried(fling(10000.0), &[0.01, 0.05, 0.2, 0.5]).split_off(1), [437.93, 1309.7331, 2214.098]);
    let short = Fling::new(400.0, 800.0, 1.0, 0.0).unwrap();
    assert_eq!(carried(short, &[0.01, 0.05, 0.2]).split_off(1), [15.761, 30.2382]);
}

#[test]
fn a_fling_leaves_at_the_release_speed_and_lands_on_its_distance() {
    let mut fling = Fling::new(3000.0, 1000.0, 1.0, 0.0).unwrap();
    let mut total = fling.step(1e-4).0;
    assert!((fling.velocity() - 3000.0).abs() < 5.0, "{}", fling.velocity());
    let mut steps = 0;
    loop {
        let (d, running) = fling.step(1.0 / 120.0);
        total += d;
        steps += 1;
        if !running { break; }
        assert!(d > 0.0, "it never turns back");
    }
    assert!((total - 1700.8963).abs() < 1e-3, "{total}");
    assert!(steps < 160, "1.3 s at 120 Hz: {steps}");
    assert_eq!(fling.velocity(), 0.0);
}

#[test]
fn a_fling_travels_at_most_three_windows_more_when_boosted() {
    let reach = |boost| carried(Fling::new(20000.0, 900.0, boost, 0.0).unwrap(), &[0.001, 3.0])[1];
    assert_eq!(reach(1.0), 2700.0);
    assert_eq!(reach(1.5), 4050.0);
    assert!(Fling::new(0.0, 900.0, 1.0, 0.0).is_none());
    assert!(Fling::new(500.0, 0.0, 1.0, 0.0).is_none());
}

/// Chromium starts the curve at the last scroll update, so the lift's delay is already spent,
/// unless the first step comes two frames late: then it starts one frame in.
#[test]
fn a_late_first_step_starts_one_frame_in() {
    let first = |lead, dt| Fling::new(3000.0, 1000.0, 1.0, lead).unwrap().step(dt).0;
    let one_frame = first(0.0, 1.0 / 60.0);
    assert_eq!(first(0.25, 1.0 / 60.0), one_frame, "250 ms after the last update");
    assert!(first(0.01, 1.0 / 120.0) > one_frame, "18 ms in: still on the curve's own clock");
}
