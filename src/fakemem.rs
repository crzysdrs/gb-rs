use peripherals::Peripheral;
use cpu::InterruptFlag;

pub struct FakeMem {
    byte: u8,
}

impl FakeMem {
    pub fn new() -> FakeMem {
        FakeMem { byte: 0 }
    }
}
impl Peripheral for FakeMem {
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        println!("Attempting lookup of unhandled address {:x}", addr);
        &mut self.byte
    }
    fn write(&mut self, addr: u16, val: u8) {
        println!(
            "Attempting write to unhandled address {:x}, value {:x}",
            addr, val
        );
    }
    fn read(&mut self, addr: u16) -> u8 {
        println!("Attempting read from unhandled address {:x}", addr);
        0
    }
    fn step(&mut self, _time: u64) -> Option<InterruptFlag> {
        None
    }
}
