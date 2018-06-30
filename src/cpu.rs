use super::alu::{ALUOps, ALU};
use super::instr::{disasm, get_op, Instr};
use super::mmu::MMU;
use super::{make_u16, split_u16};
use std::io::{Read, Seek, SeekFrom, Write};

macro_rules! alu_mem {
    ($s:expr, $mem:expr, $addr:expr, $v:expr) => {
        alu_mem_mask!($s, $mem, $addr, $v, Registers::default_mask())
    };
}

macro_rules! alu_mem_mask {
    ($s:expr, $mem:expr, $addr:expr, $v:expr, $m:expr) => {{
        let (res, flags) = $v;
        $mem.write_byte($addr, res);
        $s.reg.write_mask(Reg8::F, flags, $m);
    }};
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Reg8 {
    A,
    F,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Reg16 {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Flag {
    Z = 1 << 7,
    N = 1 << 6,
    H = 1 << 5,
    C = 1 << 4,
}

#[derive(Debug, PartialEq)]
pub enum Cond {
    Z,
    NZ,
    C,
    NC,
}

#[derive(Debug, PartialEq)]
struct Registers {
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
    ime: u8,
}

pub struct CPU {
    reg: Registers,
    halted: bool,
    dead: bool,
    trace: bool,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum InterruptFlag {
    HiLo = 1 << 4,
    Serial = 1 << 3,
    Timer = 1 << 2,
    LCDC = 1 << 1,
    VBlank = 1 << 0,
}

trait RegType<Register>
where
    Self::Output: std::ops::Not<Output = Self::Output>
        + std::ops::BitAnd<Output = Self::Output>
        + std::ops::BitOr<Output = Self::Output>
        + std::marker::Copy,
    Register: std::marker::Copy,
{
    type Output;
    fn write(&mut self, reg: Register, val: Self::Output);
    fn write_mask(&mut self, reg: Register, val: Self::Output, mask: Self::Output) {
        let old = self.read(reg);
        self.write(reg, (val & mask) | (old & !mask));
    }
    fn read(&self, reg: Register) -> Self::Output;
    fn read_mask(&self, reg: Register, mask: Self::Output) -> Self::Output {
        self.read(reg) & mask
    }
}

impl RegType<Reg8> for Registers {
    type Output = u8;
    fn write(&mut self, reg: Reg8, v: u8) {
        match reg {
            Reg8::A => {
                self.a = v;
            }
            Reg8::F => {
                self.f = v & Registers::default_mask();
            }
            Reg8::B => {
                self.b = v;
            }
            Reg8::C => {
                self.c = v;
            }
            Reg8::D => {
                self.d = v;
            }
            Reg8::E => {
                self.e = v;
            }
            Reg8::H => {
                self.h = v;
            }
            Reg8::L => {
                self.l = v;
            }
        }
    }
    fn read(&self, r: Reg8) -> u8 {
        match r {
            Reg8::A => self.a,
            Reg8::F => self.f,
            Reg8::B => self.b,
            Reg8::C => self.c,
            Reg8::D => self.d,
            Reg8::E => self.e,
            Reg8::H => self.h,
            Reg8::L => self.l,
        }
    }
}

impl RegType<Reg16> for Registers {
    type Output = u16;
    fn write(&mut self, r: Reg16, v: u16) {
        let (hi, lo) = split_u16(v);
        match r {
            Reg16::AF => {
                self.write(Reg8::A, hi);
                self.write(Reg8::F, lo);
            }
            Reg16::BC => {
                self.write(Reg8::B, hi);
                self.write(Reg8::C, lo);
            }
            Reg16::DE => {
                self.write(Reg8::D, hi);
                self.write(Reg8::E, lo);
            }
            Reg16::HL => {
                self.write(Reg8::H, hi);
                self.write(Reg8::L, lo);
            }
            Reg16::SP => self.sp = v,
            Reg16::PC => self.pc = v,
        }
    }
    fn read(&self, r: Reg16) -> u16 {
        match r {
            Reg16::AF => make_u16(self.a, self.f),
            Reg16::BC => make_u16(self.b, self.c),
            Reg16::DE => make_u16(self.d, self.e),
            Reg16::HL => make_u16(self.h, self.l),
            Reg16::SP => self.sp,
            Reg16::PC => self.pc,
        }
    }
}

impl Registers {
    pub fn new() -> Registers {
        Registers {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0,
            ime: 0,
        }
    }
    #[allow(dead_code)]
    fn dump(&self) {
        println!("{:?}", self);
    }
    fn set_flag(&mut self, f: Flag) {
        self.write_mask(Reg8::F, f as u8, f as u8);
    }
    fn clear_flag(&mut self, f: Flag) {
        self.write_mask(Reg8::F, 0, f as u8);
    }
    fn get_flag(&self, f: Flag) -> bool {
        self.read(Reg8::F) & (f as u8) != 0
    }
    fn default_mask() -> u8 {
        mask_u8!(Flag::Z | Flag::N | Flag::H | Flag::C)
    }
}

trait AnyReg: Copy {}
impl AnyReg for Reg8 {}
impl AnyReg for Reg16 {}

impl CPU {
    pub fn new(trace: bool) -> CPU {
        CPU {
            reg: Registers::new(),
            halted: false,
            dead: false,
            trace,
        }
    }
    pub fn is_dead(&mut self, _mem: &mut MMU) -> bool {
        self.dead
    }
    fn check_flag(&mut self, cond: Cond) -> bool {
        match cond {
            Cond::Z => self.reg.get_flag(Flag::Z),
            Cond::NZ => !self.reg.get_flag(Flag::Z),
            Cond::C => self.reg.get_flag(Flag::C),
            Cond::NC => !self.reg.get_flag(Flag::C),
        }
    }
    #[allow(dead_code)]
    fn dump(&self) {
        self.reg.dump();
    }
    fn manage_interrupt(&mut self, mem: &mut MMU) {
        let iflag = mem.read_byte(0xff0f);
        let ienable = mem.read_byte(0xffff);
        let interrupt = iflag & ienable;
        if interrupt == 0 {
            return;
        } else if self.reg.ime != 0 {
            self.reg.ime = 0;
            let addr = if interrupt & mask_u8!(InterruptFlag::HiLo) != 0 {
                0x0060
            } else if interrupt & mask_u8!(InterruptFlag::Serial) != 0 {
                0x0058
            } else if interrupt & mask_u8!(InterruptFlag::Timer) != 0 {
                0x0050
            } else if interrupt & mask_u8!(InterruptFlag::LCDC) != 0 {
                0x0048
            } else if interrupt & mask_u8!(InterruptFlag::VBlank) != 0 {
                0x0040
            } else {
                panic!("Unknown interrupt {:b}", interrupt);
            };
            let shift = interrupt.leading_zeros();
            //Clear highest interrupt
            mem.write_byte(0xff0f, iflag & !(0x80 >> shift));
            self.push16(mem, Reg16::PC);
            self.reg.write(Reg16::PC, addr);
            self.halted = false;
        } else if self.halted {
            self.halted = false;
            //TODO: Skip next instruction due to HALT DMG Bug
        }
    }
    pub fn toggle_trace(&mut self) {
        self.trace = !self.trace;
    }
    fn pop16(&mut self, mem: &mut MMU, t: Reg16) {
        mem.seek(SeekFrom::Start(self.reg.read(Reg16::SP) as u64))
            .expect("Can't request outside of memory");
        let mut buf = [0u8; 2];
        mem.read(&mut buf).expect("Memory wraps");
        let res = make_u16(buf[1], buf[0]);
        self.reg.write(t, res);
        self.reg.write(Reg16::SP, self.reg.read(Reg16::SP) + 2);
    }
    fn push16(&mut self, mem: &mut MMU, v: Reg16) {
        let item = self.reg.read(v);
        let (hi, lo) = split_u16(item);
        self.reg.write(Reg16::SP, self.reg.read(Reg16::SP) - 2);
        mem.seek(SeekFrom::Start(self.reg.read(Reg16::SP) as u64))
            .expect("Can't request outside of memory");
        mem.write(&[lo, hi]).expect("Memory wraps");
    }

    pub fn execute_instr(&mut self, mut mem: &mut MMU, instr: Instr) {
        match instr {
            Instr::ADC_r8_d8(x0, x1) => alu_result!(
                self,
                x0,
                ALU::adc(self.reg.read(x0), x1, self.reg.get_flag(Flag::C))
            ),
            Instr::ADC_r8_ir16(x0, x1) => alu_result!(
                self,
                x0,
                ALU::adc(
                    self.reg.read(x0),
                    mem.read_byte(self.reg.read(x1)),
                    self.reg.get_flag(Flag::C)
                )
            ),
            Instr::ADC_r8_r8(x0, x1) => alu_result!(
                self,
                x0,
                ALU::adc(
                    self.reg.read(x0),
                    self.reg.read(x1),
                    self.reg.get_flag(Flag::C)
                )
            ),
            Instr::ADD_r16_r16(x0, x1) => alu_result_mask!(
                self,
                x0,
                ALU::add(self.reg.read(x0), self.reg.read(x1)),
                mask_u8!(Flag::N | Flag::H | Flag::C)
            ), /*TODO: Half Carry Not Being Set Correctly */
            Instr::ADD_r16_r8(x0, x1) => {
                let (res, _) = ALU::add(self.reg.read(x0), x1 as i16 as u16);
                let (_, flags) = ALU::add(self.reg.read(x0) as u8, x1 as i16 as u8);
                alu_result!(self, x0, (res, flags & !mask_u8!(Flag::Z | Flag::N)))
            }
            Instr::ADD_r8_r8(x0, x1) => {
                alu_result!(self, x0, ALU::add(self.reg.read(x0), self.reg.read(x1)))
            }
            Instr::ADD_r8_d8(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), x1)),
            Instr::ADD_r8_ir16(x0, x1) => alu_result!(
                self,
                x0,
                ALU::add(self.reg.read(x0), mem.read_byte(self.reg.read(x1)))
            ),
            Instr::AND_d8(x0) => alu_result!(self, Reg8::A, ALU::and(self.reg.read(Reg8::A), x0)),
            Instr::AND_ir16(x0) => alu_result!(
                self,
                Reg8::A,
                ALU::and(self.reg.read(Reg8::A), mem.read_byte(self.reg.read(x0)))
            ),
            Instr::AND_r8(x0) => alu_result!(
                self,
                Reg8::A,
                ALU::and(self.reg.read(Reg8::A), self.reg.read(x0))
            ),
            Instr::BIT_l8_ir16(x0, x1) => self.reg.write_mask(
                Reg8::F,
                ALU::bit(x0, mem.read_byte(self.reg.read(x1))).1,
                mask_u8!(Flag::Z | Flag::H | Flag::N),
            ),
            Instr::BIT_l8_r8(x0, x1) => self.reg.write_mask(
                Reg8::F,
                ALU::bit(x0, self.reg.read(x1)).1,
                mask_u8!(Flag::Z | Flag::H | Flag::N),
            ),
            Instr::CALL_COND_a16(x0, x1) => {
                if self.check_flag(x0) {
                    self.push16(mem, Reg16::PC);
                    self.reg.write(Reg16::PC, x1);
                }
            }
            Instr::CALL_a16(x0) => {
                self.push16(mem, Reg16::PC);
                self.reg.write(Reg16::PC, x0);
            }
            Instr::CCF => {
                if self.check_flag(Cond::C) {
                    self.reg.clear_flag(Flag::C);
                } else {
                    self.reg.set_flag(Flag::C);
                }
                self.reg.clear_flag(Flag::N);
                self.reg.clear_flag(Flag::H);
            }
            Instr::CPL => {
                self.reg.write(Reg8::A, !self.reg.read(Reg8::A));
                self.reg.set_flag(Flag::N);
                self.reg.set_flag(Flag::H);
            }
            Instr::CP_d8(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), x0);
                self.reg
                    .write_mask(Reg8::F, flags, Registers::default_mask());
            }
            Instr::CP_ir16(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), mem.read_byte(self.reg.read(x0)));
                self.reg
                    .write_mask(Reg8::F, flags, Registers::default_mask());
            }
            Instr::CP_r8(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), self.reg.read(x0));
                self.reg
                    .write_mask(Reg8::F, flags, Registers::default_mask());
            }
            Instr::DAA => {
                let mut value = self.reg.read(Reg8::A) as u32 as i32;
                let mut adjust = 0;
                if self.reg.get_flag(Flag::H)
                    || (!self.reg.get_flag(Flag::N) && (value & 0xf) > 0x9)
                {
                    adjust |= 0x6;
                }
                if self.reg.get_flag(Flag::C) || (!self.reg.get_flag(Flag::N) && value > 0x99) {
                    adjust |= 0x60;
                    self.reg.set_flag(Flag::C);
                } else {
                    self.reg.clear_flag(Flag::C);
                }

                value += if self.reg.get_flag(Flag::N) {
                    -adjust
                } else {
                    adjust
                };
                value &= 0xff;
                if value == 0 {
                    self.reg.set_flag(Flag::Z);
                } else {
                    self.reg.clear_flag(Flag::Z);
                }
                self.reg.clear_flag(Flag::H);

                self.reg.write(Reg8::A, value as u8);
            }
            Instr::DEC_ir16(x0) => alu_mem_mask!(
                self,
                mem,
                self.reg.read(x0),
                ALU::dec(mem.read_byte(self.reg.read(x0))),
                mask_u8!(Flag::Z | Flag::N | Flag::H)
            ),
            Instr::DEC_r16(x0) => alu_result_mask!(self, x0, ALU::dec(self.reg.read(x0)), 0),
            Instr::DEC_r8(x0) => alu_result_mask!(
                self,
                x0,
                ALU::dec(self.reg.read(x0)),
                mask_u8!(Flag::Z | Flag::N | Flag::H)
            ),
            /* disable interrupts */
            Instr::DI => {
                self.reg.ime = 0;
            }
            /* enable interrupts */
            Instr::EI => {
                self.reg.ime = 1;
            }
            /* halt until next interrupt */
            Instr::HALT => {
                self.halted = true;
            }
            Instr::INC_ir16(x0) => alu_mem_mask!(
                self,
                mem,
                self.reg.read(x0),
                ALU::inc(mem.read_byte(self.reg.read(x0))),
                mask_u8!(Flag::Z | Flag::N | Flag::H)
            ),
            Instr::INC_r16(x0) => alu_result_mask!(self, x0, ALU::inc(self.reg.read(x0)), 0),
            Instr::INC_r8(x0) => alu_result_mask!(
                self,
                x0,
                ALU::inc(self.reg.read(x0)),
                mask_u8!(Flag::Z | Flag::N | Flag::H)
            ),
            Instr::JP_COND_a16(x0, x1) => {
                if self.check_flag(x0) {
                    self.reg.write(Reg16::PC, x1)
                }
            }
            Instr::JP_a16(x0) => {
                self.reg.write(Reg16::PC, x0);
            }
            Instr::JP_r16(x0) => {
                self.reg.write(Reg16::PC, self.reg.read(x0));
            }
            Instr::JR_COND_r8(x0, x1) => {
                if self.check_flag(x0) {
                    self.reg.write(
                        Reg16::PC,
                        ALU::add(self.reg.read(Reg16::PC), x1 as i16 as u16).0,
                    );
                }
            }
            Instr::JR_r8(x0) => {
                if x0 == -2 && (self.reg.ime == 0 || mem.read_byte(0xffff) == 0) {
                    /* infinite loop with no interrupts enabled */
                    self.dead = true;
                }
                self.reg.write(
                    Reg16::PC,
                    ALU::add(self.reg.read(Reg16::PC), x0 as i16 as u16).0,
                );
            }
            Instr::LDH_ia8_r8(x0, x1) => {
                mem.write_byte(0xff00 + x0 as u16, self.reg.read(x1));
            }
            Instr::LDH_r8_ia8(x0, x1) => {
                self.reg.write(x0, mem.read_byte(0xff00 + x1 as u16));
            }
            Instr::LD_ia16_r16(x0, x1) => {
                mem.seek(SeekFrom::Start(x0 as u64))
                    .expect("All addresses valid");
                let (hi, lo) = split_u16(self.reg.read(x1));
                mem.write(&[lo, hi]).expect("Memory wraps");
            }
            Instr::LD_ia16_r8(x0, x1) => {
                mem.write_byte(x0, self.reg.read(x1));
            }
            Instr::LD_ir16_d8(x0, x1) => mem.write_byte(self.reg.read(x0), x1),
            Instr::LD_ir16_r8(x0, x1) => {
                mem.write_byte(self.reg.read(x0), self.reg.read(x1));
            }
            Instr::LD_iir16_r8(x0, x1) => {
                mem.write_byte(self.reg.read(x0), self.reg.read(x1));
                self.reg.write(x0, ALU::inc(self.reg.read(x0)).0)
            }
            Instr::LD_dir16_r8(x0, x1) => {
                mem.write_byte(self.reg.read(x0), self.reg.read(x1));
                self.reg.write(x0, ALU::dec(self.reg.read(x0)).0)
            }
            Instr::LD_ir8_r8(x0, x1) => {
                mem.write_byte(0xff00 + self.reg.read(x0) as u16, self.reg.read(x1));
            }
            Instr::LD_r16_d16(x0, x1) => {
                self.reg.write(x0, x1);
            }
            Instr::LD_r16_r16(x0, x1) => {
                self.reg.write(x0, self.reg.read(x1));
            }
            Instr::LD_r16_r16_r8(x0, x1, x2) => {
                let (res, _) = ALU::add(self.reg.read(x1), x2 as i16 as u16);
                let (_, flags) = ALU::add(self.reg.read(x1) as u8, x2 as i16 as u8);
                alu_result!(self, x0, (res, flags & !mask_u8!(Flag::Z | Flag::N)))
            }
            Instr::LD_r8_d8(x0, x1) => {
                self.reg.write(x0, x1);
            }
            Instr::LD_r8_ia16(x0, x1) => {
                self.reg.write(x0, mem.read_byte(x1));
            }
            Instr::LD_r8_ir16(x0, x1) => {
                self.reg.write(x0, mem.read_byte(self.reg.read(x1)));
            }
            Instr::LD_r8_iir16(x0, x1) => {
                self.reg.write(x0, mem.read_byte(self.reg.read(x1)));
                self.reg.write(x1, ALU::inc(self.reg.read(x1)).0)
            }
            Instr::LD_r8_dir16(x0, x1) => {
                self.reg.write(x0, mem.read_byte(self.reg.read(x1)));
                self.reg.write(x1, ALU::dec(self.reg.read(x1)).0)
            }
            Instr::LD_r8_ir8(x0, x1) => self
                .reg
                .write(x0, mem.read_byte(0xff00 + self.reg.read(x1) as u16)),
            Instr::LD_r8_r8(x0, x1) => {
                self.reg.write(x0, self.reg.read(x1));
            }
            Instr::NOP => {}
            Instr::OR_d8(x0) => alu_result!(self, Reg8::A, ALU::or(self.reg.read(Reg8::A), x0)),
            Instr::OR_ir16(x0) => alu_result!(
                self,
                Reg8::A,
                ALU::or(self.reg.read(Reg8::A), mem.read_byte(self.reg.read(x0)))
            ),
            Instr::OR_r8(x0) => alu_result!(
                self,
                Reg8::A,
                ALU::or(self.reg.read(Reg8::A), self.reg.read(x0))
            ),
            Instr::POP_r16(x0) => self.pop16(&mut mem, x0),
            Instr::PUSH_r16(x0) => self.push16(&mut mem, x0),
            Instr::RES_l8_ir16(x0, x1) => {
                let rhs = mem.read_byte(self.reg.read(x1)) & !(1 << x0);
                mem.write_byte(self.reg.read(x1), rhs);
            }
            Instr::RES_l8_r8(x0, x1) => self.reg.write(x1, self.reg.read(x1) & !(1 << x0)),
            Instr::RET => self.pop16(&mut mem, Reg16::PC),
            Instr::RETI => {
                self.pop16(&mut mem, Reg16::PC);
                self.reg.ime = 1;
            }
            Instr::RET_COND(x0) => {
                if self.check_flag(x0) {
                    self.pop16(&mut mem, Reg16::PC);
                }
            }
            Instr::RLA => alu_result!(
                self,
                Reg8::A,
                ALU::rlca(
                    self.reg.read(Reg8::A),
                    self.reg.get_flag(Flag::C),
                    true,
                    false
                )
            ),
            Instr::RLCA => alu_result!(
                self,
                Reg8::A,
                ALU::rlca(
                    self.reg.read(Reg8::A),
                    self.reg.get_flag(Flag::C),
                    false,
                    false
                )
            ),
            Instr::RLC_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::rlca(
                        mem.read_byte(self.reg.read(x0)),
                        self.reg.get_flag(Flag::C),
                        false,
                        true
                    )
                );
            }
            Instr::RLC_r8(x0) => alu_result!(
                self,
                x0,
                ALU::rlca(self.reg.read(x0), self.reg.get_flag(Flag::C), false, true)
            ),
            Instr::RL_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::rlca(
                        mem.read_byte(self.reg.read(x0)),
                        self.reg.get_flag(Flag::C),
                        true,
                        true
                    )
                );
            }
            Instr::RL_r8(x0) => alu_result!(
                self,
                x0,
                ALU::rlca(self.reg.read(x0), self.reg.get_flag(Flag::C), true, true)
            ),
            Instr::RRA => alu_result!(
                self,
                Reg8::A,
                ALU::rrca(
                    self.reg.read(Reg8::A),
                    self.reg.get_flag(Flag::C),
                    true,
                    false
                )
            ),
            Instr::RRCA => alu_result!(
                self,
                Reg8::A,
                ALU::rrca(
                    self.reg.read(Reg8::A),
                    self.reg.get_flag(Flag::C),
                    false,
                    false
                )
            ),
            Instr::RRC_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::rrca(
                        mem.read_byte(self.reg.read(x0)),
                        self.reg.get_flag(Flag::C),
                        false,
                        true
                    )
                );
            }
            Instr::RRC_r8(x0) => alu_result!(
                self,
                x0,
                ALU::rrca(self.reg.read(x0), self.reg.get_flag(Flag::C), false, true)
            ),
            Instr::RR_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::rrca(
                        mem.read_byte(self.reg.read(x0)),
                        self.reg.get_flag(Flag::C),
                        true,
                        true
                    )
                );
            }
            Instr::RR_r8(x0) => alu_result!(
                self,
                x0,
                ALU::rrca(self.reg.read(x0), self.reg.get_flag(Flag::C), true, true)
            ),
            Instr::RST_LIT(x0) => {
                self.push16(&mut mem, Reg16::PC);
                self.reg.write(Reg16::PC, x0 as u16);
            }
            Instr::SBC_r8_d8(x0, x1) => alu_result!(
                self,
                Reg8::A,
                ALU::sbc(self.reg.read(x0), x1, self.reg.get_flag(Flag::C))
            ),
            Instr::SBC_r8_ir16(x0, x1) => alu_result!(
                self,
                Reg8::A,
                ALU::sbc(
                    self.reg.read(x0),
                    mem.read_byte(self.reg.read(x1)),
                    self.reg.get_flag(Flag::C)
                )
            ),
            Instr::SBC_r8_r8(x0, x1) => alu_result!(
                self,
                Reg8::A,
                ALU::sbc(
                    self.reg.read(x0),
                    self.reg.read(x1),
                    self.reg.get_flag(Flag::C)
                )
            ),
            Instr::SCF => self.reg.write_mask(
                Reg8::F,
                mask_u8!(Flag::C),
                mask_u8!(Flag::C | Flag::N | Flag::H),
            ),
            Instr::SET_l8_ir16(x0, x1) => {
                let rhs = mem.read_byte(self.reg.read(x1)) | 1 << x0;
                mem.write_byte(self.reg.read(x1), rhs);
            }
            Instr::SET_l8_r8(x0, x1) => self.reg.write(x1, self.reg.read(x1) | 1 << x0),
            Instr::SLA_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::sla(mem.read_byte(self.reg.read(x0)))
                );
            }
            Instr::SLA_r8(x0) => alu_result!(self, x0, ALU::sla(self.reg.read(x0))),
            Instr::SRA_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::sr(mem.read_byte(self.reg.read(x0)), true)
                );
            }
            Instr::SRA_r8(x0) => alu_result!(self, x0, ALU::sr(self.reg.read(x0), true)),
            Instr::SRL_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::sr(mem.read_byte(self.reg.read(x0)), false)
                );
            }
            Instr::SRL_r8(x0) => alu_result!(self, x0, ALU::sr(self.reg.read(x0), false)),
            /* halt cpu and lcd display until button press */
            Instr::STOP_0(_x0) => unimplemented!("Missing STOP"),
            Instr::SUB_d8(x0) => alu_result!(self, Reg8::A, ALU::sub(self.reg.read(Reg8::A), x0)),
            Instr::SUB_ir16(x0) => {
                alu_result!(
                    self,
                    Reg8::A,
                    ALU::sub(self.reg.read(Reg8::A), mem.read_byte(self.reg.read(x0)))
                );
            }
            Instr::SUB_r8(x0) => alu_result!(
                self,
                Reg8::A,
                ALU::sub(self.reg.read(Reg8::A), self.reg.read(x0))
            ),
            Instr::SWAP_ir16(x0) => {
                alu_mem!(
                    self,
                    mem,
                    self.reg.read(x0),
                    ALU::swap(mem.read_byte(self.reg.read(x0)))
                );
            }
            Instr::SWAP_r8(x0) => alu_result!(self, x0, ALU::swap(self.reg.read(x0))),
            Instr::XOR_d8(x0) => alu_result!(self, Reg8::A, ALU::xor(self.reg.read(Reg8::A), x0)),
            Instr::XOR_ir16(x0) => {
                alu_result!(
                    self,
                    Reg8::A,
                    ALU::xor(self.reg.read(Reg8::A), mem.read_byte(self.reg.read(x0)))
                );
            }
            Instr::XOR_r8(x0) => alu_result!(
                self,
                Reg8::A,
                ALU::xor(self.reg.read(Reg8::A), self.reg.read(x0))
            ),
            Instr::INVALID(instr) => panic!("Invalid Instruction {}", instr),
        }
    }
    pub fn execute(&mut self, mut mem: &mut MMU, cycles: u64) -> u32 {
        self.manage_interrupt(mem);

        if self.halted {
            return 1; /* claim one cycle has passed */
        }
        let pc = self.reg.read(Reg16::PC);
        if pc == 0x100 {
            mem.disable_bios();
        }

        mem.seek(SeekFrom::Start(pc as u64))
            .expect("All memory valid");
        let (op, i) = match Instr::disasm(&mut mem) {
            Ok((_, Instr::INVALID(op))) => {
                panic!("PC Invalid instruction {:x} @ {:x}", op, self.reg.pc)
            }
            Ok((opcode, i)) => (opcode, i),
            Err(_) => panic!("Unable to read Instruction"),
        };
        let next_pc = mem.get_current_pos();
        if self.trace {
            let ienable = mem.read_byte(0xffff);
            let iflag = mem.read_byte(0xff0f);
            mem.seek(SeekFrom::Start(pc as u64))
                .expect("All memory valid");
            let taken = mem.take(get_op(op).size as u64);
            let mut buf = std::io::Cursor::new(taken);
            let mut disasm_out = std::io::Cursor::new(Vec::new());
            disasm(pc, buf.get_mut(), &mut disasm_out, &|_| true).expect("Memory is all valid");
            let vec = disasm_out.into_inner();
            let disasm_str = std::str::from_utf8(vec.as_ref()).unwrap();

            print!("A:{:02X} ", self.reg.read(Reg8::A));
            print!(
                "F:{z}{n}{h}{c} ",
                z = if self.reg.get_flag(Flag::Z) { "Z" } else { "-" },
                n = if self.reg.get_flag(Flag::N) { "N" } else { "-" },
                h = if self.reg.get_flag(Flag::H) { "H" } else { "-" },
                c = if self.reg.get_flag(Flag::C) { "C" } else { "-" },
            );
            print!("BC:{:04X} ", self.reg.read(Reg16::BC));
            print!("DE:{:04x} ", self.reg.read(Reg16::DE));
            print!("HL:{:04x} ", self.reg.read(Reg16::HL));
            print!("SP:{:04x} ", self.reg.read(Reg16::SP));
            print!("PC:{:04x} ", self.reg.read(Reg16::PC));
            if self.trace && true {
                print!("IF:{:02x} ", iflag);
                print!("IE:{:02x} ", ienable);
                print!("IME:{:01x} ", self.reg.ime);
            }
            print!("(cy: {}) ", cycles);
            print!("ppu:+{} ", 0);
            print!("|[??]{}", disasm_str);
            mem = buf.into_inner().into_inner();
        }
        self.reg.write(Reg16::PC, next_pc);
        self.execute_instr(&mut mem, i);
        get_op(op).cycles as u32
    }
}

#[cfg(test)]
mod tests {
    use cpu::{Reg8, RegType, CPU};
    use instr::Instr;
    use mmu::MMU;

    macro_rules! test_state {
        ($instr:expr, $reg:expr, $val:expr) => {
            let mut mem = MMU::new(Vec::new(), None);
            let mut cpu = CPU::new(false);
            for i in $instr {
                cpu.execute_instr(&mut mem, i)
            }
            assert_eq!(cpu.reg.read($reg), $val);
        };
    }

    #[test]
    fn targeted() {
        test_state!(
            vec![Instr::ADD_r8_d8(Reg8::A, 0x05), Instr::DAA],
            Reg8::A,
            0x05
        );
        test_state!(
            vec![
                Instr::ADD_r8_d8(Reg8::A, 0x05),
                Instr::ADD_r8_d8(Reg8::A, 0x05),
                Instr::DAA,
            ],
            Reg8::A,
            0x10
        );
        test_state!(
            vec![
                Instr::ADD_r8_d8(Reg8::A, 0x05),
                Instr::ADD_r8_d8(Reg8::A, 0x15),
                Instr::DAA,
            ],
            Reg8::A,
            0x20
        );
        test_state!(
            vec![
                Instr::ADD_r8_d8(Reg8::A, 0x17),
                Instr::ADD_r8_d8(Reg8::A, 0x39),
                Instr::DAA,
            ],
            Reg8::A,
            0x56
        );
        test_state!(
            vec![
                Instr::ADD_r8_d8(Reg8::A, 0x17),
                Instr::SUB_d8(0x09),
                Instr::DAA,
            ],
            Reg8::A,
            0x08
        );
        test_state!(
            vec![
                Instr::ADD_r8_d8(Reg8::A, 0x32),
                Instr::SUB_d8(0x09),
                Instr::DAA,
            ],
            Reg8::A,
            0x23
        );
        test_state!(
            vec![
                Instr::ADD_r8_d8(Reg8::A, 0x05),
                Instr::SUB_d8(0x04),
                Instr::DAA,
            ],
            Reg8::A,
            0x01
        );

        /* why does the opcode having C not go through the carry? How confusing. */
        test_state!(
            vec![Instr::LD_r8_d8(Reg8::A, 0x05), Instr::RRA],
            Reg8::A,
            0x05u8 >> 1
        );
        test_state!(
            vec![Instr::LD_r8_d8(Reg8::A, 0x05), Instr::RRCA],
            Reg8::A,
            0x05u8.rotate_right(1)
        );
        test_state!(
            vec![Instr::LD_r8_d8(Reg8::A, 0x05), Instr::SCF, Instr::RRA],
            Reg8::A,
            0x80 | 0x05u8 >> 1
        );
        test_state!(
            vec![Instr::LD_r8_d8(Reg8::A, 0x80), Instr::RLA],
            Reg8::A,
            0x80u8 << 1
        );
        test_state!(
            vec![Instr::LD_r8_d8(Reg8::A, 0x80), Instr::RLCA],
            Reg8::A,
            0x80u8.rotate_left(1)
        );
        test_state!(
            vec![Instr::LD_r8_d8(Reg8::A, 0x80), Instr::SCF, Instr::RLA],
            Reg8::A,
            0x80u8 << 1 | 1
        );

        test_state!(
            vec![Instr::ADD_r8_d8(Reg8::A, 0x23), Instr::SWAP_r8(Reg8::A)],
            Reg8::A,
            0x32
        );
        test_state!(
            vec![Instr::ADD_r8_d8(Reg8::A, 0x71), Instr::SWAP_r8(Reg8::A)],
            Reg8::A,
            0x17
        );
    }
}
