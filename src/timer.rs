use super::mmu::{MemRegister};
use peripherals::{Peripheral};
use enum_primitive::FromPrimitive;

enum TimerFlags {
    ICS_4096khz = 0b00,
    ICS_262144khz = 0b01,
    ICS_65536khz = 0b10,
    ICS_16384khz = 0b11,
    START = 0b100,
}


pub struct Timer {
    TIMA : u8,
    TMA : u8,
    TAC : u8,
}

impl Peripheral for Timer {
    fn lookup(&mut self, addr : u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::TIMA => &mut self.TIMA,
            MemRegister::TMA => &mut self.TMA,
            MemRegister::TAC => &mut self.TAC,
            _ => panic!("invalid timer address")
        }
    }
    fn step(&mut self, time : u64) {
        if self.TMA & (TimerFlags::START as u8) != 0 {
            let n = 1;
            let (res, overflow) = match self.freq() {
                TimerFlags::ICS_4096khz => self.TIMA.overflowing_add(n),
                TimerFlags::ICS_262144khz => self.TIMA.overflowing_add(n),
                TimerFlags::ICS_65536khz => self.TIMA.overflowing_add(n),
                TimerFlags::ICS_16384khz => self.TIMA.overflowing_add(n),
                _ => panic!("Invalid Clock divider frequency")
            };
            self.TIMA = res;
            if overflow {
                let (new_tma, overflow) = self.TMA.overflowing_add(1);
                self.TMA = new_tma;
                //TODO: generate interrupt
            }
        }
    }
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            TIMA :0,
            TMA : 0,
            TAC : 0,
        }
    }
    fn next_interrupt(&self) -> Option<u64> {
        /* inform when the next interrupt should happen, so main process can sleep */
        Some(1)
    }
    fn freq(&self) -> TimerFlags {
        match self.TIMA & 0b11 {
            0b00 => TimerFlags::ICS_4096khz,
            0b01 => TimerFlags::ICS_262144khz,
            0b10 => TimerFlags::ICS_65536khz,
            0b11 => TimerFlags::ICS_16384khz,
            _ => panic!("Invalid Freq")
        }
    }
}
