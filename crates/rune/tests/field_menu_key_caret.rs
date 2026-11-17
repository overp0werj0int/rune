//! The Menu key, or Shift+F10, in a focused text field opens the field's menu below the caret,
//! as a text view's keyboard menu does, not at the bottom-left of a tall field.
use rune::{Cx, El, Env, Key, KeyEvent, Mods, PointerPress, Runtime, TextEdit, TextStyle, col, text_input};

struct S {
    edit: TextEdit,
    at: Option<(f32, f32)>,
}

fn view(s: &S, _: &mut Cx<S>) -> El {
    col().w_full().h_full().child(
        text_input(&s.edit, TextStyle::new(14.0, 400.0).lh(20.0))
            .key("field")
            .w(300.0)
            .h(400.0)
            .on_edit(|s: &mut S| &mut s.edit)
            .on_secondary_press(|s: &mut S, at: PointerPress, _| s.at = Some((at.x, at.y))),
    )
}

#[test]
fn the_menu_key_in_a_field_opens_below_the_caret() {
    for (key, shift) in [(Key::ContextMenu, false), (Key::Function(10), true)] {
        let mut edit = TextEdit::new("one\ntwo\nthree");
        edit.focused = true;
        edit.set_selection(5, 5);
        let mut rt = Runtime::new(S { edit, at: None }, view, (400.0, 500.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt.frame(1.0 / 60.0);
        let caret = rt.elements().into_iter().find_map(|e| e.caret).expect("a caret");
        rt.key(KeyEvent { key: key.clone(), mods: Mods { shift, ..Mods::default() } });
        rt.frame(1.0 / 60.0);
        let (x, y) = rt.state.at.expect("the menu opened");
        assert!((x - caret[0]).abs() < 0.5 && (y - (caret[1] + caret[3])).abs() < 0.5, "{key:?}: at ({x}, {y}), caret {caret:?}");
        assert!(y < 100.0, "{key:?}: below the second line, not the field's foot");
    }
}
