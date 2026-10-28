// A short fixed-capacity string for row values, so a presenter needs no allocation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The most bytes a row value can hold. Every value the screens build fits; a longer one is cut short.
pub const TEXT_CAP: usize = 32;

/// ASCII text of at most `TEXT_CAP` bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Text {
    buf: [u8; TEXT_CAP],
    len: u8,
}

impl Text {
    pub const EMPTY: Text = Text {
        buf: [0; TEXT_CAP],
        len: 0,
    };

    /// The parts joined, cut at the capacity.
    pub fn join(parts: &[&str]) -> Text {
        let mut t = Text::EMPTY;
        for p in parts {
            t.push(p);
        }
        t
    }

    /// True when the parts joined would not fit, so a test can prove nothing is ever cut.
    pub fn fits(parts: &[&str]) -> bool {
        parts.iter().map(|p| p.len()).sum::<usize>() <= TEXT_CAP
    }

    fn push(&mut self, s: &str) {
        for &b in s.as_bytes() {
            if usize::from(self.len) == TEXT_CAP {
                return;
            }
            self.buf[usize::from(self.len)] = if b.is_ascii() { b } else { b'?' };
            self.len += 1;
        }
    }

    pub fn as_str(&self) -> &str {
        // The bytes are ASCII by construction, so this never fails; an empty string is the safe answer if it ever did.
        core::str::from_utf8(&self.buf[..usize::from(self.len)]).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl core::fmt::Debug for Text {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}
