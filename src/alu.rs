
use super::cpu::Flag;

pub struct ALU {}

impl ALU {
    pub fn and(a : u8, b: u8) -> (u8, u8) {
        let res = a & b;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::H, true)
        )
    }
    pub fn xor(a : u8, b :u8) -> (u8, u8) {
        let res = a ^ b;
        (res, flag_u8!(Flag::Z, res == 0))
    }
    pub fn or(a : u8, b :u8) -> (u8, u8) {
        let res = a | b;
        (res, flag_u8!(Flag::Z, res == 0))
    }
    pub fn bit(a : u8, b : u8) -> (u8, u8) {
        let res = (1 << a) & b;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, true)
        )
    }
    pub fn adc(a:u8, b: u8, carry : bool) -> (u8, u8) {
        let (mut res, mut c) = a.overflowing_add(b);
        let mut h = Self::half_carry(a, b);
        if carry {
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
    pub fn sbc(a:u8, b: u8, carry : bool) -> (u8, u8) {
        let (mut res, mut c) = a.overflowing_sub(b);
        let mut h = Self::sub_carry(a, b);
        if carry {
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

    pub fn rlca(a: u8, c: bool, thru_carry: bool, with_zero : bool) -> (u8, u8) {
        let res = if thru_carry {
            a.rotate_left(1) & !0x1 | if c {0x1} else {0}
        } else {
            a.rotate_left(1)
        };
        (res,
         flag_u8!(Flag::Z, with_zero && res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, (a & 0b1000_0000 > 0))
        )
    }
    pub fn rrca(a: u8, c: bool, thru_carry : bool, with_zero: bool) -> (u8, u8) {
        let res = if thru_carry {
            a.rotate_right(1) & !0x80 | if c {0x80} else {0}
        } else {
            a.rotate_right(1)
        };
        (res,
         flag_u8!(Flag::Z, with_zero && res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, (a & 0b0000_0001 > 0))
        )
    }

    pub fn sla(a: u8) -> (u8, u8) {
        let res = a << 1;
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, a & 0b1000_0000 > 0)
        )
    }
    pub fn sr(a: u8, arith : bool) -> (u8, u8) {
        let res = a >> 1 | if arith {a & 0x80} else {0};
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, a & 0b0000_0001 > 0)
        )
    }

    pub fn swap(a: u8) -> (u8, u8) {
        let res = ((a & 0x0f) << 4) | ((a & 0xf0) >> 4);
        (res,
         flag_u8!(Flag::Z, res == 0)
         | flag_u8!(Flag::N, false)
         | flag_u8!(Flag::H, false)
         | flag_u8!(Flag::C, false)
        )
    }
}

pub trait ALUOps<T> {
    fn add(a : T, b : T) -> (T, u8);
    fn sub(a : T, b : T) -> (T, u8);
    fn dec(a : T) -> (T, u8);
    fn inc(a : T) -> (T, u8);
    fn half_carry(a : T, b : T) -> bool;
    fn sub_carry(a: T, b : T) -> bool;
}

impl ALUOps<u8> for ALU {
    fn half_carry(a: u8, b: u8) -> bool {
        ((a & 0xF) + (b & 0xF)) & 0x10 != 0
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
        ((a & 0xFFF) + (b & 0xFFF)) & 0x1000 != 0
    }
    fn sub_carry(a: u16, b : u16) -> bool {
        a & 0xfff < b & 0xfff
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
        let mut h = Self::sub_carry(a, b);
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
