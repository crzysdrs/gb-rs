use cpu::InterruptFlag;
use peripherals::Peripheral;
use itertools::Itertools;
use std::collections::VecDeque;

#[derive(PartialEq, Copy, Clone)]
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

#[derive(Copy, Clone)]
struct SpriteAttribute {
    y : u8,
    x : u8,
    pattern: u8,
    flags: u8,
}

pub struct Display {
    vram: [u8; 8 << 10],
    oam: [SpriteAttribute; 40],
    oam_searched : Vec<usize>,
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
    ppu: PPU,
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
            oam: [SpriteAttribute { x: 0, y: 0, flags: 0, pattern: 0} ; 40],
            oam_searched: Vec::with_capacity(10),
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
            ppu: PPU::new(),
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
    fn oam_search(&mut self)
    {
        let (idxs, _) : (Vec<usize>, Vec<&SpriteAttribute>) = self.oam.iter().enumerate().filter(
            /* ignored invisible sprites */
            |(_i, oam)|
            oam.y != 0 && oam.y < 144 + 16
        ).filter(
            /* filter only items in this row */
            |(_i, oam)|
            self.ly + 16 >= oam.y && self.ly + 16 - oam.y <= 8 //TODO: Should be 16 in Tall Sprite Mode
        ).sorted_by_key(
            |(_i, oam)|
            oam.x
        ).into_iter().take(10).unzip();
        self.oam_searched = idxs;
    }

    fn get_bg_true(&self, x: u8, y: u8) -> (u8, u8) {
        let true_x = self.wx.wrapping_add(x.wrapping_add(self.scx) % 160);
        let true_y = self.wy.wrapping_add(y.wrapping_add(self.scy) % 144);
        (true_x, true_y)
    }
    fn get_screen_bg_tile(&self, x: u8, y: u8) -> u8 {
        let (true_x, true_y) = self.get_bg_true(x, y);
        let tile_x = true_x / 8;
        let tile_y = true_y / 8;

        self.get_bg_tile(tile_x, tile_y)
    }

    fn get_bg_tile(&self, true_x: u8, true_y: u8) -> u8 {
        let bg_map = if self.lcdc & (1 << 3) == 0 {0x1800} else {0x1C00};
        let idx = bg_map + true_y as u16 * 32 + true_x as u16;
        self.vram[idx as usize]
    }
    fn tile_color(&self, tile_idx: u8, y_offset : u8, pt : PixelType) -> (u16, PixelType) {
        let start = match pt {
            PixelType::BG => {
                (if self.lcdc & (1 << 4) == 0 {0x800} else {0}) + tile_idx as u16 * 16
            }
            PixelType::Sprite => {
                tile_idx as u16 * 16
            }
        };
        (self.line_color(start, y_offset), pt)
    }
    fn line_color(&self, start: u16, y_offset :u8) -> u16 {
        let start = start as usize;
        let y_offset = y_offset as usize;
        (self.vram[start + (y_offset * 2)] as u16) << 8
            | self.vram[start + (y_offset * 2) + 1] as u16
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
            let mut ppu = PPU::new();
            for y in 0..8 {
                let line = self.tile_color(t, y, PixelType::BG);
                ppu.load_line(line);
                for _x in 0..8 {
                    let c = match ppu.shift() {
                        Palette::Empty => 0,
                        Palette::Low(_) => 1,
                        Palette::Mid(_) => 2,
                        Palette::High(_) => 3,
                    };
                    print!("{} ", c)
                }
                println!("");
            }
        }
    }
}

impl Display {
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match addr {
            0x8000...0x9fff => &mut self.vram[(addr - 0x8000) as usize],
            0xFE00...0xFE9F => {
                let idx = ((addr & 0xff) >> 2) as usize;
                let oam = &mut self.oam[idx];
                match addr & 0b11 {
                    0b00 => &mut oam.y,
                    0b01 => &mut oam.x,
                    0b10 => &mut oam.pattern,
                    0b11 => &mut oam.flags,
                    _ => panic!("invalid oam access"),
                }
            }
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

    fn bgp_shade(&self, shade_id : u8 ) -> (u8, u8, u8, u8) {
        let white = (0xff, 0xff, 0xff, 0xff);
        let dark_grey = (0xD3, 0xD3, 0xD3, 0xff);
        let light_grey = (0x80, 0x80, 0x80, 0xff);
        let black = (0x00, 0x00, 0x00, 0xff);

        match (self.bgp >> (shade_id * 2)) & 0b11 {
            0b00 => white,
            0b01 => light_grey,
            0b10 => dark_grey,
            0b11 => black,
            _ => panic!("shade out of bounds")
        }
    }
}

#[derive(PartialEq, Copy, Clone)]
enum PixelType {
    BG,
    Sprite
}

#[derive(PartialEq)]
enum Palette {
    Empty,
    Low(PixelType),
    Mid(PixelType),
    High(PixelType)
}

struct PPU
{
    shift : VecDeque<Palette>
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            shift : VecDeque::with_capacity(16)
        }
    }
    fn load_line(&mut self, (mut line, typ) : (u16, PixelType)) {
        for x in 0..8 {
            let p = match (line & 0x8000 != 0, line & 0x80 != 0) {
                (true, true) => Palette::High(typ),
                (true, false) => Palette::Mid(typ),
                (false, true) => Palette::Low(typ),
                (false, false) => Palette::Empty,
            };

            match typ {
                PixelType::Sprite => {
                    if p != Palette::Empty {
                        match self.shift[x] {
                            Palette::High(PixelType::BG) | Palette::Mid(PixelType::BG) | Palette::Low(PixelType::BG) | Palette::Empty => {
                                self.shift[x] = p;
                            },
                            _ => {/* existing non-background data */},
                        }
                    }
                },
                PixelType::BG => {
                    self.shift.push_back(p);
                }
            }
            line <<= 1;
        }
    }
    fn shift(&mut self) -> Palette {
        self.shift.pop_front().unwrap()
    }
    fn need_data(&self) -> bool {
        self.shift.len() <= 8
    }
    fn clear(&mut self) {
        self.shift.clear()
    }
}

impl Peripheral for Display {
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        *self.lookup(addr) = v;
    }
    fn step(&mut self, time: u64) -> Option<InterruptFlag> {
        self.unused_cycles += time;
        let next_state = match self.state {
            DisplayState::OAMSearch => {
                if self.unused_cycles >= 20 {
                    self.oam_search();
                    self.unused_cycles -= 20;
                    DisplayState::PixelTransfer
                } else {
                    self.state
                }
            }
            DisplayState::PixelTransfer => {
                if self.unused_cycles >= 43 {
                    /* do work */
                    let mut orig_oams = std::mem::replace(&mut self.oam_searched, Vec::with_capacity(0));
                    let oams = orig_oams.drain(..);
                    let mut oams = oams.peekable();

                    let (_, true_y) = self.get_bg_true(0, self.ly);
                    self.ppu.load_line((0, PixelType::BG));
                    for x in 0..(160 / 8) + 1 {
                        self.ppu.load_line(
                            self.tile_color(
                                self.get_screen_bg_tile(x * 8, self.ly),
                                true_y % 8,
                                PixelType::BG)
                        );
                        assert_eq!(self.ppu.need_data(), false);
                        for sub_x in 0..8 {
                            'oams_done : while oams.peek().is_some() {
                                if let Some(cur) = oams.peek() {
                                    if self.oam[*cur].x  == x * 8 + sub_x {
                                        self.ppu.load_line(
                                            self.tile_color(
                                                self.oam[*cur].pattern,
                                                self.ly + 16 - self.oam[*cur].y,
                                                PixelType::Sprite)
                                        );
                                    } else {
                                        break 'oams_done
                                    }
                                }
                                oams.next();
                            }
                            if x == 0 {
                                self.ppu.shift();
                            } else {
                                let color: (u8, u8, u8, u8) = match self.ppu.shift() {
                                    Palette::Empty => self.bgp_shade(0),
                                    Palette::Low(_) => self.bgp_shade(1),
                                    Palette::Mid(_) => self.bgp_shade(2),
                                    Palette::High(_) => self.bgp_shade(3),
                                };
                                self.rendered
                                    .push((color, ((x * 8 + sub_x - 8) as i32, self.ly as i32)));
                                //println!("{:?}", self.rendered[self.rendered.len() - 1])
                            }
                        }
                        assert_eq!(self.ppu.need_data(), true);
                        //std::mem::replace(&mut orig_oams, self.oam_searched);
                    }
                    self.ppu.clear();
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
                self.ly = 0;
                self.unused_cycles = 0;
                if self.lcdc & 0x80 != 0 {
                    DisplayState::OAMSearch
                } else {
                    self.state
                }
            }
        };

        let mut triggers = 0;

        if next_state != self.state {
            let state_trig = flag_u8!(
                StatFlag::OAMInterrupt,
                next_state == DisplayState::OAMSearch
            )
                | flag_u8!(
                    StatFlag::VBlankInterrupt,
                    next_state == DisplayState::VBlank
                )
                | flag_u8!(
                    StatFlag::HBlankInterrupt,
                    next_state == DisplayState::HBlank
                );
            self.state = next_state;
            triggers |= state_trig;
        };

        triggers |= flag_u8!(StatFlag::CoincidenceInterrupt, self.ly == self.lyc);

        // always let vblank through
        triggers &= self.stat | mask_u8!(StatFlag::VBlankInterrupt);

        self.stat &= !0b111;
        self.stat |= match self.state {
            DisplayState::OAMSearch => StatFlag::OAM,
            DisplayState::VBlank => StatFlag::VBlank,
            DisplayState::HBlank => StatFlag::HBlank,
            DisplayState::PixelTransfer => StatFlag::PixelTransfer,
            // Pretend we are in vblank when screen is off, same invariants
            _ => StatFlag::VBlank,
        } as u8 & 0b11;
        self.stat |= if self.ly == self.lyc {
            mask_u8!(StatFlag::Coincidence)
        } else {
            0
        };

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
