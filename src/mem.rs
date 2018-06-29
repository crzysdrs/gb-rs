use peripherals::Peripheral;
use cpu::InterruptFlag;

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
}

impl Peripheral for Mem {
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        if addr >= self.base && (addr as usize) < self.mem.len() + self.base as usize {
            &mut self.mem[(addr - self.base) as usize]
        } else {
            panic!(
                "Outside of range access Addr: {:x} Base: {:x}",
                addr, self.base
            );
        }
    }
    fn write(&mut self, addr: u16, val: u8) {
        if !self.read_only {
            *self.lookup(addr) = val;
        }
    }
    fn step(&mut self, _time: u64) -> Option<InterruptFlag> {
        None
    }
}
