#![feature(nll)]
#![feature(extern_prelude)]
#[macro_use]
extern crate enum_primitive;
use std::io::{Read, Write};

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


macro_rules! alu_result {
    ($s: expr, $r:expr, $v:expr) => {
        alu_result_mask!($s, $r, $v, Registers::default_mask())
    }
}
macro_rules! alu_result_mask {
    ($s: expr, $r:expr, $v:expr, $m:expr) => {
        {
            let (res, flags) = $v;
            $s.reg.write($r, res);
            $s.reg.write_mask(Reg8::F, flags, $m);
        }
    }
}


mod alu;
mod cpu;
pub mod display;
mod emptymem;
mod fakemem;
pub mod gb;
mod instr;
mod mem;
mod mmu;
mod peripherals;
mod serial;
mod timer;


use std::io;
use std::fs::File;
use std::fmt;

use gb::*;

fn make_u16(h : u8, l: u8) -> u16 {
    ((h as u16) << 8) | (l as u16)
}

fn split_u16(r : u16) -> (u8, u8) {
    (((r & 0xff00) >> 8) as u8, (r & 0xff) as u8)
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
        move |i : &instr::Instr| match i { instr::Instr::NOP => !filter_nops, _ => true };

    for r in regions.iter() {
        let mut taken = f.take(r.1);
        let mut buf = Cursor::new(taken);
        writeln!(dst, "{}:", r.2)?;
        instr::disasm(r.0, buf.get_mut(), &mut dst, &mut filter);
        f = buf.into_inner().into_inner();
    }
    Ok(())
}


macro_rules! rom_test {
    ($name:expr) => {
        let mut buf = ::std::io::BufWriter::new(Vec::new());
        {
            let mut gb = ::gb::GB::new(include_bytes!(concat!("../cpu_instrs/individual/", $name, ".gb")).to_vec(), Some(&mut buf), false);
            gb.step::<(u8, u8, u8, u8), (i32, i32)>(30 * 1000, &mut None);
        }
        assert_eq!(::std::str::from_utf8(&buf.into_inner().unwrap()).unwrap(), concat!($name, "\n\n\nPassed\n"));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let mut s = Vec::new();
        assert_eq!(::instr::Instr::disasm(&mut [0u8].as_ref()).unwrap(), (0, ::instr::Instr::NOP));
        let mut b = ::std::io::Cursor::new(s);
        ::instr::disasm(0, &mut [0u8, 0u8].as_ref(), &mut b, &|_| true).unwrap();
        assert_eq!(String::from_utf8(b.into_inner()).unwrap(), "0x0000: 00       NOP\n0x0001: 00       NOP\n");
        //::disasm_file("cpu_instrs/cpu_instrs.gb", true);
        ::disasm_file("cpu_instrs/individual/10-bit ops.gb", true);
        // let mut mem = ::MMU::::new();
        // mem.dump();
    }

    #[test]
    fn rom_test01() {
        rom_test!("01-special");
    }
    #[test]
    fn rom_test02() {
        rom_test!("02-interrupts");
    }
    #[test]
    fn rom_test03() {
        rom_test!("03-op sp,hl");
    }
    #[test]
    fn rom_test04() {
        rom_test!("04-op r,imm");
    }
    #[test]
    fn rom_test05() {
        rom_test!("05-op rp");
    }
    #[test]
    fn rom_test06() {
        rom_test!("06-ld r,r");
    }
    #[test]
    fn rom_test07() {
        rom_test!("07-jr,jp,call,ret,rst");
    }
    #[test]
    fn rom_test08() {
        rom_test!("08-misc instrs");
    }
    #[test]
    fn rom_test09() {
        rom_test!("09-op r,r");
    }
    #[test]
    fn rom_test10() {
        rom_test!("10-bit ops");
    }
    #[test]
    fn rom_test11() {
        rom_test!("11-op a,(hl)");
    }
}
