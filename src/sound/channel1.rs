use super::AudioChannel;
use super::ToneChannel;
use enum_primitive::FromPrimitive;
use mmu::MemRegister;
use peripherals::Addressable;

pub struct SoundChannel1 {
    channel: ToneChannel,
    nr10: u8,
    nr11: u8,
    nr12: u8,
    nr13: u8,
    nr14: u8,
}

impl AudioChannel for SoundChannel1 {
    fn sample(&mut self, cycles: u64) -> i16 {
        self.channel.step(cycles) as i16
    }
}

impl SoundChannel1 {
    pub fn new() -> SoundChannel1 {
        SoundChannel1 {
            channel: ToneChannel::new(true),
            nr10: 0,
            nr11: 0,
            nr12: 0,
            nr13: 0,
            nr14: 0,
        }
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::NR10 => &mut self.nr10,
            MemRegister::NR11 => &mut self.nr11,
            MemRegister::NR12 => &mut self.nr12,
            MemRegister::NR13 => &mut self.nr13,
            MemRegister::NR14 => &mut self.nr14,
            _ => panic!("Invalid Sound Register in Channel 1"),
        }
    }
}

impl Addressable for SoundChannel1 {
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        *self.lookup(addr) = v;
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::NR10 => {
                let sweep = (v & 0b0111_0000) >> 4;
                let n = v & 0b0000_0111;
                let down = (v & 0b0000_1000) != 0;
                self.channel
                    .sweep
                    .as_mut()
                    .map(|s| s.update(sweep, n, down));
            }
            MemRegister::NR11 => {
                self.channel.duty.set_duty((v & 0b1100_0000) >> 6);
                self.channel.length.set_length(64 - (v & 0b0011_1111));
            }
            MemRegister::NR12 => {
                let period = v & 0b0000_0111;
                let start_vol = (v & 0b1111_0000) >> 4;
                let down = (v & 0b0000_1000) == 0;

                self.channel.vol.update(start_vol, period, down);
            }
            MemRegister::NR13 => {
                let mut freq = self.channel.get_freq();
                freq &= !0xff;
                freq |= v as u16;
                self.channel.set_freq(freq);
            }
            MemRegister::NR14 => {
                let mut freq = self.channel.get_freq();
                freq &= 0xff;
                freq |= (v as u16 & 0b111) << 8;
                self.channel.set_freq(freq);
                if (self.nr14 & (1 << 7)) != 0 {
                    self.channel.restart(self.nr14 & (1 << 6) != 0)
                }
                self.nr14 &= !(1 << 7);
            }
            _ => panic!("Invalid Register"),
        }
    }
}
