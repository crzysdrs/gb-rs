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

impl Peripheral for EmptyMem {}

impl Addressable for EmptyMem {
    fn write_byte(&mut self, addr: u16, _val: u8) {
        assert!(addr >= self.base && addr <= self.base + self.len);
    }
    fn read_byte(&mut self, addr: u16) -> u8 {
        assert!(addr >= self.base && addr <= self.base + self.len);
        self.default
    }
}
