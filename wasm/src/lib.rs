use gb::cart::Cart;
use gb::gb::{GBReason, GB};
use gb::peripherals::{AudioSpec, PeripheralData};
use std::f64;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, ImageData};

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

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

struct SoundBuffer {
    buf_len: usize,
    channels: usize,
    bufs: Vec<web_sys::AudioBuffer>,
    target_buf: usize,
    audio_cxt: web_sys::AudioContext,
    last_finish: Option<f64>,
    sample_buf: Vec<Vec<f32>>,
}

impl SoundBuffer {
    fn new(cxt: web_sys::AudioContext, channels: usize) -> SoundBuffer {
        let num_bufs = 240;
        let sample_rate = cxt.sample_rate() as usize;
        let buf_len = sample_rate / num_bufs;
        let bufs = (0..num_bufs)
            .map(|_| {
                cxt.create_buffer(channels as u32, buf_len as u32, sample_rate as f32)
                    .expect("Create SoundBuffer AudioBufs")
            })
            .collect();

        SoundBuffer {
            sample_buf: vec![Vec::new(); channels],
            audio_cxt: cxt,
            buf_len,
            bufs,
            channels,
            target_buf: 0,
            last_finish: None,
        }
    }
    fn commit(&mut self) {
        let ring_buf = self.bufs[self.target_buf..]
            .iter()
            .chain(self.bufs[..self.target_buf].iter());

        let mut lens = None;
        for c in 0..self.channels {
            let chunks = self.sample_buf[c].chunks_exact_mut(self.buf_len);
            let tmp_lens = chunks
                .zip(ring_buf.clone())
                .map(|(mut chunk, buf)| {
                    buf.copy_to_channel(&mut chunk, c as i32)
                        .expect("Copy to SoundBuffer");
                    chunk.len()
                })
                .collect::<Vec<usize>>();
            //self.sample_buf[c].clear();
            self.sample_buf[c].drain(0usize..(tmp_lens.iter().sum()));
            lens = Some(tmp_lens);
        }
        if let Some(count) = lens {
            //log!("lens {}", count.iter().sum::<usize>());
            for (buf, len) in ring_buf.zip(count.iter()) {
                //log!("Filled Buf");
                let source = self
                    .audio_cxt
                    .create_buffer_source()
                    .expect("Unable to create buffer source");
                source
                    .connect_with_audio_node(&self.audio_cxt.destination())
                    .expect("Unable to connect to audio destination");
                source.set_buffer(Some(&buf));
                let duration = if *len != self.buf_len {
                    Some((*len as f64 / self.buf_len as f64) * buf.duration())
                } else {
                    None
                };
                let new_start = match self.last_finish {
                    None => None,
                    Some(finish) if finish < self.audio_cxt.current_time() => None,
                    Some(finish) => Some(finish),
                };
                let start = if let Some(start) = new_start {
                    source
                        .start_with_when(start)
                        .expect("Unable to start source sound");
                    start
                } else {
                    source.start().expect("Unable to start source sound");
                    self.audio_cxt.current_time()
                };
                let new_finish = if let Some(d) = duration {
                    let f = start + d;
                    source.stop_with_when(f).unwrap();
                    f
                } else {
                    start + buf.duration()
                };
                self.last_finish = Some(new_finish);
            }

            self.target_buf += count.len();
            self.target_buf %= self.bufs.len();
        }
    }
    fn sample<T: Clone + Iterator<Item = f32>>(&mut self, samples: T) {
        for c in 0..self.channels {
            let samples = samples.clone();
            self.sample_buf[c].extend(samples.skip(c).step_by(self.channels));
        }
    }
}

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

    let mut gb = GB::new(
        cart,
        None,
        false,
        None,
        None,
        Some((gb::cycles::SECOND / 65536).into()),
    );

    let width = 160;
    let height = 144;
    let rgba = 4;
    use wasm_bindgen::Clamped;
    let mut raw = vec![0; height * width * rgba];
    let mut frames = 0;
    use gb::controller::GBControl;
    let keys = std::rc::Rc::new(std::cell::RefCell::new(0xff));

    log!("Hello World");

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

    *g.borrow_mut() = Some(Closure::wrap(Box::new({
        let keys = keys.clone();
        let mut last_frame_time = std::collections::VecDeque::new();
        let channels = 2;
        let sample_rate = 4.0 * 16384.0;
        let audio = web_sys::AudioContext::new_with_context_options(
            &web_sys::AudioContextOptions::new().sample_rate(sample_rate as f32),
        )
        .unwrap();
        let perf = web_sys::window().unwrap().performance().unwrap();
        let mut sound_buf = SoundBuffer::new(audio, channels);
        move || {
            let now = perf.now() / 1000.0 * gb::dimensioned::si::S;
            last_frame_time.push_front((gb.cpu_cycles(), now));
            let time = if last_frame_time.len() >= 2 {
                let first = last_frame_time[0];
                let last = last_frame_time[last_frame_time.len() - 1];
                let cycle_delta: gb::dimensioned::si::Second<f64> = (first.0 - last.0).into();
                let time_delta = first.1 - last.1;
                let next_time = (first.1 - last.1) / (last_frame_time.len() - 1) as f64;

                //This seems like the wrong equation but the results in terms of cycles seem right on the nose.
                // The sound doesn't come out right, though.
                //let r = next_time * (cycle_delta / time_delta);
                let r = next_time * (time_delta / cycle_delta);
                let r: gb::cycles::CycleCount = r.into();
                log!(
                    "Next Cycle Estimate {} {} (Avg Cycles {})",
                    r,
                    next_time,
                    gb::cycles::CycleCount::from(cycle_delta / time_delta * gb::dimensioned::si::S)
                );
                r
            } else {
                gb::cycles::SECOND / 60
            };
            if last_frame_time.len() > 60 {
                last_frame_time.pop_back();
            }

            let start = gb.cpu_cycles();
            let vsync_time = gb::cycles::CycleCount::new(35112);
            loop {
                let remain = time - (gb.cpu_cycles() - start);
                let mut sampler = |samples: &[i16]| {
                    let scaled = samples
                        .iter()
                        .map(|x| f32::from(*x) / f32::from(std::i16::MAX));
                    sound_buf.sample(scaled);
                    true
                };

                let mut data = PeripheralData::new(
                    if remain < 2 * vsync_time {
                        Some(&mut raw)
                    } else {
                        None
                    },
                    Some(AudioSpec {
                        silence: 0,
                        freq: sample_rate as u32,
                        queue: Box::new(&mut sampler),
                    }),
                );
                gb.set_controls(*keys.borrow());
                let r = gb.step(Some(remain), &mut data);
                match r {
                    GBReason::VSync => {
                        if remain < 2 * gb::cycles::CycleCount::new(35112) {
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
                        }
                        frames += 1;
                    }
                    GBReason::Timeout => {
                        sound_buf.commit();
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
