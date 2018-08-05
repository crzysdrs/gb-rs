use cpu;
use cpu::InterruptFlag;
use enum_primitive::FromPrimitive;
use fakemem::FakeMem;
use mmu::MemRegister;
use peripherals::{Addressable, Peripheral, PeripheralData};

mod channel1;
mod channel2;

use self::channel1::SoundChannel1;
use self::channel2::SoundChannel2;

trait AudioChannel {
    fn sample(&mut self, cycles: u64) -> i16;
}

struct Clocks {
    length: u8,
    vol: u8,
    sweep: u8,
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

struct Sweep {
    period: u8,
    down: bool,
    shift: u8,
    shadow_period: u16,
    wait: WaitTimer<u8>,
    enable: bool,
}
impl Sweep {
    fn new() -> Sweep {
        Sweep {
            period: 0,
            shadow_period: 0,
            down: false,
            shift: 0,
            wait: WaitTimer::new(),
            enable: false,
        }
    }
    fn set_period(&mut self, period: u16) {
        self.shadow_period = period;
    }
    fn enabled(&self) -> bool {
        if self.period > 0 && self.shift > 0 {
            self.enable
        } else {
            true
        }
    }
    fn update(&mut self, period: u8, shift: u8, down: bool) {
        if period == 0 {
            self.period = 8;
        } else {
            self.period = period;
        }
        self.shift = shift;
        self.down = down;
    }

    fn reset(&mut self, period: u16) {
        self.shadow_period = period;
        self.enable = true
    }
    fn step(&mut self, clocks: &Clocks) {
        if self.period > 0 && self.shift > 0 && self.enable {
            if let Some(count) = self.wait.ready(clocks.sweep as u8, self.period) {
                for _ in 0..count {
                    let mut freq = (1 << 11) - self.shadow_period;
                    let shift_incr = freq >> self.shift;
                    if self.down {
                        freq = freq.saturating_sub(shift_incr);
                    } else {
                        freq = freq.saturating_add(shift_incr);
                    }
                    if freq > 2047 {
                        self.enable = false;
                        self.shadow_period = 0;
                    } else {
                        self.shadow_period = ((1 << 11) - freq) & 0x7ff;
                    }
                }
            }
        }
    }
    fn period(&self) -> u16 {
        self.shadow_period
    }
}
struct Length {
    orig_count: u8,
    count: u8,
    enable: bool,
}
impl Length {
    fn new() -> Length {
        Length {
            count: 0,
            orig_count: 0,
            enable: false,
        }
    }
    fn set_length(&mut self, count: u8) {
        self.orig_count = count;
        self.count = self.orig_count;
    }
    fn reset(&mut self, enable: bool) {
        self.count = self.orig_count;
        self.enable = enable;
    }
    fn step(&mut self, c: &Clocks) -> bool {
        if !self.enable {
            /* do nothing */
        } else if self.count >= c.length {
            self.count -= c.length;
        } else {
            self.count = 0;
        }
        self.count != 0
    }
}
//struct Wave {}
//struct LFSR {}

struct Vol {
    period: u8,
    orig_vol: u8,
    volume: u8,
    down: bool,
    wait: WaitTimer<u8>,
}

impl Vol {
    fn new() -> Vol {
        Vol {
            orig_vol: 0,
            period: 0,
            volume: 0,
            down: false,
            wait: WaitTimer::new(),
        }
    }
    fn reset(&mut self) {
        self.wait.reset();
        self.volume = self.orig_vol;
    }
    fn update(&mut self, volume: u8, period: u8, down: bool) {
        self.volume = volume & 0xf;
        self.orig_vol = self.volume;
        self.down = down;
        self.period = period;
    }
    fn step(&mut self, c: &Clocks) -> u8 {
        if self.period == 0 {
            /* do nothing */
        } else if let Some(count) = self.wait.ready(c.vol, self.period) {
            for _ in 0..count {
                self.volume = match (self.volume, self.down) {
                    (15, false) => 15,
                    (0, true) => 0,
                    (_, true) => self.volume - 1,
                    (_, false) => self.volume + 1,
                };
                self.volume &= 0xf;
            }
        }
        self.volume
    }
}
//struct Env {}

static DUTY_CYCLES: [[bool; 8]; 4] = [
    [false, false, false, false, false, false, false, true],
    [true, false, false, false, false, false, false, true],
    [true, false, false, false, false, true, true, true],
    [false, true, true, true, true, true, true, false],
];

struct Duty {
    offset: u8,
    duty: u8,
}

impl Duty {
    fn new() -> Duty {
        Duty { offset: 0, duty: 0 }
    }
    fn set_duty(&mut self, duty: u8) {
        self.duty = duty % 4;
    }
    fn reset(&mut self) {
        self.offset = 0;
    }
    fn step(&mut self, ticks: u8) -> bool {
        self.offset += ticks;
        self.offset %= 8;
        DUTY_CYCLES[self.duty as usize][self.offset as usize]
    }
}

struct ToneChannel {
    frame_seq: FrameSequencer,
    sweep: Option<Sweep>,
    timer: Timer,
    duty: Duty,
    period: u16,
    length: Length,
    vol: Vol,
    enabled: bool,
    // wave: Option<Wave>,
    // lfsr : Option<LFSR>,
    // env : Option<Env>
}

impl ToneChannel {
    fn new(with_sweep: bool) -> ToneChannel {
        let sweep = if with_sweep { Some(Sweep::new()) } else { None };
        ToneChannel {
            frame_seq: FrameSequencer::new(),
            sweep,
            period: 0,
            timer: Timer::new(),
            duty: Duty::new(),
            length: Length::new(),
            vol: Vol::new(),
            enabled: false,
        }
    }

    fn get_freq(&self) -> u16 {
        (1 << 11) - self.period
    }
    fn set_freq(&mut self, freq: u16) {
        self.period = (1 << 11) - (freq & 0x7ff);
        let p = self.period;
        self.sweep.as_mut().map(|s| s.set_period(p));
        self.timer.set_period(self.period);
    }
    fn restart(&mut self, length_enable: bool) {
        let p = self.period;
        self.sweep.as_mut().map(|s| s.reset(p));
        self.timer.reset(self.period);
        self.duty.reset();
        self.vol.reset();
        self.length.reset(length_enable);
        self.enabled = true;
    }
    fn step(&mut self, cycles: u64) -> i8 {
        if self.enabled {
            let clocks = self.frame_seq.step(cycles);
            let (timer_period, sweep_enabled) =
                self.sweep.as_mut().map_or((self.period, true), |s| {
                    s.step(&clocks);
                    (s.period(), s.enabled())
                });
            self.timer.set_period(timer_period);
            let ticks = self.timer.step(cycles);
            let high = self.duty.step(ticks as u8);
            let length_enabled = self.length.step(&clocks);
            let volume = self.vol.step(&clocks);
            self.enabled = length_enabled && sweep_enabled;
            if high {
                volume as i8
            } else {
                -(volume as i8)
            }
        } else {
            0
        }
    }
}

struct Timer {
    period_wait: WaitTimer<u16>,
    period: u16,
}

impl Timer {
    fn new() -> Timer {
        Timer {
            period_wait: WaitTimer::new(),
            period: 0,
        }
    }
    fn reset(&mut self, period: u16) {
        self.set_period(period);
        self.period_wait.reset();
    }
    fn step(&mut self, cycles: u64) -> u16 {
        let mut clocks = 0;
        if let Some(count) = self.period_wait.ready(cycles as u16, self.period) {
            clocks += count as u16;
        }
        clocks
    }
    fn set_period(&mut self, period: u16) {
        self.period = period;
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
    channel1: SoundChannel1,
    channel2: SoundChannel2,
    mem: FakeMem,
    nr50: u8,
    nr51: u8,
    nr52: u8,
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer {
            mem: FakeMem::new(),
            wait: WaitTimer::new(),
            channel1: SoundChannel1::new(),
            channel2: SoundChannel2::new(),
            nr50: 0,
            nr51: 0,
            nr52: 0,
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
            CH3_START...CH3_END => &mut self.mem,
            CH4_START...CH4_END => &mut self.mem,
            0xff30...0xff3f => &mut self.mem, /* TODO: wave pattern RAM */
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
                    let channels: &mut [&mut AudioChannel] =
                        &mut [&mut self.channel1, &mut self.channel2];
                    for (i, channel) in channels.iter_mut().enumerate() {
                        if self.nr52 & (1 << 7) != 0 {
                            let val = channel.sample(wait_time) as i16;
                            if self.nr51 & (1 << i) != 0 {
                                left = left.saturating_add(val);
                            }
                            if self.nr51 & (1 << (i + 4)) != 0 {
                                right = right.saturating_add(val);
                            }
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
