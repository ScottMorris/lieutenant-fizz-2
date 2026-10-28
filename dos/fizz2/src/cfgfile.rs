// Crash-safe writing and recovering of the settings file: `FIZZ2.NEW` is written and verified before `FIZZ2.CFG` is replaced.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// A save never truncates the good file. It writes `FIZZ2.NEW`, closes it, reads it back and compares, deletes `FIZZ2.CFG` and
// renames the new file over the name. Power lost before the delete leaves the old file; power lost between the delete and the
// rename leaves only `FIZZ2.NEW`, which `read` uses when `FIZZ2.CFG` is missing. While writing, a DOS critical error (a
// write-protected floppy, an open drive door) is answered "Fail" by a tiny handler, so no "Abort, Retry, Fail?" is printed over
// mode 13h and the failure comes back as an error for the caller to report.
use alloc::vec::Vec;
use fizz_dos_rt::dos::{self, DosError, File, Mode};

/// The file the new bytes are written to first.
pub const NEW_NAME: &str = "FIZZ2.NEW";

/// DOS error 2, file not found.
const NOT_FOUND: u16 = 2;
/// A reading or verifying mismatch, reported as DOS error 29 (write fault).
const WRITE_FAULT: u16 = 29;

/// Writes `bytes` as `name` the safe way. On failure the old `name` is untouched, or (after the delete) `NEW_NAME` holds the bytes.
pub fn save_bytes(name: &str, bytes: &[u8]) -> Result<(), DosError> {
    let _guard = dos::FailCritical::install();
    let result = write_verify(bytes).and_then(|()| replace(name));
    if result.is_err() {
        // A failure before the old file went leaves a stale new one; one after it is the copy recovery needs.
        if File::open(name, Mode::Read).is_ok() {
            let _ = dos::remove(NEW_NAME);
        }
    }
    result
}

fn write_verify(bytes: &[u8]) -> Result<(), DosError> {
    {
        let mut f = File::open(NEW_NAME, Mode::Create)?;
        f.write_all(bytes)?;
    }
    let back = File::open(NEW_NAME, Mode::Read)?.read_to_vec(0)?;
    if back == bytes { Ok(()) } else { Err(DosError(WRITE_FAULT)) }
}

fn replace(name: &str) -> Result<(), DosError> {
    match dos::remove(name) {
        Ok(()) => {}
        Err(DosError(NOT_FOUND)) => {}
        Err(e) => return Err(e),
    }
    dos::rename(NEW_NAME, name)
}

/// Reads at most `max` bytes of `name`; if it is missing, of `NEW_NAME`, which an interrupted save may have left.
pub fn read(name: &str, max: usize) -> Vec<u8> {
    let mut buf = alloc::vec![0u8; max];
    let n = match File::open(name, Mode::Read) {
        Ok(mut f) => f.read(&mut buf).unwrap_or(0),
        Err(_) => match File::open(NEW_NAME, Mode::Read) {
            Ok(mut f) => f.read(&mut buf).unwrap_or(0),
            Err(_) => 0,
        },
    };
    buf.truncate(n);
    buf
}
