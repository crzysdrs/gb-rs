use super::{AudioChannel, Clocks};

use sound::channel::{
    ChannelRegs, Duty, DutyPass, Freq, HasRegs, Length, Length64Pass, LengthPass, Timer, Vol,
    VolumePass,
};

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
            regs: Channel2Regs(ChannelRegs::new()),
            vol: Vol::new(),
            timer: Timer::new(),
            length: Length::new(),
            duty: Duty::new(),
            enabled: false,
        }
    }
}

impl AudioChannel for Channel2 {
    fn reset(&mut self) {
        self.timer.reset();
        self.duty.reset();
        self.length.reset();
        self.vol.reset();
        self.enabled = true;
    }
    fn sample(&mut self, clocks: &Clocks) -> Option<i16> {
        if !self.enabled && !self.regs.trigger() {
            return None;
        } else if self.regs.trigger() {
            self.regs.clear_trigger();
            self.reset();
        }
        let ticks = self.timer.step(&mut self.regs, clocks);
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

struct Channel2Regs(ChannelRegs);

impl HasRegs for Channel2 {
    fn regs(&self) -> &ChannelRegs {
        self.regs.regs()
    }
    fn mut_regs(&mut self) -> &mut ChannelRegs {
        self.regs.mut_regs()
    }
}
impl HasRegs for Channel2Regs {
    fn regs(&self) -> &ChannelRegs {
        &self.0
    }
    fn mut_regs(&mut self) -> &mut ChannelRegs {
        &mut self.0
    }
}

impl Freq for Channel2Regs {}
impl DutyPass for Channel2Regs {}
impl Length64Pass for Channel2Regs {}
impl VolumePass for Channel2Regs {}
