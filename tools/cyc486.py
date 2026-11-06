#!/usr/bin/env python3
# Estimates i486 clocks from callgrind profiles: each executed instruction weighted by the i486 datasheet's clock count.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Provenance: adapted from Fizz 1's DOS port, fizz-dos m5/bench/cyc486.py at commit 8d37fc7. Changes here: the weights are
# checked against the i486 datasheet (Intel, April 1989, tables 10.1 and 10.3) and follow its operand forms and assumptions;
# `rep` string instructions are charged their set-up for each start as well as their clocks for each element; conditional jumps
# are charged taken or not taken from the jump counts; and a profile can be read whole, less a shorter one, or one part for each
# frame.
#
# usage: python3 -I tools/cyc486.py PROFILE [--frames N] [--minus SHORT] [--top N]
#            one profile (or PROFILE less SHORT, so start-up cancels, as Fizz 1's port used it), divided by N frames
#        python3 -I tools/cyc486.py --parts LABELS PART... [--top N]
#            one part for each frame (`--dump-after`), named in order by the lines of the file LABELS, such as `indoor`
#        python3 -I tools/cyc486.py --self-test
#
# Record the profiles with `valgrind --tool=callgrind --dump-instr=yes --collect-jumps=yes`; tools/bench-draw.sh --486 does this
# for the DOS build's own code. Without `--collect-jumps` a conditional jump counts 2 clocks and a `rep` instruction its clocks
# for each element only, as in Fizz 1's port. The disassembly of each object named in a profile is read with objdump.
#
# What the weights are: the datasheet's cache-hit clock counts, with every access a hit, no wait states and nothing waiting on
# the bus; for a range, the datasheet's own average (x87) or a stated typical value (multiplies, bit scans). Its assumptions 4, 5
# and 8 are applied: one clock more when an address uses a register the instruction before it wrote, when it uses an index
# register, and when an instruction has both a displacement and an immediate. A division by the x87 is charged at 53-bit
# precision (73 less 11 clocks, the datasheet's note), because FIZZ2.EXE sets that control word. Not modelled: cache misses and
# line fills, the write buffer filling, bus and video memory wait states (a write to the VGA's memory costs far more than a
# clock), misaligned accesses, prefix decoding, and x87 work overlapping the integer instructions after it.
import collections
import re
import subprocess
import sys
import unittest

# --- Weights ---------------------------------------------------------------------------------------------------------------

# x87, by operand: m16, m32, m64 or m80 for a memory operand of that size, `r` for registers or none (table 10.3, cache hit;
# the average where the table gives a range).
X87 = {
    'fld': {'m32': 3, 'm64': 3, 'm80': 6, 'r': 4},
    'fild': {'m16': 14.5, 'm32': 11.5, 'm64': 16.8},
    'fst': {'m32': 7, 'm64': 8, 'r': 3},
    'fstp': {'m32': 7, 'm64': 8, 'm80': 6, 'r': 3},
    'fist': {'m16': 33.4, 'm32': 32.4},
    'fistp': {'m16': 33.4, 'm32': 33.4, 'm64': 33.4},
    'fbld': 75, 'fbstp': 175, 'fxch': 4,
    'fcom': 4, 'fcomp': 4, 'fcompp': 5, 'fucom': 4, 'fucomp': 4, 'fucompp': 5, 'ftst': 4, 'fxam': 8,
    'ficom': {'m16': 18, 'm32': 16.5}, 'ficomp': {'m16': 18, 'm32': 16.5},
    'fldz': 4, 'fld1': 4, 'fldpi': 8, 'fldl2t': 8, 'fldl2e': 8, 'fldlg2': 8, 'fldln2': 8,
    'fadd': 10, 'faddp': 10, 'fsub': 10, 'fsubp': 10, 'fsubr': 10, 'fsubrp': 10,
    'fmul': {'m32': 11, 'm64': 14, 'r': 16}, 'fmulp': 16,
    'fdiv': 62, 'fdivp': 62, 'fdivr': 62, 'fdivrp': 62,
    'fiadd': {'m16': 24, 'm32': 22.5}, 'fisub': {'m16': 24, 'm32': 22.5}, 'fisubr': {'m16': 24, 'm32': 22.5},
    'fimul': {'m16': 25, 'm32': 23.5}, 'fidiv': {'m16': 76, 'm32': 74.5}, 'fidivr': {'m16': 76, 'm32': 74.5},
    'fsqrt': 85.5, 'fscale': 31, 'fxtract': 19, 'fprem': 84, 'fprem1': 94.5, 'frndint': 29.1, 'fabs': 3, 'fchs': 6,
    'fcos': 313, 'fsin': 313, 'fsincos': 336, 'fptan': 244, 'fpatan': 289, 'f2xm1': 242, 'fyl2x': 311, 'fyl2xp1': 313,
    'finit': 17, 'fninit': 17, 'fstsw': 3, 'fnstsw': 3, 'fldcw': 4, 'fstcw': 3, 'fnstcw': 3, 'fclex': 7, 'fnclex': 7,
    'fincstp': 3, 'fdecstp': 3, 'ffree': 3, 'fnop': 3, 'fwait': 1, 'wait': 1,
}

# Integer instructions (table 10.1, cache hit), as (register form, memory form). For the arithmetic group the memory form is the
# memory destination; a memory source costs ALU_FROM_MEMORY.
INT = {
    'mov': (1, 1), 'movzx': (3, 3), 'movsx': (3, 3), 'lea': (1, 1), 'xchg': (3, 5), 'bswap': (1, 1),
    'add': (1, 3), 'adc': (1, 3), 'sub': (1, 3), 'sbb': (1, 3), 'and': (1, 3), 'or': (1, 3), 'xor': (1, 3),
    'inc': (1, 3), 'dec': (1, 3), 'neg': (1, 3), 'not': (1, 3), 'cmp': (1, 2), 'test': (1, 2),
    'xadd': (3, 4), 'cmpxchg': (6, 7),
    'push': (1, 4), 'pop': (1, 6), 'pusha': (11, 11), 'pushad': (11, 11), 'popa': (9, 9), 'popad': (9, 9),
    'pushf': (3, 3), 'pushfd': (3, 3), 'popf': (6, 6), 'popfd': (6, 6),
    'cdq': (3, 3), 'cwd': (3, 3), 'cbw': (3, 3), 'cwde': (3, 3),
    'ret': (5, 5), 'leave': (5, 5), 'nop': (3, 3), 'xlat': (4, 4),
    'clc': (2, 2), 'stc': (2, 2), 'cmc': (2, 2), 'cld': (2, 2), 'std': (2, 2), 'cli': (5, 5), 'sti': (5, 5),
    'lahf': (3, 3), 'sahf': (2, 2),
    'bt': (3, 8), 'bts': (6, 13), 'btr': (6, 13), 'btc': (6, 13),
    # Bit scans depend on the bits (notes 12 to 15): a set bit in the low byte for BSF, near the middle for BSR.
    'bsf': (21, 22), 'bsr': (55, 56),
}
ALU = {'add', 'adc', 'sub', 'sbb', 'and', 'or', 'xor'}
ALU_FROM_MEMORY = 2
SHIFT = {'shl', 'sal', 'shr', 'sar', 'rol', 'ror'}
# The string instructions alone, and with a `rep` prefix as (clocks for each start, clocks for each element): REP MOVS is
# 12+3c, REP STOS and REP LODS 7+4c, REPE/REPNE CMPS 7+7c and SCAS 7+5c.
STRING = {'movs': 7, 'stos': 5, 'lods': 5, 'cmps': 8, 'scas': 6}
REP = {'movs': (12, 3), 'stos': (7, 4), 'lods': (7, 4), 'cmps': (7, 7), 'scas': (7, 5)}
# A multiply is 10 + max(log2|m|, 3) clocks for a multiplier m (5 for a negative one, note 3 of table 10.1). An immediate is
# taken as the multiplier; any other is taken as about a thousand (log2 10), the size of the screen coordinates and table
# indices the drawing multiplies. A division by a constant done as a multiply by a 32-bit reciprocal in a register costs more.
MUL_UNKNOWN_BITS = 10
# A conditional jump taken and not taken (3/1); 2 for one whose counts are unknown.
JCC = (3, 1)
JCC_UNKNOWN = 2

REG32 = {
    'eax': 'eax', 'ax': 'eax', 'al': 'eax', 'ah': 'eax', 'ebx': 'ebx', 'bx': 'ebx', 'bl': 'ebx', 'bh': 'ebx',
    'ecx': 'ecx', 'cx': 'ecx', 'cl': 'ecx', 'ch': 'ecx', 'edx': 'edx', 'dx': 'edx', 'dl': 'edx', 'dh': 'edx',
    'esi': 'esi', 'si': 'esi', 'edi': 'edi', 'di': 'edi', 'ebp': 'ebp', 'bp': 'ebp', 'esp': 'esp', 'sp': 'esp',
}
REG16 = {'ax', 'bx', 'cx', 'dx', 'si', 'di', 'bp', 'sp'}
REG8 = {'al', 'ah', 'bl', 'bh', 'cl', 'ch', 'dl', 'dh'}
# Instructions whose first operand, when a register, is written (for the address interlock of the next instruction).
WRITES_FIRST = ALU | SHIFT | {'mov', 'movzx', 'movsx', 'lea', 'inc', 'dec', 'neg', 'not', 'pop', 'xchg', 'imul', 'bswap',
                              'shld', 'shrd', 'rcl', 'rcr', 'xadd'}
IMMEDIATE = re.compile(r'^-?(0x[0-9a-f]+|\d+)$')


def operands(text):
    """The comma-separated operands of an instruction's operand text (Intel syntax has no comma inside an address)."""
    text = re.sub(r'\s*<[^>]*>', '', text).strip()
    return [o.strip() for o in text.split(',')] if text else []


def address(op):
    """The address part of a memory operand (`[...]`, or `ds:0x...` for an absolute one), or None."""
    m = re.search(r'\[([^\]]*)\]', op)
    if m:
        return m.group(1)
    m = re.search(r'\b[cdefgs]s:(0x[0-9a-f]+|\d+)', op)
    return m.group(1) if m else None


def size(op):
    """`m16`, `m32`, `m64` or `m80` for a memory operand, `r` for anything else."""
    for word, name in (('TBYTE', 'm80'), ('QWORD', 'm64'), ('DWORD', 'm32'), ('WORD', 'm16'), ('BYTE', 'm8')):
        if word + ' PTR' in op:
            return name
    return 'm32' if address(op) is not None else 'r'


def registers(text):
    """The 32-bit registers named in `text`, by family."""
    return {REG32[t] for t in re.findall(r'[a-z]+', text) if t in REG32}


def written(ins):
    """The registers the instruction `ins` (text) writes explicitly, by family."""
    parts = ins.split(None, 1)
    if not parts:
        return set()
    m, ops = parts[0], operands(parts[1] if len(parts) > 1 else '')
    if not ops or address(ops[0]) is not None:
        return set()
    if m in WRITES_FIRST or m.startswith('set'):
        out = registers(ops[0])
        if m == 'xchg' and len(ops) > 1:
            out |= registers(ops[1])
        return out
    return set()


def ends_block(ins):
    """Whether `ins` never falls through to the next instruction (so it cannot interlock with it)."""
    m = ins.split(None, 1)[0] if ins.strip() else ''
    return m in ('jmp', 'ret', 'ud2', 'int3') or m.startswith('ret')


class Weight:
    """The clocks of one instruction: `base` for each execution; for a conditional jump, `taken` and `not_taken`; for a `rep`
    string instruction, `start` and `element`. `x87` marks floating-point work and `known` whether the table had the
    instruction."""

    def __init__(self, base, x87=False, known=True, jcc=None, rep=None):
        self.base, self.x87, self.known, self.jcc, self.rep = base, x87, known, jcc, rep

    def clocks(self, count, taken=None, repeats=None):
        """Clocks for `count` executions; `taken` is how many of them jumped (a conditional jump) or `repeats` how many went
        round again (a `rep` instruction), when the profile recorded jumps."""
        if self.jcc:
            if taken is None:
                return JCC_UNKNOWN * count
            return self.jcc[0] * taken + self.jcc[1] * (count - taken)
        if self.rep:
            start, element = self.rep
            if repeats is None:
                return element * count
            # callgrind counts one execution for each element and one more for each start (the test of ECX that ends it).
            return start * (count - repeats) + element * repeats
        return self.base * count


def mul_clocks(ops):
    imm = next((o for o in ops[1:] if IMMEDIATE.match(o)), None)
    if imm is None:
        bits, negative = MUL_UNKNOWN_BITS, False
    else:
        value = int(imm, 0)
        if value >= 1 << 31:
            value -= 1 << 32  # objdump prints a 32-bit immediate unsigned; IMUL's is signed
        bits, negative = max(abs(value).bit_length() - 1, 0), value < 0
    return 10 + max(bits, 5 if negative else 3)


def weigh(ins, prev=None):
    """The `Weight` of the instruction `ins` (objdump's Intel-syntax text), with `prev` the instruction before it in memory."""
    parts = ins.split(None, 1)
    if not parts:
        return Weight(1, known=False)
    m, rest = parts[0], parts[1] if len(parts) > 1 else ''
    if m.startswith('rep'):
        parts = rest.split(None, 1)
        s = parts[0] if parts else ''
        for k, v in REP.items():
            if s.startswith(k):
                return Weight(0, rep=v)
        return Weight(2, known=False)
    ops = operands(rest)
    mem = next((o for o in ops if address(o) is not None), None)
    for k, v in STRING.items():
        if m.startswith(k) and m not in ('movsx', 'movzx', 'movsxd'):
            return Weight(v)
    if m.startswith('f') or m in ('wait',):
        w = X87.get(m)
        if w is None:
            return Weight(10, x87=True, known=False) if m.startswith('f') else Weight(2, known=False)
        if isinstance(w, dict):
            form = size(mem) if mem else 'r'
            w = w.get(form, max(w.values()))
        return Weight(w + ea_extra(ops, mem, prev), x87=True)
    if m.startswith('j'):
        if m == 'jmp':
            return Weight(3 if ops and re.match(r'^[0-9a-f]+$', ops[0]) else 5)
        if m in ('jecxz', 'jcxz'):
            return Weight(0, jcc=(8, 5))
        return Weight(0, jcc=JCC)
    if m == 'call':
        return Weight(3 if ops and re.match(r'^[0-9a-f]+$', ops[0]) else 5)
    if m == 'loop':
        return Weight(0, jcc=(7, 6))
    if m in ('loope', 'loopz', 'loopne', 'loopnz'):
        return Weight(0, jcc=(9, 6))
    if m.startswith('set'):
        return Weight(3.5)
    if m in ('mul', 'imul'):
        return Weight(mul_clocks(ops) + ea_extra(ops, mem, prev))
    if m in ('div', 'idiv'):
        form = size(ops[0]) if ops else 'm32'
        reg = ops[0] if ops else ''
        width = 8 if form == 'm8' or reg in REG8 else 16 if form == 'm16' or reg in REG16 else 32
        c = {8: 16, 16: 24, 32: 40}[width] if m == 'div' else {8: 19, 16: 27, 32: 43}[width] + (mem is not None)
        return Weight(c + ea_extra(ops, mem, prev))
    if m in SHIFT or m in ('rcl', 'rcr'):
        count = ops[1] if len(ops) > 1 else '1'
        if m in SHIFT:
            c = (4 if mem else 2) if IMMEDIATE.match(count) and count != '1' else (4 if mem else 3)
        else:
            c = (4 if mem else 3) if count == '1' else (9 if mem else 8)
        return Weight(c + ea_extra(ops, mem, prev))
    if m in ('shld', 'shrd'):
        by_cl = len(ops) > 2 and ops[2] == 'cl'
        c = (4 if by_cl else 3) if mem else (3 if by_cl else 2)
        return Weight(c + ea_extra(ops, mem, prev))
    if m in INT:
        reg, memory = INT[m]
        if mem is None:
            c = reg
        elif m in ALU and address(ops[0]) is None:
            c = ALU_FROM_MEMORY
        else:
            c = memory
        return Weight(c + ea_extra(ops, mem, prev))
    return Weight(2 + ea_extra(ops, mem, prev), known=False)


def ea_extra(ops, mem, prev):
    """The datasheet's assumptions 4, 5 and 8 for an instruction with the memory operand `mem`: an address register written by
    the instruction before, an index register, and a displacement together with an immediate."""
    if mem is None:
        return 0
    ea = address(mem)
    extra = 0
    if ea is not None and ('*' in ea or len(re.findall(r'\b(e?[a-ds][ixlp]|e?[sd]i|e?[bs]p)\b', ea)) > 1):
        extra += 1
    if ea is not None and prev and not ends_block(prev) and registers(ea) & written(prev):
        extra += 1
    if ea is not None and re.search(r'(^|[+-])(0x[0-9a-f]+|\d+)$', ea) and any(IMMEDIATE.match(o) for o in ops):
        extra += 1
    return extra


# --- Profiles ---------------------------------------------------------------------------------------------------------------

class Disassembly:
    """Address to (instruction text, previous instruction text) for one object, read with objdump."""

    cache = {}

    @classmethod
    def of(cls, obj):
        if obj not in cls.cache:
            try:
                text = subprocess.run(['objdump', '-d', '--no-show-raw-insn', '-M', 'intel', obj],
                                      capture_output=True, text=True, check=False).stdout
            except OSError:
                text = ''
            cls.cache[obj] = cls.parse(text)
        return cls.cache[obj]

    @staticmethod
    def parse(text):
        table, prev = {}, None
        for line in text.splitlines():
            m = re.match(r'\s*([0-9a-f]+):\s+(.*)', line)
            if m:
                ins = m.group(2).split('#')[0].strip()
                table[int(m.group(1), 16)] = (ins, prev)
                prev = ins
            elif line.strip() == '' or line.endswith(':'):
                prev = None
        return table


class Profile:
    """One callgrind profile (or several summed), as counts for each instruction: executions, and for an instruction that is
    the source of jumps, how many jumped and how many jumped to itself (a `rep` going round again)."""

    def __init__(self):
        self.count = collections.Counter()  # (object, function, address) -> executions
        self.taken = collections.Counter()  # (object, address) -> jumps from there
        self.self_jumps = collections.Counter()  # (object, address) -> jumps from there to itself
        self.jumps = False

    @classmethod
    def read(cls, path):
        p = cls()
        with open(path) as f:
            p.parse(f)
        return p

    def parse(self, lines):
        names = {}
        obj = fn = None
        ir = 0
        npos = 1
        pending = None  # what the next position line is: 'call' or ('jump', taken, target)

        def name(value):
            m = re.match(r'\((\d+)\)\s*(.*)', value)
            if not m:
                return value
            if m.group(2):
                names[m.group(1)] = m.group(2)
            return names.get(m.group(1), value)

        for line in lines:
            line = line.rstrip('\n')
            if line.startswith('positions:'):
                npos = len(line.split()) - 1
                if 'instr' not in line:
                    raise SystemExit('cyc486: the profile has no instruction addresses (record it with --dump-instr=yes)')
            elif line.startswith('events:'):
                ir = line.split()[1:].index('Ir')
            elif line.startswith('ob='):
                obj = name(line[3:])
            elif line.startswith('fn='):
                fn = name(line[3:])
            elif line.startswith(('cob=', 'cfi=', 'cfl=', 'cfn=')):
                name(line.split('=', 1)[1])
            elif line.startswith('calls='):
                pending = 'call'
            elif line.startswith('jump='):
                fields = line[5:].split()
                pending = ('jump', int(fields[0]), fields[1])
                self.jumps = True
            elif line.startswith('jcnd='):
                fields = line[5:].split()
                pending = ('jump', int(fields[0].split('/')[0]), fields[1])
                self.jumps = True
            elif line[:1] in '0123456789+-*' and line:
                fields = line.split()
                if not fields[0].startswith('0x'):
                    raise SystemExit('cyc486: compressed positions (record the profile with --compress-pos=no)')
                addr = int(fields[0], 16)
                if pending == 'call':
                    pending = None  # the call's inclusive cost: counted where it was spent
                    continue
                if pending:
                    _, n, target = pending
                    pending = None
                    self.taken[obj, addr] += n
                    if int(target, 16) == addr:
                        self.self_jumps[obj, addr] += n
                    continue
                if len(fields) > npos + ir:
                    self.count[obj, fn, addr] += int(fields[npos + ir])
            else:
                pending = None

    def add(self, other, sign=1):
        for k, v in other.count.items():
            self.count[k] += sign * v
        for k, v in other.taken.items():
            self.taken[k] += sign * v
        for k, v in other.self_jumps.items():
            self.self_jumps[k] += sign * v
        self.jumps = self.jumps or other.jumps

    def functions(self, disassembly=Disassembly.of):
        """For each function: [instructions, clocks, x87 instructions, x87 clocks, rep elements]; with the instructions the
        table did not know, as {text: executions}."""
        per = collections.defaultdict(lambda: [0, 0.0, 0, 0.0, 0])
        unknown = collections.Counter()
        for (obj, fn, addr), n in self.count.items():
            if n == 0:
                continue
            ins, prev = disassembly(obj).get(addr, (None, None)) if obj else (None, None)
            w = weigh(ins, prev) if ins else Weight(2, known=False)
            jumps = self.jumps
            c = w.clocks(n, self.taken[obj, addr] if jumps else None, self.self_jumps[obj, addr] if jumps else None)
            e = per[fn]
            e[0] += n
            e[1] += c
            if w.x87:
                e[2] += n
                e[3] += c
            if w.rep:
                e[4] += self.self_jumps[obj, addr] if jumps else n
            if not w.known:
                unknown[(ins or '?').split()[0] if ins else '?'] += n
        return per, unknown


# --- Reports ----------------------------------------------------------------------------------------------------------------

MHZ = 66  # the 486DX2-66, the floor
DOSBOX_PER_US = 26.8  # DOSBox at 26,800 cycles a millisecond charges about one cycle an instruction


def short(fn):
    return re.sub(r'::h[0-9a-f]{16}$', '', fn or '?')[-72:]


def totals(per):
    return [sum(v[i] for v in per.values()) for i in range(5)]


def summary(label, t):
    return (f"{label}{t[0]:>11,.0f} instructions ({t[0] / DOSBOX_PER_US / 1000:5.2f} ms in DOSBox at 26,800)  "
            f"about {t[1]:>11,.0f} 486 clocks ({t[1] / MHZ / 1000:5.2f} ms at {MHZ} MHz, "
            f"{t[1] / max(t[0], 1):.2f} an instruction); "
            f"x87 {100 * t[3] / max(t[1], 1):.0f}% of the clocks, rep elements {t[4]:,.0f}")


def top_lines(per, n):
    out = []
    for fn, v in sorted(per.items(), key=lambda kv: -kv[1][1])[:n]:
        out.append(f"  {v[1]:>11,.0f} clocks {v[0]:>10,.0f} instr {v[1] / max(v[0], 1):5.2f} an instr "
                   f"{v[3]:>9,.0f} x87  {short(fn)}")
    return out


def scaled(per, k):
    return {fn: [x * k for x in v] for fn, v in per.items()}


def report_unknown(unknown, total):
    if unknown:
        n = sum(unknown.values())
        names = ', '.join(f'{k} {v:,}' for k, v in unknown.most_common(8))
        print(f"not in the table (weighed 2, x87 10): {n:,} executions, {100 * n / max(total, 1):.2f}%: {names}")


def main(args):
    top = 12
    if '--top' in args:
        i = args.index('--top')
        top = int(args[i + 1])
        del args[i:i + 2]
    if args and args[0] == '--parts':
        labels = [line.strip() for line in open(args[1]) if line.strip()]
        parts = args[2:]
        if len(labels) != len(parts):
            raise SystemExit(f'cyc486: {len(labels)} labels for {len(parts)} parts')
        frames, whole, unknown = [], collections.defaultdict(lambda: [0, 0.0, 0, 0.0, 0]), collections.Counter()
        jumps = True
        for label, path in zip(labels, parts):
            p = Profile.read(path)
            jumps = jumps and p.jumps
            per, unk = p.functions()
            unknown += unk
            frames.append((label, totals(per)))
            for fn, v in per.items():
                for i in range(5):
                    whole[fn][i] += v[i]
        if not jumps:
            print('cyc486: the profiles have no jump counts (--collect-jumps=yes); jumps and rep set-up are estimated')
        for name in sorted({f[0] for f in frames}, key=[f[0] for f in frames].index) + [None]:
            chosen = [t for label, t in frames if name is None or label == name]
            avg = [sum(t[i] for t in chosen) / len(chosen) for i in range(5)]
            worst = max(chosen, key=lambda t: t[1])
            print(summary(f"{name or 'all':8} {len(chosen):3} frames, average ", avg))
            print(f"{'':8} worst frame {worst[1]:>11,.0f} 486 clocks ({worst[1] / MHZ / 1000:5.2f} ms at {MHZ} MHz), "
                  f"{worst[0]:,.0f} instructions")
        print(f'top {top} functions, average a frame over all {len(frames)}:')
        print('\n'.join(top_lines(scaled(whole, 1 / len(frames)), top)))
        report_unknown(unknown, sum(t[0] for _, t in frames))
        return
    frames = 1.0
    if '--frames' in args:
        i = args.index('--frames')
        frames = float(args[i + 1])
        del args[i:i + 2]
    minus = None
    if '--minus' in args:
        i = args.index('--minus')
        minus = args[i + 1]
        del args[i:i + 2]
    if len(args) != 1:
        raise SystemExit('usage: cyc486.py PROFILE [--frames N] [--minus SHORT] [--top N] | --parts LABELS PART... | --self-test')
    p = Profile.read(args[0])
    if minus:
        p.add(Profile.read(minus), -1)
    if not p.jumps:
        print('cyc486: the profile has no jump counts (--collect-jumps=yes); jumps and rep set-up are estimated')
    per, unknown = p.functions()
    per = scaled(per, 1 / frames)
    print(summary(f'a frame, the average of {frames:,.0f}: ', totals(per)))
    print(f'top {top} functions, a frame:')
    print('\n'.join(top_lines(per, top)))
    report_unknown(collections.Counter({k: v / frames for k, v in unknown.items()}), totals(per)[0])


# --- Self-test --------------------------------------------------------------------------------------------------------------

# A profile of a small 32-bit program (as callgrind 3.22 writes it with --dump-instr=yes --collect-jumps=yes
# --compress-pos=no): `rep stosd` with ECX 10 then 0, `rep movsd` with ECX 1, a loop of five with a `jne` taken four times.
SAMPLE = """\
version: 1
positions: instr line
events: Ir
ob=(1) /t
fn=(1) _start
0x1000 0 1
0x1001 0 1
jump=1 0x1001 0
0x1001 0
0x1001 0 10
jcnd=9/10 0x1001 0
0x1001 0
0x1003 0 1
0x1005 0 1
0x1008 0 1
jump=1 0x1008 0
0x1008 0
0x1008 0 1
fn=(2) work
0x2000 0 5
0x2002 0 5
jcnd=4/5 0x2000 0
0x2002 0
calls=1 0x2000 0
0x100a 0 99
0x2004 0 1
"""
SAMPLE_DIS = {
    0x1000: ('mov ecx,0xa', None),
    0x1001: ('rep stos DWORD PTR es:[edi],eax', 'mov ecx,0xa'),
    0x1003: ('xor ecx,ecx', 'rep stos DWORD PTR es:[edi],eax'),
    0x1005: ('rep stos DWORD PTR es:[edi],eax', 'xor ecx,ecx'),
    0x1008: ('rep movs DWORD PTR es:[edi],DWORD PTR ds:[esi]', 'rep stos DWORD PTR es:[edi],eax'),
    0x2000: ('dec edx', None),
    0x2002: ('jne 2000 <work>', 'dec edx'),
    0x2004: ('ret', 'jne 2000 <work>'),
}


class Tests(unittest.TestCase):
    def clocks(self, ins, prev=None):
        return weigh(ins, prev).clocks(1)

    def test_integer_forms(self):
        self.assertEqual(self.clocks('mov eax,DWORD PTR [esp+0x8]'), 1)
        self.assertEqual(self.clocks('add eax,DWORD PTR [esp+0x8]'), 2)
        self.assertEqual(self.clocks('add DWORD PTR [esp+0x8],eax'), 3)
        self.assertEqual(self.clocks('add DWORD PTR [esp+0x8],0x1'), 4)  # displacement and immediate
        self.assertEqual(self.clocks('cmp eax,ecx'), 1)
        self.assertEqual(self.clocks('movzx eax,BYTE PTR [ecx+edx*1]'), 4)  # index register
        self.assertEqual(self.clocks('mov eax,DWORD PTR [ecx]', 'lea ecx,[eax+0x4]'), 2)  # address interlock
        self.assertEqual(self.clocks('mov eax,DWORD PTR [ecx]', 'jmp 1000 <x>'), 1)
        self.assertEqual(self.clocks('push ebx'), 1)
        self.assertEqual(self.clocks('pop ebx'), 1)
        self.assertEqual(self.clocks('shl eax,1'), 3)
        self.assertEqual(self.clocks('shl eax,0x4'), 2)
        self.assertEqual(self.clocks('shl eax,cl'), 3)
        self.assertEqual(self.clocks('imul eax,ecx,0x140'), 18)
        self.assertEqual(self.clocks('imul eax,ecx,0xffffaaab'), 24)  # -21845: 14 bits
        self.assertEqual(self.clocks('imul eax,ecx'), 10 + MUL_UNKNOWN_BITS)
        self.assertEqual(self.clocks('div ecx'), 40)
        self.assertEqual(self.clocks('idiv DWORD PTR [esp]'), 44)
        self.assertEqual(self.clocks('call 1234 <f>'), 3)
        self.assertEqual(self.clocks('call DWORD PTR [eax]'), 5)
        self.assertEqual(self.clocks('stos DWORD PTR es:[edi],eax'), 5)
        self.assertFalse(weigh('cmovne eax,ecx').known)

    def test_x87_forms(self):
        self.assertEqual(self.clocks('fld QWORD PTR [esp]'), 3)
        self.assertEqual(self.clocks('fld st(1)'), 4)
        self.assertEqual(self.clocks('fstp st(0)'), 3)
        self.assertEqual(self.clocks('fstp QWORD PTR [esp]'), 8)
        self.assertEqual(self.clocks('fmul DWORD PTR [eax]'), 11)
        self.assertEqual(self.clocks('fmul QWORD PTR [eax]'), 14)
        self.assertEqual(self.clocks('fmul st,st(1)'), 16)
        self.assertEqual(self.clocks('fdiv st,st(1)'), 62)
        self.assertEqual(self.clocks('fistp DWORD PTR [esp]'), 33.4)
        self.assertTrue(weigh('fadd st,st(1)').x87)

    def test_jumps_and_rep(self):
        jne = weigh('jne 2000 <x>')
        self.assertEqual(jne.clocks(5, taken=4), 4 * 3 + 1)
        self.assertEqual(jne.clocks(5), 10)
        stos = weigh('rep stos DWORD PTR es:[edi],eax')
        self.assertEqual(stos.clocks(11, repeats=10), 7 + 4 * 10)  # one start of ten elements
        self.assertEqual(stos.clocks(11), 44)  # no jump counts: each execution an element

    def test_parse(self):
        p = Profile()
        p.parse(SAMPLE.splitlines(True))
        self.assertTrue(p.jumps)
        self.assertEqual(p.count['/t', '_start', 0x1001], 11)
        self.assertEqual(p.self_jumps['/t', 0x1001], 10)
        self.assertEqual(p.count['/t', '_start', 0x1008], 2)
        self.assertEqual(p.self_jumps['/t', 0x1008], 1)
        self.assertEqual(p.taken['/t', 0x2002], 4)
        self.assertNotIn(('/t', 'work', 0x100a), p.count)  # a call's inclusive cost is not the caller's own
        per, unknown = p.functions(lambda obj: SAMPLE_DIS)
        self.assertEqual(unknown, collections.Counter())
        # _start: mov 1, rep stosd (10) 7+40, xor 1, rep stosd (0) 7, rep movsd (1) 12+3.
        self.assertEqual(per['_start'][1], 1 + 47 + 1 + 7 + 15)
        self.assertEqual(per['_start'][4], 11)
        # work: dec 5, jne 4 taken and 1 not, ret 5.
        self.assertEqual(per['work'][1], 5 + 13 + 5)

    def test_minus(self):
        a, b = Profile(), Profile()
        a.parse(SAMPLE.splitlines(True))
        b.parse(SAMPLE.splitlines(True))
        b.add(a)
        b.add(a, -1)
        self.assertEqual(b.functions(lambda obj: SAMPLE_DIS)[0]['work'][1], 23)

    def test_disassembly(self):
        text = '\n00001000 <_start>:\n    1000:\tmov    ecx,0xa\n    1005:\trep stos DWORD PTR es:[edi],eax\n'
        table = Disassembly.parse(text)
        self.assertEqual(table[0x1005], ('rep stos DWORD PTR es:[edi],eax', 'mov    ecx,0xa'))
        self.assertIsNone(table[0x1000][1])


if __name__ == '__main__':
    if sys.argv[1:] == ['--self-test']:
        unittest.main(argv=[sys.argv[0]], verbosity=1)
    else:
        main(sys.argv[1:])
