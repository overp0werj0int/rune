//! [`ParagraphStyle`]: one paragraph's indent, hanging start, line height and space above in a
//! text field laid out by paragraph (`El::paragraph_styles`).

/// How one paragraph of a multi-line text input is laid out: its indent, where its first line
/// starts, its line height and the space above it. Lengths are points.
///
/// It is layout, not text: the view passes it on every build, as it does spans, and it's no part
/// of `TextEdit` or its undo history.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParagraphStyle {
    indent: f32,
    first_line_indent: f32,
    hang: u32,
    lead: u32,
    line_height: f32,
    space_above: f32,
}

fn length(pt: f32) -> f32 {
    if pt.is_finite() { pt.max(0.0) } else { 0.0 }
}

impl ParagraphStyle {
    pub const fn new() -> ParagraphStyle {
        ParagraphStyle { indent: 0.0, first_line_indent: 0.0, hang: 0, lead: 0, line_height: 0.0, space_above: 0.0 }
    }

    /// Every line starts `pt` points in from the paragraph's start edge: the left of a
    /// left-to-right paragraph, the right of a right-to-left one.
    pub fn indent(self, pt: f32) -> ParagraphStyle {
        ParagraphStyle { indent: length(pt), ..self }
    }

    /// The first line starts `pt` points after the indent, or before it when negative. It never
    /// starts before the paragraph's start edge.
    pub fn first_line_indent(self, pt: f32) -> ParagraphStyle {
        ParagraphStyle { first_line_indent: if pt.is_finite() { pt } else { 0.0 }, ..self }
    }

    /// The paragraph's first `bytes` bytes hang before the indent, as a list marker does: the
    /// first line starts their width early, so the text after them starts where wrapped lines
    /// do. They stay text, which the caret, a selection and a press reach like any other. When
    /// they're wider than the room before the indent, the line starts at the paragraph's start
    /// edge and the text after them past the indent.
    pub fn hang(self, bytes: usize) -> ParagraphStyle {
        ParagraphStyle { hang: bytes.min(u32::MAX as usize) as u32, ..self }
    }

    /// The first `bytes` of the hung bytes don't hang: they stay in the text's column and
    /// indent every line of the paragraph by their width, as a nested list item's leading
    /// spaces do, while the rest of the hung bytes (its marker) hang before them.
    pub fn lead(self, bytes: usize) -> ParagraphStyle {
        ParagraphStyle { lead: bytes.min(u32::MAX as usize) as u32, ..self }
    }

    /// Every line of the paragraph is `pt` points tall, whatever sizes its spans set. 0 keeps
    /// the field's own line height.
    pub fn line_height(self, pt: f32) -> ParagraphStyle {
        ParagraphStyle { line_height: length(pt), ..self }
    }

    /// `pt` points of room above the paragraph's first line, the field's first paragraph too.
    pub fn space_above(self, pt: f32) -> ParagraphStyle {
        ParagraphStyle { space_above: length(pt), ..self }
    }

    /// [`Self::hang`] with `bytes` more, and [`Self::lead`] too when `at` is inside the lead: a
    /// field's IME preedit typed at byte `at` inside the hung bytes.
    #[doc(hidden)]
    pub fn hang_more(self, at: usize, bytes: usize) -> ParagraphStyle {
        let s = self.hang(self.hang as usize + bytes);
        if at < self.lead as usize { s.lead(self.lead as usize + bytes) } else { s }
    }

    #[doc(hidden)]
    pub fn hung(self) -> usize {
        self.hang as usize
    }

    pub(crate) fn led(self) -> usize {
        (self.lead as usize).min(self.hang as usize)
    }

    /// What a paragraph's shaping and line breaking read of it, at `scale`, its first `hang`
    /// shaped bytes hung, the first `lead` of them indenting every line.
    pub(crate) fn inset(self, scale: f32, hang: usize, lead: usize) -> Inset {
        Inset { indent: (self.indent * scale).round(), first: (self.first_line_indent * scale).round(), hang, lead, line_height: self.line_height }
    }

    /// The room above the paragraph, in device px.
    pub(crate) fn space_px(self, scale: f32) -> f32 {
        (self.space_above * scale).round()
    }

    /// What a paragraph's own layout depends on: all but the space above it.
    pub(crate) fn layout_bits(self) -> [u32; 5] {
        [self.indent.to_bits(), self.first_line_indent.to_bits(), self.hang, self.lead, self.line_height.to_bits()]
    }

    pub(crate) fn bits(self) -> [u32; 6] {
        let [a, b, c, d, e] = self.layout_bits();
        [a, b, c, d, e, self.space_above.to_bits()]
    }
}

/// A [`ParagraphStyle`] as a paragraph's layout takes it: indents in device px, the hung bytes
/// on its shaped text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Inset {
    /// Every line's start, from the paragraph's start edge.
    pub(crate) indent: f32,
    /// What the first line adds to `indent`.
    pub(crate) first: f32,
    /// The first line also starts the width of its first `hang` bytes early.
    pub(crate) hang: usize,
    /// The first `lead` of the hung bytes indent every line by their width instead.
    pub(crate) lead: usize,
    /// The line height in points, or 0 for the text style's.
    pub(crate) line_height: f32,
}

impl Inset {
    /// Whether lines break as they would without it.
    pub(crate) fn is_flush(self) -> bool {
        self.indent == 0.0 && self.first == 0.0 && self.hang == 0
    }

    pub(crate) fn bits(self) -> [u32; 5] {
        [self.indent.to_bits(), self.first.to_bits(), self.hang as u32, self.lead as u32, self.line_height.to_bits()]
    }
}
