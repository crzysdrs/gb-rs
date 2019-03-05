use super::{AudioChannel, Clocks};
use mmu::MemRegister;
use peripherals::Addressable;
use std::ops::{Deref, DerefMut};

use sound::channel::{
    AddressableChannel, ChannelRegs, Duty, DutyPass, Freq, HasRegs, Length, LengthPass, Sweep,
    SweepPass, Timer, Vol, VolumePass,
};

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
                &[0x80, 0x3f, 0x00, 0xff, 0xbf],
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
    fn reset(&mut self, clks: &Clocks, enable: bool, trigger: bool) {
        self.sweep.reset();
        self.timer.reset();
        self.duty.reset();
        self.length.update(clks, enable, trigger);
        self.vol.reset();
        self.enabled = true;
    }
    fn sample(&mut self, _wave: &[u8], cycles: u64, clocks: &Clocks) -> Option<i16> {
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
            Some(vol as i16)
        } else {
            Some(-(vol as i16))
        }
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl AddressableChannel for Channel1 {
    fn read_channel_byte(&mut self, _clks: &Clocks, addr: u16) -> u8 {
        self.regs().read_byte(addr)
    }
    fn write_channel_byte(&mut self, clks: &Clocks, addr: u16, v: u8) {
        self.regs().write_byte(addr, v);
        println!("Write to Channel1 {:x} {:x}", addr, v);
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
                    0xC0 => self.reset(clks, true, true),
                    0x80 => self.reset(clks, false, true),
                    0x40 => self.length.update(clks, true, false),
                    _ => self.length.update(clks, false, false),
                }
                if v & 0x80 != 0 {
                    self.enabled = self.regs.dac_enabled(false);
                }
            }
            _ => {}
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
impl LengthPass for Channel1Regs {}
impl SweepPass for Channel1Regs {}
