use super::mmu::MemRegister;
use crate::mmu::MMU;
use crate::peripherals::{Addressable, Peripheral};
use enum_primitive::FromPrimitive;

pub struct DMA {
    active: bool,
    dma: u8,
}

impl DMA {
    pub fn is_active(&mut self) -> bool {
        self.active
    }
    pub fn run(&mut self, mem: &mut MMU) {
        if self.active {
            let source = (self.dma as u16) << 8;
            let target = 0xfe00;
            let len = 0xA0;
            for (s, t) in (source..source + len).zip(target..target + len) {
                let b = mem.read_byte(s);
                mem.write_byte(t, b);
            }
        }
        self.active = false;
    }
}

impl Peripheral for DMA {}
impl Addressable for DMA {
    fn write_byte(&mut self, addr: u16, val: u8) {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::DMA => {
                self.dma = val;
                self.active = true;
            }
            _ => panic!("invalid dma address"),
        }
    }
    fn read_byte(&mut self, _addr: u16) -> u8 {
        0
    }
}

impl DMA {
    pub fn new() -> DMA {
        DMA {
            active: false,
            dma: 0,
        }
    }
}
