use super::{AudioChannel, Clocks};
use mmu::MemRegister;
use std::ops::{Deref, DerefMut};

use sound::channel::{
    ChannelRegs, Freq, HasRegs, LFSRPass, Length, LengthPass, Timer, Vol, VolumePass, LFSR,
};

pub struct Channel4 {
    enabled: bool,
    regs: Channel4Regs,
    vol: Vol,
    timer: Timer,
    length: Length<u8>,
    lfsr: LFSR,
}

impl Channel4 {
    pub fn new() -> Channel4 {
        Channel4 {
            regs: Channel4Regs(ChannelRegs::new(
                MemRegister::NR40 as u16,
                &[0xff, 0xff, 0x00, 0x00, 0xbf],
            )),
            vol: Vol::new(),
            timer: Timer::new(),
            length: Length::new(),
            lfsr: LFSR::new(),
            enabled: false,
        }
    }
}

impl AudioChannel for Channel4 {
    fn regs(&mut self) -> &mut ChannelRegs {
        &mut self.regs
    }
    fn reset(&mut self) {
        self.timer.reset();
        self.length.reset();
        self.lfsr.reset();
        self.vol.reset();
        self.enabled = true;
    }
    fn disable(&mut self) {
        self.enabled = false;
    }
    fn off(&mut self) {
        self.regs.reset();
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
            .step(LFSRPass::period(&self.regs) as u16, &mut self.regs, clocks);
        self.length.step(&mut self.regs, clocks)?;
        let high = self.lfsr.step(ticks, &mut self.regs, clocks);
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

struct Channel4Regs(ChannelRegs);
impl Deref for Channel4Regs {
    type Target = ChannelRegs;
    fn deref(&self) -> &ChannelRegs {
        &self.0
    }
}
impl DerefMut for Channel4Regs {
    fn deref_mut(&mut self) -> &mut ChannelRegs {
        &mut self.0
    }
}
impl HasRegs for Channel4Regs {}
impl Freq for Channel4Regs {}
impl LFSRPass for Channel4Regs {}
impl VolumePass for Channel4Regs {}
impl LengthPass<u8> for Channel4Regs {
    fn length(&self) -> u8 {
        self.deref().length()
    }
}
