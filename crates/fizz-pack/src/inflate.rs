// A raw-deflate (RFC 1951) decoder that writes into a caller-provided buffer: no allocation, no dependencies, about 1.3 KB of stack.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The design follows zlib's `puff.c` (canonical codes decoded a bit at a time from per-length counts, so there are no big lookup
// tables) and keeps the length and distance base tables out of the binary by computing them. It is built for small code, not speed:
// sections are decoded once at load time. Every failure is an `InflateError`; the output buffer bounds every write, so a deflate
// bomb stops at the recorded decoded length.

/// Why a deflate stream was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InflateError {
    /// The input ended before the final block did.
    Truncated,
    /// A block type of 3, or a stored block whose length and its complement disagree.
    BadBlock,
    /// A Huffman table that is over-subscribed, a code-length repeat that has nothing to repeat or runs past the table, or a
    /// dynamic block with no end-of-block code.
    BadTable,
    /// A bit pattern that is no code of its table, or a length or distance symbol that does not exist.
    BadSymbol,
    /// A back-reference that reaches before the start of the output.
    BadDistance,
    /// The stream would write more than the recorded decoded length.
    Overrun,
    /// The stream ended with fewer bytes than the recorded decoded length.
    Short,
}

const MAX_BITS: usize = 15;
/// Length codes 0 to 285 plus the two unused ones the fixed table defines.
const MAX_LIT: usize = 288;

struct Bits<'a> {
    src: &'a [u8],
    pos: usize,
    buf: u32,
    cnt: u32,
}

impl Bits<'_> {
    /// Reads `n` (at most 16) bits, least significant first.
    fn take(&mut self, n: u32) -> Result<u32, InflateError> {
        while self.cnt < n {
            let b = *self.src.get(self.pos).ok_or(InflateError::Truncated)?;
            self.pos += 1;
            self.buf |= u32::from(b) << self.cnt;
            self.cnt += 8;
        }
        let v = self.buf & ((1 << n) - 1);
        self.buf >>= n;
        self.cnt -= n;
        Ok(v)
    }
}

/// A canonical Huffman table: how many codes of each length, and the symbols in code order.
struct Table<'t> {
    count: [u16; MAX_BITS + 1],
    symbol: &'t mut [u16],
}

impl Table<'_> {
    /// Builds the table from per-symbol code lengths (0 = unused). Over-subscribed sets are rejected; incomplete ones are
    /// accepted and fail only if the stream uses a code that is not there.
    fn build(&mut self, lengths: &[u8]) -> Result<(), InflateError> {
        self.count = [0; MAX_BITS + 1];
        for &l in lengths {
            self.count[usize::from(l)] += 1;
        }
        let mut left = 1i32;
        let mut offs = [0u16; MAX_BITS + 2];
        for len in 1..=MAX_BITS {
            left = (left << 1) - i32::from(self.count[len]);
            if left < 0 {
                return Err(InflateError::BadTable);
            }
            offs[len + 1] = offs[len] + self.count[len];
        }
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                let o = &mut offs[usize::from(l)];
                self.symbol[usize::from(*o)] = sym as u16;
                *o += 1;
            }
        }
        Ok(())
    }

    fn decode(&self, bits: &mut Bits) -> Result<u16, InflateError> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..=MAX_BITS {
            code |= bits.take(1)? as i32;
            let count = i32::from(self.count[len]);
            if code - count < first {
                return Ok(self.symbol[(index + code - first) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(InflateError::BadSymbol)
    }
}

/// Base value and extra-bit count of length symbol (`group` 2, `first` 3) or distance symbol (`group` 1, `first` 1): the extra bits
/// grow by one every `1 << group` symbols, and each symbol starts where the previous one's range ended.
fn base_extra(sym: u16, group: u32, first: u32) -> (u32, u32) {
    let extra = |i: u32| (i >> group).saturating_sub(1);
    let base = first + (0..u32::from(sym)).map(|i| 1 << extra(i)).sum::<u32>();
    (base, extra(u32::from(sym)))
}

/// Decodes the raw deflate stream `src` into exactly `out`: the stream must fill `out` completely and not a byte more.
pub fn inflate(src: &[u8], out: &mut [u8]) -> Result<(), InflateError> {
    let mut bits = Bits {
        src,
        pos: 0,
        buf: 0,
        cnt: 0,
    };
    let mut at = 0usize;
    loop {
        let last = bits.take(1)?;
        match bits.take(2)? {
            0 => stored(&mut bits, out, &mut at)?,
            kind @ (1 | 2) => {
                let mut lsym = [0u16; MAX_LIT];
                let mut dsym = [0u16; 32];
                let mut lit = Table {
                    count: [0; MAX_BITS + 1],
                    symbol: &mut lsym,
                };
                let mut dist = Table {
                    count: [0; MAX_BITS + 1],
                    symbol: &mut dsym,
                };
                if kind == 1 {
                    let mut l = [8u8; MAX_LIT];
                    l[144..256].fill(9);
                    l[256..280].fill(7);
                    lit.build(&l)?;
                    dist.build(&[5; 32])?;
                } else {
                    dynamic(&mut bits, &mut lit, &mut dist)?;
                }
                codes(&mut bits, &lit, &dist, out, &mut at)?;
            }
            _ => return Err(InflateError::BadBlock),
        }
        if last == 1 {
            return if at == out.len() {
                Ok(())
            } else {
                Err(InflateError::Short)
            };
        }
    }
}

fn stored(bits: &mut Bits, out: &mut [u8], at: &mut usize) -> Result<(), InflateError> {
    // `take` never reads ahead by a whole byte, so dropping the partial byte is all the alignment needed.
    bits.buf = 0;
    bits.cnt = 0;
    let h = bits
        .src
        .get(bits.pos..bits.pos + 4)
        .ok_or(InflateError::Truncated)?;
    let (len, nlen) = (
        u16::from_le_bytes([h[0], h[1]]),
        u16::from_le_bytes([h[2], h[3]]),
    );
    if len != !nlen {
        return Err(InflateError::BadBlock);
    }
    let from = bits.pos + 4;
    let data = bits
        .src
        .get(from..from + usize::from(len))
        .ok_or(InflateError::Truncated)?;
    out.get_mut(*at..*at + data.len())
        .ok_or(InflateError::Overrun)?
        .copy_from_slice(data);
    bits.pos = from + data.len();
    *at += data.len();
    Ok(())
}

fn dynamic(bits: &mut Bits, lit: &mut Table, dist: &mut Table) -> Result<(), InflateError> {
    const ORDER: [u8; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let nlen = bits.take(5)? as usize + 257;
    let ndist = bits.take(5)? as usize + 1;
    let ncode = bits.take(4)? as usize + 4;
    if nlen > 286 || ndist > 30 {
        return Err(InflateError::BadTable);
    }
    let mut lengths = [0u8; 320];
    for &o in &ORDER[..ncode] {
        lengths[usize::from(o)] = bits.take(3)? as u8;
    }
    // The code-length code is built in `lit`'s storage first; `lit` is rebuilt below once the real lengths are known.
    lit.build(&lengths[..19])?;
    lengths = [0; 320];
    let mut i = 0;
    while i < nlen + ndist {
        let sym = lit.decode(bits)?;
        let (value, repeat) = match sym {
            0..=15 => (sym as u8, 1),
            16 => (
                *lengths[..i].last().ok_or(InflateError::BadTable)?,
                3 + bits.take(2)?,
            ),
            17 => (0, 3 + bits.take(3)?),
            _ => (0, 11 + bits.take(7)?),
        };
        let end = i + repeat as usize;
        lengths
            .get_mut(i..end)
            .filter(|_| end <= nlen + ndist)
            .ok_or(InflateError::BadTable)?
            .fill(value);
        i = end;
    }
    if lengths[256] == 0 {
        return Err(InflateError::BadTable);
    }
    lit.build(&lengths[..nlen])?;
    dist.build(&lengths[nlen..nlen + ndist])
}

fn codes(
    bits: &mut Bits,
    lit: &Table,
    dist: &Table,
    out: &mut [u8],
    at: &mut usize,
) -> Result<(), InflateError> {
    loop {
        let sym = lit.decode(bits)?;
        if sym < 256 {
            *out.get_mut(*at).ok_or(InflateError::Overrun)? = sym as u8;
            *at += 1;
            continue;
        }
        if sym == 256 {
            return Ok(());
        }
        if sym > 285 {
            return Err(InflateError::BadSymbol);
        }
        let len = if sym == 285 {
            258
        } else {
            let (base, extra) = base_extra(sym - 257, 2, 3);
            base + bits.take(extra)?
        };
        let dsym = dist.decode(bits)?;
        if dsym > 29 {
            return Err(InflateError::BadSymbol);
        }
        let (base, extra) = base_extra(dsym, 1, 1);
        let d = (base + bits.take(extra)?) as usize;
        if d > *at {
            return Err(InflateError::BadDistance);
        }
        if len as usize > out.len() - *at {
            return Err(InflateError::Overrun);
        }
        // Byte by byte: the source may overlap the destination (a run is a distance of 1).
        for _ in 0..len {
            out[*at] = out[*at - d];
            *at += 1;
        }
    }
}
