# Editing Innert's models and animations

Every model in the game is built by code, and every one ships as a MagicaVoxel `.vox` file in
this folder too.

- **A file you've edited** is drawn instead of the code's model.
- **A file left as it was exported** is passed over, so the game looks exactly as the code makes
  it, and changes to the code still show.
- **Deleting a file** brings back the code's model.

## The model editor

The editor is its own app, **`innert-editor.exe`**, built beside `innert.exe`:

- `cargo build --release` builds both, into `target\release\`.
- `cargo run --release --bin innert-editor` builds and starts the editor.

It opens straight into the editor, in a window called **Innert Model Editor**. There's a plain
checkered floor and nothing else: no landscape, no creatures about, and no saves. Pick anything from
the list on the left to see it alone on the floor.

The game and the editor read the same `assets/models` and `assets/anims` folders. Keep the game
running beside the editor, or MagicaVoxel, and what you save shows up in the game at once.

## Editing a model in MagicaVoxel

1. **Write the files.** They're all here already. Run `innert --export-models` to write them again,
   for example after the code's changed, or `innert --export-models <folder>` to write them
   somewhere else. It takes about 10 seconds and opens no window. `exported.txt` lists what was
   written.

   You don't have to export everything. In the editor, pick something and press **Open in
   MagicaVoxel**. That writes just that piece, if it isn't here yet, and opens it.
2. **Edit the file in MagicaVoxel and save it.** The running game picks up the change within a
   moment, with no restart.
3. **Undo an edit** by deleting the `.vox` file. The code's model comes back.

Checking which files have been edited costs about 60 ms at startup.

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
| `<creature>/<piece>.vox` | A monster's, animal's, townsperson's or merchant's pieces, which its frames are baked from and it's posed from up close, for example `armored_skeleton/blade.vox`, `wolf/upper_fore_left.vox`, `villager/head.vox` or `merchant/arm_right.vox`. The princess and the royal guards have pieces too. See the note below. |
| `<thing>/base.vox` | Something that never moves, drawn in one frame: things lying about, chests, the Innert Gate and the portals, for example `chest/base.vox`. |
| `<creature>/walk_03.vox`, `strike.vox`, ... | One frame each, written with `--export-models --export-frames`. Each replaces its frame outright. |
| `dragon/<kind>/<piece>.vox` | A dragon's or drake's jointed pieces, for example `dragon/ashwing/head.vox` or `dragon/drake/wing_left_outer.vox`. |
| `boss/<kind>/<piece>.vox` | A jointed boss's pieces, for example `boss/golem/chest.vox` or `boss/ironhide/upper_fore_left.vox`. |
| `avian/<kind>/<piece>.vox` | An avian's or gargoyle's pieces, for example `avian/harpy/wing_left_outer.vox` or `avian/gargoyle/stone_head.vox`. |
| `serpent/<name>/head.vox`, `jaw.vox`, `body.vox` | A great serpent's head, jaw and the piece its body is made of. |
| `wings/<kind>.vox` | A pair of wings, for example `wings/gull.vox`. It's the right wing; the left is its mirror. |
| `weapons/<family>/<part>.vox` | One part of a weapon or tool. See the weapon parts section below. |
| `ammo/<id>.vox` | Each kind of special ammo in flight, by its id in `assets/weapons.json`, for example `ammo/fire_arrows.vox`. |
| `effects/<number>.vox` | The fighting's effects, numbered in the order the code makes them: arrows, bolts, shot, sparks, fireballs, smoke, pools, rings, the player's arms holding a weapon, and the rest. |
| `dragon/fx/*.vox`, `avian/fx/*.vox` | What dragons breathe and leave burning, and what avians and gargoyles throw and break into. |
| `wards/<number>.vox` | The wards round raid bosses. |
| `wings/trail_<n>.vox` | The motes the elemental wings leave behind. |

**Effects, wards, and the dragons' and avians' effects** keep the code's anchor. They can grow
round it, but not below their bottom. Their `.json` doesn't know their real voxels per meter.

**Monsters, animals, the townsfolk and the merchants are drawn in baked frames, baked from pieces,
and posed from their pieces up close.** Further off each is drawn as one model a frame (standing,
walking, each attack pose and the frames between), so dozens on screen cost no more than before.
The frames are baked from jointed pieces: a head, a torso, upper arms and forearms, thighs and
shins, a tail, what's held in a hand. Each piece is its own file, `<creature>/<piece>.vox`, with
its joint at the scene's middle.

Within 40 meters, the nearest ten or so are posed afresh every frame from those same pieces, as
smoothly as time goes, rather than stepping from one baked frame to the next: partway between the
frames of their walk, easing into and out of it, and partway between the frames of each blow. The
townsfolk and merchants breathe, look about, turn their heads to whoever comes near, and talk with
their hands. Their shadows and reflections are still their baked frames. `--set live_npcs=0` draws
every one in its frames, however near.

- **Edit a piece** and it goes with that limb in every frame: paint the skeleton's `blade.vox` red
  and the blade is red in its hand as it walks, raised over its head as it winds up, and swung down
  as it strikes. The frames are baked again as soon as you save, in the editor and in a game running
  beside it.
- **The pieces of the standing frame are cut exactly from it**, so with no files edited every
  creature's standing frame is exactly as it always was, and its other frames move as they always
  did.
- **An old `<creature>/base.vox`** (the standing frame, as files had it before there were pieces) is
  still read if you edited it. What you changed in it is put on the piece that holds that spot in
  the standing frame (a voxel added goes on the piece nearest it), so it goes with that limb too.
  Before, the export put some creatures' frames in other creatures' files: `armored_skeleton/base.vox`
  held a bandit's walking frame. The game finds which frame the file was written from by the hash in
  its `.json`, and puts what you changed where you changed it.

**Some creatures share a shape.** The Frost Colossus and the Magma Golem are the Stone Warden in
other colours, so their pieces come from `boss/golem/`. Edit the golem and all three change. Give
one of them files of its own and it's drawn from those instead. The same goes for dragons and baked
creatures alike but for colour, such as the ginger cat, drawn from `gray_cat/`; `<creature>/skins.json`
lists which material becomes which.

### Trees

Trees aren't voxel models. They're shapes the world's generator grows from the world's seed. Two
things about them can still be changed:

- **`assets/trees.json`** holds the numbers the broad-leaved woods are grown from:
  - how likely each kind is: broad, upright, spreading, birch, blossom, poplar, willow;
  - how many are young, and how big young and grown ones are;
  - each form's trunk height, limb spread and rise, sizes of its masses of leaves, flatness, trunk
    thickness and taper, limbs, and where they fork;
  - the crowns' and shrubs' leaf materials;
  - how often crowns are in flower.

  It ships with the code's numbers, so the world grows exactly as it did. Save it while the game
  runs and the next strips of the world are grown from the new numbers.
- **Hand-made trees.** Put `.vox` trees in `assets/models/trees/<land>/`, where `<land>` is `woods`,
  `jungle`, `desert` or `snow`. Then raise that land's share in `hand_made` in `trees.json`, from
  0 to 1. Each one stands on its middle, turned a quarter at random, at 16 voxels a meter. Every
  share is 0 unless you raise it, so nothing changes until you do.

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

## Editing an animation

1. In the editor, pick a dragon or drake, an avian or gargoyle, a jointed boss (the golems, the
   bears, the spiders, the knights, the giants and the rest), or a monster, animal or townsperson
   (the skeletons, zombies, bandits, wolves, ghouls, bears, cats and the rest). Everything with bones
   can be animated; only things that never move, like chests, can't.
2. Pick a clip along the bottom.
   - A dragon's are `stand`, `walk`, `fly`, and its moves.
   - A boss's are `idle`, `walk`, `run`, each of its moves, `reel` and `death`, played as the game
     plays them.
   - A monster's are `idle`, `walk`, and its three poses, `windup`, `strike` and `special`. An animal
     or a townsperson has only `idle` and `walk`, and a merchant, the princess or a royal guard only
     `idle`. A gloomwing's `walk` is its wings beating, and a rime wraith's is its rags stirring.
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
5. **Save** writes `assets/anims/dragon/<kind>/<clip>.json`, `assets/anims/boss/<kind>/<clip>.json`,
   or for a monster, animal, townsperson or merchant `assets/anims/<build>/<creature>/<clip>.json`,
   for example `assets/anims/undead/armored_skeleton/walk.json` or
   `assets/anims/keeper/merchant/idle.json`.
   - With **for all of its kind** ticked it writes `assets/anims/dragon/<clip>.json`,
     `assets/anims/boss/<clip>.json` or `assets/anims/<build>/<clip>.json` instead. That file is used
     by every dragon, boss, or creature of that build without one of its own: `undead/walk.json`
     changes the walk of every skeleton, zombie and bandit.
   - A game running beside the editor uses the file at once.

**Monsters, animals and townsfolk are baked into frames, so a clip is baked too.** Their frames are
baked again from the clip when you save it, and the game draws those frames as it always did:

- `idle` is the standing frame, at the start of the clip.
- `walk` is the 20 frames of its walk, spread over the clip.
- `windup`, `strike` and `special` each go from standing, at the start of the clip, into the pose,
  at its end. The pose is how the bones are at the end; the frames on the way into it are how they
  are partway along. Key the bone at the start too, or it's moved as much all the way along. The
  frames between two poses, as a windup goes into its strike, are partway from the one to the other.
- There's no `run`, `hurt` or `death` of their own: they run in their walking frames, played faster,
  and they're tipped over whole as they're hit and as they fall.

While a bone's picked, or a clip has changes not saved, the editor shows its pieces where your keys
put them. Pick no bone once it's saved to see the frames as the game draws them.

An animation file holds only how far each bone is turned and moved from the code's pose. The code's
own animation always plays underneath, and a clip with no file plays exactly as before. In the game
the clips follow what it's doing:

- **Dragons:** `fly` while in the air, `walk` while walking, `stand` while standing, and a move's
  clip, such as `bite` or `breath`, from when it starts getting ready for that move.
- **Bosses:** `walk` or `run` with its stride, `idle` standing, a move's clip through its windup,
  the blow and getting over it, `reel` when it's reeling, and `death` as it falls.
- **Monsters, animals and townsfolk:** their frames, baked from the clips as above.

Delete the file to go back to the code's animation.

### Other editor controls

- Drag with the left mouse button to turn the view, and scroll to zoom.
- **Space** plays or pauses.
- **Export every model** writes every file here.
- **Read every model file again** reloads them all.
- If MagicaVoxel isn't found where it's usually installed, start the game with
  `--set magicavoxel=<path to MagicaVoxel.exe>`. Otherwise the editor shows the file's path to open
  yourself.

## Jewels

`jewels/` holds a file for each kind of jewel, `melee_rare.vox` and the like, see `src/jewels.rs`.
They're drawn only as icons, in the inventory, the quick bar and a weapon's card. Unlike the rest,
their colours aren't materials: colour 1 is a jewel's body, 2 its lighter facets, 3 gold and
4 white sparks. Change them in MagicaVoxel's palette, or change the shape, and save: the game
draws its icon from the file from then on. Delete the file for the code's jewel back.
