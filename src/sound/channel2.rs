use super::{AudioChannel, Clocks};
use mmu::MemRegister;
use std::ops::{Deref, DerefMut};

use sound::channel::{
    ChannelRegs, Duty, DutyPass, Freq, HasRegs, Length, LengthPass, Timer, Vol, VolumePass,
};

pub struct Channel2 {
    enabled: bool,
    regs: Channel2Regs,
    vol: Vol,
    timer: Timer,
    length: Length<u8>,
    duty: Duty,
}

impl Channel2 {
    pub fn new() -> Channel2 {
        Channel2 {
            regs: Channel2Regs(ChannelRegs::new(
                MemRegister::NR20 as u16,
                &[0xff, 0x3f, 0x00, 0xff, 0xbf],
            )),
            vol: Vol::new(),
            timer: Timer::new(),
            length: Length::new(),
            duty: Duty::new(),
            enabled: false,
        }
    }
}

impl AudioChannel for Channel2 {
    fn regs(&mut self) -> &mut ChannelRegs {
        &mut self.regs
    }
    fn disable(&mut self) {
        self.enabled = false;
    }
    fn off(&mut self) {
        self.regs.reset();
    }
    fn reset(&mut self) {
        self.timer.reset();
        self.duty.reset();
        self.length.reset();
        self.vol.reset();
        self.enabled = true;
    }
    fn sample(&mut self, _wave: &[u8], clocks: &Clocks) -> Option<i16> {
        if !self.enabled && !self.regs.trigger() {
            return None;
        } else if self.regs.trigger() {
            self.regs.clear_trigger();
            self.reset();
        }
        let ticks = self
            .timer
            .step(Freq::period(&self.regs), &mut self.regs, clocks);
        let high = self.duty.step(&mut self.regs, ticks);
        self.length.step(&mut self.regs, clocks)?;
        let vol = self.vol.step(&mut self.regs, clocks);
        if high {
            Some(vol as i16)
        } else {
            Some(-(vol as i16))
        }
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

struct Channel2Regs(ChannelRegs);

impl HasRegs for Channel2Regs {}
impl Deref for Channel2Regs {
    type Target = ChannelRegs;
    fn deref(&self) -> &ChannelRegs {
        &self.0
    }
}
impl DerefMut for Channel2Regs {
    fn deref_mut(&mut self) -> &mut ChannelRegs {
        &mut self.0
    }
}

impl Freq for Channel2Regs {}
impl DutyPass for Channel2Regs {}
impl VolumePass for Channel2Regs {}
impl LengthPass<u8> for Channel2Regs {
    fn length(&self) -> u8 {
        self.deref().length()
    }
}
