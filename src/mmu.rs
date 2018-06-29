use std::io;
use std::io::{Read, Seek, SeekFrom, Write};

use super::controller::Controller;
use super::display::Display;
use super::fakemem::FakeMem;
use super::mem::Mem;
use super::serial::Serial;
use super::timer::Timer;
use peripherals::Peripheral;

enum_from_primitive! {
    #[derive(Debug, PartialEq)]
    pub enum MemRegister {
        //Port/Mode Registers
        P1 = 0xff00,
        SB = 0xff01,
        SC = 0xff02,
        DIV = 0xff04,
        TIMA = 0xff05,
        TMA = 0xff06,
        TAC = 0xff07,
        //CGB KEY1 = 0xff4d,
        //CGB RP = 0xff56,
        //Bank Control Registers
        //CGB VBK = 0xff4f,
        //CGB SVBK = 0xff70,
        //Interrupt Flags
        IF = 0xff0f,
        IE = 0xffff,
        //IME = ?
        //LCD Display Registers
        LCDC = 0xff40,
        STAT = 0xff41,
        SCY = 0xff42,
        SCX = 0xff43,
        LY = 0xff44,
        LYC = 0xff45,
        DMA = 0xff46,
        BGP = 0xff47,
        OBP0 = 0xff48,
        OBP1 = 0xff49,
        WY = 0xff4a,
        WX = 0xff4b,
        //CGB
        // HDMA1 = 0xff51,
        // HDMA2 = 0xff52,
        // HDMA3 = 0xff53,
        // HDMA4 = 0xff54,
        // HDMA5 = 0xff55,
        // BCPS = 0xff68,
        // BCPD = 0xff69,
        // OCPS = 0xff6a,
        // OCPD = 0xff6b
    }
}

pub struct MMU<'a> {
    seek_pos: u16,
    bios_exists: bool,
    timer: Timer,
    display: Display,
    controller: Controller,
    bios: Mem,
    rom1: Mem,
    ram0: Mem,
    swap_ram: Mem,
    fake_mem: FakeMem,
    serial: Serial<'a>,
    ram1: Mem,
    ram2: Mem,
    interrupt_flag: Mem,
}

impl<'a> MMU<'a> {
    pub fn get_current_pos(&self) -> u16 {
        self.seek_pos
    }
    pub fn get_display(&mut self) -> &mut Display {
        &mut self.display
    }
    pub fn peripherals(&mut self) -> Vec<Box<&mut Peripheral>> {
        vec![
            Box::new(&mut self.bios as &mut Peripheral),
            Box::new(&mut self.timer as &mut Peripheral),
            Box::new(&mut self.display as &mut Peripheral),
            Box::new(&mut self.serial as &mut Peripheral),
        ]
    }
    pub fn new(rom: Vec<u8>, serial: Option<&mut Write>) -> MMU {
        let bios = Mem::new(true, 0, include_bytes!("../boot_rom.gb").to_vec());
        let rom1 = Mem::new(true, 0, rom);
        let ram0 = Mem::new(false, 0xc000, vec![0; 8 << 10]);
        let swap_ram = Mem::new(false, 0xa000, vec![0; 8 << 10]);
        let ram1 = Mem::new(false, 0xff80, vec![0; 0xffff - 0xff80 + 1]);
        let ram2 = Mem::new(false, 0xfea0, vec![0; 0xff00 - 0xfea0 + 1]);
        let interrupt_flag = Mem::new(false, 0xff0f, vec![0; 1]);
        let mem = MMU {
            seek_pos: 0,
            bios_exists: true,
            bios,
            rom1,
            display: Display::new(),
            timer: Timer::new(),
            serial: Serial::new(serial),
            controller: Controller::new(),
            ram0,
            swap_ram,
            fake_mem: FakeMem::new(),
            ram1,
            ram2,
            interrupt_flag,
        };
        mem
    }
    pub fn disable_bios(&mut self) {
        self.bios_exists = false;
    }

    fn lookup_peripheral(&mut self, addr: &mut u16) -> &mut Peripheral {
        match addr  {
            0x0000...0x00ff => if self.bios_exists {
                &mut self.bios as &mut Peripheral
            } else {
                &mut self.rom1 as &mut Peripheral
            },
            0x0100...0x7FFF =>  &mut self.rom1 as &mut Peripheral,
            0x8000...0x9FFF =>  &mut self.display as &mut Peripheral,
            0xA000...0xBFFF =>  &mut self.swap_ram as &mut Peripheral,
            0xC000...0xDFFF =>  &mut  self.ram0 as &mut Peripheral,
            0xE000...0xFDFF => {
                /* echo of ram0 */
                *addr -= 0x2000;
                &mut self.ram0 as &mut Peripheral
            }
            0xFE00...0xFE9F =>  &mut self.display as &mut Peripheral,
            0xFEA0...0xFEFF =>  &mut self.ram2 as &mut Peripheral,
            //0xFEA0...0xFEFF =>  self.empty0[($addr - 0xFEA0) as usize],
            //0xFF00...0xFF4B =>  self.io[($addr - 0xFF00) as usize],
            0xff40..=0xff45 =>  &mut self.display as &mut Peripheral,
            0xff47..=0xff4b =>  &mut self.display as &mut Peripheral,
            0xff00 =>  &mut self.controller as &mut Peripheral,
            0xff01..=0xff02 =>  &mut self.serial as &mut Peripheral,
            //0xFF4C...0xFF7F =>  self.empty1[($addr - 0xFF4C) as usize],
            0xFF04..=0xFF07 =>  &mut self.timer as &mut Peripheral,
            0xff0f =>  &mut self.interrupt_flag as &mut Peripheral,
            0xFF80...0xFFFF =>  &mut self.ram1  as &mut Peripheral,
            _ => &mut  self.fake_mem as &mut Peripheral,
        }
    }

    pub fn read_byte(&mut self, mut addr: u16) -> u8 {
        self.lookup_peripheral(&mut addr).read_byte(addr)
    }
    pub fn write_byte(&mut self, mut addr: u16, v: u8) {
        self.lookup_peripheral(&mut addr).write_byte(addr, v);
    }
    // fn dump(&mut self) {
    //     self.seek(SeekFrom::Start(0));
    //     disasm(0, self, &mut std::io::stdout(), &|i| match i {Instr::NOP => false, _ => true});
    // }
}

impl<'a> Write for MMU<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for (i, w) in buf.iter().enumerate() {
            let pos = self.seek_pos;
            {
                self.write_byte(pos, *w);
            }
            if self.seek_pos == std::u16::MAX {
                return Ok(i);
            }
            self.seek_pos = self.seek_pos.saturating_add(1);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> Read for MMU<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for (i, b) in buf.iter_mut().enumerate() {
            {
                let pos = self.seek_pos;
                *b = self.read_byte(pos);
            }
            if self.seek_pos == std::u16::MAX {
                return Ok(i);
            }
            self.seek_pos = self.seek_pos.saturating_add(1);
        }
        Ok(buf.len())
    }
}

fn apply_offset(mut pos: u16, seek: i64) -> io::Result<u64> {
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
        return Err(std::io::Error::new(
            io::ErrorKind::Other,
            "seeked before beginning",
        ));
    }
    Ok(pos as u64)
}

impl<'a> Seek for MMU<'a> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(x) => {
                let x = if x > std::u16::MAX as u64 {
                    std::u16::MAX
                } else {
                    x as u16
                };
                self.seek_pos = 0u16.saturating_add(x);
            }
            SeekFrom::End(x) => {
                self.seek_pos = apply_offset(0xffff, x)? as u16;
            }
            SeekFrom::Current(x) => {
                self.seek_pos = apply_offset(self.seek_pos, x)? as u16;
            }
        }
        Ok(self.seek_pos as u64)
    }
}
