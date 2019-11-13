use super::mmu::MemRegister;
use crate::cpu::Interrupt;
use crate::cycles;
use crate::hdma::Copier;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use crate::sound::WaitTimer;

use enum_primitive::FromPrimitive;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct DMA {
    active: bool,
    copy: Copier,
    wait: WaitTimer,
}

impl DMA {
    pub fn new() -> DMA {
        DMA {
            active: false,
            copy: Copier::new(0, 0, 0),
            wait: WaitTimer::new(),
        }
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn copy_bytes(&mut self) -> &mut Copier {
        &mut self.copy
    }
}

impl Peripheral for DMA {
    fn next_step(&self) -> Option<cycles::CycleCount> {
        if self.is_active() {
            Some(self.wait.next_ready(cycles::GB))
        } else {
            Some(cycles::CycleCount::new(std::u64::MAX))
        }
    }
    fn step(&mut self, _real: &mut PeripheralData, time: cycles::CycleCount) -> Option<Interrupt> {
        if !self.is_active() {
            /* do nothing */
        } else if let Some(_) = self
            .wait
            .ready(time, /* TODO: It's faster in CGB Mode */ cycles::GB)
        {
            if self.copy.empty() {
                self.active = false;
            }
        }
        None
    }
}
impl Addressable for DMA {
    fn write_byte(&mut self, addr: u16, val: u8) {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::DMA => {
                self.active = true;
                let source = (u16::from(val)) << 8;
                let target = 0xfe00;
                let len = 0xA0;
                self.wait.reset();
                self.copy = Copier::new(source, target, len);
            }
            _ => panic!("invalid dma address"),
        }
    }
    fn read_byte(&mut self, _addr: u16) -> u8 {
        0
    }
}
