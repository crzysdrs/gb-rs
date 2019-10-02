use super::mmu::MemRegister;
use crate::cpu::InterruptFlag;
use crate::cycles;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use enum_primitive::FromPrimitive;
use std::io::Write;

pub struct Serial<'a> {
    sb: u8,
    sc: u8,
    out: Option<&'a mut dyn Write>,
}

impl<'a> Serial<'a> {
    pub fn new(out: Option<&mut dyn Write>) -> Serial<'_> {
        Serial { sb: 0, sc: 0, out }
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::SB => &mut self.sb,
            MemRegister::SC => &mut self.sc,
            _ => panic!("Unhandled register in serial"),
        }
    }
}

impl<'a> Addressable for Serial<'a> {
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        *self.lookup(addr) = v;
    }
}
impl<'a> Peripheral for Serial<'a> {
    fn step(
        &mut self,
        _real: &mut PeripheralData,
        _time: cycles::CycleCount,
    ) -> Option<InterruptFlag> {
        if (self.sc & 0x80) != 0 {
            //TODO: Wait appropriate amount of time to send serial data.
            if let Some(ref mut o) = self.out {
                o.write_all(&[self.sb])
                    .expect("Failed to write to serial output file");
            }
            self.sc &= !0x80;
            self.sb = 0;
            //Some(InterruptFlag::Serial)
            None
        } else {
            None
        }
    }
}
