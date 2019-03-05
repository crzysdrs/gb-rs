use super::cpu::*;
use super::mmu::*;
use crate::cart::Cart;
#[cfg(test)]
use crate::cpu::Registers;
use crate::dma::DMA;
use crate::peripherals::{Addressable, PeripheralData};
use std::io::Write;

#[cfg(feature = "vcd_dump")]
use VCDDump::VCD;

pub struct GB<'a> {
    cpu: CPU,
    mem: MMU<'a>,
    cpu_cycles: u64,
}

#[derive(Debug, PartialEq)]
pub enum GBReason {
    Timeout,
    VSync,
    Dead,
}

impl<'a> GB<'a> {
    pub fn new<'b>(
        cart: Cart,
        serial: Option<&'b mut Write>,
        trace: bool,
        fast_boot: bool,
    ) -> GB<'b> {
        let mut gb = GB {
            cpu: CPU::new(trace),
            mem: MMU::new(cart, serial),
            cpu_cycles: 0,
        };
        if fast_boot {
            gb.cpu.initialize(&mut gb.mem);
        }
        gb
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
    pub fn get_mem(&mut self) -> &mut MMU<'a> {
        &mut self.mem
    }
    fn update_interrupts(&mut self, real: &mut PeripheralData, cycles: u64) -> u8 {
        let mut interrupt_flag = 0;
        self.mem
            .walk_peripherals(|p| match p.step(real, cycles as u64) {
                Some(i) => {
                    interrupt_flag |= mask_u8!(i);
                }
                None => {}
            });
        let flags = self.mem.read_byte_silent(0xff0f);
        let rhs = flags | interrupt_flag;
        // if !self.mem.get_display().display_enabled() {
        //     /* remove vblank from IF when display disabled.
        //     We still want it to synchronize speed with display */
        //     rhs &= !mask_u8!(InterruptFlag::VBlank);
        // }
        if rhs != flags {
            self.mem.write_byte(0xff0f, rhs);
        }
        interrupt_flag
    }

    fn run_dma(&mut self) {
        let fake_dma = DMA::new();
        let mut real_dma = self.mem.swap_dma(fake_dma);
        real_dma.run(&mut self.mem);
        self.mem.swap_dma(real_dma);
    }
    pub fn set_controls(&mut self, controls: u8) {
        self.mem.set_controls(controls);
    }

    pub fn cpu_cycles(&self) -> u64 {
        self.cpu_cycles
    }

    pub fn step_timeout(&mut self, mut time: u64, real: &mut PeripheralData) -> GBReason {
        loop {
            let cycles = self.cpu_cycles;
            match self.step(time, real) {
                r @ GBReason::Dead | r @ GBReason::Timeout => return r,
                _ => {}
            }
            time -= self.cpu_cycles - cycles;
        }
    }
    pub fn step(&mut self, time: u64, real: &mut PeripheralData) -> GBReason {
        //time in us
        let mut timeout_cycles = 0;
        while time == 0 || timeout_cycles < time {
            #[cfg(feature = "vcd_dump")]
            VCD.as_ref().map(|m| {
                m.lock().unwrap().as_mut().map(|v| {
                    let c = self.cpu_cycles;
                    v.now = c;
                })
            });
            let cycles: u64 = self.cpu.execute(&mut self.mem, self.cpu_cycles) as u64;
            let new_interrupt = self.update_interrupts(real, cycles);
            if self.mem.dma_active() {
                self.run_dma();
            }
            self.cpu_cycles += cycles;
            timeout_cycles += cycles;
            //self.mem.get_display().render::<C, P>(display);
            if self.cpu.is_dead(&mut self.mem) {
                /* cpu permanently halted */
                return GBReason::Dead;
            } else if new_interrupt & mask_u8!(InterruptFlag::VBlank) != 0 {
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
