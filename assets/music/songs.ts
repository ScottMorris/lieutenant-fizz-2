// The song manifest: which track plays on which level, and how it is built. Swapping a song is a change to this data only.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `level` is the slice's one level. Today it is "Crater Fields", Fizz 1's track used as a PLACEHOLDER (see crater.ts); to use another
// song, add its module beside crater.ts, add an entry to `TRACKS`, and change `MANIFEST.level` to its id. The note export, the
// generated music data and the players read the song only through `songById` and `levelSong`.

import { CRATER, CRATER_DEF, CRATER_TITLE } from './crater.ts';
import { layoutFor, type SongDef, type SongLayout } from './song.ts';
import type { MusicTrack } from './types.ts';

export interface SongEntry {
  /** A short title for docs and logs. */
  title: string;
  /** Whether the song is a placeholder rather than an official Fizz 2 track. */
  placeholder: boolean;
  track: MusicTrack;
  def: SongDef;
}

/** Every song the source holds. */
export const TRACKS: Record<string, SongEntry> = {
  crater: { title: CRATER_TITLE, placeholder: true, track: CRATER, def: CRATER_DEF },
};

/** Which song plays where. */
export const MANIFEST: { level: string } = { level: 'crater' };

/** The built layout of a song by id. */
export function songById(id: string): SongLayout & { id: string; entry: SongEntry } {
  const entry = TRACKS[id];
  if (!entry) throw new Error(`assets/music/songs.ts has no song "${id}"`);
  return { id, entry, ...layoutFor(entry.track, entry.def) };
}

/** The song the slice's level plays. */
export const levelSong = () => songById(MANIFEST.level);
