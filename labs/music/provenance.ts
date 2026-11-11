// The history of each tune: what the agents were asked, what they answered, and what was made of it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every tune in the lab began with a conversation between the composer (Claude, working in the repository) and one or more Haiku
// agents. This file keeps those conversations: the briefs as they were sent, the replies as they came back, and a note of each
// decision the composer took afterwards. The replies' pictures and paths are not retyped here; they are read from patterns.ts and
// paths.ts, which hold them as written, so the history cannot drift from the data the tunes are built on.
//
// The page shows a tune's history in its deck, and `write-history.ts` writes all of it to docs/MUSIC-LAB-HISTORY.md.

import { PATHS, type ScenarioPath } from './paths.ts';
import { PATTERNS, type ConceptPattern } from './patterns.ts';
import type { LabTune } from './tunes.ts';

export interface Turn {
  /** Who is speaking: the composer, or an agent by the name it had in the game. */
  speaker: string;
  /** A brief sent to an agent, an agent's reply, or a note of what the composer did next. */
  kind: 'brief' | 'reply' | 'note';
  /** Prose, a paragraph to a string. */
  text: string[];
  /** A block to show in fixed-width type: a grid or a path, exactly as it was written. */
  block?: string;
}

export const COMPOSER = 'Composer (Claude)';

// ---------- The first conversation: six patterns ----------

/** The brief sent to one Haiku agent, in full. Its reply is `PATTERNS`. */
export const PATTERN_BRIEF: string[] = [
  'You are helping seed a music composition exercise for a retro platformer game, "Lieutenant Fizz 2". No tools or file reading are needed; answer from this brief alone.',
  'The game: Ben "Lieutenant Fizz" Blaze, a kid hero with a Fizz Blaster (shoots soda bubbles), a pogo stick, a wall-kick and a grapple, alone on Earth at night. Alien Zargs led by Mildred are raiding Earth\'s chocolate: siphoning cocoa from factories into tankers to refill the chocolate rivers of their home world Zargoth. Wherever Zarg cocoa leaks, Zargoth takes root on Earth: crystals, frozen chocolate spikes, glowing moulds. The look is 16-colour EGA pixel art.',
  'Places and level types in the game: Lighthouse Point: a coast at night; a lighthouse tower climbed floor by floor (ladders, lifts, landings), a sweeping beam, a Zarg saucer lifting cocoa beans with a tractor beam. Siphon Works: a captured chocolate factory, a hub with spokes; conveyors and timed presses that set a rhythm the player must obey; vats, pipes, valves, leaks. Key-door chains: red, blue and green keys open matching doors in order; backtracking and shortcuts. Identical rooms told apart by one landmark; switch-and-gate puzzles; two stacked floors swapped by a switch. The overworld map walked between levels; secret rooms behind cracked walls. Characters and things: Zargified chickens that patrol and cluck at random, pogo bounces, wall-kicks, soda bubbles, cocoa pools (hazard), crystal growth, the Cocoa Siphon boss (a tanker on treads).',
  'Your task: choose SIX different concepts from this game and, for each, invent one abstract PATTERN that represents it. A pattern is a grid of exactly 8 rows by 16 columns drawn with only the characters `#` (filled) and `.` (empty). Read it like a tiny picture or diagram of the concept: for example the shape of a motion over time, a silhouette, a rhythm of machinery, a route through rooms. Make the six patterns clearly different from one another in shape, density and symmetry (some sparse, some busy, some rising, some repeating, some irregular). Every column should have at least zero and at most three filled cells, and each pattern should have between 10 and 30 filled cells in total.',
  'Return exactly six entries, numbered 1 to 6, each in this form and nothing else: the concept name in 2 to 5 words; "Idea:", one sentence on what the picture shows and why it stands for the concept; "Mood:", three adjectives; then the 8 lines of 16 characters. Double-check that every grid line has exactly 16 characters and every grid has exactly 8 lines.',
];

const patternReply = (pattern: ConceptPattern, number: number): Turn => ({
  speaker: 'Haiku agent',
  kind: 'reply',
  text: [`${number}. ${pattern.concept}`, `Idea: ${pattern.idea}`, `Mood: ${pattern.mood}`],
  block: pattern.grid.join('\n'),
});

/** What the composer did with each pattern, tune by tune. */
const PATTERN_NOTES: Record<string, string[]> = {
  beam: [
    'Read as sixteen eighth notes over two bars, each step of the beam sounded twice: a staircase up one octave. E dorian, for a night that is lonely but bright (its raised sixth), at 108 bpm.',
    'The tune answers the climb by walking back down, then starts the climb again a step lower; the lead’s pan follows the picture, left to right and back.',
    'Chosen as the one tune with a 7.1 mix, because a lighthouse beam is the one thing in the game that turns in a circle: the lead goes once round the speaker ring every two bars.',
  ],
  press: [
    'The picture’s bottom row was sat one step under the tonic, so each press reads tonic, the step below, and a clang an octave above that. D phrygian for the menace of its flat second, 120 bpm.',
    'The lower cell of each high pair became a second voice on the far side of the stereo field. The tune only ever moves the clang.',
  ],
  keys: [
    'Read with runs held as one note: three falling notes, an eighth, an eighth and a dotted eighth, starting off the beat, which are the picture’s three bars growing longer. A minor, 100 bpm.',
    'The first draft left the whole second half of the bar empty and failed the strong-beat check (the tune never landed on beat one or three), so a two-note pickup was added on beat three and the door’s answer moved to beat four.',
    'One note per key: the three notes sound left, middle and right.',
  ],
  crystal: [
    'The ground line became a drone on the tonic and the spikes became what the bell plays: three neighbouring notes creeping up and settling back, twice. B minor, 80 bpm, the slowest of the first eight.',
    'Each bell note has its own place in the field and a quieter echo that crosses to the other side.',
  ],
  tanker: [
    'The tank body is one long flat line, so the lead climbs two steps and hammers the fifth for ten sixteenths. The five wheels, read straight off the row beneath the body, are the low drum. C minor, 138 bpm.',
    'The tread drum pans from left to right across each bar, as if the tanker were driving past.',
  ],
  rooms: [
    'Four rooms, one cell each, all the same note; the antenna is the one note that differs. Every bar is the same room with the landmark at a different height. F lydian for the uncanny sharpened fourth, which the lower cell plays a semitone under the tune. 88 bpm.',
    'The rooms alternate left and right; only the landmark note sounds in the middle.',
  ],
  'night-map': [
    'The first combination: the three exploring pictures in one tune. The lead climbs the beam for two bars and comes down the three keys for two (the keys sat one step higher so they outline the chord); the rooms tick underneath as a pedal on the fifth. G major, 116 bpm, for the overworld.',
    'The first draft had parallel unisons between tune and bass in bars one and five and a major seventh on a strong beat in bar six; the bass was rewritten and bar six starts a step higher.',
  ],
  plan: [
    'The second combination: the three factory pictures. The presses are the bass, the tanker hammers its fifth as the lead, and every other bar a crystal spike grows out of the high bell. D minor, 132 bpm, for the last level.',
  ],
};

// ---------- The second conversation: Pathfinder ----------

/** The rules of Pathfinder, sent to all four players in the same words. */
export const PATHFINDER_RULES: string[] = [
  'We are playing a storytelling game called PATHFINDER to inspire music for a retro platformer, "Lieutenant Fizz 2". No tools or files are needed; answer from this brief alone.',
  'The game world: Ben "Lieutenant Fizz" Blaze is a kid hero alone on Earth at night, with a Fizz Blaster (soda bubbles), a pogo stick, a wall-kick and a grapple. Alien Zargs are siphoning Earth\'s chocolate; where their cocoa leaks, alien crystals, frozen chocolate spikes and glowing moulds take root. Levels are mazes: you can be high up or deep down in them.',
  'How a turn works: I give you a PLACE. You imagine one short scenario there and walk Ben through it in exactly 16 steps. At every step you say how HIGH Ben is in the room (a whole number from 0, the lowest floor, to 7, the highest ledge), what he is doing, and how TENSE the moment is (0 calm, 1 alert, 2 tense, 3 breath held). The heights become the tune of a song and the tension becomes how loud and busy it is, so the shape of your path matters.',
  'Rules of a good path: Use ACTION words only from this list: walk, climb, drop, leap, hold, ride. "hold" means Ben stays still: its height must equal the previous step\'s height. Somewhere in the path, hold for at least three steps in a row (a moment of held tension: waiting, listening, hiding). Between other steps the height changes by 1 or 2, except "drop" and "leap", which may change it by up to 4. The path should have a clear shape with a memorable opening (the first four steps are the hook of the song), a high point, and it must END within 1 of the height where it STARTED, because the song loops. Do not make a plain staircase or a zigzag; make the shape tell the story.',
];

interface PathHistory {
  /** The place as the brief gave it. */
  placeBrief: string;
  /** For a relay: the first player's half and the second player's brief. */
  relay?: {
    openerBrief: string;
    openerTitle: string;
    storySoFar: string;
    openerFeeling: string;
    handOver: string;
    finisherBrief: string;
    finalTitle: string;
    storyEnding: string;
    finisherFeeling: string;
  };
  /** The two players, in the order they played. */
  players: [string] | [string, string];
  notes: string[];
}

const PATH_HISTORY: Record<string, PathHistory> = {
  chimes: {
    players: ['Player one'],
    placeBrief:
      'PART ONE, a full path of 16 steps. PLACE: "Dripstone Deep", a wet cavern far under the chocolate factory. It is a level of one-way drops: ledges you can fall from but never climb back to, pools of spilled cocoa glowing with alien mould, crystals that chime when a drip hits them. The feeling of the level is wonder mixed with caution.',
    notes: [
      'The room was dreamed up from the one-way drop, set in a cavern because no tune yet had one.',
      'A minor at 84 bpm, the path’s floor on the tonic. The three holds while the Zarg lamp sweeps became one note held for half a bar over a change of chord, so the same note turns tense without moving.',
      'After the hook: an answer a third lower, two bars where the tune holds two notes while the counter-melody walks, and a return that reaches one step higher than the hook did. A bell plays the drips, with a crossing echo.',
    ],
  },
  lift: {
    players: ['Player three'],
    placeBrief:
      'PART ONE, a full path of 16 steps. PLACE: "Lift Nine", a tall service shaft inside the factory with slow hover-lifts that drift between floors on their own timetable. Ben must wait for a lift, ride it past a Zarg patrol, and step off at the right floor. The feeling of the level is patience and suspension: floating, weightless, a long way down.',
    notes: [
      'The room is the vertical shaft with lifts. D lydian, whose raised fourth never quite settles, for weightlessness; a low drone under everything for the hum of the shaft.',
      'This path waits twice, so the tune holds twice: the hook hangs on one note for half a bar, and the middle of the tune is a single note for a whole bar while the counter-melody drifts up and down the chord.',
      'First set at 76 bpm; the shortest form the builder could give it ended in the contrast key, so the form was pinned to the one that returns home and the tempo raised to 80 to keep the song under 80 seconds.',
    ],
  },
  valves: {
    players: ['Player two'],
    placeBrief:
      'PART ONE, a full path of 16 steps, with a run of at least three holds in a row somewhere. PLACE: "The Valve Hall", the hub of the captured chocolate factory. Three corridors (spokes) lead off the hall to three valves; after each valve Ben comes back to the hall, where a lamp lights for every valve closed. The feeling of the level is purpose and homecoming: go out, come back, a little prouder each time.',
    notes: [
      'The room is the hub and spoke. C major at 104 bpm, the only one of the six with a snare, played softly: a walking theme.',
      'The tune is built like the hall. The hook, an excursion, the hook again reaching higher (prouder), a second excursion downward, and the dominant that leads back in.',
      'The counter-melody goes out along the spokes: left in one bar, right in the next, while the tune stays in the middle. The held-tension bar was first three notes long and failed the stillness check; it became two.',
    ],
  },
  painting: {
    players: ['Player four'],
    placeBrief:
      'PART ONE, a full path of 16 steps, with a run of at least three holds in a row somewhere. PLACE: "Behind the Painting", a quiet gallery corridor in an old lighthouse keeper\'s house. A wall painting near the floor hints that a cracked wall higher up is false; behind it is a secret room full of cookies and one glowing soda can. The feeling of the level is hushed curiosity that opens into delight, then tiptoes away again.',
    notes: [
      'The room is the false wall and secret room. The path starts one step above the floor, which would have opened the tune on a second above the bass, so its floor was sat one step under the tonic and the hook opens on the tonic itself. A dorian, 92 bpm.',
      'A whole bar of one held note, fingers on the plaster, comes straight before the secret room, which is the busiest and brightest two bars of the tune and the only place the chime plays.',
      'The counter-melody answers from behind the wall: off to the right, with more room on it.',
    ],
  },
  'quick-way': {
    players: ['Player one', 'Player two'],
    placeBrief:
      'PART TWO, only the FIRST 8 steps of a path that another player will finish. PLACE: "The Long Way Round", a looping level with a shortcut: a long, hard route out through pipes and catwalks to a far switch, and a quick way home that only opens from the far end. Write the outward journey only (steps 1 to 8), end it on a cliffhanger at the far end, and write one sentence of hand-over advice for the player who will bring Ben home. Remember the finished path must end within 1 of the height where your step 1 starts.',
    relay: {
      openerBrief: '',
      openerTitle: 'Out Along the Pipes',
      storySoFar:
        'Ben has left the home shaft and worked his way out along the pipes and catwalks toward the far switch, the only thing that can open the quick way home. He reaches the far end with the switch in reach, and the catwalk starts to crack beneath him.',
      openerFeeling: 'tense, lonely, determined',
      handOver: 'Have the next player open the far switch first, then bring Ben home by the quick route so the path ends within one height of his starting height of 2.',
      finisherBrief:
        'PART TWO, finish another player\'s path. PLACE: "The Long Way Round", a looping level with a shortcut: a long, hard route out through pipes and catwalks to a far switch, and a quick way home that only opens from the far end. Another player wrote the outward journey [their title, story so far and eight steps followed]. Their hand-over advice: open the far switch first, then bring Ben home by the quick route so the path ends within one of his starting height of 2. Write steps 9 to 16 only: the switch, the shortcut opening, the fast way home. Step 9 follows from height 6. Step 16 must be at height 1, 2 or 3. Give the whole 16-step story one final title.',
      finalTitle: 'The Quick Way Home',
      storyEnding:
        'Ben slams the far switch and the catwalk breaks beneath him, so he leaps down into the shortcut and rides a cocoa lift back toward the hall. The lamps burn bright as he steps home.',
      finisherFeeling: 'Breathless, relieved, homeward',
    },
    notes: [
      'The room is the loop with a shortcut, and the relay is the level: one player goes out, another comes home. The seam between the two players falls on the barline of the hook.',
      'E minor at 118 bpm, the fastest of the six. The tune keeps climbing after the hook, reaches the far switch and holds one high note for a bar and a half while the counter-melody winds underneath; then it runs for home down the relative major.',
      'The tune sits left on the way out and crosses to the right for the run home, the counter-melody on the other side each time. Player two brought the lamps of the Valve Hall into the ending, so the two tunes share a hall.',
    ],
  },
  spans: {
    players: ['Player three', 'Player four'],
    placeBrief:
      'PART TWO, only the FIRST 8 steps of a path that another player will finish. PLACE: "The Bridge Room", an underworld pumping station built on two stacked floors. One switch decides which floor has its bridge out, so Ben keeps swapping between the upper and lower route to get across. Write the first half only (steps 1 to 8), end it at the moment the first switch has just been thrown, and write one sentence of hand-over advice for the player who will get Ben across and out. Remember the finished path must end within 1 of the height where your step 1 starts.',
    relay: {
      openerBrief: '',
      openerTitle: 'Switch Over the Gap',
      storySoFar:
        'Ben came into the pump hall to shut down the siphon, and the upper span gave out ahead of him. He dropped to the lower route and has now reached the switch that decides which floor is broken.',
      openerFeeling: 'sticky, uneasy, echoing',
      handOver: 'Throw the switch back and forth as the spans swap, climb to the upper span once it returns, and bring Ben down to the exit so the path ends between 0 and 2.',
      finisherBrief:
        'PART TWO, finish another player\'s path. PLACE: "The Bridge Room", an underworld pumping station built on two stacked floors. One switch decides which floor has its bridge out, so Ben keeps swapping between the upper and lower route to get across. Another player wrote the first half [their title, story so far and eight steps followed]. Their hand-over advice: throw the switch back and forth as the spans swap, climb to the upper span once it returns, and bring Ben down to the exit so the path ends between 0 and 2. Write steps 9 to 16 only: the bridges swapping, the crossing, the way out. Step 9 follows from height 2. Step 16 must be at height 0, 1 or 2. Give the whole 16-step story one final title.',
      finalTitle: 'Hidden Room, Swapping Spans',
      storyEnding: 'Ben swaps the spans back and forth until a route holds, then sprints down the lower route and out through the exit as the pumps fall silent.',
      finisherFeeling: 'urgent, precarious, relieved',
    },
    notes: [
      'The room is the two-floor swap. Player four’s final title carried over the hidden room from their own turn; the tune is called Swapping Spans, the half of it that belongs to this room.',
      'Both halves climb to the same broken span, so the hook says the same thing twice with different endings. D minor at 96 bpm, the path’s floor one step under the tonic.',
      'The two floors are the two melodies. The answer to the hook is the hook upside down (the lower route), and the counter-melody lives on the other floor from the tune: above it for two bars, below it for the next two, on the left when it is the upper floor and on the right when it is the lower.',
    ],
  },
};

const pathBlock = (path: ScenarioPath, from: number, to: number): string =>
  path.steps
    .slice(from, to)
    .map((step, i) => `${String(from + i + 1).padStart(2)} | ${step.height} | ${step.action.padEnd(5)} | ${step.tension} | ${step.note}`)
    .join('\n');

/** What happened to every song after it was first written. */
export const LATER: string[] = [
  'Later, every song lost its closing fill and its key lift, so each ends on the dominant in its home key and falls straight into its intro; the snare-roll build became one orchestral bar (strings, two climbing string lines, bass and timpani); and the section-change turn lost its snare roll for a timpani and a string swell.',
];

/** The conversation behind one tune, in order. */
export function historyOf(tune: LabTune): Turn[] {
  const turns: Turn[] = [];
  const say = (speaker: string, kind: Turn['kind'], text: string[], block?: string) => turns.push({ speaker, kind, text, ...(block ? { block } : {}) });
  const path = PATHS.find((p) => p.id === tune.id);
  if (path) {
    const history = PATH_HISTORY[path.id]!;
    const [first, second] = history.players;
    say(COMPOSER, 'brief', ['To all four players, the rules of the game:', ...PATHFINDER_RULES]);
    say(COMPOSER, 'brief', [`To ${first}: ${history.placeBrief}`]);
    if (!history.relay) {
      say(first, 'reply', [`TITLE: ${path.title}`, `STORY: ${path.story}`, `FEELING: ${path.feeling}`], pathBlock(path, 0, 16));
    } else {
      const relay = history.relay;
      say(first, 'reply', [`TITLE: ${relay.openerTitle}`, `STORY SO FAR: ${relay.storySoFar}`, `FEELING: ${relay.openerFeeling}`, `HAND-OVER: ${relay.handOver}`], pathBlock(path, 0, 8));
      say(COMPOSER, 'brief', [`To ${second!}: ${relay.finisherBrief}`]);
      say(second!, 'reply', [`FINAL TITLE: ${relay.finalTitle}`, `STORY ENDING: ${relay.storyEnding}`, `FEELING: ${relay.finisherFeeling}`], pathBlock(path, 8, 16));
    }
    for (const note of history.notes) say(COMPOSER, 'note', [note]);
  } else {
    say(COMPOSER, 'brief', ['To one Haiku agent:', ...PATTERN_BRIEF]);
    const used = [...new Set(tune.motifs.map((m) => m.pattern))];
    for (const id of used) {
      const index = PATTERNS.findIndex((p) => p.id === id);
      turns.push(patternReply(PATTERNS[index]!, index + 1));
    }
    for (const note of PATTERN_NOTES[tune.id] ?? []) say(COMPOSER, 'note', [note]);
  }
  for (const note of LATER) say(COMPOSER, 'note', [note]);
  return turns;
}
