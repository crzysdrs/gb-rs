use super::cpu::*;
use super::mmu::*;
use display::LCD;
use dma::DMA;

use std::io::Write;
pub struct GB<'a> {
    cpu: CPU,
    mem: MMU<'a>,
    cpu_cycles :u64,
}

impl<'a> GB<'a> {
    pub fn new<'b>(rom: Vec<u8>, serial: Option<&'b mut Write>, trace: bool) -> GB<'b> {
        GB {
            cpu: CPU::new(trace),
            mem: MMU::new(rom, serial),
            cpu_cycles: 0,
        }
    }
    pub fn toggle_trace(&mut self) {
        self.cpu.toggle_trace()
    }
    fn update_interrupts(&mut self, cycles: u64) {
        let mut ps = self.mem.peripherals();
        let mut interrupt_flag = 0;
        for p in ps.iter_mut() {
            match p.step(cycles as u64) {
                Some(i) => {
                    interrupt_flag |= mask_u8!(i);
                }
                None => {}
            }
        }

        let rhs = self.mem.read_byte(0xff0f) | interrupt_flag;
        self.mem.write_byte(0xff0f, rhs);
    }

    fn run_dma(&mut self) {
        let fake_dma = DMA::new();
        let mut real_dma = self.mem.swap_dma(fake_dma);
        real_dma.run(&mut self.mem);
        self.mem.swap_dma(real_dma);
    }
    pub fn set_controls(&mut self, controls :u8) {
        self.mem.set_controls(controls);
    }
    pub fn step<C, P>(&mut self, time: u64, display: &mut Option<&mut LCD<C, P>>) -> bool
    where
        P: std::convert::From<(i32, i32)>,
        C: std::convert::From<(u8, u8, u8, u8)>,
    {
        //time in ms
        let mut timeout_cycles = 0;
        while time == 0 || timeout_cycles < 1_000_000 / 1_000 * time {
            let cycles: u64 = self.cpu.execute(&mut self.mem, self.cpu_cycles) as u64;

            self.update_interrupts(cycles);
            if self.mem.dma_active() {
                self.run_dma();
            }
            self.cpu_cycles += cycles;
            timeout_cycles += cycles;
            self.mem.get_display().render::<C, P>(display);
            if self.cpu.is_dead(&mut self.mem) {
                /* cpu permanently halted */
                break;
            }
        }
        //self.mem.get_display().dump();
        self.cpu.is_dead(&mut self.mem)
    }
    // #[cfg(test)]
    // pub fn run_instrs(&mut self, instrs: &[Instr]) {
    //     for i in instrs.iter_mut() {
    //         self.cpu.execute_instr(&mut self.mem, i);
    //     }
    // }
}
