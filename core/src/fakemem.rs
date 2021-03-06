use crate::cpu::Interrupt;
use crate::cycles;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
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
    fn next_step(&self) -> Option<cycles::CycleCount> {
        /* Memory does not generate any interrupts and only needs to
        be updated when observed */
        Some(cycles::CycleCount::new(std::u64::MAX))
    }

    fn step(&mut self, _real: &mut PeripheralData, _time: cycles::CycleCount) -> Option<Interrupt> {
        None
    }
}
