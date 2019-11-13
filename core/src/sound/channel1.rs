use super::{AudioChannel, Clocks};
use crate::cycles;
use crate::mmu::MemRegister;
use crate::peripherals::Addressable;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};

use crate::sound::channel::{
    AddressableChannel, ChannelRegs, Duty, DutyPass, Freq, HasRegs, Length, LengthPass, Sweep,
    SweepPass, Timer, Vol, VolumePass,
};
#[derive(Serialize, Deserialize, Clone)]
pub struct Channel1 {
    enabled: bool,
    regs: Channel1Regs,
    vol: Vol,
    timer: Timer,
    sweep: Sweep,
    length: Length,
    duty: Duty,
}

impl Channel1 {
    pub fn new() -> Channel1 {
        Channel1 {
            regs: Channel1Regs(ChannelRegs::new(
                MemRegister::NR10 as u16,
                [0x80, 0x3f, 0x00, 0xff, 0xbf],
            )),
            vol: Vol::new(),
            timer: Timer::new(),
            sweep: Sweep::new(),
            length: Length::new(64),
            duty: Duty::new(),
            enabled: false,
        }
    }
}

// use cycles::{Cycles, CycleCount};

// trait AudioModifier {
//     fn sample(&mut self) -> Option<i16>;
//     fn next_change(&self) -> CycleCount;
// }

// struct ConstantAudio {
//     value: i16,
// }

// impl AudioModifier for ConstantAudio {
//     fn sample(&mut self) -> Option<i16> {
//         Some(self.value)
//     }
//     fn next_change(&self) -> CycleCount {
//         Cycles::new(std::u64::MAX)
//     }
// }
// impl ConstantAudio {
//     fn new(audio: i16) -> ConstantAudio {
//         ConstantAudio {
//             value: audio,
//         }
//     }
//     fn update(&mut self, audio: i16) {
//         self.value = audio;
//     }
// }

// struct SquareWave<T> {
//     inner : T,
//     toggle: CycleCount,
//     pass_through: bool,
//     period: CycleCount,
// }

// impl SquareWave<T>
//     where T: AudioModifier
// {
//     fn new(freq: u32) -> SquareWave {
//         SquareWave {
//             period: SECOND / freq,
//             pass_through : true,
//             toggle: cycles::from(0),

//         }
//     }
// }

// impl <T> AudioModifier for SquareWave<T> {
//     fn sample(&mut self) -> Option<i16> {
//         if self.pass_through {
//             Some(self.inner.sample())
//         } else {
//             None
//         }
//     }
//     fn next_change(&self) -> CycleCount {
//         if self.pass_through {
//             self.inner.next_change()
//         } else {
//             self.period + self.last_toggle
//         }
//     }
// }

impl AudioChannel for Channel1 {
    fn regs(&mut self) -> &mut ChannelRegs {
        &mut self.regs
    }
    fn disable(&mut self) {
        println!("Channel Disabled By Other Means");
        self.enabled = false;
    }
    fn power(&mut self, power: bool) {
        self.regs.power(power);
    }
    fn reset(&mut self, enable: bool, trigger: bool) {
        self.sweep.reset();
        self.timer.reset();
        self.duty.reset();
        self.length.update(enable, trigger);
        self.vol.reset();
        self.enabled = true;
    }
    fn sample(&mut self, _wave: &[u8], cycles: cycles::CycleCount, clocks: &Clocks) -> Option<i16> {
        if !self.enabled {
            self.length.step(clocks)?;
            return None;
        }
        self.length.step(clocks)?;
        self.sweep.step(&mut self.regs, clocks)?;
        let ticks = self.timer.step(Freq::period(&self.regs), cycles, clocks);
        let high = self.duty.step(&mut self.regs, ticks);
        let vol = self.vol.step(&mut self.regs, clocks);
        if high {
            Some(i16::from(vol))
        } else {
            Some(-i16::from(vol))
        }
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl AddressableChannel for Channel1 {
    fn read_channel_byte(&mut self, addr: u16) -> u8 {
        self.regs().read_byte(addr)
    }
    fn write_channel_byte(&mut self, addr: u16, v: u8) {
        self.regs().write_byte(addr, v);
        //println!("Write to Channel1 {:x} {:x}", addr, v);
        match addr {
            0xff11 => {
                self.length.reload(self.regs.length(self.length.max_len()));
            }
            0xff12 => {
                if !self.regs().dac_enabled(false) {
                    self.enabled = false
                }
            }
            0xff14 => {
                match v & 0xc0 {
                    0xC0 => self.reset(true, true),
                    0x80 => self.reset(false, true),
                    0x40 => self.length.update(true, false),
                    _ => self.length.update(false, false),
                }
                if v & 0x80 != 0 {
                    self.enabled = self.regs.dac_enabled(false);
                }
            }
            _ => {}
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Channel1Regs(ChannelRegs);
impl Deref for Channel1Regs {
    type Target = ChannelRegs;
    fn deref(&self) -> &ChannelRegs {
        &self.0
    }
}
impl DerefMut for Channel1Regs {
    fn deref_mut(&mut self) -> &mut ChannelRegs {
        &mut self.0
    }
}
impl HasRegs for Channel1Regs {}
impl Freq for Channel1Regs {}
impl DutyPass for Channel1Regs {}
impl VolumePass for Channel1Regs {}
impl LengthPass for Channel1Regs {}
impl SweepPass for Channel1Regs {}
