use super::mmu::MemRegister;
use enum_primitive::FromPrimitive;
use peripherals::Peripheral;

#[allow(non_camel_case_types)]
enum TimerFlags {
    ICS_4096khz = 0b00,
    ICS_262144khz = 0b01,
    ICS_65536khz = 0b10,
    ICS_16384khz = 0b11,
    START = 0b100,
}

#[allow(non_snake_case)]
pub struct Timer {
    TIMA: u8,
    TMA: u8,
    TAC: u8,
    unused_cycles: u64,
}

impl Peripheral for Timer {
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::TIMA => &mut self.TIMA,
            MemRegister::TMA => &mut self.TMA,
            MemRegister::TAC => &mut self.TAC,
            _ => panic!("invalid timer address"),
        }
    }
    fn step(&mut self, time: u64) {
        if self.TMA & (TimerFlags::START as u8) != 0 {
            self.unused_cycles += time;
            //TODO: Put correct timer counts
            let div = match self.freq() {
                TimerFlags::ICS_4096khz => 100,
                TimerFlags::ICS_262144khz => 200,
                TimerFlags::ICS_65536khz => 300,
                TimerFlags::ICS_16384khz => 400,
                _ => panic!("Invalid Clock divider frequency"),
            };
            let add = self.unused_cycles / div;
            self.unused_cycles -= add;
            let (new_tima, overflow) = self.TIMA.overflowing_add(add as u8);
            self.TIMA = new_tima;
            if overflow {
                self.TMA = self.TMA.wrapping_add(1);
                //TODO: generate interrupt
            }
        }
    }
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            TIMA: 0,
            TMA: 0,
            TAC: 0,
            unused_cycles: 0,
        }
    }
    fn freq(&self) -> TimerFlags {
        match self.TIMA & 0b11 {
            0b00 => TimerFlags::ICS_4096khz,
            0b01 => TimerFlags::ICS_262144khz,
            0b10 => TimerFlags::ICS_65536khz,
            0b11 => TimerFlags::ICS_16384khz,
            _ => panic!("Invalid Freq"),
        }
    }
}
