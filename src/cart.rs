use crate::cpu::InterruptFlag;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};

enum CGBStatus {
    GB,
    SupportsCGB,
    CGBOnly,
}

#[derive(Debug)]
enum MBCType {
    MBC1,
    MBC2,
    MBC3,
    MBC5,
    MBC6,
    MBC7,
    MMM01,
}

#[allow(dead_code)]
pub struct Cart {
    mbc: Option<MBCType>,
    ram: Vec<u8>,
    battery: bool,
    title: String,
    cgb: CGBStatus,
    rom: Vec<u8>,
    rom_reg: usize,
    ram_reg: usize,
    bank_mode: BankMode,
    ram_enable: bool,
}
#[derive(Debug)]
enum BankMode {
    ROM,
    RAM,
}

impl Cart {
    pub fn fake() -> Box<Peripheral> {
        let cart = Cart {
            title: "Fake ROM".to_string(),
            cgb: CGBStatus::GB,
            mbc: None,
            battery: false,
            ram: Vec::with_capacity(0),
            rom: Vec::with_capacity(0),
            rom_reg: 1,
            ram_reg: 0,
            bank_mode: BankMode::ROM,
            ram_enable: false,
        };
        Box::new(CartMBC1 { cart })
    }
    pub fn new(rom: Vec<u8>) -> Box<Peripheral> {
        let cgb = match rom[0x143] {
            0x80 => CGBStatus::SupportsCGB,
            0xC0 => CGBStatus::CGBOnly,
            _ => CGBStatus::GB,
        };

        let end = match cgb {
            CGBStatus::SupportsCGB | CGBStatus::CGBOnly => 0x143,
            _ => 0x144,
        };
        let title = std::str::from_utf8(rom[0x134..end].as_ref())
            .unwrap_or("Invalid Title")
            .to_owned();

        use self::MBCType::*;

        let mbc = match rom[0x147] {
            0x00 => None,
            0x01...0x03 => Some(MBC1),
            0x05...0x06 => Some(MBC2),
            0x08...0x09 => None,
            0x0B...0x0D => Some(MMM01),
            0x0F...0x13 => Some(MBC3),
            0x19...0x1E => Some(MBC5),
            0x20 => Some(MBC6),
            0x22 => Some(MBC7),
            _ => panic!("Unhandled Cart Type"),
        };

        println!("MBC: {:?}", mbc);
        match mbc {
            None | Some(MBC1) | Some(MBC3) | Some(MBC5)=> {}
            Some(m) => panic!("Unhandled MBC {:?}", m),
        };

        let battery = match rom[0x147] {
            0x03 | 0x06 | 0x09 | 0x0D | 0x10 | 0x13 | 0x1B | 0x1e | 0x22 | 0xff => true,
            _ => false,
        };

        let ram_size = match rom[0x149] {
            0x00 => {
                if let Some(MBC2) = mbc {
                    512
                } else {
                    0
                }
            }
            0x01 => 2 << 10,
            0x02 => 8 << 10,
            0x03 => 32 << 10,
            0x04 => 128 << 10,
            0x05 => 64 << 10,
            u => panic!("Unhandled Ram Size: {}", u),
        };

        println!("Ram Size: {}", ram_size);
        println!("ROM Size: {}", rom.len());
        println!("ROM Claimed Size: {}", (32 << 10) << rom[0x148]);
        println!("ROM: {:4x}", rom[0x148]);
        let cart = Cart {
            title,
            cgb,
            mbc,
            battery,
            ram: vec![0u8; ram_size],
            rom,
            rom_reg: 1,
            ram_reg: 0,
            bank_mode: BankMode::ROM,
            ram_enable : false,
        };
        match cart.mbc {
            None | Some(MBC1) => Box::new(CartMBC1 { cart }),
            Some(MBC3) => Box::new(CartMBC3 {
                cart,
                rtc_latch: None,
                rtc: RTC::new(),
            }),
            Some(MBC5) => Box::new(CartMBC5 {
                cart
            }),
            _ => unimplemented!("Unhandled MBC Cart type {:?}", cart.mbc),
        }
    }

    fn ram_offset(&self, addr: u16) -> Option<usize> {
        let addr = match (self.ram_enable, &self.bank_mode) {
            (true, BankMode::ROM) => Some(addr as usize - 0xA000),
            (true, BankMode::RAM) => {
                Some((addr as usize - 0xA000 + self.ram_reg * (8 << 10)) & (self.ram.len() - 1))
            }
            (false, _) => None,
        };
        addr
    }

    fn rom_offset(&self, base: u16, addr: u16) -> usize {
        let bank = match (base, &self.bank_mode) {
            (0x0000, BankMode::RAM) => self.ram_reg << 5, /* can't find this in mooneye docs */
            (0x0000, BankMode::ROM) => 0,
            (0x4000, BankMode::RAM) => {
                let mut rhs = self.rom_reg & 0x1f;
                if rhs == 0 {
                    rhs += 1;
                }
                self.ram_reg << 5 | rhs
            }
            (0x4000, BankMode::ROM) => self.rom_reg,
            (_, _) => panic!("Unhandled Rom Offset"),
        };
        (addr as usize - base as usize + bank * (16 << 10)) & (self.rom.len() - 1)
    }
}

struct CartMBC1 {
    cart: Cart,
}

impl Peripheral for CartMBC1 {}

impl Addressable for CartMBC1 {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000...0x3FFF => {
                let addr = self.cart.rom_offset(0x0000, addr);
                self.cart.rom[addr]
            }
            0x4000...0x7FFF => {
                let addr = self.cart.rom_offset(0x4000, addr);
                self.cart.rom[addr]
            }
            0xA000...0xBFFF => {
                if let Some(addr) = self.cart.ram_offset(addr) {
                    self.cart.ram[addr]
                } else {
                    0xff
                }
            }
            _ => panic!("Unhandled Cart Read Access {:04x}", addr),
        }
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        match addr {
            0x0000...0x1fff => {
                self.cart.ram_enable = (v & 0xF) == 0xA;
            }
            0x2000...0x3fff => {
                self.cart.rom_reg &= 0x60;
                let new_v = (v & 0b11111) as usize;
                self.cart.rom_reg |= new_v;
                if new_v == 0 {
                    self.cart.rom_reg |= 1;
                }
            }
            0x4000...0x5fff => match self.cart.bank_mode {
                BankMode::RAM => {
                    self.cart.ram_reg = (v & 0b11) as usize;
                }
                BankMode::ROM => {
                    self.cart.rom_reg &= 0x1f;
                    self.cart.rom_reg |= ((v & 0b11) << 5) as usize;
                }
            },
            0x6000...0x7FFF => {
                self.cart.bank_mode = match v & 0x1 {
                    0 => BankMode::ROM,
                    1 => BankMode::RAM,
                    _ => panic!("unhandled bank mode"),
                }
            }
            0xA000...0xBFFF => {
                if let Some(addr) = self.cart.ram_offset(addr) {
                    self.cart.ram[addr] = v;
                }
            }
            _ => panic!("Unhandled Cart Write Access {:04x}", addr),
        }
    }
}
#[derive(Clone)]
struct RTC {
    microseconds: u64,
    seconds: u8,
    minutes: u8,
    hours: u8,
    days: u16,
    halt: bool,
}
impl RTC {
    fn new() -> RTC {
        RTC {
            microseconds: 0,
            seconds: 0,
            minutes: 0,
            hours: 0,
            days: 0,
            halt: false,
        }
    }
    fn step(&mut self, time: u64) {
        if !self.halt {
            self.microseconds += time;
            if self.microseconds >= 1_000_000 {
                let seconds = self.microseconds / 1_000_000;
                self.microseconds %= 1_000_000;
                self.seconds += seconds as u8;
                let minutes = self.seconds / 60;
                self.seconds %= 60;
                self.minutes += minutes;
                let hours = self.minutes / 60;
                self.hours += hours;
                self.minutes %= 60;
                let days = self.hours / 24;
                self.days += days as u16;
                self.days %= 2 << 10; // 9 bit counter + 1 bit overflow
            }
        }
    }
    fn rtc_read(&self, mode: RTCMode) -> u8 {
        match mode {
            RTCMode::Seconds => self.seconds,
            RTCMode::Minutes => self.minutes,
            RTCMode::Hours => self.hours,
            RTCMode::DayLow => u8::from((self.days & 0xff) as u8),
            RTCMode::DayHigh => {
                (self.days & 0x100 >> 8) as u8
                    | if self.halt { 1 } else { 0 } << 6
                    | (self.days & 0x200 >> (9 - 7)) as u8
            }
        }
    }
    fn rtc_write(&mut self, mode: RTCMode, v: u8) {
        match mode {
            RTCMode::Seconds => self.seconds = v % 60,
            RTCMode::Minutes => self.minutes = v % 60,
            RTCMode::Hours => self.hours = v % 24,
            RTCMode::DayLow => self.days = (self.days & !0xff) | v as u16,
            RTCMode::DayHigh => {
                self.days = self.days & 0xff | (u16::from(v) & (1 << 7) >> 6) | (u16::from(v) & 1);
                self.halt = if v & (1 << 6) != 0 { true } else { false };
            }
        }
    }
}

struct CartMBC3 {
    cart: Cart,
    rtc: RTC,
    rtc_latch: Option<RTC>,
}

enum RTCMode {
    Seconds,
    Minutes,
    Hours,
    DayLow,
    DayHigh,
}

impl CartMBC3 {
    fn rtc_select(&self) -> Option<RTCMode> {
        match self.cart.ram_reg {
            0x08 => Some(RTCMode::Seconds),
            0x09 => Some(RTCMode::Minutes),
            0x0A => Some(RTCMode::Hours),
            0x0B => Some(RTCMode::DayLow),
            0x0C => Some(RTCMode::DayHigh),
            _ => None,
        }
    }
}

impl Peripheral for CartMBC3 {
    fn step(&mut self, _real: &mut PeripheralData, time: u64) -> Option<InterruptFlag> {
        self.rtc.step(time);
        None
    }
}

impl Addressable for CartMBC3 {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000...0x3FFF => {
                let addr = self.cart.rom_offset(0x0000, addr);
                self.cart.rom[addr]
            }
            0x4000...0x7FFF => {
                let addr = self.cart.rom_offset(0x4000, addr);
                self.cart.rom[addr]
            }
            0xA000...0xBFFF => {
                if let Some(rtc_mode) = self.rtc_select() {
                    let rtc = match self.rtc_latch.as_ref() {
                        Some(latched) => latched,
                        None => &self.rtc,
                    };
                    rtc.rtc_read(rtc_mode)
                } else {
                    if let Some(addr) = self.cart.ram_offset(addr) {
                        self.cart.ram[addr]
                    } else {
                        0xff
                    }
                }
            }
            _ => panic!("Unhandled Cart Read Access {:04x}", addr),
        }
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        match addr {
            0x0000...0x1fff => {
                self.cart.ram_enable = (v & 0xF) == 0xA;
            }
            0x2000...0x3fff => {
                let new_v = (v & 0x7f) as usize;
                self.cart.rom_reg = new_v;
                if new_v == 0 {
                    self.cart.rom_reg |= 1;
                }
            }
            0x4000...0x5fff => {
                self.cart.ram_reg = usize::from(v);
            }
            0x6000...0x7FFF => {
                if v > 0 {
                    self.rtc_latch = Some(self.rtc.clone());
                } else {
                    self.rtc_latch = None;
                }
            }
            0xA000...0xBFFF => match self.rtc_select() {
                Some(rtc) => {
                    self.rtc.rtc_write(rtc, v);
                }
                None => {
                    if let Some(addr) = self.cart.ram_offset(addr) {
                        self.cart.ram[addr] = v;
                    }
                }
            },
            _ => panic!("Unhandled Cart Write Access {:04x}", addr),
        }
    }
}

struct CartMBC5 {
    cart: Cart,
}

impl Peripheral for CartMBC5 {}

impl Addressable for CartMBC5 {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000...0x3FFF => {
                let addr = self.cart.rom_offset(0x0000, addr);
                self.cart.rom[addr]
            }
            0x4000...0x7FFF => {
                let addr = self.cart.rom_offset(0x4000, addr);
                self.cart.rom[addr]
            }
            0xA000...0xBFFF => {
                if let Some(addr) = self.cart.ram_offset(addr) {
                    self.cart.ram[addr]
                } else {
                    0xff
                }
            }
            _ => panic!("Unhandled Cart Read Access {:04x}", addr),
        }
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        match addr {
            0x0000...0x1fff => {
                self.cart.ram_enable = (v & 0xF) == 0xA;
            }
            0x2000...0x2fff => {
                self.cart.rom_reg &= 0xff00;
                self.cart.rom_reg |= usize::from(v);
            },
            0x3000...0x3fff => {
                self.cart.rom_reg &= 0x00ff;
                self.cart.rom_reg |= usize::from(v & 0b1) << 8;
            },
            0x4000...0x5fff => {
                self.cart.ram_reg = usize::from(v & 0xf);
            }
            0x6000...0x7FFF => {
                /* do nothing? Pokemon yellow wants to set bank mode? MBC5 doesn't have reg switch */
            }
            0xA000...0xBFFF => {
                if let Some(addr) = self.cart.ram_offset(addr) {
                    self.cart.ram[addr] = v;
                }
            }
            _ => panic!("Unhandled Cart Write Access {:04x}", addr),
        }
    }
}
