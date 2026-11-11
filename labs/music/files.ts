// Hands a generated file to the person at the page, and wraps one in a zip where only a zip may be saved.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Opened from a web server the lab saves files with an ordinary download link. Published as a Claude artifact, the page is in a
// frame that may not download, and saves go through the host's `downloads` capability instead, which asks first and accepts a short
// list of file types that has `zip` but not `wav`. So there a WAV is saved inside a zip, stored without compression.

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

export function crc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (let i = 0; i < data.length; i++) crc = CRC_TABLE[(crc ^ data[i]!) & 0xff]! ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

/** A zip archive holding one file, stored (not compressed). */
export function zipOf(filename: string, data: Uint8Array): Uint8Array {
  const name = new TextEncoder().encode(filename);
  const crc = crc32(data);
  const local = new DataView(new ArrayBuffer(30));
  local.setUint32(0, 0x04034b50, true);
  local.setUint16(4, 20, true);
  local.setUint16(6, 0x0800, true); // the name is UTF-8
  local.setUint32(14, crc, true);
  local.setUint32(18, data.length, true);
  local.setUint32(22, data.length, true);
  local.setUint16(26, name.length, true);
  const central = new DataView(new ArrayBuffer(46));
  central.setUint32(0, 0x02014b50, true);
  central.setUint16(4, 20, true);
  central.setUint16(6, 20, true);
  central.setUint16(8, 0x0800, true);
  central.setUint32(16, crc, true);
  central.setUint32(20, data.length, true);
  central.setUint32(24, data.length, true);
  central.setUint16(28, name.length, true);
  const end = new DataView(new ArrayBuffer(22));
  end.setUint32(0, 0x06054b50, true);
  end.setUint16(8, 1, true);
  end.setUint16(10, 1, true);
  end.setUint32(12, 46 + name.length, true);
  end.setUint32(16, 30 + name.length + data.length, true);
  const out = new Uint8Array(30 + name.length + data.length + 46 + name.length + 22);
  let at = 0;
  for (const piece of [new Uint8Array(local.buffer), name, data, new Uint8Array(central.buffer), name, new Uint8Array(end.buffer)]) {
    out.set(piece, at);
    at += piece.length;
  }
  return out;
}

interface HostDownloads {
  save(request: { filename: string; data: Blob | string }): Promise<unknown>;
}

/** The host's save capability when the page is a published artifact, else null. */
async function hostDownloads(): Promise<HostDownloads | null> {
  const claude = (globalThis as { claude?: { use?(name: string): Promise<unknown> } }).claude;
  if (!claude?.use) return null;
  try {
    return ((await claude.use('downloads')) as HostDownloads | null) ?? null;
  } catch {
    return null;
  }
}

/** Extensions the artifact host saves as they are; anything else goes inside a zip. */
const HOST_SAVES = new Set(['txt', 'json', 'zip', 'md', 'csv']);

export type Saved = { ok: true; filename: string } | { ok: false; reason: string };

/** Saves `data` as `filename`. Resolves with the name actually saved, or why nothing was. */
export async function saveFile(filename: string, data: Uint8Array | string, mime: string): Promise<Saved> {
  const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
  const host = await hostDownloads();
  if (host) {
    const extension = filename.split('.').pop() ?? '';
    const direct = HOST_SAVES.has(extension);
    const name = direct ? filename : `${filename.replace(/\.[^.]+$/, '')}.zip`;
    const body = direct ? bytes : zipOf(filename, bytes);
    try {
      await host.save({ filename: name, data: new Blob([body as BlobPart]) });
      return { ok: true, filename: name };
    } catch (error) {
      const code = (error as { code?: string }).code;
      return { ok: false, reason: code === 'declined' ? 'The save was declined.' : code === 'rate_limited' ? 'Another save is still open. Try again in a moment.' : 'This view cannot save files.' };
    }
  }
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: mime }));
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
  return { ok: true, filename };
}
