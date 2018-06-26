pub trait Peripheral {
    fn lookup(&mut self, addr: u16) -> &mut u8;
    fn read(&mut self, addr : u16) -> u8 {
        *self.lookup(addr)
    }
    fn write(&mut self, addr : u16, v : u8) {
        *self.lookup(addr) = v;
    }
    fn step(&mut self, time : u64);
}
