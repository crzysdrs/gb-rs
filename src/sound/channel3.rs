use super::{AudioChannel, Clocks};
use crate::cycles;
use crate::mmu::MemRegister;
use crate::peripherals::Addressable;
use crate::sound::channel::{
    AddressableChannel, ChannelRegs, Freq, HasRegs, Length, LengthPass, Timer, VolumeCode,
};

use std::ops::{Deref, DerefMut};

pub struct Channel3 {
    enabled: bool,
    regs: Channel3Regs,
    timer: Timer,
    length: Length,
    pos: Option<usize>,
}

impl Channel3 {
    pub fn new() -> Channel3 {
        Channel3 {
            regs: Channel3Regs(ChannelRegs::new(
                MemRegister::NR30 as u16,
                [0x7f, 0xff, 0x9f, 0xff, 0xbf],
            )),
            timer: Timer::new(),
            length: Length::new(256),
            pos: None,
            enabled: false,
        }
    }
}

impl AudioChannel for Channel3 {
    fn regs(&mut self) -> &mut ChannelRegs {
        &mut self.regs
    }
    fn reset(&mut self, clks: &Clocks, enable: bool, trigger: bool) {
        self.timer.reset();
        self.length.update(clks, enable, trigger);
        self.pos = None;
        self.enabled = true;
    }
    fn disable(&mut self) {
        self.enabled = false;
    }
    fn power(&mut self, power: bool) {
        self.regs.power(power);
    }
    fn sample(&mut self, wave: &[u8], cycles: cycles::CycleCount, clocks: &Clocks) -> Option<i16> {
        if !self.enabled {
            self.enabled = false;
            self.length.step(clocks)?;
            return None;
        }
        let ticks = self
            .timer
            .step(Freq::period(&self.regs) * 2, cycles, clocks);
        let len = self.length.step(clocks);
        if self.regs.length_stop() {
            len?;
        }
        let pos = self.pos.get_or_insert(0);
        *pos += ticks as usize;
        *pos %= 32;
        let byte = wave[*pos / 2];
        let (lo, hi) = (byte & 0xf, (byte & 0xf0) >> 4);
        let sample = i16::from(if *pos % 2 == 0 { hi } else { lo });
        match self.regs.vol_code() {
            0 => Some(0),
            1 => Some(sample),
            2 => Some(sample >> 1),
            3 => Some(sample >> 2),
            _ => unreachable!("Bad Volume Code"),
        }
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl AddressableChannel for Channel3 {
    fn read_channel_byte(&mut self, _clks: &Clocks, addr: u16) -> u8 {
        self.regs().read_byte(addr)
    }
    fn write_channel_byte(&mut self, clks: &Clocks, addr: u16, v: u8) {
        //println!("Write to Channel3 {:x} {:x}", addr, v);
        self.regs().write_byte(addr, v);
        match addr {
            0xff1a => {
                if !self.regs().dac_enabled(true) {
                    self.enabled = false
                }
            }
            0xff1b => self.length.reload(self.regs.length(self.length.max_len())),
            0xff1e => {
                match v & 0xc0 {
                    0xC0 => self.reset(clks, true, true),
                    0x80 => self.reset(clks, false, true),
                    0x40 => self.length.update(clks, true, false),
                    _ => self.length.update(clks, false, false),
                }
                if v & 0x80 != 0 {
                    self.enabled = self.regs.dac_enabled(true);
                }
            }
            _ => {}
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
impl LengthPass for Channel3Regs {}
impl VolumeCode for Channel3Regs {}
