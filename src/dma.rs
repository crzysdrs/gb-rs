use super::mmu::MemRegister;
use crate::cpu::InterruptFlag;
use crate::cycles;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use crate::sound::WaitTimer;
use enum_primitive::FromPrimitive;

pub struct DMA {
    active: bool,
    copy_queue: Option<Box<dyn Iterator<Item = (u16, u16)>>>,
    copy: Vec<(u16, u16)>,
    wait: WaitTimer,
}

impl DMA {
    pub fn new() -> DMA {
        DMA {
            active: false,
            copy_queue: None,
            copy: Vec::new(),
            wait: WaitTimer::new(),
        }
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn copy_bytes(&mut self) -> Vec<(u16, u16)> {
        let r = self.copy.clone();
        self.copy.clear();
        r
    }
}

impl Peripheral for DMA {
    fn step(
        &mut self,
        _real: &mut PeripheralData,
        time: cycles::CycleCount,
    ) -> Option<InterruptFlag> {
        if !self.is_active() {
            /* do nothing */
        } else if let (Some(c), Some(q)) = (
            self.wait
                .ready(time, /* TODO: It's faster in CGB Mode */ cycles::GB),
            self.copy_queue.as_mut(),
        ) {
            use std::convert::TryFrom;
            let start_len = self.copy.len();
            self.copy.extend(q.take(usize::try_from(c).unwrap()));
            if self.copy.len() == start_len {
                self.active = false;
                self.copy_queue = None;
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
                self.copy_queue = Some(Box::new((source..source + len).zip(target..target + len)));
            }
            _ => panic!("invalid dma address"),
        }
    }
    fn read_byte(&mut self, _addr: u16) -> u8 {
        0
    }
}
