use super::Clocks;
use super::WaitTimer;
use peripherals::Addressable;
use std::ops::{Deref, DerefMut};

use super::MaskReg;

pub struct ChannelRegs {
    base: u16,
    nrx0: MaskReg,
    nrx1: MaskReg,
    nrx2: MaskReg,
    nrx3: MaskReg,
    nrx4: MaskReg,
}

impl ChannelRegs {
    pub fn new(addr: u16, mask: &[u8; 5]) -> ChannelRegs {
        ChannelRegs {
            base: addr,
            nrx0: MaskReg {
                value: 0,
                mask: mask[0],
            },
            nrx1: MaskReg {
                value: 0,
                mask: mask[1],
            },
            nrx2: MaskReg {
                value: 0,
                mask: mask[2],
            },
            nrx3: MaskReg {
                value: 0,
                mask: mask[3],
            },
            nrx4: MaskReg {
                value: 0,
                mask: mask[4],
            },
        }
    }
    pub fn reset(&mut self) {
        self.nrx0.set(0);
        self.nrx1.set(0);
        self.nrx2.set(0);
        self.nrx3.set(0);
        self.nrx4.set(0);
    }
}

impl Addressable for ChannelRegs {
    fn read_byte(&mut self, addr: u16) -> u8 {
        let r = match addr - self.base {
            0 => &self.nrx0,
            1 => &self.nrx1,
            2 => &self.nrx2,
            3 => &self.nrx3,
            4 => &self.nrx4,
            _ => unreachable!("Bad Channel Read"),
        };
        r.read()
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        let r = match addr - self.base {
            0 => &mut self.nrx0,
            1 => &mut self.nrx1,
            2 => &mut self.nrx2,
            3 => &mut self.nrx3,
            4 => &mut self.nrx4,
            _ => unreachable!("Bad Channel Write"),
        };
        r.set(v);
    }
}

pub trait HasRegs: DerefMut + Deref<Target = ChannelRegs> {}

pub trait SweepPass: HasRegs {
    fn period(&self) -> u8 {
        (*self.nrx0 & 0b0111_0000) >> 4
    }
    fn negated(&self) -> bool {
        (*self.nrx0 & 0b0000_1000) != 0
    }
    fn shift(&self) -> u8 {
        (*self.nrx0 & 0b0000_0111)
    }
}

impl ChannelRegs {
    pub fn enabled(&self) -> bool {
        *self.nrx4 & 0b0100_0000 != 0
    }
    pub fn trigger(&self) -> bool {
        *self.nrx4 & 0b1000_0000 != 0
    }
    pub fn clear_trigger(&mut self) {
        self.nrx4.set(*self.nrx4 & !0b1000_0000);
    }
    pub fn wave_enabled(&self) -> bool {
        (*self.nrx0 & 0b1000_0000) != 0
    }
}
pub trait LengthPass<T> {
    fn length(&self) -> T;
}

impl LengthPass<u8> for ChannelRegs {
    fn length(&self) -> u8 {
        64 - (*self.nrx1 & 0b0011_1111)
    }
}

impl LengthPass<u16> for ChannelRegs {
    fn length(&self) -> u16 {
        256 - *self.nrx1 as u16
    }
}

pub trait LFSRPass: HasRegs {
    fn shift(&self) -> u16 {
        let mut shift = (((*self.nrx3 & 0b1111_0000) >> 4) + 1) as u16;
        if shift > 0xf {
            shift = 0xf;
        }
        1 << shift
    }
    fn width_mode(&self) -> bool {
        (*self.nrx3 & 0b0000_1000) != 0
    }
    fn period(&self) -> u8 {
        let b = *self.nrx3 & 0b0000_0111;
        match b {
            0 => 8,
            _ => b << 4,
        }
    }
}

pub trait VolumePass: HasRegs {
    fn vol_start(&self) -> u8 {
        (*self.nrx2 & 0b1111_0000) >> 4
    }
    fn vol_add(&self) -> bool {
        (*self.nrx2 & 0b0000_1000) != 0
    }
    fn vol_period(&self) -> u8 {
        (*self.nrx2 & 0b0000_0111)
    }
}
pub trait DutyPass: HasRegs {
    fn duty(&self) -> u8 {
        (*self.nrx1 & 0b1100_0000) >> 6
    }
}

pub trait VolumeCode: HasRegs {
    fn vol_code(&self) -> u8 {
        (*self.nrx2 & 0b0110_0000) >> 5
    }
}

pub trait Freq: HasRegs {
    fn freq(&self) -> u16 {
        let f = u16::from_bytes([*self.nrx3, *self.nrx4 & 0b111]);
        f
    }
    fn period(&self) -> u16 {
        (1 << 11) - self.freq()
    }
    fn set_freq(&mut self, new_freq: u16) {
        let [x0, x1] = new_freq.to_bytes();
        let tmp = *self.nrx4;
        self.nrx4.set((tmp & !0b111) | (x1 & 0b111));
        self.nrx3.set(x0);
    }
}

pub struct Vol {
    volume: Option<u8>,
    wait: WaitTimer<u8>,
}

impl Vol {
    pub fn new() -> Vol {
        Vol {
            volume: None,
            wait: WaitTimer::new(),
        }
    }
    pub fn reset(&mut self) {
        self.wait.reset();
        self.volume = None;
    }
    pub fn step(&mut self, reg: &mut VolumePass<Target = ChannelRegs>, c: &Clocks) -> u8 {
        let vol = self.volume.get_or_insert(reg.vol_start());
        if reg.vol_period() == 0 {
            /* do nothing */
        } else if let Some(count) = self.wait.ready(c.vol, reg.vol_period()) {
            for _ in 0..count {
                *vol = match (*vol, reg.vol_add()) {
                    (15, true) => 15,
                    (0, false) => 0,
                    (_, false) => *vol - 1,
                    (_, true) => *vol + 1,
                };
                *vol &= 0xf;
            }
        }
        *vol
    }
}

pub struct Timer {
    period_wait: WaitTimer<u16>,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            period_wait: WaitTimer::new(),
        }
    }
    pub fn reset(&mut self) {
        self.period_wait.reset();
    }
    pub fn step(
        &mut self,
        period: u16,
        _regs: &mut Freq<Target = ChannelRegs>,
        clocks: &Clocks,
    ) -> u8 {
        let mut ticks = 0;
        if let Some(count) = self.period_wait.ready(clocks.cycles as u16, period) {
            ticks += count as u8;
        }
        ticks
    }
}

static DUTY_CYCLES: [[bool; 8]; 4] = [
    [false, false, false, false, false, false, false, true],
    [true, false, false, false, false, false, false, true],
    [true, false, false, false, false, true, true, true],
    [false, true, true, true, true, true, true, false],
];

pub struct Duty {
    offset: u8,
}

impl Duty {
    pub fn new() -> Duty {
        Duty { offset: 0 }
    }
    pub fn reset(&mut self) {
        self.offset = 0;
    }
    pub fn step(&mut self, regs: &mut DutyPass<Target = ChannelRegs>, ticks: u8) -> bool {
        self.offset += ticks;
        self.offset %= 8;
        DUTY_CYCLES[regs.duty() as usize][self.offset as usize]
    }
}

pub struct LFSR {
    shift_reg: u16,
    wait: WaitTimer<u16>,
}

impl LFSR {
    pub fn new() -> LFSR {
        LFSR {
            shift_reg: 0xaa,
            wait: WaitTimer::new(),
        }
    }
    pub fn reset(&mut self) {
        self.wait.reset();
    }
    pub fn step(
        &mut self,
        ticks: u8,
        regs: &mut LFSRPass<Target = ChannelRegs>,
        _clocks: &Clocks,
    ) -> bool {
        if let Some(count) = self.wait.ready(ticks as u16, regs.shift()) {
            for _ in 0..count {
                let b0 = self.shift_reg & 0b1;
                let b1 = (self.shift_reg & 0b10) >> 1;
                let high_bit = (b0 ^ b1) & 0b1;
                self.shift_reg = (self.shift_reg >> 1) | (high_bit << 14);
                if regs.width_mode() {
                    self.shift_reg |= high_bit << 6;
                }
                self.shift_reg &= 0x7fff;
            }
        }
        !self.shift_reg & 0b1 == 1
    }
}

pub struct Sweep {
    shadow_freq: Option<u16>,
    wait: WaitTimer<u8>,
}

impl Sweep {
    pub fn new() -> Sweep {
        Sweep {
            shadow_freq: None,
            wait: WaitTimer::new(),
        }
    }
    pub fn reset(&mut self) {
        self.shadow_freq = None;
    }
    pub fn step<T>(&mut self, regs: &mut T, clocks: &Clocks) -> Option<()>
    where
        T: SweepPass<Target = ChannelRegs> + Freq,
    {
        if self.shadow_freq.is_none() {
            self.shadow_freq = Some(regs.freq());
        }
        if SweepPass::period(regs) > 0 && regs.shift() > 0 {
            if let Some(count) = self.wait.ready(clocks.sweep as u8, SweepPass::period(regs)) {
                for _ in 0..count {
                    if let Some(ref mut freq) = self.shadow_freq {
                        let shift_incr = *freq >> regs.shift();
                        if SweepPass::negated(regs) {
                            *freq = freq.saturating_sub(shift_incr);
                        } else {
                            *freq = freq.saturating_add(shift_incr);
                        }
                        if *freq > 2047 {
                            return None;
                        } else {
                            regs.set_freq(*freq);
                        }
                    }
                }
            }
        }
        Some(())
    }
}

pub struct Length<T> {
    count: Option<T>,
}

impl<T> Length<T>
where
    T: std::ops::SubAssign + std::cmp::PartialOrd + std::convert::From<u8>,
{
    pub fn new() -> Length<T> {
        Length { count: None }
    }
    pub fn reset(&mut self) {
        self.count = None;
    }
    pub fn step<X>(&mut self, reg: &mut X, c: &Clocks) -> Option<()>
    where
        X: LengthPass<T> + HasRegs,
    {
        let count = self.count.get_or_insert(reg.length());
        if !reg.enabled() {
            Some(())
        } else if *count > c.length.into() {
            *count -= c.length.into();
            Some(())
        } else {
            *count = 0.into();
            None
        }
    }
}
