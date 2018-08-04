use cpu::InterruptFlag;

pub struct AudioSpec<'a, T: 'a> {
    pub queue: Box<&'a mut FnMut(&[T]) -> bool>,
    pub freq: u32,
    pub silence: T,
}

pub struct PeripheralData<'a> {
    pub lcd: Option<&'a mut [u8]>,
    pub audio_spec: Option<AudioSpec<'a, i16>>,
}

impl PeripheralData<'a> {
    pub fn empty() -> PeripheralData<'a> {
        PeripheralData {
            lcd: None,
            audio_spec: None,
        }
    }
    pub fn new(
        lcd: Option<&'a mut [u8]>,
        audio_spec: Option<AudioSpec<'a, i16>>,
    ) -> PeripheralData<'a> {
        PeripheralData { lcd, audio_spec }
    }
}

pub trait Peripheral {
    fn read_byte(&mut self, addr: u16) -> u8;
    fn write_byte(&mut self, addr: u16, v: u8);
    fn step(&mut self, _real: &mut PeripheralData, _time: u64) -> Option<InterruptFlag> {
        None
    }
}
