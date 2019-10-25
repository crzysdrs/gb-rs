use std::fmt;
use std::io::Write;

use crate::cpu::{Cond, Reg16, Reg8};
use crate::peripherals::Addressable;

#[derive(Copy, Clone, PartialEq)]
pub struct Addr(u16);
#[derive(Copy, Clone, PartialEq)]
pub struct RelAddr(i8);

impl std::ops::Add<Addr> for RelAddr {
    type Output = Addr;
    fn add(self, other: Addr) -> Addr {
        Addr(other.0.wrapping_add(self.0 as i16 as u16))
    }
}

impl From<i8> for RelAddr {
    fn from(v: i8) -> RelAddr {
        RelAddr(v)
    }
}

impl From<RelAddr> for i16 {
    fn from(addr: RelAddr) -> i16 {
        addr.0 as i16
    }
}
impl From<u16> for Addr {
    fn from(v: u16) -> Addr {
        Addr(v)
    }
}
impl From<u8> for Addr {
    fn from(v: u8) -> Addr {
        Addr(0xff00 + v as u16)
    }
}

impl From<Addr> for u16 {
    fn from(a: Addr) -> u16 {
        a.0
    }
}
#[allow(non_camel_case_types)]
#[derive(PartialEq, Clone)]
pub enum Instr {
    ADC_r8_d8(u8),
    ADC_r8_ir16(Reg16),
    ADC_r8_r8(Reg8),
    ADD_r16_r16(Reg16, Reg16),
    ADD_r16_r8(Reg16, i8),
    ADD_r8_d8(Reg8, u8),
    ADD_r8_ir16(Reg8, Reg16),
    ADD_r8_r8(Reg8, Reg8),
    AND_d8(u8),
    AND_ir16(Reg16),
    AND_r8(Reg8),
    CALL_COND_a16(Cond, Addr),
    CALL_a16(Addr),
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
    INVALID(u8),
    JP_COND_a16(Cond, Addr),
    JP_a16(Addr),
    JP_r16(Reg16),
    JR_COND_r8(Cond, RelAddr),
    JR_r8(RelAddr),
    LDH_ia8_r8(u8, Reg8),
    LDH_r8_ia8(Reg8, u8),
    LD_ia16_r16(Addr, Reg16),
    LD_ia16_r8(Addr, Reg8),
    LD_ir16_d8(Reg16, u8),
    LD_ir16_r8(Reg16, Reg8),
    LD_iir16_r8(Reg16, Reg8),
    LD_dir16_r8(Reg16, Reg8),
    LD_ir8_r8(Reg8, Reg8),
    LD_r16_d16(Reg16, u16),
    LD_r16_r16(Reg16, Reg16),
    LD_r16_r16_r8(Reg16, Reg16, i8),
    LD_r8_d8(Reg8, u8),
    LD_r8_ia16(Reg8, Addr),
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
    RET,
    RETI,
    RET_COND(Cond),
    RLA,
    RLCA,
    RRA,
    RRCA,
    RST_LIT(u8),
    SBC_r8_d8(u8),
    SBC_r8_ir16(Reg16),
    SBC_r8_r8(Reg8),
    SCF,
    STOP,
    SUB_d8(u8),
    SUB_ir16(Reg16),
    SUB_r8(Reg8),
    XOR_d8(u8),
    XOR_ir16(Reg16),
    XOR_r8(Reg8),
    BIT_l8_ir16(u8, Reg16),
    BIT_l8_r8(u8, Reg8),
    RES_l8_ir16(u8, Reg16),
    RES_l8_r8(u8, Reg8),
    RLC_ir16(Reg16),
    RLC_r8(Reg8),
    RL_ir16(Reg16),
    RL_r8(Reg8),
    RRC_ir16(Reg16),
    RRC_r8(Reg8),
    RR_ir16(Reg16),
    RR_r8(Reg8),
    SET_l8_ir16(u8, Reg16),
    SET_l8_r8(u8, Reg8),
    SLA_ir16(Reg16),
    SLA_r8(Reg8),
    SRA_ir16(Reg16),
    SRA_r8(Reg8),
    SRL_ir16(Reg16),
    SRL_r8(Reg8),
    SWAP_ir16(Reg16),
    SWAP_r8(Reg8),
}

pub struct Disasm {
    bytes: [u8; 3],
    len: usize,
}

impl Disasm {
    pub fn new() -> Disasm {
        Disasm {
            bytes: [0; 3],
            len: 0,
        }
    }
    pub fn empty(&self) -> bool {
        self.len == 0
    }
    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub fn reset(&mut self) {
        self.len = 0;
    }
    fn append_byte(&mut self, b: u8) {
        self.bytes[self.len] = b;
        self.len += 1;
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn cb_prefix(b: u8) -> Instr {
        match b {
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
        }
    }

    pub fn feed_byte(&mut self, b: u8) -> Option<Instr> {
        self.append_byte(b);
        match self.len {
            3 => {
                let val = u16::from_le_bytes([self.bytes[1], self.bytes[2]]);
                let r = match self.bytes[0] {
                    0x01 => Instr::LD_r16_d16(Reg16::BC, val),
                    0x08 => Instr::LD_ia16_r16(Addr::from(val), Reg16::SP),
                    0x11 => Instr::LD_r16_d16(Reg16::DE, val),
                    0x21 => Instr::LD_r16_d16(Reg16::HL, val),
                    0x31 => Instr::LD_r16_d16(Reg16::SP, val),
                    0xc2 => Instr::JP_COND_a16(Cond::NZ, Addr::from(val)),
                    0xc3 => Instr::JP_a16(Addr::from(val)),
                    0xc4 => Instr::CALL_COND_a16(Cond::NZ, Addr::from(val)),
                    0xca => Instr::JP_COND_a16(Cond::Z, Addr::from(val)),
                    0xcc => Instr::CALL_COND_a16(Cond::Z, Addr::from(val)),
                    0xcd => Instr::CALL_a16(Addr::from(val)),
                    0xd2 => Instr::JP_COND_a16(Cond::NC, Addr::from(val)),
                    0xd4 => Instr::CALL_COND_a16(Cond::NC, Addr::from(val)),
                    0xda => Instr::JP_COND_a16(Cond::C, Addr::from(val)),
                    0xdc => Instr::CALL_COND_a16(Cond::C, Addr::from(val)),
                    0xea => Instr::LD_ia16_r8(Addr::from(val), Reg8::A),
                    0xfa => Instr::LD_r8_ia16(Reg8::A, Addr::from(val)),
                    _ => unreachable!("Invalid 3 byte instruction"),
                };
                Some(r)
            }
            2 => match self.bytes[0] {
                0x06 => Some(Instr::LD_r8_d8(Reg8::B, b)),
                0x0e => Some(Instr::LD_r8_d8(Reg8::C, b)),
                0x16 => Some(Instr::LD_r8_d8(Reg8::D, b)),
                0x18 => Some(Instr::JR_r8(RelAddr(b as i8))),
                0x1e => Some(Instr::LD_r8_d8(Reg8::E, b)),
                0x20 => Some(Instr::JR_COND_r8(Cond::NZ, RelAddr(b as i8))),
                0x26 => Some(Instr::LD_r8_d8(Reg8::H, b)),
                0x28 => Some(Instr::JR_COND_r8(Cond::Z, RelAddr(b as i8))),
                0x2e => Some(Instr::LD_r8_d8(Reg8::L, b)),
                0x30 => Some(Instr::JR_COND_r8(Cond::NC, RelAddr(b as i8))),
                0x36 => Some(Instr::LD_ir16_d8(Reg16::HL, b)),
                0x38 => Some(Instr::JR_COND_r8(Cond::C, RelAddr(b as i8))),
                0x3e => Some(Instr::LD_r8_d8(Reg8::A, b)),
                0xc6 => Some(Instr::ADD_r8_d8(Reg8::A, b)),
                0xcb => Some(Disasm::cb_prefix(b)),
                0xce => Some(Instr::ADC_r8_d8(b)),
                0xd6 => Some(Instr::SUB_d8(b)),
                0xde => Some(Instr::SBC_r8_d8(b)),
                0xe0 => Some(Instr::LDH_ia8_r8(b, Reg8::A)),
                0xe6 => Some(Instr::AND_d8(b)),
                0xe8 => Some(Instr::ADD_r16_r8(Reg16::SP, b as i8)),
                0xee => Some(Instr::XOR_d8(b)),
                0xf0 => Some(Instr::LDH_r8_ia8(Reg8::A, b)),
                0xf6 => Some(Instr::OR_d8(b)),
                0xf8 => Some(Instr::LD_r16_r16_r8(Reg16::HL, Reg16::SP, b as i8)),
                0xfe => Some(Instr::CP_d8(b)),
                _ => None,
            },
            1 => match b {
                0x00 => Some(Instr::NOP),
                0x02 => Some(Instr::LD_ir16_r8(Reg16::BC, Reg8::A)),
                0x03 => Some(Instr::INC_r16(Reg16::BC)),
                0x04 => Some(Instr::INC_r8(Reg8::B)),
                0x05 => Some(Instr::DEC_r8(Reg8::B)),
                0x07 => Some(Instr::RLCA),
                0x09 => Some(Instr::ADD_r16_r16(Reg16::HL, Reg16::BC)),
                0x0a => Some(Instr::LD_r8_ir16(Reg8::A, Reg16::BC)),
                0x0b => Some(Instr::DEC_r16(Reg16::BC)),
                0x0c => Some(Instr::INC_r8(Reg8::C)),
                0x0d => Some(Instr::DEC_r8(Reg8::C)),
                0x0f => Some(Instr::RRCA),
                0x10 => Some(Instr::STOP),
                0x12 => Some(Instr::LD_ir16_r8(Reg16::DE, Reg8::A)),
                0x13 => Some(Instr::INC_r16(Reg16::DE)),
                0x14 => Some(Instr::INC_r8(Reg8::D)),
                0x15 => Some(Instr::DEC_r8(Reg8::D)),
                0x17 => Some(Instr::RLA),
                0x19 => Some(Instr::ADD_r16_r16(Reg16::HL, Reg16::DE)),
                0x1a => Some(Instr::LD_r8_ir16(Reg8::A, Reg16::DE)),
                0x1b => Some(Instr::DEC_r16(Reg16::DE)),
                0x1c => Some(Instr::INC_r8(Reg8::E)),
                0x1d => Some(Instr::DEC_r8(Reg8::E)),
                0x1f => Some(Instr::RRA),
                0x22 => Some(Instr::LD_iir16_r8(Reg16::HL, Reg8::A)),
                0x23 => Some(Instr::INC_r16(Reg16::HL)),
                0x24 => Some(Instr::INC_r8(Reg8::H)),
                0x25 => Some(Instr::DEC_r8(Reg8::H)),
                0x27 => Some(Instr::DAA),
                0x29 => Some(Instr::ADD_r16_r16(Reg16::HL, Reg16::HL)),
                0x2a => Some(Instr::LD_r8_iir16(Reg8::A, Reg16::HL)),
                0x2b => Some(Instr::DEC_r16(Reg16::HL)),
                0x2c => Some(Instr::INC_r8(Reg8::L)),
                0x2d => Some(Instr::DEC_r8(Reg8::L)),
                0x2f => Some(Instr::CPL),
                0x32 => Some(Instr::LD_dir16_r8(Reg16::HL, Reg8::A)),
                0x33 => Some(Instr::INC_r16(Reg16::SP)),
                0x34 => Some(Instr::INC_ir16(Reg16::HL)),
                0x35 => Some(Instr::DEC_ir16(Reg16::HL)),
                0x37 => Some(Instr::SCF),
                0x39 => Some(Instr::ADD_r16_r16(Reg16::HL, Reg16::SP)),
                0x3a => Some(Instr::LD_r8_dir16(Reg8::A, Reg16::HL)),
                0x3b => Some(Instr::DEC_r16(Reg16::SP)),
                0x3c => Some(Instr::INC_r8(Reg8::A)),
                0x3d => Some(Instr::DEC_r8(Reg8::A)),
                0x3f => Some(Instr::CCF),
                0x40 => Some(Instr::LD_r8_r8(Reg8::B, Reg8::B)),
                0x41 => Some(Instr::LD_r8_r8(Reg8::B, Reg8::C)),
                0x42 => Some(Instr::LD_r8_r8(Reg8::B, Reg8::D)),
                0x43 => Some(Instr::LD_r8_r8(Reg8::B, Reg8::E)),
                0x44 => Some(Instr::LD_r8_r8(Reg8::B, Reg8::H)),
                0x45 => Some(Instr::LD_r8_r8(Reg8::B, Reg8::L)),
                0x46 => Some(Instr::LD_r8_ir16(Reg8::B, Reg16::HL)),
                0x47 => Some(Instr::LD_r8_r8(Reg8::B, Reg8::A)),
                0x48 => Some(Instr::LD_r8_r8(Reg8::C, Reg8::B)),
                0x49 => Some(Instr::LD_r8_r8(Reg8::C, Reg8::C)),
                0x4a => Some(Instr::LD_r8_r8(Reg8::C, Reg8::D)),
                0x4b => Some(Instr::LD_r8_r8(Reg8::C, Reg8::E)),
                0x4c => Some(Instr::LD_r8_r8(Reg8::C, Reg8::H)),
                0x4d => Some(Instr::LD_r8_r8(Reg8::C, Reg8::L)),
                0x4e => Some(Instr::LD_r8_ir16(Reg8::C, Reg16::HL)),
                0x4f => Some(Instr::LD_r8_r8(Reg8::C, Reg8::A)),
                0x50 => Some(Instr::LD_r8_r8(Reg8::D, Reg8::B)),
                0x51 => Some(Instr::LD_r8_r8(Reg8::D, Reg8::C)),
                0x52 => Some(Instr::LD_r8_r8(Reg8::D, Reg8::D)),
                0x53 => Some(Instr::LD_r8_r8(Reg8::D, Reg8::E)),
                0x54 => Some(Instr::LD_r8_r8(Reg8::D, Reg8::H)),
                0x55 => Some(Instr::LD_r8_r8(Reg8::D, Reg8::L)),
                0x56 => Some(Instr::LD_r8_ir16(Reg8::D, Reg16::HL)),
                0x57 => Some(Instr::LD_r8_r8(Reg8::D, Reg8::A)),
                0x58 => Some(Instr::LD_r8_r8(Reg8::E, Reg8::B)),
                0x59 => Some(Instr::LD_r8_r8(Reg8::E, Reg8::C)),
                0x5a => Some(Instr::LD_r8_r8(Reg8::E, Reg8::D)),
                0x5b => Some(Instr::LD_r8_r8(Reg8::E, Reg8::E)),
                0x5c => Some(Instr::LD_r8_r8(Reg8::E, Reg8::H)),
                0x5d => Some(Instr::LD_r8_r8(Reg8::E, Reg8::L)),
                0x5e => Some(Instr::LD_r8_ir16(Reg8::E, Reg16::HL)),
                0x5f => Some(Instr::LD_r8_r8(Reg8::E, Reg8::A)),
                0x60 => Some(Instr::LD_r8_r8(Reg8::H, Reg8::B)),
                0x61 => Some(Instr::LD_r8_r8(Reg8::H, Reg8::C)),
                0x62 => Some(Instr::LD_r8_r8(Reg8::H, Reg8::D)),
                0x63 => Some(Instr::LD_r8_r8(Reg8::H, Reg8::E)),
                0x64 => Some(Instr::LD_r8_r8(Reg8::H, Reg8::H)),
                0x65 => Some(Instr::LD_r8_r8(Reg8::H, Reg8::L)),
                0x66 => Some(Instr::LD_r8_ir16(Reg8::H, Reg16::HL)),
                0x67 => Some(Instr::LD_r8_r8(Reg8::H, Reg8::A)),
                0x68 => Some(Instr::LD_r8_r8(Reg8::L, Reg8::B)),
                0x69 => Some(Instr::LD_r8_r8(Reg8::L, Reg8::C)),
                0x6a => Some(Instr::LD_r8_r8(Reg8::L, Reg8::D)),
                0x6b => Some(Instr::LD_r8_r8(Reg8::L, Reg8::E)),
                0x6c => Some(Instr::LD_r8_r8(Reg8::L, Reg8::H)),
                0x6d => Some(Instr::LD_r8_r8(Reg8::L, Reg8::L)),
                0x6e => Some(Instr::LD_r8_ir16(Reg8::L, Reg16::HL)),
                0x6f => Some(Instr::LD_r8_r8(Reg8::L, Reg8::A)),
                0x70 => Some(Instr::LD_ir16_r8(Reg16::HL, Reg8::B)),
                0x71 => Some(Instr::LD_ir16_r8(Reg16::HL, Reg8::C)),
                0x72 => Some(Instr::LD_ir16_r8(Reg16::HL, Reg8::D)),
                0x73 => Some(Instr::LD_ir16_r8(Reg16::HL, Reg8::E)),
                0x74 => Some(Instr::LD_ir16_r8(Reg16::HL, Reg8::H)),
                0x75 => Some(Instr::LD_ir16_r8(Reg16::HL, Reg8::L)),
                0x76 => Some(Instr::HALT),
                0x77 => Some(Instr::LD_ir16_r8(Reg16::HL, Reg8::A)),
                0x78 => Some(Instr::LD_r8_r8(Reg8::A, Reg8::B)),
                0x79 => Some(Instr::LD_r8_r8(Reg8::A, Reg8::C)),
                0x7a => Some(Instr::LD_r8_r8(Reg8::A, Reg8::D)),
                0x7b => Some(Instr::LD_r8_r8(Reg8::A, Reg8::E)),
                0x7c => Some(Instr::LD_r8_r8(Reg8::A, Reg8::H)),
                0x7d => Some(Instr::LD_r8_r8(Reg8::A, Reg8::L)),
                0x7e => Some(Instr::LD_r8_ir16(Reg8::A, Reg16::HL)),
                0x7f => Some(Instr::LD_r8_r8(Reg8::A, Reg8::A)),
                0x80 => Some(Instr::ADD_r8_r8(Reg8::A, Reg8::B)),
                0x81 => Some(Instr::ADD_r8_r8(Reg8::A, Reg8::C)),
                0x82 => Some(Instr::ADD_r8_r8(Reg8::A, Reg8::D)),
                0x83 => Some(Instr::ADD_r8_r8(Reg8::A, Reg8::E)),
                0x84 => Some(Instr::ADD_r8_r8(Reg8::A, Reg8::H)),
                0x85 => Some(Instr::ADD_r8_r8(Reg8::A, Reg8::L)),
                0x86 => Some(Instr::ADD_r8_ir16(Reg8::A, Reg16::HL)),
                0x87 => Some(Instr::ADD_r8_r8(Reg8::A, Reg8::A)),
                0x88 => Some(Instr::ADC_r8_r8(Reg8::B)),
                0x89 => Some(Instr::ADC_r8_r8(Reg8::C)),
                0x8a => Some(Instr::ADC_r8_r8(Reg8::D)),
                0x8b => Some(Instr::ADC_r8_r8(Reg8::E)),
                0x8c => Some(Instr::ADC_r8_r8(Reg8::H)),
                0x8d => Some(Instr::ADC_r8_r8(Reg8::L)),
                0x8e => Some(Instr::ADC_r8_ir16(Reg16::HL)),
                0x8f => Some(Instr::ADC_r8_r8(Reg8::A)),
                0x90 => Some(Instr::SUB_r8(Reg8::B)),
                0x91 => Some(Instr::SUB_r8(Reg8::C)),
                0x92 => Some(Instr::SUB_r8(Reg8::D)),
                0x93 => Some(Instr::SUB_r8(Reg8::E)),
                0x94 => Some(Instr::SUB_r8(Reg8::H)),
                0x95 => Some(Instr::SUB_r8(Reg8::L)),
                0x96 => Some(Instr::SUB_ir16(Reg16::HL)),
                0x97 => Some(Instr::SUB_r8(Reg8::A)),
                0x98 => Some(Instr::SBC_r8_r8(Reg8::B)),
                0x99 => Some(Instr::SBC_r8_r8(Reg8::C)),
                0x9a => Some(Instr::SBC_r8_r8(Reg8::D)),
                0x9b => Some(Instr::SBC_r8_r8(Reg8::E)),
                0x9c => Some(Instr::SBC_r8_r8(Reg8::H)),
                0x9d => Some(Instr::SBC_r8_r8(Reg8::L)),
                0x9e => Some(Instr::SBC_r8_ir16(Reg16::HL)),
                0x9f => Some(Instr::SBC_r8_r8(Reg8::A)),
                0xa0 => Some(Instr::AND_r8(Reg8::B)),
                0xa1 => Some(Instr::AND_r8(Reg8::C)),
                0xa2 => Some(Instr::AND_r8(Reg8::D)),
                0xa3 => Some(Instr::AND_r8(Reg8::E)),
                0xa4 => Some(Instr::AND_r8(Reg8::H)),
                0xa5 => Some(Instr::AND_r8(Reg8::L)),
                0xa6 => Some(Instr::AND_ir16(Reg16::HL)),
                0xa7 => Some(Instr::AND_r8(Reg8::A)),
                0xa8 => Some(Instr::XOR_r8(Reg8::B)),
                0xa9 => Some(Instr::XOR_r8(Reg8::C)),
                0xaa => Some(Instr::XOR_r8(Reg8::D)),
                0xab => Some(Instr::XOR_r8(Reg8::E)),
                0xac => Some(Instr::XOR_r8(Reg8::H)),
                0xad => Some(Instr::XOR_r8(Reg8::L)),
                0xae => Some(Instr::XOR_ir16(Reg16::HL)),
                0xaf => Some(Instr::XOR_r8(Reg8::A)),
                0xb0 => Some(Instr::OR_r8(Reg8::B)),
                0xb1 => Some(Instr::OR_r8(Reg8::C)),
                0xb2 => Some(Instr::OR_r8(Reg8::D)),
                0xb3 => Some(Instr::OR_r8(Reg8::E)),
                0xb4 => Some(Instr::OR_r8(Reg8::H)),
                0xb5 => Some(Instr::OR_r8(Reg8::L)),
                0xb6 => Some(Instr::OR_ir16(Reg16::HL)),
                0xb7 => Some(Instr::OR_r8(Reg8::A)),
                0xb8 => Some(Instr::CP_r8(Reg8::B)),
                0xb9 => Some(Instr::CP_r8(Reg8::C)),
                0xba => Some(Instr::CP_r8(Reg8::D)),
                0xbb => Some(Instr::CP_r8(Reg8::E)),
                0xbc => Some(Instr::CP_r8(Reg8::H)),
                0xbd => Some(Instr::CP_r8(Reg8::L)),
                0xbe => Some(Instr::CP_ir16(Reg16::HL)),
                0xbf => Some(Instr::CP_r8(Reg8::A)),
                0xc0 => Some(Instr::RET_COND(Cond::NZ)),
                0xc1 => Some(Instr::POP_r16(Reg16::BC)),
                0xc5 => Some(Instr::PUSH_r16(Reg16::BC)),
                0xc7 => Some(Instr::RST_LIT(0x00)),
                0xc8 => Some(Instr::RET_COND(Cond::Z)),
                0xc9 => Some(Instr::RET),
                0xcf => Some(Instr::RST_LIT(0x08)),
                0xd0 => Some(Instr::RET_COND(Cond::NC)),
                0xd1 => Some(Instr::POP_r16(Reg16::DE)),
                0xd5 => Some(Instr::PUSH_r16(Reg16::DE)),
                0xd7 => Some(Instr::RST_LIT(0x10)),
                0xd8 => Some(Instr::RET_COND(Cond::C)),
                0xd9 => Some(Instr::RETI),
                0xdf => Some(Instr::RST_LIT(0x18)),
                0xe1 => Some(Instr::POP_r16(Reg16::HL)),
                0xe2 => Some(Instr::LD_ir8_r8(Reg8::C, Reg8::A)),
                0xe5 => Some(Instr::PUSH_r16(Reg16::HL)),
                0xe7 => Some(Instr::RST_LIT(0x20)),
                0xe9 => Some(Instr::JP_r16(Reg16::HL)),
                0xef => Some(Instr::RST_LIT(0x28)),
                0xf1 => Some(Instr::POP_r16(Reg16::AF)),
                0xf2 => Some(Instr::LD_r8_ir8(Reg8::A, Reg8::C)),
                0xf3 => Some(Instr::DI),
                0xf5 => Some(Instr::PUSH_r16(Reg16::AF)),
                0xf7 => Some(Instr::RST_LIT(0x30)),
                0xf9 => Some(Instr::LD_r16_r16(Reg16::SP, Reg16::HL)),
                0xfb => Some(Instr::EI),
                0xff => Some(Instr::RST_LIT(0x38)),
                0xd3 | 0xdb | 0xdd | 0xe3 | 0xe4 | 0xeb | 0xec | 0xed | 0xf4 | 0xfc | 0xfd => {
                    Some(Instr::INVALID(b))
                }
                _ => None,
            },
            _ => panic!("Invalid Instruction Buffer"),
        }
    }
}

pub type NameAddressFn<'a> = dyn Fn(u16) -> Option<String> + 'a;

pub trait FormatCode {
    fn to_code(&self, instr: std::ops::Range<u16>, addrs: &NameAddressFn) -> String;
}

impl std::fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_code(0..0, &|_| None))
    }
}

impl FormatCode for RelAddr {
    fn to_code(&self, instr: std::ops::Range<u16>, remap: &NameAddressFn) -> String {
        use std::convert::TryFrom;
        let rel = u16::try_from(self.0.abs()).unwrap();
        let pc = instr.end.wrapping_sub(1); /* jr computes from 1 byte back */
        let addr = if self.0 > 0 {
            pc.wrapping_add(rel)
        } else {
            pc.wrapping_sub(rel)
        };
        if let Some(s) = remap(addr) {
            format!("{}", s)
        } else if self.0 >= 0 {
            format!("${:02x}", self.0)
        } else {
            format!("-${:02x}", self.0.abs())
        }
    }
}

impl FormatCode for Addr {
    fn to_code(&self, _instr: std::ops::Range<u16>, remap: &NameAddressFn) -> String {
        if let Some(s) = remap(self.0) {
            format!("{}", s)
        } else {
            format!("${:04x}", self.0)
        }
    }
}

impl FormatCode for u8 {
    fn to_code(&self, _instr: std::ops::Range<u16>, _remap: &NameAddressFn) -> String {
        format!("${:02x}", self)
    }
}
impl FormatCode for i8 {
    fn to_code(&self, _instr: std::ops::Range<u16>, _remap: &NameAddressFn) -> String {
        format!("${:02x}", self)
    }
}
impl FormatCode for u16 {
    fn to_code(&self, _instr: std::ops::Range<u16>, _remap: &NameAddressFn) -> String {
        format!("${:04x}", self)
    }
}

impl FormatCode for Reg8 {
    fn to_code(&self, _instr: std::ops::Range<u16>, _remap: &NameAddressFn) -> String {
        format!("{:?}", self)
    }
}

impl FormatCode for Cond {
    fn to_code(&self, _instr: std::ops::Range<u16>, _remap: &NameAddressFn) -> String {
        format!("{:?}", self).to_lowercase()
    }
}

impl FormatCode for Reg16 {
    fn to_code(&self, _instr: std::ops::Range<u16>, _remap: &NameAddressFn) -> String {
        format!("{:?}", self)
    }
}

impl FormatCode for Instr {
    fn to_code(&self, instr: std::ops::Range<u16>, remap: &NameAddressFn) -> String {
        let f = |v: &dyn FormatCode| v.to_code(instr.clone(), remap);

        let omit_a = |v: &Reg8| {
            if let Reg8::A = *v {
                String::from("")
            } else {
                format!("{},", f(v))
            }
        };

        match self {
            Instr::ADC_r8_d8(x1) => format!("ADC {}", f(x1)),
            Instr::ADC_r8_ir16(x1) => format!("ADC ({})", f(x1)),
            Instr::ADC_r8_r8(x1) => format!("ADC {}", f(x1)),
            Instr::ADD_r16_r16(x0, x1) => format!("ADD {},{}", f(x0), f(x1)),
            Instr::ADD_r16_r8(x0, x1) => format!("ADD {},{}", f(x0), f(x1)),
            Instr::ADD_r8_d8(x0, x1) => format!("ADD {} {}", omit_a(x0), f(x1)),
            Instr::ADD_r8_ir16(x0, x1) => format!("ADD {} ({})", omit_a(x0), f(x1)),
            Instr::ADD_r8_r8(x0, x1) => format!("ADD {} {}", omit_a(x0), f(x1)),
            Instr::AND_d8(x0) => format!("AND {}", f(x0)),
            Instr::AND_ir16(x0) => format!("AND ({})", f(x0)),
            Instr::AND_r8(x0) => format!("AND {}", f(x0)),
            Instr::CALL_COND_a16(x0, x1) => format!("CALL {},{}", f(x0), f(x1)),
            Instr::CALL_a16(x0) => format!("CALL {}", f(x0)),
            Instr::CCF => format!("CCF"),
            Instr::CPL => format!("CPL"),
            Instr::CP_d8(x0) => format!("CP {}", f(x0)),
            Instr::CP_ir16(x0) => format!("CP ({})", f(x0)),
            Instr::CP_r8(x0) => format!("CP {}", f(x0)),
            Instr::DAA => format!("DAA"),
            Instr::DEC_ir16(x0) => format!("DEC ({})", f(x0)),
            Instr::DEC_r16(x0) => format!("DEC {}", f(x0)),
            Instr::DEC_r8(x0) => format!("DEC {}", f(x0)),
            Instr::DI => format!("DI"),
            Instr::EI => format!("EI"),
            Instr::HALT => format!("HALT"),
            Instr::INC_ir16(x0) => format!("INC ({})", f(x0)),
            Instr::INC_r16(x0) => format!("INC {}", f(x0)),
            Instr::INC_r8(x0) => format!("INC {}", f(x0)),
            Instr::INVALID(x0) => format!(".db {}", f(x0)),
            Instr::JP_COND_a16(x0, x1) => format!("JP {},{}", f(x0), f(x1)),
            Instr::JP_a16(x0) => format!("JP {}", f(x0)),
            Instr::JP_r16(x0) => format!("JP {}", f(x0)),
            Instr::JR_COND_r8(x0, x1) => format!("JR {},{}", f(x0), f(x1)),
            Instr::JR_r8(x0) => format!("JR {}", f(x0)),
            Instr::LDH_ia8_r8(x0, x1) => format!("LDH ({}),{}", f(x0), f(x1)),
            Instr::LDH_r8_ia8(x0, x1) => format!("LDH {},({})", f(x0), f(x1)),
            Instr::LD_ia16_r16(x0, x1) => format!("LD ({}),{}", f(x0), f(x1)),
            Instr::LD_ia16_r8(x0, x1) => format!("LD ({}),{}", f(x0), f(x1)),
            Instr::LD_ir16_d8(x0, x1) => format!("LD ({}),{}", f(x0), f(x1)),
            Instr::LD_ir16_r8(x0, x1) => format!("LD ({}),{}", f(x0), f(x1)),
            Instr::LD_iir16_r8(x0, x1) => format!("LD ({}+),{}", f(x0), f(x1)),
            Instr::LD_dir16_r8(x0, x1) => format!("LD ({}-),{}", f(x0), f(x1)),
            Instr::LD_ir8_r8(x0, x1) => format!("LD ($FF00 + {}),{}", f(x0), f(x1)),
            Instr::LD_r16_d16(x0, x1) => format!("LD {},{}", f(x0), f(x1)),
            Instr::LD_r16_r16(x0, x1) => format!("LD {},{}", f(x0), f(x1)),
            Instr::LD_r16_r16_r8(x0, x1, x2) => format!("LD {},{},{}", f(x0), f(x1), f(x2)),
            Instr::LD_r8_d8(x0, x1) => format!("LD {},{}", f(x0), f(x1)),
            Instr::LD_r8_ia16(x0, x1) => format!("LD {},({})", f(x0), f(x1)),
            Instr::LD_r8_ir16(x0, x1) => format!("LD {},({})", f(x0), f(x1)),
            Instr::LD_r8_iir16(x0, x1) => format!("LD {},({}+)", f(x0), f(x1)),
            Instr::LD_r8_dir16(x0, x1) => format!("LD {},({}-)", f(x0), f(x1)),
            Instr::LD_r8_ir8(x0, x1) => format!("LD {},($FF00 + {})", f(x0), f(x1)),
            Instr::LD_r8_r8(x0, x1) => format!("LD {},{}", f(x0), f(x1)),
            Instr::NOP => format!("NOP"),
            Instr::OR_d8(x0) => format!("OR {}", f(x0)),
            Instr::OR_ir16(x0) => format!("OR ({})", f(x0)),
            Instr::OR_r8(x0) => format!("OR {}", f(x0)),
            Instr::POP_r16(x0) => format!("POP {}", f(x0)),
            Instr::PUSH_r16(x0) => format!("PUSH {}", f(x0)),
            Instr::RET => format!("RET"),
            Instr::RETI => format!("RETI"),
            Instr::RET_COND(x0) => format!("RET {}", f(x0)),
            Instr::RLA => format!("RLA"),
            Instr::RLCA => format!("RLCA"),
            Instr::RRA => format!("RRA"),
            Instr::RRCA => format!("RRCA"),
            Instr::RST_LIT(x0) => format!("RST {}", f(x0)),
            Instr::SBC_r8_d8(x1) => format!("SBC {}", f(x1)),
            Instr::SBC_r8_ir16(x1) => format!("SBC ({})", f(x1)),
            Instr::SBC_r8_r8(x1) => format!("SBC {}", f(x1)),
            Instr::SCF => format!("SCF"),
            Instr::STOP => format!("STOP"),
            Instr::SUB_d8(x0) => format!("SUB {}", f(x0)),
            Instr::SUB_ir16(x0) => format!("SUB ({})", f(x0)),
            Instr::SUB_r8(x0) => format!("SUB {}", f(x0)),
            Instr::XOR_d8(x0) => format!("XOR {}", f(x0)),
            Instr::XOR_ir16(x0) => format!("XOR ({})", f(x0)),
            Instr::XOR_r8(x0) => format!("XOR {}", f(x0)),
            Instr::BIT_l8_ir16(x0, x1) => format!("BIT {},({})", f(x0), f(x1)),
            Instr::BIT_l8_r8(x0, x1) => format!("BIT {},{}", f(x0), f(x1)),
            Instr::RES_l8_ir16(x0, x1) => format!("RES {},({})", f(x0), f(x1)),
            Instr::RES_l8_r8(x0, x1) => format!("RES {},{}", f(x0), f(x1)),
            Instr::RLC_ir16(x0) => format!("RLC ({})", f(x0)),
            Instr::RLC_r8(x0) => format!("RLC {}", f(x0)),
            Instr::RL_ir16(x0) => format!("RL ({})", f(x0)),
            Instr::RL_r8(x0) => format!("RL {}", f(x0)),
            Instr::RRC_ir16(x0) => format!("RRC ({})", f(x0)),
            Instr::RRC_r8(x0) => format!("RRC {}", f(x0)),
            Instr::RR_ir16(x0) => format!("RR ({})", f(x0)),
            Instr::RR_r8(x0) => format!("RR {}", f(x0)),
            Instr::SLA_ir16(x0) => format!("SLA ({})", f(x0)),
            Instr::SLA_r8(x0) => format!("SLA {}", f(x0)),
            Instr::SRA_ir16(x0) => format!("SRA ({})", f(x0)),
            Instr::SRA_r8(x0) => format!("SRA {}", f(x0)),
            Instr::SRL_ir16(x0) => format!("SRL ({})", f(x0)),
            Instr::SRL_r8(x0) => format!("SRL {}", f(x0)),
            Instr::SET_l8_ir16(x0, x1) => format!("SET {},({})", f(x0), f(x1)),
            Instr::SET_l8_r8(x0, x1) => format!("SET {},{}", f(x0), f(x1)),
            Instr::SWAP_ir16(x0) => format!("SWAP ({})", f(x0)),
            Instr::SWAP_r8(x0) => format!("SWAP {}", f(x0)),
        }
    }
}

pub fn disasm<R: Addressable, W: Write, F: Fn(&Instr) -> bool>(
    mut start: u16,
    stop: u16,
    bytes: &mut R,
    buf: &mut W,
    filter: &F,
) -> std::io::Result<()> {
    while start < stop {
        let mut d = Disasm::new();
        let mut next_addr = start;
        let (next_addr, op) = loop {
            let b = bytes.read_byte(next_addr);
            next_addr = next_addr.wrapping_add(1);
            if let Some(i) = d.feed_byte(b) {
                break (next_addr, i);
            }
        };
        if filter(&op) {
            write!(buf, "0x{:04x}: ", start)?;
            for x in start..next_addr {
                write!(buf, "{:02x} ", bytes.read_byte(x))?;
            }
            for _ in next_addr.wrapping_sub(start)..3 {
                write!(buf, "   ")?;
            }
            write!(buf, "{}", op)?;
        }
        start = next_addr;
    }
    Ok(())
}
