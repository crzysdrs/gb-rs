use std::io::{Seek, SeekFrom, Read, Write};
use super::instr::{Instr, get_op, disasm};
use super::alu::{ALU, ALUOps};
use super::mmu::{MMU};
use super::{make_u16, split_u16};

macro_rules! alu_mem{
    ($s: expr, $mem:expr, $v:expr) => {
        alu_mem_mask!($s, $mem, $v, Registers::default_mask())
    }
}

macro_rules! alu_mem_mask{
    ($s: expr, $mem:expr, $v:expr, $m:expr) => {
        {
            let (res, flags) = $v;
            *$mem = res;
            $s.reg.write_mask(Reg8::F, flags, $m);
        }
    }
}

#[derive(Debug,PartialEq,Copy,Clone)]
pub enum Reg8 {
    A, F,
    B, C,
    D, E,
    H, L,
}

#[derive(Debug,PartialEq,Copy,Clone)]
pub enum Reg16 {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC
}

#[derive(Debug,PartialEq,Copy,Clone)]
pub enum Flag {
    Z = 1 << 7,
    N = 1 << 6,
    H = 1 << 5,
    C = 1 << 4,
}

#[derive(Debug,PartialEq)]
pub enum Cond {
    Z, NZ,
    C, NC,
    H, NH,
    N, NN,
}

#[derive(Debug,PartialEq)]
struct Registers {
    a : u8,
    f : u8,
    b : u8,
    c : u8,
    d : u8,
    e : u8,
    h : u8,
    l : u8,
    sp : u16,
    pc : u16,
    ime : u8,
}

pub struct CPU {
    reg : Registers,
    halted : bool,
    trace : bool,
}


trait RegType<Register> where
    Self::Output : std::ops::Not<Output=Self::Output> + std::ops::BitAnd<Output=Self::Output> + std::ops::BitOr<Output=Self::Output> + std::marker::Copy,
Register : std::marker::Copy
{
    type Output;
    fn write(&mut self, reg: Register, val : Self::Output);
    fn write_mask(&mut self, reg: Register, val: Self::Output, mask: Self::Output) {
        let old = self.read(reg);
        self.write(reg, (val & mask) | (old & !mask));
    }
    fn read(&self, reg: Register) -> Self::Output;
    fn read_mask(&self, reg: Register, mask : Self::Output) -> Self::Output {
        self.read(reg) & mask
    }
}

impl RegType<Reg8> for Registers {
    type Output = u8;
    fn write(&mut self, reg: Reg8, v: u8) {
        match reg {
            Reg8::A => {self.a = v;},
            Reg8::F => {self.f = v;},
            Reg8::B => {self.b = v;},
            Reg8::C => {self.c = v;},
            Reg8::D => {self.d = v;},
            Reg8::E => {self.e = v;},
            Reg8::H => {self.h = v;},
            Reg8::L => {self.l = v;},
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
                self.a = hi;
                self.f = lo;
            },
            Reg16::BC => {
                self.b = hi;
                self.c = lo;
            },
            Reg16::DE => {
                self.d = hi;
                self.e = lo;
            },
            Reg16::HL => {
                self.h = hi;
                self.l = lo;
            },
            Reg16::SP => {self.sp = v},
            Reg16::PC => {self.pc = v},
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
            a : 0,
            f : 0,
            b : 0,
            c : 0,
            d : 0,
            e : 0,
            h : 0,
            l : 0,
            sp : 0,
            pc : 0,
            ime : 0,
        }
    }
    fn dump(&self) {
        println!("{:?}", self);
    }
    fn set_flag(&mut self, f: Flag) {
        self.write_mask(Reg8::F, f as u8, f as u8);
    }
    fn clear_flag(&mut self, f: Flag) {
        self.write_mask(Reg8::F, 0, f as u8);
    }
    fn write_flags(&mut self, z: bool, n: bool, h: bool, c: bool, mask: u8) {
        let mut new_flags = 0;
        new_flags |= flag_u8!(Flag::Z, z);
        new_flags |= flag_u8!(Flag::N, n);
        new_flags |= flag_u8!(Flag::H, h);
        new_flags |= flag_u8!(Flag::C, c);
        self.write_mask(Reg8::F, new_flags, mask);
    }
    fn get_flag(&self, f : Flag) -> bool {
        self.read(Reg8::F) & (f as u8) != 0
    }
    fn default_mask() -> u8 {
        mask_u8!(Flag::Z | Flag::N | Flag::H | Flag::C)
    }
}

trait AnyReg :Copy {}
impl AnyReg for Reg8 {}
impl AnyReg for Reg16 {}


impl CPU {
    pub fn new(trace :bool ) -> CPU {
        CPU {
            reg : Registers::new(),
            halted : false,
            trace
        }
    }
    pub fn is_dead(&mut self, mut mem: &mut MMU) -> bool {
        self.halted && (self.reg.ime == 0 || *mem.find_byte(0xffff) == 0)
    }
    fn check_flag(&mut self, cond :Cond) -> bool {
        match cond {
            Cond::Z => self.reg.get_flag(Flag::Z),
            Cond::NZ => !self.reg.get_flag(Flag::Z),
            Cond::C => self.reg.get_flag(Flag::C),
            Cond::NC => !self.reg.get_flag(Flag::C),
            Cond::N => self.reg.get_flag(Flag::N),
            Cond::NN => !self.reg.get_flag(Flag::N),
            Cond::H => self.reg.get_flag(Flag::H),
            Cond::NH => !self.reg.get_flag(Flag::H),
        }
    }
    fn dump(&self) {
        self.reg.dump();
    }
    fn pop16(&mut self, mut mem: &mut MMU, t: Reg16) {
        let lo = self.pop8(&mut mem);
        let hi = self.pop8(&mut mem);
        let res = make_u16(hi, lo);
        //println!("Popping PC: {:04x} SP: {:04x} Val: {:04x}",  self.reg.read(Reg16::PC),  self.reg.sp, res);
        self.reg.write(t, res);
    }
    fn pop8(&mut self, mut mem: &mut MMU) -> u8 {
        let mut buf = [0u8; 1];
        let rloc = self.reg.read(Reg16::SP) + 1;
        mem.seek(SeekFrom::Start(rloc as u64));
        mem.read(&mut buf);
        self.reg.write(Reg16::SP, rloc);
        buf[0]
    }
    fn push8(&mut self, mut mem: &mut MMU, v: u8) {
        mem.seek(SeekFrom::Start(self.reg.read(Reg16::SP) as u64));
        mem.write(&[v]);
        self.reg.write(Reg16::SP, self.reg.read(Reg16::SP) - 1);
    }
    fn push16(&mut self, mut mem: &mut MMU, v: Reg16) {
        let item = self.reg.read(v);
        let (hi, lo) = split_u16(item);
        //println!("Pushing PC: {:04x} SP: {:04x} Val: {:04x}", self.reg.read(Reg16::PC), self.reg.sp, item);
        self.push8(&mut mem, hi);
        self.push8(&mut mem, lo);
    }
    pub fn execute(&mut self, mut mem: &mut MMU, cycles: u64) -> u32 {
        let pc = self.reg.read(Reg16::PC);
        if pc == 0x100 {
            mem.disableBios();
        }
        mem.seek(SeekFrom::Start(pc as u64));
        let (op, i) = match Instr::disasm(&mut mem) {
            Ok((_, Instr::INVALID(op))) => panic!("PC Invalid instruction {:x} @ {:x}", op, self.reg.pc),
            Ok((opcode, i)) => (opcode, i),
            Err(_) => panic!("Unable to read Instruction"),
        };
        if self.trace {
            mem.seek(SeekFrom::Start(pc as u64));
            let mut taken = mem.take(get_op(op).size as u64);
            let mut buf = std::io::Cursor::new(taken);
            let mut disasm_out = std::io::Cursor::new(Vec::new());
            disasm(pc, buf.get_mut(), &mut disasm_out, &|_| true);
            let vec = disasm_out.into_inner();
            let disasm_str = std::str::from_utf8(vec.as_ref()).unwrap();
            let f = self.reg.read(Reg8::F);
            let flag_str = format!("{z}{n}{h}{c}",
                                   z = if self.reg.get_flag(Flag::Z) {"Z"} else {"-"},
                                   n = if self.reg.get_flag(Flag::N) {"N"} else {"-"},
                                   h = if self.reg.get_flag(Flag::H) {"H"} else {"-"},
                                   c = if self.reg.get_flag(Flag::C) {"C"} else {"-"},
            );

            print!("A:{a:02X} F:{f} BC:{bc:04X} DE:{de:04x} HL:{hl:04x} SP:{sp:04x} PC:{pc:04x} (cy: {cycles}) ppu:+{ppu} |[??]{disasm}",
                     a=self.reg.read(Reg8::A),
                     f=flag_str,
                     bc=self.reg.read(Reg16::BC),
                     de=self.reg.read(Reg16::DE),
                     hl=self.reg.read(Reg16::HL),
                     sp=self.reg.read(Reg16::SP),
                     pc=self.reg.read(Reg16::PC),
                     cycles=cycles,
                     ppu=0,
                     disasm=disasm_str,
            );
            mem = buf.into_inner().into_inner();
        }
        self.reg.write(Reg16::PC, mem.get_current_pos());
        match i {
            Instr::ADC_r8_d8(x0, x1) => alu_result!(self, x0, ALU::adc(self.reg.read(x0), x1, self.reg.get_flag(Flag::C))),
            Instr::ADC_r8_ir16(x0, x1) => alu_result!(self, x0, ALU::adc(self.reg.read(x0), *mem.find_byte(self.reg.read(x1)), self.reg.get_flag(Flag::C))),
            Instr::ADC_r8_r8(x0, x1) => alu_result!(self, x0, ALU::adc(self.reg.read(x0), self.reg.read(x1), self.reg.get_flag(Flag::C))),
            Instr::ADD_r16_r16(x0, x1) => alu_result_mask!(self, x0, ALU::add(self.reg.read(x0), self.reg.read(x1)), mask_u8!(Flag::N | Flag::H | Flag::C)), /*TODO: Half Carry Not Being Set Correctly */
            Instr::ADD_r16_r8(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), x1 as i16 as u16)),
            Instr::ADD_r8_r8(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), self.reg.read(x1))),
            Instr::ADD_r8_d8(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), x1)),
            Instr::ADD_r8_ir16(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), *mem.find_byte(self.reg.read(x1)))),
            Instr::AND_d8(x0) => alu_result!(self, Reg8::A, ALU::and(self.reg.read(Reg8::A), x0)),
            Instr::AND_ir16(x0) => alu_result!(self, Reg8::A, ALU::and(self.reg.read(Reg8::A), *mem.find_byte(self.reg.read(x0)))),
            Instr::AND_r8(x0) => alu_result!(self, Reg8::A, ALU::and(self.reg.read(Reg8::A), self.reg.read(x0))),
            Instr::BIT_l8_ir16(x0, x1) => self.reg.write_mask(Reg8::F, ALU::bit(x0, *mem.find_byte(self.reg.read(x1))).1, mask_u8!(Flag::Z | Flag::H | Flag::N)),
            Instr::BIT_l8_r8(x0, x1) => self.reg.write_mask(Reg8::F, ALU::bit(x0, self.reg.read(x1)).1, mask_u8!(Flag::Z | Flag::H | Flag::N)),
            Instr::CALL_COND_a16(x0, x1) => {
                if self.check_flag(x0) {
                    self.push16(mem, Reg16::PC);
                    self.reg.write(Reg16::PC, x1);
                }
            },
            Instr::CALL_a16(x0) => {
                self.push16(mem, Reg16::PC);
                self.reg.write(Reg16::PC, x0);
            },
            Instr::CCF => {
                if self.check_flag(Cond::C) {
                    self.reg.clear_flag(Flag::C);
                } else {
                    self.reg.set_flag(Flag::C);
                }
                self.reg.clear_flag(Flag::N);
                self.reg.clear_flag(Flag::H);
            },
            Instr::CPL => {
                self.reg.write(Reg8::A, !self.reg.read(Reg8::A));
                self.reg.set_flag(Flag::N);
                self.reg.set_flag(Flag::H);
            },
            Instr::CP_d8(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), x0);
                self.reg.write_mask(Reg8::F, flags, Registers::default_mask());
            },
            Instr::CP_ir16(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), *mem.find_byte(self.reg.read(x0)));
                self.reg.write_mask(Reg8::F, flags, Registers::default_mask());
            },
            Instr::CP_r8(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), self.reg.read(x0));
                self.reg.write_mask(Reg8::F, flags, Registers::default_mask());
            },
            Instr::DAA => {
                let mut value = self.reg.read(Reg8::A) as i8;
                let mut adjust = 0;
                if self.check_flag(Cond::H) || (!self.check_flag(Cond::N) && (value & 0xf) > 0x9) {
                    adjust |= 0x6;
                }
                if self.check_flag(Cond::H) || (!self.check_flag(Cond::N) && value > 0x99) {
                    adjust |= 0x60;
                    self.reg.set_flag(Flag::C);
                } else {
                    self.reg.clear_flag(Flag::C);
                }

                value += if self.check_flag(Cond::N) {
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
            },
            Instr::DEC_ir16(x0) => alu_mem_mask!(self, mem.find_byte(self.reg.read(x0)), ALU::dec(*mem.find_byte(self.reg.read(x0))), mask_u8!(Flag::Z | Flag::N | Flag::H)),
            Instr::DEC_r16(x0) => alu_result_mask!(self, x0, ALU::dec(self.reg.read(x0)), 0),
            Instr::DEC_r8(x0) => alu_result_mask!(self, x0, ALU::dec(self.reg.read(x0)), mask_u8!(Flag::Z | Flag::N | Flag::H)),
            /* disable interrupts */
            Instr::DI => { self.reg.ime = 0; },
            /* enable interrupts */
            Instr::EI => { self.reg.ime = 1; },
            /* halt until next interrupt */
            Instr::HALT => { self.halted = true; },
            Instr::INC_ir16(x0) => alu_mem_mask!(self, mem.find_byte(self.reg.read(x0)), ALU::inc(*mem.find_byte(self.reg.read(x0))), mask_u8!(Flag::Z | Flag::N | Flag::H)),
            Instr::INC_r16(x0) => alu_result_mask!(self, x0, ALU::inc(self.reg.read(x0)), 0),
            Instr::INC_r8(x0) => alu_result_mask!(self, x0, ALU::inc(self.reg.read(x0)), mask_u8!(Flag::Z | Flag::N | Flag::H)),
            Instr::JP_COND_a16(x0, x1) => {
                if self.check_flag(x0) {
                    self.reg.write(Reg16::PC, x1)
                }
            },
            Instr::JP_a16(x0) => {
                self.reg.write(Reg16::PC, x0);
            },
            Instr::JP_r16(x0) => {
                self.reg.write(Reg16::PC, self.reg.read(x0));
            },
            Instr::JR_COND_r8(x0, x1) => {
                if self.check_flag(x0) {
                    self.reg.write(Reg16::PC, ALU::add(self.reg.read(Reg16::PC), x1 as i16 as u16).0);
                }
            },
            Instr::JR_r8(x0) => {
                self.reg.write(Reg16::PC, ALU::add(self.reg.read(Reg16::PC), x0 as i16 as u16).0);
            },
            Instr::LDH_ia8_r8(x0, x1) => {
                let b = mem.find_byte(0xff00 + x0 as u16);
                *b = self.reg.read(x1);
            },
            Instr::LDH_r8_ia8(x0, x1) => {
                let b = mem.find_byte(0xff00 + x1 as u16);
                self.reg.write(x0, *b);
            },
            Instr::LD_ia16_r16(x0, x1) => {
                mem.seek(SeekFrom::Start(x0 as u64));
                let (hi, lo) = split_u16(self.reg.read(x1));
                mem.write(&[hi, lo]);
            },
            Instr::LD_ia16_r8(x0, x1) => {
                let b = mem.find_byte(x0);
                *b = self.reg.read(x1);
            },
            Instr::LD_ir16_d8(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x0));
                *b = x1;
            },
            Instr::LD_ir16_r8(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x0));
                *b = self.reg.read(x1);
            },
            Instr::LD_iir16_r8(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x0));
                *b = self.reg.read(x1);
                self.reg.write(x0, ALU::inc(self.reg.read(x0)).0)
            },
            Instr::LD_dir16_r8(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x0));
                *b = self.reg.read(x1);
                self.reg.write(x0, ALU::dec(self.reg.read(x0)).0)
            },
            Instr::LD_ir8_r8(x0, x1) => {
                let b = mem.find_byte(0xff00 + self.reg.read(x0) as u16);
                *b = self.reg.read(x1);
            },
            Instr::LD_r16_d16(x0, x1) => {
                self.reg.write(x0, x1);
            },
            Instr::LD_r16_r16(x0, x1) => {
                self.reg.write(x0, self.reg.read(x1));
            }
            Instr::LD_r16_r16_r8(x0, x1, x2) => {
                self.reg.write(x0, (self.reg.read(x1) as i16 + x2 as i16) as u16);
            },
            Instr::LD_r8_d8(x0, x1) => {
                self.reg.write(x0, x1);
            },
            Instr::LD_r8_ia16(x0, x1) => {
                let b = mem.find_byte(x1);
                self.reg.write(x0, *b);
            },
            Instr::LD_r8_ir16(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x1));
                self.reg.write(x0, *b);
            },
            Instr::LD_r8_iir16(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x1));
                self.reg.write(x0, *b);
                self.reg.write(x1, ALU::inc(self.reg.read(x1)).0)
            },
            Instr::LD_r8_dir16(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x1));
                self.reg.write(x0, *b);
                self.reg.write(x1, ALU::dec(self.reg.read(x1)).0)
            },
            Instr::LD_r8_ir8(x0, x1) => {
                let b = mem.find_byte(0xff00 + self.reg.read(x1) as u16);
                self.reg.write(x0, *b)
            },
            Instr::LD_r8_r8(x0, x1) => {
                self.reg.write(x0, self.reg.read(x1));
            },
            Instr::NOP => {},
            Instr::OR_d8(x0) => alu_result!(self, Reg8::A, ALU::or(self.reg.read(Reg8::A), x0)),
            Instr::OR_ir16(x0) => alu_result!(self, Reg8::A, ALU::or(self.reg.read(Reg8::A), *mem.find_byte(self.reg.read(x0)))),
            Instr::OR_r8(x0) => alu_result!(self, Reg8::A, ALU::or(self.reg.read(Reg8::A), self.reg.read(x0))),
            Instr::POP_r16(x0) => self.pop16(&mut mem, x0),
            Instr::PUSH_r16(x0) => self.push16(&mut mem, x0),
            Instr::RES_l8_ir16(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x1));
                *b = (*b & !(1 << x0));
            },
            Instr::RES_l8_r8(x0, x1) => self.reg.write(x1, (self.reg.read(x1) & !(1 << x0))),
            Instr::RET => {
                self.pop16(&mut mem, Reg16::PC)
            },
            Instr::RETI => {
                self.pop16(&mut mem, Reg16::PC);
                /* TODO: and enable interrupts */
            },
            Instr::RET_COND(x0) => {
                if self.check_flag(x0) {
                    self.pop16(&mut mem, Reg16::PC);
                }
            },
            Instr::RLA => alu_result!(self, Reg8::A, ALU::rlca(self.reg.read(Reg8::A), 1, self.reg.get_flag(Flag::C), false)),
            Instr::RLCA => alu_result!(self, Reg8::A, ALU::rlca(self.reg.read(Reg8::A), 1, self.reg.get_flag(Flag::C), false)),
            Instr::RLC_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b, ALU::rlca(*b, 1, self.reg.get_flag(Flag::C), true));
            },
            Instr::RLC_r8(x0) => alu_result!(self, x0, ALU::rlca(self.reg.read(x0), 1, self.reg.get_flag(Flag::C), true)),
            Instr::RL_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b, ALU::rlca(*b, 1, self.reg.get_flag(Flag::C), true));
            },
            Instr::RL_r8(x0) => alu_result!(self, x0, ALU::rlca(self.reg.read(x0), 1, self.reg.get_flag(Flag::C), true)),
            Instr::RRA => alu_result!(self, Reg8::A, ALU::rrca(self.reg.read(Reg8::A), 1, self.reg.get_flag(Flag::C), false)),
            Instr::RRCA => alu_result!(self, Reg8::A, ALU::rrca(self.reg.read(Reg8::A), 1, self.reg.get_flag(Flag::C), false)),
            Instr::RRC_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b,  ALU::rrca(*b, 1, self.reg.get_flag(Flag::C), true));
            },
            Instr::RRC_r8(x0) => alu_result!(self, x0, ALU::rrca(self.reg.read(x0), 1, self.reg.get_flag(Flag::C), true)),
            Instr::RR_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b, ALU::rrca(*b, 1, self.reg.get_flag(Flag::C), true));
            },
            Instr::RR_r8(x0) => alu_result!(self, x0, ALU::rrca(self.reg.read(x0), 1, self.reg.get_flag(Flag::C), true)),
            Instr::RST_LIT(x0) => {
                self.push16(&mut mem, Reg16::PC);
                self.reg.write(Reg16::PC, x0 as u16);
            }
            Instr::SBC_r8_d8(x0, x1) =>  alu_result!(self, Reg8::A, ALU::sbc(self.reg.read(x0), x1, self.reg.get_flag(Flag::C))),
            Instr::SBC_r8_ir16(x0, x1) => alu_result!(self, Reg8::A, ALU::sbc(self.reg.read(x0), *mem.find_byte(self.reg.read(x1)), self.reg.get_flag(Flag::C))),
            Instr::SBC_r8_r8(x0, x1) =>  alu_result!(self, Reg8::A, ALU::sbc(self.reg.read(x0), self.reg.read(x1), self.reg.get_flag(Flag::C))),
            Instr::SCF => self.reg.set_flag(Flag::C),
            Instr::SET_l8_ir16(x0, x1) => {
                let b = mem.find_byte(self.reg.read(x1));
                *b = *b | 1 << x0;
            },
            Instr::SET_l8_r8(x0, x1) => self.reg.write(x1, self.reg.read(x1) | 1 << x0),
            Instr::SLA_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b,  ALU::sla(*b, 1));
            },
            Instr::SLA_r8(x0) => alu_result!(self, x0, ALU::sla(self.reg.read(x0), 1)),
            Instr::SRA_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b,  ALU::sr(*b, 1, true));
            },
            Instr::SRA_r8(x0) => alu_result!(self, x0, ALU::sr(self.reg.read(x0), 1, true)),
            Instr::SRL_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b,  ALU::sr(*b, 1, false));
            },
            Instr::SRL_r8(x0) => alu_result!(self, x0, ALU::sr(self.reg.read(x0), 1, false)),
            /* halt cpu and lcd display until button press */
            Instr::STOP_0(x0) => unimplemented!("Missing STOP"),
            Instr::SUB_d8(x0) => alu_result!(self, Reg8::A, ALU::sub(self.reg.read(Reg8::A), x0)),
            Instr::SUB_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_result!(self, Reg8::A, ALU::sub(self.reg.read(Reg8::A), *b));
            },
            Instr::SUB_r8(x0) => alu_result!(self, Reg8::A, ALU::sub(self.reg.read(Reg8::A), self.reg.read(x0))),
            Instr::SWAP_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b, ALU::swap(*b));
            },
            Instr::SWAP_r8(x0) => alu_result!(self, x0, ALU::swap(self.reg.read(x0))),
            Instr::XOR_d8(x0) => alu_result!(self, Reg8::A, ALU::xor(self.reg.read(Reg8::A), x0)),
            Instr::XOR_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_result!(self, Reg8::A, ALU::xor(self.reg.read(Reg8::A), *b));
            },
            Instr::XOR_r8(x0) => alu_result!(self, Reg8::A, ALU::xor(self.reg.read(Reg8::A), self.reg.read(x0))),
            Instr::INVALID(i) => panic!("Invalid Instruction {}", i),
        }
        get_op(op).cycles as u32
    }
}
