// Renders a built tune to a WAV file in the browser: stereo, or 7.1 with one speaker per channel.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Undertone books the whole song on an offline audio context and encodes the result, so the file is the same notes, voices and
// humanising the lab plays, with nothing recorded from the speakers. The stereo render goes through Undertone's master limiter as
// the live player does. A limiter cannot hold eight channels, so the 7.1 render is scaled down as a whole if its peak is too hot.

import { encodeWav } from '@liminal-hq/undertone';
import type { BuiltTune } from './build.ts';
import { songPattern, type VoiceOptions } from './voices.ts';

/** Seconds rendered after the last bar so the reverb and echoes ring out. */
export const TAIL_SECONDS = 3;
/** The highest sample a rendered file may hold. */
export const CEILING = 0.94;

export interface Rendered {
  wav: Uint8Array;
  channels: number;
  seconds: number;
  /** The peak of each channel after any scaling, in the file's channel order. */
  peaks: number[];
}

/** The order of the channels in a 7.1 file. */
export const SURROUND_CHANNELS = ['Front left', 'Front right', 'Centre', 'Subwoofer', 'Side left', 'Side right', 'Rear left', 'Rear right'];

export async function renderTune(built: BuiltTune, options: VoiceOptions): Promise<Rendered> {
  const count = options.mix === 'surround' ? 8 : 2;
  const buffer = await songPattern(built, options).render({
    cycles: built.bars,
    bpm: built.tune.track.bpm,
    channels: count,
    tail: TAIL_SECONDS,
    seed: `fizz2-lab|${built.tune.id}`,
    master: count === 2,
  });
  const channels = Array.from({ length: buffer.numberOfChannels }, (_, c) => new Float32Array(buffer.getChannelData(c)));
  const peakOf = (data: Float32Array): number => {
    let peak = 0;
    for (let i = 0; i < data.length; i++) peak = Math.max(peak, Math.abs(data[i]!));
    return peak;
  };
  let peaks = channels.map(peakOf);
  const top = Math.max(...peaks);
  if (top > CEILING) {
    const scale = CEILING / top;
    for (const data of channels) for (let i = 0; i < data.length; i++) data[i]! *= scale;
    peaks = peaks.map((p) => p * scale);
  }
  const wav = encodeWav({ sampleRate: buffer.sampleRate, channels }, { bitDepth: 16, seed: built.tune.id });
  return { wav, channels: channels.length, seconds: channels[0]!.length / buffer.sampleRate, peaks };
}
