use super::mmu::MemRegister;
use crate::cpu::Interrupt;
use crate::cycles;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use enum_primitive::FromPrimitive;

use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone)]
pub enum GBControl {
    Right = 1 << 4,
    Left = 1 << 5,
    Up = 1 << 6,
    Down = 1 << 7,
    A = 1,
    B = 1 << 1,
    Select = 1 << 2,
    Start = 1 << 3,
}

#[derive(Serialize, Deserialize, Clone)]
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

impl Addressable for Controller {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::P1 => {
                let button_keys = (self.p1 & 0b0010_0000) == 0;
                let dir_keys = (self.p1 & 0b0001_0000) == 0;
                match (button_keys, dir_keys) {
                    (false, true) => self.p1 | ((self.read >> 4) & 0x0f),
                    (true, false) => self.p1 | (self.read & 0x0f),
                    (_, _) => self.p1 | 0xf,
                }
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
}
impl Peripheral for Controller {
    fn next_step(&self) -> Option<cycles::CycleCount> {
        /* Controller does not generate any interrupts and only needs to
        be updated when observed */
        Some(cycles::CycleCount::new(std::u64::MAX))
    }
    fn step(&mut self, _real: &mut PeripheralData, _time: cycles::CycleCount) -> Option<Interrupt> {
        let res = if (self.old ^ self.read) & !self.read != 0 {
            let mut i = Interrupt::new();
            i.set_hi_lo(true);
            Some(i)
        } else {
            None
        };
        self.old = self.read;
        res
    }
}
