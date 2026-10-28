#!/usr/bin/env python3
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""Draws the animated diagram of how a sound is placed, for docs/SOUND.md.

The picture shows three panels: where a chicken is across the view, the level each ear gets at every position, and how the two
levels reach each playback path. The curves are drawn with the same formula as crates/fizz-sfx/src/place.rs (Fizz 1's sound
field), in floating point, because a picture does not need the whole-number tables.

The SVG is animated with SMIL: a chicken walks across and back and the markers follow it. Its embedded script that pauses the
animation for readers who ask for reduced motion is ignored when the SVG is shown as an image, so docs/SOUND.md also links the
static PNG.

usage: python3 tools/docs/positional-sound.py            write docs/images/positional-sound.svg
       python3 tools/docs/positional-sound.py --check    regenerate in memory and fail with a diff if the file is out of date
       python3 tools/docs/positional-sound.py --png      write the SVG, then render docs/images/positional-sound.png
                                                         with rsvg-convert (paused at the first frame)
"""
import difflib
import math
import os
import shutil
import subprocess
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
SVG = os.path.join(ROOT, "docs", "images", "positional-sound.svg")
PNG = os.path.join(ROOT, "docs", "images", "positional-sound.png")

W,H=1000,1250
BG='#0b0b14'; PANEL='#14142a'; TXT='#cfe8ff'; DIM='#8aa4bd'; GRID='#2a2a55'; LC='#5aa9ff'; RC='#ffb347'; ACC='#ffd84d'
def place(dx):
    c=lambda v:max(-1,min(1,v)); d=abs(dx); pan=0.6*c(dx)
    g=1.0 if d<=1 else max(0.4,1-0.5*(d-1))
    a=(pan+1)*math.pi/4
    return pan,g,math.cos(a)*g,math.sin(a)*g


def build():
    o = []
    A = o.append
    A(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" role="img" aria-labelledby="t d" font-family="ui-monospace,Menlo,Consolas,monospace">')
    A('<title id="t">How a cluck is placed on screen</title>')
    A('<desc id="d">Three panels. First, five chicken positions across and beyond the screen. Second, the level each ear gets for every position: full within one half-screen, then fading to a floor of 0.4, with the pan capped at 0.6 so the far ear never goes silent. Third, how the left and right levels reach the OPL3 pair, an OPL2 card, the Web and the PC speaker.</desc>')
    A('<defs><marker id="ar" markerWidth="10" markerHeight="8" refX="9" refY="4" orient="auto"><path d="M0,0 L10,4 L0,8 z" fill="%s"/></marker></defs>'%DIM)
    A(f'<rect width="{W}" height="{H}" fill="{BG}"/>')
    def text(x,y,s,size=14,fill=TXT,anchor='start',weight='normal',halo=False):
        h=' stroke="#14142a" stroke-width="4" paint-order="stroke"' if halo else ''
        A(f'<text x="{x}" y="{y}" font-size="{size}" fill="{fill}" text-anchor="{anchor}" font-weight="{weight}"{h}>{s}</text>')
    # ---------- panel 1
    X0,X1=80,920; cx=500; U=(X1-X0)/6   # dx -3..3
    def px(dx): return cx+dx*U
    A(f'<rect x="20" y="20" width="960" height="300" rx="8" fill="{PANEL}"/>')
    text(40,50,'1. Where is the chicken?',20,ACC,weight='bold')
    text(40,72,'Measured in half-screens from the middle of the view, sideways (dx). 1 is the screen edge.',13,DIM)
    # zones
    A(f'<rect x="{px(-1)}" y="95" width="{px(1)-px(-1)}" height="120" fill="#1d2f1d" stroke="#5c9" stroke-width="2"/>')
    text(cx-60,112,'the screen',13,'#5c9','middle')
    text(cx,205,'full loudness (inside one half-screen)',12,'#5c9','middle')
    for a,b in ((-2.2,-1),(1,2.2)):
        A(f'<rect x="{px(a)}" y="95" width="{px(b)-px(a)}" height="120" fill="#2b2b14" opacity="0.8"/>')
    text((px(-2.2)+px(-1))/2,205,'fading',12,'#cc9','middle'); text((px(1)+px(2.2))/2,205,'fading',12,'#cc9','middle')
    for a,b in ((-3,-2.2),(2.2,3)):
        A(f'<rect x="{px(a)}" y="95" width="{px(b)-px(a)}" height="120" fill="#2a1414" opacity="0.8"/>')
    text((px(-3)+px(-2.2))/2,205,'floor 0.4',12,'#c77','middle'); text((px(2.2)+px(3))/2,205,'floor 0.4',12,'#c77','middle')
    A(f'<line x1="{cx}" y1="95" x2="{cx}" y2="215" stroke="{DIM}" stroke-dasharray="4 4"/>')
    text(cx,232,'camera',12,DIM,'middle')
    # ruler
    A(f'<line x1="{X0}" y1="250" x2="{X1}" y2="250" stroke="{DIM}"/>')
    for v in range(-3,4):
        A(f'<line x1="{px(v)}" y1="245" x2="{px(v)}" y2="255" stroke="{DIM}"/>')
        text(px(v),272,('%+d'%v if v else '0'),13,TXT,'middle')
    text(X0-40,255,'dx',13,DIM)
    positions=[('A',-2.0),('B',-1.0),('C',0.0),('D',1.0),('E',2.0)]
    for n,v in positions:
        x=px(v)
        A(f'<ellipse cx="{x}" cy="150" rx="17" ry="13" fill="#a0561f" stroke="#000"/><circle cx="{x+9}" cy="144" r="2.5" fill="#fff"/><polygon points="{x+16},148 {x+24},151 {x+16},153" fill="#ffd84d"/>')
        text(x,185,n,15,ACC,'middle',weight='bold')
    text(40,303,'Up and down count too: loudness uses whichever of dx and dy is further out, but only dx decides left and right.',12,DIM)
    # ---------- panel 2
    A(f'<rect x="20" y="340" width="960" height="425" rx="8" fill="{PANEL}"/>')
    text(40,370,'2. How loud is each ear?',20,ACC,weight='bold')
    text(40,392,'Pan = 0.6 x dx (never more than 0.6). Loudness is 1 inside the screen, then loses 0.5 per half-screen, never below 0.4.',13,DIM)
    GX0,GX1,GY0,GY1=80,920,660,420   # y: level 0 at 660, 1 at 420
    def gy(l): return GY0-(GY0-GY1)*l
    for l in (0,0.25,0.5,0.75,1):
        A(f'<line x1="{GX0}" y1="{gy(l)}" x2="{GX1}" y2="{gy(l)}" stroke="{GRID}"/>')
        text(GX0-8,gy(l)+4,('%g'%l),12,DIM,'end')
    for v in range(-3,4):
        A(f'<line x1="{px(v)}" y1="{GY1}" x2="{px(v)}" y2="{GY0}" stroke="{GRID}"/>')
        text(px(v),GY0+18,('%+d'%v if v else '0'),12,DIM,'middle')
    text(GX0-40,GY1-6,'level',12,DIM)
    text((GX0+GX1)/2,GY0+38,'dx (half-screens from the middle)',12,DIM,'middle')
    for side,col,idx in (('left ear',LC,2),('right ear',RC,3)):
        pts=[]
        N=240
        for i in range(N+1):
            v=-3+6*i/N; pts.append('%.1f,%.1f'%(px(v),gy(place(v)[idx])))
        A(f'<polyline points="{" ".join(pts)}" fill="none" stroke="{col}" stroke-width="3"/>')
    text(px(-2.85),gy(0.52),'left ear',14,LC,weight='bold'); text(px(2.85),gy(0.52),'right ear',14,RC,'end',weight='bold')
    for n,v in positions:
        x=px(v); pan,g,l,r=place(v)
        A(f'<line x1="{x}" y1="{GY1}" x2="{x}" y2="{GY0}" stroke="{ACC}" stroke-dasharray="3 5" opacity="0.7"/>')
        A(f'<circle cx="{x}" cy="{gy(l)}" r="5" fill="{LC}"/><circle cx="{x}" cy="{gy(r)}" r="5" fill="{RC}"/>')
        text(x,GY1-4,n,14,ACC,'middle',weight='bold')
        hi=max(l,r); 
        text(x-9,gy(l)+(16 if l<r else -9),'%.2f'%l,11,LC,'end',halo=True); text(x+9,gy(r)+(16 if r<l else -9),'%.2f'%r,11,RC,'start',halo=True)
    A(f'<line x1="{px(1)}" y1="{gy(0.4)}" x2="{px(3)}" y2="{gy(0.4)}" stroke="#c77" stroke-dasharray="6 4"/>')
    text(px(2.2),gy(0.4)-6,'distance floor 0.4',11,'#c77','middle')
    text(40,GY0+68,'C is dead centre: both ears at 0.71 (-3 dB), so the middle is as loud as one voice.',12,DIM)
    text(40,GY0+86,'Far from the screen the sound is quieter but never lost: at most about 10 dB down on the far ear, and the loudness never drops below 0.4.',12,DIM)
    # ---------- panel 3
    A(f'<rect x="20" y="785" width="960" height="440" rx="8" fill="{PANEL}"/>')
    text(40,815,'3. How the two levels reach your ears',20,ACC,weight='bold')
    def box(x,y,w,h,title,lines,col='#33336a'):
        A(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="#1b1b3a" stroke="{col}" stroke-width="2"/>')
        text(x+10,y+22,title,13,TXT,weight='bold')
        for i,l in enumerate(lines): text(x+10,y+42+i*17,l,12,DIM)
    box(40,835,215,300,'The rules',['Event::Cluck','  { variant, chicken }','','Only says that a','chicken clucked.','Never sees the view','or the sound: the','hash cannot change.'])
    box(290,835,225,300,'The presenter',['Looks up that chicken','and the camera, then','place(x, y, view):','  dx, pan, loudness','  left  = cos x loud','  right = sin x loud','','Same formula as','Fizz 1 (sound-field).'],ACC)
    A(f'<line x1="255" y1="985" x2="290" y2="985" stroke="{DIM}" stroke-width="2" marker-end="url(#ar)"/>')
    outs=[('DOS, OPL3 card (stereo)',['channel 5 = left (output bit 0x10)','channel 14 = right (0x20, bank 1)','the same effect on both, each at','its own level (0.75 dB steps)'],LC),
          ('DOS, AdLib or OPL2 (mono)',['channel 5 only, both speakers','loudness follows distance; no pan'],'#7c7'),
          ('Web',['an equal-power stereo panner and a','gain with the same numbers','(Classic mode: dead centre, full level)'],RC),
          ('DOS, PC speaker',['ignores placement (no volume control)'],'#c77')]
    y=835
    for i,(t,ls,col) in enumerate(outs):
        h=34+17*len(ls)+8
        box(570,y,410,h,t,[],col)
        for j,l in enumerate(ls): text(580,y+24+17*(j+1),l,12,DIM)
        A(f'<line x1="515" y1="985" x2="568" y2="{y+h/2}" stroke="{DIM}" stroke-width="2" marker-end="url(#ar)"/>')
        y+=h+12

    # ---------- animation (SMIL): one walk across and back, everything synced
    DUR=16
    fw=[-3+6*i/60 for i in range(61)]; seq=fw+fw[-2::-1]       # 121 samples, there and back
    kt=';'.join('%.5f'%(i/(len(seq)-1)) for i in range(len(seq)))
    def vals(f): return ';'.join(f(v) for v in seq)
    def anim(attr,f,extra=''):
        return f'<animate attributeName="{attr}" values="{vals(f)}" keyTimes="{kt}" dur="{DUR}s" repeatCount="indefinite" calcMode="linear"{extra}/>'
    # panel 1: the walking chicken with a cluck ring
    A(f'<g id="walker" transform="translate({px(-3):.1f},0)"><animateTransform attributeName="transform" type="translate" values="{vals(lambda v:"%.1f 0"%px(v))}" keyTimes="{kt}" dur="{DUR}s" repeatCount="indefinite" calcMode="linear"/>'
      f'<circle cx="0" cy="150" r="10" fill="none" stroke="{ACC}" stroke-width="2" opacity="0"><animate attributeName="r" values="14;46" dur="1.6s" repeatCount="indefinite"/><animate attributeName="opacity" values="0.9;0" dur="1.6s" repeatCount="indefinite"/></circle>'
      f'<ellipse cx="0" cy="150" rx="19" ry="15" fill="#e08a3c" stroke="{ACC}" stroke-width="2"/><circle cx="10" cy="144" r="3" fill="#fff"/><polygon points="18,148 28,151 18,154" fill="#ffd84d"/>'
      f'<text x="0" y="128" font-size="12" fill="{ACC}" text-anchor="middle">cluck!</text></g>')
    # panel 2: marker line and the two dots follow the chicken along the curves
    A(f'<line x1="{px(-3):.1f}" y1="{GY1}" x2="{px(-3):.1f}" y2="{GY0}" stroke="{TXT}" stroke-width="2" opacity="0.8">'
      + anim('x1',lambda v:'%.1f'%px(v)) + anim('x2',lambda v:'%.1f'%px(v)) + '</line>')
    for idx,col in ((2,LC),(3,RC)):
        A(f'<circle r="8" fill="{col}" stroke="#fff" stroke-width="2" cx="{px(-3):.1f}" cy="{gy(place(-3)[idx]):.1f}">'
          + anim('cx',lambda v:'%.1f'%px(v)) + anim('cy',lambda v,idx=idx:'%.1f'%gy(place(v)[idx])) + '</circle>')
    # panel 3: two ear meters in the presenter box
    MB=1118; MH=62
    for x,col,idx,lab in ((330,LC,2,'L'),(430,RC,3,'R')):
        A(f'<rect x="{x}" y="{MB-MH}" width="40" height="{MH}" fill="#0b0b14" stroke="{GRID}"/>')
        A(f'<rect x="{x}" y="{MB-place(-3)[idx]*MH:.1f}" width="40" height="{place(-3)[idx]*MH:.1f}" fill="{col}">'
          + anim('y',lambda v,idx=idx:'%.1f'%(MB-place(v)[idx]*MH)) + anim('height',lambda v,idx=idx:'%.1f'%(place(v)[idx]*MH)) + '</rect>')
        text(x+20,MB+14,lab,11,col,'middle',weight='bold')
    text(402,1050,'live levels',11,DIM,'middle')
    A('<script><![CDATA[(function(){var s=document.currentScript&&document.currentScript.ownerSVGElement;if(s&&window.matchMedia&&matchMedia("(prefers-reduced-motion: reduce)").matches&&s.pauseAnimations){s.pauseAnimations();}})();]]></script>')

    A('</svg>')
    return '\n'.join(o) + '\n'


def main():
    args = sys.argv[1:]
    if args not in ([], ["--check"], ["--png"]):
        sys.exit("usage: positional-sound.py [--check | --png]")
    svg = build()
    if args == ["--check"]:
        have = open(SVG, encoding="utf-8").read() if os.path.exists(SVG) else ""
        if have != svg:
            sys.stdout.writelines(difflib.unified_diff(have.splitlines(True), svg.splitlines(True), "committed", "generated", n=1))
            sys.exit("docs/images/positional-sound.svg is out of date: run python3 tools/docs/positional-sound.py")
        print("positional sound diagram: ok")
        return
    with open(SVG, "w", encoding="utf-8") as f:
        f.write(svg)
    if args == ["--png"]:
        if not shutil.which("rsvg-convert"):
            sys.exit("rsvg-convert not found: the PNG was not written")
        subprocess.run(["rsvg-convert", "-o", PNG, SVG], check=True)


if __name__ == "__main__":
    main()
