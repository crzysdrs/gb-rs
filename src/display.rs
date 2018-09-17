use cpu::InterruptFlag;
use itertools::Itertools;
use peripherals::{Addressable, Peripheral, PeripheralData};
use std::collections::VecDeque;

pub const SCREEN_X: usize = 160;
pub const SCREEN_Y: usize = 144;
pub const BYTES_PER_PIXEL: usize = 4;

#[derive(PartialEq, Copy, Clone)]
enum DisplayState {
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

enum LCDCFlag {
    BGDisplayPriority = 1 << 0,
    SpriteDisplayEnable = 1 << 1,
    SpriteSize = 1 << 2,
    BGTileMapSelect = 1 << 3,
    BGWinTileDataSelect = 1 << 4,
    WindowEnable = 1 << 5,
    WindowTileMapDisplaySelect = 1 << 6,
    LCDDisplayEnable = 1 << 7,
}

enum OAMFlag {
    PaletteNumber = 1 << 4,
    FlipX = 1 << 5,
    FlipY = 1 << 6,
    Priority = 1 << 7,
}

#[derive(Copy, Clone)]
pub struct SpriteAttribute {
    y: u8,
    x: u8,
    pattern: SpriteIdx,
    flags: u8,
}

pub struct Display {
    vram: [u8; 8 << 10],
    oam: [SpriteAttribute; 40],
    oam_searched: Vec<SpriteAttribute>,
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
    unused_cycles: u64,
    state: DisplayState,
    changed_state: bool,
}

// pub trait LCD<C, P> {
//     fn draw_point(&mut self, c: C, point: P);
//     fn screen_power(&mut self, on: bool);
//     fn draw_line(&mut self, start: P, c: &Vec<u8>);
// }

// impl<'a> LCD<sdl2::pixels::Color, sdl2::rect::Point> for &'a mut [u8] {
//     fn draw_line(&mut self, start: sdl2::rect::Point, c: &Vec<u8>) {
//         let start = (start.x as usize + start.y as usize * SCREEN_X) * BYTES_PER_PIXEL as usize;
//         let end = start + SCREEN_X * BYTES_PER_PIXEL;
//         if c.len() > 0 {
//             assert_eq!(c.len(), SCREEN_X * BYTES_PER_PIXEL);
//             self[start..end].copy_from_slice(c[..].as_ref());
//         }
//     }
//     fn draw_point(&mut self, c: sdl2::pixels::Color, point: sdl2::rect::Point) {
//         let start = (point.x as usize + point.y as usize * SCREEN_X) as usize;
//         self[start..start + BYTES_PER_PIXEL].copy_from_slice(&[c.r, c.g, c.b, c.a]);
//     }
//     fn screen_power(&mut self, on: bool) {
//         if !on {
//             self[..].copy_from_slice(&[0xff; SCREEN_X * SCREEN_Y * BYTES_PER_PIXEL]);
//         }
//     }
// }

// impl<'a> LCD<sdl2::pixels::Color, sdl2::rect::Point> for sdl2::render::Texture<'a> {
//     fn draw_line(&mut self, start: sdl2::rect::Point, c: &Vec<u8>) {
//         if c.len() > 0 {
//             self.update(
//                 Some(sdl2::rect::Rect::new(
//                     start.x,
//                     start.y,
//                     (c.len() / 4) as u32,
//                     1,
//                 )),
//                 c.as_ref(),
//                 c.len(),
//             ).unwrap();
//         }
//     }
//     fn draw_point(&mut self, c: sdl2::pixels::Color, point: sdl2::rect::Point) {
//         self.update(
//             Some(sdl2::rect::Rect::new(point.x, point.y, 1, 1)),
//             &[c.r, c.g, c.b, c.a],
//             4,
//         ).unwrap();
//     }
//     fn screen_power(&mut self, on: bool) {
//         if !on {
//             let c: u8 = 0xff;

//             self.update(
//                 Some(sdl2::rect::Rect::new(0, 0, 160, 144)),
//                 &[c; 160 * 144 * 4],
//                 4,
//             ).unwrap();
//         }
//     }
// }

impl Display {
    pub fn new() -> Display {
        Display {
            changed_state: false,
            vram: [0; 8 << 10],
            oam: [SpriteAttribute {
                x: 0,
                y: 0,
                flags: 0,
                pattern: SpriteIdx(0),
            }; 40],
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
            state: DisplayState::OAMSearch,
            unused_cycles: 0,
        }
    }
    pub fn oam_lookup(&self, idx: OAMIdx) -> Option<&SpriteAttribute> {
        let idx = idx.0 as usize;
        if idx > self.oam.len() {
            None
        } else {
            Some(&self.oam[idx])
        }
    }
    pub fn display_enabled(&self) -> bool {
        self.lcdc & mask_u8!(LCDCFlag::LCDDisplayEnable) != 0
    }
    // pub fn render<C: From<(u8, u8, u8, u8)>, P: From<(i32, i32)>>(
    //     &mut self,
    //     lcd: &mut Option<&mut LCD<C, P>>,
    // ) {
    //     if lcd.is_none() {
    //         /* do nothing */
    //     } else if let Some(lcd) = lcd {
    //         if self.changed_state && self.state == DisplayState::VBlank && !self.display_enabled() {
    //             //lcd.screen_power(false);
    //         } else {
    //             lcd.draw_line((0, self.ly as i32).into(), &mut self.rendered);
    //         }
    //     }
    //     self.rendered.clear();
    // }
    fn sprite_size(&self) -> u8 {
        if self.lcdc & mask_u8!(LCDCFlag::SpriteSize) == 0 {
            8
        } else {
            16
        }
    }
    fn oam_search(&mut self) {
        assert_eq!(self.oam_searched.capacity(), 10);
        self.oam_searched.clear();
        if self.lcdc & mask_u8!(LCDCFlag::SpriteDisplayEnable) != 0 {
            self.oam_searched.extend(
                self.oam
                    .iter()
                    .filter(
                        /* ignored invisible sprites */
                        |oam| oam.x != 0 && oam.x < 168 && oam.y != 0 && oam.y < 144 + 16,
                    ).filter(/* filter only items in this row */ |oam| {
                        self.ly + 16 >= oam.y && self.ly + 16 - oam.y < self.sprite_size()
                    }).sorted_by_key(|oam| oam.x)
                    .into_iter()
                    .take(10)
                    .map(|x| *x),
            );
        }
    }

    fn get_bg_true(&self, x: u8, y: u8) -> (u8, u8) {
        let true_x = self.scx.wrapping_add(x);
        let true_y = self.scy.wrapping_add(y);
        (true_x, true_y)
    }
    fn get_screen_bg_tile(&self, x: u8, y: u8) -> Tile {
        let (true_x, true_y) = self.get_bg_true(x, y);
        let tile_x = true_x / 8;
        let tile_y = true_y / 8;
        Tile::BG(self.get_bg_tile(tile_x, tile_y), Coord(0, true_y % 8))
    }

    fn get_bg_tile(&self, true_x: u8, true_y: u8) -> BGIdx {
        let bg_map = if self.lcdc & mask_u8!(LCDCFlag::BGTileMapSelect) == 0 {
            0x1800
        } else {
            0x1C00
        };
        let idx = bg_map + true_y as u16 * 32 + true_x as u16;
        BGIdx(self.vram[idx as usize])
    }
    fn get_win_tile(&self, x: u8) -> Tile {
        let x = x.wrapping_sub(self.wx.wrapping_sub(7));
        let y = self.ly - self.wy;
        let win_map = if self.lcdc & mask_u8!(LCDCFlag::WindowTileMapDisplaySelect) == 0 {
            0x1800u16
        } else {
            0x1c00u16
        };
        let idx = win_map + (y / 8) as u16 * 32 + (x / 8) as u16;
        Tile::Window(BGIdx(self.vram[idx as usize]), Coord(0, y % 8))
    }

    #[cfg(test)]
    pub fn all_bgs(&self) -> [u8; 1024] {
        let mut bgs = [0u8; 1024];

        for y in 0..32 {
            for x in 0..32 {
                bgs[y as usize * 32 + x as usize] = self.get_bg_tile(x, y).0;
            }
        }
        bgs
    }
    pub fn dump(&mut self) {
        println!("BG Tile Map");
        for y in 0..32 {
            for x in 0..32 {
                let bgidx = self.get_bg_tile(x, y);
                print!("{:02x} ", bgidx.0);
            }
            println!("");
        }

        for t in 0..=0x20 {
            println!("Tile {}", t);
            let idx = BGIdx(t);
            let mut ppu = PPU::new();
            for y in 0..8 {
                let t = Tile::BG(idx, Coord(0, y));
                ppu.load(&t, t.fetch(self));
                for _x in 0..8 {
                    let c = match ppu.shift() {
                        Pixel(_, PaletteShade::Empty) => 0,
                        Pixel(_, PaletteShade::Low) => 1,
                        Pixel(_, PaletteShade::Mid) => 2,
                        Pixel(_, PaletteShade::High) => 3,
                    };
                    print!("{} ", c)
                }
                println!("");
            }
        }
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match addr {
            0x8000...0x9fff => &mut self.vram[(addr - 0x8000) as usize],
            0xFE00...0xFE9F => {
                let idx = ((addr & 0xff) >> 2) as usize;
                let oam = &mut self.oam[idx];
                match addr & 0b11 {
                    0b00 => &mut oam.y,
                    0b01 => &mut oam.x,
                    0b10 => &mut oam.pattern.0,
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

    fn bgp_shade(&self, p: Pixel) -> (u8, u8, u8, u8) {
        let white = (0xff, 0xff, 0xff, 0xff);
        let dark_grey = (0xaa, 0xaa, 0xaa, 0xff);
        let light_grey = (0x55, 0x55, 0x55, 0xff);
        let black = (0x00, 0x00, 0x00, 0xff);
        let pal = if self.display_enabled() {
            match p {
                Pixel(Palette::BG, _) => {
                    if self.lcdc & mask_u8!(LCDCFlag::BGDisplayPriority) == 0 {
                        0
                    } else {
                        self.bgp
                    }
                }
                Pixel(Palette::OBP0, _) => self.obp0,
                Pixel(Palette::OBP1, _) => self.obp1,
            }
        } else {
            0
        };

        let shade_id = match p {
            Pixel(_, PaletteShade::High) => 3,
            Pixel(_, PaletteShade::Mid) => 2,
            Pixel(_, PaletteShade::Low) => 1,
            Pixel(_, PaletteShade::Empty) => 0,
        };
        match (pal >> (shade_id * 2)) & 0b11 {
            0b00 => white,
            0b01 => dark_grey,
            0b10 => light_grey,
            0b11 => black,
            _ => panic!("shade out of bounds"),
        }
    }

    fn add_oams<'sprite, T: Iterator<Item = &'sprite SpriteAttribute>>(
        &mut self,
        oams: &mut std::iter::Peekable<T>,
        x: u8,
        y: u8,
    ) {
        'oams_done: while oams.peek().is_some() {
            let use_oam = if let Some(cur) = oams.peek() {
                cur.x <= x
            } else {
                false
            };
            if !use_oam {
                break 'oams_done;
            }
            let oam = oams.next().unwrap();
            if oam.x == x {
                let t = Tile::Sprite(*oam, Coord(0, y + 16 - oam.y));
                let l = t.fetch(self);
                self.ppu.load(&t, l);
            }
        }
    }

    fn draw_window<'sprite, T: Iterator<Item = &'sprite SpriteAttribute>>(
        &mut self,
        lcd_line: &mut [u8],
        oams: &mut std::iter::Peekable<T>,
        window: bool,
        bg_offset: u8,
        range: &mut std::ops::Range<u8>,
    ) {
        /* offscreen pixels */
        if std::ops::Range::is_empty(range) {
            return;
        }
        let fake = Tile::BG(BGIdx(0), Coord(0, 0));
        let l = fake.fetch(self);
        const IGNORED_OFFSET: usize = 8;
        self.ppu.load(&fake, l);
        for _x in 0..bg_offset {
            self.ppu.shift();
        }
        let mut target_pixel = 0;
        for x in range.start..range.end + IGNORED_OFFSET as u8 {
            if self.ppu.need_data() {
                let t = if window {
                    self.get_win_tile(x)
                } else {
                    self.get_screen_bg_tile(x, self.ly)
                };
                let l = t.fetch(self);
                self.ppu.load(&t, l);
            }
            assert_eq!(self.ppu.need_data(), false);
            self.add_oams(oams, x, self.ly);

            if x >= range.start + IGNORED_OFFSET as u8 {
                let c: (u8, u8, u8, u8) = {
                    let p = self.ppu.shift();
                    self.bgp_shade(p)
                };
                let start = target_pixel * BYTES_PER_PIXEL;
                lcd_line[start..start + BYTES_PER_PIXEL].copy_from_slice(&[c.0, c.1, c.2, c.3]);
                target_pixel += 1;
            } else {
                self.ppu.shift();
            }
        }
        self.ppu.clear();
    }
}

#[derive(Copy, Clone)]
struct SpriteIdx(u8);
#[derive(Copy, Clone)]
struct BGIdx(u8);
#[derive(Copy, Clone)]
struct TileIdx(u8);
#[derive(Copy, Clone)]
pub struct OAMIdx(u8);
#[derive(Copy, Clone)]
struct Coord(u8, u8);

impl Coord {
    // fn x(&self) -> u8 {
    //     self.0
    // }
    fn y(&self) -> u8 {
        self.1
    }
}

#[derive(Clone)]
enum Tile {
    BG(BGIdx, Coord),
    Window(BGIdx, Coord),
    Sprite(SpriteAttribute, Coord),
}

impl Tile {
    #[allow(dead_code)]
    pub fn show(&self, display: &mut Display) {
        let mut tmp: Tile = self.to_owned();
        for i in 0..display.sprite_size() {
            let c: &mut Coord = match tmp {
                Tile::BG(_, ref mut c) => c,
                Tile::Window(_, ref mut c) => c,
                Tile::Sprite(_, ref mut c) => c,
            };
            *c = Coord(0, i);
            let (_, _, line) = tmp.fetch(display);
            for x in 0..8 {
                let num = match Tile::line_palette(line, x) {
                    PaletteShade::High => 3,
                    PaletteShade::Mid => 2,
                    PaletteShade::Low => 1,
                    PaletteShade::Empty => 0,
                };
                print!("{} ", num);
            }
            println!();
        }
    }
    pub fn line_palette(line: u16, x: usize) -> PaletteShade {
        let hi = 0x8000;
        let lo = 0x0080;
        let t = (((line << x) & hi) >> 14) | (((line << x) & lo) >> 7);

        match t {
            0b11 => PaletteShade::High,
            0b10 => PaletteShade::Mid,
            0b01 => PaletteShade::Low,
            0b00 => PaletteShade::Empty,
            _ => unreachable!(),
        }
    }

    pub fn fetch(&self, display: &mut Display) -> (bool, Palette, u16) {
        let (start, line_offset) = match *self {
            Tile::Window(idx, coord) | Tile::BG(idx, coord) => {
                let bytes_per_tile = 16;
                let start = if display.lcdc & mask_u8!(LCDCFlag::BGWinTileDataSelect) == 0 {
                    /* signed tile idx */
                    (0x1000 + idx.0 as i8 as i16 * bytes_per_tile) as u16
                } else {
                    /*unsigned tile_idx */
                    0 + idx.0 as u16 * bytes_per_tile as u16
                };
                (start as usize, (coord.y() % 8) as usize)
            }
            Tile::Sprite(oam, coord) => {
                let bytes_per_line = 2;
                let idx = if display.sprite_size() == 16 {
                    oam.pattern.0 >> 1
                } else {
                    oam.pattern.0
                };
                let start = idx as u16 * bytes_per_line * display.sprite_size() as u16;
                let y = if oam.flags & mask_u8!(OAMFlag::FlipY) != 0 {
                    display.sprite_size() - 1 - coord.y()
                } else {
                    coord.y()
                };
                (start as usize, y as usize)
            }
        };

        let b1 = display.vram[start + (line_offset * 2)];
        let b2 = display.vram[start + (line_offset * 2) + 1];

        let line = match *self {
            Tile::Sprite(oam, _) => {
                if oam.flags & mask_u8!(OAMFlag::FlipX) != 0 {
                    u16::from_le_bytes([b1.reverse_bits(), b2.reverse_bits()])
                } else {
                    u16::from_le_bytes([b1, b2])
                }
            }
            _ => u16::from_le_bytes([b1, b2]),
        };

        let (priority, palette) = match *self {
            Tile::Sprite(oam, _) => {
                let priority = oam.flags & mask_u8!(OAMFlag::Priority) == 0;
                let palette = if oam.flags & mask_u8!(OAMFlag::PaletteNumber) == 0 {
                    Palette::OBP0
                } else {
                    Palette::OBP1
                };
                (priority, palette)
            }
            Tile::BG(_, _) | Tile::Window(_, _) => (false, Palette::BG),
        };
        (priority, palette, line)
    }
}

#[derive(PartialEq, Copy, Clone)]
enum PaletteShade {
    Empty,
    Low,
    Mid,
    High,
}
#[derive(Copy, Clone)]
enum Palette {
    BG,
    OBP0,
    OBP1,
}

struct Pixel(Palette, PaletteShade);

struct PPU {
    shift: VecDeque<Pixel>,
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            shift: VecDeque::with_capacity(16),
        }
    }
    fn load(&mut self, _t: &Tile, (priority, palette, line): (bool, Palette, u16)) {
        for x in 0..8 {
            let p = Tile::line_palette(line, x);

            match palette {
                Palette::OBP1 | Palette::OBP0 => {
                    if p != PaletteShade::Empty {
                        match self.shift[x] {
                            Pixel(Palette::BG, _) if priority => {
                                self.shift[x] = Pixel(palette, p);
                            }
                            Pixel(Palette::BG, PaletteShade::Empty) => {
                                self.shift[x] = Pixel(palette, p);
                            }
                            _ => { /* existing non-background data */ }
                        }
                    }
                }
                Palette::BG => {
                    self.shift.push_back(Pixel(palette, p));
                }
            }
        }
    }
    fn shift(&mut self) -> Pixel {
        self.shift.pop_front().unwrap()
    }
    fn need_data(&self) -> bool {
        self.shift.len() <= 8
    }
    fn clear(&mut self) {
        self.shift.clear()
    }
}

impl Addressable for Display {
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        *self.lookup(addr) = v;
    }
}
impl Peripheral for Display {
    fn step(&mut self, real: &mut PeripheralData, time: u64) -> Option<InterruptFlag> {
        let mut new_ly = self.ly;
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
                    self.ppu.clear();
                    let orig_oams =
                        std::mem::replace(&mut self.oam_searched, Vec::with_capacity(0));
                    {
                        let (true_x, _true_y) = self.get_bg_true(0, self.ly);
                        let has_window =
                            self.lcdc & mask_u8!(LCDCFlag::WindowEnable) != 0 && self.ly >= self.wy;
                        let bg_split = if has_window {
                            std::cmp::min(std::cmp::max(7, self.wx) - 7, SCREEN_X as u8)
                        } else {
                            SCREEN_X as u8
                        };
                        let mut split_line = real.lcd.as_mut().map(|lcd| {
                            let y = self.ly as usize;
                            let line_start = SCREEN_X * y * BYTES_PER_PIXEL;
                            let line_end = SCREEN_X * (y + 1) * BYTES_PER_PIXEL;
                            let (l, r) = lcd[line_start..line_end]
                                .split_at_mut(bg_split as usize * BYTES_PER_PIXEL);
                            [
                                (l, true_x % 8, 0..bg_split, false),
                                (r, 0, bg_split..SCREEN_X as u8, true),
                            ]
                        });
                        split_line.as_mut().map(|windows| {
                            for w in windows {
                                let (line, offset, range, is_win) = w;
                                let mut oams = orig_oams.iter().peekable();
                                self.draw_window(line, &mut oams, *is_win, *offset, range);
                            }
                        });

                        self.ppu.clear();
                        self.unused_cycles -= 43;
                    }
                    std::mem::replace(&mut self.oam_searched, orig_oams);
                    DisplayState::HBlank
                } else {
                    self.state
                }
            }
            DisplayState::HBlank => {
                if self.unused_cycles >= 51 {
                    /* do work */
                    self.unused_cycles -= 51;
                    new_ly += 1;
                    if new_ly == 144 {
                        DisplayState::VBlank
                    } else {
                        DisplayState::OAMSearch
                    }
                } else {
                    self.state
                }
            }
            DisplayState::VBlank => {
                if self.unused_cycles >= (43 + 51 + 20) {
                    /* do work */
                    self.unused_cycles -= 43 + 51 + 20;
                    new_ly += 1;
                    if new_ly == 153 {
                        new_ly = 0;
                        DisplayState::OAMSearch
                    } else {
                        self.state
                    }
                } else {
                    self.state
                }
            }
        };

        let mut triggers = 0;

        //TODO: Determine which interrupts actually fire during Display Off
        // At the very least, VBLANK does not.
        self.changed_state = next_state != self.state;
        if next_state != self.state {
            let state_trig = flag_u8!(
                StatFlag::OAMInterrupt,
                next_state == DisplayState::OAMSearch
            ) | flag_u8!(
                StatFlag::VBlankInterrupt,
                next_state == DisplayState::VBlank
            ) | flag_u8!(
                StatFlag::HBlankInterrupt,
                next_state == DisplayState::HBlank
            );
            self.state = next_state;
            triggers |= state_trig;
        };

        if new_ly != self.ly {
            self.ly = new_ly;
            triggers |= flag_u8!(StatFlag::CoincidenceInterrupt, self.ly == self.lyc);
        }

        // always let vblank through
        triggers &= self.stat | mask_u8!(StatFlag::VBlankInterrupt);

        self.stat &= !0b111;
        self.stat |= match self.state {
            DisplayState::OAMSearch => StatFlag::OAM,
            DisplayState::VBlank => StatFlag::VBlank,
            DisplayState::HBlank => StatFlag::HBlank,
            DisplayState::PixelTransfer => StatFlag::PixelTransfer,
        } as u8
            & 0b11;
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
