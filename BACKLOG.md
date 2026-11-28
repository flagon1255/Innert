# Innert backlog

Read this before starting any work. Items under **Queued** wait until the user says go; add new
asks there with the date and the user's own words. When an item is done, move it to **Done** with
the commit it landed in.

## Queued

15. **Hand cannons held closer, without their stick** (2026-11-29, "Also, make cannons closser to
    me, remove their stick"). RUNNING in `cannonhold`.
    - And war fans further right in first person (2026-11-29, "War wind blowers  should be more
      right to screen, because it covers"); read as the war fan family. Same agent, own commit.

Still owed: a quiet-machine frame-time A/B of everything merged since 2026-11-24.

Agents' house rules for this repo: absolute paths under their own worktree only; never `git
stash` (shared across worktrees); never push; commit as flagon1255 with no trailers; hidden GPU
runs only, under the GPU lock, never while League of Legends or CS2 runs; poll for the lock rather
than ending a turn to wait.

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

- A soft run of plucked strings in place of the war-horn "fanfare" for levelling up, chests, loot
  and the like, made in code (audio.rs plucked()); the two horn recordings removed (4fdbd657,
  2026-11-29).

- Cave camps instead of the cave orbs: bandit (40%), risen-dead (30%) and empty camps by a burning
  fire, one light each, with crackle, gear and rattle sounds; and creatures near the player are
  now drawn ahead of bosses lying far off in their lairs (they used to fill the draw budget and
  hide everything near) (1b75ed85, 2026-11-28). Bandits stand rather than sit.
- Easier-found drops: a beam for every drop, rarity-coloured, 2.4–11 m tall, seen to 90 m, taller
  and edged by day, slim up close; compass marks; each drop shown as its real model (wings,
  jewels, bucket); drops never sink (6ad3d33f, 2026-11-28).
- More trees: 14 new kinds (oak, beech, maple, larch, snag, baobab, Joshua tree, kapok, banyan,
  mangrove, bald cypress, dreambloom, bloodthorn, rime tree), more variety in the old; felling
  fixed on slopes and for floating bits; falling leaves seen and on in every preset, with a
  checkbox (ef74515a, 2026-11-28). Old saves see different kinds where trees stood.
- Smoother NPCs and summons: summons posed live and eased near the camera, townsfolk and monsters
  fading between near and far posing, moves carrying on, jointed bosses' weapons swinging over
  the top (091197a4, 2026-11-28). Fish and plain serpents still baked only; monsters still burst
  on death.

- More bosses: the Winterglass Duelist, Hive Matriarch, Gloamdelver and Fenhulk; demi-bosses the
  Sand-Drinker, Rimeshade, Bog-Tyrant and Barrow-Warden; demi-boss hitboxes grow with their size;
  everything that walks leaps out of pits and holes; a boss kill plays only its own death cry
  (804b6a57, 2026-11-28). Left: a road highwayman demi-boss; boss models not at the 2× grid; the
  Matriarch replaced jungle spider/Ironhide lairs and the Duelist the Grave Knight in snowy
  castles; ravine demi-bosses by climate change lair ids in old saves.
- A slim aim ring: a thin soft-red line round the same ground, ticks on rings 2 m and up
  (1d4b6d67, 2026-11-28).
- Steadier summons: no longer vanish (draw budget, Raise Dead deleting them, left behind,
  teleports, gates), go for what the player strikes, then whoever strikes the player, then the
  nearest foe; never neutrals (1851a5a6, 2026-11-28). A summon on a cliff edge can still walk on
  air.

- The menu map and no world at launch: Innert opens on the menu in ~1.3–2.2 s (was 11–13 s), the
  menu on the left and a 480 m map of the world to be entered on the right (Continue: where the
  save left off; New: the seed in the box, redrawn as it changes; Realms: the gate), places
  named under the mouse, real colours round the cursor; the world built behind the loading
  screen on New or Continue (a1428251, 2026-11-27). New Free Roam now takes ~15 s to play (it
  used to reuse the start-up world); `--set world_at_start=1` brings back the old start.
- An Options window with key rebinding: a Controls window from the Esc menu and F1 settings,
  every action grouped, click to rebind, clashes swapped and shown, reset to defaults, kept in
  %LOCALAPPDATA%\Innert\keys.txt, prompts naming the bound keys (b9aeb6e0, 2026-11-27). Still
  saying keys in plain words: loading-screen tips, some weapon power texts ("Right click"), wing
  perks ("Shift").

- Road lamps: iron lamps with glass lanterns along the stone roads, oil lanterns on wooden poles
  along the gravel (sand only near places), within 170 m of every town and castle, along about
  half the town roads, at both ends of bridges; lit at night, no light by day (9f70b020,
  2026-11-27). Night cost about +0.2–0.5 ms near lamps on a busy machine; check on a GTX 960.
- The start-up menu without the controls list, new mode blurbs (the user's own edit, 018720ec).

- More magic and parts: 13 new powers (fire, frost, nature, earth, arcane, death, holy), a
  Necromancer kit of five summonings (Bone Phalanx, Bone Archers, Grave Colossus, Wraith Flight,
  Corpse Rising; 8 raised dead at once, right click on cooldown sends them), 46 new weapon parts
  (cd59efdb, 2026-11-27). Left: talisman cord/feathers sit inside the fist in hand; skull-focus
  candles barely show; summons can't be hurt; FPS to settle in the quiet A/B.

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
