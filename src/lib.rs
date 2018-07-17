#![feature(nll)]
#![feature(extern_prelude)]
#![cfg_attr(feature = "strict", deny(warnings))]
#![feature(reverse_bits)]
#[macro_use]
extern crate enum_primitive;
extern crate itertools;

macro_rules! flag_u8 {
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
mod instr;
mod mem;
mod mmu;
mod peripherals;
mod serial;
mod timer;

fn make_u16(h: u8, l: u8) -> u16 {
    ((h as u16) << 8) | (l as u16)
}

fn split_u16(r: u16) -> (u8, u8) {
    (((r & 0xff00) >> 8) as u8, (r & 0xff) as u8)
}

#[cfg(test)]
fn disasm_file(file: &str, filter_nops: bool) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Cursor;
    use std::io::{Read, Write};
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
    let mut filter = move |i: &instr::Instr| match i {
        instr::Instr::NOP => !filter_nops,
        _ => true,
    };

    for r in regions.iter() {
        let taken = f.take(r.1);
        let mut buf = Cursor::new(taken);
        writeln!(dst, "{}:", r.2)?;
        instr::disasm(r.0, buf.get_mut(), &mut dst, &mut filter).unwrap();
        f = buf.into_inner().into_inner();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    macro_rules! blarg_test {
        ($name:tt, $path:expr, $test:expr) => {
            #[test]
            fn $name() {
                use cart::Cart;
                let mut buf = ::std::io::BufWriter::new(Vec::new());
                {
                    let mut gb = ::gb::GB::new(
                        Cart::new(include_bytes!($path).to_vec()),
                        Some(&mut buf),
                        false,
                    );
                    gb.step_timeout::<(u8, u8, u8, u8), (i32, i32)>(30 * 1000000, &mut None);
                }
                assert_eq!(
                    ::std::str::from_utf8(&buf.into_inner().unwrap()).unwrap(),
                    concat!("Test: ", $test, "\n\n\nPassed\n")
                );
            }
        };
    }

    macro_rules! mooneye_test {
        ($name:tt, $path:expr) => {
            #[test]
            fn $name() {
                use cart::Cart;
                use cpu::Reg8;
                use cpu::RegType;

                let mut buf = ::std::io::BufWriter::new(Vec::new());
                let (finished, reg) = {
                    let mut gb = ::gb::GB::new(
                        Cart::new(include_bytes!($path).to_vec()),
                        Some(&mut buf),
                        false,
                    );
                    gb.magic_breakpoint();

                    (
                        gb.step_timeout::<(u8, u8, u8, u8), (i32, i32)>(30 * 1000000, &mut None),
                        gb.get_reg(),
                    )
                };
                let buf = buf.into_inner().unwrap();
                let output = ::std::str::from_utf8(&buf).unwrap();
                println!("{}", output);
                assert_eq!(output, "TEST OK");
                assert_eq!(finished, ::gb::GBReason::Dead);
                assert_eq!(reg.read(Reg8::B), 3);
                assert_eq!(reg.read(Reg8::C), 5);
                assert_eq!(reg.read(Reg8::D), 8);
                assert_eq!(reg.read(Reg8::E), 13);
                assert_eq!(reg.read(Reg8::H), 21);
                assert_eq!(reg.read(Reg8::L), 34);
            }
        };
    }
    #[test]
    fn it_works() {
        let s = Vec::new();
        assert_eq!(
            ::instr::Instr::disasm(&mut [0u8].as_ref()).unwrap(),
            (0, ::instr::Instr::NOP)
        );
        let mut b = ::std::io::Cursor::new(s);
        ::instr::disasm(0, &mut [0u8, 0u8].as_ref(), &mut b, &|_| true).unwrap();
        assert_eq!(
            String::from_utf8(b.into_inner()).unwrap(),
            "0x0000: 00       NOP\n0x0001: 00       NOP\n"
        );
        //::disasm_file("cpu_instrs/cpu_instrs.gb", true);
        ::disasm_file("blarg/cpu_instrs/10-bit_ops.gb", true).unwrap();
        // let mut mem = ::MMU::::new();
        // mem.dump();
    }
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
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_bits_ram_en_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/bits_ram_en.gb"
    );
    //mooneye_test!(mooneye_gb_tests_build_emulator_only_mbc1_multicart_rom_8mb_gb, "../mooneye-gb/tests/build/emulator-only/mbc1/multicart_rom_8Mb.gb");
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_ram_256kb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/ram_256Kb.gb"
    );
    mooneye_test!(
        mooneye_gb_tests_build_emulator_only_mbc1_ram_64kb_gb,
        "../mooneye-gb/tests/build/emulator-only/mbc1/ram_64Kb.gb"
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
        "../mooneye-gb/tests/build/emulator-only/mbc1/rom_512Kb.gb"
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
        "../blarg/cpu_instrs/01-special.gb",
        "01-special"
    );
    blarg_test!(
        blarg_cpu_instrs_02_interrupts_gb,
        "../blarg/cpu_instrs/02-interrupts.gb",
        "02-interrupts"
    );
    blarg_test!(
        blarg_cpu_instrs_03_op_sp_hl_gb,
        "../blarg/cpu_instrs/03-op_sp,hl.gb",
        "03-op_sp,hl"
    );
    blarg_test!(
        blarg_cpu_instrs_04_op_r_imm_gb,
        "../blarg/cpu_instrs/04-op_r,imm.gb",
        "04-op_r,imm"
    );
    blarg_test!(
        blarg_cpu_instrs_05_op_rp_gb,
        "../blarg/cpu_instrs/05-op_rp.gb",
        "05-op_rp"
    );
    blarg_test!(
        blarg_cpu_instrs_06_ld_r_r_gb,
        "../blarg/cpu_instrs/06-ld_r,r.gb",
        "06-ld_r,r"
    );
    blarg_test!(
        blarg_cpu_instrs_07_jr_jp_call_ret_rst_gb,
        "../blarg/cpu_instrs/07-jr,jp,call,ret,rst.gb",
        "07-jr,jp,call,ret,rst"
    );
    blarg_test!(
        blarg_cpu_instrs_08_misc_instrs_gb,
        "../blarg/cpu_instrs/08-misc_instrs.gb",
        "08-misc_instrs"
    );
    blarg_test!(
        blarg_cpu_instrs_09_op_r_r_gb,
        "../blarg/cpu_instrs/09-op_r,r.gb",
        "09-op_r,r"
    );
    blarg_test!(
        blarg_cpu_instrs_10_bit_ops_gb,
        "../blarg/cpu_instrs/10-bit_ops.gb",
        "10-bit_ops"
    );
    blarg_test!(
        blarg_cpu_instrs_11_op_a_hl_gb,
        "../blarg/cpu_instrs/11-op_a,(hl).gb",
        "11-op_a,(hl)"
    );
    blarg_test!(
        blarg_cpu_instr_timing,
        "../blarg/instr_timing/instr_timing.gb",
        "instr_timing"
    );
}
