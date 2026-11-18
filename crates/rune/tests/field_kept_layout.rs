//! A long field shown again after another screen takes back its paragraphs' layouts: a note's
//! editor back from its preview does not shape every line again. Of the fields past the stale
//! layouts' budget only the one used last keeps them, so what stays shaped is one field's text.
use std::borrow::Cow;

use rune::{Cx, El, Env, GrowthSpec, Runtime, ScrollSpec, TextEdit, TextStyle, col, el, text, text_input};

#[derive(Clone, Copy, PartialEq)]
enum Shown {
    /// One of the long fields, as a note's editor.
    Field(usize),
    /// A virtual list of rows, as a note's preview, with a short field in it or not.
    Rows { short: bool },
}

struct Screen {
    fields: [TextEdit; 2],
    short: TextEdit,
    shown: Shown,
    n: u32,
}

fn field(edit: &TextEdit) -> El {
    text_input(edit, TextStyle::new(14.0, 400.0).lh(20.0)).w_full().dir_by_paragraph()
}

/// One scroll, as a note's page.
fn view(s: &Screen, _: &mut Cx<Screen>) -> El {
    let page = match s.shown {
        Shown::Field(i) => el().key("source").w_full().child(field(&s.fields[i]).key("field").on_edit(move |s: &mut Screen| &mut s.fields[i])),
        Shown::Rows { short } => {
            let rows = col().key("rows").w_full().growth(GrowthSpec::default()).virtual_rows(40, 20.0, |_: &Screen, i| Cow::Owned(format!("row-{i}")), |s: &Screen, i| text(format!("Preview {} row {i}", s.n)));
            let page = col().key("preview").w_full().child(rows);
            if short { page.child(field(&s.short).key("short").on_edit(|s: &mut Screen| &mut s.short)) } else { page }
        }
    };
    col().key("doc").size(640.0, 400.0).scroll(ScrollSpec { stick_bottom: false }).child(page)
}

/// 1,500 lines, more than the stale layouts' budget holds; not plain ASCII, so every line
/// shapes at once rather than waiting.
fn long_text(name: &str) -> String {
    (0..1500).map(|i| format!("{name} {i}: café, naïve, résumé and more words\n")).collect()
}

/// Shows `shown`, then a second of commits on it; what its first frame shaped.
fn show(rt: &mut Runtime<Screen>, shown: Shown) -> u32 {
    rt.update(|s, _| s.shown = shown);
    rt.frame(1.0 / 60.0);
    let shaped = rt.stats.shaped;
    // What left has drawn its exit, and what is unused has gone stale.
    for _ in 0..6 {
        rt.update(|s, _| s.n += 1);
        rt.frame(1.0 / 6.0);
    }
    shaped
}

fn screen() -> Runtime<Screen> {
    let fields = [TextEdit::new(&long_text("Note")), TextEdit::new(&long_text("Other"))];
    let short = TextEdit::new("A short field\nof two lines, é");
    let mut rt = Runtime::new(Screen { fields, short, shown: Shown::Field(0), n: 0 }, view, (640.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(rt.stats.shaped >= 1500, "the first layout shapes every line: {}", rt.stats.shaped);
    rt
}

const ROWS: Shown = Shown::Rows { short: false };

#[test]
fn a_long_field_shown_again_keeps_its_layout() {
    let mut rt = screen();
    show(&mut rt, ROWS);
    let shaped = show(&mut rt, Shown::Field(0));
    assert!(shaped < 10, "shown again, the field shaped {shaped} paragraphs");
}

#[test]
fn only_the_long_field_shown_last_keeps_its_layout() {
    let mut rt = screen();
    show(&mut rt, ROWS);
    assert!(show(&mut rt, Shown::Field(1)) >= 1500, "another field shapes its own text");
    show(&mut rt, ROWS);
    assert!(show(&mut rt, Shown::Field(1)) < 10, "the field shown last kept its layout");
    show(&mut rt, ROWS);
    assert!(show(&mut rt, Shown::Field(0)) >= 1500, "the one before it gave its layout up");
}

#[test]
fn a_short_field_gone_after_it_leaves_the_long_one_its_layout() {
    let mut rt = screen();
    show(&mut rt, Shown::Rows { short: true });
    show(&mut rt, ROWS);
    let shaped = show(&mut rt, Shown::Field(0));
    assert!(shaped < 10, "shown again after a short field went, the field shaped {shaped} paragraphs");
}
