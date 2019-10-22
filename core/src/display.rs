use crate::cart::CGBStatus;
use crate::cpu::Interrupt;
use crate::cycles;
use crate::mmu::MemRegister;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use itertools::Itertools;
use modular_bitfield::prelude::*;
use std::collections::VecDeque;
use std::convert::TryFrom;

pub const SCREEN_X: usize = 160;
pub const SCREEN_Y: usize = 144;
pub const BYTES_PER_PIXEL: usize = 4;

pub const PALETTE_COLORS: [[u8; 3]; 76] = [
    //[r, g, b]
    [0x00, 0x00, 0x00],
    [0x00, 0x00, 0xFF],
    [0x00, 0x3A, 0x3A],
    [0x00, 0x4A, 0x00],
    [0x00, 0x63, 0x00],
    [0x00, 0x63, 0xC5],
    [0x00, 0x84, 0x00],
    [0x00, 0x84, 0x84],
    [0x00, 0x84, 0xFF],
    [0x00, 0xFF, 0x00],
    [0x31, 0x84, 0x00],
    [0x3A, 0x29, 0x00],
    [0x42, 0x73, 0x7B],
    [0x4A, 0x00, 0x00],
    [0x52, 0x52, 0x52],
    [0x52, 0x52, 0x8C],
    [0x52, 0xDE, 0x00],
    [0x52, 0xFF, 0x00],
    [0x5A, 0x31, 0x08],
    [0x5A, 0x5A, 0x5A],
    [0x5A, 0xBD, 0xFF],
    [0x63, 0x00, 0x00],
    [0x63, 0x94, 0x73],
    [0x63, 0xA5, 0xFF],
    [0x63, 0xEF, 0xEF],
    [0x6B, 0x52, 0x31],
    [0x6B, 0xFF, 0x00],
    [0x7B, 0x4A, 0x00],
    [0x7B, 0xFF, 0x00],
    [0x7B, 0xFF, 0x31],
    [0x84, 0x31, 0x00],
    [0x84, 0x6B, 0x29],
    [0x8C, 0x8C, 0xDE],
    [0x94, 0x3A, 0x00],
    [0x94, 0x3A, 0x3A],
    [0x94, 0x42, 0x00],
    [0x94, 0x94, 0x94],
    [0x94, 0x94, 0xFF],
    [0x94, 0xB5, 0xFF],
    [0x9C, 0x63, 0x00],
    [0x9C, 0x84, 0x31],
    [0xA5, 0x84, 0x52],
    [0xA5, 0x9C, 0xFF],
    [0xA5, 0xA5, 0xA5],
    [0xAD, 0x5A, 0x42],
    [0xAD, 0xAD, 0x84],
    [0xB5, 0x73, 0x00],
    [0xB5, 0xB5, 0xFF],
    [0xCE, 0x9C, 0x84],
    [0xD6, 0x00, 0x00],
    [0xE6, 0x00, 0x00],
    [0xF7, 0xC5, 0xA5],
    [0xFF, 0x00, 0x00],
    [0xFF, 0x00, 0xFF],
    [0xFF, 0x42, 0x00],
    [0xFF, 0x52, 0x4A],
    [0xFF, 0x63, 0x52],
    [0xFF, 0x73, 0x00],
    [0xFF, 0x84, 0x00],
    [0xFF, 0x84, 0x84],
    [0xFF, 0x94, 0x94],
    [0xFF, 0x9C, 0x00],
    [0xFF, 0xAD, 0x63],
    [0xFF, 0xC5, 0x42],
    [0xFF, 0xCE, 0x00],
    [0xFF, 0xD6, 0x00],
    [0xFF, 0xDE, 0x00],
    [0xFF, 0xE6, 0xC5],
    [0xFF, 0xFF, 0x00],
    [0xFF, 0xFF, 0x3A],
    [0xFF, 0xFF, 0x7B],
    [0xFF, 0xFF, 0x94],
    [0xFF, 0xFF, 0x9C],
    [0xFF, 0xFF, 0xA5],
    [0xFF, 0xFF, 0xCE],
    [0xFF, 0xFF, 0xFF],
];
pub struct CustomPalette {
    bg: [u8; 4],
    obj0: [u8; 4],
    obj1: [u8; 4],
}

pub struct KeyPalette {
    pub keys: &'static str,
    palette: CustomPalette,
}
pub const KEY_PALETTES: [KeyPalette; 12] = [
    KeyPalette {
        keys: "Right",
        palette: CustomPalette {
            bg: [75, 17, 54, 0],
            obj0: [75, 17, 54, 0],
            obj1: [75, 17, 54, 0],
        },
    },
    KeyPalette {
        keys: "A + Down",
        palette: CustomPalette {
            bg: [75, 68, 52, 0],
            obj0: [75, 68, 52, 0],
            obj1: [75, 68, 52, 0],
        },
    },
    KeyPalette {
        keys: "Up",
        palette: CustomPalette {
            bg: [75, 62, 30, 0],
            obj0: [75, 62, 30, 0],
            obj1: [75, 62, 30, 0],
        },
    },
    KeyPalette {
        keys: "B + Right",
        palette: CustomPalette {
            bg: [0, 7, 66, 75],
            obj0: [0, 7, 66, 75],
            obj1: [0, 7, 66, 75],
        },
    },
    KeyPalette {
        keys: "B + Left",
        palette: CustomPalette {
            bg: [75, 43, 14, 0],
            obj0: [75, 43, 14, 0],
            obj1: [75, 43, 14, 0],
        },
    },
    KeyPalette {
        keys: "Down",
        palette: CustomPalette {
            bg: [73, 60, 37, 0],
            obj0: [73, 60, 37, 0],
            obj1: [73, 60, 37, 0],
        },
    },
    KeyPalette {
        keys: "B + Up",
        palette: CustomPalette {
            bg: [67, 48, 31, 18],
            obj0: [75, 62, 30, 0],
            obj1: [75, 62, 30, 0],
        },
    },
    KeyPalette {
        keys: "A + Right",
        palette: CustomPalette {
            bg: [75, 29, 5, 0],
            obj0: [75, 59, 34, 0],
            obj1: [75, 59, 34, 0],
        },
    },
    KeyPalette {
        keys: "A + Left",
        palette: CustomPalette {
            bg: [75, 32, 15, 0],
            obj0: [75, 59, 34, 0],
            obj1: [75, 62, 30, 0],
        },
    },
    KeyPalette {
        keys: "A + Up",
        palette: CustomPalette {
            bg: [75, 59, 34, 0],
            obj0: [75, 29, 6, 0],
            obj1: [75, 23, 1, 0],
        },
    },
    KeyPalette {
        keys: "Left",
        palette: CustomPalette {
            bg: [75, 23, 1, 0],
            obj0: [75, 59, 34, 0],
            obj1: [75, 29, 6, 0],
        },
    },
    KeyPalette {
        keys: "B + Down",
        palette: CustomPalette {
            bg: [75, 68, 27, 0],
            obj0: [75, 23, 1, 0],
            obj1: [75, 29, 6, 0],
        },
    },
];

#[derive(PartialEq, Copy, Clone, Debug)]
enum DisplayState {
    OAMSearch,     //20 Clocks
    PixelTransfer, //43 + Clocks
    HBlank,        //51 Clocks
    VBlank,        //(20 + 43 + 51) * 10
}

#[bitfield]
pub struct ColorEntry {
    r: B5,
    g: B5,
    b: B5,
    unused: B1,
}

#[derive(BitfieldSpecifier, Debug, PartialEq, Copy, Clone)]
pub enum StatMode {
    HBlank = 0b00,
    VBlank = 0b01,
    OAM = 0b10,
    PixelTransfer = 0b11,
}

#[bitfield]
pub struct StatFlag {
    #[bits = 2]
    mode: StatMode,
    coincidence_flag: bool,
    hblank: bool,
    vblank: bool,
    oam: bool,
    coincidence: bool,
    unused: B1,
}

#[bitfield]
pub struct LCDCControl {
    bg_display_priority: bool,
    sprite_display_enable: bool,
    sprite_size: bool,
    bg_tile_map_select: bool,
    bg_win_tile_data_select: bool,
    window_enable: bool,
    window_tile_map_display_select: bool,
    lcd_display_enable: bool,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct OAMFlag {
    color_palette: B3,
    vram_bank: B1,
    palette_number: B1,
    flip_x: bool,
    flip_y: bool,
    priority: bool,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct BGMapFlag {
    color_palette: B3,
    vram_bank: B1,
    unused: B1,
    flip_x: bool,
    flip_y: bool,
    priority: B1,
}

#[derive(Copy, Clone)]
pub struct SpriteAttribute {
    y: u8,
    x: u8,
    pattern: SpriteIdx,
    flags: OAMFlag,
}

#[derive(Copy, Clone)]
struct Color {
    high: u8,
    low: u8,
}

enum DisplayMode {
    StrictGB,
    CGBCompat,
    CGB,
}

pub struct Display {
    vram: [u8; (2 * 8) << 10],
    oam: [SpriteAttribute; 40],
    oam_searched: Vec<(usize, SpriteAttribute)>,
    scx: u8,
    scy: u8,
    lcdc: LCDCControl,
    stat: StatFlag,
    ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    //TODO CGB
    vbk: u8,
    bgps: u8,
    bgpd: u8,
    obps: u8,
    obpd: u8,
    //END TODO CGB
    ppu: PPU,
    unused_cycles: cycles::CycleCount,
    state: DisplayState,
    frame: u64,
    cgb_mode: DisplayMode,
    bgpalette: [[Color; 4]; 8],
    objpalette: [[Color; 4]; 8],
    time: cycles::CycleCount,
}

impl Display {
    fn bank_vram(vram: &mut [u8], bank: u8) -> &mut [u8] {
        let start = usize::from(bank) * (8 << 10);
        let len = 8 << 10;
        &mut vram[start..start + len]
    }
    fn bank_vram_ro(vram: &[u8], bank: u8) -> &[u8] {
        let start = usize::from(bank) * (8 << 10);
        let len = 8 << 10;
        &vram[start..start + len]
    }

    pub fn new(cgb: CGBStatus) -> Display {
        let cgb_mode = match cgb {
            CGBStatus::GB => {
                if false {
                    /* TODO: optionally allow user to go back to GB */
                    DisplayMode::StrictGB
                } else {
                    DisplayMode::CGBCompat
                }
            }
            CGBStatus::SupportsCGB | CGBStatus::CGBOnly => DisplayMode::CGB,
        };
        Display {
            time: 0 * cycles::GB,
            cgb_mode,
            frame: 0,
            vram: [0; (2 * 8) << 10],
            oam: [SpriteAttribute {
                x: 0,
                y: 0,
                flags: OAMFlag::new(),
                pattern: SpriteIdx(0),
            }; 40],
            oam_searched: Vec::with_capacity(10),
            scx: 0,
            scy: 0,
            lcdc: LCDCControl::new(),
            stat: StatFlag::new(),
            ly: 144,
            lyc: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            //TODO: CGB
            vbk: 0,
            bgpd: 0,
            bgps: 0,
            obpd: 0,
            obps: 0,
            //END CGB
            ppu: PPU::new(),
            state: DisplayState::VBlank,
            unused_cycles: cycles::Cycles::new(0),
            bgpalette: [[Color { high: 0, low: 0 }; 4]; 8],
            objpalette: [[Color { high: 0, low: 0 }; 4]; 8],
        }
    }
    pub fn init(&mut self, checksum: u8, dis: u8, key_palette: Option<usize>) {
        fn convert_palette(display: &mut Display, p: &[u8; 4], reg: MemRegister) {
            p.iter()
                .map(|m| {
                    let c = PALETTE_COLORS[usize::from(*m)];
                    let mut entry = ColorEntry::new();
                    entry.set_b(c[2] >> 3);
                    entry.set_g(c[1] >> 3);
                    entry.set_r(c[0] >> 3);
                    entry
                })
                .flat_map(|d| d.to_bytes().iter().copied().collect::<Vec<_>>().into_iter())
                .for_each(|d| display.write_byte(reg as u16, d))
        }

        let chosen = key_palette
            .and_then(|p| KEY_PALETTES.get(p))
            .map(|key| &key.palette);

        let builtin = &match (checksum, dis) {
            (0xDB, _) | (0x15, _) => CustomPalette {
                bg: [75, 68, 52, 0],
                obj0: [75, 68, 52, 0],
                obj1: [75, 68, 52, 0],
            },
            (0x6B, _) | (0x18, 0x4B) | (0x6A, 0x4B) => CustomPalette {
                bg: [75, 32, 15, 0],
                obj0: [63, 65, 33, 13],
                obj1: [75, 20, 52, 1],
            },
            (0x14, _) => CustomPalette {
                bg: [75, 59, 34, 0],
                obj0: [75, 29, 6, 0],
                obj1: [75, 59, 34, 0],
            },
            (0xA8, _) | (0x86, _) => CustomPalette {
                bg: [72, 38, 22, 2],
                obj0: [63, 65, 33, 13],
                obj1: [75, 59, 34, 0],
            },
            (0x4B, _) | (0x90, _) | (0x9A, _) | (0xBD, _) | (0x28, 0x46) => CustomPalette {
                bg: [75, 29, 6, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 59, 34, 0],
            },
            (0x3D, _) | (0x6A, 0x49) => CustomPalette {
                bg: [75, 17, 54, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 59, 34, 0],
            },
            (0x3E, _) | (0xE0, _) => CustomPalette {
                bg: [75, 61, 52, 0],
                obj0: [75, 61, 52, 0],
                obj1: [75, 20, 52, 1],
            },
            (0x4E, _) => CustomPalette {
                bg: [75, 23, 1, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 70, 8, 52],
            },
            (0x17, _) | (0x8B, _) | (0x27, 0x4E) | (0x61, 0x41) => CustomPalette {
                bg: [75, 29, 6, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 23, 1, 0],
            },
            (0x70, _) => CustomPalette {
                bg: [75, 59, 34, 0],
                obj0: [75, 9, 10, 3],
                obj1: [75, 23, 1, 0],
            },
            (0x3C, _) => CustomPalette {
                bg: [75, 23, 1, 0],
                obj0: [75, 23, 1, 0],
                obj1: [75, 59, 34, 0],
            },
            (0x46, 0x45) => CustomPalette {
                bg: [47, 71, 44, 0],
                obj0: [0, 75, 59, 34],
                obj1: [0, 75, 59, 34],
            },
            (0x5C, _) | (0x49, _) | (0xB3, 0x42) | (0x27, 0x42) => CustomPalette {
                bg: [42, 68, 4, 0],
                obj0: [56, 49, 21, 0],
                obj1: [1, 75, 70, 8],
            },
            (0x10, _)
            | (0xF6, _)
            | (0x68, _)
            | (0x29, _)
            | (0x52, _)
            | (0x01, _)
            | (0x5D, _)
            | (0x6D, _) => CustomPalette {
                bg: [75, 62, 30, 0],
                obj0: [75, 23, 1, 0],
                obj1: [75, 29, 6, 0],
            },
            (0x19, _) => CustomPalette {
                bg: [75, 61, 52, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 59, 34, 0],
            },
            (0xD3, 0x52) => CustomPalette {
                bg: [75, 32, 15, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 32, 15, 0],
            },
            (0xBF, 0x20) => CustomPalette {
                bg: [75, 32, 15, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 59, 34, 0],
            },
            (0xF2, _) | (0x69, _) | (0x0D, 0x52) => CustomPalette {
                bg: [75, 68, 52, 0],
                obj0: [75, 68, 52, 0],
                obj1: [75, 20, 52, 1],
            },
            (0x95, _) | (0xB3, 0x52) => CustomPalette {
                bg: [75, 17, 54, 0],
                obj0: [75, 17, 54, 0],
                obj1: [75, 20, 52, 1],
            },
            (0x8C, _) => CustomPalette {
                bg: [75, 45, 12, 0],
                obj0: [75, 57, 35, 0],
                obj1: [75, 45, 12, 0],
            },
            (0x59, _) | (0xC6, 0x41) => CustomPalette {
                bg: [75, 45, 12, 0],
                obj0: [75, 57, 35, 0],
                obj1: [75, 20, 52, 1],
            },
            (0x97, _) | (0x39, _) | (0x43, _) => CustomPalette {
                bg: [75, 62, 30, 0],
                obj0: [75, 23, 1, 0],
                obj1: [75, 23, 1, 0],
            },
            (0xC9, _) => CustomPalette {
                bg: [74, 24, 40, 19],
                obj0: [75, 57, 35, 0],
                obj1: [75, 23, 1, 0],
            },
            (0x9C, _) => CustomPalette {
                bg: [75, 32, 15, 0],
                obj0: [75, 32, 15, 0],
                obj1: [63, 65, 33, 13],
            },
            (0xF4, 0x2D) => CustomPalette {
                bg: [75, 29, 5, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 23, 1, 0],
            },
            (0x16, _)
            | (0x92, _)
            | (0x35, _)
            | (0x75, _)
            | (0x99, _)
            | (0x0C, _)
            | (0xB7, _)
            | (0x67, _) => CustomPalette {
                bg: [75, 62, 30, 0],
                obj0: [75, 62, 30, 0],
                obj1: [75, 62, 30, 0],
            },
            (0xD3, 0x49) => CustomPalette {
                bg: [75, 45, 12, 0],
                obj0: [75, 62, 30, 0],
                obj1: [75, 23, 1, 0],
            },
            (0x88, _) => CustomPalette {
                bg: [42, 68, 4, 0],
                obj0: [42, 68, 4, 0],
                obj1: [42, 68, 4, 0],
            },
            (0x34, _) | (0x66, 0x45) | (0xF4, 0x20) => CustomPalette {
                bg: [75, 28, 46, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 59, 34, 0],
            },
            (0x46, 0x52) => CustomPalette {
                bg: [75, 23, 1, 0],
                obj0: [68, 52, 21, 0],
                obj1: [75, 29, 6, 0],
            },
            (0xE8, _) | (0x28, 0x41) | (0xA5, 0x41) => CustomPalette {
                bg: [0, 7, 66, 75],
                obj0: [0, 7, 66, 75],
                obj1: [0, 7, 66, 75],
            },
            (0xA5, 0x52) => CustomPalette {
                bg: [75, 62, 30, 0],
                obj0: [75, 29, 6, 0],
                obj1: [75, 29, 6, 0],
            },
            (0xB3, 0x55) => CustomPalette {
                bg: [75, 45, 12, 0],
                obj0: [75, 57, 35, 0],
                obj1: [75, 57, 35, 0],
            },
            (0xAA, _) => CustomPalette {
                bg: [75, 29, 5, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 29, 5, 0],
            },
            (0x36, _) => CustomPalette {
                bg: [16, 58, 68, 75],
                obj0: [75, 75, 23, 1],
                obj1: [75, 59, 34, 0],
            },
            (0xFF, _) | (0x71, _) => CustomPalette {
                bg: [75, 61, 52, 0],
                obj0: [75, 61, 52, 0],
                obj1: [75, 61, 52, 0],
            },
            (0x1D, _) => CustomPalette {
                bg: [42, 68, 4, 0],
                obj0: [56, 49, 21, 0],
                obj1: [56, 49, 21, 0],
            },
            (0x0D, 0x45) => CustomPalette {
                bg: [75, 32, 15, 0],
                obj0: [63, 65, 33, 13],
                obj1: [63, 65, 33, 13],
            },
            (0xF7, _) | (0xA2, _) => CustomPalette {
                bg: [75, 62, 30, 0],
                obj0: [75, 29, 6, 0],
                obj1: [75, 23, 1, 0],
            },
            (0x9D, _) => CustomPalette {
                bg: [75, 32, 15, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 62, 30, 0],
            },
            (0x58, _) => CustomPalette {
                bg: [75, 43, 14, 0],
                obj0: [75, 43, 14, 0],
                obj1: [75, 43, 14, 0],
            },
            (0x6F, _) => CustomPalette {
                bg: [75, 64, 39, 0],
                obj0: [75, 64, 39, 0],
                obj1: [75, 64, 39, 0],
            },
            (0x61, 0x45) => CustomPalette {
                bg: [75, 23, 1, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 23, 1, 0],
            },
            (0xD1, _) | (0xF0, _) | (0xCE, _) | (0xBF, 0x43) => CustomPalette {
                bg: [26, 75, 55, 0],
                obj0: [75, 75, 23, 1],
                obj1: [75, 62, 30, 0],
            },
            (0x3F, _) | (0xC6, 0x20) | (0x18, 0x49) | (0x66, 0x4C) | (_, _) => CustomPalette {
                bg: [75, 29, 5, 0],
                obj0: [75, 59, 34, 0],
                obj1: [75, 59, 34, 0],
            },
        };

        if let Some(p) = chosen.or(Some(builtin)) {
            self.write_byte(MemRegister::BGPS as u16, 0x80);
            convert_palette(self, &p.bg, MemRegister::BGPD);
            self.write_byte(MemRegister::OBPS as u16, 0x80);
            convert_palette(self, &p.obj0, MemRegister::OBPD);
            convert_palette(self, &p.obj1, MemRegister::OBPD);
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
        self.lcdc.get_lcd_display_enable()
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
        if self.lcdc.get_sprite_size() {
            16
        } else {
            8
        }
    }
    fn oam_search(&mut self) {
        assert_eq!(self.oam_searched.capacity(), 10);
        self.oam_searched.clear();
        if self.lcdc.get_sprite_display_enable() {
            self.oam_searched.extend(
                self.oam
                    .iter()
                    .enumerate()
                    .filter(
                        /* ignored invisible sprites */
                        |(_, oam)| oam.x != 0 && oam.x < 168 && oam.y != 0 && oam.y < 144 + 16,
                    )
                    .filter(/* filter only items in this row */ |(_, oam)| {
                        self.ly + 16 >= oam.y && self.ly + 16 - oam.y < self.sprite_size()
                    })
                    .sorted_by_key(|(i, oam)| {
                        if let DisplayMode::CGBCompat = self.cgb_mode {
                            *i
                        } else {
                            usize::try_from(oam.x).unwrap()
                        }
                    })
                    .take(10)
                    .map(|(i, oam)| (i, *oam)),
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
        let bg_map = if !self.lcdc.get_bg_tile_map_select() {
            0x1800
        } else {
            0x1C00
        };
        let idx = bg_map + u16::from(true_y) * 32 + u16::from(true_x);
        let flags = match self.cgb_mode {
            DisplayMode::StrictGB | DisplayMode::CGBCompat => 0,
            DisplayMode::CGB => Display::bank_vram_ro(&self.vram, 1)[idx as usize],
        };
        let flags = BGMapFlag::try_from(&[flags][..]).unwrap();
        BGIdx(Display::bank_vram_ro(&self.vram, 0)[idx as usize], flags)
    }
    fn get_win_tile(&self, x: u8) -> Tile {
        let x = x.wrapping_sub(self.wx.wrapping_sub(7));
        let y = self.ly.wrapping_sub(self.wy);
        let win_map = if !self.lcdc.get_window_tile_map_display_select() {
            0x1800u16
        } else {
            0x1c00u16
        };
        let idx = win_map + (u16::from(y) / 8) * 32 + (u16::from(x) / 8);
        let flags = match self.cgb_mode {
            DisplayMode::StrictGB | DisplayMode::CGBCompat => 0,
            DisplayMode::CGB => Display::bank_vram_ro(&self.vram, 1)[idx as usize],
        };
        let flags = BGMapFlag::try_from(&[flags][..]).unwrap();
        Tile::Window(
            BGIdx(Display::bank_vram_ro(&self.vram, 0)[idx as usize], flags),
            Coord(0, y % 8),
        )
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
                print!("{:02x}:{:02x} ", bgidx.0, bgidx.1.to_bytes()[0]);
            }
            println!();
        }

        for t in 0..=0x20 {
            let idx = BGIdx(t, BGMapFlag::new());
            println!("BG Tile {}", t);
            let mut ppu = PPU::new();
            for y in 0..8 {
                let t = Tile::BG(idx, Coord(0, y));
                ppu.load(&t, t.fetch(self));
                for _x in 0..8 {
                    let c = match ppu.shift() {
                        Pixel(_, _, PaletteShade::Empty) => 0,
                        Pixel(_, _, PaletteShade::Low) => 1,
                        Pixel(_, _, PaletteShade::Mid) => 2,
                        Pixel(_, _, PaletteShade::High) => 3,
                    };
                    print!("{} ", c)
                }
                println!();
            }
        }
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match addr {
            0x8000..=0x9fff => {
                &mut Display::bank_vram(&mut self.vram, self.vbk & 0b1)[(addr - 0x8000) as usize]
            }
            0xFE00..=0xFE9F => {
                let idx = ((addr & 0xff) >> 2) as usize;
                let oam = &mut self.oam[idx];
                match addr & 0b11 {
                    0b00 => &mut oam.y,
                    0b01 => &mut oam.x,
                    0b10 => &mut oam.pattern.0,
                    0b11 => unimplemented!("OAM Flags should be accessed elsewhere"),
                    _ => panic!("invalid oam access"),
                }
            }
            0xff40 => unimplemented!("LCDC should be accessed elsewhere"),
            0xff41 => unimplemented!("STAT should be accessed elsewhere"),
            0xff42 => &mut self.scy,
            0xff43 => &mut self.scx,
            0xff44 => &mut self.ly,
            0xff45 => &mut self.lyc,
            0xff47 => &mut self.bgp,
            0xff48 => &mut self.obp0,
            0xff49 => &mut self.obp1,
            0xff4a => &mut self.wy,
            0xff4b => &mut self.wx,
            //TODO: CGB
            0xff4f => &mut self.vbk,
            0xff68 => &mut self.bgps,
            0xff69 => &mut self.bgpd,
            0xff6a => &mut self.obps,
            0xff6b => &mut self.obpd,
            _ => panic!("unhandled address in display {:x}", addr),
        }
    }

    fn bgp_shade(&self, p: Pixel) -> (u8, u8, u8, u8) {
        fn rgb_from_palette_color(color: Color) -> (u8, u8, u8, u8) {
            let rgb = ColorEntry::try_from(&[color.low, color.high][..]).unwrap();
            (
                rgb.get_r() << 3, //r
                rgb.get_g() << 3, //g
                rgb.get_b() << 3, //b
                0xff,
            )
        }
        let white = (0xff, 0xff, 0xff, 0xff);
        let dark_grey = (0xaa, 0xaa, 0xaa, 0xff);
        let light_grey = (0x55, 0x55, 0x55, 0xff);
        let black = (0x00, 0x00, 0x00, 0xff);
        if let (true, Pixel(_, palette, shade)) = (self.display_enabled(), p) {
            let shade_id: u32 = match shade {
                PaletteShade::High => 3,
                PaletteShade::Mid => 2,
                PaletteShade::Low => 1,
                PaletteShade::Empty => 0,
            };
            match palette {
                Palette::BG | Palette::OBP0 | Palette::OBP1 => {
                    let pal = match palette {
                        Palette::BG => {
                            /* turns off all background/windowing */
                            if !self.lcdc.get_bg_display_priority() {
                                0
                            } else {
                                self.bgp
                            }
                        }
                        Palette::OBP0 => self.obp0,
                        Palette::OBP1 => self.obp1,
                        _ => unreachable!(),
                    };
                    let shade_id = (pal >> (shade_id * 2)) & 0b11;
                    let gbc = match self.cgb_mode {
                        DisplayMode::CGB | DisplayMode::CGBCompat => true,
                        DisplayMode::StrictGB => false,
                    };
                    if gbc {
                        let palette = match palette {
                            Palette::BG => &self.bgpalette[0],
                            Palette::OBP0 => &self.objpalette[0],
                            Palette::OBP1 => &self.objpalette[1],
                            _ => unreachable!(),
                        };
                        rgb_from_palette_color(palette[usize::from(shade_id)])
                    } else {
                        match shade_id {
                            0b00 => white,
                            0b01 => dark_grey,
                            0b10 => light_grey,
                            0b11 => black,
                            _ => panic!("shade out of bounds"),
                        }
                    }
                }
                Palette::BGColor(index) | Palette::OBPColor(index) => {
                    let palette = match palette {
                        Palette::BGColor(_) => &self.bgpalette,
                        Palette::OBPColor(_) => &self.objpalette,
                        _ => unreachable!(),
                    };
                    let color = palette[usize::from(index)][usize::try_from(shade_id).unwrap()];
                    rgb_from_palette_color(color)
                }
            }
        } else {
            white
        }
    }

    fn add_oams<'sprite, T: Iterator<Item = &'sprite (usize, SpriteAttribute)>>(
        &mut self,
        oams: &mut std::iter::Peekable<T>,
        x: u8,
        y: u8,
    ) {
        'oams_done: while oams.peek().is_some() {
            let use_oam = if let Some((_priority, cur)) = oams.peek() {
                cur.x <= x
            } else {
                false
            };
            if !use_oam {
                break 'oams_done;
            }
            let (priority, oam) = oams.next().unwrap();
            if oam.x == x {
                let t = Tile::Sprite(*priority, *oam, Coord(0, y + 16 - oam.y));
                let l = t.fetch(self);
                self.ppu.load(&t, l);
            }
        }
    }

    fn draw_window<'sprite, T: Iterator<Item = &'sprite (usize, SpriteAttribute)>>(
        &mut self,
        lcd_line: &mut [u8],
        oams: &mut std::iter::Peekable<T>,
        window: bool,
        bg_offset: u8,
        range: &mut std::ops::Range<u8>,
    ) {
        /* offscreen pixels */
        // if std::ops::Range::is_empty(range) {
        //     return;
        // }
        let fake = Tile::BG(BGIdx(0, BGMapFlag::new()), Coord(0, 0));
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
struct BGIdx(u8, BGMapFlag);
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
    fn y(self) -> u8 {
        self.1
    }
}

#[derive(Clone)]
enum Tile {
    BG(BGIdx, Coord),
    Window(BGIdx, Coord),
    Sprite(usize, SpriteAttribute, Coord),
}

impl Tile {
    #[allow(dead_code)]
    pub fn show(&self, display: &mut Display) {
        let mut tmp: Tile = self.to_owned();
        for i in 0..display.sprite_size() {
            let c: &mut Coord = match tmp {
                Tile::BG(_, ref mut c) => c,
                Tile::Window(_, ref mut c) => c,
                Tile::Sprite(_, _, ref mut c) => c,
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
    pub fn fetch(&self, display: &mut Display) -> (Priority, Palette, u16) {
        let (start, line_offset, flip_x, vbank) = match *self {
            Tile::Window(idx, coord) | Tile::BG(idx, coord) => {
                let bytes_per_tile: u16 = 16;
                let start = if !display.lcdc.get_bg_win_tile_data_select() {
                    /* signed tile idx */
                    let signed_idx = i16::from(idx.0 as i8);
                    (0x1000 + signed_idx * bytes_per_tile as i16) as u16
                } else {
                    /*unsigned tile_idx */
                    u16::from(idx.0) * bytes_per_tile
                };
                let bgmap = idx.1;
                let y = if bgmap.get_flip_y() {
                    bytes_per_tile as i16 - 1 - i16::from(coord.y())
                } else {
                    i16::from(coord.y())
                };
                (
                    start as usize,
                    (y % 8) as usize,
                    bgmap.get_flip_x(),
                    bgmap.get_vram_bank(),
                )
            }
            Tile::Sprite(_, oam, coord) => {
                let bytes_per_line = 2;
                let idx = if display.sprite_size() == 16 {
                    oam.pattern.0 >> 1
                } else {
                    oam.pattern.0
                };
                let start = u16::from(idx) * bytes_per_line * u16::from(display.sprite_size());
                let y = if oam.flags.get_flip_y() {
                    display.sprite_size() - 1 - coord.y()
                } else {
                    coord.y()
                };
                (
                    start as usize,
                    y as usize,
                    oam.flags.get_flip_x(),
                    oam.flags.get_vram_bank(),
                )
            }
        };

        let b1 = Display::bank_vram(&mut display.vram, vbank)[start + (line_offset * 2)];
        let b2 = Display::bank_vram(&mut display.vram, vbank)[start + (line_offset * 2) + 1];

        let line = if flip_x {
            u16::from_le_bytes([b1.reverse_bits(), b2.reverse_bits()])
        } else {
            u16::from_le_bytes([b1, b2])
        };

        let gbc = match display.cgb_mode {
            DisplayMode::CGB => true,
            DisplayMode::StrictGB | DisplayMode::CGBCompat => false,
        };
        let low_priority = std::usize::MAX;
        let (priority, palette) = match *self {
            Tile::Sprite(p, oam, _) => {
                let priority = if oam.flags.get_priority() {
                    Priority::BG
                } else {
                    Priority::Obj(p)
                };
                let palette = if gbc {
                    Palette::OBPColor(oam.flags.get_color_palette())
                } else if oam.flags.get_palette_number() == 0 {
                    Palette::OBP0
                } else {
                    Palette::OBP1
                };
                (priority, palette)
            }
            Tile::BG(idx, _) | Tile::Window(idx, _) => {
                if gbc {
                    let bgmap = idx.1;
                    let palette_number = bgmap.get_color_palette();
                    (
                        if bgmap.get_priority() != 0 {
                            Priority::BG
                        } else {
                            Priority::Obj(low_priority)
                        },
                        Palette::BGColor(palette_number),
                    )
                } else {
                    (Priority::Obj(low_priority), Palette::BG)
                }
            }
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
    BGColor(u8),
    OBPColor(u8),
}

struct Pixel(Priority, Palette, PaletteShade);

struct PPU {
    shift: VecDeque<Pixel>,
}

#[derive(Copy, Clone)]
enum Priority {
    Obj(usize),
    BG,
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            shift: VecDeque::with_capacity(16),
        }
    }
    fn load(&mut self, _t: &Tile, (priority, palette, line): (Priority, Palette, u16)) {
        for x in 0..8 {
            let p = Tile::line_palette(line, x);

            match palette {
                Palette::OBPColor(_) | Palette::OBP1 | Palette::OBP0 => {
                    if p != PaletteShade::Empty {
                        match (&mut self.shift[x], priority, p) {
                            (old @ Pixel(_, Palette::OBP0, _), _, _)
                            | (old @ Pixel(_, Palette::OBP1, _), _, _)
                            | (old @ Pixel(_, Palette::OBPColor(_), _), _, _) => {
                                match (old.0, priority) {
                                    (Priority::Obj(old_pri), Priority::Obj(new_pri))
                                        if old_pri < new_pri =>
                                    { /* preserve old object */ }
                                    _ => *old = Pixel(priority, palette, p),
                                }
                            }
                            (
                                old @ Pixel(Priority::BG, _, PaletteShade::Empty),
                                priority,
                                shade,
                            )
                            | (
                                old @ Pixel(Priority::Obj(_), _, _),
                                priority @ Priority::Obj(_),
                                shade,
                            )
                            | (
                                old @ Pixel(Priority::Obj(_), _, PaletteShade::Empty),
                                priority @ Priority::BG,
                                shade,
                            ) => *old = Pixel(priority, palette, shade),
                            _ => { /* preserve background */ }
                        };
                    }
                }
                Palette::BGColor(_) | Palette::BG => {
                    self.shift.push_back(Pixel(priority, palette, p));
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

enum ColorPaletteMask {
    HighLow = 0b0000_0001,
    PaletteDataMask = 0b0000_0110,
    PaletteNumMask = 0b0011_1000,
    NextWrite = 0b1000_0000,
}

impl Addressable for Display {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match addr {
            0xFE00..=0xFE9F => {
                let idx = ((addr & 0xff) >> 2) as usize;
                let oam = &mut self.oam[idx];
                match addr & 0b11 {
                    0b11 => oam.flags.to_bytes()[0],
                    _ => *self.lookup(addr),
                }
            }
            0xff40 => self.lcdc.to_bytes()[0],
            0xff41 => self.stat.to_bytes()[0],
            _ => *self.lookup(addr),
        }
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        fn update_color_palette(palette: &mut [[Color; 4]; 8], mut control: u8, data: u8) -> u8 {
            let high = (control & mask_u8!(ColorPaletteMask::HighLow)) != 0;
            let palette_data =
                usize::from((control & mask_u8!(ColorPaletteMask::PaletteDataMask)) >> 1);
            let palette_num =
                usize::from((control & mask_u8!(ColorPaletteMask::PaletteNumMask)) >> 3);
            let next_palette = (control & mask_u8!(ColorPaletteMask::NextWrite)) != 0;

            let color = &mut palette[palette_num][palette_data];
            if high {
                color.high = data;
            } else {
                color.low = data;
            }
            if next_palette {
                let mask = mask_u8!(
                    ColorPaletteMask::PaletteDataMask
                        | ColorPaletteMask::PaletteNumMask
                        | ColorPaletteMask::HighLow
                );
                let prev = control & mask;
                control &= !mask;
                control |= (prev + 1) & mask;
            }
            control
        }
        match addr {
            0xFE00..=0xFE9F => {
                let idx = ((addr & 0xff) >> 2) as usize;
                let oam = &mut self.oam[idx];
                match addr & 0b11 {
                    0b11 => oam.flags = OAMFlag::try_from(&[v][..]).unwrap(),
                    _ => *self.lookup(addr) = v,
                }
            }
            0xff68 => self.bgps = v,
            0xff69 => {
                self.bgpd = v;
                self.bgps = update_color_palette(&mut self.bgpalette, self.bgps, self.bgpd);
            }
            0xff6a => self.obps = v,
            0xff6b => {
                self.obpd = v;
                self.obps = update_color_palette(&mut self.objpalette, self.obps, self.obpd);
            }
            0xff44 => { /* read only */ }
            0xff40 => {
                self.lcdc = LCDCControl::try_from(&[v][..]).unwrap();
            }
            0xff41 => {
                self.stat = StatFlag::try_from(&[v][..]).unwrap();
            }
            _ => *self.lookup(addr) = v,
        }
    }
}

// struct StateData {
//     ly: u32
// }

// use cycles::CycleCount as Cycles;

// enum ObservableChange {
//     Change(Cycles),
//     NoChange,
// }
// struct Transition<D> {
//     cond : Box<dyn Fn(Cycles, D) -> Option<Cycles>>,
//     action: Box<dyn FnMut(Cycles, D) -> ObservableChange>,
// }

// trait State<D>
// {
//     fn entry(&mut self, D) -> ObservableChange;
//     fn action(&mut self, Cycles, D) -> ObservableChange;
//     fn exit(&mut self, D) -> ObservableChange;
//     fn next_exit(&mut self, D) -> Transition<D>;
// }

// struct StateMachine<D> {
//     states : Vec<dyn State<D>>
// }

impl Peripheral for Display {
    fn step(&mut self, real: &mut PeripheralData, time: cycles::CycleCount) -> Option<Interrupt> {
        // let s = StateMachine::<StateData> {
        //     states: vec![

        //     ]
        // };

        let mut new_ly = self.ly;
        self.unused_cycles += time;
        self.time += time;
        let next_state = match self.state {
            DisplayState::OAMSearch => {
                if self.unused_cycles >= 20 * cycles::GB {
                    self.oam_search();
                    self.unused_cycles -= 20 * cycles::GB;
                    DisplayState::PixelTransfer
                } else {
                    self.state
                }
            }
            DisplayState::PixelTransfer => {
                if self.unused_cycles >= 43 * cycles::GB {
                    /* do work */
                    self.ppu.clear();
                    let orig_oams =
                        std::mem::replace(&mut self.oam_searched, Vec::with_capacity(0));
                    {
                        let (true_x, _true_y) = self.get_bg_true(0, self.ly);
                        let has_window = self.lcdc.get_window_enable() && self.ly >= self.wy;
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
                        if let Some(windows) = split_line.as_mut() {
                            for w in windows {
                                let (line, offset, range, is_win) = w;
                                let mut oams = orig_oams.iter().peekable();
                                self.draw_window(line, &mut oams, *is_win, *offset, range);
                            }
                        };

                        self.ppu.clear();
                        self.unused_cycles -= 43 * cycles::GB;
                    }
                    std::mem::replace(&mut self.oam_searched, orig_oams);
                    DisplayState::HBlank
                } else {
                    self.state
                }
            }
            DisplayState::HBlank => {
                if self.unused_cycles >= 51 * cycles::GB {
                    /* do work */
                    self.unused_cycles -= 51 * cycles::GB;
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
                if self.unused_cycles >= (43 + 51 + 20) * cycles::GB {
                    /* do work */
                    self.unused_cycles -= (43 + 51 + 20) * cycles::GB;
                    new_ly += 1;
                    if new_ly == 154 {
                        self.frame += 1;
                        self.time = 0 * cycles::GB;
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

        //TODO: Determine which interrupts actually fire during Display Off
        // At the very least, VBLANK does not.
        let changed_state = next_state != self.state;
        if changed_state {
            let new_mode = match next_state {
                DisplayState::OAMSearch => StatMode::OAM,
                DisplayState::VBlank => StatMode::VBlank,
                DisplayState::HBlank => StatMode::HBlank,
                DisplayState::PixelTransfer => StatMode::PixelTransfer,
            };
            self.stat.set_mode(new_mode);
            assert_eq!(new_mode, self.stat.get_mode());
            self.state = next_state;
        }

        let mut new_interrupt = Interrupt::new();
        if new_ly != self.ly {
            self.ly = new_ly;
            self.stat.set_coincidence_flag(self.ly == self.lyc);
            if self.stat.get_coincidence() {
                new_interrupt.set_lcdc(self.stat.get_coincidence_flag());
            }
        }

        if changed_state {
            if self.stat.get_mode() == StatMode::VBlank {
                new_interrupt.set_vblank(true);
            }
            let lcdc = new_interrupt.get_lcdc()
                || match self.stat.get_mode() {
                    StatMode::OAM => self.stat.get_oam(),
                    StatMode::VBlank => self.stat.get_vblank(),
                    StatMode::HBlank => self.stat.get_hblank(),
                    StatMode::PixelTransfer => false,
                };
            new_interrupt.set_lcdc(lcdc);
        }
        Some(new_interrupt)
    }
}
