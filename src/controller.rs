use super::mmu::MemRegister;
use cpu::InterruptFlag;
use enum_primitive::FromPrimitive;
use peripherals::{Peripheral, PeripheralData};

pub struct Controller {
    p1: u8,
    read: u8,
    old: u8,
}

impl Controller {
    pub fn new() -> Controller {
        Controller {
            p1: 0x3f,
            read: 0xff,
            old: 0xff,
        }
    }
    pub fn set_controls(&mut self, controls: u8) {
        self.read = controls;
    }
}

impl Peripheral for Controller {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::P1 => {
                self.p1 &= !0x0f;
                if self.p1 & 0x30 == 0x30 {
                    self.p1 |= 0x0f;
                } else if self.p1 & 0x10 == 0x10 {
                    self.p1 |= self.read & 0x0f;
                } else {
                    self.p1 |= (self.read >> 4) & 0x0f;
                }
                self.p1
            }
            _ => panic!("invalid controller address"),
        }
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::P1 => self.p1 = v & 0xF0,
            _ => panic!("invalid controller address"),
        }
    }
    fn step(&mut self, _real: &mut PeripheralData, _time: u64) -> Option<InterruptFlag> {
        let res = if (self.old ^ self.read) & !self.read != 0 {
            Some(InterruptFlag::HiLo)
        } else {
            None
        };
        self.old = self.read;
        res
    }
}
