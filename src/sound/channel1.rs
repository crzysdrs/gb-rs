use super::{AudioChannel, Clocks};
use std::ops::{Deref, DerefMut};

use sound::channel::{
    ChannelRegs, Duty, DutyPass, Freq, HasRegs, Length, LengthPass, Sweep, SweepPass, Timer, Vol,
    VolumePass,
};

pub struct Channel1 {
    enabled: bool,
    regs: Channel1Regs,
    vol: Vol,
    timer: Timer,
    sweep: Sweep,
    length: Length<u8>,
    duty: Duty,
}

impl Channel1 {
    pub fn new() -> Channel1 {
        Channel1 {
            regs: Channel1Regs(ChannelRegs::new()),
            vol: Vol::new(),
            timer: Timer::new(),
            sweep: Sweep::new(),
            length: Length::new(),
            duty: Duty::new(),
            enabled: false,
        }
    }
}

impl AudioChannel for Channel1 {
    fn regs(&mut self) -> &mut ChannelRegs {
        &mut self.regs
    }
    fn disable(&mut self) {
        self.enabled = false;
    }
    fn reset(&mut self) {
        self.sweep.reset();
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
        self.sweep.step(&mut self.regs, clocks)?;
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
}

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
impl LengthPass<u8> for Channel1Regs {
    fn length(&self) -> u8 {
        self.deref().length()
    }
}
impl SweepPass for Channel1Regs {}
