# TRIAL ONLY, to be deleted when the lab sprites are imported for good.
# Copies Ben's standing, look, run and fire frames from the pose workshop into assets/sprites/ep2-ben-lab.json.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
#   python3 -I tools/sprites/trial-import-lab.py ~/source/liminal-hq/fizz-pose-workshop/assets.json assets/sprites/ep2-ben-lab.json [assets.json of the feat/run-left branch]
#   bun tools/sprites/emit-rust.ts
import json, sys
src, out = sys.argv[1], sys.argv[2]
frames = json.load(open(src))['frames']
# The left-facing run is on the lab's unmerged feat/run-left branch: pass that branch's assets.json as a third argument.
if len(sys.argv) > 3:
    frames.update(json.load(open(sys.argv[3]))['frames'])
# Same 16 EGA letters as fizz-2; crop to the bounding box of non-transparent pixels, lose none.
def crop(g):
    rs = [i for i, r in enumerate(g) if r.strip('.')]
    cs = [j for r in g for j, c in enumerate(r) if c != '.']
    r0, r1, c0, c1 = min(rs), max(rs), min(cs), max(cs)
    return [r[c0:c1 + 1] for r in g[r0:r1 + 1]]
sprites = []
def box(grids):
    r0 = min(i for _, g in grids for i, r in enumerate(g) if r.strip('.'))
    r1 = max(i for _, g in grids for i, r in enumerate(g) if r.strip('.'))
    c0 = min(j for _, g in grids for r in g for j, c in enumerate(r) if c != '.')
    c1 = max(j for _, g in grids for r in g for j, c in enumerate(r) if c != '.')
    return r0, r1, c0, c1
def cut(g, r0, r1, c0, c1):
    # Canvases differ in width between the sets, so pad with transparent pixels to the box.
    rows = [r.ljust(c1 + 1, '.')[c0:c1 + 1] for r in g[r0:r1 + 1]]
    assert sum(c != '.' for r in rows for c in r) == sum(c != '.' for r in g for c in r)
    return rows
# ben_standRight and ben_stand (left) are each cropped alone.
g = frames['ben_standRight']['grid']
rows = cut(g, *box([('', g)]))
sprites.append(('lab_standRight', rows, None))
g = frames['ben_stand']['grid']
sprites.append(('lab_standLeft', cut(g, *box([('', g)])), None))
# The front-facing look frames (the lab's look-up-down animation) share one box, anchored on the body's centre column (11) and the
# feet line (below row 33), so Ben does not move between them. They are symmetric and are not flipped.
front = [(n, frames[f]['grid']) for n, f in (('lab_lookUp', 'ben_frontLookUp'), ('lab_bendHalf', 'ben_frontBendHalf'), ('lab_bendFull', 'ben_frontBendFull'))]
r0, r1, c0, c1 = box(front)
for n, g in front:
    sprites.append((n, cut(g, r0, r1, c0, c1), (11 - c0, 34 - r0)))
# The run, fire and left-run frames share one box (the union of their pixels) so Ben's body stays put between them. They face right.
sets = [('lab_run', 'ben_walkRight'), ('lab_fire', 'ben_fireRight')] + ([('lab_runL', 'ben_walkLeft')] if 'ben_walkLeft1' in frames else [])
grids = [(f'{pre}{i}', frames[f'{src}{i}']['grid']) for pre, src in sets for i in range(1, 7)]
r0, r1, c0, c1 = box(grids)
for name, g in grids:
    # Anchored on the canvas's centre column (12) and the feet line, so left and right frames line up.
    sprites.append((name, cut(g, r0, r1, c0, c1), (12 - c0, 35 - r0)))
note = "Trial only: Ben's standing frames (ben_stand, ben_standRight), front-facing look frames (ben_frontLookUp, ben_frontBendHalf, ben_frontBendFull) and six right-facing run frames (ben_walkRight1 to 6) and six fire frames (ben_fireRight1 to 6) and six left-facing run frames (ben_walkLeft1 to 6) from the pose workshop, cropped to their non-transparent pixels (the run and fire frames to one shared box). Not the final import."
o = ['{', '  "set": "ep2-ben-lab",', f'  "note": {json.dumps(note)},', '  "sprites": [']
for i, (name, rows, anchor) in enumerate(sprites):
    h, w = len(rows), len(rows[0])
    ax, ay = anchor or (w // 2, h)
    o += ['    {', f'      "name": "{name}",', f'      "w": {w},', f'      "h": {h},', f'      "anchor": [{ax}, {ay}],',
          '      "source": "drawn",', '      "rows": [']
    o += [f'        "{r}"' + (',' if j < h - 1 else '') for j, r in enumerate(rows)]
    o += ['      ]', '    }' + (',' if i < len(sprites) - 1 else '')]
o += ['  ]', '}', '']
open(out, 'w').write('\n'.join(o))
