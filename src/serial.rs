use super::mmu::MemRegister;
use enum_primitive::FromPrimitive;
use peripherals::Peripheral;
use std::io::Write;

pub struct Serial<'a> {
    sb: u8,
    sc: u8,
    out: Option<&'a mut Write>,
}

impl<'a> Serial<'a> {
    pub fn new<'b>(out: Option<&'b mut Write>) -> Serial<'b> {
        Serial { sb: 0, sc: 0, out }
    }
}
impl<'a> Peripheral for Serial<'a> {
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::SB => &mut self.sb,
            MemRegister::SC => &mut self.sc,
            _ => panic!("Unhandled register in serial"),
        }
    }
    fn step(&mut self, _time: u64) {
        //TODO: Wait appropriate amount of time to send serial data.
        if (self.sc & 0x80) != 0 {
            if let Some(ref mut o) = self.out {
                o.write_all(&[self.sb])
                    .expect("Failed to write to serial output file");
            }
            self.sc &= !0x80;
        }
    }
}
