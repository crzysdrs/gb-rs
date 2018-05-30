#![feature(match_default_bindings)]
use std::io::{Seek, Read, Write, BufWriter};
use std::io;
use std::fs::File;
use std::fmt;
use std::mem;
use std::marker::Sized;

#[derive(Debug,PartialEq)]
enum Reg8 {
    A, F,
    B, C,
    D, E,
    H, L,
}

#[derive(Debug,PartialEq)]
enum Reg16 {
    AF,
    BC,
    DE,
    HL, HLS, HLP,
    SP,
    PC
}

#[derive(Debug,PartialEq)]
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

#[derive(Debug,PartialEq)]
enum Flag {
    Z, N, H, C
}

#[derive(Debug,PartialEq)]
enum Cond {
    Z, NZ,
    C, NC,
}
#[derive(Debug,PartialEq)]
enum Term {
    Register,
    Immed,
}

#[derive(Debug,PartialEq)]
enum Operand {
    Op(Term),
    OpIndirect(Term),
}

#[derive(Debug,PartialEq)]
enum Instr {
    NOP,
    LD_r16_d16(Reg16, u16),
    LD_ir16_r8(Reg16, Reg8),
    INC_r16(Reg16),
    INC_r8(Reg8),
    DEC_r8(Reg8),
    LD_r8_d8(Reg8, u8),
    RLCA,
    LD_ia16_r16(u16, Reg16),
    ADD_r16_r16(Reg16, Reg16),
    LD_r8_ir16(Reg8, Reg16),
    DEC_r16(Reg16),
    RRCA,
    STOP_0(u8),
    RLA,
    JR_r8(i8),
    RRA,
    JR_COND_r8(Cond, i8),
    DAA,
    CPL,
    INC_ir16(Reg16),
    DEC_ir16(Reg16),
    LD_ir16_d8(Reg16, u8),
    SCF,
    CCF,
    LD_r8_r8(Reg8, Reg8),
    HALT,
    ADD_r8_r8(Reg8, Reg8),
    ADD_r8_ir16(Reg8, Reg16),
    ADC_r8_r8(Reg8, Reg8),
    ADC_r8_ir16(Reg8, Reg16),
    SUB_r8(Reg8),
    SUB_ir16(Reg16),
    SBC_r8_r8(Reg8, Reg8),
    SBC_r8_ir16(Reg8, Reg16),
    AND_r8(Reg8),
    AND_ir16(Reg16),
    XOR_r8(Reg8),
    XOR_ir16(Reg16),
    OR_r8(Reg8),
    OR_ir16(Reg16),
    CP_r8(Reg8),
    CP_ir16(Reg16),
    RET_COND(Cond),
    POP_r16(Reg16),
    JP_COND_a16(Cond, u16),
    JP_a16(u16),
    CALL_COND_a16(Cond, u16),
    PUSH_r16(Reg16),
    ADD_r8_d8(Reg8, u8),
    RST_LIT(u8),
    RET,
    PREFIX_CB(Reg8),
    CALL_a16(u16),
    ADC_r8_d8(Reg8, u8),
    INVALID,
    SUB_d8(u8),
    RETI,
    SBC_r8_d8(Reg8, u8),
    LDH_ia8_r8(u8, Reg8),
    LD_ir8_r8(Reg8, Reg8),
    AND_d8(u8),
    ADD_r16_r8(Reg16, i8),
    JP_ir16(Reg16),
    LD_ia16_r8(u16, Reg8),
    XOR_d8(u8),
    LDH_r8_ia8(Reg8, u8),
    LD_r8_ir8(Reg8, Reg8),
    DI,
    OR_d8(u8),
    LD_r16_r16_r8(Reg16, Reg16, i8),
    LD_r16_r16(Reg16, Reg16),
    LD_r8_ia16(Reg8, u16),
    EI,
    CP_d8(u8),}

struct OpCode {
    mnemonic : & 'static str,
    cycles : u8,
}

static opcodes : [OpCode; 1] = [
    OpCode { mnemonic: "NOP", cycles: 3} ];

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
    fn disasm<R: Read>(bytes : &mut R) -> io::Result<Instr> {
        let mut opcode = [0u8; 1];
        bytes.read_exact(&mut opcode)?;
        let i = match opcode[0] {
            0x00 => {
                Instr::NOP
            },
            0x01 => {
                let x0 = Reg16::BC;
                let x1 = read_u16(bytes)? as u16;
                Instr::LD_r16_d16(x0, x1)
            },
            0x02 => {
                let x0 = Reg16::BC;
                let x1 = Reg8::A;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x03 => {
                let x0 = Reg16::BC;
                Instr::INC_r16(x0)
            },
            0x04 => {
                let x0 = Reg8::B;
                Instr::INC_r8(x0)
            },
            0x05 => {
                let x0 = Reg8::B;
                Instr::DEC_r8(x0)
            },
            0x06 => {
                let x0 = Reg8::B;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_r8_d8(x0, x1)
            },
            0x07 => {
                Instr::RLCA
            },
            0x08 => {
                let x0 = read_u16(bytes)? as u16;
                let x1 = Reg16::SP;
                Instr::LD_ia16_r16(x0, x1)
            },
            0x09 => {
                let x0 = Reg16::HL;
                let x1 = Reg16::BC;
                Instr::ADD_r16_r16(x0, x1)
            },
            0x0a => {
                let x0 = Reg8::A;
                let x1 = Reg16::BC;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x0b => {
                let x0 = Reg16::BC;
                Instr::DEC_r16(x0)
            },
            0x0c => {
                let x0 = Reg8::C;
                Instr::INC_r8(x0)
            },
            0x0d => {
                let x0 = Reg8::C;
                Instr::DEC_r8(x0)
            },
            0x0e => {
                let x0 = Reg8::C;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_r8_d8(x0, x1)
            },
            0x0f => {
                Instr::RRCA
            },
            0x10 => {
                let x0 = 0;
                Instr::STOP_0(x0)
            },
            0x11 => {
                let x0 = Reg16::DE;
                let x1 = read_u16(bytes)? as u16;
                Instr::LD_r16_d16(x0, x1)
            },
            0x12 => {
                let x0 = Reg16::DE;
                let x1 = Reg8::A;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x13 => {
                let x0 = Reg16::DE;
                Instr::INC_r16(x0)
            },
            0x14 => {
                let x0 = Reg8::D;
                Instr::INC_r8(x0)
            },
            0x15 => {
                let x0 = Reg8::D;
                Instr::DEC_r8(x0)
            },
            0x16 => {
                let x0 = Reg8::D;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_r8_d8(x0, x1)
            },
            0x17 => {
                Instr::RLA
            },
            0x18 => {
                let x0 = read_u8(bytes)? as i8;
                Instr::JR_r8(x0)
            },
            0x19 => {
                let x0 = Reg16::HL;
                let x1 = Reg16::DE;
                Instr::ADD_r16_r16(x0, x1)
            },
            0x1a => {
                let x0 = Reg8::A;
                let x1 = Reg16::DE;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x1b => {
                let x0 = Reg16::DE;
                Instr::DEC_r16(x0)
            },
            0x1c => {
                let x0 = Reg8::E;
                Instr::INC_r8(x0)
            },
            0x1d => {
                let x0 = Reg8::E;
                Instr::DEC_r8(x0)
            },
            0x1e => {
                let x0 = Reg8::E;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_r8_d8(x0, x1)
            },
            0x1f => {
                Instr::RRA
            },
            0x20 => {
                let x0 = Cond::NZ;
                let x1 = read_u8(bytes)? as i8;
                Instr::JR_COND_r8(x0, x1)
            },
            0x21 => {
                let x0 = Reg16::HL;
                let x1 = read_u16(bytes)? as u16;
                Instr::LD_r16_d16(x0, x1)
            },
            0x22 => {
                let x0 = Reg16::HLP;
                let x1 = Reg8::A;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x23 => {
                let x0 = Reg16::HL;
                Instr::INC_r16(x0)
            },
            0x24 => {
                let x0 = Reg8::H;
                Instr::INC_r8(x0)
            },
            0x25 => {
                let x0 = Reg8::H;
                Instr::DEC_r8(x0)
            },
            0x26 => {
                let x0 = Reg8::H;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_r8_d8(x0, x1)
            },
            0x27 => {
                Instr::DAA
            },
            0x28 => {
                let x0 = Cond::Z;
                let x1 = read_u8(bytes)? as i8;
                Instr::JR_COND_r8(x0, x1)
            },
            0x29 => {
                let x0 = Reg16::HL;
                let x1 = Reg16::HL;
                Instr::ADD_r16_r16(x0, x1)
            },
            0x2a => {
                let x0 = Reg8::A;
                let x1 = Reg16::HLP;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x2b => {
                let x0 = Reg16::HL;
                Instr::DEC_r16(x0)
            },
            0x2c => {
                let x0 = Reg8::L;
                Instr::INC_r8(x0)
            },
            0x2d => {
                let x0 = Reg8::L;
                Instr::DEC_r8(x0)
            },
            0x2e => {
                let x0 = Reg8::L;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_r8_d8(x0, x1)
            },
            0x2f => {
                Instr::CPL
            },
            0x30 => {
                let x0 = Cond::NC;
                let x1 = read_u8(bytes)? as i8;
                Instr::JR_COND_r8(x0, x1)
            },
            0x31 => {
                let x0 = Reg16::SP;
                let x1 = read_u16(bytes)? as u16;
                Instr::LD_r16_d16(x0, x1)
            },
            0x32 => {
                let x0 = Reg16::HLS;
                let x1 = Reg8::A;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x33 => {
                let x0 = Reg16::SP;
                Instr::INC_r16(x0)
            },
            0x34 => {
                let x0 = Reg16::HL;
                Instr::INC_ir16(x0)
            },
            0x35 => {
                let x0 = Reg16::HL;
                Instr::DEC_ir16(x0)
            },
            0x36 => {
                let x0 = Reg16::HL;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_ir16_d8(x0, x1)
            },
            0x37 => {
                Instr::SCF
            },
            0x38 => {
                let x0 = Cond::C;
                let x1 = read_u8(bytes)? as i8;
                Instr::JR_COND_r8(x0, x1)
            },
            0x39 => {
                let x0 = Reg16::HL;
                let x1 = Reg16::SP;
                Instr::ADD_r16_r16(x0, x1)
            },
            0x3a => {
                let x0 = Reg8::A;
                let x1 = Reg16::HLS;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x3b => {
                let x0 = Reg16::SP;
                Instr::DEC_r16(x0)
            },
            0x3c => {
                let x0 = Reg8::A;
                Instr::INC_r8(x0)
            },
            0x3d => {
                let x0 = Reg8::A;
                Instr::DEC_r8(x0)
            },
            0x3e => {
                let x0 = Reg8::A;
                let x1 = read_u8(bytes)? as u8;
                Instr::LD_r8_d8(x0, x1)
            },
            0x3f => {
                Instr::CCF
            },
            0x40 => {
                let x0 = Reg8::B;
                let x1 = Reg8::B;
                Instr::LD_r8_r8(x0, x1)
            },
            0x41 => {
                let x0 = Reg8::B;
                let x1 = Reg8::C;
                Instr::LD_r8_r8(x0, x1)
            },
            0x42 => {
                let x0 = Reg8::B;
                let x1 = Reg8::D;
                Instr::LD_r8_r8(x0, x1)
            },
            0x43 => {
                let x0 = Reg8::B;
                let x1 = Reg8::E;
                Instr::LD_r8_r8(x0, x1)
            },
            0x44 => {
                let x0 = Reg8::B;
                let x1 = Reg8::H;
                Instr::LD_r8_r8(x0, x1)
            },
            0x45 => {
                let x0 = Reg8::B;
                let x1 = Reg8::L;
                Instr::LD_r8_r8(x0, x1)
            },
            0x46 => {
                let x0 = Reg8::B;
                let x1 = Reg16::HL;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x47 => {
                let x0 = Reg8::B;
                let x1 = Reg8::A;
                Instr::LD_r8_r8(x0, x1)
            },
            0x48 => {
                let x0 = Reg8::C;
                let x1 = Reg8::B;
                Instr::LD_r8_r8(x0, x1)
            },
            0x49 => {
                let x0 = Reg8::C;
                let x1 = Reg8::C;
                Instr::LD_r8_r8(x0, x1)
            },
            0x4a => {
                let x0 = Reg8::C;
                let x1 = Reg8::D;
                Instr::LD_r8_r8(x0, x1)
            },
            0x4b => {
                let x0 = Reg8::C;
                let x1 = Reg8::E;
                Instr::LD_r8_r8(x0, x1)
            },
            0x4c => {
                let x0 = Reg8::C;
                let x1 = Reg8::H;
                Instr::LD_r8_r8(x0, x1)
            },
            0x4d => {
                let x0 = Reg8::C;
                let x1 = Reg8::L;
                Instr::LD_r8_r8(x0, x1)
            },
            0x4e => {
                let x0 = Reg8::C;
                let x1 = Reg16::HL;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x4f => {
                let x0 = Reg8::C;
                let x1 = Reg8::A;
                Instr::LD_r8_r8(x0, x1)
            },
            0x50 => {
                let x0 = Reg8::D;
                let x1 = Reg8::B;
                Instr::LD_r8_r8(x0, x1)
            },
            0x51 => {
                let x0 = Reg8::D;
                let x1 = Reg8::C;
                Instr::LD_r8_r8(x0, x1)
            },
            0x52 => {
                let x0 = Reg8::D;
                let x1 = Reg8::D;
                Instr::LD_r8_r8(x0, x1)
            },
            0x53 => {
                let x0 = Reg8::D;
                let x1 = Reg8::E;
                Instr::LD_r8_r8(x0, x1)
            },
            0x54 => {
                let x0 = Reg8::D;
                let x1 = Reg8::H;
                Instr::LD_r8_r8(x0, x1)
            },
            0x55 => {
                let x0 = Reg8::D;
                let x1 = Reg8::L;
                Instr::LD_r8_r8(x0, x1)
            },
            0x56 => {
                let x0 = Reg8::D;
                let x1 = Reg16::HL;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x57 => {
                let x0 = Reg8::D;
                let x1 = Reg8::A;
                Instr::LD_r8_r8(x0, x1)
            },
            0x58 => {
                let x0 = Reg8::E;
                let x1 = Reg8::B;
                Instr::LD_r8_r8(x0, x1)
            },
            0x59 => {
                let x0 = Reg8::E;
                let x1 = Reg8::C;
                Instr::LD_r8_r8(x0, x1)
            },
            0x5a => {
                let x0 = Reg8::E;
                let x1 = Reg8::D;
                Instr::LD_r8_r8(x0, x1)
            },
            0x5b => {
                let x0 = Reg8::E;
                let x1 = Reg8::E;
                Instr::LD_r8_r8(x0, x1)
            },
            0x5c => {
                let x0 = Reg8::E;
                let x1 = Reg8::H;
                Instr::LD_r8_r8(x0, x1)
            },
            0x5d => {
                let x0 = Reg8::E;
                let x1 = Reg8::L;
                Instr::LD_r8_r8(x0, x1)
            },
            0x5e => {
                let x0 = Reg8::E;
                let x1 = Reg16::HL;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x5f => {
                let x0 = Reg8::E;
                let x1 = Reg8::A;
                Instr::LD_r8_r8(x0, x1)
            },
            0x60 => {
                let x0 = Reg8::H;
                let x1 = Reg8::B;
                Instr::LD_r8_r8(x0, x1)
            },
            0x61 => {
                let x0 = Reg8::H;
                let x1 = Reg8::C;
                Instr::LD_r8_r8(x0, x1)
            },
            0x62 => {
                let x0 = Reg8::H;
                let x1 = Reg8::D;
                Instr::LD_r8_r8(x0, x1)
            },
            0x63 => {
                let x0 = Reg8::H;
                let x1 = Reg8::E;
                Instr::LD_r8_r8(x0, x1)
            },
            0x64 => {
                let x0 = Reg8::H;
                let x1 = Reg8::H;
                Instr::LD_r8_r8(x0, x1)
            },
            0x65 => {
                let x0 = Reg8::H;
                let x1 = Reg8::L;
                Instr::LD_r8_r8(x0, x1)
            },
            0x66 => {
                let x0 = Reg8::H;
                let x1 = Reg16::HL;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x67 => {
                let x0 = Reg8::H;
                let x1 = Reg8::A;
                Instr::LD_r8_r8(x0, x1)
            },
            0x68 => {
                let x0 = Reg8::L;
                let x1 = Reg8::B;
                Instr::LD_r8_r8(x0, x1)
            },
            0x69 => {
                let x0 = Reg8::L;
                let x1 = Reg8::C;
                Instr::LD_r8_r8(x0, x1)
            },
            0x6a => {
                let x0 = Reg8::L;
                let x1 = Reg8::D;
                Instr::LD_r8_r8(x0, x1)
            },
            0x6b => {
                let x0 = Reg8::L;
                let x1 = Reg8::E;
                Instr::LD_r8_r8(x0, x1)
            },
            0x6c => {
                let x0 = Reg8::L;
                let x1 = Reg8::H;
                Instr::LD_r8_r8(x0, x1)
            },
            0x6d => {
                let x0 = Reg8::L;
                let x1 = Reg8::L;
                Instr::LD_r8_r8(x0, x1)
            },
            0x6e => {
                let x0 = Reg8::L;
                let x1 = Reg16::HL;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x6f => {
                let x0 = Reg8::L;
                let x1 = Reg8::A;
                Instr::LD_r8_r8(x0, x1)
            },
            0x70 => {
                let x0 = Reg16::HL;
                let x1 = Reg8::B;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x71 => {
                let x0 = Reg16::HL;
                let x1 = Reg8::C;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x72 => {
                let x0 = Reg16::HL;
                let x1 = Reg8::D;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x73 => {
                let x0 = Reg16::HL;
                let x1 = Reg8::E;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x74 => {
                let x0 = Reg16::HL;
                let x1 = Reg8::H;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x75 => {
                let x0 = Reg16::HL;
                let x1 = Reg8::L;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x76 => {
                Instr::HALT
            },
            0x77 => {
                let x0 = Reg16::HL;
                let x1 = Reg8::A;
                Instr::LD_ir16_r8(x0, x1)
            },
            0x78 => {
                let x0 = Reg8::A;
                let x1 = Reg8::B;
                Instr::LD_r8_r8(x0, x1)
            },
            0x79 => {
                let x0 = Reg8::A;
                let x1 = Reg8::C;
                Instr::LD_r8_r8(x0, x1)
            },
            0x7a => {
                let x0 = Reg8::A;
                let x1 = Reg8::D;
                Instr::LD_r8_r8(x0, x1)
            },
            0x7b => {
                let x0 = Reg8::A;
                let x1 = Reg8::E;
                Instr::LD_r8_r8(x0, x1)
            },
            0x7c => {
                let x0 = Reg8::A;
                let x1 = Reg8::H;
                Instr::LD_r8_r8(x0, x1)
            },
            0x7d => {
                let x0 = Reg8::A;
                let x1 = Reg8::L;
                Instr::LD_r8_r8(x0, x1)
            },
            0x7e => {
                let x0 = Reg8::A;
                let x1 = Reg16::HL;
                Instr::LD_r8_ir16(x0, x1)
            },
            0x7f => {
                let x0 = Reg8::A;
                let x1 = Reg8::A;
                Instr::LD_r8_r8(x0, x1)
            },
            0x80 => {
                let x0 = Reg8::A;
                let x1 = Reg8::B;
                Instr::ADD_r8_r8(x0, x1)
            },
            0x81 => {
                let x0 = Reg8::A;
                let x1 = Reg8::C;
                Instr::ADD_r8_r8(x0, x1)
            },
            0x82 => {
                let x0 = Reg8::A;
                let x1 = Reg8::D;
                Instr::ADD_r8_r8(x0, x1)
            },
            0x83 => {
                let x0 = Reg8::A;
                let x1 = Reg8::E;
                Instr::ADD_r8_r8(x0, x1)
            },
            0x84 => {
                let x0 = Reg8::A;
                let x1 = Reg8::H;
                Instr::ADD_r8_r8(x0, x1)
            },
            0x85 => {
                let x0 = Reg8::A;
                let x1 = Reg8::L;
                Instr::ADD_r8_r8(x0, x1)
            },
            0x86 => {
                let x0 = Reg8::A;
                let x1 = Reg16::HL;
                Instr::ADD_r8_ir16(x0, x1)
            },
            0x87 => {
                let x0 = Reg8::A;
                let x1 = Reg8::A;
                Instr::ADD_r8_r8(x0, x1)
            },
            0x88 => {
                let x0 = Reg8::A;
                let x1 = Reg8::B;
                Instr::ADC_r8_r8(x0, x1)
            },
            0x89 => {
                let x0 = Reg8::A;
                let x1 = Reg8::C;
                Instr::ADC_r8_r8(x0, x1)
            },
            0x8a => {
                let x0 = Reg8::A;
                let x1 = Reg8::D;
                Instr::ADC_r8_r8(x0, x1)
            },
            0x8b => {
                let x0 = Reg8::A;
                let x1 = Reg8::E;
                Instr::ADC_r8_r8(x0, x1)
            },
            0x8c => {
                let x0 = Reg8::A;
                let x1 = Reg8::H;
                Instr::ADC_r8_r8(x0, x1)
            },
            0x8d => {
                let x0 = Reg8::A;
                let x1 = Reg8::L;
                Instr::ADC_r8_r8(x0, x1)
            },
            0x8e => {
                let x0 = Reg8::A;
                let x1 = Reg16::HL;
                Instr::ADC_r8_ir16(x0, x1)
            },
            0x8f => {
                let x0 = Reg8::A;
                let x1 = Reg8::A;
                Instr::ADC_r8_r8(x0, x1)
            },
            0x90 => {
                let x0 = Reg8::B;
                Instr::SUB_r8(x0)
            },
            0x91 => {
                let x0 = Reg8::C;
                Instr::SUB_r8(x0)
            },
            0x92 => {
                let x0 = Reg8::D;
                Instr::SUB_r8(x0)
            },
            0x93 => {
                let x0 = Reg8::E;
                Instr::SUB_r8(x0)
            },
            0x94 => {
                let x0 = Reg8::H;
                Instr::SUB_r8(x0)
            },
            0x95 => {
                let x0 = Reg8::L;
                Instr::SUB_r8(x0)
            },
            0x96 => {
                let x0 = Reg16::HL;
                Instr::SUB_ir16(x0)
            },
            0x97 => {
                let x0 = Reg8::A;
                Instr::SUB_r8(x0)
            },
            0x98 => {
                let x0 = Reg8::A;
                let x1 = Reg8::B;
                Instr::SBC_r8_r8(x0, x1)
            },
            0x99 => {
                let x0 = Reg8::A;
                let x1 = Reg8::C;
                Instr::SBC_r8_r8(x0, x1)
            },
            0x9a => {
                let x0 = Reg8::A;
                let x1 = Reg8::D;
                Instr::SBC_r8_r8(x0, x1)
            },
            0x9b => {
                let x0 = Reg8::A;
                let x1 = Reg8::E;
                Instr::SBC_r8_r8(x0, x1)
            },
            0x9c => {
                let x0 = Reg8::A;
                let x1 = Reg8::H;
                Instr::SBC_r8_r8(x0, x1)
            },
            0x9d => {
                let x0 = Reg8::A;
                let x1 = Reg8::L;
                Instr::SBC_r8_r8(x0, x1)
            },
            0x9e => {
                let x0 = Reg8::A;
                let x1 = Reg16::HL;
                Instr::SBC_r8_ir16(x0, x1)
            },
            0x9f => {
                let x0 = Reg8::A;
                let x1 = Reg8::A;
                Instr::SBC_r8_r8(x0, x1)
            },
            0xa0 => {
                let x0 = Reg8::B;
                Instr::AND_r8(x0)
            },
            0xa1 => {
                let x0 = Reg8::C;
                Instr::AND_r8(x0)
            },
            0xa2 => {
                let x0 = Reg8::D;
                Instr::AND_r8(x0)
            },
            0xa3 => {
                let x0 = Reg8::E;
                Instr::AND_r8(x0)
            },
            0xa4 => {
                let x0 = Reg8::H;
                Instr::AND_r8(x0)
            },
            0xa5 => {
                let x0 = Reg8::L;
                Instr::AND_r8(x0)
            },
            0xa6 => {
                let x0 = Reg16::HL;
                Instr::AND_ir16(x0)
            },
            0xa7 => {
                let x0 = Reg8::A;
                Instr::AND_r8(x0)
            },
            0xa8 => {
                let x0 = Reg8::B;
                Instr::XOR_r8(x0)
            },
            0xa9 => {
                let x0 = Reg8::C;
                Instr::XOR_r8(x0)
            },
            0xaa => {
                let x0 = Reg8::D;
                Instr::XOR_r8(x0)
            },
            0xab => {
                let x0 = Reg8::E;
                Instr::XOR_r8(x0)
            },
            0xac => {
                let x0 = Reg8::H;
                Instr::XOR_r8(x0)
            },
            0xad => {
                let x0 = Reg8::L;
                Instr::XOR_r8(x0)
            },
            0xae => {
                let x0 = Reg16::HL;
                Instr::XOR_ir16(x0)
            },
            0xaf => {
                let x0 = Reg8::A;
                Instr::XOR_r8(x0)
            },
            0xb0 => {
                let x0 = Reg8::B;
                Instr::OR_r8(x0)
            },
            0xb1 => {
                let x0 = Reg8::C;
                Instr::OR_r8(x0)
            },
            0xb2 => {
                let x0 = Reg8::D;
                Instr::OR_r8(x0)
            },
            0xb3 => {
                let x0 = Reg8::E;
                Instr::OR_r8(x0)
            },
            0xb4 => {
                let x0 = Reg8::H;
                Instr::OR_r8(x0)
            },
            0xb5 => {
                let x0 = Reg8::L;
                Instr::OR_r8(x0)
            },
            0xb6 => {
                let x0 = Reg16::HL;
                Instr::OR_ir16(x0)
            },
            0xb7 => {
                let x0 = Reg8::A;
                Instr::OR_r8(x0)
            },
            0xb8 => {
                let x0 = Reg8::B;
                Instr::CP_r8(x0)
            },
            0xb9 => {
                let x0 = Reg8::C;
                Instr::CP_r8(x0)
            },
            0xba => {
                let x0 = Reg8::D;
                Instr::CP_r8(x0)
            },
            0xbb => {
                let x0 = Reg8::E;
                Instr::CP_r8(x0)
            },
            0xbc => {
                let x0 = Reg8::H;
                Instr::CP_r8(x0)
            },
            0xbd => {
                let x0 = Reg8::L;
                Instr::CP_r8(x0)
            },
            0xbe => {
                let x0 = Reg16::HL;
                Instr::CP_ir16(x0)
            },
            0xbf => {
                let x0 = Reg8::A;
                Instr::CP_r8(x0)
            },
            0xc0 => {
                let x0 = Cond::NZ;
                Instr::RET_COND(x0)
            },
            0xc1 => {
                let x0 = Reg16::BC;
                Instr::POP_r16(x0)
            },
            0xc2 => {
                let x0 = Cond::NZ;
                let x1 = read_u16(bytes)? as u16;
                Instr::JP_COND_a16(x0, x1)
            },
            0xc3 => {
                let x0 = read_u16(bytes)? as u16;
                Instr::JP_a16(x0)
            },
            0xc4 => {
                let x0 = Cond::NZ;
                let x1 = read_u16(bytes)? as u16;
                Instr::CALL_COND_a16(x0, x1)
            },
            0xc5 => {
                let x0 = Reg16::BC;
                Instr::PUSH_r16(x0)
            },
            0xc6 => {
                let x0 = Reg8::A;
                let x1 = read_u8(bytes)? as u8;
                Instr::ADD_r8_d8(x0, x1)
            },
            0xc7 => {
                let x0 = 0x00;
                Instr::RST_LIT(x0)
            },
            0xc8 => {
                let x0 = Cond::Z;
                Instr::RET_COND(x0)
            },
            0xc9 => {
                Instr::RET
            },
            0xca => {
                let x0 = Cond::Z;
                let x1 = read_u16(bytes)? as u16;
                Instr::JP_COND_a16(x0, x1)
            },
            0xcb => {
                /*
                let x0 = Reg8::;
                Instr::PREFIX_CB(x0)
                 */
                Instr::INVALID
            },
            0xcc => {
                let x0 = Cond::Z;
                let x1 = read_u16(bytes)? as u16;
                Instr::CALL_COND_a16(x0, x1)
            },
            0xcd => {
                let x0 = read_u16(bytes)? as u16;
                Instr::CALL_a16(x0)
            },
            0xce => {
                let x0 = Reg8::A;
                let x1 = read_u8(bytes)? as u8;
                Instr::ADC_r8_d8(x0, x1)
            },
            0xcf => {
                let x0 = 0x08;
                Instr::RST_LIT(x0)
            },
            0xd0 => {
                let x0 = Cond::NC;
                Instr::RET_COND(x0)
            },
            0xd1 => {
                let x0 = Reg16::DE;
                Instr::POP_r16(x0)
            },
            0xd2 => {
                let x0 = Cond::NC;
                let x1 = read_u16(bytes)? as u16;
                Instr::JP_COND_a16(x0, x1)
            },
            0xd3 => {
                Instr::INVALID
            },
            0xd4 => {
                let x0 = Cond::NC;
                let x1 = read_u16(bytes)? as u16;
                Instr::CALL_COND_a16(x0, x1)
            },
            0xd5 => {
                let x0 = Reg16::DE;
                Instr::PUSH_r16(x0)
            },
            0xd6 => {
                let x0 = read_u8(bytes)? as u8;
                Instr::SUB_d8(x0)
            },
            0xd7 => {
                let x0 = 0x10;
                Instr::RST_LIT(x0)
            },
            0xd8 => {
                let x0 = Cond::C;
                Instr::RET_COND(x0)
            },
            0xd9 => {
                Instr::RETI
            },
            0xda => {
                let x0 = Cond::C;
                let x1 = read_u16(bytes)? as u16;
                Instr::JP_COND_a16(x0, x1)
            },
            0xdb => {
                Instr::INVALID
            },
            0xdc => {
                let x0 = Cond::C;
                let x1 = read_u16(bytes)? as u16;
                Instr::CALL_COND_a16(x0, x1)
            },
            0xdd => {
                Instr::INVALID
            },
            0xde => {
                let x0 = Reg8::A;
                let x1 = read_u8(bytes)? as u8;
                Instr::SBC_r8_d8(x0, x1)
            },
            0xdf => {
                let x0 = 0x18;
                Instr::RST_LIT(x0)
            },
            0xe0 => {
                let x0 = read_u8(bytes)? as u8;
                let x1 = Reg8::A;
                Instr::LDH_ia8_r8(x0, x1)
            },
            0xe1 => {
                let x0 = Reg16::HL;
                Instr::POP_r16(x0)
            },
            0xe2 => {
                let x0 = Reg8::C;
                let x1 = Reg8::A;
                Instr::LD_ir8_r8(x0, x1)
            },
            0xe3 => {
                Instr::INVALID
            },
            0xe4 => {
                Instr::INVALID
            },
            0xe5 => {
                let x0 = Reg16::HL;
                Instr::PUSH_r16(x0)
            },
            0xe6 => {
                let x0 = read_u8(bytes)? as u8;
                Instr::AND_d8(x0)
            },
            0xe7 => {
                let x0 = 0x20;
                Instr::RST_LIT(x0)
            },
            0xe8 => {
                let x0 = Reg16::SP;
                let x1 = read_u8(bytes)? as i8;
                Instr::ADD_r16_r8(x0, x1)
            },
            0xe9 => {
                let x0 = Reg16::HL;
                Instr::JP_ir16(x0)
            },
            0xea => {
                let x0 = read_u16(bytes)? as u16;
                let x1 = Reg8::A;
                Instr::LD_ia16_r8(x0, x1)
            },
            0xeb => {
                Instr::INVALID
            },
            0xec => {
                Instr::INVALID
            },
            0xed => {
                Instr::INVALID
            },
            0xee => {
                let x0 = read_u8(bytes)? as u8;
                Instr::XOR_d8(x0)
            },
            0xef => {
                let x0 = 0x28;
                Instr::RST_LIT(x0)
            },
            0xf0 => {
                let x0 = Reg8::A;
                let x1 = read_u8(bytes)? as u8;
                Instr::LDH_r8_ia8(x0, x1)
            },
            0xf1 => {
                let x0 = Reg16::AF;
                Instr::POP_r16(x0)
            },
            0xf2 => {
                let x0 = Reg8::A;
                let x1 = Reg8::C;
                Instr::LD_r8_ir8(x0, x1)
            },
            0xf3 => {
                Instr::DI
            },
            0xf4 => {
                Instr::INVALID
            },
            0xf5 => {
                let x0 = Reg16::AF;
                Instr::PUSH_r16(x0)
            },
            0xf6 => {
                let x0 = read_u8(bytes)? as u8;
                Instr::OR_d8(x0)
            },
            0xf7 => {
                let x0 = 0x30;
                Instr::RST_LIT(x0)
            },
            0xf8 => {
                let x0 = Reg16::HL;
                let x1 = Reg16::SP;
                let x2 = read_u8(bytes)? as i8;
                Instr::LD_r16_r16_r8(x0, x1, x2)
            },
            0xf9 => {
                let x0 = Reg16::SP;
                let x1 = Reg16::HL;
                Instr::LD_r16_r16(x0, x1)
            },
            0xfa => {
                let x0 = Reg8::A;
                let x1 = read_u16(bytes)? as u16;
                Instr::LD_r8_ia16(x0, x1)
            },
            0xfb => {
                Instr::EI
            },
            0xfc => {
                Instr::INVALID
            },
            0xfd => {
                Instr::INVALID
            },
            0xfe => {
                let x0 = read_u8(bytes)? as u8;
                Instr::CP_d8(x0)
            },
            0xff => {
                let x0 = 0x38;
                Instr::RST_LIT(x0)
            },
            _ => {
                Instr::INVALID
            }
        };
        Ok(i)
    }
}

impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Instr::NOP => write!(f, "NOP"),
            Instr::LD_r16_d16(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::LD_ir16_r8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::INC_r16(x0) => write!(f, "INC {:?}", x0),
            Instr::INC_r8(x0) => write!(f, "INC {:?}", x0),
            Instr::DEC_r8(x0) => write!(f, "DEC {:?}", x0),
            Instr::LD_r8_d8(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::RLCA => write!(f, "RLCA"),
            Instr::LD_ia16_r16(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::ADD_r16_r16(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::LD_r8_ir16(x0, x1) => write!(f, "LD {:?},({:?})", x0, x1),
            Instr::DEC_r16(x0) => write!(f, "DEC {:?}", x0),
            Instr::RRCA => write!(f, "RRCA"),
            Instr::STOP_0(x0) => write!(f, "STOP {:?}", x0),
            Instr::RLA => write!(f, "RLA"),
            Instr::JR_r8(x0) => write!(f, "JR {:?}", x0),
            Instr::RRA => write!(f, "RRA"),
            Instr::JR_COND_r8(x0, x1) => write!(f, "JR {:?},{:?}", x0, x1),
            Instr::DAA => write!(f, "DAA"),
            Instr::CPL => write!(f, "CPL"),
            Instr::INC_ir16(x0) => write!(f, "INC ({:?})", x0),
            Instr::DEC_ir16(x0) => write!(f, "DEC ({:?})", x0),
            Instr::LD_ir16_d8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::SCF => write!(f, "SCF"),
            Instr::CCF => write!(f, "CCF"),
            Instr::LD_r8_r8(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::HALT => write!(f, "HALT"),
            Instr::ADD_r8_r8(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::ADD_r8_ir16(x0, x1) => write!(f, "ADD {:?},({:?})", x0, x1),
            Instr::ADC_r8_r8(x0, x1) => write!(f, "ADC {:?},{:?}", x0, x1),
            Instr::ADC_r8_ir16(x0, x1) => write!(f, "ADC {:?},({:?})", x0, x1),
            Instr::SUB_r8(x0) => write!(f, "SUB {:?}", x0),
            Instr::SUB_ir16(x0) => write!(f, "SUB ({:?})", x0),
            Instr::SBC_r8_r8(x0, x1) => write!(f, "SBC {:?},{:?}", x0, x1),
            Instr::SBC_r8_ir16(x0, x1) => write!(f, "SBC {:?},({:?})", x0, x1),
            Instr::AND_r8(x0) => write!(f, "AND {:?}", x0),
            Instr::AND_ir16(x0) => write!(f, "AND ({:?})", x0),
            Instr::XOR_r8(x0) => write!(f, "XOR {:?}", x0),
            Instr::XOR_ir16(x0) => write!(f, "XOR ({:?})", x0),
            Instr::OR_r8(x0) => write!(f, "OR {:?}", x0),
            Instr::OR_ir16(x0) => write!(f, "OR ({:?})", x0),
            Instr::CP_r8(x0) => write!(f, "CP {:?}", x0),
            Instr::CP_ir16(x0) => write!(f, "CP ({:?})", x0),
            Instr::RET_COND(x0) => write!(f, "RET {:?}", x0),
            Instr::POP_r16(x0) => write!(f, "POP {:?}", x0),
            Instr::JP_COND_a16(x0, x1) => write!(f, "JP {:?},{:?}", x0, x1),
            Instr::JP_a16(x0) => write!(f, "JP {:?}", x0),
            Instr::CALL_COND_a16(x0, x1) => write!(f, "CALL {:?},{:?}", x0, x1),
            Instr::PUSH_r16(x0) => write!(f, "PUSH {:?}", x0),
            Instr::ADD_r8_d8(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::RST_LIT(x0) => write!(f, "RST {:?}", x0),
            Instr::RET => write!(f, "RET"),
            Instr::PREFIX_CB(x0) => write!(f, "PREFIX {:?}", x0),
            Instr::CALL_a16(x0) => write!(f, "CALL {:?}", x0),
            Instr::ADC_r8_d8(x0, x1) => write!(f, "ADC {:?},{:?}", x0, x1),
            Instr::INVALID => write!(f, "INVALID"),
            Instr::SUB_d8(x0) => write!(f, "SUB {:?}", x0),
            Instr::RETI => write!(f, "RETI"),
            Instr::SBC_r8_d8(x0, x1) => write!(f, "SBC {:?},{:?}", x0, x1),
            Instr::LDH_ia8_r8(x0, x1) => write!(f, "LDH ({:?}),{:?}", x0, x1),
            Instr::LD_ir8_r8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::AND_d8(x0) => write!(f, "AND {:?}", x0),
            Instr::ADD_r16_r8(x0, x1) => write!(f, "ADD {:?},{:?}", x0, x1),
            Instr::JP_ir16(x0) => write!(f, "JP ({:?})", x0),
            Instr::LD_ia16_r8(x0, x1) => write!(f, "LD ({:?}),{:?}", x0, x1),
            Instr::XOR_d8(x0) => write!(f, "XOR {:?}", x0),
            Instr::LDH_r8_ia8(x0, x1) => write!(f, "LDH {:?},({:?})", x0, x1),
            Instr::LD_r8_ir8(x0, x1) => write!(f, "LD {:?},({:?})", x0, x1),
            Instr::DI => write!(f, "DI"),
            Instr::OR_d8(x0) => write!(f, "OR {:?}", x0),
            Instr::LD_r16_r16_r8(x0, x1, x2) => write!(f, "LD {:?},{:?},{:?}", x0, x1, x2),
            Instr::LD_r16_r16(x0, x1) => write!(f, "LD {:?},{:?}", x0, x1),
            Instr::LD_r8_ia16(x0, x1) => write!(f, "LD {:?},({:?})", x0, x1),
            Instr::EI => write!(f, "EI"),
            Instr::CP_d8(x0) => write!(f, "CP {:?}", x0),
            _ => write!(f, "Unimplemented!")
        }
    }
}

fn disasm<R: Read, W: Write> (bytes : &mut R, buf : &mut W) -> io::Result<()> {
    while let Ok(op) = Instr::disasm(bytes) {
        writeln!(buf, "{}", op)?;
    };
    Ok(())
}

fn disasm_file(file : &str) -> io::Result<()> {
    use std::io::Cursor;
    let f = File::open(file)?;
    let mut buf = Cursor::new(f);
    disasm(buf.get_mut(), &mut std::io::stdout())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let mut s = Vec::new();
        assert_eq!(::Instr::disasm(&mut [0u8].as_ref()).unwrap(), ::Instr::NOP);
        let mut b = ::std::io::Cursor::new(s);
        ::disasm(&mut [0u8, 0u8].as_ref(), &mut b).unwrap();
        assert_eq!(String::from_utf8(b.into_inner()).unwrap(), "NOP\nNOP\n");
        ::disasm_file("cpu_instrs/cpu_instrs.gb");
    }
}
