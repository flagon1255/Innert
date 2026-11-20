# Editing Innert's models and animations

Every model in the game is built by code. Any model can also be a MagicaVoxel `.vox` file in
this folder. When a file is here, the game draws it instead of the code's model. When it isn't, the
code's model is used as before.

## Editing a model in MagicaVoxel

1. **Write the files.** Run `innert --export-models` to write every model here, or
   `innert --export-models <folder>` to write them somewhere else. It takes about 10 seconds and
   opens no window. `exported.txt` lists what was written.

   You don't have to export everything. In the game, press **F7** for the model editor, pick
   something, and press **Open in MagicaVoxel**. That writes just that piece, if it isn't here yet,
   and opens it.
2. **Edit the file in MagicaVoxel and save it.** The running game picks up the change within a
   moment, with no restart.
3. **Undo an edit** by deleting the `.vox` file. The code's model comes back.

Files left as they were exported are ignored: the game draws the code's model, so changes to the
code still show. Only the files you've edited are drawn in its place, and leaving the rest here
costs little at startup.

### Colours are materials

A colour index in the palette is a game material: index 54 is red paint, index 71 is gold, and so
on. `palette.txt` lists every one. `palette.png` can be loaded as MagicaVoxel's palette.

Paint with those colours. Two kinds of colour don't exist in the game:

- indices past the last material, shown magenta;
- palette entries you've recoloured.

When the game reads the file, each of these becomes the material nearest its colour.

### Where things are

Beside each `.vox` is a `.json` written by the export. It records where the model sits in the
game:

- `pivot`: the voxel the game holds still. For a creature it's the middle of its feet. For a
  dragon's piece it's the joint.
- `origin`: where the file's first voxel sits in MagicaVoxel's scene.

The pivot is put at the middle of the scene, so in MagicaVoxel the joint is at the origin. You can
grow a model in any direction and it still sits where it did. Keep the `.json` beside its `.vox`.

### What each file is

| File | What it is |
| --- | --- |
| `<creature>/base.vox` | A creature's or boss's standing frame, for example `golem/base.vox`. See the note below. |
| `<creature>/walk_03.vox`, `strike.vox`, ... | One frame each, written with `--export-models --export-frames`. Each replaces its frame outright. |
| `dragon/<kind>/<piece>.vox` | A dragon's jointed pieces, for example `dragon/ashwing/head.vox` or `dragon/pyre_matriarch/wing_left_outer.vox`. |
| `serpent/<name>/head.vox`, `jaw.vox`, `body.vox` | A great serpent's head, jaw and the piece its body is made of. |
| `wings/<kind>.vox` | A pair of wings, for example `wings/gull.vox`. It's the right wing; the left is its mirror. |
| `weapons/<family>/<part>.vox` | One part of a weapon or tool. See the weapon parts section below. |

**Creatures and monsters are baked into frames** (standing, walking, each attack pose), so only
the standing frame is exported. Whatever you change in it is changed the same way in every other
frame. A voxel is only removed or recoloured in a frame where that frame has the same material in
the same place, so moving parts like legs keep their own shape. Added voxels go where that frame
has air.

**Some creatures share a shape.** The Frost Colossus and the Magma Golem are the Stone Warden in
other colours. Their shape is in `golem/base.vox`, and `golem/skins.json` lists which material
becomes which for each of them. Edit the golem and all three change. Give one of them its own
`base.vox` and it's drawn from that instead. The same goes for dragons alike but for colour.

### Weapon parts

Every part in `assets/weapons.json` has a file for every family it fits. That covers blades, hilts,
guards, cores, heads, shafts, foci, crystals, bindings, stocks, locks, prods, sights, barrels and
the rest, and the gathering tools too. For example: `weapons/sword/iron_blade.vox`.

- **How a part is exported.** Each part is built on its family's reference weapon, which uses the
  first parts listed for each slot. In MagicaVoxel the scene's middle is the weapon's pommel or
  foot.
- **How an edited part is used.** It's drawn on every weapon of that family that has the part. It's
  placed where the code puts that part on that weapon, so it still joins the rest.
- **What stays in code.** Where the weapon is held, its tip, muzzle and nocks: the `points` in the
  `.json`, shown for reference. Also which parts are rolled, their rarities, and their colours. If
  another part tints this one (a core's glow down a blade's fuller, say), the tint is applied to
  your shape too.
- **Parts that move.** A part drawn differently in another frame, such as a bow's limbs drawn back
  or a crossbow's lever, has its own file for that frame, for example `<part>_frame_3.vox`.
- **Parts that weren't changed.** A file identical to the code's part changes nothing.

### Limits

- **Size.** A creature's frame can be at most 255 voxels a side. Nothing can go below its feet.
  MagicaVoxel models are at most 256 a side; bigger ones are written as several models side by
  side, and read back as one.
- **Growing a dragon's or serpent's piece.** While the game runs, a grown piece is fitted into the
  box it had when the game started. Restart the game to see all of it.
- **Weapons you already carry** are rebuilt from their parts' files when the files change.

## Editing an animation in the game

1. Press **F7** and pick a dragon. Jointed rigs can be animated; dragons are the first. Other
   creatures can only be played back.
2. Pick a clip along the bottom: `stand`, `walk`, `fly`, or one of the moves.
   - **Play** plays it at the speed slider's pace.
   - Drag on the bar to scrub through it.
3. Pick a bone on the right. It lights up. Turn it about its x, y or z axis, or move it, with:
   - the sliders;
   - **Q/E**, or dragging with the right mouse button, about the axis picked;
   - **X, Y, Z** to pick the axis;
   - **G** to switch between turning and moving.

   Every change keys the bone at the current time.
4. Keys show as dots on the bar.
   - **K** keys the bone as it is now; **D** removes the nearest key.
   - The ease box sets how the bone eases from that key to the next: smooth, linear, in, out, or
     step.
5. **Save** writes `assets/anims/dragon/<kind>/<clip>.json`. With **for every dragon** ticked it
   writes `assets/anims/dragon/<clip>.json`, used by every dragon without a file of its own. Dragons
   in the game use it at once.

An animation file holds only how far each bone is turned and moved from the code's pose. The code's
own animation always plays underneath, and a clip with no file plays exactly as before. In the game
the clips follow what the dragon is doing:

- `fly` while it's in the air;
- `walk` while it walks;
- `stand` while it stands;
- a move's clip, such as `bite` or `breath`, from when it starts getting ready for that move.

Delete the file to go back to the code's animation.

### Other editor controls

- Drag with the left mouse button to turn the view, and scroll to zoom.
- **Space** plays or pauses.
- **Export every model** writes every file here.
- **Read every model file again** reloads them all.
- If MagicaVoxel isn't found where it's usually installed, start the game with
  `--set magicavoxel=<path to MagicaVoxel.exe>`. Otherwise the editor shows the file's path to open
  yourself.
