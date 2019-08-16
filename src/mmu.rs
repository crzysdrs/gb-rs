use super::controller::Controller;
use super::display::Display;
use super::fakemem::FakeMem;
use super::mem::Mem;
use super::serial::Serial;
use super::timer::Timer;
use crate::cart::Cart;
use crate::cycles;
use crate::dma::DMA;
use crate::hdma::HDMA;
use crate::peripherals::{Addressable, Peripheral, PeripheralData};
use crate::sound::Mixer;

use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
enum_from_primitive! {
    #[derive(Debug, PartialEq, Clone, Copy)]
    pub enum MemRegister {
        //Port/Mode Registers
        P1 = 0xFF00,
        SB = 0xFF01,
        SC = 0xFF02,
        DIV = 0xFF04,
        TIMA = 0xFF05,
        TMA = 0xFF06,
        TAC = 0xFF07,

        /* sound channel 1 */
        NR10 = 0xFF10,
        NR11 = 0xFF11,
        NR12 = 0xFF12,
        NR13 = 0xFF13,
        NR14 = 0xFF14,

        NR20 = 0xFF15,
        NR21 = 0xFF16,
        NR22 = 0xFF17,
        NR23 = 0xFF18,
        NR24 = 0xFF19,

        NR30 = 0xFF1A,
        NR31 = 0xFF1B,
        NR32 = 0xFF1C,
        NR33 = 0xFF1D,
        NR34 = 0xFF1E,

        NR40 = 0xFF1F,
        NR41 = 0xFF20,
        NR42 = 0xFF21,
        NR43 = 0xFF22,
        NR44 = 0xFF23,

        NR50 = 0xFF24,
        NR51 = 0xFF25,
        NR52 = 0xFF26,

        //CGB KEY1 = 0xFF4d,
        //CGB RP = 0xFF56,
        //Bank Control Registers
        //CGB VBK = 0xFF4f,
        //CGB SVBK = 0xFF70,
        //Interrupt Flags
        IF = 0xFF0F,
        IE = 0xFFFF,
        //IME = ?
        //LCD Display Registers
        LCDC = 0xFF40,
        STAT = 0xFF41,
        SCY = 0xFF42,
        SCX = 0xFF43,
        LY = 0xFF44,
        LYC = 0xFF45,
        DMA = 0xFF46,
        BGP = 0xFF47,
        OBP0 = 0xFF48,
        OBP1 = 0xFF49,
        WY = 0xFF4A,
        WX = 0xFF4B,
        //CGB
        HDMA1 = 0xFF51,
        HDMA2 = 0xFF52,
        HDMA3 = 0xFF53,
        HDMA4 = 0xFF54,
        HDMA5 = 0xFF55,
        BCPS = 0xFF68,
        BCPD = 0xFF69,
        OCPS = 0xFF6a,
        OCPD = 0xFF6b
    }
}

pub struct MemReg {
    reg: u8,
    read_mask: u8,
    write_mask: u8,
}

impl Default for MemReg {
    fn default() -> Self {
        MemReg::new(0, 0xff, 0xff)
    }
}

impl MemReg {
    pub fn new(v: u8, r: u8, w: u8) -> MemReg {
        MemReg {
            reg: v,
            read_mask: r,
            write_mask: w,
        }
    }
    pub fn reg(&self) -> u8 {
        self.reg
    }
}

impl Peripheral for MemReg {}
impl Addressable for MemReg {
    fn write_byte(&mut self, _addr: u16, val: u8) {
        self.reg = (val & !self.write_mask) | (val & self.write_mask);
    }
    fn read_byte(&mut self, _addr: u16) -> u8 {
        self.reg & self.read_mask
    }
}

pub struct MMU<'a, 'b, 'c> {
    pub bus: &'b mut MMUInternal<'a>,
    data: &'b mut PeripheralData<'c>,
}

pub struct MMUInternal<'a> {
    seek_pos: u16,
    bios_exists: bool,
    timer: Timer,
    display: Display,
    controller: Controller,
    dma: DMA,
    hdma: HDMA,
    svbk: MemReg, //TODO: GBC
    key1: MemReg, //TODO: GBC
    bios: Mem,
    cart: Cart,
    ram0: Vec<Mem>,
    fake_mem: FakeMem,
    serial: Serial<'a>,
    ram1: Mem,
    ram2: Mem,
    sound: Mixer,
    interrupt_flag: Mem,
    time: cycles::CycleCount,
    effectful_change: bool,
    last_sync: cycles::CycleCount,
}

pub fn side_effect_free<T, F: FnMut(&mut MMUInternal) -> T>(
    mmu: &mut MMUInternal,
    mut func: F,
) -> T {
    let temp = mmu.effectful_change;
    mmu.effectful_change = false;
    let t = func(mmu);
    mmu.effectful_change = temp;
    t
}

pub fn side_effect_free_mem<T, F: FnMut(&mut MMU) -> T>(mmu: &mut MMU, mut func: F) -> T {
    let temp = mmu.bus.effectful_change;
    mmu.bus.effectful_change = false;
    let t = func(mmu);
    mmu.bus.effectful_change = temp;
    t
}

impl MMUInternal<'_> {
    pub fn double_speed(&self) -> bool {
        (self.key1.reg & (1 << 7)) != 0
    }
    pub fn toggle_speed(&mut self) {
        self.key1.reg &= !0x1; //remove speed request
        self.key1.reg ^= 1 << 7;
    }
    pub fn speed_change(&self) -> bool {
        (self.key1.reg & 0x1) != 0
    }
    pub fn new(
        cart: Cart,
        serial: Option<&mut dyn Write>,
        boot_rom: Option<Vec<u8>>,
    ) -> MMUInternal {
        let bios = Mem::new(
            true,
            0,
            match boot_rom {
                Some(rom) => rom,
                None => vec![0; 0],
            },
        );
        let ram0 = (0..8)
            .map(|i| Mem::new(false, if i > 0 { 0xd000 } else { 0xc000 }, vec![0; 4 << 10]))
            .collect();
        let ram1 = Mem::new(false, 0xff80, vec![0; 0xffff - 0xff80 + 1]);
        let ram2 = Mem::new(false, 0xfea0, vec![0; 0xff00 - 0xfea0 + 1]);
        let interrupt_flag = Mem::new(false, 0xff0f, vec![0; 1]);
        let cart_mode = cart.cgb();
        MMUInternal {
            time: cycles::Cycles::new(0),
            last_sync: cycles::Cycles::new(0),
            seek_pos: 0,
            bios_exists: true,
            bios,
            cart,
            svbk: MemReg::new(0, 0b111, 0b111),
            key1: MemReg::new(0, !0, 0b1),
            display: Display::new(cart_mode),
            timer: Timer::new(),
            serial: Serial::new(serial),
            controller: Controller::new(),
            sound: Mixer::new(), //Mem::new(false, 0xff10, vec![0u8; 0xff3f - 0xff10 + 1]),
            ram0,
            fake_mem: FakeMem::new(),
            ram1,
            ram2,
            dma: DMA::new(),
            hdma: HDMA::new(),
            interrupt_flag,
            effectful_change: true,
        }
    }
    fn lookup_peripheral(&mut self, addr: &mut u16) -> &mut dyn Peripheral {
        match addr {
            0x0000..=0x00FF => {
                if self.bios_exists {
                    &mut self.bios as &mut dyn Peripheral
                } else {
                    &mut self.cart as &mut dyn Peripheral
                }
            }
            0x0100..=0x0200 => &mut self.cart as &mut dyn Peripheral,
            0x0200..=0x08FF => {
                //TODO:GBC
                if self.bios_exists && self.bios.len() > 256 {
                    //*addr -= 0x100;
                    &mut self.bios as &mut dyn Peripheral
                } else {
                    &mut self.cart as &mut dyn Peripheral
                }
            }
            0x0900..=0x7FFF => &mut self.cart as &mut dyn Peripheral,
            0x8000..=0x9FFF => &mut self.display as &mut dyn Peripheral,
            0xA000..=0xBFFF => &mut self.cart as &mut dyn Peripheral,
            0xC000..=0xCFFF => &mut self.ram0[0] as &mut dyn Peripheral,
            0xD000..=0xDFFF => {
                &mut self.ram0[usize::from(std::cmp::max(self.svbk.reg(), 1))]
                    as &mut dyn Peripheral
            }
            0xE000..=0xEFFF => {
                /* echo of ram0 */
                *addr -= 0x2000;
                &mut self.ram0[0] as &mut dyn Peripheral
            }
            0xF000..=0xFDFF => {
                /* echo of ram0 */
                *addr -= 0x2000;
                &mut self.ram0[1] as &mut dyn Peripheral
            }
            0xFE00..=0xFE9F => &mut self.display as &mut dyn Peripheral,
            0xFEA0..=0xFEFF => &mut self.ram2 as &mut dyn Peripheral,
            //0xFEA0..=0xFEFF =>  self.empty0[($addr - 0xFEA0) as usize],
            //0xFF00..=0xFF4B =>  self.io[($addr - 0xFF00) as usize],
            0xff40..=0xff45 | 0xff47..=0xff4b | 0xff4f | 0xff68..=0xff6b => {
                &mut self.display as &mut dyn Peripheral
            }
            0xff00 => &mut self.controller as &mut dyn Peripheral,
            0xff01..=0xff02 => &mut self.serial as &mut dyn Peripheral,
            //0xFF4C..=0xFF7F =>  self.empty1[($addr - 0xFF4C) as usize],
            0xFF04..=0xFF07 => &mut self.timer as &mut dyn Peripheral,
            0xff0f => &mut self.interrupt_flag as &mut dyn Peripheral,
            0xff10..=0xFF3F => &mut self.sound as &mut dyn Peripheral,
            0xff70 => &mut self.svbk as &mut dyn Peripheral, //TODO: GBC
            0xff4d => &mut self.key1 as &mut dyn Peripheral, //TODO: GBC
            0xff46 => &mut self.dma as &mut dyn Peripheral,
            0xff51..=0xff55 => &mut self.hdma as &mut dyn Peripheral,
            0xFF80..=0xFFFF => &mut self.ram1 as &mut dyn Peripheral,
            _ => &mut self.fake_mem as &mut dyn Peripheral,
        }
    }
    pub fn sync_peripherals(&mut self, data: &mut PeripheralData) {
        if self.last_sync < self.time {
            let mut interrupt_flag = 0;
            let cycles = self.time - self.last_sync;
            if self.dma.is_active() {
                self.dma.step(data, cycles);
                for (s, d) in self.dma.copy_bytes() {
                    let v = self.read_byte(s);
                    self.write_byte(d, v);
                }
            }
            if self.hdma.is_active() {
                /* TODO: This needs to hook into Vblanks in some cases */
                self.hdma.step(data, cycles);
                for (s, d) in self.hdma.copy_bytes() {
                    let v = self.read_byte(s);
                    self.write_byte(d, v);
                }
            }
            self.walk_peripherals(|p| {
                if let Some(i) = p.step(data, cycles) {
                    interrupt_flag |= mask_u8!(i);
                }
            });
            let flags = side_effect_free(self, |mmu| mmu.read_byte(0xff0f));
            let mut rhs = flags | interrupt_flag;
            if !self.get_display().display_enabled() {
                /* remove vblank from IF when display disabled.
                We still want it to synchronize speed with display */
                rhs &= !mask_u8!(crate::cpu::InterruptFlag::VBlank);
            }
            if rhs != flags {
                side_effect_free(self, |mmu| mmu.write_byte(0xff0f, rhs));
            }
            if interrupt_flag & mask_u8!(crate::cpu::InterruptFlag::VBlank) != 0 {
                data.vblank = true
            }
            self.last_sync = self.time;
        }
    }
    pub fn time(&self) -> cycles::CycleCount {
        self.time
    }
    pub fn set_controls(&mut self, controls: u8) {
        self.controller.set_controls(controls);
    }
    pub fn walk_peripherals<F>(&mut self, mut walk: F)
    where
        F: FnMut(&mut dyn Peripheral) -> (),
    {
        let ps: &mut [&mut dyn Peripheral] = &mut [
            &mut self.bios as &mut dyn Peripheral,
            &mut self.timer as &mut dyn Peripheral,
            &mut self.display as &mut dyn Peripheral,
            &mut self.serial as &mut dyn Peripheral,
            &mut self.controller as &mut dyn Peripheral,
            &mut self.sound as &mut dyn Peripheral,
            &mut self.cart as &mut dyn Peripheral,
        ];
        for p in ps.iter_mut() {
            walk(*p)
        }
    }

    #[allow(dead_code)]
    pub fn get_display(&self) -> &Display {
        &self.display
    }
    pub fn set_time(&mut self, v: cycles::CycleCount) {
        self.time = v;
    }
    pub fn disable_bios(&mut self) {
        self.bios_exists = false;
    }
    pub fn cycles_passed(&mut self, time: u64) {
        if self.effectful_change {
            self.time += if self.double_speed() {
                cycles::CGB
            } else {
                cycles::GB
            } * time;
        }
        #[cfg(feature = "vcd_dump")]
        {
            use crate::VCDDump::VCD;
            VCD.as_ref().map(|m| {
                m.lock().unwrap().as_mut().map(|v| {
                    let c = self.time.value_unsafe;
                    v.now = c;
                })
            });
        }
    }
    #[allow(unused_variables)]
    fn main_bus(&mut self, write: bool, addr: u16, v: u8) {
        #[cfg(feature = "vcd_dump")]
        {
            let (vcd_addr, vcd_val) = if write {
                ("write_addr", "write_data")
            } else {
                ("read_addr", "read_data")
            };
            use crate::VCDDump::VCD;
            VCD.as_ref().map(|m| {
                m.lock().unwrap().as_mut().map(|vcd| {
                    let (mut writer, mem) = vcd.writer();
                    let (wire, id) = mem.get(vcd_addr).unwrap();
                    wire.write(&mut writer, *id, addr as u64);
                    let (wire, id) = mem.get(vcd_val).unwrap();
                    wire.write(&mut writer, *id, v as u64);
                })
            });
        }
    }
}

impl<'a, 'b, 'c> MMU<'a, 'b, 'c> {
    // fn dump(&mut self) {
    //     self.seek(SeekFrom::Start(0));
    //     disasm(0, self, &mut std::io::stdout(), &|i| match i {Instr::NOP => false, _ => true});
    // }
    pub fn new(bus: &'b mut MMUInternal<'a>, data: &'b mut PeripheralData<'c>) -> MMU<'a, 'b, 'c> {
        MMU { bus, data }
    }

    pub fn sync_peripherals(&mut self) {
        self.bus.sync_peripherals(&mut self.data);
    }
    pub fn seen_vblank(&self) -> bool {
        self.data.vblank
    }
    pub fn read_byte_silent(&mut self, mut addr: u16) -> u8 {
        side_effect_free(self.bus, |bus| {
            bus.lookup_peripheral(&mut addr).read_byte(addr)
        })
    }
    pub fn write_byte_silent(&mut self, mut addr: u16, v: u8) {
        side_effect_free(self.bus, |bus| {
            bus.lookup_peripheral(&mut addr).write_byte(addr, v)
        });
    }
}

impl Addressable for MMU<'_, '_, '_> {
    fn read_byte(&mut self, addr: u16) -> u8 {
        self.bus.sync_peripherals(&mut self.data);
        self.bus.read_byte(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        self.bus.sync_peripherals(&mut self.data);
        self.bus.write_byte(addr, v);
    }
}

impl MMUInternal<'_> {
    fn read_byte(&mut self, mut addr: u16) -> u8 {
        self.cycles_passed(1);
        let v = self.lookup_peripheral(&mut addr).read_byte(addr);
        self.main_bus(false, addr, v);
        v
    }
    fn write_byte(&mut self, mut addr: u16, v: u8) {
        self.cycles_passed(1);
        self.main_bus(true, addr, v);
        if addr == 0xff50 {
            self.disable_bios();
        } else {
            self.lookup_peripheral(&mut addr).write_byte(addr, v);
        }
    }
}

impl Read for MMUInternal<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = self.read_byte(self.seek_pos);
            if self.seek_pos == std::u16::MAX {
                return Ok(i);
            }
            self.seek_pos = self.seek_pos.saturating_add(1);
        }
        Ok(buf.len())
    }
}

impl Read for MMU<'_, '_, '_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.bus.read(buf)
    }
}

impl Write for MMUInternal<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for (i, w) in buf.iter().enumerate() {
            self.write_byte(self.seek_pos, *w);
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
impl Write for MMU<'_, '_, '_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bus.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.bus.flush()
    }
}

fn apply_offset(mut pos: u16, seek: i64) -> io::Result<u64> {
    let seek = if seek > i64::from(std::i16::MAX) {
        std::i16::MAX
    } else if seek < i64::from(std::i16::MIN) {
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
    Ok(u64::from(pos))
}

impl Seek for MMUInternal<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(x) => {
                let x = if x > u64::from(std::u16::MAX) {
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
        Ok(u64::from(self.seek_pos))
    }
}
