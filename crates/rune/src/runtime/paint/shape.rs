//! Painter-stream instances for vector shapes (`El::shape`).
use super::*;

/// Instances for a vector shape whose origin sits at `origin` (device px), with `k` device px per
/// coordinate unit on each axis and `k_px` device px per point for stroke widths. A polyline's
/// segments, and a stroke that starts where the one before it ended, meet on a [`bisector`].
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime) fn push_shape(out: &mut Vec<Instance>, shape: &Shape, origin: [f32; 2], k: [f32; 2], k_px: f32, opacity: f32, srgb: bool, clip: [f32; 4], fade: [f32; 4]) {
    let at = |p: [f32; 2]| [origin[0] + p[0] * k[0], origin[1] + p[1] * k[1]];
    // The direction a stroke leaves `p` in, from the next op, when that op continues it.
    let next_dir = |idx: usize, end: [f32; 2]| match shape.ops.get(idx + 1) {
        Some(Op::Stroke { points, .. }) if points.len() >= 2 && points[0] == end => Some(unit(at(points[0]), at(points[1]))),
        _ => None,
    };
    let mut prev: Option<([f32; 2], [f32; 2])> = None;
    for (idx, op) in shape.ops.iter().enumerate() {
        match op {
            Op::Stroke { points, closed, width, end_width, cap, color } => {
                let fill = premul(*color, opacity, srgb);
                let n = points.len();
                let count = if *closed { n } else { n - 1 };
                let pts: Vec<[f32; 2]> = points.iter().map(|&p| at(p)).collect();
                let dirs: Vec<[f32; 2]> = (0..count).map(|i| unit(pts[i], pts[(i + 1) % n])).collect();
                let into = if *closed { dirs.last().copied() } else { prev.filter(|(end, _)| *end == points[0]).map(|(_, d)| d) };
                let onward = if *closed { dirs.first().copied() } else { next_dir(idx, points[n - 1]) };
                prev = Some((points[n - 1], dirs[count - 1]));
                if fill[3] <= 0.0005 {
                    continue;
                }
                // A taper's half-width at each point, by how far along it the point is.
                let total: f32 = (0..count).map(|i| dist(pts[i], pts[(i + 1) % n])).sum();
                let mut run = 0.0;
                let half: Vec<f32> = std::iter::once(0.0).chain((0..count).map(|i| { run += dist(pts[i], pts[(i + 1) % n]); run })).map(|r| {
                    let w = end_width.map_or(*width, |e| width + (e - width) * if total > 0.0 { r / total } else { 0.0 });
                    w * k_px * 0.5
                }).collect();
                for i in 0..count {
                    let (a, b) = (pts[i], pts[(i + 1) % n]);
                    let (hw, hw_b) = (half[i], half[i + 1]);
                    let pad = hw.max(hw_b).max(0.5) + 1.0;
                    let before = if i == 0 { into } else { Some(dirs[i - 1]) };
                    let after = if i + 1 == count { onward } else { Some(dirs[i + 1]) };
                    let cut_a = before.map_or([0.0; 2], |d| bisector(d, dirs[i]));
                    let cut_b = after.map_or([0.0; 2], |d| bisector(dirs[i], d));
                    let (x0, y0) = (a[0].min(b[0]) - pad, a[1].min(b[1]) - pad);
                    out.push(Instance {
                        rect: [x0, y0, a[0].max(b[0]) + pad - x0, a[1].max(b[1]) + pad - y0],
                        radii: [a[0], a[1], b[0], b[1]],
                        fill,
                        // The half-width at `b`, where it differs (`segment` in shader.wgsl).
                        border: [if hw_b != hw { hw_b.max(1e-3) } else { 0.0 }, 0.0, 0.0, 0.0],
                        // Which ends are round: both under a round cap, and a joined end under either.
                        params: [hw, KIND_SEGMENT, {
                            let round = |cut: [f32; 2]| *cap == LineCap::Round || cut != [0.0; 2];
                            f32::from(u8::from(round(cut_a)) + 2 * u8::from(round(cut_b)))
                        }, 1.0],
                        uv: [cut_a[0], cut_a[1], cut_b[0], cut_b[1]],
                        clip,
                        fade: crate::render::pack_fade(fade, clip, clip),
                    });
                }
            }
            Op::Fill { points, color } => {
                prev = None;
                let fill = premul(*color, opacity, srgb);
                if fill[3] <= 0.0005 {
                    continue;
                }
                let tris = crate::shape::triangulate(points);
                push_triangles(out, points, &tris, at, fill, clip, fade);
            }
            Op::FillCompound { contours, rule, color } => {
                prev = None;
                let fill = premul(*color, opacity, srgb);
                if fill[3] <= 0.0005 {
                    continue;
                }
                let (verts, tris) = crate::shape::triangulate_compound(contours, *rule);
                push_triangles(out, &verts, &tris, at, fill, clip, fade);
            }
            Op::FillArea { area, color } => {
                prev = None;
                let fill = premul(*color, opacity, srgb);
                if fill[3] <= 0.0005 {
                    continue;
                }
                let (verts, tris) = area.triangles();
                push_triangles(out, verts, tris, at, fill, clip, fade);
            }
        }
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

fn unit(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = (dx * dx + dy * dy).sqrt();
    if l > 1e-6 { [dx / l, dy / l] } else { [0.0; 2] }
}

/// The plane two strokes meet on at a joint, the first leaving along `d0` and the next along
/// `d1`: its normal, the bisector of the two. Each segment shades only its own side, so a joint
/// is drawn once, neither doubled at its antialiased edge nor seamed, and its outer side stays
/// round, since both sides measure from the joint. A turn of 90 degrees or more keeps the round
/// overlap (zero: no plane), as an arrowhead's barbs meet its shaft.
fn bisector(d0: [f32; 2], d1: [f32; 2]) -> [f32; 2] {
    if d0[0] * d1[0] + d0[1] * d1[1] <= 0.0 {
        return [0.0; 2];
    }
    unit([0.0; 2], [d0[0] + d1[0], d0[1] + d1[1]])
}

#[allow(clippy::too_many_arguments)]
fn push_triangles(
    out: &mut Vec<Instance>,
    verts: &[[f32; 2]],
    tris: &[([usize; 3], u32)],
    at: impl Fn([f32; 2]) -> [f32; 2],
    fill: [f32; 4],
    clip: [f32; 4],
    fade: [f32; 4],
) {
    for &([ia, ib, ic], aa) in tris {
        let (a, b, c) = (at(verts[ia]), at(verts[ib]), at(verts[ic]));
        let (x0, y0) = (a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1]));
        let (x1, y1) = (a[0].max(b[0]).max(c[0]), a[1].max(b[1]).max(c[1]));
        out.push(Instance {
            rect: [x0, y0, x1 - x0, y1 - y0],
            radii: [a[0], a[1], b[0], b[1]],
            fill,
            border: [0.0; 4],
            params: [aa as f32, KIND_TRIANGLE, 0.0, 1.0],
            uv: [c[0], c[1], 0.0, 0.0],
            clip,
            fade: crate::render::pack_fade(fade, clip, clip),
        });
    }
}
