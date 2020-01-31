use clap::{App, Arg};
use gb::instr::{Addr, Disasm, Instr, NameAddressFn};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;

use intervaltree::IntervalTree;
#[derive(Debug, Clone, PartialEq)]
pub struct WlaSym {
    labels: HashMap<(u8, u16), String>,
    memmap: HashMap<u8, u16>,
}

impl WlaSym {
    fn merge(&mut self, other: WlaSym) {
        self.labels.extend(other.labels.into_iter());
        self.memmap.extend(other.memmap.into_iter());
    }

    fn mem_map(&self, bank: u8) -> u16 {
        let offset = self.memmap.get(&bank);
        match bank {
            0 => *offset.unwrap_or(&0x0000),
            _ => *offset.unwrap_or(&0x4000),
        }
    }
}
impl Default for WlaSym {
    fn default() -> WlaSym {
        WlaSym {
            labels: HashMap::new(),
            memmap: HashMap::new(),
        }
    }
}

mod wla_parse {
    #[derive(Clone, PartialEq, Debug)]
    struct Label {
        bank: u8,
        addr: u16,
        value: String,
    }
    #[derive(Clone, PartialEq, Debug)]
    struct MemMap {
        bank: u8,
        start: u16,
    }
    enum WlaSection {
        Labels(Vec<Label>),
        MemMap(Vec<MemMap>),
        Unknown,
    }

    use nom::branch::alt;
    use nom::bytes::complete::take_while_m_n;
    use nom::bytes::complete::{take_till, take_while};
    use nom::character::complete::char;
    use nom::combinator::{map, map_res, not, peek};
    use nom::multi::{many0, many_till};
    use nom::sequence::pair;
    use std::collections::HashMap;

    use super::WlaSym;

    use nom::IResult as NomResult;
    fn wla_label(input: &str) -> NomResult<&str, &str> {
        let (input, _) = char('[')(input)?;
        let (input, r) = take_till(|c| c as char == ']')(input)?;
        let (input, _) = char(']')(input)?;
        Ok((input, r))
    }

    fn from_hex_u8(input: &str) -> Result<u8, std::num::ParseIntError> {
        u8::from_str_radix(input, 16)
    }

    fn from_hex_u16(input: &str) -> Result<u16, std::num::ParseIntError> {
        u16::from_str_radix(input, 16)
    }
    fn is_hex_digit(c: char) -> bool {
        c.is_digit(16)
    }
    fn comment(input: &str) -> NomResult<&str, ()> {
        let (input, _) = take_while(|c| (c as char).is_whitespace())(input)?;
        let (input, _) = char(';')(input)?;
        let (input, _) = take_till(|c| c as char == '\n')(input)?;
        Ok((input, ()))
    }

    fn skip(input: &str) -> NomResult<&str, ()> {
        map(take_till(|c| c as char == '\n'), |_| ())(input)
    }
    fn empty(input: &str) -> NomResult<&str, ()> {
        map(take_while(|c: char| c.is_whitespace() && c != '\n'), |_| ())(input)
    }
    fn label(input: &str) -> NomResult<&str, Label> {
        let (input, bank) = map_res(take_while_m_n(2, 2, is_hex_digit), from_hex_u8)(input)?;
        let (input, _) = char(':')(input)?;
        let (input, addr) = map_res(take_while_m_n(4, 4, is_hex_digit), from_hex_u16)(input)?;
        let (input, _) = char(' ')(input)?;
        let (input, label) = take_till(|c| c as char == '\n')(input)?;
        Ok((
            input,
            Label {
                bank,
                addr,
                value: label.to_owned(),
            },
        ))
    }
    fn memmap(input: &str) -> NomResult<&str, MemMap> {
        let (input, bank) = map_res(take_while_m_n(2, 2, is_hex_digit), from_hex_u8)(input)?;
        let (input, _) = char(' ')(input)?;
        let (input, addr) = map_res(take_while_m_n(4, 4, is_hex_digit), from_hex_u16)(input)?;
        Ok((input, MemMap { bank, start: addr }))
    }
    fn newline(input: &str) -> NomResult<&str, ()> {
        map(char('\n'), |_| ())(input)
    }
    fn extra(input: &str) -> NomResult<&str, ()> {
        map(alt((comment, empty)), |_| ())(input)
    }
    fn parse_section(input: &str) -> NomResult<&str, WlaSection> {
        let (input, (_, read_label)) = many_till(pair(extra, newline), wla_label)(input)?;
        let (input, _) = newline(input)?;
        println!("Reading Section {}", read_label);
        let section = match read_label {
            "labels" => label_section,
            "memmap" => memmap_section,
            _ => unknown_section,
        };
        section(input)
    }

    fn unknown_section(input: &str) -> NomResult<&str, WlaSection> {
        map(
            many0(pair(pair(peek(not(wla_label)), skip), char('\n'))),
            |_| WlaSection::Unknown,
        )(input)
    }
    fn memmap_section(input: &str) -> NomResult<&str, WlaSection> {
        map(
            many0(pair(
                alt((map(memmap, |l| Some(l)), map(extra, |_| None))),
                newline,
            )),
            |s| {
                let s = s.into_iter().filter_map(|s| s.0).collect();
                WlaSection::MemMap(s)
            },
        )(input)
    }
    fn label_section(input: &str) -> NomResult<&str, WlaSection> {
        map(
            many0(pair(
                alt((map(label, |l| Some(l)), map(extra, |_| None))),
                newline,
            )),
            |s| {
                let s = s.into_iter().filter_map(|s| s.0).collect();
                WlaSection::Labels(s)
            },
        )(input)
    }
    pub fn parse(input: &str) -> NomResult<&str, WlaSym> {
        let mut labels = HashMap::new();
        let mut memmap = HashMap::new();
        let (input, sections) = many0(parse_section)(input)?;
        for s in sections {
            match s {
                WlaSection::Labels(items) => {
                    labels.extend(items.into_iter().map(|l| ((l.bank, l.addr), l.value)));
                }
                WlaSection::MemMap(items) => {
                    memmap.extend(items.into_iter().map(|l| (l.bank, l.start)));
                }
                _ => {}
            }
        }
        Ok((input, WlaSym { labels, memmap }))
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn wla_label() {
            assert_eq!(
                label(&"12:1234 some_label\n"[..]),
                Ok((
                    &"\n"[..],
                    Label {
                        bank: 0x12,
                        addr: 0x1234,
                        value: String::from("some_label")
                    }
                ))
            );
        }
        #[test]
        fn wla_comment() {
            assert_eq!(
                comment(&"; dsfhjlashjdfjakdshfkjahdsf\n"[..]),
                Ok((&"\n"[..], ()))
            );
        }

        #[test]
        fn wla_labels() {
            assert_eq!(
                parse(
                    &concat!(
                        "[labels]\n\n",
                        ";extraneous comment\n",
                        "12:1234 some_label\n",
                        "[other]\n",
                        "jkldsfj\n",
                        "jlaksdf\n",
                        "[memmap]\n",
                        "00 0000\n",
                        "01 4000\n",
                        "02 8000\n",
                        "[labels]\n",
                        "45:5467 more_label\n"
                    )[..]
                ),
                Ok((
                    &""[..],
                    WlaSym {
                        memmap: vec![(0, 0), (1, 0x4000), (2, 0x8000)].into_iter().collect(),
                        labels: vec![
                            ((0x12, 0x1234), String::from("some_label")),
                            ((0x45, 0x5467), String::from("more_label")),
                        ]
                        .into_iter()
                        .collect()
                    }
                ))
            );
        }
    }
}
#[derive(Debug, Clone, Copy)]
enum Section {
    Code,
    Nop,
    //String,
    Data,
}

struct Makefile<T> {
    inner: T,
}

impl<T> Makefile<T>
where
    T: std::io::Write,
{
    fn new(inner: T) -> Self {
        Makefile { inner }
    }

    fn rule(
        &mut self,
        targets: &[String],
        sources: &[String],
        cmds: &[String],
    ) -> std::io::Result<()> {
        writeln!(self.inner, "{} : {}", targets.join(" "), sources.join(" "))?;
        for c in cmds {
            writeln!(self.inner, "\t{}", c)?;
        }
        Ok(())
    }
}

const BANK_SIZE: u64 = 0x4000;

fn remap_address(target: u16, cur_bank: u8, bank_map: &IntervalTree<u16, u8>) -> Option<Vec<u64>> {
    //println!("Target {:x}", t);
    let v: Vec<_> = bank_map
        .query_point(target.into())
        .flat_map(|b| {
            if cur_bank == 0 || cur_bank == b.value {
                Some(b.value as u64 * BANK_SIZE + target as u64 - b.range.start as u64)
            } else {
                None
            }
        })
        .collect();
    if v.len() > 0 {
        Some(v)
    } else {
        None
    }
}

fn dump_bank<S>(
    mut out: std::fs::File,
    rom_targets: &HashMap<u64, String>,
    rom: &mut std::fs::File,
    sections: S,
    remap: &NameAddressFn,
) -> std::io::Result<()>
where
    S: Iterator<Item = (std::ops::Range<u64>, std::ops::Range<u16>, Section)>,
{
    for (f, r, typ) in sections {
        assert_eq!(rom.seek(SeekFrom::Current(0)).unwrap(), f.start);
        println!("{:x?} {:?}", r, typ);
        //writeln!(out, "; {:x?} {:?}", r, typ);
        //assert_eq!(rom.seek(SeekFrom::Current(0)).unwrap(), r.start);
        //assert_eq!(rom.seek(SeekFrom::Current(0)).unwrap(), r.start);
        if let Some(n) = rom_targets.get(&f.start) {
            writeln!(out, "{}:", n)?;
        }
        match typ {
            Section::Data => {
                let len = f.end - f.start;
                let mut disbuf = Vec::with_capacity(len as usize);
                disbuf.resize(len as usize, 0);
                rom.read_exact(&mut disbuf)?;
                for chunk in disbuf.chunks(16) {
                    write!(out, "\t.db ")?;
                    for b in chunk {
                        write!(out, "${:02x} ", b)?;
                    }
                    writeln!(out)?;
                }
                writeln!(out)?;
            }
            Section::Code => {
                let mut d = Disasm::new();
                use std::convert::TryFrom;
                for a in
                    r.clone().chain(std::iter::repeat(r.end - 1).take(
                        usize::try_from((f.end - f.start) - u64::from(r.end - r.start)).unwrap(),
                    ))
                {
                    let mut b = [0u8; 1];
                    rom.read_exact(&mut b)?;
                    if let Some(i) = d.feed_byte(b[0]) {
                        use gb::instr::FormatCode;
                        let len = u16::try_from(d.len()).unwrap();
                        writeln!(
                            out,
                            "\t{instr} ; Addr: {addr:08x}",
                            addr = a + 1 - len,
                            instr = i.to_code(a..(a + len), remap)
                        )?;
                        d.reset();
                    }
                }
                if d.len() > 0 {
                    write!(out, "\t.db ")?;
                    for b in d.to_bytes() {
                        write!(out, "${:02x} ", b)?;
                    }
                    writeln!(out)?;
                }
            }
            // Section::String => {
            //     unimplemented!()
            // },
            Section::Nop => {
                let len = f.end - f.start;
                rom.seek(SeekFrom::Current(len as i64))?;
                writeln!(out, "\t.DSB {}, $00", len)?;
            }
        }
        assert_eq!(rom.seek(SeekFrom::Current(0)).unwrap(), f.end);
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let matches = App::new("ROM Disassembler")
        .version("0.0.1")
        .author("Mitch Souders. <mitch.souders@gmail.com>")
        .about("Disassembles GB Roms")
        .arg(
            Arg::with_name("SYMBOLS_ROM")
                .short("s")
                .multiple(true)
                .number_of_values(1)
                .value_name("FILE")
                .help("Maybe helps find existing symbols in your file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ROM")
                .help("Sets the rom file to use")
                .required(true)
                .index(1),
        )
        .get_matches();

    dis_rom(
        matches.value_of("ROM").unwrap(),
        matches.values_of("SYMBOLS_ROM").map(|v| v.collect()),
        &std::path::PathBuf::from("dis"),
    )
}

fn dis_rom(
    rom_name: &str,
    symbols: Option<Vec<&str>>,
    dis_dir: &std::path::PathBuf,
) -> std::io::Result<()> {
    let mut entry_points = HashSet::new();
    let rom_size = std::fs::metadata(rom_name)?.len();
    let mut rom = std::fs::File::open(rom_name)?;
    let num_banks = rom_size / BANK_SIZE;

    let mut master_syms = WlaSym::default();
    if let Some(symbols) = symbols {
        for symbol in symbols {
            let sym_file = std::fs::read_to_string(symbol).unwrap();
            let (remaining, syms) = wla_parse::parse(&sym_file).unwrap();
            assert_eq!(remaining, "");
            master_syms.merge(syms);
        }
    }

    let bank_map = IntervalTree::from_iter((0..num_banks as u8).map(|b| {
        let start = master_syms.mem_map(b);
        (start..(start.saturating_add(BANK_SIZE as u16)), b)
    }));

    use std::borrow::Cow;
    let entrys = master_syms
        .labels
        .iter()
        .flat_map(|((b, addr), label)| {
            remap_address(*addr, *b, &bank_map).map(|addrs| {
                addrs
                    .into_iter()
                    .map(move |addr| (Cow::Owned(label.to_owned()), addr))
            })
        })
        .flatten()
        .chain(
            [
                ("main", 0x0100),
                ("rst_0", 0x0000),
                ("rst_8", 0x0008),
                ("rst_10", 0x0010),
                ("rst_18", 0x0018),
                ("rst_20", 0x0020),
                ("rst_28", 0x0028),
                ("rst_30", 0x0030),
                ("rst_38", 0x0038),
                ("VBlankInterrupt", 0x0040),
                ("LCDCInterrupt", 0x0048),
                ("TimerOverflowInterrupt", 0x0050),
                ("SerialTransferCompleteInterrupt", 0x0058),
                ("JoypadTransitionInterrupt", 0x0060),
            ]
            .iter()
            .map(|(s, a)| (Cow::Borrowed(*s), *a as u64)),
        )
        .collect::<Vec<_>>();

    let mut data: Vec<(Cow<str>, Range<u64>)> = vec![
        ("HeaderLogo", 0x104..0x134),
        ("CGBFlag", 0x143..0x144),
        ("SGBFlag", 0x146..0x147),
        ("CartridgeType", 0x147..0x148),
        ("ROMSize", 0x148..0x149),
        ("RAMSize", 0x149..0x14A),
        ("DestinationCode", 0x14A..0x14B),
        ("MaskRomVersion", 0x14C..0x14D),
        ("HeaderChecksum", 0x14D..0x14E),
        ("GlobalChecksum", 0x14E..0x150),
    ]
    .into_iter()
    .map(|(n, r)| (Cow::Borrowed(n), r))
    .collect::<Vec<_>>();

    rom.seek(SeekFrom::Start(0x143))?;
    let mut b = [0];
    rom.read_exact(&mut b).unwrap();
    let cgb = (b[0] & 0x80) != 0;
    if cgb {
        data.extend(
            vec![
                ("Title", 0x134..0x13f),
                ("ManufacterCode", 0x13f..0x143),
                ("LicenseeCode", 0x144..0x145),
            ]
            .into_iter()
            .map(|(n, r)| (Cow::Borrowed(n), r)),
        );
    } else {
        data.extend(
            vec![("Title", 0x134..0x144), ("OldLicenseeCode", 0x14B..0x14C)]
                .into_iter()
                .into_iter()
                .map(|(n, r)| (Cow::Borrowed(n), r)),
        );
    }
    enum WorkListItem {
        Code,
        Unknown,
    };
    let mut worklist: Vec<(u64, _)> = entrys
        .iter()
        .map(|(_n, a)| (*a, WorkListItem::Code))
        .collect();

    let mut ranges: Vec<_> = data
        .iter()
        .cloned()
        .map(|(_, r)| (r, Section::Data))
        .collect();

    let mut unique_names = HashSet::new();

    let mut known_targets = entrys
        .into_iter()
        .map(|(n, v)| (v, n))
        .chain(data.iter().cloned().map(|(n, v)| (v.start, n)))
        .map(|(s, n)| {
            let mut i = 0;
            let mut new = n.clone();
            let named = loop {
                if let Some(_) = unique_names.get(&new) {
                    new = Cow::Owned(format!("{}_InternalLabel{}", n.as_ref(), i));
                    i += 1;
                } else {
                    unique_names.insert(new.clone());
                    break new;
                }
            };
            (s, named)
        })
        .map(|(v, n)| (v, Some(n)))
        .collect::<HashMap<_, _>>();

    rom.seek(SeekFrom::Start(0))?;

    while let Some((region_pos, _region_type)) = worklist.pop() {
        let mut pos = region_pos;
        let mut disasm = Disasm::new();
        if let Some(_) = entry_points.get(&pos) {
            continue;
        } else {
            entry_points.insert(pos);
        }
        rom.seek(SeekFrom::Start(pos))?;
        let mut start_region = pos % BANK_SIZE == 0;
        //println!("Entry Point: 0x{:x}", pos);
        fn check_nop(
            ranges: &mut Vec<(Range<u64>, Section)>,
            instr_pos: u64,
            nop_start: &mut Option<u64>,
        ) -> bool {
            if let Some(s) = nop_start {
                if instr_pos - *s > 10 {
                    ranges.push((*s..instr_pos, Section::Nop));
                }
                *nop_start = None;
                true
            } else {
                false
            }
        }

        let mut nop_start = None;
        'region: loop {
            let instr_start_pos = pos;
            let instr = 'instr: loop {
                if !start_region && pos % BANK_SIZE == 0 {
                    worklist.push((pos, WorkListItem::Code));
                    break 'region; /* hit end of region */
                } else if data.iter().any(|(_, r)| r.contains(&pos)) {
                    break 'region; /* header data */
                }
                start_region = false;
                let mut b = [0];
                match rom.read_exact(&mut b) {
                    Err(e) => {
                        if let std::io::ErrorKind::UnexpectedEof = e.kind() {
                            break 'region;
                        } else {
                            Err(e)?;
                        }
                    }
                    Ok(_) => {}
                }
                pos += 1;
                match disasm.feed_byte(b[0]) {
                    None => {}
                    Some(x) => break 'instr x,
                }
            };
            match instr {
                Instr::NOP => {
                    if nop_start.is_none() {
                        nop_start = Some(instr_start_pos);
                    }
                }
                _ => {
                    check_nop(&mut ranges, instr_start_pos, &mut nop_start);
                }
            }
            //println!("{}", instr);
            let cur_bank = pos / BANK_SIZE;
            let target = match instr {
                Instr::INVALID(_) => {
                    worklist.push((pos, WorkListItem::Unknown));
                    //pos -= 1;
                    break 'region;
                }
                Instr::JP_r16(_) => {
                    println!("Warning: Indirect Jump at 0x{:x}.", instr_start_pos);
                    None
                }
                Instr::JR_COND_r8(_, a) | Instr::JR_r8(a) => {
                    assert_eq!(instr_start_pos + 2, pos);
                    if cur_bank == 0 {
                        Some((a + Addr::from(pos as u16)).into())
                    } else {
                        let start = master_syms.mem_map(cur_bank as u8);
                        Some((a + Addr::from(start + pos as u16 % BANK_SIZE as u16)).into())
                    }
                }
                Instr::JP_a16(a) | Instr::CALL_a16(a) | Instr::CALL_COND_a16(_, a) => {
                    Some(u16::from(a))
                }
                _ => None,
            };

            if let Some(t) = target {
                match remap_address(t, cur_bank as u8, &bank_map) {
                    Some(addrs) => {
                        for a in addrs {
                            worklist.push((a, WorkListItem::Code));
                            known_targets.entry(a).or_insert(None);
                        }
                    }
                    None => println!(
                        "Target address 0x{:x}, could be anything (from 0x{:x})",
                        t, pos
                    ),
                }

                // match t {
                //     0x4000..=0x7fff => {
                //         if cur_bank == 0 {
                //             // This could be any bank target
                //             for b in 1..num_banks {
                //                 let addr = b * BANK_SIZE + t as u64 % BANK_SIZE;
                //                 worklist.push(addr);
                //                 known_targets.entry(addr).or_insert(None);
                //             }
                //         } else {
                //             let addr = BANK_SIZE * cur_bank + t as u64 - BANK_SIZE;
                //             worklist.push(addr);
                //             known_targets.entry(addr).or_insert(None);
                //         }
                //     }
                //     0x0150..=0x3fff | 0x100..=0x103 | 0x0000..=0x00ff => {
                //         known_targets.entry(t as u64).or_insert(None);
                //         worklist.push(t as u64);
                //     }
                //     _ => {
                //         println!("Target address 0x{:x}, could be anything", t);
                //     }
                // }
            }
            //println!("0x{:x}: {:?}", start_pos, instr);
            match instr {
                Instr::RET
                | Instr::RETI
                | Instr::JR_r8(_)
                | Instr::JP_r16(_)
                | Instr::JP_a16(_) => {
                    worklist.push((pos, WorkListItem::Unknown));
                    break 'region;
                }
                _ => {}
            }
            disasm.reset();
        }
        check_nop(&mut ranges, pos, &mut nop_start);
        if pos > region_pos {
            ranges.push((
                region_pos..pos,
                Section::Code,
                // match region_type {
                //     WorkListItem::Code => Section::Code,
                //     WorkListItem::Unknown => Section::Data,
                // }
            ));
        }
    }

    //rom.seek(SeekFrom::Start(0));

    // let mut deduped :Vec<_> =
    //     rom.bytes().enumerate()
    //     .filter_map(|(pos, r)| {
    //         if let Ok(v) = r {
    //             Some((pos as u64, v))
    //         } else {
    //             None
    //         }
    //     })
    //     //.map(|(pos, b)| b)
    //     .collect();
    // deduped.dedup_by_key(|(_, b)| b.clone());

    use std::iter::FromIterator;
    let tree = IntervalTree::from_iter(ranges.into_iter());

    let rom_targets: HashMap<u64, String> = known_targets
        .iter()
        .map(|(k, v)| {
            let name = if let Some(v) = v {
                v.to_string()
            } else {
                let b = k / BANK_SIZE;
                let b_addr = k % BANK_SIZE;
                format!("Label_Bank{}_{:04x}", b, b_addr)
            };
            (*k, name)
        })
        .collect();
    //println!("ROM Targets: {:x?}", rom_targets);
    fn byte_regions(
        file: std::ops::Range<u64>,
        map: Range<u16>,
        tree: &IntervalTree<u64, Section>,
    ) -> impl Iterator<Item = (std::ops::Range<u64>, std::ops::Range<u16>, Section)> + '_ {
        let mut ranges: Vec<_> = tree.query(file.clone()).collect();
        ranges.sort_by_key(|e| (e.range.start, e.range.end));
        let mut points = ranges
            .iter()
            .map(|r| r.range.clone())
            .chain(std::iter::once(file.clone()))
            .flat_map(|r| {
                let started = file.contains(&r.start);
                let finished = file.contains(&(r.end));
                let mut v = Vec::new();
                if started {
                    v.push(r.start);
                }
                if finished {
                    v.push(r.end);
                }
                v
            })
            .chain(std::iter::once(file.end.clone()))
            .collect::<Vec<_>>();

        points.sort();
        points.dedup();

        use std::convert::TryFrom;
        let len = points.len();
        let points2 = points.clone();
        points
            .into_iter()
            .take(len - 1)
            .zip(points2.into_iter().skip(1))
            .map(move |(s, f)| {
                let r = s..f;
                let typ = tree
                    .query(r.clone())
                    .map(|r| r.value)
                    .fold(None, |acc, v| {
                        match (acc, v) {
                            (None, v) => Some(v),
                            //(_, Section::Code) => Some(Section::Code),
                            (_, Section::Nop) => Some(Section::Nop),
                            (v, _) => v,
                        }
                    })
                    .unwrap_or(Section::Data);
                let mapped_start = u16::try_from(r.start - file.start).unwrap() + map.start;
                //println!("{:x?} {:x?}", r, map);
                let map = mapped_start
                    ..(mapped_start.saturating_add(u16::try_from(r.end - r.start).unwrap()));

                (r, map, typ)
            })
    }

    rom.seek(SeekFrom::Start(0))?;

    if !dis_dir.exists() {
        std::fs::create_dir(&dis_dir).unwrap();
    }
    let mut makefile =
        Makefile::new(std::fs::File::create(dis_dir.clone().join("Makefile")).unwrap());
    let header_name = dis_dir.clone().join("header.s");
    let link_name = dis_dir.clone().join("rom.link");
    let mut header = std::fs::File::create(&header_name).unwrap();
    let mut linkfile = std::fs::File::create(&link_name).unwrap();
    writeln!(linkfile, "[objects]")?;
    writeln!(linkfile, "rom.o")?;

    write!(
        header,
        concat!(".ROMBANKMAP\n", "BANKSTOTAL {}\n",),
        num_banks,
    )?;
    for _ in 0..num_banks {
        writeln!(header, "BANKSIZE  ${size:04x}\nBANKS 1\n", size = BANK_SIZE)?;
    }
    writeln!(header, ".ENDRO")?;
    write!(header, concat!(".MEMORYMAP\n", "DEFAULTSLOT 0\n",),)?;
    for b in 0..num_banks {
        let start = master_syms.mem_map(b as u8);
        writeln!(
            header,
            "SLOT {} ${:04x} size ${size:04x} ",
            b,
            start,
            size = BANK_SIZE
        )?;
    }
    writeln!(header, ".ENDME")?;

    for b in 0..num_banks {
        let start = master_syms.mem_map(b as u8);
        let range = start..(start.saturating_add(BANK_SIZE as u16));
        let out = dis_dir.clone().join(format!("bank{:03}.s", b));
        let mut out_file = std::fs::File::create(out).unwrap();
        writeln!(out_file, ".BANK {} SLOT {}", b, b)?;
        writeln!(out_file, ".ORG 0")?;
        let file_range = (b * BANK_SIZE) as u64..((b + 1) * BANK_SIZE) as u64;
        let byte_regions = byte_regions(file_range.clone(), range, &tree);
        let remap = |addr| {
            remap_address(addr, b as u8, &bank_map).and_then(|v| {
                if v.len() == 1 {
                    rom_targets.get(&v[0]).map(|s| s.to_owned())
                } else {
                    None
                }
            })
        };

        rom.seek(SeekFrom::Start(file_range.start))?;
        dump_bank(out_file, &rom_targets, &mut rom, byte_regions, &remap)?;

        assert_eq!(rom.seek(SeekFrom::Current(0)).unwrap(), file_range.end);
    }

    let mut sources = Vec::new();
    sources.push(
        header_name
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
    );

    for b in 0..num_banks {
        let out = format!("bank{:03}.s", b);
        writeln!(header, ".include \"{}\"", out)?;
        sources.push(out);
    }
    makefile.rule(
        &["rom.gb".to_string()],
        &[
            link_name
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "rom.o".to_string(),
        ],
        &["wlalink $< $@".to_string()],
    )?;
    makefile.rule(
        &["rom.o".to_string()],
        sources.as_slice(),
        &["wla-gb -o $@ $<".to_string()],
    )?;

    //writeln!(header, ".rombanks {}", num_banks);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;
    #[test]
    fn dis_test() {
        let dir = TempDir::new("dis").unwrap();
        let rom_dir = std::path::PathBuf::from("../blarg/roms/cpu_instrs/");
        let rom = rom_dir.join("01-special.gb");
        let syms = rom_dir.join("01-special.sym");
        let default_sym = dir.path().join("default.sym");
        {
            let mut default = std::fs::File::create(&default_sym).unwrap();
            write!(default, "[memmap]\n01 c000\n").unwrap();
        }
        dis_rom(
            &rom.to_string_lossy(),
            Some(vec![
                &syms.to_string_lossy(),
                &default_sym.to_string_lossy(),
            ]),
            &std::path::PathBuf::from(dir.path()),
        )
        .unwrap();
        {
            let mut child = std::process::Command::new("make")
                .arg("rom.gb")
                .current_dir(dir.path())
                .spawn()
                .expect("Failed to execute");
            let result = child.wait().unwrap();
            assert!(result.success());
        }
        {
            let child = std::process::Command::new("md5sum")
                .arg(std::fs::canonicalize(&rom).unwrap())
                .arg(dir.path().join("rom.gb"))
                .current_dir(dir.path())
                .stdout(std::process::Stdio::piped())
                .spawn()
                .expect("Failed to execute");
            let output = child.wait_with_output().unwrap();
            println!(
                "md5sum output {}",
                std::str::from_utf8(&output.stdout).unwrap()
            );
            use std::io::BufRead;
            let mut lines = output.stdout.lines();
            let l1 = lines.next().unwrap().unwrap();
            let l2 = lines.next().unwrap().unwrap();
            let md5_len = 32;
            assert_eq!(l1[0..md5_len], l2[0..md5_len]);
        }

        dir.close().unwrap();
    }
}
