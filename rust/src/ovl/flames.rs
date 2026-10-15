//! FLAMES.OVL (translated)
//!
//! A 32-byte level-2 overlay (load segment 1a29 = 1000:a290, shared with NPC/COMBAT/...). Its
//! single entry returns the code offset just after its own body (0xa299): loading FLAMES evicts
//! whatever level-2 overlay was resident, so the caller (FONT_f032, through vector 1778:057e)
//! may use 1000:a299.. (linear 0x1a299) as a free scratch area (it copies the first 0x3000 bytes
//! of DGROUP there). No data tables, no DGROUP access.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Code-segment offset returned by `flames_entry`: the first byte after FLAMES_a290's return
/// (linear address 0x10000 + 0xa299 in emulated memory).
pub const FLAMES_SCRATCH_OFF: u16 = 0xa299;

/// orig 1000:a290 (14 bytes). Vector 1778:057e (called by FONT_f032). `PUSH DS; CALL a29e;
/// POP DS; RET` — returns AX as set by `flames_noop`, i.e. 0xa299, the free space following
/// the overlay in the level-2 region.
pub fn flames_entry(gm: &mut Game) -> u16 {
    flames_noop(gm)
}

/// orig 1000:a29e (3 bytes). `POP AX; PUSH AX; RET`: returns its own return address (0xa299).
pub fn flames_noop(gm: &mut Game) -> u16 {
    FLAMES_SCRATCH_OFF
}
