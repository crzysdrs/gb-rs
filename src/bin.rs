extern crate clap;
extern crate gb;
extern crate sdl2;
use gb::gb::GB;
use std::fs::File;

use gb::display::LCD;
use sdl2::pixels::Color;
use std::io::{Read, Write};

fn sdl(gb: &mut GB) -> Result<(), std::io::Error> {
    use sdl2::event::Event;
    use sdl2::gfx::framerate::FPSManager;
    use sdl2::keyboard::Keycode;
    use sdl2::mouse::MouseButton;
    use sdl2::rect::{Point, Rect};
    use sdl2::render::{Canvas, Texture, TextureCreator};
    use sdl2::video::{Window, WindowContext};
    use std::time::Duration;

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    // the window is the representation of a window in your operating system,
    // however you can only manipulate properties of that window, like its size, whether it's
    // fullscreen, ... but you cannot change its content without using a Canvas or using the
    // `surface()` method.
    let window = video_subsystem
        .window("rust-sdl2 demo: Game of Life", 160, 144)
        .position_centered()
        .build()
        .unwrap();

    // the canvas allows us to both manipulate the property of the window and to change its content
    // via hardware or software rendering. See CanvasBuilder for more info.
    let mut canvas = window
        .into_canvas()
        .target_texture()
        .present_vsync()
        .build()
        .unwrap();

    println!("Using SDL_Renderer \"{}\"", canvas.info().name);

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    // clears the canvas with the color we set in `set_draw_color`.
    canvas.clear();
    // However the canvas has not been updated to the window yet, everything has been processed to
    // an internal buffer, but if we want our buffer to be displayed on the window, we need to call
    // `present`. We need to call this everytime we want to render a new frame on the window.
    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut frame: u32 = 0;
    let mut fps = FPSManager::new();
    fps.set_framerate(60);

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    'running: loop {
        // get the inputs here
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Space),
                    repeat: false,
                    ..
                } => {}
                Event::MouseButtonDown {
                    x,
                    y,
                    mouse_btn: MouseButton::Left,
                    ..
                } => {}
                _ => {}
            }
        }

        frame += 1;

        {
            if gb.step(1_000u64 / 60, &mut Some(&mut canvas)) {
                break 'running;
            }
        }
        canvas.present();
        fps.delay();
    }

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    use clap::{App, Arg, SubCommand};

    let matches = App::new("GB Rom Emulator")
        .version("0.0.1")
        .author("Mitch Souders. <mitch.souders@gmail.com>")
        .about("Runs GB Roms")
        .arg(
            Arg::with_name("serial")
                .short("s")
                .long("serial")
                .value_name("FILE")
                .help("Sets a serial output file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ROM")
                .help("Sets the rom file to use")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("trace")
                .short("t")
                .help("Enables Traced Runs"),
        )
        .arg(
            Arg::with_name("no-display")
                .short("n")
                .help("Don't show a display (useful for testing, benchmarks)"),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches();

    let rom = matches.value_of("ROM").unwrap();
    let rom_vec = std::fs::read(rom)?;

    let mut serial: Box<Write> = matches.value_of("serial").map_or(
        Box::new(std::io::sink()),
        |p| {
            let f = File::create(p).expect("Unable to create serial output file");
            Box::new(std::io::BufWriter::new(f))
        },
    );

    let mut gb = GB::new(
        rom_vec,
        Some(&mut *serial),
        matches.occurrences_of("trace") > 0,
    );

    if matches.occurrences_of("no-display") > 0 {
        gb.step::<(u8, u8, u8, u8), (i32, i32)>(0, &mut None);
        Ok(())
    } else {
        sdl(&mut gb)
    }
}
