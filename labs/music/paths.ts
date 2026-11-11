// The six Pathfinder paths: scenarios walked step by step, whose heights are the hooks of the lab's six flowing tunes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pathfinder is a game four players took turns at. Each was given a place, an environment dreamed up from one of the maze types of
// the level design boards, and walked Ben through one scenario there in sixteen steps: how high he is in the room (0 to 7), what he
// is doing, and how tense the moment is (0 calm to 3 breath held). Four paths are one player's; two are relays, the first eight
// steps by one player and the last eight by another who was handed the story so far. The words and numbers are kept as written.
//
// A path is read as a pattern like any other (patterns.ts): one filled cell a column, at the step's height. A "hold" repeats the
// height before it, so held tension becomes a held note.

import { addPatterns, type ConceptPattern } from './patterns.ts';

export type Action = 'walk' | 'climb' | 'drop' | 'leap' | 'hold' | 'ride';

export interface PathStep {
  height: number;
  action: Action;
  /** 0 calm, 1 alert, 2 tense, 3 breath held. */
  tension: number;
  note: string;
}

export interface ScenarioPath {
  id: string;
  title: string;
  /** The place the players were given, and the maze type it was dreamed up from. */
  place: string;
  mazeType: string;
  story: string;
  feeling: string;
  /** Who walked it: one player, or two in relay. */
  players: string;
  steps: PathStep[];
}

const steps = (rows: string): PathStep[] =>
  rows
    .trim()
    .split('\n')
    .map((row) => {
      const [height, action, tension, note] = row.split('|').map((cell) => cell.trim());
      return { height: Number(height), action: action as Action, tension: Number(tension), note: note! };
    });

export const PATHS: readonly ScenarioPath[] = [
  {
    id: 'chimes',
    title: 'Chimes in the Cocoa',
    place: 'Dripstone Deep, a wet cavern far under the chocolate factory: ledges you can fall from but never climb back to, pools of spilled cocoa glowing with alien mould, crystals that chime when a drip hits them.',
    mazeType: 'The one-way drop, in a cavern',
    story:
      'Ben lands on a wet ledge deep under the chocolate factory and falls through one-way drops into pools of glowing cocoa. He freezes while a Zarg lamp sweeps the water, then rides a soda bubble up to the highest crystal ledge. He drops back down to the lowest pool and leaps home to the first chime.',
    feeling: 'wondrous, wary, glimmering',
    players: 'Player one',
    steps: steps(`
4 | walk | 0 | first drip rings on crystal
2 | drop | 1 | falls into glowing cocoa pool
3 | walk | 1 | wades along mould-lit shelf
5 | leap | 1 | leaps onto a crystal that chimes
5 | hold | 2 | Zarg lamp sweeps the water
5 | hold | 3 | breath held, nothing moves
5 | hold | 3 | one drip, then silence
6 | climb | 2 | climbs spikes that shiver and creak
7 | ride | 2 | rides a soda bubble skyward
5 | drop | 2 | drops past glowing mould
3 | drop | 1 | lands on a wet shelf
2 | walk | 0 | walks among chiming crystals
0 | drop | 1 | drops to the deepest pool
2 | leap | 0 | leaps up on a ringing crystal
3 | walk | 0 | walks back toward the first chime
4 | walk | 0 | back on the first ledge`),
  },
  {
    id: 'lift',
    title: 'Patience in Lift Nine',
    place: 'Lift Nine, a tall service shaft inside the factory with slow hover-lifts that drift between floors on their own timetable.',
    mazeType: 'The vertical shaft with lifts',
    story:
      'Ben reaches Lift Nine, a tall service shaft where hover-lifts drift between floors on their own timetable. He must wait for the right lift, ride it past a Zarg patrol, and step off at the floor that leads to the exit. The shaft is quiet, but every floor is a long way down.',
    feeling: 'floating, patient, weightless',
    players: 'Player three',
    steps: steps(`
2 | walk | 0 | Ben steps onto the dim landing
3 | climb | 1 | rungs ring faintly under his boots
5 | leap | 1 | he springs for a drifting hover-lift
5 | hold | 2 | lift sways, Ben waits it out
5 | hold | 2 | the timetable is not his
5 | hold | 3 | Zarg patrol passes beneath, lights sweeping
6 | ride | 2 | lift drifts up, past the patrol
7 | ride | 2 | high point: the shaft opens below
5 | drop | 2 | lift sinks toward the right floor
4 | walk | 1 | he crosses to the sealed hatch
4 | hold | 2 | the hatch is bolted; he waits
4 | hold | 3 | a Zarg drone hums beyond
3 | drop | 2 | he drops through the grate
2 | walk | 1 | he walks toward the exit door
2 | hold | 0 | clear; he breathes out
2 | hold | 0 | the lift sighs somewhere far above`),
  },
  {
    id: 'valves',
    title: 'Lamps for the Valves',
    place: 'The Valve Hall, the hub of the captured chocolate factory: three corridors lead off the hall to three valves, and a lamp lights in the hall for every valve closed.',
    mazeType: 'Hub and spoke',
    story:
      'Ben climbs the high spoke to shut the first valve, then drops back into the hall, where its lamp flickers on. He goes down the deep spoke to shut a second valve and climbs home, proud to see two lamps burning.',
    feeling: 'purposeful, rising, homeward',
    players: 'Player two',
    steps: steps(`
2 | walk | 0 | crosses the hall to spoke
3 | climb | 1 | climbs the grated ladder up
3 | hold | 2 | a Zarg drone hums past
5 | leap | 2 | leaps the cocoa-slick gap
7 | ride | 2 | rides steam up to valve
7 | hold | 3 | breath held, wheel groans
7 | hold | 3 | braced on the wheel, it turns
7 | hold | 2 | the valve shuts with a clunk
5 | drop | 1 | drops down the steam shaft
3 | walk | 1 | walks the gantry toward hall
2 | drop | 0 | lands, and the first lamp lights
1 | walk | 1 | walks toward the low spoke
0 | drop | 2 | drops into the deep spoke
0 | hold | 2 | final turn, the valve shuts
1 | climb | 2 | climbs out of the deep spoke
2 | walk | 0 | walks home, second lamp glowing`),
  },
  {
    id: 'painting',
    title: 'Behind the Painting',
    place: 'A quiet gallery corridor in an old lighthouse keeper’s house: a wall painting near the floor hints that a cracked wall higher up is false, and behind it is a secret room.',
    mazeType: 'False wall and secret room',
    story:
      'Ben tiptoes through the quiet gallery of an old lighthouse keeper’s house, where a painting near the floor hints that a cracked wall higher up is false. He climbs past the frames to the cracked plaster and tumbles through into a secret room stacked with cookies and one glowing soda can. He tiptoes back out and leaves the gallery as quietly as he found it.',
    feeling: 'hushed, curious, delighted',
    players: 'Player four',
    steps: steps(`
1 | walk | 0 | Ben tiptoes along the gallery
1 | hold | 1 | he studies the low painting
2 | climb | 1 | he hooks the frame's edge
3 | climb | 2 | hand over hand toward the cornice
5 | leap | 2 | he leaps to the crooked shelf
5 | hold | 2 | he freezes, listening for footsteps
5 | hold | 3 | breath held over the false wall
5 | hold | 3 | fingers tracing the cracked plaster
6 | walk | 2 | he edges along the cornice
7 | climb | 2 | he reaches the cracked wall
7 | hold | 3 | he presses and the wall gives
4 | drop | 2 | he tumbles through the false wall
3 | walk | 0 | cookies stacked, one glowing can
1 | drop | 1 | he tiptoes out, glancing back
2 | walk | 0 | quiet steps past the painting
1 | walk | 0 | the gallery settles again`),
  },
  {
    id: 'quick-way',
    title: 'The Quick Way Home',
    place: 'The Long Way Round, a looping level with a shortcut: a long, hard route out through pipes and catwalks to a far switch, and a quick way home that only opens from the far end.',
    mazeType: 'The loop with a shortcut',
    story:
      'Ben has left the home shaft and worked his way out along the pipes and catwalks toward the far switch, the only thing that can open the quick way home. He reaches the far end with the switch in reach, and the catwalk starts to crack beneath him. Ben slams the far switch and the catwalk breaks beneath him, so he leaps down into the shortcut and rides a cocoa lift back toward the hall. The lamps burn bright as he steps home.',
    feeling: 'tense, lonely, determined; then breathless, relieved, homeward',
    players: 'Player one out, player two home',
    steps: steps(`
2 | walk | 0 | leaves the home shaft behind
3 | climb | 1 | climbs the first pipe rung
3 | hold | 2 | waits for a Zarg patrol
4 | climb | 2 | climbs a rusted catwalk ladder
6 | ride | 2 | rides a gust of cocoa steam
4 | drop | 3 | drops off the pipe run
5 | walk | 3 | walks the narrow catwalk
6 | climb | 3 | reaches the far switch, catwalk cracks
6 | hold | 3 | slams the far switch; hatch opens
3 | leap | 3 | leaps down as the catwalk breaks
3 | hold | 2 | lands on the grate, catches breath
4 | climb | 2 | climbs the quick shaft's rungs
5 | ride | 1 | rides a cocoa lift upward
3 | drop | 1 | drops onto the hall gantry
2 | walk | 0 | walks the gantry, lamps flicker on
3 | climb | 0 | climbs to the hall rail`),
  },
  {
    id: 'spans',
    title: 'Swapping Spans',
    place: 'The Bridge Room, an underworld pumping station built on two stacked floors: one switch decides which floor has its bridge out, so Ben keeps swapping between the upper and lower route.',
    mazeType: 'The two-floor swap, with a switch',
    story:
      'Ben came into the pump hall to shut down the siphon, and the upper span gave out ahead of him. He dropped to the lower route and has now reached the switch that decides which floor is broken. Ben swaps the spans back and forth until a route holds, then sprints down the lower route and out through the exit as the pumps fall silent.',
    feeling: 'sticky, uneasy, echoing; then urgent, precarious, relieved',
    players: 'Player three in, player four out',
    steps: steps(`
1 | walk | 0 | Ben enters, sludge underfoot
3 | climb | 1 | rungs slick with cocoa crust
5 | climb | 1 | he reaches the upper deck
6 | walk | 2 | the upper span ends in air
6 | hold | 3 | he stops, breath held
3 | drop | 2 | he drops onto the lower span
2 | walk | 2 | he walks to the switch panel
2 | hold | 3 | he throws the switch
2 | hold | 3 | the spans groan and swap places
3 | climb | 2 | he climbs toward the returning span
5 | leap | 2 | he leaps for the upper deck
6 | climb | 2 | he hauls himself onto the span
6 | hold | 3 | the switch clacks, the span wobbles
2 | drop | 3 | the span gives out again
1 | walk | 2 | he sprints down the lower route
0 | walk | 0 | the pumps fall silent`),
  },
];

/** A path as a pattern: one cell a column, at the step's height. */
export function pathPattern(path: ScenarioPath): ConceptPattern {
  const grid = Array.from({ length: 8 }, (_, row) => path.steps.map((step) => (7 - row === step.height ? '#' : '.')).join(''));
  return { id: path.id, concept: path.title, idea: path.story, mood: path.feeling, grid };
}

export const pathById = (id: string): ScenarioPath | undefined => PATHS.find((path) => path.id === id);

/**
 * A path's tension as a velocity pattern over its two bars: calm steps a little under full, held breath at full. Sampled at each
 * note of a part, it makes the part swell where the scenario tightens.
 */
export function tensionAccent(path: ScenarioPath): string {
  const level = (step: PathStep): string => (0.78 + step.tension * 0.073).toFixed(2);
  return `<[${path.steps.slice(0, 8).map(level).join(' ')}] [${path.steps.slice(8).map(level).join(' ')}]>`;
}

addPatterns(PATHS.map(pathPattern));
