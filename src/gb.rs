use super::cpu::*;
use super::mmu::*;
#[cfg(test)]
use crate::cpu::Registers;
use crate::dma::DMA;
use crate::peripherals::{Peripheral, PeripheralData};
use std::io::Write;

use crate::cycles;

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

    pub fn cpu_cycles(&self) -> cycles::CycleCount {
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

    pub fn step_timeout(
        &mut self,
        time: Option<cycles::CycleCount>,
        real: &mut PeripheralData,
    ) -> GBReason {
        let finish_time = time.map(|x| x + self.mem.time());
        loop {
            let time = finish_time.map(|x| x - self.mem.time());
            match self.step(time, real) {
                r @ GBReason::Dead | r @ GBReason::Timeout => return r,
                _ => {}
            }
        }
    }
    pub fn step(
        &mut self,
        time: Option<cycles::CycleCount>,
        real: &mut PeripheralData,
    ) -> GBReason {
        let finish_time = time.map(|x| x + self.mem.time());
        real.reset_vblank();
        let mut mmu = MMU::new(&mut self.mem, real);
        while time.is_none()
            || finish_time
                .map(|x| mmu.bus.time() < x)
                .unwrap_or_else(|| false)
        {
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
