# Innert backlog

Read this before starting any work. Items under **Queued** wait until the user says go; add new
asks there with the date and the user's own words. When an item is done, move it to **Done** with
the commit it landed in.

## Queued

Magic RUNNING from AVI c7055b8d, the menu map from 0b267383 (2026-11-27), each in `.claude\worktrees\<name>`.

4. **The menu's land as a map** (2026-11-27, "Could this menu be a map?", then "create a map like
   this"). RUNNING in `menumap`.
   - The voxel land beside the start-up menu becomes a map: a wider area of the world you're about
     to enter (Continue: round the saved position; New: round the seed's spawn, redrawn as the seed
     changes), towns, castles and camps marked, roads once the roads item lands, the hover naming
     the place under the cursor. Black and white, free once playing.

3. **More weapon parts, more magic** (2026-11-27, "add more weapon parts, more magic").
   RUNNING in `moremagic`.
   - New weapon parts across the weapon families, and new spells/powers for the casting classes,
     beyond the eight lightning powers just added.
   - More summoning for the Necromancer (2026-11-27, "addd more summoning capability for
     necromancer"): several summon kinds (skeleton warriors, archers, a big golem, wraiths,
     raising fresh corpses), animated rising and attacks, a cap on how many, simple commands.

## Loose ends (found along the way, not yet asked for)

- Distant explosions may not be drawn: the game draws at most 140 objects a frame, nearest first,
  so a cannon blast's fireball 16 m off can drop out in busy scenes.
- At 1280×600 the inventory's item card runs off the bottom of the window.
- The Beacon power lands about 19 m off when cast downward off a ledge.
- The Stormbell looks faint by day (bolts are now brighter by day, b2bd43e5; the jellyfish itself
  wasn't touched).
- The kraken's arms are chunky. Fish look dark underwater. There's no fishing rod.
- Glass doesn't let sunlight through.
- Demi-boss hitboxes aren't scaled with their size.
- Should pistols also get the powder-gun damage boost?
- A clean frame-time A/B, with the machine quiet, of everything merged since 2026-11-24. The
  flail's whip chain measured about 3.5 ms worse on the slowest 1% of frames under load.
- Bell *sounds* that aren't bell weapons are still in: the dusk/night warning bell, the gate and
  portal being placed, the Censer's Ember Knell, the Nova and Beacon powers, the target-mark
  chime. Ask the user whether any of those should go too.
- Water: a lake or river pool deeper than about 15.9 m inland would be held to sea level near the
  camera (its level overflows a byte). Rapids that have never flowed lack their white churn. Night
  is so dark water shows only by the moon's reflection.
- Finer models: only the weapon in hand, the arms, wolves and the Grave Knight are at 2×. Weapons
  on the ground and in stalls, foes' weapons and every other creature are at the old grid. Doing
  all creatures at 2× would be about 185 MB, over the 150 MB cap; 1.5× about 78 MB. The new wood
  grain may be too contrasty.
- The flail's new haft and head models aren't in `assets/models/exported.txt` until an
  `--export-models` run.
- The Judgment Bolt and Stormrunner aim rings are big solid red bands up close.
- Loading screen: going through a gate still freezes 0–175 ms behind black before it shows; a
  faint ring banding in the background glow.

## Not committed in the main checkout (the user's own)

- `src/character/studio.rs`: the paint window titled "Customize your character".
- `assets/models/armored_skeleton/base.vox`: a model edit.

## Done (latest first)

- Roads: gravel, sand and rocky roads between towns and from each castle to its town, timber and
  stone bridges, signposts naming where each way leads (floating labels within 14 m), towns and
  castles named, roads on the map (38f1c36b, 2026-11-27). Left: a grass patch on one bridge end,
  a small step where two roads run side by side out of a town; roads only in the normal and
  frozen lands.

- Menu and loading cubes answer the mouse: columns rise and whiten under the cursor, a click
  ripples out; the land stays beside the start-up menu (9adf54e6, 2026-11-27). Lift/brightness
  are single numbers at the top of `src/loading/touch.rs` if it wants to be stronger or softer.

- More lightning: eight new lightning powers (Stormbrand Edge, Stormrunner Slam, Thundersplit
  Arrow, Galvanic Railshot, Twin Arc, Judgment Bolt, Galvanic Torrent, Grave Lightning) across
  all six classes; every bolt bright enough to read by day; spears stay the Priest's
  (b2bd43e5, 2026-11-27).
- Finer models: the weapon in hand and the arms at twice the detail, wolves and the Grave Knight
  too; model files keep their own grid (e3bd86cb, 2026-11-27).
- Bells that toll are gone (a saved bell becomes a sceptre, the Priest starts with a censer), and
  flails lash out like whips (49c9273c, 2026-11-27).
- A real loading screen: the land you'll stand on rising in black and white as the world's built,
  with a bar and what's being made (e537a416, 2026-11-27).
- Water: falls and rapids drawn at every distance, so an upper pool no longer stands over the
  lower one as a slab; swimming and the bucket no longer find the old place's water after a
  teleport (8472ba28, 2026-11-27).
- Heading fonts show their spaces: "Start a new Realms game?" no longer reads "anew game"
  (441f59ea, 2026-11-27).
- Black and white UI, blackletter title (285498ef).
- Older builds no longer crash on newer shaders (10c6e5d3).
- Smoother NPC animation (ee9755ea).
- UI refresh (9eec71c5).
- Gun sounds and cannon blast sizes (bfad306a).
- Character painting (07ca8eca).
- Weapons, lightning magic, classes Archer, Fighter, Priest, Mage, Rogue, Necromancer (d2d468ba).
- Sharks and fish (7ff32733).
- More bosses: squids, wyrms, winged lions (1f731929).
- Water held to the worldgen level (c4270b0d).
- Deeper crevices with demi-bosses, no structures on crevices (4a6eb724).
- Sky bosses (9edaf816).
- Polish (d6a475fc).
- Powder guns hit harder at low rarity, reload just a click (77cb0c1c).
- Building materials, tools, falling trees (7147e705).
- Character creation (4095008).
- Specialist merchants (05a85b2).
- Night lights (8c1a57f).
