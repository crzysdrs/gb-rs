use super::{AudioChannel, Clocks};
use std::ops::{Deref, DerefMut};

use sound::channel::{ChannelRegs, Freq, HasRegs, Length, LengthPass, Timer, VolumeCode};

pub struct Channel3 {
    enabled: bool,
    regs: Channel3Regs,
    timer: Timer,
    length: Length<u16>,
    pos: Option<usize>,
}

impl Channel3 {
    pub fn new() -> Channel3 {
        Channel3 {
            regs: Channel3Regs(ChannelRegs::new()),
            timer: Timer::new(),
            length: Length::new(),
            pos: None,
            enabled: false,
        }
    }
}

impl AudioChannel for Channel3 {
    fn regs(&mut self) -> &mut ChannelRegs {
        &mut self.regs
    }
    fn reset(&mut self) {
        self.timer.reset();
        self.length.reset();
        self.pos = None;
        self.enabled = true;
    }
    fn disable(&mut self) {
        self.enabled = false;
    }
    fn sample(&mut self, wave: &[u8], clocks: &Clocks) -> Option<i16> {
        if !self.regs.wave_enabled() {
            self.enabled = false;
            return None;
        } else if !self.enabled && !self.regs.trigger() {
            return None;
        } else if self.regs.trigger() {
            self.regs.clear_trigger();
            self.reset();
        }
        let ticks = self
            .timer
            .step(Freq::period(&self.regs) / 2, &mut self.regs, clocks);
        self.length.step(&mut self.regs, clocks)?;
        let pos = self.pos.get_or_insert(0);
        *pos += ticks as usize;
        *pos %= 32;
        let byte = wave[*pos / 2];
        let (lo, hi) = (byte & 0xf, (byte & 0xf0) >> 4);
        let sample = (if *pos % 2 == 0 { lo } else { hi }) as i16;
        match self.regs.vol_code() {
            0 => Some(0),
            1 => Some(sample),
            2 => Some(sample >> 1),
            3 => Some(sample >> 2),
            _ => unreachable!("Bad Volume Code"),
        }
    }
}

struct Channel3Regs(ChannelRegs);

impl HasRegs for Channel3Regs {}
impl Deref for Channel3Regs {
    type Target = ChannelRegs;
    fn deref(&self) -> &ChannelRegs {
        &self.0
    }
}
impl DerefMut for Channel3Regs {
    fn deref_mut(&mut self) -> &mut ChannelRegs {
        &mut self.0
    }
}

impl Freq for Channel3Regs {}
impl LengthPass<u16> for Channel3Regs {
    fn length(&self) -> u16 {
        self.deref().length()
    }
}
impl VolumeCode for Channel3Regs {}
