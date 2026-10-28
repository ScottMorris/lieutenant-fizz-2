# Episode 2 lore

These notes record the setting of Episode 2 and the lore proposed for its levels. The facts in the setting section come from the series plan in Fizz 1's `docs/SERIES.md`. Everything under the level headings is a proposal for the author to accept or change; none of it is canon yet.

## Setting

Episode 2 takes place on Earth. Ben "Lieutenant Fizz" Blaze is on his own. The Zargs are raiding Earth's chocolate. Mildred is back with a bigger plan: drain Earth's cocoa and fill Zargoth's rivers for good. Billy is only a voice on the comms.

Zargoth's own world is crystals, chocolate rivers, mechanical hover platforms, frozen chocolate spikes and cake buildings with cookie doors (Fizz 1's `docs/GAME_DESIGN.md`). Wherever Episode 2 shows those features on Earth, they are the raid's effect on the planet.

## Siphon Works (proposal)

Siphon Works is a real Earth chocolate factory that Mildred's crew has taken over.

- **The factory's own cocoa.** Tempering canals, cooling channels and vats are ordinary factory equipment. They explain why cocoa pools and channels exist on Earth at all.
- **The Zarg siphons.** The Zargs have sunk siphon intakes into the vats and run hoses and pipes up to their tankers. The level's boss, the Cocoa Siphon, is Mildred's tanker on treads that drains the factory.
- **The leaks.** Where an intake leaks, cocoa spills over the floors and into pools. Wherever Zarg cocoa pools, Zargoth takes root: crystal growth, frozen chocolate spikes and glowing moulds.

The level reads as Earth at the top, with steel tanks and tiled factory floors, and as increasingly Zargoth-like with depth and nearness to the intakes. The player sees Earth being turned into Zargoth.

The same lore sets the level's texturing and rules:

- Two tile families, an Earth factory family and a Zarg overgrowth family, with transition tiles where they meet.
- Sticky rims on canals, brown staining on steel and pipe residue as the factory's side of the story.
- Crystal and spike overgrowth near the leaks.
- Cocoa pools as hazards, as chocolate pools are in Fizz 1.

## Lighthouse Point (proposal)

Lighthouse Point is an Earth coast at night. A lighthouse overlooks the shore, a chocolate factory stands on it, and a Zarg saucer lifts cocoa beans from the factory roof with a tractor beam, as the README banner shows. The Milestone 1 slice is a cut-down interior of this place (`crates/fizz-ep2/src/level.rs` names it Lighthouse Point): two shafts climbed by ledges, a red door between them and a gallery in the open air with the exit. The saucers over the coast are the raid's visible edge; Siphon Works is the larger operation behind it.

## The creatures (proposal)

The chickens are Earth's locals, changed by the Zargs. The other enemies are the raiders and the creatures that came with their cocoa. Roster 2 in the sprite design canvas sets out the mix by area: chickens at about a third of the overworld, and new alien creatures everywhere else. The overworld encounter table gives the reason in one line: the locals are being displaced by what has overrun the farms and coast.

## Open questions

- Does the overgrowth recede when a siphon is shut down, or does it stay after the boss?
- Which Earth places other than the factory and the coast show the takeover?
- Does Billy's voice on the comms comment on the spreading Zargoth growth?
- Is Siphon Works the first factory level the player visits, or a later one?

## Sources

- Fizz 1: `docs/SERIES.md` (Episode 2 plan), `docs/STORY.md` and `docs/GAME_DESIGN.md`.
- The private level design canvas, with the Siphon Works and Lighthouse Point boards, and the private sprite design canvas, with the three enemy rosters.
