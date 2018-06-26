extern crate clap;
extern crate sdl2;
extern crate gb;
use std::fs::File;
use gb::gb::{GB};

use std::io::{Read, Write};

fn main() -> Result<(), std::io::Error>  {
    use clap::{Arg, App, SubCommand};

    let matches = App::new("GB Rom Emulator")
        .version("0.0.1")
        .author("Mitch Souders. <mitch.souders@gmail.com>")
        .about("Runs GB Roms")
        .arg(Arg::with_name("serial")
             .short("s")
             .long("serial")
             .value_name("FILE")
             .help("Sets a serial output file")
             .takes_value(true))
        .arg(Arg::with_name("ROM")
             .help("Sets the rom file to use")
             .required(true)
             .index(1))
        .arg(Arg::with_name("trace")
             .short("t")
             .help("Enables Traced Runs"))
        .arg(Arg::with_name("v")
             .short("v")
             .multiple(true)
             .help("Sets the level of verbosity"))
        .get_matches();

    use sdl2::rect::{Point, Rect};
    use sdl2::pixels::Color;
    use sdl2::event::Event;
    use sdl2::mouse::MouseButton;
    use sdl2::keyboard::Keycode;
    use sdl2::video::{Window, WindowContext};
    use sdl2::render::{Canvas, Texture, TextureCreator};
    use std::time::Duration;

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    // the window is the representation of a window in your operating system,
    // however you can only manipulate properties of that window, like its size, whether it's
    // fullscreen, ... but you cannot change its content without using a Canvas or using the
    // `surface()` method.
    let window = video_subsystem
        .window("rust-sdl2 demo: Game of Life",
                160,
                144)
        .position_centered()
        .build()
        .unwrap();

    // the canvas allows us to both manipulate the property of the window and to change its content
    // via hardware or software rendering. See CanvasBuilder for more info.
    let mut canvas = window.into_canvas()
        .target_texture()
        .present_vsync()
        .build().unwrap();

    println!("Using SDL_Renderer \"{}\"", canvas.info().name);

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    // clears the canvas with the color we set in `set_draw_color`.
    canvas.clear();
    // However the canvas has not been updated to the window yet, everything has been processed to
    // an internal buffer, but if we want our buffer to be displayed on the window, we need to call
    // `present`. We need to call this everytime we want to render a new frame on the window.
    canvas.present();

    let rom = matches.value_of("ROM").unwrap();
    let rom_vec = std::fs::read(rom)?;

    let mut serial : Box<Write> =
        matches.value_of("serial")
        .map_or(
            Box::new(std::io::sink()),
            |p|  {
                let f = File::create(p).expect("Unable to create serial output file");
                Box::new(std::io::BufWriter::new(f))
            }
        );

    let mut gb = GB::new(rom_vec, Some(&mut *serial), matches.occurrences_of("trace") > 0);
    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut frame : u32 = 0;

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    'running: loop {
        // get the inputs here
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                Event::KeyDown { keycode: Some(Keycode::Space), repeat: false, .. } => {
                    //game.toggle_state();
                },
                Event::MouseButtonDown { x, y, mouse_btn: MouseButton::Left, .. } => {
                    // let x = (x as u32) / SQUARE_SIZE;
                    // let y = (y as u32) / SQUARE_SIZE;
                    // match game.get_mut(x as i32, y as i32) {
                    //     Some(square) => {*square = !(*square);},
                    //     None => {panic!()}
                    // };
                },
                _ => {}
            }
        }


        //if let game_of_life::State::Playing = game.state() {
        frame += 1;
        //};
        {
            if gb.step(1_000u64 / 60, &mut Some(&mut canvas)) {
                break 'running;
            }
        }
        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
