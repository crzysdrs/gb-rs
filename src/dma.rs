use super::mmu::MemRegister;
use enum_primitive::FromPrimitive;
use peripherals::Peripheral;
use cpu::InterruptFlag;

pub struct DMA {
    active : bool,
    cur_addr : u16,
    dma : u8,
}

impl Peripheral for DMA {
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::DMA => &mut self.dma,
            _ => panic!("invalid dma address"),
        }
    }
    fn step(&mut self, time: u64) -> Option<InterruptFlag> {

    }
}

impl DMA {
    pub fn new() -> DMA {
        DMA {
            active: false,
            start_addr : 0,
            dma : 0,
        }
    }
}
