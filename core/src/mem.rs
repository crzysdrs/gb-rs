use crate::cycles;
use crate::peripherals::{Addressable, Peripheral};
use std::ops::Deref;

pub struct Mem {
    read_only: bool,
    base: u16,
    mem: Vec<u8>,
}

impl Mem {
    pub fn new(read_only: bool, base: u16, mem: Vec<u8>) -> Mem {
        Mem {
            read_only,
            base,
            mem,
        }
    }
    fn offset(&self, addr: u16) -> Option<usize> {
        if addr >= self.base && (addr as usize) < self.mem.len() + self.base as usize {
            Some((addr - self.base) as usize)
        } else {
            None
        }
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        if let Some(offset) = self.offset(addr) {
            &mut self.mem[offset]
        } else {
            panic!(
                "Outside of range access Addr: {:x} Base: {:x}",
                addr, self.base
            );
        }
    }
    pub fn set_readonly(&mut self, readonly: bool) {
        self.read_only = readonly;
    }
}

impl Deref for Mem {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.mem
    }
}

impl Peripheral for Mem {
    fn next_step(&self) -> Option<cycles::CycleCount> {
        /* Memory does not generate any interrupts and only needs to
        be updated when observed */
        Some(cycles::CycleCount::new(std::u64::MAX))
    }
}

impl Addressable for Mem {
    fn write_byte(&mut self, addr: u16, val: u8) {
        if !self.read_only {
            *self.lookup(addr) = val;
            self.wrote(addr, val);
        }
    }
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
}
