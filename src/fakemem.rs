use crate::cpu::InterruptFlag;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};

pub struct FakeMem {}

impl FakeMem {
    pub fn new() -> FakeMem {
        FakeMem {}
    }
}
impl Addressable for FakeMem {
    fn write_byte(&mut self, addr: u16, val: u8) {
        println!(
            "Attempting write to unhandled address {:x}, value {:x}",
            addr, val
        );
    }
    fn read_byte(&mut self, addr: u16) -> u8 {
        println!("Attempting read from unhandled address {:x}", addr);
        0
    }
}
impl Peripheral for FakeMem {
    fn step(&mut self, _real: &mut PeripheralData, _time: u64) -> Option<InterruptFlag> {
        None
    }
}
