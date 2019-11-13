use super::{AudioChannel, Clocks};
use crate::cycles;
use crate::mmu::MemRegister;
use crate::peripherals::Addressable;
use std::ops::{Deref, DerefMut};

use crate::sound::channel::{
    AddressableChannel, ChannelRegs, Duty, DutyPass, Freq, HasRegs, Length, LengthPass, Timer, Vol,
    VolumePass,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Channel2 {
    enabled: bool,
    regs: Channel2Regs,
    vol: Vol,
    timer: Timer,
    length: Length,
    duty: Duty,
}

impl Channel2 {
    pub fn new() -> Channel2 {
        Channel2 {
            regs: Channel2Regs(ChannelRegs::new(
                MemRegister::NR20 as u16,
                [0xff, 0x3f, 0x00, 0xff, 0xbf],
            )),
            vol: Vol::new(),
            timer: Timer::new(),
            length: Length::new(64),
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
    fn power(&mut self, power: bool) {
        self.regs.power(power);
    }
    fn reset(&mut self, enable: bool, trigger: bool) {
        self.timer.reset();
        self.duty.reset();
        self.length.update(enable, trigger);
        self.vol.reset();
        self.enabled = true;
    }
    fn sample(&mut self, _wave: &[u8], cycles: cycles::CycleCount, clocks: &Clocks) -> Option<i16> {
        if !self.enabled {
            self.length.step(clocks)?;
            self.enabled = false;
            return None;
        }
        let ticks = self.timer.step(Freq::period(&self.regs), cycles, clocks);
        let high = self.duty.step(&mut self.regs, ticks);
        self.length.step(clocks)?;
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

impl AddressableChannel for Channel2 {
    fn read_channel_byte(&mut self, addr: u16) -> u8 {
        self.regs().read_byte(addr)
    }
    fn write_channel_byte(&mut self, addr: u16, v: u8) {
        self.regs().write_byte(addr, v);
        //println!("Write to Channel2 {:x} {:x}", addr, v);
        match addr {
            0xff16 => self.length.reload(self.regs.length(self.length.max_len())),
            0xff17 => {
                if !self.regs().dac_enabled(false) {
                    self.enabled = false
                }
            }
            0xff19 => {
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
impl LengthPass for Channel2Regs {}
