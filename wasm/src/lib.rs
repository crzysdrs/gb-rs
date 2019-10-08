use gb::cart::Cart;
use gb::gb::{GBReason, GB};
use gb::peripherals::{AudioSpec, PeripheralData};
use std::f64;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, ImageData};

#[wasm_bindgen]
pub struct ClosureHandle(Closure<dyn FnMut()>);

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

// fn set_onended(sound: &web_sys::AudioBufferSourceNode, f: &Closure<dyn FnMut(web_sys::Event)>) {
//     sound.set_onended(Some(f.as_ref().unchecked_ref()));
// }

#[wasm_bindgen]
pub fn start() {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document.get_element_by_id("canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| ())
        .unwrap();

    let cart = Cart::new(
        include_bytes!("../Legend of Zelda, The - Link's Awakening DX (U) (V1.2) [C][!].gbc")
            .to_vec(),
    );

    let mut gb = GB::new(cart, None, false, None, None);

    let width = 160;
    let height = 144;
    let rgba = 4;
    use wasm_bindgen::Clamped;
    let mut raw = vec![0; height * width * rgba];
    let mut frames = 0;
    use gb::controller::GBControl;
    let keys = std::rc::Rc::new(std::cell::RefCell::new(0xff));

    web_sys::console::log_1(&"Hello World".into());

    use web_sys::KeyboardEvent;
    fn map_key_code(code: &str) -> Option<GBControl> {
        match code {
            "KeyZ" => Some(GBControl::A),
            "KeyX" => Some(GBControl::B),
            "Enter" => Some(GBControl::Start),
            "Backslash" => Some(GBControl::Select),
            "ArrowUp" => Some(GBControl::Up),
            "ArrowDown" => Some(GBControl::Down),
            "ArrowLeft" => Some(GBControl::Left),
            "ArrowRight" => Some(GBControl::Right),
            _ => None,
        }
    }
    let keyup = Closure::wrap(Box::new({
        let keys = keys.clone();
        move |event: KeyboardEvent| {
            if event.repeat() {
                return;
            }
            if let Some(c) = map_key_code(&event.code()) {
                *keys.borrow_mut() |= c as u8;
            }
        }
    }) as Box<dyn FnMut(KeyboardEvent)>);
    let keydown = Closure::wrap(Box::new({
        let keys = keys.clone();
        move |event: KeyboardEvent| {
            if event.repeat() {
                return;
            }
            if let Some(c) = map_key_code(&event.code()) {
                *keys.borrow_mut() &= !(c as u8);
            }
        }
    }) as Box<dyn FnMut(KeyboardEvent)>);

    web_sys::window()
        .unwrap()
        .set_onkeydown(Some(keydown.as_ref().unchecked_ref()));
    web_sys::window()
        .unwrap()
        .set_onkeyup(Some(keyup.as_ref().unchecked_ref()));
    keyup.forget();
    keydown.forget();

    let f = std::rc::Rc::new(std::cell::RefCell::new(None));
    let g = f.clone();
    //let track = web_sys::AudioStreamTrack::new();

    let sample_rate = 4.0 * 16384.0;
    let audio = web_sys::AudioContext::new_with_context_options(
        &web_sys::AudioContextOptions::new().sample_rate(sample_rate as f32),
    )
    .unwrap();

    *g.borrow_mut() = Some(Closure::wrap(Box::new({
        let keys = keys.clone();
        let mut chans = [Vec::new(), Vec::new()];
        let mut prev_sound: Option<
            std::rc::Rc<std::cell::RefCell<web_sys::AudioBufferSourceNode>>,
        > = None;
        let mut prev_sound_end = None;
        let mut first_frame = None;
        let mut last_frame_time = std::collections::VecDeque::new();
        //let mut last_sound = None;
        move || {
            for c in chans.iter_mut() {
                c.clear();
            }
            let time = if last_frame_time.len() >= 2 {
                let avg: f64 = last_frame_time
                    .iter()
                    .zip(last_frame_time.iter().skip(1))
                    .map(|(a, b)| b - a)
                    .sum::<f64>()
                    / (last_frame_time.len() - 1) as f64;
                if avg < std::f64::EPSILON {
                    gb::cycles::SECOND / 30
                } else {
                    gb::cycles::SECOND / ((1.0 / avg) as u64)
                }
            } else {
                gb::cycles::SECOND / 60
            };
            if last_frame_time.len() > 10 {
                last_frame_time.pop_front();
            }
            last_frame_time.push_back(audio.current_time());

            let start = gb.cpu_cycles();
            loop {
                let remain = time - (gb.cpu_cycles() - start);
                let mut sampler = |samples: &[i16]| {
                    let mut scaled = samples
                        .iter()
                        .map(|x| f32::from(*x) / f32::from(std::i16::MAX));
                    for c in chans.iter_mut() {
                        c.push(scaled.next().unwrap());
                    }
                    true
                };

                let mut data = PeripheralData::new(
                    Some(&mut raw),
                    Some(AudioSpec {
                        silence: 0,
                        freq: sample_rate as u32,
                        queue: Box::new(&mut sampler),
                    }),
                );
                gb.set_controls(*keys.borrow());
                let r = gb.step(Some(remain), &mut data);

                if let None = first_frame {
                    first_frame = Some(audio.current_time());
                }

                match r {
                    GBReason::VSync => {
                        let lcd = ImageData::new_with_u8_clamped_array_and_sh(
                            Clamped(&mut raw),
                            width as u32,
                            height as u32,
                        )
                        .unwrap();
                        let context = canvas
                            .get_context("2d")
                            .unwrap()
                            .unwrap()
                            .dyn_into::<CanvasRenderingContext2d>()
                            .unwrap();

                        context.scale(2.0, 2.0).unwrap();
                        context.put_image_data(&lcd, 0.0, 0.0).unwrap();
                        frames += 1;
                    }
                    GBReason::Timeout => {
                        let audio_buf = audio
                            .create_buffer(2, chans[0].len() as u32, sample_rate)
                            .unwrap();
                        for (i, c) in chans.iter_mut().enumerate() {
                            audio_buf.copy_to_channel(&mut c[..], i as i32).unwrap();
                        }
                        //audio_buf.copy_to_channel(&mut chan_r[..], 1).unwrap();
                        //for (i, c) in [chan_l, chan_r].iter_mut().enumerate() {
                        //audio_buf.copy_to_channel(&mut c[..], i as i32).unwrap();
                        //}
                        let source = audio.create_buffer_source().unwrap();
                        source
                            .connect_with_audio_node(&audio.destination())
                            .unwrap();
                        source.set_buffer(Some(&audio_buf));
                        //source.playback_rate().set_value( (1.0 / 59.7) / audio_buf.duration() as f32);
                        let source = std::rc::Rc::new(std::cell::RefCell::new(source));
                        if let Some(p) = prev_sound_end.as_ref() {
                            // let c = Closure::wrap(Box::new({
                            //     let source = source.clone();
                            //     move |_event| {
                            //         source.borrow_mut().start();
                            //     }
                            // }) as Box<dyn FnMut(web_sys::Event)>);

                            // set_onended(p.borrow_mut().as_ref(), &c);
                            // c.forget();
                            if *p < audio.current_time() {
                                source.borrow_mut().start().unwrap();
                            } else {
                                source.borrow_mut().start_with_when(*p).unwrap();
                            }
                        } else {
                            source.borrow_mut().start().unwrap();
                            //source.borrow_mut().start_with_when(first_frame.unwrap() + frames as f64 / 60.0);
                        }
                        prev_sound = Some(source);
                        prev_sound_end = Some(audio.current_time() + audio_buf.duration());

                        // if frames % 60 == 0 {
                        //     web_sys::console::log_1(
                        //         &format!(
                        //             "{} {} {}",
                        //             audio_buf.duration(),
                        //             audio_buf.length(),
                        //             audio_buf.sample_rate()
                        //         )
                        //             .into(),
                        //     );
                        // }
                        request_animation_frame(f.borrow().as_ref().unwrap());
                        break;
                    }
                    r => panic!("{:?} Unexpected Response", r),
                }
            }
        }
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}
