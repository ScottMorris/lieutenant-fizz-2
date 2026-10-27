// The interrupt registry: a protected-mode stub per IRQ line that calls a Rust handler, and the end-of-interrupt.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from fizz-2-dos-poc1 crates/fizz2-dos-rt/src/irq.rs (itself from Desk spikes/d0-rust-dos/rt/src/irq.rs) and
// the IRQ 0 stub in fizz-2-dos-poc1 crates/fizz2-dos-rt/src/timer.rs. Changes here: the Desk HAL types (`IrqRegistry`, `Pic8259`,
// `HalError`, `Slot`) are gone; a handler is a plain `fn() -> Action`; the handler's return value says whether to acknowledge the
// interrupt or to chain to the previous handler, which the timer needs per interrupt; all state is atomics, so there is no
// `static mut`.
//
// For each of the 16 lines there is a stub in assembly:
//
// 1. The host calls the stub on a stack of its own, with CS holding the code selector and DS, ES and SS whatever the interrupted
//    program had or the host chose. The stub saves what it clobbers, loads DS and ES with the flat data selector (read through CS,
//    because DS cannot be trusted yet), and switches to a locked interrupt stack in the program's own data segment, so Rust code can
//    dereference a pointer to a local through DS.
// 2. It calls `rt_irq_dispatch(line)`, which runs the line's handler. `Action::Eoi` makes the dispatcher send the end-of-interrupt;
//    `Action::Chain` sends nothing, because the previous handler (the BIOS clock, say) acknowledges the interrupt itself.
// 3. It switches back and returns with `iretd`, or, for `Action::Chain`, jumps to the previous handler.
//
// A handler runs with interrupts off (the host delivers a hardware interrupt through an interrupt gate and the stub never enables
// them), so it is never re-entered and the one interrupt stack is enough. A handler does bounded work, allocates nothing, calls
// no DOS service and never touches the x87. Everything the stubs and handlers touch is in the program image, which `install` locks
// with DPMI 0600h before the first vector is changed.
//
// The vectors are put back by `uninstall`, and by a clean-up function on the runtime's list that runs at exit and in the panic
// handler, so a program cannot leave an interrupt pointing at code that has gone. A spurious IRQ 7 or 15 is not detected; the
// program owns lines 0 and 1, which cannot be spurious.
use crate::{cleanup, dpmi, port};
use core::arch::global_asm;
use core::ptr::addr_of;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// Bytes of the interrupt stack. The handlers (a port read and a ring push) use a few hundred.
const STACK_SIZE: usize = 8192;

#[repr(C, align(16))]
struct IrqStack([AtomicU32; STACK_SIZE / 4]);

/// What the dispatcher does when a handler returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// The handler dealt with the interrupt; send the end-of-interrupt and return to the interrupted code.
    Eoi,
    /// Pass the interrupt to the handler that was installed before ours, which acknowledges it.
    Chain,
}

/// An interrupt handler. It runs in interrupt context: see the module comment for what it may do.
pub type Handler = fn() -> Action;

/// Why `install` or `uninstall` refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The line is not one the runtime can hook (above 15, or the cascade, line 2).
    InvalidLine,
    /// The line already has a handler.
    Busy,
    /// The line has no handler of ours to remove.
    NotInstalled,
    /// The clean-up list is full.
    CleanupFull,
    /// The DPMI host refused; the code is its AX.
    Dpmi(u16),
}

impl From<dpmi::Error> for Error {
    fn from(e: dpmi::Error) -> Error {
        Error::Dpmi(e.0)
    }
}

#[unsafe(no_mangle)]
static RT_IRQ_STACK: IrqStack = IrqStack([const { AtomicU32::new(0) }; STACK_SIZE / 4]);
/// The interrupted stack: ESP, then SS (the layout `lss` loads).
#[unsafe(no_mangle)]
static RT_IRQ_SAVED: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
/// The flat data selector, where the stub can read it through CS.
#[unsafe(no_mangle)]
static RT_IRQ_DSEL: AtomicU32 = AtomicU32::new(0);
/// Nonzero when the handler that just ran asked to chain.
#[unsafe(no_mangle)]
static RT_IRQ_CHAINNOW: AtomicU32 = AtomicU32::new(0);
/// The previous handler of each line as a far pointer: 32-bit offset, then the selector (the layout of `jmp fword ptr`).
#[unsafe(no_mangle)]
static RT_IRQ_OLD: [AtomicU32; 32] = [const { AtomicU32::new(0) }; 32];
/// The registered handlers, as addresses of `Handler`s; 0 for none.
static HANDLERS: [AtomicUsize; 16] = [const { AtomicUsize::new(0) }; 16];
/// The protected-mode vector number of each installed line, or 0.
static VECTORS: [AtomicU32; 16] = [const { AtomicU32::new(0) }; 16];

global_asm!(
    ".text",
    ".macro RT_IRQ_STUB n",
    "rt_irq_stub_\\n:",
    "push eax",
    "push ecx",
    "push edx",
    "push ds",
    "push es",
    "mov eax, dword ptr cs:[{dsel}]",
    "mov ds, ax",
    "mov es, ax",
    "mov dword ptr [{saved}], esp",
    "mov word ptr [{saved} + 4], ss",
    "mov ss, ax",
    "mov esp, offset {stack} + {stack_size}",
    "cld",
    "sub esp, 12",
    "push \\n",
    "call {dispatch}",
    "add esp, 16",
    "mov dword ptr [{chainnow}], eax",
    "lss esp, fword ptr [{saved}]",
    "pop es",
    "pop ds",
    "pop edx",
    "pop ecx",
    "pop eax",
    "cmp dword ptr cs:[{chainnow}], 0",
    "jne 2f",
    "iretd",
    "2:",
    "jmp fword ptr cs:[{old} + 8 * \\n]",
    ".endm",
    ".irp n, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15",
    "RT_IRQ_STUB \\n",
    ".endr",
    ".data",
    ".global rt_irq_stub_table",
    "rt_irq_stub_table:",
    ".irp n, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15",
    ".long rt_irq_stub_\\n",
    ".endr",
    ".text",
    dsel = sym RT_IRQ_DSEL,
    saved = sym RT_IRQ_SAVED,
    stack = sym RT_IRQ_STACK,
    stack_size = const STACK_SIZE,
    dispatch = sym rt_irq_dispatch,
    chainnow = sym RT_IRQ_CHAINNOW,
    old = sym RT_IRQ_OLD,
);

unsafe extern "C" {
    static rt_irq_stub_table: [u32; 16];
    static __image_start: u8;
    static __image_end: u8;
}

/// Called by the stub, on the interrupt stack, with the flat data selector loaded. Returns 1 to chain, 0 otherwise.
#[unsafe(no_mangle)]
extern "C" fn rt_irq_dispatch(line: u32) -> u32 {
    let line = (line & 15) as usize;
    let h = HANDLERS[line].load(Ordering::Relaxed);
    let action = if h == 0 {
        Action::Eoi
    } else {
        // SAFETY: the value was stored from a `Handler` by `install`.
        let f: Handler = unsafe { core::mem::transmute::<usize, Handler>(h) };
        f()
    };
    match action {
        Action::Eoi => {
            eoi(line as u8);
            0
        }
        Action::Chain => 1,
    }
}

/// Sends the end-of-interrupt for `line` to the PIC (and to the master as well, for a line on the slave).
fn eoi(line: u8) {
    if line >= 8 {
        port::outb(0xA0, 0x20);
    }
    port::outb(0x20, 0x20);
}

static LOCKED: AtomicBool = AtomicBool::new(false);
static CLEANUP: AtomicBool = AtomicBool::new(false);

/// Locks the whole image (code, data, bss, both stacks) so the host never pages anything a handler touches.
fn lock_image() -> Result<(), Error> {
    if LOCKED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let start = dpmi::ptr_to_linear(addr_of!(__image_start));
    let end = dpmi::ptr_to_linear(addr_of!(__image_end));
    dpmi::lock(start, end - start)?;
    LOCKED.store(true, Ordering::Relaxed);
    Ok(())
}

/// The protected-mode vector of an IRQ line: the DPMI host's interrupt base for the controller it is on.
pub fn vector_of(line: u8) -> u8 {
    let (_, _, _, _, master, slave) = dpmi::version();
    let (master, slave) = (if master == 0 { 0x08 } else { master }, if slave == 0 { 0x70 } else { slave });
    if line >= 8 { slave + (line - 8) } else { master + line }
}

/// Whether `line` has a handler of ours installed.
pub fn is_installed(line: u8) -> bool {
    VECTORS.get(usize::from(line)).is_some_and(|v| v.load(Ordering::Relaxed) != 0)
}

/// Installs `handler` on `line`, remembering the previous handler so that it can be chained to and put back. The vector is also
/// put back at exit and in the panic handler.
pub fn install(line: u8, handler: Handler) -> Result<(), Error> {
    if line > 15 || line == 2 {
        return Err(Error::InvalidLine);
    }
    if is_installed(line) {
        return Err(Error::Busy);
    }
    lock_image()?;
    // The clean-up entry goes first: a full list must refuse the installation, not leave a vector that nothing puts back.
    if !CLEANUP.load(Ordering::Relaxed) {
        if !cleanup::register(restore_all) {
            return Err(Error::CleanupFull);
        }
        CLEANUP.store(true, Ordering::Relaxed);
    }
    let n = usize::from(line);
    let vector = vector_of(line);
    let (old_sel, old_off) = dpmi::get_pm_vector(vector)?;
    let cs: u16;
    // SAFETY: reading a segment register has no effect.
    unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags)) };
    // The vector still points at the old handler, so no stub for this line can run while the tables are written.
    HANDLERS[n].store(handler as usize, Ordering::Relaxed);
    RT_IRQ_OLD[2 * n].store(old_off, Ordering::Relaxed);
    RT_IRQ_OLD[2 * n + 1].store(u32::from(old_sel), Ordering::Relaxed);
    RT_IRQ_DSEL.store(u32::from(dpmi::data_selector()), Ordering::Relaxed);
    // SAFETY: `rt_irq_stub_table` is the assembler's table of the 16 stub offsets, initialised data.
    let stub = unsafe { (*addr_of!(rt_irq_stub_table))[n] };
    if let Err(e) = dpmi::set_pm_vector(vector, cs, stub) {
        HANDLERS[n].store(0, Ordering::Relaxed);
        return Err(e.into());
    }
    VECTORS[n].store(u32::from(vector), Ordering::Release);
    Ok(())
}

/// Puts the previous handler back on `line`. If the host refuses, the line stays installed so the handler keeps running (not an
/// empty table entry that swallows the interrupt) and `restore_all` can try again.
pub fn uninstall(line: u8) -> Result<(), Error> {
    if !is_installed(line) {
        return Err(Error::NotInstalled);
    }
    let n = usize::from(line);
    let vector = VECTORS[n].load(Ordering::Relaxed) as u8;
    let off = RT_IRQ_OLD[2 * n].load(Ordering::Relaxed);
    let sel = RT_IRQ_OLD[2 * n + 1].load(Ordering::Relaxed) as u16;
    dpmi::set_pm_vector(vector, sel, off)?;
    VECTORS[n].store(0, Ordering::Release);
    HANDLERS[n].store(0, Ordering::Relaxed);
    Ok(())
}

/// Puts every vector back. Safe to call twice, and from the clean-up list.
fn restore_all() {
    for line in 0..16u8 {
        if is_installed(line) {
            let _ = uninstall(line);
        }
    }
}
