use cpu::InterruptFlag;

pub trait Peripheral {
    fn read_byte(&mut self, addr: u16) -> u8;
    fn write_byte(&mut self, addr: u16, v: u8);
    fn step(&mut self, _time: u64) -> Option<InterruptFlag> {
        None
    }
}
