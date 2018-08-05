use peripherals::{Addressable, Peripheral};

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
    pub fn fake() -> Cart {
        Cart {
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
        }
    }
    pub fn new(rom: Vec<u8>) -> Cart {
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
            None | Some(MBC1) => {}
            _ => panic!("Unhandled MBC"),
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
        Cart {
            title,
            cgb,
            mbc,
            battery,
            ram: vec![0u8; ram_size],
            rom,
            rom_reg: 1,
            ram_reg: 0,
            bank_mode: BankMode::ROM,
            ram_enable: false,
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

impl Peripheral for Cart {}

impl Addressable for Cart {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000...0x3FFF => {
                let addr = self.rom_offset(0x0000, addr);
                self.rom[addr]
            }
            0x4000...0x7FFF => {
                let addr = self.rom_offset(0x4000, addr);
                self.rom[addr]
            }
            0xA000...0xBFFF => {
                if let Some(addr) = self.ram_offset(addr) {
                    self.ram[addr]
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
                self.ram_enable = (v & 0xF) == 0xA;
            }
            0x2000...0x3fff => {
                self.rom_reg &= 0x60;
                let new_v = (v & 0b11111) as usize;
                self.rom_reg |= new_v;
                if new_v == 0 {
                    self.rom_reg |= 1;
                }
            }
            0x4000...0x5fff => {
                match self.bank_mode {
                    BankMode::RAM => {
                        //self.rom_reg &= 0x1f;
                        self.ram_reg = (v & 0b11) as usize;
                    }
                    BankMode::ROM => {
                        self.rom_reg &= 0x1f;
                        self.rom_reg |= ((v & 0b11) << 5) as usize;
                        //self.ram_reg = 0;
                    }
                }
            }
            0x6000...0x7FFF => {
                self.bank_mode = match v & 0x1 {
                    0 => BankMode::ROM,
                    1 => BankMode::RAM,
                    _ => panic!("unhandled bank mode"),
                }
            }
            0xA000...0xBFFF => {
                if let Some(addr) = self.ram_offset(addr) {
                    self.ram[addr] = v;
                }
            }
            _ => panic!("Unhandled Cart Write Access {:04x}", addr),
        }
    }
}
