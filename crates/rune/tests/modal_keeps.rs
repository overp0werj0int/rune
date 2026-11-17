//! `El::modal_keeps`: a modal that leaves chosen elements outside it live to the pointer, as a
//! window's title bar stays live over a drawer, while the keyboard and the wheel stay contained.
mod support;

use rune::tok::TextStyle;
use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, ScrollSpec, TextEdit, col, el, text_input};
use support::{click, find as info, tick};

#[derive(Default)]
struct S {
    open: bool,
    field: TextEdit,
    log: Vec<&'static str>,
}

/// A bar with a button, a page that scrolls and holds a field, and over them a drawer that
/// keeps the bar live.
fn view(s: &S, _: &mut Cx<S>) -> El {
    let bar = el().key("bar").absolute(0.0, 0.0).size(200.0, 30.0)
        .child(el().key("bar-button").size(30.0, 30.0).focusable(true).on_press(|s: &mut S| {
            s.open = false;
            s.log.push("bar");
        }));
    let page = col().key("page").absolute(0.0, 30.0).size(200.0, 170.0).scroll(ScrollSpec { stick_bottom: false })
        .on_press(|s: &mut S| s.log.push("page"))
        .child(text_input(&s.field, TextStyle::new(14.0, 400.0)).key("field").w(120.0).on_edit(|s: &mut S| &mut s.field))
        .child(el().key("tall").size(200.0, 600.0));
    let mut root = el().key("root").size(200.0, 200.0).child(bar).child(page);
    if s.open {
        let drawer = col().key("drawer").absolute(100.0, 30.0).size(100.0, 170.0)
            .modal(true, |s: &mut S| s.open = false)
            .modal_keeps(["bar"])
            .child(el().key("a").size(40.0, 30.0).focusable(true).on_press(|s: &mut S| s.log.push("a")))
            .child(el().key("b").size(40.0, 30.0).focusable(true).on_press(|s: &mut S| s.log.push("b")));
        root = root.child(drawer);
    }
    root
}

fn opened() -> Runtime<S> {
    let mut rt = Runtime::new(S::default(), view, (200.0, 200.0), Env::default());
    rt.frame(0.0);
    rt.set_full_keyboard_access(true);
    rt.update(|s, _| s.field.focused = true);
    tick(&mut rt);
    assert!(info(&rt, "field").focused, "the field had the keyboard before the drawer opened");
    rt.update(|s, _| s.open = true);
    tick(&mut rt);
    rt
}

fn key(rt: &mut Runtime<S>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    tick(rt);
}

#[test]
fn a_kept_element_takes_presses_and_hover() {
    let mut rt = opened();
    rt.pointer_move(15.0, 15.0);
    tick(&mut rt);
    assert!(!info(&rt, "bar-button").inert, "the kept bar stays live");
    assert!(info(&rt, "field").inert, "the page behind does not");
    click(&mut rt, (15.0, 15.0));
    assert_eq!(rt.state.log, ["bar"], "the bar's button acts on one press");
    assert!(!rt.state.open, "and its handler closed the drawer");
}

#[test]
fn the_rest_of_the_window_stays_behind_the_modal() {
    let mut rt = opened();
    click(&mut rt, (50.0, 150.0));
    assert!(rt.state.log.is_empty(), "a press on the page reaches nothing: {:?}", rt.state.log);
    assert!(!rt.state.open, "it dismisses the drawer");

    let mut rt = opened();
    let before = info(&rt, "tall").rect[1];
    rt.wheel(50.0, 150.0, -200.0);
    tick(&mut rt);
    assert_eq!(info(&rt, "tall").rect[1], before, "a wheel over the page scrolls nothing");
}

#[test]
fn the_keyboard_stays_in_the_modal_and_comes_back() {
    let mut rt = opened();
    assert!(!info(&rt, "field").focused, "opening moved focus out of the field");
    for _ in 0..5 {
        key(&mut rt, Key::Tab);
        assert!(info(&rt, "a").focused || info(&rt, "b").focused, "Tab stays in the drawer, never the kept bar");
    }
    key(&mut rt, Key::Text("x".into()));
    assert_eq!(rt.state.field.text, "", "typing never reaches the field behind");
    key(&mut rt, Key::Escape);
    assert!(!rt.state.open, "Escape closes it");
    assert!(info(&rt, "field").focused, "and focus returns to the field");
}
