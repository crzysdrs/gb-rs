use super::mmu::MemRegister;
use cpu::InterruptFlag;
use enum_primitive::FromPrimitive;
use peripherals::Peripheral;

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
enum TimerFlags {
    ICS_4096hz = 0b00,
    ICS_262144hz = 0b01,
    ICS_65536hz = 0b10,
    ICS_16384hz = 0b11,
    START = 0b100,
}

#[allow(non_snake_case)]
pub struct Timer {
    TIMA: u8,
    TMA: u8,
    TAC: u8,
    DIV: u8,
    unused_cycles: u64,
    div_unused_cycles: u64,
}

impl Peripheral for Timer {
    fn read_byte(&mut self, addr: u16) -> u8 {
        *self.lookup(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        *self.lookup(addr) = v;
    }

    fn step(&mut self, time: u64) -> Option<InterruptFlag> {
        self.DIV = self.DIV.wrapping_add(Timer::compute_time(
            time,
            &mut self.div_unused_cycles,
            TimerFlags::ICS_65536hz,
        ) as u8);

        if self.TAC & (TimerFlags::START as u8) != 0 {
            let freq = self.freq();
            let add = Timer::compute_time(time, &mut self.unused_cycles, freq);
            let (new_tima, overflow) = self.TIMA.overflowing_add(add as u8);
            self.TIMA = new_tima;
            if overflow {
                self.TIMA = self.TMA;
                return Some(InterruptFlag::Timer);
            }
        }
        None
    }
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            TIMA: 0,
            TMA: 0,
            TAC: 0,
            DIV: 0,
            div_unused_cycles: 0,
            unused_cycles: 0,
        }
    }
    fn freq(&self) -> TimerFlags {
        match self.TAC & 0b11 {
            0b00 => TimerFlags::ICS_4096hz,
            0b01 => TimerFlags::ICS_262144hz,
            0b10 => TimerFlags::ICS_65536hz,
            0b11 => TimerFlags::ICS_16384hz,
            _ => panic!("Invalid Freq"),
        }
    }

    fn compute_time(time: u64, unused: &mut u64, freq: TimerFlags) -> u64 {
        *unused += time;
        let div = match freq {
            TimerFlags::ICS_4096hz => 256,
            TimerFlags::ICS_262144hz => 4,
            TimerFlags::ICS_65536hz => 16,
            TimerFlags::ICS_16384hz => 64,
            _ => panic!("Invalid Clock divider frequency"),
        };
        let add = *unused / div;
        *unused -= add * div;
        add
    }
    fn lookup(&mut self, addr: u16) -> &mut u8 {
        match MemRegister::from_u64(addr.into()).expect("Valid Register") {
            MemRegister::TIMA => &mut self.TIMA,
            MemRegister::TMA => &mut self.TMA,
            MemRegister::TAC => &mut self.TAC,
            MemRegister::DIV => &mut self.DIV,
            _ => panic!("invalid timer address"),
        }
    }
}
