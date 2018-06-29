use super::cpu::*;
use super::mmu::*;
use display::LCD;

use std::io::Write;
pub struct GB<'a> {
    cpu: CPU,
    mem: MMU<'a>,
}

impl<'a> GB<'a> {
    pub fn new<'b>(rom: Vec<u8>, serial: Option<&'b mut Write>, trace: bool) -> GB<'b> {
        GB {
            cpu: CPU::new(trace),
            mem: MMU::new(rom, serial),
        }
    }
    pub fn step<C, P>(&mut self, time: u64, display: &mut Option<&mut LCD<C, P>>) -> bool
    where
        P: std::convert::From<(i32, i32)>,
        C: std::convert::From<(u8, u8, u8, u8)>,
    {
        let mut cpu_cycles = 0;
        //time in ms
        while time == 0 || cpu_cycles < 4_000_000 / 1_000 * time {
            let cycles: u64 = self.cpu.execute(&mut self.mem, cpu_cycles) as u64;
            let mut ps = self.mem.peripherals();
            let mut interrupt_flag = 0;
            for p in ps.iter_mut() {
                match p.step(cycles as u64) {
                    Some(i) => {
                        interrupt_flag |= mask_u8!(i);
                    },
                    None => {},
                }
            }

            {
                let b = self.mem.find_byte(0xff0f);
                *b = *b | interrupt_flag;
            }
            cpu_cycles += cycles;
            self.mem.get_display().render::<C, P>(display);
            if self.cpu.is_dead(&mut self.mem) {
                /* cpu permanently halted */
                break;
            }
        }
        self.cpu.is_dead(&mut self.mem)
    }
    // #[cfg(test)]
    // pub fn run_instrs(&mut self, instrs: &[Instr]) {
    //     for i in instrs.iter_mut() {
    //         self.cpu.execute_instr(&mut self.mem, i);
    //     }
    // }
}
