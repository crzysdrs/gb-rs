use super::cpu::*;
use super::mmu::*;
#[cfg(test)]
use crate::cpu::Registers;
use crate::dma::DMA;
use crate::peripherals::{Peripheral, PeripheralData};
use std::io::Write;

#[cfg(feature = "vcd_dump")]
use VCDDump::VCD;

pub struct GB<'a> {
    cpu: CPU,
    mem: MMUInternal<'a>,
}

#[derive(Debug, PartialEq)]
pub enum GBReason {
    Timeout,
    VSync,
    Dead,
}

impl<'a> GB<'a> {
    pub fn new<'b>(
        cart: Box<Peripheral>,
        serial: Option<&'b mut Write>,
        trace: bool,
        fast_boot: bool,
    ) -> GB {
        let mut gb = GB {
            cpu: CPU::new(trace),
            mem: MMUInternal::new(cart, serial),
        };
        if fast_boot {
            let mut data = PeripheralData::empty();
            gb.cpu.initialize(&mut MMU::new(&mut gb.mem, &mut data));
        }
        gb
    }

    pub fn cpu_cycles(&self) -> u64 {
        self.mem.time()
    }
    pub fn toggle_trace(&mut self) {
        self.cpu.toggle_trace()
    }
    #[cfg(test)]
    pub fn get_reg(&self) -> Registers {
        self.cpu.get_reg()
    }
    #[cfg(test)]
    pub fn magic_breakpoint(&mut self) {
        self.cpu.magic_breakpoint();
    }
    #[cfg(test)]
    pub fn get_mem(&mut self) -> &mut MMUInternal<'a> {
        &mut self.mem
    }
    pub fn set_controls(&mut self, controls: u8) {
        self.mem.set_controls(controls);
    }

    pub fn step_timeout(&mut self, mut time: u64, real: &mut PeripheralData) -> GBReason {
        loop {
            let start_time = self.mem.time();
            match self.step(time, real) {
                r @ GBReason::Dead | r @ GBReason::Timeout => return r,
                _ => {}
            }
            time -= self.mem.time() - start_time;
        }
    }
    pub fn step(&mut self, time: u64, real: &mut PeripheralData) -> GBReason {
        //time in us
        let mut timeout_cycles = 0;
        real.reset_vblank();
        let mut mmu = MMU::new(&mut self.mem,real);
        while time == 0 || timeout_cycles < time {
            let start_time = mmu.bus.time();
            #[cfg(feature = "vcd_dump")]
            VCD.as_ref().map(|m| {
                m.lock().unwrap().as_mut().map(|v| {
                    let c = self.cpu_cycles;
                    v.now = c;
                })
            });
            self.cpu.execute(&mut mmu);
            if mmu.bus.dma_active() {
                let fake_dma = DMA::new();
                let mut real_dma = mmu.bus.swap_dma(fake_dma);
                real_dma.run(&mut mmu);
                mmu.bus.swap_dma(real_dma);
            }
            timeout_cycles += mmu.bus.time() - start_time;

            mmu.sync_peripherals();

            if self.cpu.is_dead(&mut mmu) {
                /* cpu permanently halted */
                return GBReason::Dead;
            } else if mmu.seen_vblank() {
                return GBReason::VSync;
            }
        }
        GBReason::Timeout
    }
    // #[cfg(test)]
    // pub fn run_instrs(&mut self, instrs: &[Instr]) {
    //     for i in instrs.iter_mut() {
    //         self.cpu.execute_instr(&mut self.mem, i);
    //     }
    // }
}
