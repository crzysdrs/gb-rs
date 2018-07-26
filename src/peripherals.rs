use cpu::InterruptFlag;

pub struct PeripheralData<'a> {
    pub lcd: Option<&'a mut [u8]>,
}

impl PeripheralData<'a> {
    pub fn empty() -> PeripheralData<'a> {
        PeripheralData { lcd: None }
    }
    pub fn new(lcd: Option<&'a mut [u8]>) -> PeripheralData<'a> {
        PeripheralData { lcd }
    }
}

pub trait Peripheral {
    fn read_byte(&mut self, addr: u16) -> u8;
    fn write_byte(&mut self, addr: u16, v: u8);
    fn step(&mut self, _real: &mut PeripheralData, _time: u64) -> Option<InterruptFlag> {
        None
    }
}
