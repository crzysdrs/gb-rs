use peripherals::{Peripheral};

enum DisplayState {
    OAMSearch, //20 Clocks
    PixelTransfer, //43 + Clocks
    HBlank, //51 Clocks
    VBlank, //(20 + 43 + 51) * 10
}

pub struct Display {
    vram : [u8; 8 << 10],
    oam : [u8; 4 * 40],
    scx : u8,
    scy : u8,
    lcdc : u8,
    stat : u8,
    ly : u8,
    lyc : u8,
    bgp : u8,
    obp0: u8,
    obp1 : u8,
    wy : u8,
    wx : u8,

    ppu : [u8; 16],
    rendered : Vec<((u8, u8, u8, u8), (i32, i32))>,
    unused_cycles : u64,
    state : DisplayState,
}

pub trait LCD<C,P>{
    fn draw_point(&mut self, c: C, point: P);
}

impl <T> LCD<sdl2::pixels::Color, sdl2::rect::Point>  for sdl2::render::Canvas<T> where T: sdl2::render::RenderTarget {
    fn draw_point (&mut self, c: sdl2::pixels::Color, point: sdl2::rect::Point) {
        self.set_draw_color(c);
        self.draw_point(point);

    }
}

impl Display {
    pub fn new() -> Display {
        Display {
            vram : [0; 8 << 10],
            oam : [0; 4 * 40],
            scx : 0,
            scy : 0,
            lcdc : 0,
            stat : 0,
            ly : 0,
            lyc : 0,
            bgp : 0,
            obp0 : 0,
            obp1 : 0,
            wy : 0,
            wx : 0,
            ppu : [0u8; 16],
            rendered : Vec::new(),
            state : DisplayState::OAMSearch,
            unused_cycles: 0,
        }
    }
    pub fn render<C : From<(u8, u8, u8, u8)>,P : From<(i32, i32)>>(&mut self, lcd : &mut Option<&mut LCD<C,P>>) {
        let print = std::mem::replace(&mut self.rendered, Vec::new());
        if let Some(lcd) = lcd {
            for (c, p) in print.into_iter() {
                //println!("Drawing Point {:?} {:?}", c, p);
                lcd.draw_point(c.into(), p.into());
            }
        }
    }
    fn tile_color(&mut self, x: u8) -> (u8, u8) {
        let true_x = self.wx.wrapping_add(x.wrapping_add(self.scx) % 160);
        let true_y = self.wy.wrapping_add(self.ly.wrapping_add(self.scy) % 144);
        //println!("X: {}, Y: {}, True X: {}, True Y: {}", x, self.ly, true_x, true_y);
        let tile_idx = self.get_bg_tile(true_x / 8, true_y / 8);

        self.tile_8_8(tile_idx, true_y % 8)
    }
    fn tile_offset(&mut self, t :u8) -> u16 {
        t as u16 * 16
    }

    fn tile_8_8(&mut self, t : u8, y : u8) -> (u8, u8)
    {
        let t_off :usize = self.tile_offset(t) as usize;
        let line_off = y as usize * 2;

        (self.vram[t_off + line_off], self.vram[t_off + line_off + 1])
    }

    fn bit_color(c_hi : u8, c_lo : u8) -> u8 {
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
                for x in 0..8 {
                    print!("{} ", Display::bit_color(c_hi, c_lo));
                    c_hi <<= 1;
                    c_lo <<= 1;
                }
                println!("");
            }
        }
    }
}


impl Peripheral for Display
{
    fn lookup(&mut self, addr : u16) -> &mut u8 {
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
            _ => panic!("unhandled address in display {:x}", addr)
        }
    }

    fn step(&mut self, time : u64) {
        self.unused_cycles += time;

        match self.state {
            DisplayState::OAMSearch => {
                self.stat &= !0b11;
                self.stat |= 0b10;

                if self.unused_cycles >= 20 {
                    /* do work */
                    self.unused_cycles -= 20;
                    self.state = DisplayState::PixelTransfer;
                }
            },
            DisplayState::PixelTransfer => {
                self.stat &= !0b11;
                self.stat |= 0b11;

                if self.unused_cycles >= 43 {
                    /* do work */
                    for x in 0..(160 / 8) {
                        let (mut c_hi, mut c_lo) = self.tile_color(x * 8);
                        for sub_x in 0..8 {
                            let color : (u8, u8, u8, u8) = match Display::bit_color(c_hi, c_lo) {
                                0b00 => (0xff, 0xff, 0xff, 0xff),
                                0b01 => (0, 0, 0, 0xff),
                                0b10 => (0, 0, 0, 0xff),
                                0b11 => (0, 0, 0, 0xff),
                                c => panic!("invalid pixel color {:b}", c)
                            };
                            c_hi <<= 1;
                            c_lo <<= 1;
                            self.rendered.push((color , ((x * 8 + sub_x) as i32, self.ly as i32)));
                            //println!("{:?}", self.rendered[self.rendered.len() - 1])
                        }
                    }
                    self.unused_cycles -= 43;
                    self.state = DisplayState::HBlank;
                }
            },
            DisplayState::HBlank => {
                self.stat &= !0b11;
                self.stat |= 0b00;

                if self.unused_cycles >= 51 {
                    /* do work */
                    self.unused_cycles -= 51;
                    self.ly += 1;
                    if self.ly == 144 {
                        self.state = DisplayState::VBlank;
                    } else {
                        self.state = DisplayState::OAMSearch;
                    }
                }
            },
            DisplayState::VBlank => {
                self.stat &= !0b11;
                self.stat |= 0b01;

                if self.unused_cycles >= (43 + 51 + 20) {
                    /* do work */
                    self.unused_cycles -= (43 + 51 + 20);
                    self.ly += 1;
                    if self.ly == 153 {
                        self.state = DisplayState::OAMSearch;
                        self.ly = 0;
                    }
                }
            },
        }

        if self.ly == self.lyc {
            self.stat |= 1 << 2;
        } else {
            self.stat &= 1 << 2;
        }

    }
}
