use cpu;
use cpu::InterruptFlag;
use enum_primitive::FromPrimitive;
use mem::Mem;
use mmu::MemRegister;
use peripherals::{Addressable, Peripheral, PeripheralData};

mod channel;
mod channel1;
mod channel2;
mod channel3;
mod channel4;

use self::channel::ChannelRegs;
use self::channel1::Channel1;
use self::channel2::Channel2;
use self::channel3::Channel3;
use self::channel4::Channel4;

pub trait AudioChannel {
    fn reset(&mut self);
    fn disable(&mut self);
    fn regs(&mut self) -> &mut ChannelRegs;
    fn sample(&mut self, wave: &[u8], clocks: &Clocks) -> Option<i16>;
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        if let Some(reg) = MemRegister::from_u64(addr.into()) {
            match reg {
                MemRegister::NR10 | MemRegister::NR20 | MemRegister::NR30 | MemRegister::NR40 => {
                    &mut self.regs().nrx0
                }
                MemRegister::NR11 | MemRegister::NR21 | MemRegister::NR31 | MemRegister::NR41 => {
                    &mut self.regs().nrx1
                }
                MemRegister::NR12 | MemRegister::NR22 | MemRegister::NR32 | MemRegister::NR42 => {
                    &mut self.regs().nrx2
                }
                MemRegister::NR13 | MemRegister::NR23 | MemRegister::NR33 | MemRegister::NR43 => {
                    &mut self.regs().nrx3
                }
                MemRegister::NR14 | MemRegister::NR24 | MemRegister::NR34 | MemRegister::NR44 => {
                    &mut self.regs().nrx4
                }
                _ => unreachable!("Invalid register in AudioChannel"),
            }
        } else {
            unreachable!("Invalid addr in AudioChannel");
        }
    }
}

impl<T> Addressable for T
where
    T: AudioChannel,
{
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        *self.lookup(addr) = v;
    }
}

pub struct Clocks {
    length: u8,
    vol: u8,
    sweep: u8,
    cycles: u64,
}
struct FrameSequencer {
    time: u64,
    wait: WaitTimer<u64>,
}

impl FrameSequencer {
    fn new() -> FrameSequencer {
        FrameSequencer {
            time: 0,
            wait: WaitTimer::new(),
        }
    }
    fn step(&mut self, cycles: u64) -> Clocks {
        /* 512 hz clock */
        let mut new = Clocks {
            length: 0,
            vol: 0,
            sweep: 0,
            cycles: cycles,
        };
        if let Some(count) = self.wait.ready(cycles, (cpu::CYCLES_PER_S / 512).into()) {
            for _ in 0..count {
                self.time = (self.time + 1) % 8;
                if self.time % 2 == 0 {
                    new.length += 1;
                }
                if self.time == 7 {
                    new.vol += 1;
                }
                if self.time == 3 || self.time == 6 {
                    new.sweep += 1;
                }
            }
        }
        new
    }
}

struct WaitTimer<T> {
    acc: T,
}

impl<T> WaitTimer<T>
where
    T: num::Integer
        + std::ops::AddAssign
        + std::ops::SubAssign
        + std::convert::Into<u64>
        + std::marker::Copy,
{
    fn new() -> WaitTimer<T> {
        WaitTimer { acc: T::zero() }
    }
    fn ready(&mut self, new_cycles: T, required: T) -> Option<u64> {
        if required == T::zero() {
            return None;
        }
        self.acc += new_cycles;
        if self.acc >= required {
            let res = self.acc / required;
            self.acc -= res * required;
            Some(res.into())
        } else {
            None
        }
    }
    fn reset(&mut self) {
        self.acc = T::zero();
    }
}

pub struct Mixer {
    wait: WaitTimer<u64>,
    frame_seq: FrameSequencer,
    channel1: Channel1,
    channel2: Channel2,
    channel3: Channel3,
    channel4: Channel4,
    nr50: u8,
    nr51: u8,
    nr52: u8,
    wave: Mem,
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer {
            wait: WaitTimer::new(),
            frame_seq: FrameSequencer::new(),
            channel1: Channel1::new(),
            channel2: Channel2::new(),
            channel3: Channel3::new(),
            channel4: Channel4::new(),
            nr50: 0,
            nr51: 0,
            nr52: 0,
            wave: Mem::new(false, 0xff30, vec![0u8; 32]),
        }
    }
    fn lookup(&mut self, addr: u16) -> &mut Addressable {
        const CH1_START: u16 = MemRegister::NR10 as u16;
        const CH1_END: u16 = MemRegister::NR14 as u16;
        const CH2_START: u16 = MemRegister::NR21 as u16;
        const CH2_END: u16 = MemRegister::NR24 as u16;
        const CH3_START: u16 = MemRegister::NR30 as u16;
        const CH3_END: u16 = MemRegister::NR34 as u16;
        const CH4_START: u16 = MemRegister::NR41 as u16;
        const CH4_END: u16 = MemRegister::NR44 as u16;

        match addr {
            CH1_START...CH1_END => &mut self.channel1,
            CH2_START...CH2_END => &mut self.channel2,
            CH3_START...CH3_END => &mut self.channel3,
            CH4_START...CH4_END => &mut self.channel4,
            0xff30...0xff3f => &mut self.wave,
            _ => unreachable!("out of bounds mixer access {:x}", addr),
        }
    }
    fn lookup_internal(&mut self, addr: u16) -> Option<&mut u8> {
        if let Some(reg) = MemRegister::from_u64(addr.into()) {
            match reg {
                MemRegister::NR50 => Some(&mut self.nr50),
                MemRegister::NR51 => Some(&mut self.nr51),
                MemRegister::NR52 => Some(&mut self.nr52),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl Addressable for Mixer {
    fn read_byte(&mut self, addr: u16) -> u8 {
        if let Some(b) = self.lookup_internal(addr) {
            *b
        } else {
            self.lookup(addr).read_byte(addr)
        }
    }

    fn write_byte(&mut self, addr: u16, v: u8) {
        if let Some(b) = self.lookup_internal(addr) {
            *b = v;
        } else {
            self.lookup(addr).write_byte(addr, v);
        }
    }
}

impl Peripheral for Mixer {
    fn step(&mut self, real: &mut PeripheralData, time: u64) -> Option<InterruptFlag> {
        if let Some(ref mut audio) = real.audio_spec {
            let wait_time: u64 = (cpu::CYCLES_PER_S / audio.freq) as u64;
            if let Some(count) = self.wait.ready(time, wait_time) {
                for _ in 0..count {
                    let mut left: i16 = 0;
                    let mut right: i16 = 0;
                    let clocks = self.frame_seq.step(wait_time);
                    let channels: &mut [&mut AudioChannel] = &mut [
                        &mut self.channel1,
                        &mut self.channel2,
                        &mut self.channel3,
                        &mut self.channel4,
                    ];
                    //let chan = 3;
                    //self.nr51 = (1 << chan) | (1 << chan + 4);

                    for (i, channel) in channels.iter_mut().enumerate() {
                        if self.nr52 & (1 << 7) != 0 {
                            if let Some(val) = channel.sample(&self.wave, &clocks) {
                                if self.nr51 & (1 << i) != 0 {
                                    left = left.saturating_add(val);
                                }
                                if self.nr51 & (1 << (i + 4)) != 0 {
                                    right = right.saturating_add(val);
                                }
                            } else {
                                channel.disable();
                            }
                        } else {
                            channel.disable();
                        }
                    }
                    let left_vol = self.nr50 & 0b111;
                    let right_vol = (self.nr50 & 0b0111_0000) >> 4;

                    (audio.queue)(&[
                        audio.silence + left.saturating_mul((1 << 7) * left_vol as i16),
                        audio.silence + right.saturating_mul((1 << 7) * right_vol as i16),
                    ]);
                }
            }
        }

        None
    }
}
