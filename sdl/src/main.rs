use clap::Parser;
use gb::cart::Cart;
use gb::controller::GBControl;
use gb::gb::{GBReason, GB};
use gb::peripherals::{AudioSpec, PeripheralData};
use gb::rewind::Rewind;
use sdl2::pixels::Color;
use std::io::Read;
use std::path::PathBuf;

fn sdl(gb: GB) -> Result<(), std::io::Error> {
    use sdl2::audio::AudioSpecDesired;
    use sdl2::event::Event;
    use sdl2::gfx::framerate::FPSManager;
    use sdl2::keyboard::Keycode;

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    // the window is the representation of a window in your operating system,
    // however you can only manipulate properties of that window, like its size, whether it's
    // fullscreen, ..= but you cannot change its content without using a Canvas or using the
    // `surface()` method.

    let window = video_subsystem
        .window("rust-sdl2 demo: Game of Life", 160, 144)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    // the canvas allows us to both manipulate the property of the window and to change its content
    // via hardware or software rendering. See CanvasBuilder for more info.
    let mut canvas = window
        .into_canvas()
        .target_texture()
        .present_vsync()
        .accelerated()
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
    let mut fps = FPSManager::new();
    fps.set_framerate(90).expect("Unable to set framerate");

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();

    let tc = canvas.texture_creator();
    let mut texture = tc
        .create_texture_streaming(
            /* wtf, I am writing in RGBA order, even WASM agrees */
            sdl2::pixels::PixelFormatEnum::ABGR8888,
            160,
            144,
        )
        .unwrap();

    let mut controls: u8 = 0xff;
    let desired_spec = AudioSpecDesired {
        freq: Some(16384 * 4),
        channels: Some(2),
        samples: None,
    };
    //let mut frames = 0;
    assert_eq!((4_194_304 / 4) % desired_spec.freq.unwrap(), 0);
    let audio_subsystem = sdl_context.audio().unwrap();
    let device = audio_subsystem
        .open_queue::<i16, _>(None, &desired_spec)
        .unwrap();
    device.resume();

    fn match_keycode(key: Keycode) -> Option<GBControl> {
        match key {
            Keycode::Left => Some(GBControl::Left),
            Keycode::Right => Some(GBControl::Right),
            Keycode::Up => Some(GBControl::Up),
            Keycode::Down => Some(GBControl::Down),
            Keycode::Z => Some(GBControl::A),
            Keycode::X => Some(GBControl::B),
            Keycode::Tab => Some(GBControl::Select),
            Keycode::Return => Some(GBControl::Start),
            _ => None,
        }
    }
    let mut mute = false;

    let mut rewind = Rewind::new(gb);

    'running: loop {
        let mod_state = sdl_context.keyboard().mod_state();
        let paused = mod_state.contains(sdl2::keyboard::Mod::LSHIFTMOD)
            || mod_state.contains(sdl2::keyboard::Mod::RSHIFTMOD);

        // get the inputs here
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyUp {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyUp {
                    keycode: Some(Keycode::T),
                    repeat: false,
                    ..
                } => rewind.gb().toggle_trace(),
                Event::KeyUp {
                    keycode: Some(Keycode::M),
                    repeat: false,
                    ..
                } => {
                    mute = !mute;
                }
                Event::KeyDown {
                    keycode: Some(k),
                    repeat: _,
                    ..
                } => {
                    if paused {
                        match k {
                            Keycode::Right => rewind.forward(),
                            Keycode::Left => rewind.back(),
                            _ => {}
                        }
                    }
                    if let Some(k) = match_keycode(k) {
                        controls &= !(k as u8);
                    }
                }
                Event::KeyUp {
                    keycode: Some(k),
                    repeat: false,
                    ..
                } => {
                    if let Some(k) = match_keycode(k) {
                        controls |= k as u8;
                    }
                }
                _ => {}
            }
        }

        rewind.gb().set_controls(controls);
        let start = rewind.gb().cpu_cycles();
        let time = gb::cycles::SECOND / 60;
        'frame: loop {
            let remain = time - (rewind.gb().cpu_cycles() - start);
            let r = texture.with_lock(sdl2::rect::Rect::new(0, 0, 160, 144), |mut slice, _size| {
                let audio_spec = Some(AudioSpec {
                    silence: 0,
                    freq: device.spec().freq as u32,
                    queue: Box::new(|samples| {
                        if !mute {
                            device.queue_audio(samples)
                        } else {
                            device.queue_audio(&[0, 0])
                        }
                        .is_ok()
                    }),
                });
                if !paused {
                    rewind.step(
                        Some(remain),
                        &mut PeripheralData::new(Some(&mut slice), None, audio_spec),
                    )
                } else {
                    rewind.restore(&mut PeripheralData::new(Some(&mut slice), None, None));
                    GBReason::Timeout
                }
            });
            match r {
                Ok(GBReason::VSync) => {
                    //frames += 1;
                }
                Ok(GBReason::Dead) => {
                    break 'running;
                }
                Ok(GBReason::Timeout) => {
                    break 'frame;
                }
                _ => {}
            }
        }
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }
    device.pause();
    device.clear();
    Ok(())
}

#[derive(Parser)]
struct Args {
    #[clap(short, long, help = "Sets a serial output file")]
    serial: bool,
    #[clap(help = "ROM File")]
    rom: PathBuf,

    #[clap(short, long)]
    trace: bool,

    #[clap(short, long)]
    boot_rom: Option<PathBuf>,

    #[clap(long, help = "Don't show a display (useful for testing, benchmarks)")]
    no_display: bool,

    #[clap(
        short,
        long,
        help = "Specify a custom GBC Palette (if using GB rom w/ no boot rom)"
    )]
    palette: Option<usize>,

    #[clap(short, long, parse(from_occurrences))]
    verbose: u32,
}

fn main() -> Result<(), std::io::Error> {
    use gb::dimensioned::si;
    //let s = si::Second::from(gb::cycles::SECOND);
    //let s : si::Second<u64> = gb::cycles::SECOND.into();
    let s = gb::cycles::CGB::from(1.0 * si::S);

    println!("Cycles from SI: {}", s);
    let s2: si::Second<f64> = gb::cycles::SECOND.into();
    println!("SI from Cycles: {}", s2);
    let s3 = gb::cycles::CGB::from(1.0 * si::S / 4096.0);
    println!("Hz {}", s3);
    // let palettes: Vec<_> = gb::display::KEY_PALETTES
    //     .iter()
    //     .enumerate()
    //     .map(|(i, k)| format!("{} ({})", i, k.keys))
    //     .collect();
    //let palettes: Vec<_> = palettes.iter().map(|x| x.as_str()).collect();
    let args = Args::parse();

    let boot_rom = match &args.boot_rom {
        Some(name) => {
            let mut file = std::fs::File::open(name)?;
            match file.metadata()?.len() {
                256 | 2304 => {}
                _ => {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid Boot Rom Size (Not 256 or 2304 bytes)",
                    ))?;
                }
            }
            let mut v = Vec::with_capacity(256);
            file.read_to_end(&mut v)?;
            Some(v)
        }
        None => None,
    };
    let maybe_rom: std::io::Result<Vec<u8>> = match args.rom.extension() {
        None => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Missing file extension {}", args.rom.display()),
        )),
        Some(ext) => match ext.to_str() {
            Some("zip") => {
                let f = std::fs::File::open(&args.rom)?;
                let mut z = zip::ZipArchive::new(f)?;
                let mut res = None;
                for c_id in 0..z.len() {
                    if let Ok(mut c_file) = z.by_index(c_id) {
                        if c_file.name().ends_with(".gb") || c_file.name().ends_with(".gbc") {
                            let mut buf = Vec::new();
                            c_file.read_to_end(&mut buf)?;
                            res = Some(buf);
                        }
                    }
                }
                if let Some(buf) = res {
                    Ok(buf)
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "No rom file found in archive",
                    ))
                }
            }
            Some("gb") | Some("gbc") => Ok(std::fs::read(&args.rom)?),
            Some(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Unknown Extension {}", e),
            )),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Invalid Extension"),
            )),
        },
    };

    let rom_vec = maybe_rom?;

    let cart = Cart::new(rom_vec);

    // let mut serial: Box<dyn Write> =
    //     matches
    //         .value_of("serial")
    //         .map_or(Box::new(std::io::sink()), |p| {
    //             let f = File::create(p).expect("Unable to create serial output file");
    //             Box::new(std::io::BufWriter::new(f))
    //         });

    let mut gb = GB::new(
        cart,
        args.trace,
        boot_rom,
        args.palette,
        Some((gb::cycles::SECOND / 65536).into()),
    );

    if args.no_display {
        loop {
            if let GBReason::Dead = gb.step(None, &mut PeripheralData::empty()) {
                break;
            }
        }
        Ok(())
    } else {
        sdl(gb)
    }
}
