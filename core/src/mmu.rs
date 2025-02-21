use super::controller::Controller;
use super::cpu::Interrupt;
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
use serde::{Deserialize, Serialize};

use std::io;
use std::io::{Seek, SeekFrom};
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
        BGPS = 0xFF68,
        BGPD = 0xFF69,
        OBPS = 0xFF6a,
        OBPD = 0xFF6b
    }
}

#[derive(Serialize, Deserialize, Clone)]
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
        self.reg & self.read_mask
    }
    pub fn set_reg(&mut self, val: u8) {
        self.reg = (self.reg & !self.write_mask) | (val & self.write_mask);
    }
}

impl Peripheral for MemReg {
    fn next_step(&self) -> Option<cycles::CycleCount> {
        /* MemRegs does not generate any interrupts and only needs to
        be updated when observed */
        Some(cycles::CycleCount::new(std::u64::MAX))
    }
}
impl Addressable for MemReg {
    fn write_byte(&mut self, _addr: u16, val: u8) {
        self.set_reg(val);
    }
    fn read_byte(&mut self, _addr: u16) -> u8 {
        self.reg()
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct SyncPeripheral<T>
where
    T: Peripheral,
{
    last_sync: cycles::CycleCount,
    next_sync: Option<cycles::CycleCount>,
    peripheral: T,
}

impl<T> SyncPeripheral<T>
where
    T: Peripheral,
{
    fn new(peripheral: T) -> SyncPeripheral<T> {
        SyncPeripheral {
            last_sync: cycles::CycleCount::new(0),
            next_sync: None,
            peripheral,
        }
    }
    pub fn inner(&self) -> &T {
        &self.peripheral
    }
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.peripheral
    }
    fn reset(&mut self) {
        self.next_sync = Some(cycles::CycleCount::new(0));
    }
}

impl<T> Peripheral for SyncPeripheral<T>
where
    T: Peripheral,
{
    fn next_step(&self) -> Option<cycles::CycleCount> {
        self.peripheral.next_step()
    }
    fn step(&mut self, real: &mut PeripheralData, time: cycles::CycleCount) -> Option<Interrupt> {
        self.last_sync += time;
        if let Some(next_sync) = self.next_sync {
            if next_sync > time {
                self.next_sync = Some(next_sync - time);
                return None;
            }
        }
        let result = self.peripheral.force_step(real, self.last_sync);
        self.last_sync = cycles::CycleCount::new(0);
        self.next_sync = self.peripheral.next_step();
        result
    }
    fn force_step(
        &mut self,
        real: &mut PeripheralData,
        time: cycles::CycleCount,
    ) -> Option<Interrupt> {
        self.next_sync = None;
        self.step(real, time)
    }
}

impl<T> Addressable for SyncPeripheral<T>
where
    T: Peripheral,
{
    fn read_byte(&mut self, addr: u16) -> u8 {
        self.peripheral.read_byte(addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        self.peripheral.write_byte(addr, v);
    }
    fn is_rom(&mut self, addr: u16) -> bool {
        self.peripheral.is_rom(addr)
    }
}

pub struct MMU<'b, 'c> {
    pub bus: &'b mut MMUInternal,
    data: &'b mut PeripheralData<'c>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MMUInternal {
    seek_pos: u16,
    bios_exists: bool,
    timer: SyncPeripheral<Timer>,
    display: SyncPeripheral<Display>,
    controller: SyncPeripheral<Controller>,
    dma: SyncPeripheral<DMA>,
    hdma: SyncPeripheral<HDMA>,
    svbk: MemReg, //TODO: GBC
    key1: MemReg, //TODO: GBC
    bios: Mem,
    cart: SyncPeripheral<Cart>,
    ram0: Vec<Mem>,
    fake_mem: FakeMem,
    serial: SyncPeripheral<Serial>,
    ram1: Mem,
    ram2: Mem,
    sound: SyncPeripheral<Mixer>,
    interrupt_flag: MemReg,
    interrupt_enable: MemReg,
    time: cycles::CycleCount,
    last_sync: cycles::CycleCount,
}

impl MMUInternal {
    pub fn ienable(&mut self) -> &mut MemReg {
        &mut self.interrupt_enable
    }
    pub fn iflag(&mut self) -> &mut MemReg {
        &mut self.interrupt_flag
    }

    pub fn mbc_rom(&mut self) -> &mut Vec<u8> {
        self.cart.inner_mut().mbc_rom()
    }
    pub fn set_controls(&mut self, controls: u8) {
        self.controller.inner_mut().set_controls(controls);
        self.controller.reset();
    }
    pub fn double_speed(&self) -> bool {
        (self.key1.reg & (1 << 7)) != 0
    }
    pub fn speed_change(&self) -> bool {
        (self.key1.reg & 0x1) != 0
    }
    pub fn new(
        cart: Cart,
        boot_rom: Option<Vec<u8>>,
        audio_sample_rate: Option<cycles::CycleCount>,
    ) -> Self {
        let bios_exists = boot_rom.is_some();
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
        let ram1 = Mem::new(false, 0xff80, vec![0; 0xfffe - 0xff80 + 1]);
        let ram2 = Mem::new(false, 0xfea0, vec![0; 0xff00 - 0xfea0 + 1]);
        let interrupt_flag = MemReg::default();
        let interrupt_enable = MemReg::default();
        let cart_mode = cart.cgb();
        MMUInternal {
            time: cycles::Cycles::new(0),
            last_sync: cycles::Cycles::new(0),
            seek_pos: 0,
            bios_exists,
            bios,
            cart: SyncPeripheral::new(cart),
            svbk: MemReg::new(0, 0b111, 0b111),
            key1: MemReg::new(0, !0, 0b1),
            display: SyncPeripheral::new(Display::new(cart_mode)),
            timer: SyncPeripheral::new(Timer::new()),
            serial: SyncPeripheral::new(Serial::new()),
            controller: SyncPeripheral::new(Controller::new()),
            sound: SyncPeripheral::new(Mixer::new(audio_sample_rate)),
            ram0,
            fake_mem: FakeMem::new(),
            ram1,
            ram2,
            dma: SyncPeripheral::new(DMA::new()),
            hdma: SyncPeripheral::new(HDMA::new()),
            interrupt_flag,
            interrupt_enable,
        }
    }
    pub fn lookup_peripheral(&mut self, addr: &mut u16) -> &mut dyn Peripheral {
        match addr {
            0x0000..=0x00FF => {
                if self.bios_exists {
                    &mut self.bios as &mut dyn Peripheral
                } else {
                    &mut self.cart as &mut dyn Peripheral
                }
            }
            0x0100..=0x01FF => &mut self.cart as &mut dyn Peripheral,
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
            0xFF80..=0xFFFE => &mut self.ram1 as &mut dyn Peripheral,
            0xFFFF => &mut self.interrupt_enable as &mut dyn Peripheral,
            _ => &mut self.fake_mem as &mut dyn Peripheral,
        }
    }
    pub fn sync_peripherals(&mut self, data: &mut PeripheralData, force: bool) {
        if force || self.last_sync < self.time {
            let cycles = self.time - self.last_sync;

            let interrupt_flag = (&mut [
                (&mut self.timer) as &mut dyn Peripheral,
                (&mut self.display) as &mut dyn Peripheral,
                (&mut self.serial) as &mut dyn Peripheral,
                (&mut self.controller) as &mut dyn Peripheral,
                (&mut self.sound) as &mut dyn Peripheral,
                //(&mut self.cart) as &mut dyn Peripheral,
            ])
                .into_iter()
                .map(|p| {
                    if force {
                        p.force_step(data, cycles)
                    } else {
                        p.step(data, cycles)
                    }
                })
                .filter_map(|x| x)
                .fold(Interrupt::new(), |acc, i| acc | i);
            self.last_sync = self.time;

            if self.dma.inner_mut().is_active() {
                let mut v = vec![];
                self.dma.force_step(data, cycles);
                v.extend(self.dma.inner_mut().copy_bytes());
                for (s, d) in v {
                    let v = self.read_byte_noeffect(data, s);
                    self.write_byte_noeffect(data, d, v);
                }
            }
            if self.hdma.inner_mut().is_active() {
                let mut v = vec![];
                /* TODO: This needs to hook into Hblanks in some cases */
                self.hdma.force_step(data, cycles);
                v.extend(self.hdma.inner_mut().copy_bytes());
                for (s, d) in v {
                    let v = self.read_byte_noeffect(data, s);
                    self.write_byte_noeffect(data, d, v);
                }
            }

            let flags = Interrupt::try_from(&[self.interrupt_flag.reg()][..]).unwrap();
            let mut rhs = flags | interrupt_flag;
            if !self.get_display().display_enabled() {
                /* remove vblank from IF when display disabled.
                We still want it to synchronize speed with display */
                rhs.set_vblank(false);
            }
            if rhs != flags {
                self.interrupt_flag.set_reg(rhs.to_bytes()[0]);
            }

            if interrupt_flag.get_vblank() {
                data.vblank = true;
            }
        }
    }
    pub fn time(&self) -> cycles::CycleCount {
        self.time
    }
    #[allow(dead_code)]
    pub fn get_display(&self) -> &Display {
        self.display.inner()
    }
    pub fn get_display_mut(&mut self) -> &mut Display {
        self.display.inner_mut()
    }
    pub fn disable_bios(&mut self) {
        self.bios_exists = false;
    }
    pub fn cycles_passed(&mut self, time: u64) {
        self.time += if self.double_speed() {
            cycles::CGB
        } else {
            cycles::GB
        } * time;
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

impl<'b, 'c> MMU<'b, 'c> {
    // fn dump(&mut self) {
    //     self.seek(SeekFrom::Start(0));
    //     disasm(0, self, &mut std::io::stdout(), &|i| match i {Instr::NOP => false, _ => true});
    // }
    pub fn new(bus: &'b mut MMUInternal, data: &'b mut PeripheralData<'c>) -> Self {
        MMU { bus, data }
    }
    pub fn toggle_speed(&mut self) {
        self.bus.key1.reg &= !0x1; //remove speed request
        self.bus.key1.reg ^= 1 << 7;

        self.bus.timer.force_step(self.data, cycles::Cycles::new(0));
        self.bus.timer.inner_mut().toggle_double();
        self.bus.timer.force_step(self.data, cycles::Cycles::new(0));
    }
    pub fn sync_peripherals(&mut self, force: bool) {
        self.bus.sync_peripherals(&mut self.data, force);
    }
    pub fn ack_vblank(&mut self) -> bool {
        let r = self.data.vblank;
        self.data.vblank = false;
        r
    }
    pub fn read_byte_noeffect(&mut self, addr: u16) -> u8 {
        self.bus.read_byte_noeffect(&mut self.data, addr)
    }
    pub fn write_byte_noeffect(&mut self, addr: u16, v: u8) {
        self.bus.write_byte_noeffect(&mut self.data, addr, v);
    }
}

impl Addressable for MMU<'_, '_> {
    fn read_byte(&mut self, addr: u16) -> u8 {
        self.bus.read_byte(&mut self.data, addr)
    }
    fn write_byte(&mut self, addr: u16, v: u8) {
        self.bus.write_byte(&mut self.data, addr, v);
    }
}

impl MMUInternal {
    fn read_byte_noeffect(&mut self, data: &mut PeripheralData, mut addr: u16) -> u8 {
        let mut tmp_addr = addr;
        let p = self.lookup_peripheral(&mut tmp_addr);
        let v = if p.is_rom(tmp_addr) {
            p.read_byte(tmp_addr)
        } else {
            self.sync_peripherals(data, false);
            let p = self.lookup_peripheral(&mut addr);
            p.force_step(data, cycles::Cycles::new(0));
            let v = p.read_byte(addr);
            p.force_step(data, cycles::Cycles::new(0));
            v
        };
        self.main_bus(false, addr, v);
        v
    }
    fn read_byte(&mut self, data: &mut PeripheralData, addr: u16) -> u8 {
        self.cycles_passed(1);
        self.read_byte_noeffect(data, addr)
    }
    fn write_byte_noeffect(&mut self, data: &mut PeripheralData, mut addr: u16, v: u8) {
        self.main_bus(true, addr, v);
        self.sync_peripherals(data, false);
        if addr == 0xff50 {
            self.disable_bios();
        } else {
            let p = self.lookup_peripheral(&mut addr);
            p.force_step(data, cycles::Cycles::new(0));
            p.write_byte(addr, v);
            p.force_step(data, cycles::Cycles::new(0));
        }
    }
    fn write_byte(&mut self, data: &mut PeripheralData, addr: u16, v: u8) {
        self.cycles_passed(1);
        self.write_byte_noeffect(data, addr, v);
    }
}

// impl Read for MMUInternal<'_> {
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         for (i, b) in buf.iter_mut().enumerate() {
//             *b = self.read_byte(self.seek_pos);
//             if self.seek_pos == std::u16::MAX {
//                 return Ok(i);
//             }
//             self.seek_pos = self.seek_pos.saturating_add(1);
//         }
//         Ok(buf.len())
//     }
// }

// impl Read for MMU<'_, '_, '_> {
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         self.bus.read(buf)
//     }
// }

// impl Write for MMUInternal<'_> {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         for (i, w) in buf.iter().enumerate() {
//             self.write_byte(self.seek_pos, *w);
//             if self.seek_pos == std::u16::MAX {
//                 return Ok(i);
//             }
//             self.seek_pos = self.seek_pos.saturating_add(1);
//         }
//         Ok(buf.len())
//     }

//     fn flush(&mut self) -> io::Result<()> {
//         Ok(())
//     }
// }
// impl Write for MMU<'_, '_, '_> {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         self.bus.write(buf)
//     }

//     fn flush(&mut self) -> io::Result<()> {
//         self.bus.flush()
//     }
// }

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

impl Seek for MMUInternal {
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
