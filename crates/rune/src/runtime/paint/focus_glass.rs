//! The keyboard's focus ring drawn as a glass card (`Cx::focus_glass`): a shadow around it,
//! under the element, then over the element's fill a faint sheen that fades down, a soft glow
//! inside its edge and a rim lit from above, all in the ring's colour.
use super::*;

/// How long the card takes to slide to the next focused element, in seconds.
const SLIDE: f64 = 0.18;

/// The glass card as the keyboard moves it (`Cx::focus_glass`): where it was drawn last frame,
/// and, while it slides to a newly focused element, where it set out from and when.
#[derive(Default)]
pub(crate) struct Slide {
    last: Option<Drawn>,
    from: Option<([f32; 4], [f32; 4], f64)>,
    /// A card was drawn this frame.
    seen: bool,
}

#[derive(Clone, Copy)]
struct Drawn {
    id: NodeId,
    rect: [f32; 4],
    radii: [f32; 4],
    clip: [f32; 4],
}

impl Slide {
    /// Whether the card is on its way: a frame that draws it must paint again.
    pub fn moving(&self) -> bool {
        self.from.is_some()
    }

    /// Before a frame's paint.
    pub fn begin(&mut self) {
        self.seen = false;
    }

    /// After it. Once the keyboard lets go of focus, the next card appears where its element
    /// is rather than sliding in. A frame that drew none while the keyboard still holds focus
    /// keeps the last: a cached layer replays its card without painting it again.
    pub fn end(&mut self, keyboard: bool) {
        if !self.seen && !keyboard {
            self.last = None;
            self.from = None;
        }
    }

    /// Element `id`'s card at time `now`: when focus has come to it from another element under
    /// the same clip, it slides there from where the card was, easing out; anywhere else it
    /// jumps. Returns whether it is still moving.
    pub fn place(&mut self, gl: &mut Glass, id: NodeId, clip: [f32; 4], now: f64, reduced: bool) -> bool {
        self.seen = true;
        if let Some(d) = self.last.filter(|d| d.id != id) {
            self.from = (d.clip == clip && !reduced).then_some((d.rect, d.radii, now));
        }
        if let Some((rect, radii, t0)) = self.from {
            let u = ((now - t0) / SLIDE).clamp(0.0, 1.0) as f32;
            let e = 1.0 - (1.0 - u).powi(3);
            let mix = |a: [f32; 4], b: [f32; 4]| std::array::from_fn(|k| a[k] + (b[k] - a[k]) * e);
            gl.rect = mix(rect, gl.rect);
            gl.radii = mix(radii, gl.radii);
            if u >= 1.0 {
                self.from = None;
            }
        }
        self.last = Some(Drawn { id, rect: gl.rect, radii: gl.radii, clip });
        self.from.is_some()
    }
}

pub(in crate::runtime) struct Glass {
    /// Device pixels: the ring's outer edge.
    pub rect: [f32; 4],
    pub radii: [f32; 4],
    pub tint: Color,
    /// The rim's width in points.
    pub rim: f32,
}

impl Glass {
    /// The card's shadow: soft and a little below it, as wide as a small control can carry.
    pub fn shadow(&self, k_px: f32) -> crate::el::Shadow {
        let blur = (self.rect[2].min(self.rect[3]) / k_px * 0.5).clamp(4.0, 22.0);
        crate::el::Shadow { x: 0.0, y: blur * 0.27, blur, spread: 0.0, color: Color::BLACK.alpha(0.3), inset: false }
    }

    /// The sheen, the inner glow and the rim, over the element's fill and under its content.
    pub fn push(&self, out: &mut Vec<Instance>, (k_px, s, sc): (f32, f32, f32), o: f32, srgb: bool, clip: [f32; 4], fade: [f32; 4]) {
        let t = |a: f32| premul(self.tint.alpha(self.tint.a * a), o, srgb);
        let packed = crate::render::pack_fade(fade, clip, clip);
        let (rect, radii) = (self.rect, self.radii);
        out.push(Instance { rect, radii, fill: t(0.105), border: [0.0; 4], params: [0.0, KIND_GRADIENT, 0.0, 1.0], uv: t(0.035), clip, fade: packed });
        let size = rect[2].min(rect[3]) / k_px;
        let glow = crate::el::Shadow { x: 0.0, y: 0.0, blur: (size * 0.3).min(10.0), spread: 0.0, color: Color::TRANSPARENT, inset: true };
        out.push(border_paint::inset_shadow(&glow, t(0.08), (rect, radii), (k_px, s), clip, fade));
        let rim = pixel_snap::line_width(self.rim * s) * sc;
        out.push(Instance { rect, radii, fill: [0.0; 4], border: t(0.55), params: [rim, KIND_GRADIENT, 0.0, 1.0], uv: t(0.12), clip, fade: packed });
    }
}
