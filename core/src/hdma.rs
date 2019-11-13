use super::mmu::MemRegister;
use crate::cpu::Interrupt;
use crate::cycles;
use crate::mmu::MemReg;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use crate::sound::WaitTimer;
use enum_primitive::FromPrimitive;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Copier {
    src: u16,
    dst: u16,
    len: u16,
}

impl Copier {
    pub fn new(s: u16, d: u16, l: u16) -> Copier {
        Copier {
            src: s,
            dst: d,
            len: l,
        }
    }
    pub fn extend(&mut self, l: u16) {
        self.len += l;
    }
    pub fn empty(&self) -> bool {
        self.len == 0
    }
}

impl Iterator for Copier {
    type Item = (u16, u16);
    fn next(&mut self) -> Option<(u16, u16)> {
        if self.len > 0 {
            self.len -= 1;
            let r = (self.src, self.dst);
            self.src = self.src.wrapping_add(1);
            self.dst = self.dst.wrapping_add(1);
            Some(r)
        } else {
            None
        }
    }
}
#[derive(Serialize, Deserialize, Clone)]
pub struct HDMA {
    copy: Copier,
    wait: WaitTimer,
    hblank_dma: bool,
    remain: u16,
    hdma1: MemReg,
    hdma2: MemReg,
    hdma3: MemReg,
    hdma4: MemReg,
    hdma5: MemReg,
}

impl HDMA {
    pub fn new() -> HDMA {
        HDMA {
            copy: Copier::new(0, 0, 0),
            hblank_dma: false,
            remain: 0,
            wait: WaitTimer::new(),
            hdma1: MemReg::default(),
            hdma2: MemReg::default(),
            hdma3: MemReg::default(),
            hdma4: MemReg::default(),
            hdma5: MemReg::new(0b1000_0000, 0xff, 0xff),
        }
    }
    pub fn is_active(&self) -> bool {
        (self.hdma5.reg() & 0b1000_0000) == 0
    }
    pub fn copy_bytes(&mut self) -> &mut Copier {
        &mut self.copy
    }
}

impl Peripheral for HDMA {
    fn next_step(&self) -> Option<cycles::CycleCount> {
        if self.is_active() {
            Some(self.wait.next_ready(cycles::CGB))
        } else {
            Some(cycles::CycleCount::new(std::u64::MAX))
        }
    }
    fn step(&mut self, _real: &mut PeripheralData, time: cycles::CycleCount) -> Option<Interrupt> {
        if !self.is_active() {
            /* do nothing */
        } else if self.remain == 0 && self.copy.empty() {
            self.hdma5.write_byte(0, 0b1000_0000);
        } else if let Some(c) = self.wait.ready(time, cycles::CGB) {
            use std::convert::TryFrom;
            let copy = if self.hblank_dma {
                16 * u16::try_from(c).unwrap()
            } else {
                /* TODO: This actually needs to stop the CPU (or fake time add) */
                self.remain
            };
            self.remain = self.remain.saturating_sub(copy);
            self.copy.extend(copy);
        }
        None
    }
}
impl Addressable for HDMA {
    fn write_byte(&mut self, addr: u16, val: u8) {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::HDMA1 => self.hdma1.write_byte(addr, val),
            MemRegister::HDMA2 => self.hdma2.write_byte(addr, val),
            MemRegister::HDMA3 => self.hdma3.write_byte(addr, val),
            MemRegister::HDMA4 => self.hdma4.write_byte(addr, val),
            MemRegister::HDMA5 => {
                assert_eq!(self.hdma5.reg(), 0b1000_0000); /* hopfully inactive (maybe bug)*/
                self.hdma5.write_byte(addr, 0b0000_0000); /* indicates active */
                self.hblank_dma = (val & 0b1000_0000) != 0;
                let len = (u16::from(val & 0b0111_1111) + 1) * 0x10;
                self.remain = len;
                let source = u16::from_le_bytes([self.hdma2.reg(), self.hdma1.reg()]) & 0xfff0;
                let dest =
                    (u16::from_le_bytes([self.hdma4.reg(), self.hdma3.reg()]) & 0x1ff0) | 0x8000;
                self.wait.reset();
                self.copy = Copier::new(source, dest, 0);
            }
            _ => panic!("invalid hdma address"),
        }
    }
    fn read_byte(&mut self, addr: u16) -> u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::HDMA1 => self.hdma1.read_byte(addr),
            MemRegister::HDMA2 => self.hdma2.read_byte(addr),
            MemRegister::HDMA3 => self.hdma3.read_byte(addr),
            MemRegister::HDMA4 => self.hdma4.read_byte(addr),
            MemRegister::HDMA5 => {
                if self.is_active() {
                    use std::convert::TryFrom;
                    u8::try_from(self.remain / 10 - 1).unwrap()
                } else {
                    0xff
                }
            }
            _ => panic!("invalid hdma address"),
        }
    }
}
