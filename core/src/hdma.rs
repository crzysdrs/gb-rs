use super::mmu::MemRegister;
use crate::cpu::InterruptFlag;
use crate::cycles;
use crate::mmu::MemReg;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use crate::sound::WaitTimer;
use enum_primitive::FromPrimitive;

pub struct HDMA {
    copy_queue: Option<Box<dyn Iterator<Item = (u16, u16)>>>,
    copy: Vec<(u16, u16)>,
    wait: WaitTimer,

    hdma1: MemReg,
    hdma2: MemReg,
    hdma3: MemReg,
    hdma4: MemReg,
    hdma5: MemReg,
}

impl HDMA {
    pub fn new() -> HDMA {
        HDMA {
            copy_queue: None,
            copy: Vec::new(),
            wait: WaitTimer::new(),
            hdma1: MemReg::default(),
            hdma2: MemReg::default(),
            hdma3: MemReg::default(),
            hdma4: MemReg::default(),
            hdma5: MemReg::new(0b1000_0000, 0xff, 0xff),
        }
    }
    pub fn is_active(&self) -> bool {
        self.hdma5.reg() == 0
    }
    pub fn copy_bytes(&mut self) -> Vec<(u16, u16)> {
        let r = self.copy.clone();
        self.copy.clear();
        r
    }
}

impl Peripheral for HDMA {
    fn step(
        &mut self,
        _real: &mut PeripheralData,
        time: cycles::CycleCount,
    ) -> Option<InterruptFlag> {
        if !self.is_active() {
            /* do nothing */
        } else if let (Some(c), Some(q)) =
            (self.wait.ready(time, cycles::CGB), self.copy_queue.as_mut())
        {
            use std::convert::TryFrom;
            let start_len = self.copy.len();
            self.copy.extend(q.take(16 * usize::try_from(c).unwrap()));
            if self.copy.len() == start_len {
                self.hdma5.write_byte(0, 0b1000_0000);
                self.copy_queue = None;
            }
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
                let _hblank_dma = (val & 0b1000_000) != 0;
                let len = (u16::from(val & 0b0111_1111) + 1) * 0x10;
                let source = u16::from_le_bytes([self.hdma2.reg(), self.hdma1.reg()]) & 0xfff0;
                let dest =
                    (u16::from_le_bytes([self.hdma4.reg(), self.hdma3.reg()]) & 0x1ff0) | 0x8000;
                self.wait.reset();
                self.copy_queue = Some(Box::new((source..source + len).zip(dest..dest + len)));
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
            MemRegister::HDMA5 => self.hdma4.read_byte(addr),
            _ => panic!("invalid hdma address"),
        }
    }
}