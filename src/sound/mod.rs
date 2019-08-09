use crate::cpu::InterruptFlag;
use crate::cycles;
use crate::emptymem::EmptyMem;
use crate::mem::Mem;
use crate::mmu::MemRegister;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use std::ops::Deref;

mod channel;
mod channel1;
mod channel2;
mod channel3;
mod channel4;

use self::channel::{AddressableChannel, ChannelRegs};
use self::channel1::Channel1;
use self::channel2::Channel2;
use self::channel3::Channel3;
use self::channel4::Channel4;
struct MaskReg {
    value: u8,
    mask: u8,
}

// impl MaskReg {
//     fn set(&mut self, byte: u8) {
//         self.value = byte;
//     }
//     fn unmasked(&self) -> u8 {
//         self.value | self.mask
//     }
// }

impl Deref for MaskReg {
    type Target = u8;

    fn deref(&self) -> &u8 {
        &self.value
    }
}

impl Addressable for MaskReg {
    fn write_byte(&mut self, addr: u16, val: u8) {
        self.value = val;
        self.wrote(addr, val);
    }
    fn read_byte(&mut self, _addr: u16) -> u8 {
        self.value | self.mask
    }
}

pub trait AudioChannel {
    fn reset(&mut self, clks: &Clocks, enable: bool, trigger: bool);
    fn disable(&mut self);
    fn power(&mut self, powered: bool);
    fn enabled(&self) -> bool;
    fn regs(&mut self) -> &mut ChannelRegs;
    fn sample(&mut self, wave: &[u8], cycles: cycles::CycleCount, clocks: &Clocks) -> Option<i16>;
    // fn lookup(&mut self, addr: u16) -> &mut u8 {
    //     if let Some(reg) = MemRegister::from_u64(addr.into()) {
    //         match reg {
    //             MemRegister::NR10 | MemRegister::NR20 | MemRegister::NR30 | MemRegister::NR40 => {
    //                 &mut self.regs().nrx0
    //             }
    //             MemRegister::NR11 | MemRegister::NR21 | MemRegister::NR31 | MemRegister::NR41 => {
    //                 &mut self.regs().nrx1
    //             }
    //             MemRegister::NR12 | MemRegister::NR22 | MemRegister::NR32 | MemRegister::NR42 => {
    //                 &mut self.regs().nrx2
    //             }
    //             MemRegister::NR13 | MemRegister::NR23 | MemRegister::NR33 | MemRegister::NR43 => {
    //                 &mut self.regs().nrx3
    //             }
    //             MemRegister::NR14 | MemRegister::NR24 | MemRegister::NR34 | MemRegister::NR44 => {
    //                 &mut self.regs().nrx4
    //             }
    //             _ => unreachable!("Invalid register in AudioChannel"),
    //         }
    //     } else {
    //         unreachable!("Invalid addr in AudioChannel");
    //     }
    // }
}

// impl<T> Addressable for T
// where
//     T: AudioChannel,
// {
//     fn read_byte(&mut self, addr: u16) -> u8 {
//         self.regs().read_byte(addr)
//     }
//     fn write_byte(&mut self, addr: u16, v: u8) {
//         self.regs().write_byte(addr, v);
//     }
// }

enum Clk {
    High,
    Low,
    Rising,
    Falling,
}

impl Clk {
    fn settle(&self) -> Clk {
        match *self {
            Clk::High => Clk::High,
            Clk::Low => Clk::Low,
            Clk::Rising => Clk::High,
            Clk::Falling => Clk::Low,
        }
    }
    fn changing(&self) -> bool {
        match *self {
            Clk::Rising | Clk::Falling => true,
            _ => false,
        }
    }
    fn tick(&self) -> u32 {
        if let Clk::Rising = *self {
            1
        } else {
            0
        }
    }
    fn fall(&self) -> Clk {
        match self {
            Clk::Rising | Clk::High => Clk::Falling,
            _ => Clk::Low,
        }
    }
}

pub struct Clocks {
    length: Clk,
    vol: Clk,
    sweep: Clk,
}

impl Clocks {
    #[allow(dead_code)]
    fn ticked(&self) -> bool {
        self.length.changing() || self.vol.changing() || self.sweep.changing()
    }
}

struct FrameSequencer {
    time: u64,
    wait: WaitTimer,
    clks: Clocks,
}

impl FrameSequencer {
    fn new() -> FrameSequencer {
        FrameSequencer {
            time: 0,
            wait: WaitTimer::new(),
            clks: Clocks {
                length: Clk::Low,
                vol: Clk::Low,
                sweep: Clk::Low,
            },
        }
    }
    fn clks(&self) -> &Clocks {
        &self.clks
    }
    fn settle(&mut self) {
        for c in [
            &mut self.clks.length,
            &mut self.clks.vol,
            &mut self.clks.sweep,
        ]
        .iter_mut()
        {
            std::mem::replace(*c, c.settle());
        }
    }
    fn step(&mut self, cycles: cycles::CycleCount) {
        /* 512 hz clock */
        self.settle();
        use dimensioned::si;
        if let Some(count) = self.wait.ready(cycles, cycles::Cycles::from(si::S / 512.0)) {
            for _ in 0..count {
                if self.time % 2 == 0 {
                    self.clks.length = Clk::Rising;
                } else {
                    self.clks.length = self.clks.length.fall();
                }
                if self.time == 7 {
                    self.clks.vol = Clk::Rising;
                } else {
                    self.clks.vol = self.clks.vol.fall();
                }
                if self.time == 3 || self.time == 6 {
                    self.clks.sweep = Clk::Rising;
                } else {
                    self.clks.sweep = self.clks.sweep.fall();
                }
                self.time = (self.time + 1) % 8;
                #[cfg(feature = "vcd_dump")]
                {
                    use crate::VCDDump::VCD;
                    VCD.as_ref().map(|m| {
                        m.lock().unwrap().as_mut().map(|v| {
                            for (name, val) in &[
                                ("vol", &mut self.clks.vol),
                                ("length", &mut self.clks.length),
                                ("sweep", &mut self.clks.sweep),
                            ] {
                                let (mut writer, mem) = v.writer();
                                let (wire, id) = mem.get(*name).unwrap();
                                wire.write(
                                    &mut writer,
                                    *id,
                                    match *val {
                                        Clk::Rising | Clk::High => 1,
                                        Clk::Falling | Clk::Low => 0,
                                    },
                                );
                            }
                        })
                    });
                }
            }
        }
    }
}

struct WaitTimer {
    acc: cycles::CycleCount,
}

impl WaitTimer {
    fn new() -> WaitTimer {
        WaitTimer {
            acc: cycles::Cycles::new(0),
        }
    }
    fn ready(
        &mut self,
        new_cycles: cycles::CycleCount,
        required: cycles::CycleCount,
    ) -> Option<u64> {
        if required == cycles::Cycles::new(0) {
            return None;
        }
        self.acc += new_cycles;
        if self.acc >= required {
            let res = self.acc / required;
            self.acc -= res * required;
            Some(res.value_unsafe)
        } else {
            None
        }
    }
    fn reset(&mut self) {
        self.acc = cycles::Cycles::new(0);
    }
}

pub struct Mixer {
    wait: WaitTimer,
    frame_seq: FrameSequencer,
    channel1: Channel1,
    channel2: Channel2,
    channel3: Channel3,
    channel4: Channel4,
    nr50: MaskReg,
    nr51: MaskReg,
    nr52: MaskReg,
    wave: Mem,
    unused: EmptyMem,
}

impl<T> AddressableChannel for T
where
    T: Addressable,
{
    fn read_channel_byte(&mut self, _clks: &Clocks, addr: u16) -> u8 {
        self.read_byte(addr)
    }
    fn write_channel_byte(&mut self, _clks: &Clocks, addr: u16, val: u8) {
        self.write_byte(addr, val)
    }
}
impl std::default::Default for Mixer {
    fn default() -> Self {
        Mixer {
            wait: WaitTimer::new(),
            frame_seq: FrameSequencer::new(),
            channel1: Channel1::new(),
            channel2: Channel2::new(),
            channel3: Channel3::new(),
            channel4: Channel4::new(),
            nr50: MaskReg {
                value: 0,
                mask: 0x00,
            },
            nr51: MaskReg {
                value: 0,
                mask: 0x00,
            },
            nr52: MaskReg {
                value: 0,
                mask: 0x70,
            },
            unused: EmptyMem::new(0xff, 0xff1f, 17),
            wave: Mem::new(false, 0xff30, vec![0u8; 32]),
        }
    }
}
impl Mixer {
    pub fn new() -> Mixer {
        Mixer::default()
    }
    fn lookup(&mut self, addr: u16) -> (&Clocks, Option<&mut AddressableChannel>) {
        const CH1_START: u16 = MemRegister::NR10 as u16;
        const CH1_END: u16 = MemRegister::NR14 as u16;
        const CH2_START: u16 = MemRegister::NR20 as u16;
        const CH2_END: u16 = MemRegister::NR24 as u16;
        const CH3_START: u16 = MemRegister::NR30 as u16;
        const CH3_END: u16 = MemRegister::NR34 as u16;
        const CH4_START: u16 = MemRegister::NR40 as u16;
        const CH4_END: u16 = MemRegister::NR44 as u16;

        const NR50: u16 = MemRegister::NR50 as u16;
        const NR51: u16 = MemRegister::NR51 as u16;
        const NR52: u16 = MemRegister::NR52 as u16;

        (
            &self.frame_seq.clks,
            match addr {
                CH1_START...CH1_END => Some(&mut self.channel1),
                CH2_START...CH2_END => Some(&mut self.channel2),
                CH3_START...CH3_END => Some(&mut self.channel3),
                CH4_START...CH4_END => Some(&mut self.channel4),
                0xff27...0xff2f => Some(&mut self.unused),
                0xff30...0xff3f => Some(&mut self.wave),
                NR50 => Some(&mut self.nr50),
                NR51 => Some(&mut self.nr51),
                NR52 => Some(&mut self.nr52),
                _ => None,
            },
        )
    }
}

impl Addressable for Mixer {
    fn read_byte(&mut self, addr: u16) -> u8 {
        if let (clks, Some(b)) = self.lookup(addr) {
            b.read_channel_byte(&clks, addr)
        } else {
            panic!("Unhandled Read in Mixer {:x}", addr);
        }
    }

    fn write_byte(&mut self, addr: u16, v: u8) {
        const NR52: u16 = MemRegister::NR52 as u16;
        let ignored = self.nr52.read_byte(0) & (1 << 7) == 0;
        if let (clks, Some(b)) = self.lookup(addr) {
            if !ignored || addr == NR52 {
                b.write_channel_byte(&clks, addr, v);
            }
            if addr == NR52 {
                if v & (1 << 7) == 0 {
                    self.nr51.write_byte(MemRegister::NR51 as u16, 0);
                    self.nr50.write_byte(MemRegister::NR50 as u16, 0);
                }
                let channels: &mut [&mut AudioChannel] = &mut [
                    &mut self.channel1,
                    &mut self.channel2,
                    &mut self.channel3,
                    &mut self.channel4,
                ];
                for channel in channels.iter_mut() {
                    if v & (1 << 7) == 0 {
                        //println!("Power Disable Channels");
                        channel.disable();
                        channel.power(false);
                    } else {
                        channel.power(true);
                    }
                }
            }
        } else {
            panic!("Unhandled Write In Mixer {:x}", addr);
        }
    }
}

impl Peripheral for Mixer {
    fn step(
        &mut self,
        real: &mut PeripheralData,
        cycles: cycles::CycleCount,
    ) -> Option<InterruptFlag> {
        let orig_status = *self.nr52;
        let mut status = *self.nr52;
        let channels: &mut [&mut AudioChannel] = &mut [
            &mut self.channel1,
            &mut self.channel2,
            &mut self.channel3,
            &mut self.channel4,
        ];
        if let Some(ref mut audio) = real.audio_spec {
            {
                self.frame_seq.step(cycles);
                //if self.frame_seq.clks().ticked() {
                for (_i, channel) in channels.iter_mut().enumerate() {
                    if *self.nr52 & (1 << 7) != 0
                        && channel.sample(&self.wave, cycles, &self.frame_seq.clks()).is_none()
                        && channel.enabled() {
                            //println!("Disable Channel {}", i);
                            channel.disable();
                    }
                }
                //}
            }
            self.frame_seq.settle();
            let wait_time = cycles::SECOND / u64::from(audio.freq);
            if let Some(count) = self.wait.ready(cycles, wait_time) {
                for _ in 0..count {
                    let mut left: i16 = 0;
                    let mut right: i16 = 0;
                    for (i, channel) in channels.iter_mut().enumerate() {
                        if *self.nr52 & (1 << 7) != 0 {
                            if let Some(val) =
                                channel.sample(&self.wave, cycles::Cycles::new(0), &self.frame_seq.clks())
                            {
                                if *self.nr51 & (1 << i) != 0 {
                                    left = left.saturating_add(val);
                                }
                                if *self.nr51 & (1 << (i + 4)) != 0 {
                                    right = right.saturating_add(val);
                                }
                            }
                        }
                    }

                    let left_vol = *self.nr50 & 0b111;
                    let right_vol = (*self.nr50 & 0b0111_0000) >> 4;

                    (audio.queue)(&[
                        audio.silence + left.saturating_mul((1 << 7) * i16::from(left_vol)),
                        audio.silence + right.saturating_mul((1 << 7) * i16::from(right_vol)),
                    ]);
                }
            }
        }
        status &= 0xf0;
        for (i, channel) in channels.iter_mut().enumerate() {
            status |= if channel.enabled() { 1 << i } else { 0 };
        }
        self.wave.set_readonly(status & (1 << 2) != 0);
        if status != orig_status {
            self.write_byte(MemRegister::NR52 as u16, status);
        }
        None
    }
}
