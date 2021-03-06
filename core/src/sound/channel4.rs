use super::{AudioChannel, Clocks};
use crate::cycles;
use crate::mmu::MemRegister;
use crate::peripherals::Addressable;
use crate::sound::channel::{
    AddressableChannel, ChannelRegs, Freq, HasRegs, LFSRPass, Length, LengthPass, Timer, Vol,
    VolumePass, LFSR,
};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
#[derive(Serialize, Deserialize, Clone)]
pub struct Channel4 {
    enabled: bool,
    regs: Channel4Regs,
    vol: Vol,
    timer: Timer,
    length: Length,
    lfsr: LFSR,
}

impl Channel4 {
    pub fn new() -> Channel4 {
        Channel4 {
            regs: Channel4Regs(ChannelRegs::new(
                MemRegister::NR40 as u16,
                [0xff, 0xff, 0x00, 0x00, 0xbf],
            )),
            vol: Vol::new(),
            timer: Timer::new(),
            length: Length::new(64),
            lfsr: LFSR::new(),
            enabled: false,
        }
    }
}

impl AudioChannel for Channel4 {
    fn regs(&mut self) -> &mut ChannelRegs {
        &mut self.regs
    }
    fn reset(&mut self, enable: bool, trigger: bool) {
        self.timer.reset();
        self.length.update(enable, trigger);
        self.lfsr.reset();
        self.vol.reset();
        self.enabled = true;
    }
    fn disable(&mut self) {
        self.enabled = false;
    }
    fn power(&mut self, power: bool) {
        self.regs.power(power);
    }
    fn sample(&mut self, _wave: &[u8], cycles: cycles::CycleCount, clocks: &Clocks) -> Option<i16> {
        if !self.enabled {
            self.enabled = false;
            self.length.step(clocks)?;
            return None;
        }
        let ticks = self
            .timer
            .step(u16::from(LFSRPass::period(&self.regs)), cycles, clocks);
        self.length.step(clocks)?;
        let high = self.lfsr.step(ticks, &mut self.regs, clocks);
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

impl AddressableChannel for Channel4 {
    fn read_channel_byte(&mut self, addr: u16) -> u8 {
        self.regs().read_byte(addr)
    }
    fn write_channel_byte(&mut self, addr: u16, v: u8) {
        //println!("Write to Channel4 {:x} {:x}", addr, v);
        self.regs().write_byte(addr, v);
        match addr {
            0xff20 => self.length.reload(self.regs.length(self.length.max_len())),
            0xff21 => {
                if !self.regs().dac_enabled(false) {
                    self.enabled = false
                }
            }
            0xff23 => {
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
impl LengthPass for Channel4Regs {}
