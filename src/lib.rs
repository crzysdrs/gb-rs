#![feature(match_default_bindings)]
#![feature(nll)]

use std::io::{Seek, SeekFrom, Read, Write};
use std::io;
use std::fs::File;
use std::fmt;

macro_rules! flag_u8 {
    ($x:path, $cond:expr) => {
        if $cond {
            $x as u8
        } else {
            0
        }
    }
}

macro_rules! mask_u8 {
    ($($x:path)|* ) => {
        0
            $(
                | flag_u8!($x, true)
            )*
    }
}

#[derive(Debug)]
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
}

struct CPU {
    reg : Registers,
}

fn make_u16(h : u8, l: u8) -> u16 {
    ((h as u16) << 8) | (l as u16)
}

fn split_u16(r : u16) -> (u8, u8) {
    (((r & 0xff00) >> 8) as u8, (r & 0xff) as u8)
}

trait RegType<Register> where
    Self::Output : std::ops::Not<Output=Self::Output> + std::ops::BitAnd<Output=Self::Output> + std::ops::BitOr<Output=Self::Output> + Copy,
Register : Copy
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
     fn new() -> Registers {
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

struct ALU {

}

impl ALU {
    fn and(a : u8, b: u8) -> (u8, u8) {
        let res = a & b;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::H, true)
        )
    }
    fn xor(a : u8, b :u8) -> (u8, u8) {
        let res = a ^ b;
        (res, flag_u8!(Flag::Z, res == 0))
    }
    fn or(a : u8, b :u8) -> (u8, u8) {
        let res = a | b;
        (res, flag_u8!(Flag::Z, res == 0))
    }
    fn bit(a : u8, b : u8) -> (u8, u8) {
        let res = (1 << a) & b;
        (res,  flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, true)
         | flag_u8!(Flag::H, false)
        )
    }
    fn adc(a:u8, b: u8, c : bool) -> (u8, u8) {
        let (mut res, mut c) = a.overflowing_add(b);
        let mut h = Self::half_carry(a, b);
        if c {
            let (res2, c2) = res.overflowing_add(1);
            h = h || Self::half_carry(res, 1);
            c = c || c2;
            res = res2;
        }
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, h)
         | flag_u8!(Flag::C, c)
        )
    }
    fn sbc(a:u8, b: u8, c : bool) -> (u8, u8) {
        let (mut res, mut c) = a.overflowing_sub(b);
        let mut h = Self::sub_carry(a, b);
        if c {
            let (res2, c2) = res.overflowing_sub(1);
            h = h || Self::sub_carry(res, 1);
            c = c || c2;
            res = res2;
        }
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, true)
         | flag_u8!(Flag::H, h)
         | flag_u8!(Flag::C, c)
        )
    }

    fn rlca(a: u8, count: u32, c: bool) -> (u8, u8) {
        let res = a.rotate_left(count) | if c {1} else {0};
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, a & 0b1000_0000 > 0)
        )
    }
    fn rrca(a: u8, count: u32, c: bool) -> (u8, u8) {
        let res = a.rotate_right(count) | if c {1} else {0};
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, a & 0b1000_0000 > 0)
        )
    }

    fn sla(a: u8, count: u32) -> (u8, u8) {
        let res = a << count;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, a & 0b1000_0000 > 0)
        )
    }
    fn sra(a: u8, count: u32) -> (u8, u8) {
        let res = a >> count;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, a & 0b0000_0001 > 0)
        )
    }

    fn swap(a: u8) -> (u8, u8) {
        let res = a & 0x0f << 4 | a & 0xf0 >> 4;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, false)
        )
    }
}
trait ALUOps<T> {
    fn add(a : T, b : T) -> (T, u8);
    fn sub(a : T, b : T) -> (T, u8);
    fn dec(a : T) -> (T, u8);
    fn inc(a : T) -> (T, u8);
    fn half_carry(a : T, b : T) -> bool;
    fn sub_carry(a: T, b : T) -> bool;
}

impl ALUOps<u8> for ALU {
    fn half_carry(a: u8, b: u8) -> bool {
        ((a & 0xF) + (b & 0xF)) == 0x10
    }
    fn sub_carry(a: u8, b : u8) -> bool {
        a & 0xf < b & 0xf
    }
    fn add(a : u8, b: u8) -> (u8, u8) {
        let (mut res, mut c) = a.overflowing_add(b);
        let mut h = Self::half_carry(a, b);
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, h)
         | flag_u8!(Flag::C, c)
        )
    }
    fn sub(a : u8, b : u8) -> (u8, u8) {
        let (mut res, mut c) = a.overflowing_sub(b);
        let h = Self::sub_carry(a, b);
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, true)
         | flag_u8!(Flag::H, h)
         | flag_u8!(Flag::C, c)
        )
    }
    fn dec(a : u8) -> (u8, u8) {
        let (res, flags) = Self::sub(a, 1);
        (res, (flags & !(Flag::C as u8)))
    }
    fn inc(a : u8) -> (u8, u8) {
        let (res, flags) = Self::add(a, 1);
        (res, (flags & !(Flag::C as u8)))
    }
}

impl ALUOps<u16> for ALU {
    fn half_carry(a: u16, b: u16) -> bool {
        ((a & 0xF) + (b & 0xF)) == 0x10
    }
    fn sub_carry(a: u16, b : u16) -> bool {
        a & 0xf < b & 0xf
    }
    fn add(a : u16, b: u16) -> (u16, u8) {
        let (mut res, mut c) = a.overflowing_add(b);
        let mut h = Self::half_carry(a, b);
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, h)
         | flag_u8!(Flag::C, c)
        )
    }
    fn sub(a : u16, b : u16) -> (u16, u8) {
        let (mut res, mut c) = a.overflowing_sub(b);
        let mut h = a & 0xf < b & 0xf;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, true)
         | flag_u8!(Flag::H, h)
         | flag_u8!(Flag::C, c)
        )
    }
    fn dec(a : u16) -> (u16, u8) {
        let (res, flags) = Self::sub(a, 1);
        (res, (flags & !(Flag::C as u8)))
    }
    fn inc(a : u16) -> (u16, u8) {
        let (res, flags) = Self::add(a, 1);
        (res, (flags & !(Flag::C as u8)))
    }
}

macro_rules! alu_result {
    ($s: expr, $r:expr, $v:expr) => {
        {
            let (res, flags) = $v;
            $s.reg.write($r, res);
            $s.reg.write_mask(Reg8::F, flags, Registers::default_mask());
        }
    }
}

macro_rules! alu_mem{
    ($s: expr, $mem:expr, $v:expr) => {
        {
            let (res, flags) = $v;
            *$mem = res;
            $s.reg.write_mask(Reg8::F, flags, Registers::default_mask());
        }
    }
}


impl CPU {
    fn new() -> CPU {
        CPU { reg : Registers::new() }
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
    fn pop16(&mut self, mut mem: &mut GBMemory, t: Reg16) {
        let mut buf = [0u8; 2];
        mem.seek(SeekFrom::Start((self.reg.read(Reg16::SP) + 2) as u64));
        mem.read(&mut buf);
        self.reg.write(Reg16::SP, self.reg.read(Reg16::SP) + 2);
        let res = make_u16(buf[0], buf[1]);
        println!("Pop {} {}", self.reg.read(Reg16::SP), res);
        self.reg.write(t, res);
    }
    fn push16(&mut self, mut mem: &mut GBMemory, v: Reg16) {
        let item = self.reg.read(v);
        println!("Push {} {}", self.reg.read(Reg16::SP), item);
        let (hi, lo) = split_u16(item);
        mem.seek(SeekFrom::Start(self.reg.read(Reg16::SP) as u64));
        mem.write(&[hi, lo]);
        self.reg.write(Reg16::SP, self.reg.read(Reg16::SP) - 2);
    }
    fn execute(&mut self, mut mem: &mut GBMemory) {
        mem.seek(SeekFrom::Start(self.reg.read(Reg16::PC) as u64));
        let i = match Instr::disasm(&mut mem) {
            Ok((_, Instr::INVALID(_))) => panic!("Invalid instruction"),
            Ok((opcode, i)) => i,
            Err(_) => panic!("Unable to read Instruction"),
        };
        self.reg.write(Reg16::PC, mem.seek_pos);
        match i {
            Instr::ADC_r8_d8(x0, x1) => alu_result!(self, x0, ALU::adc(self.reg.read(x0), x1, self.reg.get_flag(Flag::C))),
            Instr::ADC_r8_ir16(x0, x1) => alu_result!(self, x0, ALU::adc(self.reg.read(x0), *mem.find_byte(self.reg.read(x1)), self.reg.get_flag(Flag::C))),
            Instr::ADC_r8_r8(x0, x1) => alu_result!(self, x0, ALU::adc(self.reg.read(x0), self.reg.read(x1), self.reg.get_flag(Flag::C))),
            Instr::ADD_r16_r16(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), self.reg.read(x1))),
            Instr::ADD_r16_r8(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), x1 as i16 as u16)),
            Instr::ADD_r8_r8(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), self.reg.read(x1))),
            Instr::ADD_r8_d8(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), x1)),
            Instr::ADD_r8_ir16(x0, x1) => alu_result!(self, x0, ALU::add(self.reg.read(x0), *mem.find_byte(self.reg.read(x1)))),
            Instr::AND_d8(x0) => alu_result!(self, Reg8::A, ALU::and(self.reg.read(Reg8::A), x0)),
            Instr::AND_ir16(x0) => alu_result!(self, Reg8::A, ALU::and(self.reg.read(Reg8::A), *mem.find_byte(self.reg.read(x0)))),
            Instr::AND_r8(x0) => alu_result!(self, Reg8::A, ALU::and(self.reg.read(Reg8::A), self.reg.read(x0))),
            Instr::BIT_l8_ir16(x0, x1) => self.reg.write(Reg8::F, ALU::bit(x0, *mem.find_byte(self.reg.read(x1))).1),
            Instr::BIT_l8_r8(x0, x1) => self.reg.write(Reg8::F, ALU::bit(x0, self.reg.read(x1)).1),
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
                self.reg.write(Reg8::F, flags);
            },
            Instr::CP_ir16(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), *mem.find_byte(self.reg.read(x0)));
                self.reg.write(Reg8::F, flags);
            },
            Instr::CP_r8(x0) => {
                let (_, flags) = ALU::sub(self.reg.read(Reg8::A), self.reg.read(x0));
                self.reg.write(Reg8::F, flags);
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
            Instr::DEC_ir16(x0) => {
                let (res, flags) = ALU::dec(*mem.find_byte(self.reg.read(x0)));
                *mem.find_byte(self.reg.read(x0)) = res;
                self.reg.write(Reg8::F, flags);
            },
            Instr::DEC_r16(x0) => alu_result!(self, x0, ALU::dec(self.reg.read(x0))),
            Instr::DEC_r8(x0) => alu_result!(self, x0, ALU::dec(self.reg.read(x0))),
            /* disable interrupts */
            Instr::DI => unimplemented!("Missing DI"),
            /* enable interrupts */
            Instr::EI => unimplemented!("Missing EI"),
            /* halt until next interrupt */
            Instr::HALT => unimplemented!("Missing HALT"),
            Instr::INC_ir16(x0) => {
                let (res, flags) = ALU::inc(*mem.find_byte(self.reg.read(x0)));
                *mem.find_byte(self.reg.read(x0)) = res;
                self.reg.write(Reg8::F, flags);
            },
            Instr::INC_r16(x0) => alu_result!(self, x0, ALU::inc(self.reg.read(x0))),
            Instr::INC_r8(x0) => alu_result!(self, x0, ALU::inc(self.reg.read(x0))),
            Instr::JP_COND_a16(x0, x1) => {
                if self.check_flag(x0) {
                    self.reg.write(Reg16::PC, x1)
                }
            },
            Instr::JP_a16(x0) => {
                self.reg.write(Reg16::PC, x0);
            },
            Instr::JP_ir16(x0) => {
                let mut buf = [0u8; 2];
                mem.seek(SeekFrom::Start(self.reg.read(x0) as u64));
                mem.read(&mut buf);
                self.reg.write(Reg16::PC, make_u16(buf[1], buf[0]));
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
                *b = (*b & !(1 << x0)) | (1 << x0);
            },
            Instr::RES_l8_r8(x0, x1) => self.reg.write(x1, (self.reg.read(x1) & !(1 << x0)) | (1 << x0)),
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
            Instr::RLA => alu_result!(self, Reg8::A, ALU::rlca(self.reg.read(Reg8::A), 1, false)),
            Instr::RLCA => alu_result!(self, Reg8::A, ALU::rlca(self.reg.read(Reg8::A), 1, true)),
            Instr::RLC_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b, ALU::rlca(*b, 1, true));
            },
            Instr::RLC_r8(x0) => alu_result!(self, x0, ALU::rlca(self.reg.read(x0), 1, true)),
            Instr::RL_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b, ALU::rlca(*b, 1, false));
            },
            Instr::RL_r8(x0) => alu_result!(self, x0, ALU::rlca(self.reg.read(x0), 1, false)),
            Instr::RRA => alu_result!(self, Reg8::A, ALU::rrca(self.reg.read(Reg8::A), 1, false)),
            Instr::RRCA => alu_result!(self, Reg8::A, ALU::rrca(self.reg.read(Reg8::A), 1, true)),
            Instr::RRC_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b,  ALU::rrca(*b, 1, true));
            },
            Instr::RRC_r8(x0) => alu_result!(self, x0, ALU::rrca(self.reg.read(x0), 1, true)),
            Instr::RR_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b,ALU::rrca(*b, 1, false));
            },
            Instr::RR_r8(x0) => alu_result!(self, x0, ALU::rrca(self.reg.read(x0), 1, false)),
            Instr::RST_LIT(x0) => {
                self.push16(&mut mem, Reg16::PC);
                self.reg.write(Reg16::PC, x0 as u16);
            }
            Instr::SBC_r8_d8(x0, x1) =>  alu_result!(self, x0, ALU::sbc(self.reg.read(x0), x1, self.reg.get_flag(Flag::C))),
            Instr::SBC_r8_ir16(x0, x1) => alu_result!(self, x0, ALU::sbc(self.reg.read(x0), *mem.find_byte(self.reg.read(x1)), self.reg.get_flag(Flag::C))),
            Instr::SBC_r8_r8(x0, x1) =>  alu_result!(self, x0, ALU::sbc(self.reg.read(x0), self.reg.read(x1), self.reg.get_flag(Flag::C))),
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
                alu_mem!(self, b,  ALU::sra(*b, 1));
            },
            Instr::SRA_r8(x0) => alu_result!(self, x0, ALU::sra(self.reg.read(x0), 1)),
            Instr::SRL_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b,  ALU::sra(*b, 1));
            },
            Instr::SRL_r8(x0) => alu_result!(self, x0, ALU::sla(self.reg.read(x0), 1)),
            /* halt cpu and lcd display until button press */
            Instr::STOP_0(x0) => unimplemented!("Missing STOP"),
            Instr::SUB_d8(x0) => alu_result!(self, Reg8::A, ALU::sub(self.reg.read(Reg8::A), x0)),
            Instr::SUB_ir16(x0) => {
                let b = mem.find_byte(self.reg.read(x0));
                alu_mem!(self, b, ALU::sub(self.reg.read(Reg8::A), *b));
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
                alu_mem!(self, b, ALU::xor(self.reg.read(Reg8::A), *b));
            },
            Instr::XOR_r8(x0) => alu_result!(self, Reg8::A, ALU::xor(self.reg.read(Reg8::A), self.reg.read(x0))),
            Instr::INVALID(i) => panic!("Invalid Instruction {}", i),
        }
    }
}

struct GBMemory {
    ram1 : Vec<u8>,
    empty1 : Vec<u8>,
    io : Vec<u8>,
    empty0 : Vec<u8>,
    sprites : Vec<u8>,
    //Echo of ram
    ram0 : Vec<u8>,
    swap_ram : Vec<u8>,
    video : Vec<u8>,
    rom1 : Vec<u8>,
    rom0 : Vec<u8>,
    seek_pos: u16,
}

impl GBMemory {
    fn new() -> GBMemory {
        let mut mem = GBMemory {
            rom0 : vec![0u8; 16 << 10],
            rom1 : vec![0u8; 16 << 10],
            video : vec![0u8; 8 << 10],
            swap_ram : vec![0u8; 8 << 10],
            ram0 : vec![0u8; 8 << 10],
            sprites : vec![0u8; 0xA0],
            empty0 : vec![0u8; 0xFF00 - 0xFEA0],
            io: vec![0u8; 0xFF4C - 0xFEA0],
            empty1 : vec![0u8; 0xFF80 - 0xFF4C],
            ram1 : vec![0u8; 0x10000 - 0xFF80],
            seek_pos: 0,
        };
        let bytes = include_bytes!("../boot_rom.gb");
        mem.seek(SeekFrom::Start(0));
        mem.write(bytes);
        mem
    }

    fn find_byte(&mut self, addr : u16) -> &mut u8 {
        /* these should really be bitwise operations */
        match addr {
            0x0000...0x3FFF => &mut self.rom0[addr as usize],
            0x4000...0x7FFF => &mut self.rom1[(addr - 0x4000) as usize],
            0x8000...0x9FFF => &mut self.video[(addr - 0x8000) as usize],
            0xA000...0xBFFF => &mut self.swap_ram[(addr - 0xA000) as usize],
            0xC000...0xDFFF => &mut self.ram0[(addr - 0xC000) as usize],
            0xE000...0xFDFF => &mut self.ram0[(addr - 0xE000) as usize],
            0xFE00...0xFE9F => &mut self.sprites[(addr - 0xFE00) as usize],
            0xFEA0...0xFEFF => &mut self.empty0[(addr - 0xFEA0) as usize],
            0xFF00...0xFF4B => &mut self.io[(addr - 0xFF00) as usize],
            0xFF4C...0xFF7F => &mut self.empty1[(addr - 0xFF4C) as usize],
            0xFF80...0xFFFF => &mut self.ram1[(addr - 0xFF80) as usize],
            _ => panic!("Memory Access Out Of Range")
        }
    }

    fn dump(&mut self) {
        self.seek(SeekFrom::Start(0));
        disasm(0, self, &mut std::io::stdout(), &|i| match i {Instr::NOP => false, _ => true});
    }
}

impl Write for GBMemory {
    fn write(&mut self, buf : &[u8]) -> io::Result<usize> {
        for (i, w) in buf.iter().enumerate() {
            let pos = self.seek_pos;
            {
                let b = self.find_byte(pos);
                *b = *w;
            }
            if self.seek_pos == std::u16::MAX {
                return Ok(i)
            }
            self.seek_pos = self.seek_pos.saturating_add(1);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for GBMemory {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for (i, b) in buf.iter_mut().enumerate() {
            {
                let pos = self.seek_pos;
                let &mut r = self.find_byte(pos);
                *b = r;
            }
            if self.seek_pos == std::u16::MAX {
                return Ok(i)
            }
            self.seek_pos = self.seek_pos.saturating_add(1);
        }
        Ok(buf.len())
    }
}
fn apply_offset(mut pos : u16,  seek : i64) -> io::Result<u64> {
    let seek = if seek > std::i16::MAX as i64 {
        std::i16::MAX
    } else if seek < std::i16::MIN as i64 {
        std::i16::MIN
    } else {
        seek as i16
    };
    if seek > 0 {
        pos = pos.saturating_add(seek as u16);
    } else if pos.checked_sub(seek as u16).is_some() {
        pos -= seek as u16;
    } else {
        return Err(std::io::Error::new(io::ErrorKind::Other, "seeked before beginning"));
    }
    Ok(pos as u64)
}


impl Seek for GBMemory {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(x) => {
                let x = if x > std::u16::MAX as u64{ std::u16::MAX } else { x as u16};
                self.seek_pos = 0u16.saturating_add(x);
            },
            SeekFrom::End(x) => {
                self.seek_pos = apply_offset(0xffff, x)? as u16;
            },
            SeekFrom::Current(x) => {
                self.seek_pos = apply_offset(self.seek_pos, x)? as u16;
            }
        }
        Ok(self.seek_pos as u64)
    }
}

#[derive(Debug,PartialEq,Copy,Clone)]
enum Reg8 {
    A, F,
    B, C,
    D, E,
    H, L,
}

#[derive(Debug,PartialEq,Copy,Clone)]
enum Reg16 {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC
}

#[derive(Debug,PartialEq,Copy,Clone)]
enum Register {
    A, F, AF,
    B, C, BC,
    D, E, DE,
    H, L, HL,
    SP,
    PC
}
#[derive(Debug,PartialEq)]
enum Immed {
    Immed8(i8),
    Immed16(i16),
    Addr8(u8),
    Addr16(u16),
    Offset(i8),
}

#[derive(Debug,PartialEq,Copy,Clone)]
enum Flag {
    Z = 1 << 7,
    N = 1 << 6,
    H = 1 << 5,
    C = 1 << 4,
}

#[derive(Debug,PartialEq)]
enum Cond {
    Z, NZ,
    C, NC,
    H, NH,
    N, NN,
}
// #[derive(Debug,PartialEq)]
// enum Term {
//     Register,
//     Immed,
// }

// #[derive(Debug,PartialEq)]
// enum Operand {
//     Op(Term),
//     OpIndirect(Term),
// }

#[derive(Debug,PartialEq)]
enum Instr {
    ADC_r8_d8(Reg8, u8),
    ADC_r8_ir16(Reg8, Reg16),
    ADC_r8_r8(Reg8, Reg8),
    ADD_r16_r16(Reg16, Reg16),
    ADD_r16_r8(Reg16, i8),
    ADD_r8_d8(Reg8, u8),
    ADD_r8_ir16(Reg8, Reg16),
    ADD_r8_r8(Reg8, Reg8),
    AND_d8(u8),
    AND_ir16(Reg16),
    AND_r8(Reg8),
    BIT_l8_ir16(u8, Reg16),
    BIT_l8_r8(u8, Reg8),
    CALL_COND_a16(Cond, u16),
    CALL_a16(u16),
    CCF,
    CPL,
    CP_d8(u8),
    CP_ir16(Reg16),
    CP_r8(Reg8),
    DAA,
    DEC_ir16(Reg16),
    DEC_r16(Reg16),
    DEC_r8(Reg8),
    DI,
    EI,
    HALT,
    INC_ir16(Reg16),
    INC_r16(Reg16),
    INC_r8(Reg8),
    INVALID(u16),
    JP_COND_a16(Cond, u16),
    JP_a16(u16),
    JP_ir16(Reg16),
    JR_COND_r8(Cond, i8),
    JR_r8(i8),
    LDH_ia8_r8(u8, Reg8),
    LDH_r8_ia8(Reg8, u8),
    LD_ia16_r16(u16, Reg16),
    LD_ia16_r8(u16, Reg8),
    LD_ir16_d8(Reg16, u8),
    LD_ir16_r8(Reg16, Reg8),
    LD_iir16_r8(Reg16, Reg8),
    LD_dir16_r8(Reg16, Reg8),
    LD_ir8_r8(Reg8, Reg8),
    LD_r16_d16(Reg16, u16),
    LD_r16_r16(Reg16, Reg16),
    LD_r16_r16_r8(Reg16, Reg16, i8),
    LD_r8_d8(Reg8, u8),
    LD_r8_ia16(Reg8, u16),
    LD_r8_ir16(Reg8, Reg16),
    LD_r8_iir16(Reg8, Reg16),
    LD_r8_dir16(Reg8, Reg16),
    LD_r8_ir8(Reg8, Reg8),
    LD_r8_r8(Reg8, Reg8),
    NOP,
    OR_d8(u8),
    OR_ir16(Reg16),
    OR_r8(Reg8),
    POP_r16(Reg16),
    PUSH_r16(Reg16),
    RES_l8_ir16(u8, Reg16),
    RES_l8_r8(u8, Reg8),
    RET,
    RETI,
    RET_COND(Cond),
    RLA,
    RLCA,
    RLC_ir16(Reg16),
    RLC_r8(Reg8),
    RL_ir16(Reg16),
    RL_r8(Reg8),
    RRA,
    RRCA,
    RRC_ir16(Reg16),
    RRC_r8(Reg8),
    RR_ir16(Reg16),
    RR_r8(Reg8),
    RST_LIT(u8),
    SBC_r8_d8(Reg8, u8),
    SBC_r8_ir16(Reg8, Reg16),
    SBC_r8_r8(Reg8, Reg8),
    SCF,
    SET_l8_ir16(u8, Reg16),
    SET_l8_r8(u8, Reg8),
    SLA_ir16(Reg16),
    SLA_r8(Reg8),
    SRA_ir16(Reg16),
    SRA_r8(Reg8),
    SRL_ir16(Reg16),
    SRL_r8(Reg8),
    STOP_0(u8),
    SUB_d8(u8),
    SUB_ir16(Reg16),
    SUB_r8(Reg8),
    SWAP_ir16(Reg16),
    SWAP_r8(Reg8),
    XOR_d8(u8),
    XOR_ir16(Reg16),
    XOR_r8(Reg8),
}

struct OpCode {
    mnemonic : & 'static str,
    size : u8,
    cycles : u8,
    cycles_false : Option<u8>,
}

static OPCODES : [OpCode; 512] = [
    OpCode { mnemonic : "NOP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 3, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLCA", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 3, cycles: 20, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRCA", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "STOP", size : 2, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 3, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLA", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "JR", size : 2, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRA", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "JR", size : 2, cycles: 12, cycles_false: Some(8) },
    OpCode { mnemonic : "LD", size : 3, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "DAA", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "JR", size : 2, cycles: 12, cycles_false: Some(8) },
    OpCode { mnemonic : "ADD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "CPL", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "JR", size : 2, cycles: 12, cycles_false: Some(8) },
    OpCode { mnemonic : "LD", size : 3, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "SCF", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "JR", size : 2, cycles: 12, cycles_false: Some(8) },
    OpCode { mnemonic : "ADD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "DEC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "CCF", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "HALT", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "AND", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "OR", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "CP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "RET", size : 1, cycles: 20, cycles_false: Some(8) },
    OpCode { mnemonic : "POP", size : 1, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "JP", size : 3, cycles: 16, cycles_false: Some(12) },
    OpCode { mnemonic : "JP", size : 3, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "CALL", size : 3, cycles: 24, cycles_false: Some(12) },
    OpCode { mnemonic : "PUSH", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RET", size : 1, cycles: 20, cycles_false: Some(8) },
    OpCode { mnemonic : "RET", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "JP", size : 3, cycles: 16, cycles_false: Some(12) },
    OpCode { mnemonic : "CB", size : 2, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "CALL", size : 3, cycles: 24, cycles_false: Some(12) },
    OpCode { mnemonic : "CALL", size : 3, cycles: 24, cycles_false: None },
    OpCode { mnemonic : "ADC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RET", size : 1, cycles: 20, cycles_false: Some(8) },
    OpCode { mnemonic : "POP", size : 1, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "JP", size : 3, cycles: 16, cycles_false: Some(12) },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "CALL", size : 3, cycles: 24, cycles_false: Some(12) },
    OpCode { mnemonic : "PUSH", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SUB", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RET", size : 1, cycles: 20, cycles_false: Some(8) },
    OpCode { mnemonic : "RETI", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "JP", size : 3, cycles: 16, cycles_false: Some(12) },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "CALL", size : 3, cycles: 24, cycles_false: Some(12) },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "SBC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "LDH", size : 2, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "POP", size : 1, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "PUSH", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "AND", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "ADD", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "JP", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "LD", size : 3, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "XOR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "LDH", size : 2, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "POP", size : 1, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "DI", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "PUSH", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "OR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "LD", size : 2, cycles: 12, cycles_false: None },
    OpCode { mnemonic : "LD", size : 1, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "LD", size : 3, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "EI", size : 1, cycles: 4, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "INVALID", size : 1, cycles: 1, cycles_false: None },
    OpCode { mnemonic : "CP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RST", size : 1, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RLC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RRC", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RR", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SLA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SRA", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SWAP", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SRL", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "BIT", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "RES", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 16, cycles_false: None },
    OpCode { mnemonic : "SET", size : 2, cycles: 8, cycles_false: None },
];

fn get_op(opcode :u16) -> &'static OpCode {
    if opcode & 0xCB00 == 0xCB00 {
        &OPCODES[(1 << 8 | (opcode & 0xFF)) as usize]
    } else if (opcode & 0xFF) == opcode {
        &OPCODES[opcode as usize]
    } else {
        panic!("opcode out of range");
    }
}

fn read_u8<R:Read>(bytes: &mut R) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    bytes.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16<R:Read>(bytes: &mut R) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    bytes.read_exact(&mut buf)?;
    Ok(((buf[1] as u16) << 8) | (buf[0] as u16))
}

impl Instr {
    fn prefix_cb_disasm<R: Read>(bytes : &mut R) -> io::Result<(u16, Instr)> {
        let mut opcode = [0u8; 1];
        bytes.read_exact(&mut opcode)?;
        let real = (0xCB as u16) << 8 | (opcode[0] as u16);
        let i = match opcode[0] {
            0x00 => Instr::RLC_r8(Reg8::B),
            0x01 => Instr::RLC_r8(Reg8::C),
            0x02 => Instr::RLC_r8(Reg8::D),
            0x03 => Instr::RLC_r8(Reg8::E),
            0x04 => Instr::RLC_r8(Reg8::H),
            0x05 => Instr::RLC_r8(Reg8::L),
            0x06 => Instr::RLC_ir16(Reg16::HL),
            0x07 => Instr::RLC_r8(Reg8::A),
            0x08 => Instr::RRC_r8(Reg8::B),
            0x09 => Instr::RRC_r8(Reg8::C),
            0x0a => Instr::RRC_r8(Reg8::D),
            0x0b => Instr::RRC_r8(Reg8::E),
            0x0c => Instr::RRC_r8(Reg8::H),
            0x0d => Instr::RRC_r8(Reg8::L),
            0x0e => Instr::RRC_ir16(Reg16::HL),
            0x0f => Instr::RRC_r8(Reg8::A),
            0x10 => Instr::RL_r8(Reg8::B),
            0x11 => Instr::RL_r8(Reg8::C),
            0x12 => Instr::RL_r8(Reg8::D),
            0x13 => Instr::RL_r8(Reg8::E),
            0x14 => Instr::RL_r8(Reg8::H),
            0x15 => Instr::RL_r8(Reg8::L),
            0x16 => Instr::RL_ir16(Reg16::HL),
            0x17 => Instr::RL_r8(Reg8::A),
            0x18 => Instr::RR_r8(Reg8::B),
            0x19 => Instr::RR_r8(Reg8::C),
            0x1a => Instr::RR_r8(Reg8::D),
            0x1b => Instr::RR_r8(Reg8::E),
            0x1c => Instr::RR_r8(Reg8::H),
            0x1d => Instr::RR_r8(Reg8::L),
            0x1e => Instr::RR_ir16(Reg16::HL),
            0x1f => Instr::RR_r8(Reg8::A),
            0x20 => Instr::SLA_r8(Reg8::B),
            0x21 => Instr::SLA_r8(Reg8::C),
            0x22 => Instr::SLA_r8(Reg8::D),
            0x23 => Instr::SLA_r8(Reg8::E),
            0x24 => Instr::SLA_r8(Reg8::H),
            0x25 => Instr::SLA_r8(Reg8::L),
            0x26 => Instr::SLA_ir16(Reg16::HL),
            0x27 => Instr::SLA_r8(Reg8::A),
            0x28 => Instr::SRA_r8(Reg8::B),
            0x29 => Instr::SRA_r8(Reg8::C),
            0x2a => Instr::SRA_r8(Reg8::D),
            0x2b => Instr::SRA_r8(Reg8::E),
            0x2c => Instr::SRA_r8(Reg8::H),
            0x2d => Instr::SRA_r8(Reg8::L),
            0x2e => Instr::SRA_ir16(Reg16::HL),
            0x2f => Instr::SRA_r8(Reg8::A),
            0x30 => Instr::SWAP_r8(Reg8::B),
            0x31 => Instr::SWAP_r8(Reg8::C),
            0x32 => Instr::SWAP_r8(Reg8::D),
            0x33 => Instr::SWAP_r8(Reg8::E),
            0x34 => Instr::SWAP_r8(Reg8::H),
            0x35 => Instr::SWAP_r8(Reg8::L),
            0x36 => Instr::SWAP_ir16(Reg16::HL),
            0x37 => Instr::SWAP_r8(Reg8::A),
            0x38 => Instr::SRL_r8(Reg8::B),
            0x39 => Instr::SRL_r8(Reg8::C),
            0x3a => Instr::SRL_r8(Reg8::D),
            0x3b => Instr::SRL_r8(Reg8::E),
            0x3c => Instr::SRL_r8(Reg8::H),
            0x3d => Instr::SRL_r8(Reg8::L),
            0x3e => Instr::SRL_ir16(Reg16::HL),
            0x3f => Instr::SRL_r8(Reg8::A),
            0x40 => Instr::BIT_l8_r8(0, Reg8::B),
            0x41 => Instr::BIT_l8_r8(0, Reg8::C),
            0x42 => Instr::BIT_l8_r8(0, Reg8::D),
            0x43 => Instr::BIT_l8_r8(0, Reg8::E),
            0x44 => Instr::BIT_l8_r8(0, Reg8::H),
            0x45 => Instr::BIT_l8_r8(0, Reg8::L),
            0x46 => Instr::BIT_l8_ir16(0, Reg16::HL),
            0x47 => Instr::BIT_l8_r8(0, Reg8::A),
            0x48 => Instr::BIT_l8_r8(1, Reg8::B),
            0x49 => Instr::BIT_l8_r8(1, Reg8::C),
            0x4a => Instr::BIT_l8_r8(1, Reg8::D),
            0x4b => Instr::BIT_l8_r8(1, Reg8::E),
            0x4c => Instr::BIT_l8_r8(1, Reg8::H),
            0x4d => Instr::BIT_l8_r8(1, Reg8::L),
            0x4e => Instr::BIT_l8_ir16(1, Reg16::HL),
            0x4f => Instr::BIT_l8_r8(1, Reg8::A),
            0x50 => Instr::BIT_l8_r8(2, Reg8::B),
            0x51 => Instr::BIT_l8_r8(2, Reg8::C),
            0x52 => Instr::BIT_l8_r8(2, Reg8::D),
            0x53 => Instr::BIT_l8_r8(2, Reg8::E),
            0x54 => Instr::BIT_l8_r8(2, Reg8::H),
            0x55 => Instr::BIT_l8_r8(2, Reg8::L),
            0x56 => Instr::BIT_l8_ir16(2, Reg16::HL),
            0x57 => Instr::BIT_l8_r8(2, Reg8::A),
            0x58 => Instr::BIT_l8_r8(3, Reg8::B),
            0x59 => Instr::BIT_l8_r8(3, Reg8::C),
            0x5a => Instr::BIT_l8_r8(3, Reg8::D),
            0x5b => Instr::BIT_l8_r8(3, Reg8::E),
            0x5c => Instr::BIT_l8_r8(3, Reg8::H),
            0x5d => Instr::BIT_l8_r8(3, Reg8::L),
            0x5e => Instr::BIT_l8_ir16(3, Reg16::HL),
            0x5f => Instr::BIT_l8_r8(3, Reg8::A),
            0x60 => Instr::BIT_l8_r8(4, Reg8::B),
            0x61 => Instr::BIT_l8_r8(4, Reg8::C),
            0x62 => Instr::BIT_l8_r8(4, Reg8::D),
            0x63 => Instr::BIT_l8_r8(4, Reg8::E),
            0x64 => Instr::BIT_l8_r8(4, Reg8::H),
            0x65 => Instr::BIT_l8_r8(4, Reg8::L),
            0x66 => Instr::BIT_l8_ir16(4, Reg16::HL),
            0x67 => Instr::BIT_l8_r8(4, Reg8::A),
            0x68 => Instr::BIT_l8_r8(5, Reg8::B),
            0x69 => Instr::BIT_l8_r8(5, Reg8::C),
            0x6a => Instr::BIT_l8_r8(5, Reg8::D),
            0x6b => Instr::BIT_l8_r8(5, Reg8::E),
            0x6c => Instr::BIT_l8_r8(5, Reg8::H),
            0x6d => Instr::BIT_l8_r8(5, Reg8::L),
            0x6e => Instr::BIT_l8_ir16(5, Reg16::HL),
            0x6f => Instr::BIT_l8_r8(5, Reg8::A),
            0x70 => Instr::BIT_l8_r8(6, Reg8::B),
            0x71 => Instr::BIT_l8_r8(6, Reg8::C),
            0x72 => Instr::BIT_l8_r8(6, Reg8::D),
            0x73 => Instr::BIT_l8_r8(6, Reg8::E),
            0x74 => Instr::BIT_l8_r8(6, Reg8::H),
            0x75 => Instr::BIT_l8_r8(6, Reg8::L),
            0x76 => Instr::BIT_l8_ir16(6, Reg16::HL),
            0x77 => Instr::BIT_l8_r8(6, Reg8::A),
            0x78 => Instr::BIT_l8_r8(7, Reg8::B),
            0x79 => Instr::BIT_l8_r8(7, Reg8::C),
            0x7a => Instr::BIT_l8_r8(7, Reg8::D),
            0x7b => Instr::BIT_l8_r8(7, Reg8::E),
            0x7c => Instr::BIT_l8_r8(7, Reg8::H),
            0x7d => Instr::BIT_l8_r8(7, Reg8::L),
            0x7e => Instr::BIT_l8_ir16(7, Reg16::HL),
            0x7f => Instr::BIT_l8_r8(7, Reg8::A),
            0x80 => Instr::RES_l8_r8(0, Reg8::B),
            0x81 => Instr::RES_l8_r8(0, Reg8::C),
            0x82 => Instr::RES_l8_r8(0, Reg8::D),
            0x83 => Instr::RES_l8_r8(0, Reg8::E),
            0x84 => Instr::RES_l8_r8(0, Reg8::H),
            0x85 => Instr::RES_l8_r8(0, Reg8::L),
            0x86 => Instr::RES_l8_ir16(0, Reg16::HL),
            0x87 => Instr::RES_l8_r8(0, Reg8::A),
            0x88 => Instr::RES_l8_r8(1, Reg8::B),
            0x89 => Instr::RES_l8_r8(1, Reg8::C),
            0x8a => Instr::RES_l8_r8(1, Reg8::D),
            0x8b => Instr::RES_l8_r8(1, Reg8::E),
            0x8c => Instr::RES_l8_r8(1, Reg8::H),
            0x8d => Instr::RES_l8_r8(1, Reg8::L),
            0x8e => Instr::RES_l8_ir16(1, Reg16::HL),
            0x8f => Instr::RES_l8_r8(1, Reg8::A),
            0x90 => Instr::RES_l8_r8(2, Reg8::B),
            0x91 => Instr::RES_l8_r8(2, Reg8::C),
            0x92 => Instr::RES_l8_r8(2, Reg8::D),
            0x93 => Instr::RES_l8_r8(2, Reg8::E),
            0x94 => Instr::RES_l8_r8(2, Reg8::H),
            0x95 => Instr::RES_l8_r8(2, Reg8::L),
            0x96 => Instr::RES_l8_ir16(2, Reg16::HL),
            0x97 => Instr::RES_l8_r8(2, Reg8::A),
            0x98 => Instr::RES_l8_r8(3, Reg8::B),
            0x99 => Instr::RES_l8_r8(3, Reg8::C),
            0x9a => Instr::RES_l8_r8(3, Reg8::D),
            0x9b => Instr::RES_l8_r8(3, Reg8::E),
            0x9c => Instr::RES_l8_r8(3, Reg8::H),
            0x9d => Instr::RES_l8_r8(3, Reg8::L),
            0x9e => Instr::RES_l8_ir16(3, Reg16::HL),
            0x9f => Instr::RES_l8_r8(3, Reg8::A),
            0xa0 => Instr::RES_l8_r8(4, Reg8::B),
            0xa1 => Instr::RES_l8_r8(4, Reg8::C),
            0xa2 => Instr::RES_l8_r8(4, Reg8::D),
            0xa3 => Instr::RES_l8_r8(4, Reg8::E),
            0xa4 => Instr::RES_l8_r8(4, Reg8::H),
            0xa5 => Instr::RES_l8_r8(4, Reg8::L),
            0xa6 => Instr::RES_l8_ir16(4, Reg16::HL),
            0xa7 => Instr::RES_l8_r8(4, Reg8::A),
            0xa8 => Instr::RES_l8_r8(5, Reg8::B),
            0xa9 => Instr::RES_l8_r8(5, Reg8::C),
            0xaa => Instr::RES_l8_r8(5, Reg8::D),
            0xab => Instr::RES_l8_r8(5, Reg8::E),
            0xac => Instr::RES_l8_r8(5, Reg8::H),
            0xad => Instr::RES_l8_r8(5, Reg8::L),
            0xae => Instr::RES_l8_ir16(5, Reg16::HL),
            0xaf => Instr::RES_l8_r8(5, Reg8::A),
            0xb0 => Instr::RES_l8_r8(6, Reg8::B),
            0xb1 => Instr::RES_l8_r8(6, Reg8::C),
            0xb2 => Instr::RES_l8_r8(6, Reg8::D),
            0xb3 => Instr::RES_l8_r8(6, Reg8::E),
            0xb4 => Instr::RES_l8_r8(6, Reg8::H),
            0xb5 => Instr::RES_l8_r8(6, Reg8::L),
            0xb6 => Instr::RES_l8_ir16(6, Reg16::HL),
            0xb7 => Instr::RES_l8_r8(6, Reg8::A),
            0xb8 => Instr::RES_l8_r8(7, Reg8::B),
            0xb9 => Instr::RES_l8_r8(7, Reg8::C),
            0xba => Instr::RES_l8_r8(7, Reg8::D),
            0xbb => Instr::RES_l8_r8(7, Reg8::E),
            0xbc => Instr::RES_l8_r8(7, Reg8::H),
            0xbd => Instr::RES_l8_r8(7, Reg8::L),
            0xbe => Instr::RES_l8_ir16(7, Reg16::HL),
            0xbf => Instr::RES_l8_r8(7, Reg8::A),
            0xc0 => Instr::SET_l8_r8(0, Reg8::B),
            0xc1 => Instr::SET_l8_r8(0, Reg8::C),
            0xc2 => Instr::SET_l8_r8(0, Reg8::D),
            0xc3 => Instr::SET_l8_r8(0, Reg8::E),
            0xc4 => Instr::SET_l8_r8(0, Reg8::H),
            0xc5 => Instr::SET_l8_r8(0, Reg8::L),
            0xc6 => Instr::SET_l8_ir16(0, Reg16::HL),
            0xc7 => Instr::SET_l8_r8(0, Reg8::A),
            0xc8 => Instr::SET_l8_r8(1, Reg8::B),
            0xc9 => Instr::SET_l8_r8(1, Reg8::C),
            0xca => Instr::SET_l8_r8(1, Reg8::D),
            0xcb => Instr::SET_l8_r8(1, Reg8::E),
            0xcc => Instr::SET_l8_r8(1, Reg8::H),
            0xcd => Instr::SET_l8_r8(1, Reg8::L),
            0xce => Instr::SET_l8_ir16(1, Reg16::HL),
            0xcf => Instr::SET_l8_r8(1, Reg8::A),
            0xd0 => Instr::SET_l8_r8(2, Reg8::B),
            0xd1 => Instr::SET_l8_r8(2, Reg8::C),
            0xd2 => Instr::SET_l8_r8(2, Reg8::D),
            0xd3 => Instr::SET_l8_r8(2, Reg8::E),
            0xd4 => Instr::SET_l8_r8(2, Reg8::H),
            0xd5 => Instr::SET_l8_r8(2, Reg8::L),
            0xd6 => Instr::SET_l8_ir16(2, Reg16::HL),
            0xd7 => Instr::SET_l8_r8(2, Reg8::A),
            0xd8 => Instr::SET_l8_r8(3, Reg8::B),
            0xd9 => Instr::SET_l8_r8(3, Reg8::C),
            0xda => Instr::SET_l8_r8(3, Reg8::D),
            0xdb => Instr::SET_l8_r8(3, Reg8::E),
            0xdc => Instr::SET_l8_r8(3, Reg8::H),
            0xdd => Instr::SET_l8_r8(3, Reg8::L),
            0xde => Instr::SET_l8_ir16(3, Reg16::HL),
            0xdf => Instr::SET_l8_r8(3, Reg8::A),
            0xe0 => Instr::SET_l8_r8(4, Reg8::B),
            0xe1 => Instr::SET_l8_r8(4, Reg8::C),
            0xe2 => Instr::SET_l8_r8(4, Reg8::D),
            0xe3 => Instr::SET_l8_r8(4, Reg8::E),
            0xe4 => Instr::SET_l8_r8(4, Reg8::H),
            0xe5 => Instr::SET_l8_r8(4, Reg8::L),
            0xe6 => Instr::SET_l8_ir16(4, Reg16::HL),
            0xe7 => Instr::SET_l8_r8(4, Reg8::A),
            0xe8 => Instr::SET_l8_r8(5, Reg8::B),
            0xe9 => Instr::SET_l8_r8(5, Reg8::C),
            0xea => Instr::SET_l8_r8(5, Reg8::D),
            0xeb => Instr::SET_l8_r8(5, Reg8::E),
            0xec => Instr::SET_l8_r8(5, Reg8::H),
            0xed => Instr::SET_l8_r8(5, Reg8::L),
            0xee => Instr::SET_l8_ir16(5, Reg16::HL),
            0xef => Instr::SET_l8_r8(5, Reg8::A),
            0xf0 => Instr::SET_l8_r8(6, Reg8::B),
            0xf1 => Instr::SET_l8_r8(6, Reg8::C),
            0xf2 => Instr::SET_l8_r8(6, Reg8::D),
            0xf3 => Instr::SET_l8_r8(6, Reg8::E),
            0xf4 => Instr::SET_l8_r8(6, Reg8::H),
            0xf5 => Instr::SET_l8_r8(6, Reg8::L),
            0xf6 => Instr::SET_l8_ir16(6, Reg16::HL),
            0xf7 => Instr::SET_l8_r8(6, Reg8::A),
            0xf8 => Instr::SET_l8_r8(7, Reg8::B),
            0xf9 => Instr::SET_l8_r8(7, Reg8::C),
            0xfa => Instr::SET_l8_r8(7, Reg8::D),
            0xfb => Instr::SET_l8_r8(7, Reg8::E),
            0xfc => Instr::SET_l8_r8(7, Reg8::H),
            0xfd => Instr::SET_l8_r8(7, Reg8::L),
            0xfe => Instr::SET_l8_ir16(7, Reg16::HL),
            0xff => Instr::SET_l8_r8(7, Reg8::A),
            i => Instr::INVALID(real),
        };
        Ok((real, i))
    }

    fn disasm<R: Read>(bytes : &mut R) -> io::Result<(u16, Instr)> {
        let mut instr = [0u8; 4];
        bytes.read_exact(&mut instr[..1])?;
        let size = get_op(instr[0] as u16).size as usize;
        bytes.read_exact(&mut instr[1..size])?;
        let mut opcode : u16 = instr[0] as u16;
        let i = match instr[0] {
            0x00 => Instr::NOP,
            0x01 => Instr::LD_r16_d16(Reg16::BC, read_u16(&mut instr[1..3].as_ref())?),
            0x02 => Instr::LD_ir16_r8(Reg16::BC, Reg8::A),
            0x03 => Instr::INC_r16(Reg16::BC),
            0x04 => Instr::INC_r8(Reg8::B),
            0x05 => Instr::DEC_r8(Reg8::B),
            0x06 => Instr::LD_r8_d8(Reg8::B, read_u8(&mut instr[1..2].as_ref())?),
            0x07 => Instr::RLCA,
            0x08 => Instr::LD_ia16_r16(read_u16(&mut instr[1..3].as_ref())?, Reg16::SP),
            0x09 => Instr::ADD_r16_r16(Reg16::HL, Reg16::BC),
            0x0a => Instr::LD_r8_ir16(Reg8::A, Reg16::BC),
            0x0b => Instr::DEC_r16(Reg16::BC),
            0x0c => Instr::INC_r8(Reg8::C),
            0x0d => Instr::DEC_r8(Reg8::C),
            0x0e => Instr::LD_r8_d8(Reg8::C, read_u8(&mut instr[1..2].as_ref())?),
            0x0f => Instr::RRCA,
            0x10 => Instr::STOP_0(0),
            0x11 => Instr::LD_r16_d16(Reg16::DE, read_u16(&mut instr[1..3].as_ref())?),
            0x12 => Instr::LD_ir16_r8(Reg16::DE, Reg8::A),
            0x13 => Instr::INC_r16(Reg16::DE),
            0x14 => Instr::INC_r8(Reg8::D),
            0x15 => Instr::DEC_r8(Reg8::D),
            0x16 => Instr::LD_r8_d8(Reg8::D, read_u8(&mut instr[1..2].as_ref())?),
            0x17 => Instr::RLA,
            0x18 => Instr::JR_r8(read_u8(&mut instr[1..2].as_ref())? as i8),
            0x19 => Instr::ADD_r16_r16(Reg16::HL, Reg16::DE),
            0x1a => Instr::LD_r8_ir16(Reg8::A, Reg16::DE),
            0x1b => Instr::DEC_r16(Reg16::DE),
            0x1c => Instr::INC_r8(Reg8::E),
            0x1d => Instr::DEC_r8(Reg8::E),
            0x1e => Instr::LD_r8_d8(Reg8::E, read_u8(&mut instr[1..2].as_ref())?),
            0x1f => Instr::RRA,
            0x20 => Instr::JR_COND_r8(Cond::NZ, read_u8(&mut instr[1..2].as_ref())? as i8),
            0x21 => Instr::LD_r16_d16(Reg16::HL, read_u16(&mut instr[1..3].as_ref())?),
            0x22 => Instr::LD_iir16_r8(Reg16::HL, Reg8::A),
            0x23 => Instr::INC_r16(Reg16::HL),
            0x24 => Instr::INC_r8(Reg8::H),
            0x25 => Instr::DEC_r8(Reg8::H),
            0x26 => Instr::LD_r8_d8(Reg8::H, read_u8(&mut instr[1..2].as_ref())?),
            0x27 => Instr::DAA,
            0x28 => Instr::JR_COND_r8(Cond::Z, read_u8(&mut instr[1..2].as_ref())? as i8),
            0x29 => Instr::ADD_r16_r16(Reg16::HL, Reg16::HL),
            0x2a => Instr::LD_r8_iir16(Reg8::A, Reg16::HL),
            0x2b => Instr::DEC_r16(Reg16::HL),
            0x2c => Instr::INC_r8(Reg8::L),
            0x2d => Instr::DEC_r8(Reg8::L),
            0x2e => Instr::LD_r8_d8(Reg8::L, read_u8(&mut instr[1..2].as_ref())?),
            0x2f => Instr::CPL,
            0x30 => Instr::JR_COND_r8(Cond::NC, read_u8(&mut instr[1..2].as_ref())? as i8),
            0x31 => Instr::LD_r16_d16(Reg16::SP, read_u16(&mut instr[1..3].as_ref())?),
            0x32 => Instr::LD_dir16_r8(Reg16::HL, Reg8::A),
            0x33 => Instr::INC_r16(Reg16::SP),
            0x34 => Instr::INC_ir16(Reg16::HL),
            0x35 => Instr::DEC_ir16(Reg16::HL),
            0x36 => Instr::LD_ir16_d8(Reg16::HL, read_u8(&mut instr[1..2].as_ref())?),
            0x37 => Instr::SCF,
            0x38 => Instr::JR_COND_r8(Cond::C, read_u8(&mut instr[1..2].as_ref())? as i8),
            0x39 => Instr::ADD_r16_r16(Reg16::HL, Reg16::SP),
            0x3a => Instr::LD_r8_dir16(Reg8::A, Reg16::HL),
            0x3b => Instr::DEC_r16(Reg16::SP),
            0x3c => Instr::INC_r8(Reg8::A),
            0x3d => Instr::DEC_r8(Reg8::A),
            0x3e => Instr::LD_r8_d8(Reg8::A, read_u8(&mut instr[1..2].as_ref())?),
            0x3f => Instr::CCF,
            0x40 => Instr::LD_r8_r8(Reg8::B, Reg8::B),
            0x41 => Instr::LD_r8_r8(Reg8::B, Reg8::C),
            0x42 => Instr::LD_r8_r8(Reg8::B, Reg8::D),
            0x43 => Instr::LD_r8_r8(Reg8::B, Reg8::E),
            0x44 => Instr::LD_r8_r8(Reg8::B, Reg8::H),
            0x45 => Instr::LD_r8_r8(Reg8::B, Reg8::L),
            0x46 => Instr::LD_r8_ir16(Reg8::B, Reg16::HL),
            0x47 => Instr::LD_r8_r8(Reg8::B, Reg8::A),
            0x48 => Instr::LD_r8_r8(Reg8::C, Reg8::B),
            0x49 => Instr::LD_r8_r8(Reg8::C, Reg8::C),
            0x4a => Instr::LD_r8_r8(Reg8::C, Reg8::D),
            0x4b => Instr::LD_r8_r8(Reg8::C, Reg8::E),
            0x4c => Instr::LD_r8_r8(Reg8::C, Reg8::H),
            0x4d => Instr::LD_r8_r8(Reg8::C, Reg8::L),
            0x4e => Instr::LD_r8_ir16(Reg8::C, Reg16::HL),
            0x4f => Instr::LD_r8_r8(Reg8::C, Reg8::A),
            0x50 => Instr::LD_r8_r8(Reg8::D, Reg8::B),
            0x51 => Instr::LD_r8_r8(Reg8::D, Reg8::C),
            0x52 => Instr::LD_r8_r8(Reg8::D, Reg8::D),
            0x53 => Instr::LD_r8_r8(Reg8::D, Reg8::E),
            0x54 => Instr::LD_r8_r8(Reg8::D, Reg8::H),
            0x55 => Instr::LD_r8_r8(Reg8::D, Reg8::L),
            0x56 => Instr::LD_r8_ir16(Reg8::D, Reg16::HL),
            0x57 => Instr::LD_r8_r8(Reg8::D, Reg8::A),
            0x58 => Instr::LD_r8_r8(Reg8::E, Reg8::B),
            0x59 => Instr::LD_r8_r8(Reg8::E, Reg8::C),
            0x5a => Instr::LD_r8_r8(Reg8::E, Reg8::D),
            0x5b => Instr::LD_r8_r8(Reg8::E, Reg8::E),
            0x5c => Instr::LD_r8_r8(Reg8::E, Reg8::H),
            0x5d => Instr::LD_r8_r8(Reg8::E, Reg8::L),
            0x5e => Instr::LD_r8_ir16(Reg8::E, Reg16::HL),
            0x5f => Instr::LD_r8_r8(Reg8::E, Reg8::A),
            0x60 => Instr::LD_r8_r8(Reg8::H, Reg8::B),
            0x61 => Instr::LD_r8_r8(Reg8::H, Reg8::C),
            0x62 => Instr::LD_r8_r8(Reg8::H, Reg8::D),
            0x63 => Instr::LD_r8_r8(Reg8::H, Reg8::E),
            0x64 => Instr::LD_r8_r8(Reg8::H, Reg8::H),
            0x65 => Instr::LD_r8_r8(Reg8::H, Reg8::L),
            0x66 => Instr::LD_r8_ir16(Reg8::H, Reg16::HL),
            0x67 => Instr::LD_r8_r8(Reg8::H, Reg8::A),
            0x68 => Instr::LD_r8_r8(Reg8::L, Reg8::B),
            0x69 => Instr::LD_r8_r8(Reg8::L, Reg8::C),
            0x6a => Instr::LD_r8_r8(Reg8::L, Reg8::D),
            0x6b => Instr::LD_r8_r8(Reg8::L, Reg8::E),
            0x6c => Instr::LD_r8_r8(Reg8::L, Reg8::H),
            0x6d => Instr::LD_r8_r8(Reg8::L, Reg8::L),
            0x6e => Instr::LD_r8_ir16(Reg8::L, Reg16::HL),
            0x6f => Instr::LD_r8_r8(Reg8::L, Reg8::A),
            0x70 => Instr::LD_ir16_r8(Reg16::HL, Reg8::B),
            0x71 => Instr::LD_ir16_r8(Reg16::HL, Reg8::C),
            0x72 => Instr::LD_ir16_r8(Reg16::HL, Reg8::D),
            0x73 => Instr::LD_ir16_r8(Reg16::HL, Reg8::E),
            0x74 => Instr::LD_ir16_r8(Reg16::HL, Reg8::H),
            0x75 => Instr::LD_ir16_r8(Reg16::HL, Reg8::L),
            0x76 => Instr::HALT,
            0x77 => Instr::LD_ir16_r8(Reg16::HL, Reg8::A),
            0x78 => Instr::LD_r8_r8(Reg8::A, Reg8::B),
            0x79 => Instr::LD_r8_r8(Reg8::A, Reg8::C),
            0x7a => Instr::LD_r8_r8(Reg8::A, Reg8::D),
            0x7b => Instr::LD_r8_r8(Reg8::A, Reg8::E),
            0x7c => Instr::LD_r8_r8(Reg8::A, Reg8::H),
            0x7d => Instr::LD_r8_r8(Reg8::A, Reg8::L),
            0x7e => Instr::LD_r8_ir16(Reg8::A, Reg16::HL),
            0x7f => Instr::LD_r8_r8(Reg8::A, Reg8::A),
            0x80 => Instr::ADD_r8_r8(Reg8::A, Reg8::B),
            0x81 => Instr::ADD_r8_r8(Reg8::A, Reg8::C),
            0x82 => Instr::ADD_r8_r8(Reg8::A, Reg8::D),
            0x83 => Instr::ADD_r8_r8(Reg8::A, Reg8::E),
            0x84 => Instr::ADD_r8_r8(Reg8::A, Reg8::H),
            0x85 => Instr::ADD_r8_r8(Reg8::A, Reg8::L),
            0x86 => Instr::ADD_r8_ir16(Reg8::A, Reg16::HL),
            0x87 => Instr::ADD_r8_r8(Reg8::A, Reg8::A),
            0x88 => Instr::ADC_r8_r8(Reg8::A, Reg8::B),
            0x89 => Instr::ADC_r8_r8(Reg8::A, Reg8::C),
            0x8a => Instr::ADC_r8_r8(Reg8::A, Reg8::D),
            0x8b => Instr::ADC_r8_r8(Reg8::A, Reg8::E),
            0x8c => Instr::ADC_r8_r8(Reg8::A, Reg8::H),
            0x8d => Instr::ADC_r8_r8(Reg8::A, Reg8::L),
            0x8e => Instr::ADC_r8_ir16(Reg8::A, Reg16::HL),
            0x8f => Instr::ADC_r8_r8(Reg8::A, Reg8::A),
            0x90 => Instr::SUB_r8(Reg8::B),
            0x91 => Instr::SUB_r8(Reg8::C),
            0x92 => Instr::SUB_r8(Reg8::D),
            0x93 => Instr::SUB_r8(Reg8::E),
            0x94 => Instr::SUB_r8(Reg8::H),
            0x95 => Instr::SUB_r8(Reg8::L),
            0x96 => Instr::SUB_ir16(Reg16::HL),
            0x97 => Instr::SUB_r8(Reg8::A),
            0x98 => Instr::SBC_r8_r8(Reg8::A, Reg8::B),
            0x99 => Instr::SBC_r8_r8(Reg8::A, Reg8::C),
            0x9a => Instr::SBC_r8_r8(Reg8::A, Reg8::D),
            0x9b => Instr::SBC_r8_r8(Reg8::A, Reg8::E),
            0x9c => Instr::SBC_r8_r8(Reg8::A, Reg8::H),
            0x9d => Instr::SBC_r8_r8(Reg8::A, Reg8::L),
            0x9e => Instr::SBC_r8_ir16(Reg8::A, Reg16::HL),
            0x9f => Instr::SBC_r8_r8(Reg8::A, Reg8::A),
            0xa0 => Instr::AND_r8(Reg8::B),
            0xa1 => Instr::AND_r8(Reg8::C),
            0xa2 => Instr::AND_r8(Reg8::D),
            0xa3 => Instr::AND_r8(Reg8::E),
            0xa4 => Instr::AND_r8(Reg8::H),
            0xa5 => Instr::AND_r8(Reg8::L),
            0xa6 => Instr::AND_ir16(Reg16::HL),
            0xa7 => Instr::AND_r8(Reg8::A),
            0xa8 => Instr::XOR_r8(Reg8::B),
            0xa9 => Instr::XOR_r8(Reg8::C),
            0xaa => Instr::XOR_r8(Reg8::D),
            0xab => Instr::XOR_r8(Reg8::E),
            0xac => Instr::XOR_r8(Reg8::H),
            0xad => Instr::XOR_r8(Reg8::L),
            0xae => Instr::XOR_ir16(Reg16::HL),
            0xaf => Instr::XOR_r8(Reg8::A),
            0xb0 => Instr::OR_r8(Reg8::B),
            0xb1 => Instr::OR_r8(Reg8::C),
            0xb2 => Instr::OR_r8(Reg8::D),
            0xb3 => Instr::OR_r8(Reg8::E),
            0xb4 => Instr::OR_r8(Reg8::H),
            0xb5 => Instr::OR_r8(Reg8::L),
            0xb6 => Instr::OR_ir16(Reg16::HL),
            0xb7 => Instr::OR_r8(Reg8::A),
            0xb8 => Instr::CP_r8(Reg8::B),
            0xb9 => Instr::CP_r8(Reg8::C),
            0xba => Instr::CP_r8(Reg8::D),
            0xbb => Instr::CP_r8(Reg8::E),
            0xbc => Instr::CP_r8(Reg8::H),
            0xbd => Instr::CP_r8(Reg8::L),
            0xbe => Instr::CP_ir16(Reg16::HL),
            0xbf => Instr::CP_r8(Reg8::A),
            0xc0 => Instr::RET_COND(Cond::NZ),
            0xc1 => Instr::POP_r16(Reg16::BC),
            0xc2 => Instr::JP_COND_a16(Cond::NZ, read_u16(&mut instr[1..3].as_ref())?),
            0xc3 => Instr::JP_a16(read_u16(&mut instr[1..3].as_ref())?),
            0xc4 => Instr::CALL_COND_a16(Cond::NZ, read_u16(&mut instr[1..3].as_ref())?),
            0xc5 => Instr::PUSH_r16(Reg16::BC),
            0xc6 => Instr::ADD_r8_d8(Reg8::A, read_u8(&mut instr[1..2].as_ref())?),
            0xc7 => Instr::RST_LIT(0x00),
            0xc8 => Instr::RET_COND(Cond::Z),
            0xc9 => Instr::RET,
            0xca => Instr::JP_COND_a16(Cond::Z, read_u16(&mut instr[1..3].as_ref())?),
            0xcb => {
                let (cb_op, cb_instr) = Instr::prefix_cb_disasm(&mut instr[1..2].as_ref())?;
                opcode = cb_op;
                cb_instr
            },
            0xcc => Instr::CALL_COND_a16(Cond::Z, read_u16(&mut instr[1..3].as_ref())?),
            0xcd => Instr::CALL_a16(read_u16(&mut instr[1..3].as_ref())?),
            0xce => Instr::ADC_r8_d8(Reg8::A, read_u8(&mut instr[1..2].as_ref())?),
            0xcf => Instr::RST_LIT(0x08),
            0xd0 => Instr::RET_COND(Cond::NC),
            0xd1 => Instr::POP_r16(Reg16::DE),
            0xd2 => Instr::JP_COND_a16(Cond::NC, read_u16(&mut instr[1..3].as_ref())?),
            //0xd3 => Instr::INVALID,
            0xd4 => Instr::CALL_COND_a16(Cond::NC, read_u16(&mut instr[1..3].as_ref())?),
            0xd5 => Instr::PUSH_r16(Reg16::DE),
            0xd6 => Instr::SUB_d8(read_u8(&mut instr[1..2].as_ref())?),
            0xd7 => Instr::RST_LIT(0x10),
            0xd8 => Instr::RET_COND(Cond::C),
            0xd9 => Instr::RETI,
            0xda => Instr::JP_COND_a16(Cond::C, read_u16(&mut instr[1..3].as_ref())?),
            //0xdb => Instr::INVALID,
            0xdc => Instr::CALL_COND_a16(Cond::C, read_u16(&mut instr[1..3].as_ref())?),
            //0xdd => Instr::INVALID,
            0xde => Instr::SBC_r8_d8(Reg8::A, read_u8(&mut instr[1..2].as_ref())?),
            0xdf => Instr::RST_LIT(0x18),
            0xe0 => Instr::LDH_ia8_r8(read_u8(&mut instr[1..2].as_ref())?, Reg8::A),
            0xe1 => Instr::POP_r16(Reg16::HL),
            0xe2 => Instr::LD_ir8_r8(Reg8::C, Reg8::A),
            //0xe3 => Instr::INVALID,
            //0xe4 => Instr::INVALID,
            0xe5 => Instr::PUSH_r16(Reg16::HL),
            0xe6 => Instr::AND_d8(read_u8(&mut instr[1..2].as_ref())?),
            0xe7 => Instr::RST_LIT(0x20),
            0xe8 => Instr::ADD_r16_r8(Reg16::SP, read_u8(&mut instr[1..2].as_ref())? as i8),
            0xe9 => Instr::JP_ir16(Reg16::HL),
            0xea => Instr::LD_ia16_r8(read_u16(&mut instr[1..3].as_ref())?, Reg8::A),
            // 0xeb => Instr::INVALID,
            // 0xec => Instr::INVALID,
            // 0xed => Instr::INVALID,
            0xee => Instr::XOR_d8(read_u8(&mut instr[1..2].as_ref())?),
            0xef => Instr::RST_LIT(0x28),
            0xf0 => Instr::LDH_r8_ia8(Reg8::A, read_u8(&mut instr[1..2].as_ref())?),
            0xf1 => Instr::POP_r16(Reg16::AF),
            0xf2 => Instr::LD_r8_ir8(Reg8::A, Reg8::C),
            0xf3 => Instr::DI,
            //0xf4 => Instr::INVALID,
            0xf5 => Instr::PUSH_r16(Reg16::AF),
            0xf6 => Instr::OR_d8(read_u8(&mut instr[1..2].as_ref())?),
            0xf7 => Instr::RST_LIT(0x30),
            0xf8 => Instr::LD_r16_r16_r8(Reg16::HL, Reg16::SP, read_u8(&mut instr[1..2].as_ref())? as i8),
            0xf9 => Instr::LD_r16_r16(Reg16::SP, Reg16::HL),
            0xfa => Instr::LD_r8_ia16(Reg8::A, read_u16(&mut instr[1..3].as_ref())?),
            0xfb => Instr::EI,
            //0xfc => Instr::INVALID,
            //0xfd => Instr::INVALID,
            0xfe => Instr::CP_d8(read_u8(&mut instr[1..2].as_ref())?),
            0xff => Instr::RST_LIT(0x38),
            i => {
                Instr::INVALID(i as u16)
            }
        };
        Ok((opcode as u16, i))
    }
}


impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Instr::ADC_r8_d8(x0, x1) => write!(f, "ADC {:?},{:?}", x0, x1),
            Instr::ADC_r8_ir16(x0, x1) => write!(f, "ADC {:?},({:?})", x0, x1),
            Instr::ADC_r8_r8(x0, x1) => write!(f, "ADC {:?},{:?}", x0, x1),
            Instr::ADD_r16_r16(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::ADD_r16_r8(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::ADD_r8_d8(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::ADD_r8_ir16(x0, x1) => write!(f, "ADD {:?},({:?})", x0, x1),
            Instr::ADD_r8_r8(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::AND_d8(x0) => write!(f, "AND {:?}", x0),
            Instr::AND_ir16(x0) => write!(f, "AND ({:?})", x0),
            Instr::AND_r8(x0) => write!(f, "AND {:?}", x0),
            Instr::BIT_l8_ir16(x0, x1) => write!(f, "BIT {:?},({:?})", x0, x1),
            Instr::BIT_l8_r8(x0, x1) => write!(f, "BIT {:?},{:?}", x0, x1),
            Instr::CALL_COND_a16(x0, x1) => write!(f, "CALL {:?},{:?}", x0, x1),
            Instr::CALL_a16(x0) => write!(f, "CALL {:?}", x0),
            Instr::CCF => write!(f, "CCF"),
            Instr::CPL => write!(f, "CPL"),
            Instr::CP_d8(x0) => write!(f, "CP {:?}", x0),
            Instr::CP_ir16(x0) => write!(f, "CP ({:?})", x0),
            Instr::CP_r8(x0) => write!(f, "CP {:?}", x0),
            Instr::DAA => write!(f, "DAA"),
            Instr::DEC_ir16(x0) => write!(f, "DEC ({:?})", x0),
            Instr::DEC_r16(x0) => write!(f, "DEC {:?}", x0),
            Instr::DEC_r8(x0) => write!(f, "DEC {:?}", x0),
            Instr::DI => write!(f, "DI"),
            Instr::EI => write!(f, "EI"),
            Instr::HALT => write!(f, "HALT"),
            Instr::INC_ir16(x0) => write!(f, "INC ({:?})", x0),
            Instr::INC_r16(x0) => write!(f, "INC {:?}", x0),
            Instr::INC_r8(x0) => write!(f, "INC {:?}", x0),
            Instr::INVALID(x0) => write!(f, "INVALID 0x{:x}", x0),
            Instr::JP_COND_a16(x0, x1) => write!(f, "JP {:?},{:?}", x0, x1),
            Instr::JP_a16(x0) => write!(f, "JP {:?}", x0),
            Instr::JP_ir16(x0) => write!(f, "JP ({:?})", x0),
            Instr::JR_COND_r8(x0, x1) => write!(f, "JR {:?},{:?}", x0, x1),
            Instr::JR_r8(x0) => write!(f, "JR {:?}", x0),
            Instr::LDH_ia8_r8(x0, x1) => write!(f, "LDH ({:?}),{:?}", x0, x1),
            Instr::LDH_r8_ia8(x0, x1) => write!(f, "LDH {:?},({:?})", x0, x1),
            Instr::LD_ia16_r16(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::LD_ia16_r8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::LD_ir16_d8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::LD_ir16_r8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::LD_iir16_r8(x0, x1) => write!(f, "LD ({:?}+),{:?}", x0, x1),
            Instr::LD_dir16_r8(x0, x1) => write!(f, "LD ({:?}-),{:?}", x0, x1),
            Instr::LD_ir8_r8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::LD_r16_d16(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::LD_r16_r16(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::LD_r16_r16_r8(x0, x1, x2) => write!(f, "LD {:?},{:?},{:?}", x0, x1, x2),
            Instr::LD_r8_d8(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::LD_r8_ia16(x0, x1) => write!(f, "LD {:?},({:?})", x0, x1),
            Instr::LD_r8_ir16(x0, x1) => write!(f, "LD {:?},({:?})", x0, x1),
            Instr::LD_r8_iir16(x0, x1) => write!(f, "LD {:?},({:?}+)", x0, x1),
            Instr::LD_r8_dir16(x0, x1) => write!(f, "LD {:?},({:?}-)", x0, x1),
            Instr::LD_r8_ir8(x0, x1) => write!(f, "LD {:?},({:?})", x0, x1),
            Instr::LD_r8_r8(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::NOP => write!(f, "NOP"),
            Instr::OR_d8(x0) => write!(f, "OR {:?}", x0),
            Instr::OR_ir16(x0) => write!(f, "OR ({:?})", x0),
            Instr::OR_r8(x0) => write!(f, "OR {:?}", x0),
            Instr::POP_r16(x0) => write!(f, "POP {:?}", x0),
            Instr::PUSH_r16(x0) => write!(f, "PUSH {:?}", x0),
            Instr::RES_l8_ir16(x0, x1) => write!(f, "RES {:?},({:?})", x0, x1),
            Instr::RES_l8_r8(x0, x1) => write!(f, "RES {:?},{:?}", x0, x1),
            Instr::RET => write!(f, "RET"),
            Instr::RETI => write!(f, "RETI"),
            Instr::RET_COND(x0) => write!(f, "RET {:?}", x0),
            Instr::RLA => write!(f, "RLA"),
            Instr::RLCA => write!(f, "RLCA"),
            Instr::RLC_ir16(x0) => write!(f, "RLC ({:?})", x0),
            Instr::RLC_r8(x0) => write!(f, "RLC {:?}", x0),
            Instr::RL_ir16(x0) => write!(f, "RL ({:?})", x0),
            Instr::RL_r8(x0) => write!(f, "RL {:?}", x0),
            Instr::RRA => write!(f, "RRA"),
            Instr::RRCA => write!(f, "RRCA"),
            Instr::RRC_ir16(x0) => write!(f, "RRC ({:?})", x0),
            Instr::RRC_r8(x0) => write!(f, "RRC {:?}", x0),
            Instr::RR_ir16(x0) => write!(f, "RR ({:?})", x0),
            Instr::RR_r8(x0) => write!(f, "RR {:?}", x0),
            Instr::RST_LIT(x0) => write!(f, "RST {:?}", x0),
            Instr::SBC_r8_d8(x0, x1) => write!(f, "SBC {:?},{:?}", x0, x1),
            Instr::SBC_r8_ir16(x0, x1) => write!(f, "SBC {:?},({:?})", x0, x1),
            Instr::SBC_r8_r8(x0, x1) => write!(f, "SBC {:?},{:?}", x0, x1),
            Instr::SCF => write!(f, "SCF"),
            Instr::SET_l8_ir16(x0, x1) => write!(f, "SET {:?},({:?})", x0, x1),
            Instr::SET_l8_r8(x0, x1) => write!(f, "SET {:?},{:?}", x0, x1),
            Instr::SLA_ir16(x0) => write!(f, "SLA ({:?})", x0),
            Instr::SLA_r8(x0) => write!(f, "SLA {:?}", x0),
            Instr::SRA_ir16(x0) => write!(f, "SRA ({:?})", x0),
            Instr::SRA_r8(x0) => write!(f, "SRA {:?}", x0),
            Instr::SRL_ir16(x0) => write!(f, "SRL ({:?})", x0),
            Instr::SRL_r8(x0) => write!(f, "SRL {:?}", x0),
            Instr::STOP_0(x0) => write!(f, "STOP {:?}", x0),
            Instr::SUB_d8(x0) => write!(f, "SUB {:?}", x0),
            Instr::SUB_ir16(x0) => write!(f, "SUB ({:?})", x0),
            Instr::SUB_r8(x0) => write!(f, "SUB {:?}", x0),
            Instr::SWAP_ir16(x0) => write!(f, "SWAP ({:?})", x0),
            Instr::SWAP_r8(x0) => write!(f, "SWAP {:?}", x0),
            Instr::XOR_d8(x0) => write!(f, "XOR {:?}", x0),
            Instr::XOR_ir16(x0) => write!(f, "XOR ({:?})", x0),
            Instr::XOR_r8(x0) => write!(f, "XOR {:?}", x0),
        }
    }
}

fn disasm<R: Read, W: Write, F: Fn(&Instr) -> bool> (mut start :u16, bytes : &mut R, buf : &mut W, filter: &F) -> io::Result<()> {
    let mut local = [0u8; 3];
    loop {
        if bytes.read_exact(&mut local[..1]).is_err() {
            break;
        }
        let opcode = local[0] as u16;
        let mut size = get_op(opcode).size as usize;
        let mut bytes = bytes.take((size - 1) as u64);
        let op : io::Result<Instr> = bytes.read(&mut local[1..size])
            .and_then(|bytes_read : usize|
                      match Instr::disasm(&mut local[..bytes_read + 1].as_ref()) {
                          Err(_) => {
                              size = bytes_read + 1;
                              Ok(Instr::INVALID(opcode))
                          },
                          Ok((_, op)) => Ok(op),
                      }
            ).or_else(|_| {
                size = 1;
                Ok(Instr::INVALID(opcode))
            }
            );

        let bytes = bytes.into_inner();
        let op = op?;

        if filter(&op) {
            write!(buf, "0x{:04x}: ", start);
            for x in local[0..size].iter() {
                write!(buf, "{:02x} ", x)?;
            }
            for i in size..3 {
                write!(buf, "   ");
            }
            writeln!(buf, "{}", op)?;
        }
        start += size as u16;
    };
    Ok(())
}

fn disasm_file(file : &str, filter_nops : bool) -> io::Result<()> {
    use std::io::Cursor;
    let mut f = File::open(file)?;
    let regions = [
        (0x0000, 8, "Restart"),
        (0x0008, 8, "Restart"),
        (0x0010, 8, "Restart"),
        (0x0018, 8, "Restart"),
        (0x0020, 8, "Restart"),
        (0x0028, 8, "Restart"),
        (0x0030, 8, "Restart"),
        (0x0038, 8, "Restart"),
        (0x0040, 8, "VBlank"),
        (0x0048, 8, "LCDC"),
        (0x0050, 8, "Timer Overflow"),
        (0x0058, 8, "Serial Transfer"),
        (0x0060, (0x100 - 0x60), "P10-P13"),
        (0x0100, 4, "Start"),
        (0x0104, (0x134 - 0x104), "GameBoy Logo"),
        (0x0134, (0x143 - 0x134), "Title"),
        (0x0143, (0x150 - 0x143), "Other Data"),
        (0x0150, (0xffff - 0x0150), "The Rest"),
    ];
    let mut dst = std::io::stdout();
    let mut filter =
        move |i : &Instr| match i { Instr::NOP => !filter_nops, _ => true };

    for r in regions.iter() {
        let mut taken = f.take(r.1);
        let mut buf = Cursor::new(taken);
        writeln!(dst, "{}:", r.2)?;
        disasm(r.0, buf.get_mut(), &mut dst, &mut filter);
        f = buf.into_inner().into_inner();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let mut s = Vec::new();
        assert_eq!(::Instr::disasm(&mut [0u8].as_ref()).unwrap(), (0, ::Instr::NOP));
        let mut b = ::std::io::Cursor::new(s);
        ::disasm(0, &mut [0u8, 0u8].as_ref(), &mut b, &|_| true).unwrap();
        assert_eq!(String::from_utf8(b.into_inner()).unwrap(), "0x0000: 00       NOP\n0x0001: 00       NOP\n");
        //::disasm_file("cpu_instrs/cpu_instrs.gb", true);

        let mut mem = ::GBMemory::new();
        mem.dump();
    }
}


fn main() {
    let mut mem = GBMemory::new();
    let mut cpu = CPU::new();

    loop {
        cpu.execute(&mut mem);
        cpu.dump();
    }
}
