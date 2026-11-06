// The slice's drawing compiled as FIZZ2.EXE compiles it, run as a static 32-bit Linux program so callgrind can count it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: bench486 cameras            the ground alone at the 48 cameras of the `frame` example's `bench`, one draw each
//        bench486 script [--full] [--night]
//                                    the replay with every tick drawn, as FIZZ2.EXE /BENCH draws its first pass
//
// Built by tools/bench-draw.sh --486 for the `dos-elf.json` CPU (i486, no SSE, so `f64` is x87 code) at the DOS workspace's
// release profile, with the DOS runtime's own `memcpy` and `memset` (dos/rt/src/mem.rs, `rep movsd` and `rep stosd`) and its x87
// control word (dos/rt/src/fpu.rs, 027Fh). The machine code of the drawing is therefore the DOS build's, which the host build
// (x86-64, SSE2, vector moves) is not; only the start, the allocator and the output, outside what is counted, are this file's.
// Each draw is one call of an out-of-line function, `bench486_draw_ground` or `bench486_draw`, for callgrind's
// `--toggle-collect` and `--dump-after`.
//
// `cameras` prints `BENCH CAM <class> <x> <y>` before each draw, exactly as `frame bench all 1` does, so the parts callgrind
// dumps can be named and the two lists compared. `script` prints `BENCH TICKS`, then `BENCH FRAME <tick> <digest>` for
// `DIGEST_TICKS`, the lines FIZZ2.EXE /BENCH prints, to prove the pictures are the DOS build's.
#![no_std]
#![no_main]

extern crate alloc;

#[path = "../../../dos/rt/src/fpu.rs"]
#[allow(dead_code)] // `control_word` is FIZZ2.EXE's, for its report
mod fpu;
#[path = "../../../dos/rt/src/mem.rs"]
mod mem;

use alloc::boxed::Box;
use core::alloc::{GlobalAlloc, Layout};
use core::arch::{asm, global_asm};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use fizz_ep2::{SLICE_SCRIPT, World};
use fizz_present_dos::{DIGEST_TICKS, Frame, VIEW_H, VIEW_W, ViewFx, frame_digest, render_slice_with};
use fizz_sim::{Buttons, InputFrame};
use fizz_slice_view::DrawOptions;
use fizz_slice_view::oblique::{Ground, draw_ground_at};

// Linux starts a static program with `esp` at `argc`, then the `argv` pointers. The stack is aligned for the Rust code and the
// pointer to `argc` passed on.
global_asm!(
    ".globl _start",
    "_start:",
    "mov eax, esp",
    "and esp, -16",
    "sub esp, 12",
    "push eax",
    "call bench486_main",
    "ud2",
);

fn write(fd: u32, bytes: &[u8]) {
    // SAFETY: Linux's `write` (system call 4) reads `bytes.len()` bytes from the pointer and changes no memory of ours.
    unsafe {
        asm!("int 0x80", inlateout("eax") 4u32 => _, in("ebx") fd, in("ecx") bytes.as_ptr(), in("edx") bytes.len(), options(nostack));
    }
}

fn exit(code: u32) -> ! {
    // SAFETY: Linux's `exit` (system call 1) does not return.
    unsafe { asm!("int 0x80", in("eax") 1u32, in("ebx") code, options(noreturn, nostack)) }
}

/// A bump allocator over a static block: the drawing allocates little and frees nothing that matters for a run this short.
struct Bump {
    heap: UnsafeCell<[u8; HEAP]>,
    used: AtomicUsize,
}

const HEAP: usize = 8 << 20;

// SAFETY: the program has one thread; `used` hands each allocation a range no other allocation gets.
unsafe impl Sync for Bump {}

// SAFETY: each block is inside `heap`, aligned as asked and never handed out twice; null is returned when the heap is used up.
unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = self.heap.get() as usize;
        let start = (base + self.used.load(Ordering::Relaxed) + layout.align() - 1) & !(layout.align() - 1);
        let end = start + layout.size();
        if end > base + HEAP {
            return core::ptr::null_mut();
        }
        self.used.store(end - base, Ordering::Relaxed);
        start as *mut u8
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static ALLOCATOR: Bump = Bump {
    heap: UnsafeCell::new([0; HEAP]),
    used: AtomicUsize::new(0),
};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    write(2, b"bench486: panic\n");
    exit(101)
}

/// Writes `parts` to standard output as one line.
fn line(parts: &[&[u8]]) {
    let mut out = [0u8; 128];
    let mut n = 0;
    for part in parts {
        out[n..n + part.len()].copy_from_slice(part);
        n += part.len();
    }
    out[n] = b'\n';
    write(1, &out[..=n]);
}

fn decimal(v: u64, out: &mut [u8; 20]) -> &[u8] {
    let (mut v, mut i) = (v, out.len());
    loop {
        i -= 1;
        out[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            return &out[i..];
        }
    }
}

fn hex(v: u64, out: &mut [u8; 16]) -> &[u8] {
    for (i, d) in out.iter_mut().enumerate() {
        let n = ((v >> (60 - 4 * i)) & 15) as u8;
        *d = if n < 10 { b'0' + n } else { b'a' + n - 10 };
    }
    out
}

/// The ground at one camera: what callgrind counts in `cameras` mode.
#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn bench486_draw_ground(frame: &mut Frame, g: &Ground, x: i32, y: i32) {
    // The `frame` example's `bench` draws round 0 at tick 600 (the clouds drift with the tick).
    draw_ground_at(frame.pixels_mut(), VIEW_W, VIEW_H, g, x, y, 600);
}

/// One tick's picture: what callgrind counts in `script` mode, and what FIZZ2.EXE times as `BENCH DRAW`.
#[inline(never)]
#[unsafe(no_mangle)]
pub extern "C" fn bench486_draw(world: &World, frame: &mut Frame, options: &DrawOptions, fx: &ViewFx) {
    render_slice_with(world, frame, options, fx);
}

/// Whether the view at (`cam_x`, `cam_y`) shows any open air: the `frame` example's `sees_open_air`, which sorts its cameras into
/// indoor and gallery.
fn sees_open_air(world: &World, cam_x: i32, cam_y: i32) -> bool {
    let (mw, mh) = (world.map.w, world.map.h);
    let (c0, c1) = (cam_x.div_euclid(16), (cam_x + VIEW_W as i32 - 1).div_euclid(16));
    let (r0, r1) = (cam_y.div_euclid(16), (cam_y + VIEW_H as i32 - 1).div_euclid(16));
    (r0..=r1).any(|r| {
        (c0..=c1).any(|c| {
            let (tx, ty) = (c.clamp(0, mw - 1), (mh - 1 - r).clamp(0, mh - 1));
            let t = world.map.data[(ty * mw + tx) as usize];
            t == fizz_ep2::tiles::EMPTY || t == fizz_ep2::tiles::EXIT
        })
    })
}

/// The `frame` example's `bench all 1`: its 48 cameras, indoor first and then gallery, each drawn once.
fn cameras(frame: &mut Frame) {
    let world = fizz_ep2::level::world();
    let g = Ground {
        data: &world.map.data,
        w: world.map.w,
        h: world.map.h,
        red_closed: true,
        blue_closed: true,
    };
    for class in ["indoor", "gallery"] {
        for i in 0..48 {
            let (x, y) = (((i * 7) % 12) * 16, i * 12);
            if sees_open_air(&world, x, y) != (class == "gallery") {
                continue;
            }
            let (mut a, mut b) = ([0u8; 20], [0u8; 20]);
            line(&[
                b"BENCH CAM ",
                class.as_bytes(),
                b" ",
                decimal(x as u64, &mut a),
                b" ",
                decimal(y as u64, &mut b),
            ]);
            bench486_draw_ground(frame, &g, x, y);
        }
    }
}

/// FIZZ2.EXE's `bench::run` without the clock and the copy to the screen: step, observe, draw, every tick of the script.
fn script(frame: &mut Frame, options: &DrawOptions) {
    let mut world = fizz_ep2::level::world();
    let mut puffs = ViewFx::new();
    let mut prev = Buttons::NONE;
    let mut ticks = 0u64;
    let mut digests = [(0u64, 0u64); DIGEST_TICKS.len()];
    let mut n = 0;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            fizz_ep2::step(&mut world, InputFrame::from_transition(prev, now));
            puffs.observe(&world, VIEW_W, VIEW_H, options);
            bench486_draw(&world, frame, options, &puffs);
            prev = now;
            ticks += 1;
            if DIGEST_TICKS.contains(&world.tick.0) {
                digests[n] = (world.tick.0, frame_digest(frame));
                n += 1;
            }
        }
    }
    let mut a = [0u8; 20];
    line(&[b"BENCH TICKS ", decimal(ticks, &mut a)]);
    for &(tick, digest) in &digests[..n] {
        let (mut a, mut h) = ([0u8; 20], [0u8; 16]);
        line(&[b"BENCH FRAME ", decimal(tick, &mut a), b" ", hex(digest, &mut h)]);
    }
}

/// The C string at `p`, without its terminator.
///
/// # Safety
/// `p` must point at a terminated string that lives for the program, as `argv`'s do.
unsafe fn arg(p: *const u8) -> &'static [u8] {
    let mut n = 0;
    // SAFETY: the caller vouches for the terminator, so every byte read up to it is inside the string.
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: the `n` bytes before the terminator were just read.
    unsafe { core::slice::from_raw_parts(p, n) }
}

#[unsafe(no_mangle)]
extern "C" fn bench486_main(stack: *const usize) -> ! {
    // SAFETY: Linux puts `argc` at the start of the stack and `argc` pointers to terminated strings after it.
    let args: alloc::vec::Vec<&[u8]> = unsafe { (1..*stack).map(|i| arg(*stack.add(1 + i) as *const u8)).collect() };
    fpu::set_double_precision();
    let mut frame = Box::new(Frame::new());
    let has = |flag: &[u8]| args.iter().any(|a| *a == flag);
    match args.first().copied() {
        Some(b"cameras") => cameras(&mut frame),
        Some(b"script") => {
            // FIZZ2.EXE's defaults: captions on, Motion Full, the Sky's Auto (which /BENCH measures at its start, Light), by day.
            let options = DrawOptions {
                full_sky: has(b"--full"),
                night: has(b"--night"),
                ..DrawOptions::default()
            };
            script(&mut frame, &options);
        }
        _ => {
            write(2, b"usage: bench486 cameras | bench486 script [--full] [--night]\n");
            exit(2)
        }
    }
    exit(0)
}
