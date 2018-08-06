use super::Clocks;
use super::WaitTimer;

pub struct ChannelRegs {
    pub nrx0: u8,
    pub nrx1: u8,
    pub nrx2: u8,
    pub nrx3: u8,
    pub nrx4: u8,
}

impl ChannelRegs {
    pub fn new() -> ChannelRegs {
        ChannelRegs {
            nrx0: 0,
            nrx1: 0,
            nrx2: 0,
            nrx3: 0,
            nrx4: 0,
        }
    }
}

pub trait HasRegs {
    fn regs(&self) -> &ChannelRegs;
    fn mut_regs(&mut self) -> &mut ChannelRegs;
}

pub trait SweepPass: HasRegs {
    fn period(&self) -> u8 {
        (self.regs().nrx0 & 0b0111_0000) >> 4
    }
    fn negated(&self) -> bool {
        (self.regs().nrx0 & 0b0000_1000) != 0
    }
    fn shift(&self) -> u8 {
        (self.regs().nrx0 & 0b0000_0111)
    }
}

pub trait LengthPass: HasRegs {
    fn length(&self) -> u8 {
        unreachable!();
    }
    fn enabled(&self) -> bool {
        self.regs().nrx4 & 0b0100_0000 != 0
    }
    fn trigger(&self) -> bool {
        self.regs().nrx4 & 0b1000_0000 != 0
    }
    fn clear_trigger(&mut self) {
        self.mut_regs().nrx4 &= !0b1000_0000;
    }
}

pub trait Length64Pass: HasRegs {}

impl<T> LengthPass for T
where
    T: Length64Pass,
{
    fn length(&self) -> u8 {
        64 - (self.regs().nrx1 & 0b0011_1111)
    }
}

pub trait VolumePass: HasRegs {
    fn vol_start(&self) -> u8 {
        (self.regs().nrx2 & 0b1111_0000) >> 4
    }
    fn vol_add(&self) -> bool {
        (self.regs().nrx2 & 0b0000_1000) != 0
    }
    fn vol_period(&self) -> u8 {
        (self.regs().nrx2 & 0b0000_0111)
    }
}
pub trait DutyPass: HasRegs {
    fn duty(&self) -> u8 {
        (self.regs().nrx1 & 0b1100_0000) >> 6
    }
}

pub trait Freq: HasRegs {
    fn freq(&self) -> u16 {
        let f = u16::from_bytes([self.regs().nrx3, self.regs().nrx4 & 0b111]);
        f
    }
    fn period(&self) -> u16 {
        (1 << 11) - self.freq()
    }
    fn set_freq(&mut self, new_freq: u16) {
        let [x0, x1] = new_freq.to_bytes();
        self.mut_regs().nrx4 &= !0b111;
        self.mut_regs().nrx4 |= x1 & 0b111;
        self.mut_regs().nrx3 = x0;
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
    pub fn step(&mut self, reg: &mut VolumePass, c: &Clocks) -> u8 {
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
    pub fn step(&mut self, regs: &mut Freq, clocks: &Clocks) -> u8 {
        let mut ticks = 0;
        if let Some(count) = self
            .period_wait
            .ready(clocks.cycles as u16, Freq::period(regs))
        {
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
    pub fn step(&mut self, regs: &mut DutyPass, ticks: u8) -> bool {
        self.offset += ticks;
        self.offset %= 8;
        DUTY_CYCLES[regs.duty() as usize][self.offset as usize]
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
        T: SweepPass + Freq,
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

pub struct Length {
    count: Option<u8>,
}

impl Length {
    pub fn new() -> Length {
        Length { count: None }
    }
    pub fn reset(&mut self) {
        self.count = None;
    }
    pub fn step(&mut self, reg: &mut LengthPass, c: &Clocks) -> Option<()> {
        let count = self.count.get_or_insert(reg.length());
        if !reg.enabled() {
            Some(())
        } else if *count > c.length {
            *count -= c.length;
            Some(())
        } else {
            *count = 0;
            None
        }
    }
}
