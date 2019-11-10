use crate::cycles;
use crate::peripherals::{Addressable, Peripheral};

pub struct EmptyMem {
    default: u8,
    base: u16,
    len: u16,
}

impl EmptyMem {
    pub fn new(default: u8, base: u16, len: u16) -> EmptyMem {
        EmptyMem { base, default, len }
    }
}

impl Peripheral for EmptyMem {
    fn next_step(&self) -> Option<cycles::CycleCount> {
        /* Memory does not generate any interrupts and only needs to
        be updated when observed */
        Some(cycles::CycleCount::new(std::u64::MAX))
    }
}

impl Addressable for EmptyMem {
    fn write_byte(&mut self, addr: u16, _val: u8) {
        assert!(addr >= self.base && addr <= self.base + self.len);
    }
    fn read_byte(&mut self, addr: u16) -> u8 {
        assert!(addr >= self.base && addr <= self.base + self.len);
        self.default
    }
}
