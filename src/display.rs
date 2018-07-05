use cpu::InterruptFlag;
use itertools::Itertools;
use peripherals::Peripheral;
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

enum LCDCFlag {
    //BGDisplayPriority = 1 << 0,
    //SpriteDisplayEnable = 1 << 1,
    SpriteSize = 1 << 2,
    BGTileMapSelect = 1 << 3,
    BGWinTileDataSelect = 1 << 4,
    //WindowEnable = 1 << 5,
    //WindowTileMapDisplaySelect = 1 << 6,
    //LCDDisplayEnable = 1 << 7,
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
    oam_searched: Vec<usize>,
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
            rendered: Vec::with_capacity(160),
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
    fn sprite_size(&self) -> u8 {
        if self.lcdc & mask_u8!(LCDCFlag::SpriteSize) == 0 {
            8
        } else {
            16
        }
    }
    fn oam_search(&mut self) {
        let (idxs, _): (Vec<usize>, Vec<&SpriteAttribute>) =
            self.oam
                .iter()
                .enumerate()
                .filter(
                    /* ignored invisible sprites */
                    |(_i, oam)| oam.y != 0 && oam.y < 144 + 16,
                )
                .filter(/* filter only items in this row */ |(_i, oam)| {
                    self.ly + 16 >= oam.y && self.ly + 16 - oam.y < self.sprite_size()
                })
                .sorted_by_key(|(_i, oam)| oam.x)
                .into_iter()
                .take(10)
                .unzip();
        self.oam_searched = idxs;
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
        let pal = match p {
            Pixel(Palette::BG, _) => self.bgp,
            Pixel(Palette::OBP0, _) => self.obp0,
            Pixel(Palette::OBP1, _) => self.obp1,
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

    fn add_oams<T: Iterator<Item = usize>>(
        &mut self,
        oams: &mut std::iter::Peekable<T>,
        x: u8,
        y: u8,
    ) {
        'oams_done: while oams.peek().is_some() {
            if let Some(cur) = oams.peek() {
                if self.oam[*cur].x == x {
                    let t = Tile::Sprite(OAMIdx(*cur as u8), Coord(0, y + 16 - self.oam[*cur].y));
                    let l = t.fetch(self);
                    self.ppu.load(&t, l);
                } else {
                    break 'oams_done;
                }
            }
            oams.next();
        }
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

enum Tile {
    BG(BGIdx, Coord),
    Sprite(OAMIdx, Coord),
}

impl Tile {
    pub fn fetch(&self, display: &mut Display) -> (bool, Palette, u16) {
        let (start, line_offset) = match *self {
            Tile::BG(idx, coord) => {
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
            Tile::Sprite(oamidx, coord) => {
                let bytes_per_line = 2;
                let attrib = display
                    .oam_lookup(oamidx)
                    .expect("Only valid OAM addresses");
                let idx = attrib.pattern;
                let start = idx.0 as u16 * bytes_per_line * display.sprite_size() as u16;
                let y = if attrib.flags & mask_u8!(OAMFlag::FlipY) != 0 {
                    display.sprite_size() - coord.y()
                } else {
                    coord.y()
                };
                (start as usize, y as usize)
            }
        };

        let b1 = display.vram[start + (line_offset * 2)];
        let b2 = display.vram[start + (line_offset * 2) + 1];

        let line = match *self {
            Tile::Sprite(oamidx, _) => {
                let attrib = display
                    .oam_lookup(oamidx)
                    .expect("Only valid OAM addresses");
                if attrib.flags & mask_u8!(OAMFlag::FlipX) != 0 {
                    u16::from_bytes([b1.reverse_bits(), b2.reverse_bits()])
                } else {
                    u16::from_bytes([b1, b2])
                }
            }
            _ => u16::from_bytes([b1, b2]),
        };

        let (priority, palette) = match *self {
            Tile::Sprite(oamidx, _) => {
                let attrib = display
                    .oam_lookup(oamidx)
                    .expect("Only valid OAM addresses");
                let priority = attrib.flags & mask_u8!(OAMFlag::Priority) == 0;
                let palette = if attrib.flags & mask_u8!(OAMFlag::PaletteNumber) == 0 {
                    Palette::OBP0
                } else {
                    Palette::OBP1
                };
                (priority, palette)
            }
            Tile::BG(_, _) => (false, Palette::BG),
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
    fn load(&mut self, _t: &Tile, (priority, palette, mut line): (bool, Palette, u16)) {
        for x in 0..8 {
            let p = match (line & 0x8000 != 0, line & 0x80 != 0) {
                (true, true) => PaletteShade::High,
                (true, false) => PaletteShade::Mid,
                (false, true) => PaletteShade::Low,
                (false, false) => PaletteShade::Empty,
            };

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
            line <<= 1;
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
                    let mut orig_oams =
                        std::mem::replace(&mut self.oam_searched, Vec::with_capacity(0));
                    let oams = orig_oams.drain(..);
                    let mut oams = oams.peekable();

                    let (true_x, _true_y) = self.get_bg_true(0, self.ly);

                    /* offscreen pixels */
                    let fake = Tile::BG(BGIdx(0), Coord(0, 0));
                    let l = fake.fetch(self);
                    self.ppu.load(&fake, l);

                    // for x in 0..8 - true_x % 8 {
                    //     self.ppu.shift();
                    //     self.add_oams(&mut oams, x, self.ly);
                    //}
                    for _x in 0..(true_x % 8) {
                        self.ppu.shift();
                    }
                    //assert_eq!(true_x % 8, 0);

                    for x in 0..(160 + 8) {
                        if self.ppu.need_data() {
                            let t = self.get_screen_bg_tile(x, self.ly);
                            let l = t.fetch(self);
                            self.ppu.load(&t, l);
                        }
                        assert_eq!(self.ppu.need_data(), false);
                        self.add_oams(&mut oams, x, self.ly);

                        if x >= 8 {
                            let color: (u8, u8, u8, u8) = {
                                let p = self.ppu.shift();
                                self.bgp_shade(p)
                            };
                            self.rendered
                                .push((color, (((x - 8) as i32, self.ly as i32))));
                        } else {
                            self.ppu.shift();
                        }
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
