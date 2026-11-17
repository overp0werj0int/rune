//! A field laid out by paragraph paints each `TextHighlight` once, on its own words: not on
//! the same bytes counted from the start of every other paragraph.
use rune::render::{KIND_GLYPH, KIND_RECT};
use rune::{Color, Cx, El, Env, Runtime, TextEdit, TextHighlight, TextStyle, text_input};

const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

/// "lift" in the second paragraph, marked red with blue letters. Its bytes, counted from the
/// third paragraph's start, fall inside that paragraph's words.
const TEXT: &str = "one\nlift\nthe third paragraph runs long\n";

fn view(s: &TextEdit, _: &mut Cx<TextEdit>) -> El {
    let mark = TextHighlight::new(4..8, RED).color(BLUE);
    text_input(s, TextStyle::new(14.0, 400.0).lh(20.0)).key("field").w(300.0).dir_by_paragraph().text_highlights([mark]).on_edit(|s: &mut TextEdit| s)
}

fn painted(rt: &Runtime<TextEdit>, c: Color, kind: f32) -> Vec<[f32; 4]> {
    rt.instances.iter().filter(|q| q.params[1] == kind && (q.fill[0] - c.r).abs() < 0.01 && (q.fill[1] - c.g).abs() < 0.01 && (q.fill[2] - c.b).abs() < 0.01 && q.fill[3] > 0.99).map(|q| q.rect).collect()
}

#[test]
fn a_highlight_in_a_by_paragraph_field_paints_once_on_its_words() {
    let mut rt = Runtime::new(TextEdit::new(TEXT), view, (300.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let field = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("field")).expect("the field").rect;
    let marks = painted(&rt, RED, KIND_RECT);
    assert_eq!(marks.len(), 1, "one mark, not one per paragraph: {marks:?}");
    let (top, bottom) = (marks[0][1], marks[0][1] + marks[0][3]);
    let mid = (top + bottom) / 2.0 - field[1];
    assert!((20.0..40.0).contains(&mid), "on the second line: {marks:?} in {field:?}");
    // Its letters take its colour, and no others do.
    let letters = painted(&rt, BLUE, KIND_GLYPH);
    assert!((3..=4).contains(&letters.len()), "the letters of lift, or its fi ligature: {letters:?}");
    assert!(letters.iter().all(|r| r[1] + r[3] > top && r[1] < bottom), "all on its line: {letters:?}");
}
