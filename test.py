#!/usr/bin/env python
from bs4 import BeautifulSoup
from collections import defaultdict, namedtuple
from bases import Bases
import re

gens = {}

Gen = namedtuple("Gen", "op op_name name size cycles cycles_false fmt init elems")

def read_cell(opcode, c):
    if re.match("\xa0+", c):
        g = Gen(opcode, "INVALID", "INVALID", 1, 1, None, ["opcode"], [], [])
    else:
        mnemonic, l2, l3 = c.split("<br/>")
        m = re.match("([0-9]+)\xa0+([0-9]+)(?:/([0-9]+))?", l2)
        bytes, cycles, cycles_false = (int(m.group(1)), int(m.group(2)), int(m.group(3)) if m.group(3) else None)
        if cycles_false:
            (cycles, cycles_false) = (cycles_false, cycles)

        (z, n, h, c) = l3.split(" ")
        generic = mnemonic
        generic = re.sub("HL\+", "HLP", generic)
        generic = re.sub("\+", " ", generic)
        clean_mnem = generic = re.sub("HL\-", "HLS", generic)
        generic = re.sub(r"(SET|BIT|RES) [0-9]+", r"\1 l8", generic)
        generic = re.sub(r"(JP|JR|CALL|RET) ({})\b".format("|".join(["Z",  "NZ", "C", "NC"])), r"\1 COND", generic)
        generic = re.sub(r"\b({})\b".format("|".join(["AF","BC", "DE", "HL", "HLP", "HLS", "SP", "PC"])), "r16", generic)
        generic = re.sub(r"\b({})\b".format("|".join(["A", "F", "B", "C", "D", "E", "H", "L"])), "r8", generic)

        generic = re.sub(r"\b[0-9]+H\b", "LIT", generic)

        items = re.split("\s+|,", generic)
        name = items[0]
        new_items = []
        args = []
        fmt = []
        for i in items[1:]:
            match = re.match(r"\(([a-z0-9+-]+)\)", i)
            if match:
                fmt.append("({:?})")
                new_items.append("i" + match.group(1))
            else:
                fmt.append("{:?}")
                new_items.append(i)

        items = re.split("\s+|,", clean_mnem)[1:]
        init = []
        elems = []
        for i in items:
            i = re.sub(r"\(|\)", "", i)
            if re.match(r"(JP|JR|CALL|RET)", name) and re.match("|".join(["Z",  "NZ", "C", "NC"]), i):
                init.append("Cond::{}".format(i))
                elems.append("Cond")
            elif re.match("|".join(["AF","BC", "DE", "HL", "HLP", "HLS", "SP", "PC"]), i):
                init.append("Reg16::{}".format(i))
                elems.append("Reg16")
            elif re.match("|".join(["A", "F", "B", "C", "D", "E", "H", "L"]), i):
                init.append("Reg8::{}".format(i))
                elems.append("Reg8")
            elif re.match("[0-9]+H", i):
                init.append("0x{}".format(i[:-1]))
                elems.append("u8")
            elif re.match("[0-9]+", i):
                init.append(str(i))
                elems.append("u8")
            elif re.match("[a-z][0-9]+", i):
                m = re.match("([a-z])([0-9]+)", i)
                conv = {
                    "d8" : "u8",
                    "d16" : "u16",
                    "a8" : "u8",
                    "a16" : "u16",
                    "r8" : "i8"
                }
                init.append("read_u{}(bytes)?{}".format(m.group(2), "" if conv[i][0] == "u" else
                                                        " as {}".format(conv[i])))
                elems.append(conv[i])
            else:
                print i
                assert(False)

        g = Gen(
            opcode,
            name,
            "_".join([name] + new_items),
            bytes,
            cycles,
            cycles_false,
            name + (" " if len(fmt) > 0 else "") + ",".join(fmt),
            init,
            elems
        )
    gens[opcode] = g


soup = BeautifulSoup(open("gameboy_opcodes.html").read(), 'html.parser')

tables = soup.find_all('table')
base = tables[0]
extend = tables[1]


def do_table(table, prefix = 0):
    for (r_id, r) in enumerate(table.find_all('tr')[1:]):
        for (d_id, d) in enumerate(r.find_all('td')[1:]):
            read_cell((prefix << 8) | (r_id << 4) | d_id,  d.decode_contents())

do_table(base)
do_table(extend, prefix=0xCB)

seen = {}
display = ""
defs = ""
read = ""
data = ""

import itertools
for i in itertools.chain(range(0,256), range(0xCB << 8 + 0, (0xCB << 8) + 0x100)):
    print "0x{:02}".format(i)

    args = ", ".join(["x{}".format(z) for (z, _) in enumerate(gens[i].elems)])
    read += "0x{:02x} => Instr::{}{},\n".format(
        i & 0xff,
        gens[i].name,
        "({})".format(", ".join(gens[i].init)) if len(args) else "",

    )
    data += "OpCode {{ mnemonic : \"{}\", size : {}, cycles: {}, cycles_branch: {} }},\n".format(
        gens[i].op_name,
        gens[i].size,
        gens[i].cycles / 4,
        "None" if gens[i].cycles_false is None else "Some({})".format(gens[i].cycles_false / 4)
    )

    if gens[i].name in seen:
        continue
    seen[gens[i].name] = True
    display += "Instr::{}{} => write!(f, \"{}\"{}{}),\n".format(
        gens[i].name,
        "({})".format(args) if len(args) > 0 else "",
        gens[i].fmt,
        ", " if len(args) else "",
        args,
    )
    defs += "{}{},\n".format(
        gens[i].name,
        "" if len(gens[i].elems) == 0 else "({})".format(", ".join(gens[i].elems))
    )

#print defs
#print read
#print display
print data
