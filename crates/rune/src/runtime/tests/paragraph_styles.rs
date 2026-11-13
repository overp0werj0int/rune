use super::*;
use crate::text::TextFlow;
use crate::{Ime, Key, KeyEvent, Mods, ParagraphStyle, Span, TextEdit, TextStyle, text_input};
use parley::layout::Affinity;

const GUTTER: f32 = 40.0;
const LINE: f32 = 20.0;
const HEADING_LINE: f32 = 30.0;
const HEADING_ABOVE: f32 = 12.0;

/// How many leading bytes of a Markdown `line` are its mark, and whether it's a heading.
fn mark(line: &str) -> (usize, bool) {
    if line.starts_with("# ") { (2, true) } else if line.starts_with("- ") || line.starts_with("> ") { (2, false) } else { (0, false) }
}

/// A Markdown source field as an editor styles it from its text on every build: marks hang
/// in a gutter, a heading is bigger with its own line height and room above.
fn view(s: &TextEdit, _: &mut Cx<TextEdit>) -> El {
    let (mut spans, mut styles, mut at) = (Vec::new(), Vec::new(), 0u32);
    for line in s.text.split('\n') {
        let (hung, heading) = mark(line);
        let style = ParagraphStyle::new().indent(GUTTER).hang(hung);
        styles.push(if heading { style.line_height(HEADING_LINE).space_above(HEADING_ABOVE) } else { style });
        if heading {
            spans.push(Span { start: at, end: at + line.len() as u32, size: Some(21.0), weight: Some(600.0), ..Span::default() });
        }
        at += line.len() as u32 + 1;
    }
    text_input(s, TextStyle::new(14.0, 400.0).lh(LINE)).key("field").w(300.0).spans(spans).paragraph_styles(styles)
        .on_edit(|s: &mut TextEdit| s).on_submit(|s: &mut TextEdit, _| s.insert("\n"))
}

fn field(text: &str, caret: usize) -> Runtime<TextEdit> {
    let mut edit = TextEdit::new(text);
    edit.focused = true;
    edit.set_caret(caret);
    let mut rt = Runtime::new(edit, view, (300.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn flow(rt: &Runtime<TextEdit>) -> TextFlow<'_> {
    rt.nodes.iter().find(|n| n.key.as_deref() == Some("field")).and_then(|n| n.text_layout.as_ref()).expect("the field's layout").view()
}

/// Where the field's display byte `at` is drawn: its caret's left, top and bottom.
fn at(rt: &Runtime<TextEdit>, at: usize) -> [f32; 3] {
    let bb = flow(rt).caret(at, Affinity::Downstream, 0.0);
    [bb.x0 as f32, bb.y0 as f32, bb.y1 as f32]
}

/// The focused caret as the window draws it: `[x, y, w, h]`.
fn caret(rt: &Runtime<TextEdit>) -> [f32; 4] {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("field")).and_then(|e| e.caret).expect("a caret")
}

/// The field's box in the window: `[x, y, w, h]`.
fn rect(rt: &Runtime<TextEdit>) -> [f32; 4] {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("field")).expect("the field").rect
}

fn press(rt: &mut Runtime<TextEdit>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(1.0 / 120.0);
}

fn click(rt: &mut Runtime<TextEdit>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 120.0);
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.51
}

const LIST: &str = "- amber lantern orchard quiet harbor copper meadow signal drift pebble willow\nplain words";

#[test]
fn a_hung_mark_sits_before_the_indent_and_every_line_of_text_starts_on_it() {
    let rt = field(LIST, 0);
    let (mark_x, text_x) = (at(&rt, 0)[0], at(&rt, 2)[0]);
    assert!(near(text_x, GUTTER), "the text after the mark starts at the indent, not {text_x}");
    assert!(mark_x > 0.0 && mark_x < GUTTER - 4.0, "the mark hangs before it, at {mark_x}");
    assert!(near(caret(&rt)[0], mark_x), "the drawn caret stands before the mark");
    // The item wraps, and its second line starts under its text, not under its mark.
    let second = (3..LIST.find('\n').unwrap()).find(|&b| at(&rt, b)[1] > at(&rt, 2)[1]).expect("a wrapped line");
    assert!(near(at(&rt, second)[0], GUTTER), "a wrapped line starts at the indent, not {}", at(&rt, second)[0]);
    let plain = LIST.find('\n').unwrap() + 1;
    assert!(near(at(&rt, plain)[0], GUTTER), "a paragraph with no mark starts at the indent");
}

#[test]
fn the_caret_steps_through_a_hung_mark_and_home_goes_before_it() {
    let mut rt = field(LIST, 0);
    let xs: Vec<f32> = (0..3).map(|_| { let x = caret(&rt)[0]; press(&mut rt, Key::Right); x }).collect();
    assert_eq!(rt.state.focus, 3);
    assert!(xs[0] < xs[1] && xs[1] < xs[2] && near(xs[2], GUTTER), "each mark character is a stop: {xs:?}");
    for want in [2, 1, 0] {
        press(&mut rt, Key::Left);
        assert_eq!(rt.state.focus, want);
    }
    rt.update(|s, _| s.set_caret(9));
    rt.frame(0.0);
    press(&mut rt, Key::Home);
    assert_eq!(rt.state.focus, 0, "Home goes to the mark, the line's first character");
    assert!(near(caret(&rt)[0], xs[0]));
}

#[test]
fn up_and_down_cross_a_hung_mark_and_keep_their_column() {
    let mut rt = field(LIST, 0);
    let (mark_x, line0) = (caret(&rt)[0], caret(&rt)[1]);
    let second = (3..LIST.find('\n').unwrap()).find(|&b| at(&rt, b)[1] > at(&rt, 2)[1]).unwrap();
    let plain = LIST.find('\n').unwrap() + 1;
    // From before the mark, left of every other line's start: Down lands on each line's start.
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.focus, second);
    assert!(near(caret(&rt)[0], GUTTER) && caret(&rt)[1] > line0);
    while rt.state.focus < plain { press(&mut rt, Key::Down); }
    assert_eq!(rt.state.focus, plain, "into the next paragraph at its start");
    // Back up, the column kept: the caret returns to the mark it left.
    while caret(&rt)[1] > line0 { press(&mut rt, Key::Up); }
    assert_eq!(rt.state.focus, 0);
    assert!(near(caret(&rt)[0], mark_x));
}

#[test]
fn a_selection_covers_a_hung_mark() {
    let mut rt = field(LIST, 0);
    rt.update(|s, _| s.set_selection(0, 7));
    rt.frame(1.0 / 120.0);
    let (mark_x, end_x, top, bottom) = (at(&rt, 0)[0], at(&rt, 7)[0], at(&rt, 0)[1], at(&rt, 0)[2]);
    let boxes = flow(&rt).range_boxes(0, 7, true);
    assert_eq!(boxes.len(), 1);
    let bb = boxes[0].0;
    assert!(near(bb.x0 as f32, mark_x) && near(bb.x1 as f32, end_x), "one box from the mark's left to the selection's end: {bb:?}");
    // And it's drawn there: a filled rect over the mark and the words after it.
    let field = rect(&rt);
    let drawn = rt.instances.iter().any(|i| {
        near(i.rect[0], field[0] + mark_x) && near(i.rect[0] + i.rect[2], field[0] + end_x)
            && i.rect[1] >= field[1] + top - 0.51 && i.rect[1] + i.rect[3] <= field[1] + bottom + 0.51 && i.fill[3] > 0.0
    });
    assert!(drawn, "the selection is painted over the hung mark");
}

#[test]
fn a_press_in_the_margin_lands_on_the_mark() {
    let mut rt = field(LIST, 20);
    let field = rect(&rt);
    let (mark_x, mid) = (at(&rt, 0)[0], (at(&rt, 0)[1] + at(&rt, 0)[2]) * 0.5);
    click(&mut rt, field[0] + 2.0, field[1] + mid);
    assert_eq!(rt.state.focus, 0, "left of the mark is before it");
    let on_mark = at(&rt, 1)[0] + 0.5;
    click(&mut rt, field[0] + on_mark, field[1] + mid);
    assert_eq!(rt.state.focus, 1, "on the mark is at that character");
    click(&mut rt, field[0] + GUTTER + 0.5, field[1] + mid);
    assert_eq!(rt.state.focus, 2, "at the indent is after the mark");
    // The margin beside a wrapped line belongs to that line, not to the mark above it.
    let second = (3..LIST.find('\n').unwrap()).find(|&b| at(&rt, b)[1] > at(&rt, 2)[1]).unwrap();
    let second_mid = (at(&rt, second)[1] + at(&rt, second)[2]) * 0.5;
    click(&mut rt, field[0] + mark_x, field[1] + second_mid);
    assert_eq!(rt.state.focus, second);
}

#[test]
fn an_ime_preedit_beside_a_hung_mark_keeps_the_hang() {
    // After the mark: the preedit is the item's text, starting at the indent.
    let mut rt = field("- item", 2);
    let mark_x = at(&rt, 0)[0];
    rt.ime(Ime::Preedit("にほん".into(), None));
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.text, "- item");
    assert!(near(at(&rt, 0)[0], mark_x), "the mark stays where it hung");
    assert!(near(at(&rt, 2)[0], GUTTER), "the preedit starts at the indent");
    assert!(caret(&rt)[0] > GUTTER, "the caret follows the preedit into the text");
    let ime = rt.ime_area.expect("the IME's caret area");
    assert!(ime[0] >= rect(&rt)[0] + GUTTER - 0.51, "the IME's window opens at the text, not in the margin");
    rt.ime(Ime::Commit("にほん".into()));
    rt.frame(2.0 / 120.0);
    assert_eq!(rt.state.text, "- にほんitem");
    assert!(near(at(&rt, 0)[0], mark_x) && near(at(&rt, 2)[0], GUTTER));

    // Inside the mark: the preedit hangs with it, and the item's text stays on the indent.
    let mut rt = field("- item", 1);
    rt.ime(Ime::Preedit("あ".into(), None));
    rt.frame(1.0 / 120.0);
    let text = 2 + "あ".len();
    assert!(near(at(&rt, text)[0], GUTTER), "the text after the mark and its preedit starts at the indent, not {}", at(&rt, text)[0]);
    assert!(at(&rt, 0)[0] < mark_x, "the wider mark hangs further out");
}

#[test]
fn undo_after_typing_a_mark_takes_the_hang_with_the_text() {
    let mut rt = field("item\nnext", 0);
    assert!(near(at(&rt, 0)[0], GUTTER));
    let height = rect(&rt)[3];
    for ch in ["-", " "] {
        rt.ime(Ime::Commit(ch.into()));
        rt.frame(1.0 / 120.0);
    }
    assert_eq!(rt.state.text, "- item\nnext");
    assert!(at(&rt, 0)[0] < GUTTER - 4.0 && near(at(&rt, 2)[0], GUTTER), "typing the mark hangs it");
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Mods::default() } });
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.text, "item\nnext", "one undo takes both characters");
    assert!(near(at(&rt, 0)[0], GUTTER), "and the hang with them");
    assert!(near(caret(&rt)[0], GUTTER));
    assert_eq!(rect(&rt)[3], height);
    // Nothing else was recorded: a second undo has no layout step to take back.
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, ..Mods::default() } });
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.text, "item\nnext");
    assert!(near(at(&rt, 0)[0], GUTTER));
    // Redo brings the mark and its hang back together.
    rt.key(KeyEvent { key: Key::Command('z'), mods: Mods { cmd: true, shift: true, ..Mods::default() } });
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.text, "- item\nnext");
    assert!(near(at(&rt, 2)[0], GUTTER));
}

#[test]
fn a_heading_has_its_own_line_height_and_room_above() {
    let text = "body one\n# Title\nbody two";
    let mut rt = field(text, 0);
    let (heading, after) = (text.find('#').unwrap(), text.rfind('\n').unwrap() + 1);
    let field = rect(&rt);
    assert_eq!(field[3], LINE + HEADING_ABOVE + HEADING_LINE + LINE, "the field is as tall as its lines and the room above the heading");
    let [x, top, bottom] = at(&rt, heading);
    assert_eq!((top, bottom), (LINE + HEADING_ABOVE, LINE + HEADING_ABOVE + HEADING_LINE), "the heading's line is its own height, under its room");
    assert!(x < GUTTER - 4.0 && near(at(&rt, heading + 2)[0], GUTTER), "its mark hangs at its own size");
    assert_eq!(at(&rt, after)[1], LINE + HEADING_ABOVE + HEADING_LINE, "the next paragraph follows it");
    // The drawn caret is the heading line's height, and taller than a body line's.
    let body = caret(&rt);
    rt.update(|s, _| s.set_caret(heading + 3));
    rt.frame(1.0 / 120.0);
    let drawn = caret(&rt);
    assert!(drawn[3] > body[3] && drawn[3] <= HEADING_LINE + 0.01, "caret {drawn:?} against a body line's {body:?}");
    assert!(drawn[1] >= field[1] + top - 0.01 && drawn[1] + drawn[3] <= field[1] + bottom + 0.01, "the caret stays inside the heading's line");
    // Presses: the room above belongs to the heading, each line to itself.
    click(&mut rt, field[0] + 100.0, field[1] + LINE + HEADING_ABOVE * 0.5);
    assert!((heading..after).contains(&rt.state.focus), "the room above the heading is the heading's: {}", rt.state.focus);
    click(&mut rt, field[0] + 100.0, field[1] + LINE + HEADING_ABOVE + HEADING_LINE - 2.0);
    assert!((heading..after).contains(&rt.state.focus), "the heading's last pixel rows are its own: {}", rt.state.focus);
    click(&mut rt, field[0] + 100.0, field[1] + LINE + HEADING_ABOVE + HEADING_LINE + 2.0);
    assert!(rt.state.focus >= after, "the row under it is the next paragraph's: {}", rt.state.focus);
    click(&mut rt, field[0] + 100.0, field[1] + LINE - 2.0);
    assert!(rt.state.focus < heading, "the row above its room is the paragraph before's: {}", rt.state.focus);
}

#[test]
fn an_empty_paragraph_keeps_its_indent_and_an_edit_reshapes_only_its_own() {
    let mut rt = field("- one\n\n- three", 6);
    assert!(near(caret(&rt)[0], GUTTER), "an empty line's caret stands at the indent, not {}", caret(&rt)[0]);
    let kept = |rt: &Runtime<TextEdit>| match flow(rt) {
        TextFlow::Paragraphs(p) => [0, 2].map(|k| Rc::as_ptr(&p.paragraphs[k].layout)),
        TextFlow::Single(_) => panic!("a styled field lays out by paragraph"),
    };
    let before = kept(&rt);
    rt.ime(Ime::Commit("x".into()));
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.text, "- one\nx\n- three");
    assert_eq!(kept(&rt), before, "the paragraphs around the edit keep their layouts");
    assert_eq!(rt.stats.shaped, 1);
}

#[test]
fn a_right_to_left_paragraph_indents_from_its_right() {
    let rt = field("- שלום עולם", 0);
    let width = 300.0;
    let (mark_x, text_x) = (at(&rt, 0)[0], at(&rt, 2)[0]);
    assert!(near(text_x, width - GUTTER), "the text starts at the indent from the right edge, not {text_x}");
    assert!(mark_x > width - GUTTER + 4.0 && mark_x <= width, "the mark hangs right of it, at {mark_x}");
}

#[test]
fn a_first_line_indent_moves_only_the_first_line() {
    fn view(s: &TextEdit, _: &mut Cx<TextEdit>) -> El {
        text_input(s, TextStyle::new(14.0, 400.0).lh(LINE)).key("field").w(300.0)
            .paragraph_styles([ParagraphStyle::new().first_line_indent(18.0), ParagraphStyle::new().indent(24.0).first_line_indent(-60.0)])
            .on_edit(|s: &mut TextEdit| s)
    }
    let words = "amber lantern orchard quiet harbor copper meadow signal drift pebble willow";
    let mut rt = Runtime::new(TextEdit::new(format!("{words}\n{words}")), view, (300.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let second = (1..words.len()).find(|&b| at(&rt, b)[1] > at(&rt, 0)[1]).unwrap();
    assert!(near(at(&rt, 0)[0], 18.0) && near(at(&rt, second)[0], 0.0));
    let next = words.len() + 1;
    let wrapped = (next + 1..next + words.len()).find(|&b| at(&rt, b)[1] > at(&rt, next)[1]).unwrap();
    assert!(near(at(&rt, next)[0], 0.0), "a first line never starts before the paragraph's edge");
    assert!(near(at(&rt, wrapped)[0], 24.0));
}
