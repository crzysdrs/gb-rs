use super::{AudioChannel, ToneChannel};
use enum_primitive::FromPrimitive;
use mmu::MemRegister;
use peripherals::Addressable;

pub struct SoundChannel2 {
    channel: ToneChannel,
    nr21: u8,
    nr22: u8,
    nr23: u8,
    nr24: u8,
}

impl SoundChannel2 {
    pub fn new() -> SoundChannel2 {
        SoundChannel2 {
            channel: ToneChannel::new(false),
            nr21: 0,
            nr22: 0,
            nr23: 0,
            nr24: 0,
        }
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::NR21 => &mut self.nr21,
            MemRegister::NR22 => &mut self.nr22,
            MemRegister::NR23 => &mut self.nr23,
            MemRegister::NR24 => &mut self.nr24,
            _ => panic!("Invalid Sound Register in Channel 2"),
        }
    }
}

impl AudioChannel for SoundChannel2 {
    fn sample(&mut self, cycles: u64) -> i16 {
        self.channel.step(cycles) as i16
    }
}

impl Addressable for SoundChannel2 {
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        *self.lookup(addr) = v;
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::NR21 => {
                self.channel.duty.set_duty((v & 0b1100_0000) >> 6);
                self.channel.length.set_length(64 - (v & 0b0011_1111));
            }
            MemRegister::NR22 => {
                let period = v & 0b0000_0111;
                let start_vol = (v & 0b1111_0000) >> 4;
                let down = (v & 0b0000_1000) == 0;

                self.channel.vol.update(start_vol, period, down);
            }
            MemRegister::NR23 => {
                let mut freq = self.channel.get_freq();
                freq &= !0xff;
                freq |= v as u16;
                self.channel.set_freq(freq);
            }
            MemRegister::NR24 => {
                let mut freq = self.channel.get_freq();
                freq &= 0xff;
                freq |= (v as u16 & 0b111) << 8;
                self.channel.set_freq(freq);
                if (self.nr24 & (1 << 7)) != 0 {
                    self.channel.restart(self.nr24 & (1 << 6) != 0)
                }
                self.nr24 &= !(1 << 7);
            }
            _ => panic!("Invalid Register"),
        }
    }
}
