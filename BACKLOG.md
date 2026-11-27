# Innert backlog

Read this before starting any work. Items under **Queued** wait until the user says go; add new
asks there with the date and the user's own words. When an item is done, move it to **Done** with
the commit it landed in.

## Queued

1. **Remove bells** (2026-11-26, "Remove bells", then "Only Bells, that do bell sound").
   - Only the bells that make a bell sound go, not everything bell-shaped or bell-named. Find
     what plays a bell sound (the bell weapon family's attacks, the Stormbell and the like) and
     ask the user if any case is unclear.
   - Take those out everywhere: loot, merchants, class starting kits (the Priest starts with a
     bell), weapons.json, and the parts and powers only they use.
   - Saves holding one must still load: turn it into another faith weapon (a talisman or scepter)
     of the same rarity and level.

2. **Water still doesn't render right** (2026-11-26, "water still doesnt render"; first reported
   2026-11-24 with a screenshot).
   - The 2026-11-24 shot: Free Roam seed 1, a desert lake by palms. A darker teal slab of water
     stands *above* the lighter water round it, with a straight front edge and a vertical face over
     the sand.
   - **Check first which build the user ran.** Their `target\release\innert.exe` was built
     2026-11-22 21:16 and not rebuilt until 2026-11-26 20:22. So the water-step fix (c4270b0d,
     holding the flowing water to the worldgen level) was *not* in the game they played on
     2026-11-24. Ask for a fresh screenshot from the current build (rebuilt 2026-11-26) and
     reproduce that.
   - Suspects if it's still there: a strip from `take_strip()` starting at the lake level next to
     water that drained earlier; inland lakes above sea level and the edge refill in `water.comp`
     `flux()`.

3. **Flails like whips** (2026-11-26, "flails should be like whips").
   - The flail's head on its chain lashes out like a whip: longer reach, a fast snapping strike
     with the chain trailing and following through, and a crack at the end.
   - Animated, with the chain moving visibly. Keep balance roughly where it is unless the longer
     reach needs a trade-off.

4. **More lightning spells and lightning special attacks** (2026-11-26, "Add more variety of
   lighting spells, lighting specials attacks"; the user has asked for lightning several times).
   - Lightning spears stay the **Priest's** (souls-like faith miracle: forms in the hand, wind-up
     throw, hold to charge).
   - Other lightning for other weapons and classes: forking bolts, ball lightning, storm clouds,
     static fields, chain tethers, and lightning specials on melee and ranged weapons.
   - Every special animated, ranged and magic ones firing visible projectiles. Bright enough to
     read by day.

5. **A real loading screen instead of "The world takes shape..."** (2026-11-26, "Instead of
   world takes shape show loading screen, a world being built or a loading screen").
   - Today it's a plain line of text (main.rs ~5970, `"The world takes shape..."`).
   - Replace it with a proper loading screen: the world visibly being built (terrain rising,
     voxels filling in), or a designed screen with the blackletter title and progress. It must fit
     the black and white theme and cost nothing once the game runs.

6. **Finer models: a bit more detail on weapons and all models** (2026-11-26, "maybe make
   waepon models tiny bit more res? so they could have more detail, for all models").
   - Models (weapons, creatures, bosses, NPCs, props) are drawn with finer voxels than today's
     16 per metre, so they can carry more detail. The *world's* voxel size stays 16 per metre
     (standing rule: never coarser; this is about models only).
   - "Tiny bit more": start at about 1.5× (24 per metre), maybe 2× (32) for held weapons only,
     since those are seen up close.
   - **Costs to weigh first:** model memory grows about 3.4× at 1.5× and 8× at 2×, so about
     23 MB becomes ~78 MB or ~185 MB, against the 150 MB cap. Hand-held weapons are cheap,
     creatures and bosses are not. Ray steps through models get longer; measure FPS
     (GTX 960 target).
   - Every shipped .vox (including the user's edited `armored_skeleton/base.vox`) needs
     upscaling to the new grid, and MagicaVoxel editing and the innert-editor must keep working
     at the new size. Code-built shapes regenerate at the new resolution. Existing models look
     the same at first (each voxel becomes a 1.5× or 2× block), and then gain detail where
     it's added.
   - Plan: held weapons first, as a test of look and cost, then the rest if the FPS and memory
     hold.

## Loose ends (found along the way, not yet asked for)

- Distant explosions may not be drawn: the game draws at most 140 objects a frame, nearest first,
  so a cannon blast's fireball 16 m off can drop out in busy scenes.
- At 1280×600 the inventory's item card runs off the bottom of the window.
- The Beacon power lands about 19 m off when cast downward off a ledge.
- Heavenfall and the Stormbell look faint by day.
- The kraken's arms are chunky. Fish look dark underwater. There's no fishing rod.
- Glass doesn't let sunlight through.
- Demi-boss hitboxes aren't scaled with their size.
- Should pistols also get the powder-gun damage boost?
- A clean frame-time A/B, with the machine quiet, of everything merged since 2026-11-24.

## Not committed in the main checkout (the user's own)

- `src/character/studio.rs`: the paint window titled "Customize your character".
- `assets/models/armored_skeleton/base.vox`: a model edit.

## Done (latest first)

- Heading fonts show their spaces: "Start a new Realms game?" no longer reads "anew game"; the
  space is widened in Cinzel and the blackletter as they load, and the confirm button reads
  "Start new game" instead of "Start anew" (2026-11-27).

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
