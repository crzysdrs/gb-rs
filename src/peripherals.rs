use crate::cpu::InterruptFlag;
use crate::cycles;
#[cfg(feature = "vcd_dump")]
use crate::VCDDump::VCD;

pub struct AudioSpec<'a, T: 'a> {
    pub queue: Box<&'a mut dyn FnMut(&[T]) -> bool>,
    pub freq: u32,
    pub silence: T,
}

pub struct PeripheralData<'a> {
    pub lcd: Option<&'a mut [u8]>,
    pub audio_spec: Option<AudioSpec<'a, i16>>,
    pub vblank: bool,
}

impl<'a> PeripheralData<'a> {
    pub fn empty() -> PeripheralData<'a> {
        PeripheralData {
            lcd: None,
            audio_spec: None,
            vblank: false,
        }
    }
    pub fn reset_vblank(&mut self) {
        self.vblank = false;
    }
    // pub fn test() -> (Vec<u8>, Box<FnMut(&[i16]) -> bool>, PeripheralData<'a>) {
    //     let v = vec![0u8; 166 * 144];
    //     let func = |_x| { true };

    //     (v, Box::new(func),
    //      PeripheralData {
    //          lcd: Some(&mut v),
    //          audio_spec: Some(AudioSpec {
    //              queue: Box::new(&mut func),
    //              freq: 16384 * 4,
    //              silence: 0,
    //          })
    //      }
    //      )
    // }
    pub fn new(
        lcd: Option<&'a mut [u8]>,
        audio_spec: Option<AudioSpec<'a, i16>>,
    ) -> PeripheralData<'a> {
        PeripheralData {
            lcd,
            audio_spec,
            vblank: false,
        }
    }
}

pub trait Peripheral: Addressable {
    fn step(
        &mut self,
        _real: &mut PeripheralData,
        _time: cycles::CycleCount,
    ) -> Option<InterruptFlag> {
        None
    }
}

pub trait Addressable {
    fn read_byte(&mut self, addr: u16) -> u8;
    fn write_byte(&mut self, addr: u16, v: u8);

    #[allow(unused_variables)]
    fn wrote(&mut self, addr: u16, _v: u8) {
        #[cfg(feature = "vcd_dump")]
        {
            let read = self.read_byte(addr); // just in case it modified the value
            VCD.as_ref().map(|m| {
                m.lock().unwrap().as_mut().map(|vcd| {
                    let (mut writer, mem) = vcd.writer();
                    if let Some((wire, id)) =
                        mem.get(&std::borrow::Cow::Owned(format!("0x{:04x}", addr)))
                    {
                        wire.write(&mut writer, *id, read as u64);
                    }
                })
            });
        }
    }
}
