#![cfg_attr(feature = "strict", deny(warnings))]

#[macro_use]
extern crate enum_primitive;
extern crate itertools;
extern crate num;
extern crate vcd;

#[macro_use]
extern crate dimensioned;
use dimensioned as dim;
pub mod cycles {
    make_units! {
        Cycles;
        ONE: Unitless;
        base {
            CGB: CGB, "gbc cycles", Time;
        }
        derived {
        }
        constants {
        }
        fmt = true;
    }
    pub const SECOND: CGB<u64> = Cycles {
        value_unsafe: 4_194_304 / 2,
        _marker: std::marker::PhantomData,
    };
    pub const GB: CGB<u64> = Cycles {
        value_unsafe: 2,
        _marker: std::marker::PhantomData,
    };
    pub const CGB: CGB<u64> = Cycles {
        value_unsafe: 1,
        _marker: std::marker::PhantomData,
    };
    pub type CycleCount = CGB<u64>;
}
use dim::si;
use dim::typenum::{Integer, Prod, Z0};
use std::convert::From;
use std::ops::Mul;

impl<Time> From<si::SI<f64, tarr![Z0, Z0, Time, Z0, Z0, Z0, Z0]>>
    for cycles::Cycles<Prod<u64, u64>, tarr![Time]>
where
    Time: Integer,
{
    fn from(other: si::SI<f64, tarr![Z0, Z0, Time, Z0, Z0, Z0, Z0]>) -> Self {
        let length_fac = cycles::SECOND.value_unsafe.pow(Time::to_i32() as u32);
        //println!("TIME: {} LENGTH: {}", Time::to_i32(), length_fac);
        cycles::Cycles::new((other.value_unsafe as f64 * length_fac as f64) as u64)
    }
}

impl<Time> Into<si::SI<Prod<dyn Mul<f64, Output = f64>, f64>, tarr![Z0, Z0, Time, Z0, Z0, Z0, Z0]>>
    for cycles::Cycles<u64, tarr![Time]>
where
    Time: Integer,
{
    fn into(
        self,
    ) -> si::SI<Prod<dyn Mul<f64, Output = f64>, f64>, tarr![Z0, Z0, Time, Z0, Z0, Z0, Z0]> {
        //println!("TIME: {} {}", Time::to_i32(), si::S.value_unsafe);

        let time_fac = (si::S / cycles::SECOND.value_unsafe as f64)
            .value_unsafe
            .powi(Time::to_i32());
        let fac = time_fac;

        si::SI::new(self.value_unsafe as f64 * fac)
    }
}

// impl<V, Time> Into<
//         si::SI<Prod<V, f64>, tarr![Z0, Z0, Time, Z0, Z0, Z0, Z0]>>
//     for cycles::Cycles<V, tarr![Time]>
//     where V: Mul<f64>,Time: Integer,
// {
//     fn into(self) -> si::SI<Prod<V, f64>, tarr![Z0, Z0, Time, Z0, Z0, Z0, Z0]> {
//         let time_fac = si::S.value_unsafe.powi(Time::to_i32()) as f64;
//         let fac = time_fac;

//         si::SI::new( self.value_unsafe * fac )
//     }
// }

#[cfg(feature = "vcd_dump")]
#[macro_use]
extern crate lazy_static;

#[cfg(feature = "vcd_dump")]
mod VCDDump {

    use std::collections::HashMap;
    use std::sync::Mutex;
    type VCDMap = HashMap<std::borrow::Cow<'static, str>, (Wire, vcd::IdCode)>;
    pub struct VCDItems {
        vcd: std::fs::File,
        last_emit: u64,
        pub now: u64,
        mem: VCDMap,
    }

    impl VCDItems {
        pub fn writer(&mut self) -> (vcd::Writer, &mut VCDMap) {
            let mut w = vcd::Writer::new(&mut self.vcd);
            if self.now > self.last_emit {
                self.last_emit = self.now;
                w.timestamp(self.now);
            }
            (w, &mut self.mem)
        }
    }

    pub struct Wire {
        typ: WireType,
        name: std::borrow::Cow<'static, str>,
    }

    impl Wire {
        fn num_to_vcd(width: u32, num: u64) -> Vec<vcd::Value> {
            (0..width)
                .into_iter()
                .rev()
                .map(|i| {
                    if num & (1 << i) != 0 {
                        vcd::Value::V1
                    } else {
                        vcd::Value::V0
                    }
                })
                .collect()
        }

        fn size(&self) -> u32 {
            match self.typ {
                WireType::Scalar => 1,
                WireType::Vector(s) => s,
            }
        }
        pub fn write(&self, writer: &mut vcd::Writer, id: vcd::IdCode, val: u64) {
            match self.typ {
                WireType::Scalar => writer.change_scalar(id, Wire::num_to_vcd(self.size(), val)[0]),
                WireType::Vector(_) => {
                    writer.change_vector(id, &Wire::num_to_vcd(self.size(), val))
                }
            };
        }
    }
    enum WireType {
        Scalar,
        Vector(u32),
    }

    use std::borrow::Cow;

    lazy_static! {
        pub static ref VCD : Option<Mutex<Option<VCDItems>>> = {
            fn make_vcd() -> std::io::Result<VCDItems> {
                let mut file = std::fs::File::create("test.vcd")?;
                let mut h = HashMap::new();
                let mut writer = vcd::Writer::new(&mut file);
                writer.timescale(1, vcd::TimescaleUnit::US)?;
                writer.add_module("mem")?;
                let mut wires = vec![
                    Wire {typ: WireType::Vector(16), name:Cow::Borrowed("write_addr")},
                    Wire {typ: WireType::Vector(8), name:Cow::Borrowed("write_data")},
                    Wire {typ: WireType::Vector(16), name:Cow::Borrowed("read_addr")},
                    Wire {typ: WireType::Vector(8), name:Cow::Borrowed("read_data")},
                    Wire {typ: WireType::Scalar, name:Cow::Borrowed("length")},
                    Wire {typ: WireType::Scalar, name:Cow::Borrowed("vol")},
                    Wire {typ: WireType::Scalar, name:Cow::Borrowed("sweep")},
                ];
                wires.extend((0xff00..0xff50).into_iter().map(
                    |addr|
                    Wire {typ: WireType::Vector(8), name:Cow::Owned(format!("0x{:04x}", addr))}
                )
                );

                for w in wires.drain(0..) {
                    let id = writer.add_wire(
                        w.size(), &w.name)?;
                    h.insert(w.name.to_owned(), (w, id));
                }
                writer.upscope()?;
                writer.enddefinitions()?;
                // Write the initial values
                writer.begin(vcd::SimulationCommand::Dumpvars)?;
                for (_name, (wire, id)) in &h {
                    match wire.typ {
                        WireType::Vector(s) => writer.change_vector(*id, &vec![vcd::Value::X; s as usize]),
                        WireType::Scalar => writer.change_scalar(*id, vcd::Value::X),
                    };
                }
                writer.end()?;

                Ok(VCDItems {
                    last_emit: 0,
                    now: 0,
                    vcd: file,
                    mem: h,
                })
            }
            if cfg!(not(test)) && cfg!(debug_assertions) {
                Some(Mutex::new(make_vcd().ok()))
            } else {
                None
            }

        };
    }
}

macro_rules! flag_u8 {
    ($x:path) => {
        $x as u8
    };
    ($x:path, $cond:expr) => {
        if $cond {
            $x as u8
        } else {
            0
        }
    };
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
    ($s:expr, $r:expr, $v:expr) => {
        alu_result_mask!($s, $r, $v, Registers::default_mask())
    };
}

macro_rules! alu_result_mask {
    ($s:expr, $r:expr, $v:expr, $m:expr) => {{
        let (res, flags) = $v;
        $s.reg.write($r, res);
        $s.reg.write_mask(Reg8::F, flags, $m);
    }};
}

mod alu;
pub mod cart;
mod controller;
mod cpu;
pub mod display;
mod dma;
mod emptymem;
mod fakemem;
pub mod gb;
mod hdma;
pub mod instr;
mod mem;
mod mmu;
pub mod peripherals;
mod serial;
pub mod sound;
mod timer;

#[cfg(test)]
fn test_data() -> (Vec<u8>, impl Fn(&[i16]) -> bool) {
    (vec![0u8; 160 * 144 * 4], |_| true)
}
// #[cfg(test)]
// fn disasm_file(file: &str, filter_nops: bool) -> std::io::Result<()> {
//     use std::fs::File;
//     use std::io::Cursor;
//     use std::io::{Read, Write};
//     let mut f = File::open(file)?;
//     let regions = [
//         (0x0000, 8, "Restart"),
//         (0x0008, 8, "Restart"),
//         (0x0010, 8, "Restart"),
//         (0x0018, 8, "Restart"),
//         (0x0020, 8, "Restart"),
//         (0x0028, 8, "Restart"),
//         (0x0030, 8, "Restart"),
//         (0x0038, 8, "Restart"),
//         (0x0040, 8, "VBlank"),
//         (0x0048, 8, "LCDC"),
//         (0x0050, 8, "Timer Overflow"),
//         (0x0058, 8, "Serial Transfer"),
//         (0x0060, (0x100 - 0x60), "P10-P13"),
//         (0x0100, 4, "Start"),
//         (0x0104, (0x134 - 0x104), "GameBoy Logo"),
//         (0x0134, (0x143 - 0x134), "Title"),
//         (0x0143, (0x150 - 0x143), "Other Data"),
//         (0x0150, (0xffff - 0x0150), "The Rest"),
//     ];
//     let mut dst = std::io::stdout();
//     let mut filter = move |i: &instr::Instr| match i {
//         instr::Instr::NOP => !filter_nops,
//         _ => true,
//     };

//     for r in regions.iter() {
//         let taken = f.take(r.1);
//         let mut buf = Cursor::new(taken);
//         writeln!(dst, "{}:", r.2)?;
//         instr::disasm(r.0, buf.get_mut(), &mut dst, &mut filter).unwrap();
//         f = buf.into_inner().into_inner();
//     }
//     Ok(())
// }

#[cfg(test)]
mod tests {
    fn read_screen(gb: &mut crate::gb::GB) -> String {
        use itertools::Itertools;
        let bg_tiles = gb.get_mem().get_display().all_bgs();
        bg_tiles
            .chunks(32)
            .map(|line| std::str::from_utf8(&line).unwrap().trim())
            .intersperse(&"\n".to_owned())
            .map(|s| s.replace('\0', &" "))
            .filter(|s| s.len() > 0)
            .collect::<String>()
            .trim()
            .to_owned()
    }

    macro_rules! passed {
        ($test: expr) => {
            concat!($test, "\n\n\nPassed")
        };
    }
    macro_rules! blarg_test {
        ($name:tt, $path:expr, $test:expr) => {
            #[test]
            fn $name() {
                use crate::cart::Cart;
                let mut buf = ::std::io::BufWriter::new(Vec::new());
                let (mut v, mut f) = crate::test_data();
                let mut p = crate::peripherals::PeripheralData::new(
                    Some(&mut v),
                    Some(crate::peripherals::AudioSpec {
                        queue: Box::new(&mut f),
                        freq: 16384 * 4,
                        silence: 0,
                    }),
                );
                let screen = {
                    let mut gb = crate::gb::GB::new(
                        Cart::new(include_bytes!($path).to_vec()),
                        Some(&mut buf),
                        false,
                        None,
                        None,
                    );
                    gb.step_timeout(Some((30.0 * dimensioned::si::S).into()), &mut p);
                    read_screen(&mut gb)
                };
                assert_eq!(screen, $test);
            }
        };
    }

    macro_rules! mooneye_test {
        ($name:tt, $path:expr) => {
            #[test]
            fn $name() {
                use crate::cart::Cart;
                use crate::cpu::Reg8;
                use crate::cpu::RegType;

                let mut buf = ::std::io::BufWriter::new(Vec::new());
                let (finished, reg, screen) = {
                    let mut gb = crate::gb::GB::new(
                        Cart::new(include_bytes!($path).to_vec()),
                        Some(&mut buf),
                        false,
                        None,
                        None,
                    );
                    gb.magic_breakpoint();

                    (
                        gb.step_timeout(
                            Some((dimensioned::si::S * 30.0).into()),
                            &mut crate::peripherals::PeripheralData::empty(),
                        ),
                        gb.get_reg(),
                        read_screen(&mut gb),
                    )
                };
                let buf = buf.into_inner().unwrap();
                let output = ::std::str::from_utf8(&buf).unwrap();
                println!("{}", output);
                assert_eq!(finished, crate::gb::GBReason::Dead);
                assert_eq!(reg.read(Reg8::B), 3);
                assert_eq!(reg.read(Reg8::C), 5);
                assert_eq!(reg.read(Reg8::D), 8);
                assert_eq!(reg.read(Reg8::E), 13);
                assert_eq!(reg.read(Reg8::H), 21);
                assert_eq!(reg.read(Reg8::L), 34);
                assert_eq!(screen, "Test OK");
                //assert_eq!(output, "TEST OK");
            }
        };
    }
    // #[test]
    // fn it_works() {
    //     let s = Vec::new();
    //     assert_eq!(
    //         crate::instr::Instr::disasm(&mut [0u8].as_ref()).unwrap(),
    //         (0, crate::instr::Instr::NOP)
    //     );
    //     let mut b = ::std::io::Cursor::new(s);
    //     crate::instr::disasm(0, &mut [0u8, 0u8].as_ref(), &mut b, &|_| true).unwrap();
    //     assert_eq!(
    //         String::from_utf8(b.into_inner()).unwrap(),
    //         "0x0000: 00       NOP\n0x0001: 00       NOP\n"
    //     );
    //     //::disasm_file("cpu_instrs/cpu_instrs.gb", true);
    //     crate::disasm_file("blarg/cpu_instrs/10-bit_ops.gb", true).unwrap();
    //     // let mut mem = ::MMU::::new();
    //     // mem.dump();
    // }
    #[should_panic]
    // mooneye_test!(mooneye_gb_tests_build_acceptance_add_sp_e_timing_gb, "../mooneye-gb/tests/build/acceptance/add_sp_e_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_bits_mem_oam_gb, "../mooneye-gb/tests/build/acceptance/bits/mem_oam.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_bits_reg_f_gb, "../mooneye-gb/tests/build/acceptance/bits/reg_f.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_bits_unused_hwio_gs_gb, "../mooneye-gb/tests/build/acceptance/bits/unused_hwio-GS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_hwio_s_gb, "../mooneye-gb/tests/build/acceptance/boot_hwio-S.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_hwio_dmg0_gb, "../mooneye-gb/tests/build/acceptance/boot_hwio-dmg0.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_hwio_dmgabcmgb_gb, "../mooneye-gb/tests/build/acceptance/boot_hwio-dmgABCmgb.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_regs_dmg0_gb, "../mooneye-gb/tests/build/acceptance/boot_regs-dmg0.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_regs_dmgabc_gb, "../mooneye-gb/tests/build/acceptance/boot_regs-dmgABC.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_regs_mgb_gb, "../mooneye-gb/tests/build/acceptance/boot_regs-mgb.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_regs_sgb_gb, "../mooneye-gb/tests/build/acceptance/boot_regs-sgb.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_boot_regs_sgb2_gb, "../mooneye-gb/tests/build/acceptance/boot_regs-sgb2.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_call_cc_timing_gb, "../mooneye-gb/tests/build/acceptance/call_cc_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_call_cc_timing2_gb, "../mooneye-gb/tests/build/acceptance/call_cc_timing2.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_call_timing_gb, "../mooneye-gb/tests/build/acceptance/call_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_call_timing2_gb, "../mooneye-gb/tests/build/acceptance/call_timing2.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_di_timing_gs_gb, "../mooneye-gb/tests/build/acceptance/di_timing-GS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_div_timing_gb, "../mooneye-gb/tests/build/acceptance/div_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ei_sequence_gb, "../mooneye-gb/tests/build/acceptance/ei_sequence.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ei_timing_gb, "../mooneye-gb/tests/build/acceptance/ei_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_halt_ime0_ei_gb, "../mooneye-gb/tests/build/acceptance/halt_ime0_ei.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_halt_ime0_nointr_timing_gb, "../mooneye-gb/tests/build/acceptance/halt_ime0_nointr_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_halt_ime1_timing_gb, "../mooneye-gb/tests/build/acceptance/halt_ime1_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_halt_ime1_timing2_gs_gb, "../mooneye-gb/tests/build/acceptance/halt_ime1_timing2-GS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_if_ie_registers_gb, "../mooneye-gb/tests/build/acceptance/if_ie_registers.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_interrupts_ie_push_gb, "../mooneye-gb/tests/build/acceptance/interrupts/ie_push.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_intr_timing_gb, "../mooneye-gb/tests/build/acceptance/intr_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_jp_cc_timing_gb, "../mooneye-gb/tests/build/acceptance/jp_cc_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_jp_timing_gb, "../mooneye-gb/tests/build/acceptance/jp_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ld_hl_sp_e_timing_gb, "../mooneye-gb/tests/build/acceptance/ld_hl_sp_e_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_oam_dma_basic_gb, "../mooneye-gb/tests/build/acceptance/oam_dma/basic.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_oam_dma_reg_read_gb, "../mooneye-gb/tests/build/acceptance/oam_dma/reg_read.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_oam_dma_sources_dmgabcmgbs_gb, "../mooneye-gb/tests/build/acceptance/oam_dma/sources-dmgABCmgbS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_oam_dma_restart_gb, "../mooneye-gb/tests/build/acceptance/oam_dma_restart.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_oam_dma_start_gb, "../mooneye-gb/tests/build/acceptance/oam_dma_start.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_oam_dma_timing_gb, "../mooneye-gb/tests/build/acceptance/oam_dma_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_pop_timing_gb, "../mooneye-gb/tests/build/acceptance/pop_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_hblank_ly_scx_timing_gs_gb, "../mooneye-gb/tests/build/acceptance/ppu/hblank_ly_scx_timing-GS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_intr_1_2_timing_gs_gb, "../mooneye-gb/tests/build/acceptance/ppu/intr_1_2_timing-GS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_intr_2_0_timing_gb, "../mooneye-gb/tests/build/acceptance/ppu/intr_2_0_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_intr_2_mode0_timing_gb, "../mooneye-gb/tests/build/acceptance/ppu/intr_2_mode0_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_intr_2_mode0_timing_sprites_gb, "../mooneye-gb/tests/build/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_intr_2_mode3_timing_gb, "../mooneye-gb/tests/build/acceptance/ppu/intr_2_mode3_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_intr_2_oam_ok_timing_gb, "../mooneye-gb/tests/build/acceptance/ppu/intr_2_oam_ok_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_lcdon_timing_dmgabcmgbs_gb, "../mooneye-gb/tests/build/acceptance/ppu/lcdon_timing-dmgABCmgbS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_lcdon_write_timing_gs_gb, "../mooneye-gb/tests/build/acceptance/ppu/lcdon_write_timing-GS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_stat_irq_blocking_gb, "../mooneye-gb/tests/build/acceptance/ppu/stat_irq_blocking.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_stat_lyc_onoff_gb, "../mooneye-gb/tests/build/acceptance/ppu/stat_lyc_onoff.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ppu_vblank_stat_intr_gs_gb, "../mooneye-gb/tests/build/acceptance/ppu/vblank_stat_intr-GS.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_push_timing_gb, "../mooneye-gb/tests/build/acceptance/push_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_rapid_di_ei_gb, "../mooneye-gb/tests/build/acceptance/rapid_di_ei.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ret_cc_timing_gb, "../mooneye-gb/tests/build/acceptance/ret_cc_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_ret_timing_gb, "../mooneye-gb/tests/build/acceptance/ret_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_reti_intr_timing_gb, "../mooneye-gb/tests/build/acceptance/reti_intr_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_reti_timing_gb, "../mooneye-gb/tests/build/acceptance/reti_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_rst_timing_gb, "../mooneye-gb/tests/build/acceptance/rst_timing.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_serial_boot_sclk_align_dmgabcmgb_gb, "../mooneye-gb/tests/build/acceptance/serial/boot_sclk_align-dmgABCmgb.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_div_write_gb, "../mooneye-gb/tests/build/acceptance/timer/div_write.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_rapid_toggle_gb, "../mooneye-gb/tests/build/acceptance/timer/rapid_toggle.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim00_gb, "../mooneye-gb/tests/build/acceptance/timer/tim00.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim00_div_trigger_gb, "../mooneye-gb/tests/build/acceptance/timer/tim00_div_trigger.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim01_gb, "../mooneye-gb/tests/build/acceptance/timer/tim01.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim01_div_trigger_gb, "../mooneye-gb/tests/build/acceptance/timer/tim01_div_trigger.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim10_gb, "../mooneye-gb/tests/build/acceptance/timer/tim10.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim10_div_trigger_gb, "../mooneye-gb/tests/build/acceptance/timer/tim10_div_trigger.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim11_gb, "../mooneye-gb/tests/build/acceptance/timer/tim11.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tim11_div_trigger_gb, "../mooneye-gb/tests/build/acceptance/timer/tim11_div_trigger.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tima_reload_gb, "../mooneye-gb/tests/build/acceptance/timer/tima_reload.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tima_write_reloading_gb, "../mooneye-gb/tests/build/acceptance/timer/tima_write_reloading.gb");
    // mooneye_test!(mooneye_gb_tests_build_acceptance_timer_tma_write_reloading_gb, "../mooneye-gb/tests/build/acceptance/timer/tma_write_reloading.gb");
    // mooneye_test!(
    //     mooneye_gb_tests_build_emulator_only_mbc1_bits_ram_en_gb,
    //     "../mooneye-gb/tests/build/emulator-only/mbc1/bits_ram_en.gb"
    // );
    //mooneye_test!(mooneye_gb_tests_build_emulator_only_mbc1_multicart_rom_8mb_gb, "../mooneye-gb/tests/build/emulator-only/mbc1/multicart_rom_8Mb.gb");
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_ram_256kb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/ram_256kb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_ram_64kb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/ram_64kb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_rom_16mb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/rom_16Mb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_rom_1mb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/rom_1Mb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_rom_2mb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/rom_2Mb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_rom_4mb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/rom_4Mb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_rom_512kb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/rom_512kb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_rom_8mb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/rom_8Mb.gb"
    );
    // mooneye_test!(mooneye_gb_tests_build_madness_mgb_oam_dma_halt_sprites_gb, "../mooneye-gb/tests/build/madness/mgb_oam_dma_halt_sprites.gb");
    // mooneye_test!(mooneye_gb_tests_build_manual_only_sprite_priority_gb, "../mooneye-gb/tests/build/manual-only/sprite_priority.gb");
    // mooneye_test!(mooneye_gb_tests_build_misc_bits_unused_hwio_c_gb, "../mooneye-gb/tests/build/misc/bits/unused_hwio-C.gb");
    // mooneye_test!(mooneye_gb_tests_build_misc_boot_hwio_c_gb, "../mooneye-gb/tests/build/misc/boot_hwio-C.gb");
    // mooneye_test!(mooneye_gb_tests_build_misc_boot_regs_a_gb, "../mooneye-gb/tests/build/misc/boot_regs-A.gb");
    // mooneye_test!(mooneye_gb_tests_build_misc_boot_regs_cgb_gb, "../mooneye-gb/tests/build/misc/boot_regs-cgb.gb");
    // mooneye_test!(mooneye_gb_tests_build_misc_ppu_vblank_stat_intr_c_gb, "../mooneye-gb/tests/build/misc/ppu/vblank_stat_intr-C.gb");
    // mooneye_test!(mooneye_gb_tests_build_utils_bootrom_dumper_gb, "../mooneye-gb/tests/build/utils/bootrom_dumper.gb");
    // mooneye_test!(mooneye_gb_tests_build_utils_dump_boot_hwio_gb, "../mooneye-gb/tests/build/utils/dump_boot_hwio.gb");
    blarg_test!(
        blarg_cpu_instrs_01_special_gb,
        "../blarg/roms/cpu_instrs/01-special.gb",
        passed!("01-special")
    );
    blarg_test!(
        blarg_cpu_instrs_02_interrupts_gb,
        "../blarg/roms/cpu_instrs/02-interrupts.gb",
        passed!("02-interrupts")
    );
    blarg_test!(
        blarg_cpu_instrs_03_op_sp_hl_gb,
        "../blarg/roms/cpu_instrs/03-op sp,hl.gb",
        passed!("03-op sp,hl")
    );
    blarg_test!(
        blarg_cpu_instrs_04_op_r_imm_gb,
        "../blarg/roms/cpu_instrs/04-op r,imm.gb",
        passed!("04-op r,imm")
    );
    blarg_test!(
        blarg_cpu_instrs_05_op_rp_gb,
        "../blarg/roms/cpu_instrs/05-op rp.gb",
        passed!("05-op rp")
    );
    blarg_test!(
        blarg_cpu_instrs_06_ld_r_r_gb,
        "../blarg/roms/cpu_instrs/06-ld r,r.gb",
        passed!("06-ld r,r")
    );
    blarg_test!(
        blarg_cpu_instrs_07_jr_jp_call_ret_rst_gb,
        "../blarg/roms/cpu_instrs/07-jr,jp,call,ret,rst.gb",
        passed!("07-jr,jp,call,ret,rs\nt")
    );
    blarg_test!(
        blarg_cpu_instrs_08_misc_instrs_gb,
        "../blarg/roms/cpu_instrs/08-misc instrs.gb",
        passed!("08-misc instrs")
    );
    blarg_test!(
        blarg_cpu_instrs_09_op_r_r_gb,
        "../blarg/roms/cpu_instrs/09-op r,r.gb",
        passed!("09-op r,r")
    );
    blarg_test!(
        blarg_cpu_instrs_10_bit_ops_gb,
        "../blarg/roms/cpu_instrs/10-bit ops.gb",
        passed!("10-bit ops")
    );
    blarg_test!(
        blarg_cpu_instrs_11_op_a_hl_gb,
        "../blarg/roms/cpu_instrs/11-op a,(hl).gb",
        passed!("11-op a,(hl)")
    );
    blarg_test!(
        blarg_cpu_instr_timing,
        "../blarg/roms/instr_timing/instr_timing.gb",
        passed!("instr_timing")
    );
    blarg_test!(
        blarg_sound_01_registers,
        "../blarg/roms/dmg_sound/01-registers.gb",
        passed!("01-registers")
    );
    blarg_test!(
        blarg_sound_02_len_ctr,
        "../blarg/roms/dmg_sound/02-len ctr.gb",
        "02-len ctr\n\n0 1 2 3\nPassed"
    );
    blarg_test!(
        blarg_sound_03_trigger,
        "../blarg/roms/dmg_sound/03-trigger.gb",
        "03-trigger\n\n0 1 2 3\nPassed"
    );
    // blarg_test!(
    //     blarg_sound_04_sweep,
    //     "../blarg/roms/dmg_sound/04-sweep.gb",
    //     passed!("04-sweep")
    // );
    // blarg_test!(
    //     blarg_sound_05_sweep_details,
    //     "../blarg/roms/dmg_sound/05-sweep details.gb",
    //     passed!("05-sweep details")
    // );
    // blarg_test!(
    //     blarg_sound_06_overflow_on_trigger,
    //     "../blarg/roms/dmg_sound/06-overflow on trigger.gb",
    //     passed!("06-overflow on trigger")
    // );
    // blarg_test!(
    //     blarg_sound_07_len_sweep_period_sync,
    //     "../blarg/roms/dmg_sound/07-len sweep period sync.gb",
    //     passed!("07-len sweep period sync")
    // );
    // blarg_test!(
    //     blarg_sound_08_len_ctr_during_power,
    //     "../blarg/roms/dmg_sound/08-len ctr during power.gb",
    //     passed!("08-len ctr during power")
    // );
    // blarg_test!(
    //     blarg_sound_09_wave_read_while_on,
    //     "../blarg/roms/dmg_sound/09-wave read while on.gb",
    //     passed!("09-wave read while on")
    // );
    // blarg_test!(
    //     blarg_sound_10_wave_trigger_while_on,
    //     "../blarg/roms/dmg_sound/10-wave trigger while on.gb",
    //     passed!("10-wave trigger while on")
    // );
    // blarg_test!(
    //     blarg_sound_11_regs_after_power,
    //     "../blarg/roms/dmg_sound/11-regs after power.gb",
    //     passed!("11-regs after power")
    // );
    // blarg_test!(
    //     blarg_sound_12_wave_write_while_on,
    //     "../blarg/roms/dmg_sound/12-wave write while on.gb",
    //     passed!("12-wave write while on")
    // );
    blarg_test!(
        blarg_mem_timing_read,
        "../blarg/roms/mem_timing/01-read_timing.gb",
        passed!("01-read_timing")
    );
    blarg_test!(
        blarg_mem_timing_write,
        "../blarg/roms/mem_timing/02-write_timing.gb",
        passed!("02-write_timing")
    );
    blarg_test!(
        blarg_mem_timing_modify,
        "../blarg/roms/mem_timing/03-modify_timing.gb",
        passed!("03-modify_timing")
    );

    blarg_test!(
        blarg_mem_timing_2_read,
        "../blarg/roms/mem_timing-2/01-read_timing.gb",
        passed!("01-read_timing")
    );
    blarg_test!(
        blarg_mem_timing_2_write,
        "../blarg/roms/mem_timing-2/02-write_timing.gb",
        passed!("02-write_timing")
    );
    blarg_test!(
        blarg_mem_timing_3_modify,
        "../blarg/roms/mem_timing-2/03-modify_timing.gb",
        passed!("03-modify_timing")
    );
}
