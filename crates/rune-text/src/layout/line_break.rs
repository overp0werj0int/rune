//! Line breaking with paragraph indents, line caps and pills kept with their labels.

use crate::*;

/// First-line `text_indent` in whole device pixels (finite and clamped to keep layout arithmetic
/// finite even for huge inputs).
pub(crate) fn indent_px(text_indent: f32, scale: f32) -> f32 {
    if text_indent == 0.0 || !text_indent.is_finite() {
        0.0
    } else {
        (text_indent * scale).clamp(-1e7, 1e7).round()
    }
}

/// Breaks a shaped `layout` at `max_width` again and aligns its lines, as
/// [`TextSystem::layout_keyed`] breaks the layouts it shapes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebreak(layout: &mut Layout<Ink>, blocks: &[crate::Block], text_indent: f32, inset: Inset, font_size: f32, scale: f32, max_width: Option<f32>, align: Alignment) {
    rebreak_at_y(layout, blocks, text_indent, inset, font_size, scale, max_width, align, 0.0);
}

/// [`rebreak`] starting at fractional vertical pixel phase `y_frac` (`y - y.floor()`), so a
/// wrapped paragraph in a [`ParagraphLayout`](super::ParagraphLayout) quantizes each sub-line's
/// `y` identically to [`Layout`](parley::Layout)'s accumulated `line_y`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebreak_at_y(layout: &mut Layout<Ink>, blocks: &[crate::Block], text_indent: f32, inset: Inset, font_size: f32, scale: f32, max_width: Option<f32>, align: Alignment, y_frac: f64) {
    let indent = indent_px(text_indent, scale);
    layout.set_text_indent(indent, parley::IndentOptions::default());
    if blocks.iter().any(|&b| b != crate::Block::Paragraph) || layout.inline_boxes().next().is_some() || !inset.is_flush() {
        break_lines_with_blocks_at(layout, blocks, indent, inset, font_size, scale, max_width, Vec::new(), y_frac);
    } else if y_frac == 0.0 {
        layout.break_all_lines(max_width);
    } else {
        let mut breaker = layout.break_lines();
        breaker.state_mut().set_line_y(y_frac);
        breaker.break_remaining(max_width.unwrap_or(f32::MAX));
    }
    layout.align(align, parley::AlignmentOptions::default());
}

/// Breaks a layout [`TextSystem::layout_keyed`] shaped from `text`: by blocks, pills and
/// `inset` when `indented` (it has blocks or pills) or `inset` isn't flush, else at `max_width`;
/// then, when `pretty`, a plain one's paragraphs that end on a lone short word again, as
/// [`super::pretty`] scores them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn break_new(layout: &mut Layout<Ink>, text: &str, blocks: &[crate::Block], text_indent: f32, inset: Inset, indented: bool, font_size: f32, scale: f32, max_width: Option<f32>, pretty: bool) {
    let indent = indent_px(text_indent, scale);
    layout.set_text_indent(indent, parley::IndentOptions::default());
    if indented || !inset.is_flush() {
        break_lines_with_blocks(layout, blocks, indent, inset, font_size, scale, max_width, Vec::new());
        // Pills (a link's icon, a mention's chip) wrap prettily too, as their inline boxes do in
        // Chromium; indented blocks (lists, quotes, code) and paragraph insets keep the greedy lines.
        if let (true, Some(width)) = (pretty && inset.is_flush() && blocks.iter().all(|&b| b == crate::Block::Paragraph), max_width) {
            let caps = super::pretty::caps(layout, text, font_size, scale, width);
            if !caps.is_empty() {
                break_lines_with_blocks(layout, blocks, indent, inset, font_size, scale, max_width, caps);
            }
        }
        return;
    }
    layout.break_all_lines(max_width);
    if let (true, Some(width)) = (pretty, max_width) {
        let caps = super::pretty::caps(layout, text, font_size, scale, width);
        if !caps.is_empty() {
            break_lines_capped(layout, &[], indent, Starts::default(), font_size, scale, max_width, &caps, &mut Vec::new(), 0.0);
        }
    }
}

/// Breaks `layout` into lines, each paragraph indented by its block and by `inset`, whose hung
/// bytes start its first line their own width early. A pill's chip that would end a line apart
/// from its label, or run past the line's end, moves to the next line. `caps` start as the room
/// given lines (pretty wrapping's); a pill's adjustments come after them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn break_lines_with_blocks(
    layout: &mut Layout<Ink>,
    blocks: &[crate::Block],
    first_indent: f32,
    inset: Inset,
    font_size: f32,
    scale: f32,
    max_width: Option<f32>,
    caps: Vec<((usize, usize), f32)>,
) {
    break_lines_with_blocks_at(layout, blocks, first_indent, inset, font_size, scale, max_width, caps, 0.0);
}

#[allow(clippy::too_many_arguments)]
fn break_lines_with_blocks_at(
    layout: &mut Layout<Ink>,
    blocks: &[crate::Block],
    first_indent: f32,
    inset: Inset,
    font_size: f32,
    scale: f32,
    max_width: Option<f32>,
    caps: Vec<((usize, usize), f32)>,
    y_frac: f64,
) {
    let mut starts = Starts { first: inset.indent + inset.first, rest: inset.indent };
    if inset.hang > 0 {
        // The hung bytes' width is theirs once they're on a line: break with them in the
        // text's column, measure them there, and break again with them before it. The lead
        // stays in the column and moves the other lines in by its width.
        break_lines_at(layout, blocks, first_indent, Starts { first: inset.indent, rest: inset.indent }, font_size, scale, max_width, caps.clone(), y_frac);
        let lead = advance(layout, 0, inset.lead);
        starts.first -= advance(layout, inset.lead, inset.hang);
        starts.rest += lead;
    }
    starts.first = starts.first.max(0.0);
    break_lines_at(layout, blocks, first_indent, starts, font_size, scale, max_width, caps, y_frac);
}

/// Where a paragraph's first line and its other lines start, in device px from its start edge,
/// on top of its block's indent.
#[derive(Clone, Copy, Default)]
struct Starts {
    first: f32,
    rest: f32,
}

/// The advance of the first line's clusters within bytes `from..to`.
fn advance(layout: &Layout<Ink>, from: usize, to: usize) -> f32 {
    let Some(line) = layout.get(0) else { return 0.0 };
    line.runs().flat_map(|run| run.clusters()).filter(|c| from <= c.text_range().start && c.text_range().end <= to).map(|c| c.advance()).sum()
}

/// Breaks `layout` into lines, each paragraph indented by its block and `starts`. A pill's chip
/// that would end a line apart from its label, or run past the line's end, moves to the next line.
#[allow(clippy::too_many_arguments)]
fn break_lines_at(
    layout: &mut Layout<Ink>,
    blocks: &[crate::Block],
    first_indent: f32,
    starts: Starts,
    font_size: f32,
    scale: f32,
    max_width: Option<f32>,
    mut caps: Vec<((usize, usize), f32)>,
    y_frac: f64,
) {
    let pills = layout.inline_boxes().filter(|b| b.id & PILL_BOX != 0).count() / 2;
    let span_boxes = layout.inline_boxes().filter(|b| b.id & SPAN_BOX != 0).count();
    // `(para_idx, line_in_para)` and the room it gets instead: less to break before a chip, or
    // more to keep a chip with its label. Paragraphs break independently at `\n`, so one pass
    // can adjust the first misplaced pill or span-box spacer in every paragraph at once.
    let mut lines_meta = Vec::new();
    for _ in 0..=2 * (pills + span_boxes) {
        break_lines_capped(layout, blocks, first_indent, starts, font_size, scale, max_width, &caps, &mut lines_meta, y_frac);
        if max_width.is_none() || (pills == 0 && span_boxes == 0) || !collect_misplaced_pills(layout, &lines_meta, &mut caps) {
            return;
        }
    }
}

/// [`break_lines_with_blocks`] once, with `caps` setting the content room on given
/// `(para_idx, line_in_para)` lines (the last for a line wins). Fills `out` with
/// `((para_idx, line_in_para), uncapped_content_room)`.
#[allow(clippy::too_many_arguments)]
fn break_lines_capped(
    layout: &mut Layout<Ink>,
    blocks: &[crate::Block],
    first_indent: f32,
    starts: Starts,
    font_size: f32,
    scale: f32,
    max_width: Option<f32>,
    caps: &[((usize, usize), f32)],
    out: &mut Vec<((usize, usize), f32)>,
    y_frac: f64,
) {
    out.clear();
    let layout_max = max_width.unwrap_or(f32::MAX);
    let rtl = layout.is_rtl();
    let mut breaker = layout.break_lines();
    if y_frac != 0.0 {
        breaker.state_mut().set_line_y(y_frac);
    }
    breaker.state_mut().set_layout_max_advance(layout_max);
    let (mut para_idx, mut line_in_para) = (0usize, 0usize);
    while !breaker.is_done() {
        let blk = blocks.get(para_idx).copied().unwrap_or(crate::Block::Paragraph);
        let [indent, right_pad] = blk.pad_px(font_size, scale);
        // A paragraph's own indent is from its start edge: a right-to-left line, which ends
        // at the left, gives up the room at its right instead.
        let own = if line_in_para == 0 { starts.first } else { starts.rest };
        let line_max = max_width
            .map_or(f32::MAX, |w| (w - indent - own - right_pad).max(1.0))
            .min(layout_max);
        let indent = if rtl { indent } else { indent + own };
        let key = (para_idx, line_in_para);
        let text_indent = if key == (0, 0) { first_indent } else { 0.0 };
        let content_room = max_width.map_or(f32::MAX, |_| (line_max - text_indent).max(1.0));
        out.push((key, content_room));
        // Caps express content advance; Parley subtracts `text_indent` from `line_max_advance`.
        let cap = caps.iter().rfind(|c| c.0 == key).map(|c| c.1);
        let capped = cap.map_or(line_max, |c| c + text_indent);
        breaker.state_mut().set_line_x(indent);
        // Parley wants no line wider than the layout: a widened line widens it while it breaks.
        breaker.state_mut().set_layout_max_advance(layout_max.max(capped));
        breaker.state_mut().set_line_max_advance(capped);
        match breaker.break_next() {
            Some(parley::YieldData::LineBreak(data)) => {
                // Keep the line's alignment box at `line_max` when a cap tightened/widened breaking.
                if cap.is_some() && max_width.is_some() {
                    breaker.set_prior_line_width(line_max);
                }
                if data.reason == parley::BreakReason::Explicit {
                    para_idx += 1;
                    line_in_para = 0;
                } else {
                    line_in_para += 1;
                }
            }
            Some(_) => {}
            None => break,
        }
    }
    breaker.state_mut().set_layout_max_advance(layout_max);
}

/// Appends the first misplaced pill per paragraph to `caps`, returning whether any was added.
fn collect_misplaced_pills(
    layout: &Layout<Ink>,
    lines_meta: &[((usize, usize), f32)],
    caps: &mut Vec<((usize, usize), f32)>,
) -> bool {
    let before = caps.len();
    let mut skip_para = None;
    let mut boxes: Vec<parley::PositionedInlineBox> = Vec::new();
    for (li, line) in layout.lines().enumerate() {
        let Some(&(key, room)) = lines_meta.get(li) else { break };
        if skip_para == Some(key.0) {
            continue;
        }
        boxes.clear();
        boxes.extend(line.items().filter_map(|it| match it {
            PositionedLayoutItem::InlineBox(b) if b.id & PILL_BOX != 0 => Some(b),
            _ => None,
        }));
        let m = line.metrics();
        // Device px from the start of the line's content, as the breaker counts.
        let at = |x: f32| x - m.inline_min_coord - m.offset;
        let mut adjusted = false;
        for (k, lead) in boxes.iter().enumerate().filter(|(_, b)| b.id & 1 == 0) {
            let trail = boxes.iter().find(|b| b.id == lead.id | 1);
            if trail.is_some_and(|t| at(t.x) + t.width <= room + 0.5) {
                continue;
            }
            // A chip starting a line whose trail alone went on to the next: widen this line to
            // hold the whole pill. A label that itself went on is wider than a line: it wraps.
            let fix = if trail.is_none() && at(lead.x) < 0.5 {
                layout.get(li + 1).and_then(|next| {
                    let nm = next.metrics();
                    let t = next.items().find_map(|it| match it {
                        PositionedLayoutItem::InlineBox(b) if b.id == lead.id | 1 => Some(b),
                        _ => None,
                    })?;
                    let x = t.x - nm.inline_min_coord - nm.offset;
                    (x < 0.5).then(|| m.advance + x + t.width + 1.0)
                })
            } else {
                let mut first = k;
                while first >= 2
                    && boxes[first - 1].id & 1 == 1
                    && (boxes[first - 1].x + boxes[first - 1].width - boxes[first].x).abs() < 0.5
                    && boxes[first - 2].id == boxes[first - 1].id & !1
                {
                    first -= 2;
                }
                let x = at(boxes[first].x);
                (x >= 0.5).then(|| x + boxes[first].width * 0.5)
            };
            if let Some(w) = fix {
                let cap = (key, w);
                if !caps.contains(&cap) {
                    caps.push(cap);
                    skip_para = Some(key.0);
                    adjusted = true;
                    break;
                }
            }
        }
        if adjusted {
            continue;
        }
        // Keep a padded `SpanBox`'s leading spacer with its first word and its trailing spacer
        // with its last word while still allowing multi-word span boxes to wrap inside.
        let lr = line.text_range();
        let orphaned_lead = line.items().find_map(|it| match it {
            PositionedLayoutItem::InlineBox(b) if b.id & SPAN_BOX != 0 && b.id & 1 == 0 => {
                let ib = layout.inline_boxes().find(|ib| ib.id == b.id)?;
                (ib.index >= lr.end && at(b.x) >= 0.5).then(|| at(b.x) + b.width * 0.5)
            }
            _ => None,
        });
        if let Some(w) = orphaned_lead {
            let cap = (key, w);
            if !caps.contains(&cap) {
                caps.push(cap);
                skip_para = Some(key.0);
                continue;
            }
        }
        if let Some(next) = layout.get(li + 1) {
            let nm = next.metrics();
            let nr = next.text_range();
            let orphaned_trail = next.items().find_map(|it| match it {
                PositionedLayoutItem::InlineBox(b) if b.id & SPAN_BOX != 0 && b.id & 1 == 1 => {
                    let ib = layout.inline_boxes().find(|ib| ib.id == b.id)?;
                    let x = b.x - nm.inline_min_coord - nm.offset;
                    (x < 0.5 && ib.index <= nr.start).then_some((x, b.width))
                }
                _ => None,
            });
            if let Some((x, tw)) = orphaned_trail {
                let shrink = (m.advance - 1.0).max(1.0);
                let cap = if m.advance > 1.5 && !caps.contains(&(key, shrink)) {
                    (key, shrink)
                } else {
                    (key, m.advance + x + tw + 1.0)
                };
                if !caps.contains(&cap) {
                    caps.push(cap);
                    skip_para = Some(key.0);
                }
            }
        }
    }
    caps.len() > before
}

#[cfg(test)]
#[path = "text_indent_tests.rs"]
mod tests;
