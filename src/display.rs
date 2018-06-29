use peripherals::Peripheral;
use cpu::InterruptFlag;

#[derive(PartialEq,Copy,Clone)]
enum DisplayState {
    Off,
    OAMSearch,     //20 Clocks
    PixelTransfer, //43 + Clocks
    HBlank,        //51 Clocks
    VBlank,        //(20 + 43 + 51) * 10
}

enum StatFlag {
    CoincidenceInterrupt = 1 << 6,
    OAMInterrupt = 1 << 5,
    VBlankInterrupt = 1 << 4,
    HBlankInterrupt = 1 << 3,
    Coincidence = 1 << 2,
    HBlank = 0b00,
    VBlank = 0b01,
    OAM = 0b10,
    PixelTransfer = 0b11,
}

pub struct Display {
    vram: [u8; 8 << 10],
    oam: [u8; 4 * 40],
    scx: u8,
    scy: u8,
    lcdc: u8,
    stat: u8,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,

    //ppu: [u8; 16],
    rendered: Vec<((u8, u8, u8, u8), (i32, i32))>,
    unused_cycles: u64,
    state: DisplayState,
}

pub trait LCD<C, P> {
    fn draw_point(&mut self, c: C, point: P);
    fn screen_power(&mut self, on: bool);
}

impl<T> LCD<sdl2::pixels::Color, sdl2::rect::Point> for sdl2::render::Canvas<T>
where
    T: sdl2::render::RenderTarget,
{
    fn draw_point(&mut self, c: sdl2::pixels::Color, point: sdl2::rect::Point) {
        self.set_draw_color(c);
        self.draw_point(point).expect("Couldn't draw a point");
    }
    fn screen_power(&mut self, on: bool) {
        if on {
            self.set_draw_color(sdl2::pixels::Color::RGB(0xff, 0xff, 0xff));
        } else {
            self.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
        }
        self.clear();
    }
}

impl Display {
    pub fn new() -> Display {
        Display {
            vram: [0; 8 << 10],
            oam: [0; 4 * 40],
            scx: 0,
            scy: 0,
            lcdc: 0,
            stat: 0,
            ly: 0,
            lyc: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            //ppu: [0u8; 16],
            rendered: Vec::with_capacity(160),
            state: DisplayState::OAMSearch,
            unused_cycles: 0,
        }
    }
    pub fn render<C: From<(u8, u8, u8, u8)>, P: From<(i32, i32)>>(
        &mut self,
        lcd: &mut Option<&mut LCD<C, P>>,
    ) {
        if self.state == DisplayState::Off || lcd.is_none() {
            /* no display */
        } else if let Some(lcd) = lcd {
            for (c, p) in self.rendered.drain(..) {
                //println!("Drawing Point {:?} {:?}", c, p);
                lcd.draw_point(c.into(), p.into());
            }
        }
        self.rendered.clear();
    }
    fn tile_color(&mut self, x: u8) -> (u8, u8) {
        let true_x = self.wx.wrapping_add(x.wrapping_add(self.scx) % 160);
        let true_y = self.wy.wrapping_add(self.ly.wrapping_add(self.scy) % 144);
        //println!("X: {}, Y: {}, True X: {}, True Y: {}", x, self.ly, true_x, true_y);
        let tile_idx = self.get_bg_tile(true_x / 8, true_y / 8);

        self.tile_8_8(tile_idx, true_y % 8)
    }
    fn tile_offset(&mut self, t: u8) -> u16 {
        t as u16 * 16
    }

    fn tile_8_8(&mut self, t: u8, y: u8) -> (u8, u8) {
        let t_off: usize = self.tile_offset(t) as usize;
        let line_off = y as usize * 2;

        (self.vram[t_off + line_off], self.vram[t_off + line_off + 1])
    }

    fn bit_color(c_hi: u8, c_lo: u8) -> u8 {
        (((c_hi & 0x80) >> 6) | ((c_lo & 0x80) >> 7))
    }

    fn get_bg_tile(&mut self, x: u8, y: u8) -> u8 {
        let idx = 0x1800 + y as u16 * 32 + x as u16;
        if self.vram[idx as usize] != 0 {
            //println!("Tile x: {}, Tile Y : {}, Val: {}", x, y, self.vram[idx as usize]);
        }
        self.vram[idx as usize]
    }

    pub fn dump(&mut self) {
        println!("BG Tile Map");
        for y in 0..32 {
            for x in 0..32 {
                print!("{:02x} ", self.get_bg_tile(x, y));
            }
            println!("");
        }

        for t in 0..=0x20 {
            println!("Tile {}", t);
            for y in 0..8 {
                let (mut c_hi, mut c_lo) = self.tile_8_8(t, y);
                for _x in 0..8 {
                    print!("{} ", Display::bit_color(c_hi, c_lo));
                    c_hi <<= 1;
                    c_lo <<= 1;
                }
                println!("");
            }
        }
    }
}

impl Peripheral for Display {
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match addr {
            0x8000...0x9fff => &mut self.vram[(addr - 0x8000) as usize],
            0xFE00...0xFE9F => &mut self.oam[(addr & 0xff) as usize],
            0xff40 => &mut self.lcdc,
            0xff41 => &mut self.stat,
            0xff42 => &mut self.scy,
            0xff43 => &mut self.scx,
            0xff44 => &mut self.ly,
            0xff45 => &mut self.lyc,
            0xff47 => &mut self.bgp,
            0xff48 => &mut self.obp0,
            0xff49 => &mut self.obp1,
            0xff4a => &mut self.wy,
            0xff4b => &mut self.wx,
            _ => panic!("unhandled address in display {:x}", addr),
        }
    }

    fn step(&mut self, time: u64) -> Option<InterruptFlag> {
        self.unused_cycles += time;
        let next_state = match self.state {
            DisplayState::OAMSearch => {
                if self.unused_cycles >= 20 {
                    /* do work */
                    self.unused_cycles -= 20;
                    DisplayState::PixelTransfer
                } else {
                    self.state
                }
            }
            DisplayState::PixelTransfer => {
                if self.unused_cycles >= 43 {
                    /* do work */
                    for x in 0..(160 / 8) {
                        let (mut c_hi, mut c_lo) = self.tile_color(x * 8);
                        for sub_x in 0..8 {
                            let color: (u8, u8, u8, u8) = match Display::bit_color(c_hi, c_lo) {
                                0b00 => (0xff, 0xff, 0xff, 0xff),
                                0b01 => (0, 0, 0, 0xff),
                                0b10 => (0, 0, 0, 0xff),
                                0b11 => (0, 0, 0, 0xff),
                                c => panic!("invalid pixel color {:b}", c),
                            };
                            c_hi <<= 1;
                            c_lo <<= 1;
                            self.rendered
                                .push((color, ((x * 8 + sub_x) as i32, self.ly as i32)));
                            //println!("{:?}", self.rendered[self.rendered.len() - 1])
                        }
                    }
                    self.unused_cycles -= 43;
                    DisplayState::HBlank
                } else {
                    self.state
                }
            }
            DisplayState::HBlank => {
                if self.unused_cycles >= 51 {
                    /* do work */
                    self.unused_cycles -= 51;
                    self.ly += 1;
                    if self.ly == 144 {
                        DisplayState::VBlank
                    } else {
                        DisplayState::OAMSearch
                    }
                } else {
                    self.state
                }
            }
            DisplayState::VBlank => {
                self.stat &= !0b11;
                self.stat |= 0b01;

                if self.lcdc & 0x80 == 0 {
                    DisplayState::Off
                } else if self.unused_cycles >= (43 + 51 + 20) {
                    /* do work */
                    self.unused_cycles -= 43 + 51 + 20;
                    self.ly += 1;
                    if self.ly == 153 {
                        self.ly = 0;
                        DisplayState::OAMSearch
                    } else {
                        self.state
                    }
                } else {
                    self.state
                }
            }
            DisplayState::Off => {
                if self.lcdc & 0x80 != 0 {
                    DisplayState::VBlank
                } else {
                    self.state
                }
            }
        };

        let mut triggers = if next_state != self.state {
            let state_trig =
                flag_u8!(StatFlag::OAMInterrupt, next_state == DisplayState::OAMSearch)
                | flag_u8!(StatFlag::VBlankInterrupt, next_state == DisplayState::VBlank)
                | flag_u8!(StatFlag::HBlankInterrupt, next_state == DisplayState::HBlank);
            self.state = next_state;
            state_trig
        } else {
            flag_u8!(StatFlag::CoincidenceInterrupt, self.ly == self.lyc)
        };
        triggers &= self.stat;

        self.stat &= 0b111;
        self.stat |= match self.state {
            DisplayState::OAMSearch => StatFlag::OAM,
            DisplayState::VBlank => StatFlag::VBlank,
            DisplayState::HBlank => StatFlag::HBlank,
            DisplayState::PixelTransfer => StatFlag::PixelTransfer,
            // Pretend we are in vblank when screen is off, same invariants
            _ => StatFlag::VBlank,
        } as u8 & 0b11;
        self.stat |= if self.ly == self.lyc {mask_u8!(StatFlag::Coincidence)} else {0};

        if triggers & mask_u8!(StatFlag::VBlankInterrupt) != 0 {
            /* TODO: The LCDC interrupt may also need to be triggered here */
            Some(InterruptFlag::VBlank)
        } else if triggers != 0 {
            Some(InterruptFlag::LCDC)
        } else {
            None
        }
    }
}
