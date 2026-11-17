//! A highlight's letter colour lands on the glyphs under its mark, in a paragraph with an indent
//! too.
use rune::render::{KIND_GLYPH, KIND_RECT};
use rune::{Color, Cx, El, Env, ParagraphStyle, Runtime, TextEdit, TextHighlight, TextStyle, text_input};

const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

const TEXT: &str = "Some of this is nostalgia. Most of it isn't. A tool that responds";

fn view(s: &TextEdit, _: &mut Cx<TextEdit>) -> El {
    let at = TEXT.find("tool").unwrap();
    let mark = TextHighlight::new(at..at + 4, RED).color(BLUE);
    text_input(s, TextStyle::new(14.0, 400.0).lh(20.0)).key("field").w(760.0).text_highlights([mark]).on_edit(|s: &mut TextEdit| s)
        .paragraph_styles(vec![ParagraphStyle::new().indent(45.0)])
}

fn painted(rt: &Runtime<TextEdit>, c: Color, kind: f32) -> Vec<[f32; 4]> {
    rt.instances.iter().filter(|q| q.params[1] == kind && (q.fill[0] - c.r).abs() < 0.01 && (q.fill[1] - c.g).abs() < 0.01 && (q.fill[2] - c.b).abs() < 0.01 && q.fill[3] > 0.99).map(|q| q.rect).collect()
}

#[test]
fn a_highlights_letters_sit_under_its_mark() {
    let mut rt = Runtime::new(TextEdit::new(TEXT), view, (780.0, 100.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let marks = painted(&rt, RED, KIND_RECT);
    assert_eq!(marks.len(), 1, "{marks:?}");
    let (x0, x1) = (marks[0][0], marks[0][0] + marks[0][2]);
    let letters = painted(&rt, BLUE, KIND_GLYPH);
    assert_eq!(letters.len(), 4, "the four letters of tool: {letters:?} under {marks:?}");
    assert!(letters.iter().all(|r| r[0] >= x0 - 1.0 && r[0] + r[2] <= x1 + 1.0), "inside the mark {x0}..{x1}: {letters:?}");
}
