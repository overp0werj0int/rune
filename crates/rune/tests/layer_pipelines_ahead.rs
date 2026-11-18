//! The first frame that fades a group finds the isolated layers' pipelines made: a renderer makes
//! them on a thread of its own from when it is made, so a fade's first frame (a note's text coming
//! in from 40%) doesn't wait tens of milliseconds for them. The group still composites as one.
use std::time::{Duration, Instant};

use rune::render::{Gpu, OFFSCREEN_FORMAT, Offscreen, Renderer};
use rune::text::TextSystem;
use rune::{Color, Cx, El, Env, Runtime, el};

const SIZE: u32 = 60;
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

/// A white page, and with `faded` a group at half opacity of a red square under a blue one.
fn view(faded: &bool, _: &mut Cx<bool>) -> El {
    let group = el().size(40.0, 40.0).opacity(0.5)
        .child(el().pin(Some(0.0), Some(0.0), None, None).size(30.0, 30.0).fill(RED))
        .child(el().pin(Some(10.0), Some(10.0), None, None).size(30.0, 30.0).fill(BLUE));
    el().size(SIZE as f32, SIZE as f32).fill(Color::WHITE).children(faded.then_some(group))
}

fn runtime() -> Runtime<bool> {
    let text = TextSystem::new(&[], ("Inter", "Inter"));
    let mut rt = Runtime::with_text(false, view, (SIZE as f32, SIZE as f32), Env::new(false, 1.0), text);
    rt.frame(0.0);
    rt
}

/// Prepares and draws the runtime's frame; how long the prepare took, and the frame's pixels.
fn draw(gpu: &Gpu, renderer: &mut Renderer, rt: &mut Runtime<bool>) -> (f64, Vec<u8>) {
    let target = Offscreen::new(&gpu.device, SIZE, SIZE);
    let t = Instant::now();
    renderer.prepare(&gpu.device, &gpu.queue, &mut rt.text.atlas, &rt.instances, (SIZE, SIZE));
    let prepared = t.elapsed().as_secs_f64() * 1000.0;
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer.draw(&mut encoder, &target.view, rt.clear_color());
    target.copy(&mut encoder);
    renderer.submit(&gpu.queue, encoder);
    (prepared, target.read(&gpu.device))
}

fn fade(rt: &mut Runtime<bool>, faded: bool) {
    rt.update(move |s, _| *s = faded);
    rt.frame(1.0);
}

fn pixel(px: &[u8], x: u32, y: u32) -> [u8; 4] {
    let at = ((y * SIZE + x) * 4) as usize;
    [px[at], px[at + 1], px[at + 2], px[at + 3]]
}

fn near(got: [u8; 4], want: [u8; 4]) -> bool {
    got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2)
}

#[test]
fn the_first_faded_frame_composites_the_group_as_one() {
    let gpu = Gpu::headless();
    let mut renderer = Renderer::new(&gpu.device, OFFSCREEN_FORMAT);
    let mut rt = runtime();
    draw(&gpu, &mut renderer, &mut rt);
    fade(&mut rt, true);
    let (_, px) = draw(&gpu, &mut renderer, &mut rt);
    // Half blue over white where the squares overlap, as one layer composites; drawn square by
    // square, the red would show through the blue (188, 137, 225).
    assert!(near(pixel(&px, 25, 25), [188, 188, 255, 255]), "the overlap reads {:?}", pixel(&px, 25, 25));
    assert!(near(pixel(&px, 5, 5), [255, 188, 188, 255]), "half red over white reads {:?}", pixel(&px, 5, 5));
}

#[test]
fn the_first_faded_frame_waits_for_no_pipeline() {
    let gpu = Gpu::headless();
    let mut renderer = Renderer::new(&gpu.device, OFFSCREEN_FORMAT);
    let mut rt = runtime();
    draw(&gpu, &mut renderer, &mut rt);
    // A window shows for a while before anything in it fades.
    std::thread::sleep(Duration::from_secs(1));
    fade(&mut rt, true);
    let (first, _) = draw(&gpu, &mut renderer, &mut rt);
    fade(&mut rt, false);
    draw(&gpu, &mut renderer, &mut rt);
    fade(&mut rt, true);
    let (again, _) = draw(&gpu, &mut renderer, &mut rt);
    assert!(first < again + 15.0, "the first faded frame took {first:.1} ms to prepare, a later one {again:.1} ms");
}
