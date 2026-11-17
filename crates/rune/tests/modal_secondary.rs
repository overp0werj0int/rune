//! A secondary press (a right-click) outside an open modal is a press outside it: it closes a
//! modal that closes on outside presses, as a primary one does, and the menu of what is behind
//! never opens. Inside the modal it opens the menu it lands on.
mod support;

use rune::{Cx, El, Env, Runtime, el};
use support::tick;

#[derive(Default)]
struct S {
    open: bool,
    outside: bool,
    log: Vec<&'static str>,
}

fn view(s: &S, _: &mut Cx<S>) -> El {
    let page = el().key("page").absolute(0.0, 0.0).size(200.0, 200.0).on_secondary_press(|s: &mut S, _, _| s.log.push("page menu"));
    let mut root = el().key("root").size(200.0, 200.0).child(page);
    if s.open {
        let card = el().key("card").absolute(50.0, 50.0).size(100.0, 100.0)
            .modal(s.outside, |s: &mut S| {
                s.open = false;
                s.log.push("dismissed");
            })
            .child(el().key("row").size(100.0, 30.0).on_secondary_press(|s: &mut S, _, _| s.log.push("row menu")));
        root = root.child(card);
    }
    root
}

fn opened(outside: bool) -> Runtime<S> {
    let mut rt = Runtime::new(S { open: true, outside, log: Vec::new() }, view, (200.0, 200.0), Env::default());
    rt.frame(0.0);
    tick(&mut rt);
    rt
}

fn right_click(rt: &mut Runtime<S>, x: f32, y: f32) {
    rt.pointer_move(x, y);
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    tick(rt);
}

#[test]
fn a_right_click_outside_closes_the_modal_and_opens_nothing_behind() {
    let mut rt = opened(true);
    right_click(&mut rt, 10.0, 10.0);
    assert!(!rt.state.open, "the modal closed");
    assert_eq!(rt.state.log, ["dismissed"], "the page behind opened no menu");
}

#[test]
fn a_modal_kept_open_by_outside_presses_stays_on_a_right_click() {
    let mut rt = opened(false);
    right_click(&mut rt, 10.0, 10.0);
    assert!(rt.state.open, "a modal that ignores outside presses stays");
    assert!(rt.state.log.is_empty(), "nothing behind it opened a menu: {:?}", rt.state.log);
}

#[test]
fn a_right_click_inside_opens_the_menu_it_lands_on() {
    let mut rt = opened(true);
    right_click(&mut rt, 60.0, 60.0);
    assert!(rt.state.open, "the modal stays");
    assert_eq!(rt.state.log, ["row menu"]);
}
