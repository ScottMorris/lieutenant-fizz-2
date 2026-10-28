// Where each part of the music sits in the OPL3's stereo field: a table of output sides by role, kept as data to be tuned by ear.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The OPL3 hard-pans each channel left, right or both: there is nothing in between. Fizz 1 leans a lead 0.15 to the left and mirrors a
// quiet sine partner to the other side (`PART_PAN` and `withDepth`); here that is a centred voice plus its partner on one side, about
// 6 dB below the part's own level (the partner's gain is half the part's). The sides below are the C0 to C8 output bits, 0x10 left,
// 0x20 right and 0x30 both. Rhythm-mode drums share channels (the hi-hat and snare are channel 7) and always sit in the centre.

import type { PartRole } from '../../assets/music/types.ts';

export type Side = 'centre' | 'left' | 'right';

/** The C0 to C8 output bits of each side. */
export const OUTPUT_BITS: Record<Side, number> = { left: 0x10, right: 0x20, centre: 0x30 };

/** A fixed side, or a pair that alternates on each step of the cycle (the arpeggio's ping-pong). */
export type Placement = Side | readonly [Side, Side];

export interface RoleOutput {
  /** The part's own voice. */
  main: Placement;
  /** Its quiet partner layer, if the song has one for this role. */
  partner: Placement;
}

/**
 * The output side of each role. Low and central parts (bass, kick, snare, pad, drone) are centred, and so are their partners. The
 * lead, counter and bell keep their main voice centred and push the partner to the side Fizz 1 mirrors it to (the lead leans left
 * there, so its partner goes right). The arpeggio alternates left and right on each eighth of the bar, as `partPanAt` does, and
 * its partner takes the opposite side. A role missing from the table is centred.
 */
export const ROLE_OUTPUT: Partial<Record<PartRole, RoleOutput>> = {
  bass: { main: 'centre', partner: 'centre' },
  kick: { main: 'centre', partner: 'centre' },
  snare: { main: 'centre', partner: 'centre' },
  pad: { main: 'centre', partner: 'centre' },
  drone: { main: 'centre', partner: 'centre' },
  hats: { main: 'centre', partner: 'centre' },
  perc: { main: 'centre', partner: 'centre' },
  lead: { main: 'centre', partner: 'right' },
  counter: { main: 'centre', partner: 'left' },
  bell: { main: 'centre', partner: 'left' },
  arp: { main: ['left', 'right'], partner: ['right', 'left'] },
};

/** The step (0 or 1) of an alternating placement for a note that starts `tickInBar` ticks into a bar of `ticksPerBar`, `steps` to the bar. */
export const stepAt = (tickInBar: number, ticksPerBar: number, steps: number): number =>
  Math.floor((tickInBar / ticksPerBar) * steps + 1e-9) % 2;

/** The side a note takes. */
export function sideFor(role: PartRole | undefined, partner: boolean, tickInBar: number, ticksPerBar: number, steps: number): Side {
  const entry = role ? ROLE_OUTPUT[role] : undefined;
  if (!entry) return 'centre';
  const placement = partner ? entry.partner : entry.main;
  return typeof placement === 'string' ? placement : placement[stepAt(tickInBar, ticksPerBar, steps)]!;
}
