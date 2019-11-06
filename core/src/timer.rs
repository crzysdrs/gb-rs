use super::mmu::MemRegister;
use crate::cpu::Interrupt;
use crate::cycles;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use crate::sound::WaitTimer;
use enum_primitive::FromPrimitive;
use modular_bitfield::prelude::*;
use std::convert::TryFrom;

#[derive(BitfieldSpecifier)]
#[allow(non_camel_case_types)]
pub enum TimerSpeed {
    ICS_4096hz = 0b00,
    ICS_262144hz = 0b01,
    ICS_65536hz = 0b10,
    ICS_16384hz = 0b11,
}

#[bitfield]
#[derive(Copy, Clone)]
pub struct TimerControl {
    #[bits = 2]
    speed: TimerSpeed,
    start: bool,
    unused: B5,
}

#[allow(non_snake_case)]
pub struct Timer {
    TIMA: u8,
    TMA: u8,
    TAC: TimerControl,
    DIV: u8,
    timer: WaitTimer,
    div_timer: WaitTimer,
    double: bool,
}

impl Addressable for Timer {
    fn read_byte(&mut self, addr: u16) -> u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::TAC => self.TAC.to_bytes()[0],
            _ => *self.lookup(addr),
        }
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::DIV => {
                self.DIV = 0;
            }
            MemRegister::TAC => {
                self.TAC = TimerControl::try_from(&[v][..]).unwrap();
            }
            _ => {
                *self.lookup(addr) = v;
            }
        }
    }
}
impl Peripheral for Timer {
    fn next_step(&self) -> Option<cycles::CycleCount> {
        if self.TAC.get_start() {
            let count = Timer::divider(self.freq(), self.double);
            let next_interrupt = u64::from(std::u8::MAX - self.TIMA);
            let required = self.timer.next_ready(count) + count * next_interrupt;
            Some(required)
        } else {
            Some(cycles::CycleCount::new(std::u64::MAX))
        }
    }
    fn step(&mut self, _real: &mut PeripheralData, time: cycles::CycleCount) -> Option<Interrupt> {
        use std::convert::TryInto;
        if let Some(c) = self
            .div_timer
            .ready(time, Timer::divider(TimerSpeed::ICS_16384hz, self.double))
        {
            self.DIV = self.DIV.wrapping_add((c & 0xff).try_into().unwrap());
        }
        if self.TAC.get_start() {
            if let Some(mut add) = self
                .timer
                .ready(time, Timer::divider(self.freq(), self.double))
            {
                while add > 0 {
                    let next = if add > u64::from(std::u8::MAX) {
                        add -= u64::from(std::u8::MAX);
                        std::u8::MAX
                    } else {
                        let v = add;
                        add = 0;
                        v as u8
                    };

                    let (new_tima, overflow) = self.TIMA.overflowing_add(next);
                    self.TIMA = new_tima;
                    if overflow {
                        assert_eq!(add, 0);
                        self.TIMA = self.TMA;
                        let mut interrupt = Interrupt::new();
                        interrupt.set_timer(true);
                        return Some(interrupt);
                    }
                }
            }
        } else {
            self.timer.reset();
        }
        None
    }
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            TIMA: 0,
            TMA: 0,
            TAC: TimerControl::new(),
            DIV: 0,
            div_timer: WaitTimer::new(),
            timer: WaitTimer::new(),
            double: false,
        }
    }
    pub fn toggle_double(&mut self) {
        self.double = !self.double;
    }
    fn freq(&self) -> TimerSpeed {
        self.TAC.get_speed()
    }

    fn divider(freq: TimerSpeed, double: bool) -> cycles::CycleCount {
        use dimensioned::si;
        cycles::CycleCount::from(
            si::S
                / f64::from(
                    match freq {
                        TimerSpeed::ICS_4096hz => 4_096,
                        TimerSpeed::ICS_262144hz => 262_144,
                        TimerSpeed::ICS_65536hz => 65_536,
                        TimerSpeed::ICS_16384hz => 16_384,
                    } * if double { 2 } else { 1 },
                ),
        )
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::TIMA => &mut self.TIMA,
            MemRegister::TMA => &mut self.TMA,
            MemRegister::TAC => unimplemented!("Modify TAC elsewhere"),
            MemRegister::DIV => &mut self.DIV,
            _ => panic!("invalid timer address"),
        }
    }
}
