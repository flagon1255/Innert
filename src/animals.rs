//! Animals wandering the landscape: bears, dogs, cats and squirrels. Each is a little model made
//! of voxels, built here from a few rounded shapes, with a few frames of walking. They're drawn by
//! shaders\traverse.comp, which traces rays through their voxels, so they cast shadows and show up
//! in reflections like everything else.

use std::f64::consts::{PI, TAU};
use std::sync::atomic::{AtomicU32, Ordering};

use nalgebra::{UnitQuaternion, Vector2, Vector3};
use vulkano::buffer::BufferContents;

use crate::bosses::{self, Boss};
use crate::combat::Status;
use crate::weapons::Element;
use crate::player::WaterLevel;
use crate::worldgen::{PropKind, World, is_passable, material::*};

/// What an animal, or something lying around to pick up, looks like. Each has its own model, in
/// several frames.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Bear,
    Dog,
    GrayCat,
    GingerCat,
    Squirrel,
    Apple,
    Meat,
    Candle,
    Skull,
    Chest,
    OpenChest,
    // Bosses, see src\bosses.rs. Golems, drakes and the great bear come in kinds for the cold, the
    // desert or the jungle.
    Golem,
    Drake,
    Ironhide,
    FrostGolem,
    MagmaGolem,
    FrostWyrm,
    VenomDrake,
    Frostfang,
    Spider,
    Treant,
    Direwolf,
    WinterAlpha,
    GraveKnight,
    StormDrake,
    Rimeweaver,
    // Monsters: smaller, and many of them, going for the player on sight. Wolves roam the land,
    // the rest haunt dungeons.
    Wolf,
    Skeleton,
    ArmoredSkeleton,
    Zombie,
    ArmoredZombie,
    Spiderling,
}

/// The build of a boss or monster, which decides how it moves and fights
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Build {
    Golem,
    Drake,
    Bear,
    Spider,
    Treant,
    Wolf,
    /// Walking dead, bones or rotting flesh, maybe in armor
    Undead,
}

/// The materials a boss is made of: its body, darker parts, things on it, what glows, and one
/// more, like horns or crystals
#[derive(Clone, Copy)]
struct Skin {
    body: u8,
    dark: u8,
    accent: u8,
    glow: u8,
    extra: u8,
}

impl Look {
    pub const ALL: [Look; 32] = [
        Look::Bear,
        Look::Dog,
        Look::GrayCat,
        Look::GingerCat,
        Look::Squirrel,
        Look::Apple,
        Look::Meat,
        Look::Candle,
        Look::Skull,
        Look::Chest,
        Look::OpenChest,
        Look::Golem,
        Look::Drake,
        Look::Ironhide,
        Look::FrostGolem,
        Look::MagmaGolem,
        Look::FrostWyrm,
        Look::VenomDrake,
        Look::Frostfang,
        Look::Spider,
        Look::Treant,
        Look::Direwolf,
        Look::WinterAlpha,
        Look::GraveKnight,
        Look::StormDrake,
        Look::Rimeweaver,
        Look::Wolf,
        Look::Skeleton,
        Look::ArmoredSkeleton,
        Look::Zombie,
        Look::ArmoredZombie,
        Look::Spiderling,
    ];

    /// A boss, guarding a lair, rather than a monster or an animal
    pub fn is_boss(self) -> bool {
        self.build().is_some() && !self.is_monster()
    }

    /// A monster: it fights like a boss, but it's smaller and there are many of them about
    pub fn is_monster(self) -> bool {
        matches!(self, Look::Wolf | Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie | Look::Spiderling)
    }

    /// How a boss or monster moves and fights, None for an animal or a thing
    pub fn build(self) -> Option<Build> {
        Some(match self {
            Look::Golem | Look::FrostGolem | Look::MagmaGolem => Build::Golem,
            Look::Drake | Look::FrostWyrm | Look::VenomDrake | Look::StormDrake => Build::Drake,
            Look::Ironhide | Look::Frostfang => Build::Bear,
            Look::Spider | Look::Rimeweaver | Look::Spiderling => Build::Spider,
            Look::Treant => Build::Treant,
            Look::Direwolf | Look::WinterAlpha | Look::Wolf => Build::Wolf,
            Look::GraveKnight | Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie => Build::Undead,
            _ => return None,
        })
    }

    /// How much bigger its model is than its build's, which is made at the size of the monster
    /// where there is one
    pub fn scale(self) -> f64 {
        match self {
            Look::Direwolf | Look::WinterAlpha => 2.1,
            Look::GraveKnight => 2.3,
            Look::Spiderling => 0.5,
            _ => 1.0,
        }
    }

    /// The monster a boss calls up to help it, if it does
    pub fn brood(self) -> Option<Look> {
        match self {
            Look::Direwolf | Look::WinterAlpha => Some(Look::Wolf),
            Look::GraveKnight => Some(Look::Skeleton),
            Look::Spider | Look::Rimeweaver => Some(Look::Spiderling),
            _ => None,
        }
    }

    /// Bones rather than rotting flesh, for the walking dead
    fn bony(self) -> bool {
        matches!(self, Look::Skeleton | Look::ArmoredSkeleton | Look::GraveKnight)
    }

    /// Wearing armor, for the walking dead
    fn armored(self) -> bool {
        matches!(self, Look::ArmoredSkeleton | Look::ArmoredZombie | Look::GraveKnight)
    }

    /// What its attacks are made of: what a drake breathes, what a golem's blows leave behind
    pub fn element(self) -> Option<Element> {
        match self {
            Look::FrostGolem | Look::FrostWyrm | Look::Frostfang | Look::WinterAlpha | Look::Rimeweaver => Some(Element::Frost),
            Look::MagmaGolem | Look::Drake => Some(Element::Fire),
            Look::VenomDrake | Look::Spider | Look::Treant | Look::Spiderling | Look::Zombie | Look::ArmoredZombie => Some(Element::Poison),
            Look::GraveKnight => Some(Element::Void),
            Look::StormDrake => Some(Element::Storm),
            _ => None,
        }
    }

    /// The kind of `self`, a boss, that lives where the ground's `ground`: the cold, the desert or
    /// the jungle have their own
    pub fn for_ground(self, ground: u8) -> Look {
        let cold = matches!(ground, SNOW | ICE);
        let desert = matches!(ground, SAND | SANDSTONE | SANDSTONE_DARK | DRY_GRASS);
        let jungle = matches!(ground, JUNGLE_GRASS | VINE);
        let stony = matches!(ground, STONE | ROCK | GRAVEL);
        match self {
            Look::Golem if cold => Look::FrostGolem,
            Look::Golem if desert => Look::MagmaGolem,
            Look::Drake if cold => Look::FrostWyrm,
            Look::Drake if jungle => Look::VenomDrake,
            Look::Drake if stony => Look::StormDrake,
            Look::Ironhide if cold => Look::Frostfang,
            Look::Direwolf if cold => Look::WinterAlpha,
            Look::Spider if cold => Look::Rimeweaver,
            _ => self,
        }
    }

    fn skin(self) -> Skin {
        let make = |body, dark, accent, glow, extra| Skin { body, dark, accent, glow, extra };
        match self {
            Look::FrostGolem => make(ICE, STONE, SNOW, GLOW_FROST, CRYSTAL),
            Look::MagmaGolem => make(OBSIDIAN, BARK_DARK, GLOW_FIRE, GLOW_FIRE, GLOW_FIRE),
            Look::Drake => make(TEMPLE_RED, OBSIDIAN, CLOTH, GLOW_FIRE, BONE),
            Look::FrostWyrm => make(ICE, STEEL, LINEN, GLOW_FROST, CRYSTAL),
            Look::VenomDrake => make(PINE, OBSIDIAN, MOSS, GLOW_POISON, BONE),
            Look::Ironhide => make(FUR_DARK, FUR_BROWN, DARK_STEEL, GLOW_BLOOD, BONE),
            Look::Frostfang => make(FUR_WHITE, FUR_GRAY, STEEL, GLOW_FROST, ICE),
            Look::Spider => make(FUR_DARK, BARK_DARK, GLOW_POISON, GLOW_BLOOD, BONE),
            // Dark enough to stand out against the snow it lives on
            Look::Rimeweaver => make(STEEL, OBSIDIAN, GLOW_FROST, GLOW_FROST, ICE),
            Look::Spiderling => make(FUR_BROWN, BARK_DARK, GLOW_POISON, GLOW_BLOOD, BONE),
            Look::Treant => make(BARK_DARK, WOOD, MOSS, GLOW_POISON, LEAVES),
            Look::StormDrake => make(STEEL, DARK_STEEL, LINEN, GLOW_STORM, BONE),
            Look::Direwolf => make(FUR_DARK, FUR_GRAY, FUR_GRAY, GLOW_BLOOD, BONE),
            Look::WinterAlpha => make(FUR_GRAY, FUR_DARK, FUR_WHITE, GLOW_FROST, ICE),
            Look::Wolf => make(FUR_GRAY, FUR_DARK, FUR_CREAM, GLOW_FIRE, BONE),
            // The walking dead: their bones or skin, rags, armor, the light in their eyes, and
            // their blade. A knight's bones in black armor, void burning in its eyes.
            Look::GraveKnight => make(BONE, BARK_DARK, DARK_STEEL, GLOW_VOID, DARK_STEEL),
            Look::Skeleton => make(BONE, BARK_DARK, CLOTH, GLOW_FROST, STEEL),
            Look::ArmoredSkeleton => make(BONE, BARK_DARK, STEEL, GLOW_FROST, STEEL),
            Look::Zombie => make(MOSS, PINE, LINEN, GLOW_POISON, BONE),
            Look::ArmoredZombie => make(MOSS, PINE, DARK_STEEL, GLOW_POISON, BONE),
            _ => make(ROCK, GRAVEL, MOSS, GLOW_FROST, CRYSTAL),
        }
    }

    pub fn of_prop(kind: PropKind) -> Look {
        match kind {
            PropKind::Candle => Look::Candle,
            PropKind::Skull => Look::Skull,
            PropKind::Chest => Look::Chest,
        }
    }

    /// Voxels per meter of its model: the same size as the world's, like everything else
    fn resolution(self) -> f64 {
        MODEL_RES
    }

    /// Radius of a sphere around the whole model wherever it's turned, in meters
    pub fn reach(self) -> f64 {
        match self {
            Look::Chest | Look::OpenChest => 0.95,
            Look::Bear => 1.6,
            Look::Dog => 0.8,
            Look::GrayCat | Look::GingerCat => 0.45,
            Look::Squirrel => 0.3,
            Look::Apple => 0.14,
            Look::Meat => 0.22,
            Look::Candle => 0.2,
            Look::Skull => 0.2,
            Look::Golem | Look::FrostGolem | Look::MagmaGolem => 4.4,
            Look::Drake | Look::FrostWyrm | Look::VenomDrake | Look::StormDrake => 3.4,
            Look::Ironhide | Look::Frostfang => 3.4,
            Look::Spider | Look::Rimeweaver | Look::Spiderling => 3.1 * self.scale(),
            Look::Treant => 5.5,
            Look::Wolf | Look::Direwolf | Look::WinterAlpha => 1.2 * self.scale(),
            // A blade raised overhead
            Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie | Look::GraveKnight => 2.5 * self.scale(),
        }
    }

    /// The body to aim at: how high its middle is, and how big it is, in meters
    pub fn body(self) -> (f64, f64) {
        match self {
            Look::Bear => (0.85, 0.75),
            Look::Dog => (0.5, 0.35),
            Look::GrayCat | Look::GingerCat => (0.28, 0.22),
            Look::Squirrel => (0.14, 0.16),
            Look::Apple => (0.06, 0.2),
            Look::Meat => (0.05, 0.25),
            Look::Candle => (0.1, 0.1),
            Look::Skull => (0.1, 0.12),
            Look::Chest | Look::OpenChest => (0.22, 0.4),
            Look::Golem | Look::FrostGolem | Look::MagmaGolem => (2.0, 1.1),
            Look::Drake | Look::FrostWyrm | Look::VenomDrake | Look::StormDrake => (1.25, 1.0),
            Look::Ironhide | Look::Frostfang => (1.6, 1.05),
            Look::Spider | Look::Rimeweaver | Look::Spiderling => (0.9 * self.scale(), 0.95 * self.scale()),
            Look::Treant => (2.4, 1.2),
            Look::Wolf | Look::Direwolf | Look::WinterAlpha => (0.55 * self.scale(), 0.42 * self.scale()),
            Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie | Look::GraveKnight => (1.0 * self.scale(), 0.36 * self.scale()),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Look::Bear => "bear",
            Look::Dog => "dog",
            Look::GrayCat | Look::GingerCat => "cat",
            Look::Squirrel => "squirrel",
            Look::Apple => "apple",
            Look::Meat => "meat",
            Look::Candle => "candle",
            Look::Skull => "skull",
            Look::Chest | Look::OpenChest => "chest",
            Look::Golem => "Stone Warden",
            Look::Drake => "Ember Drake",
            Look::Ironhide => "Ironhide",
            Look::FrostGolem => "Frost Colossus",
            Look::MagmaGolem => "Magma Golem",
            Look::FrostWyrm => "Frost Wyrm",
            Look::VenomDrake => "Venom Drake",
            Look::Frostfang => "Frostfang",
            Look::Spider => "Broodmother",
            Look::Treant => "Ancient Treant",
            Look::Direwolf => "Alpha Direwolf",
            Look::WinterAlpha => "Winter Alpha",
            Look::GraveKnight => "Grave Knight",
            Look::StormDrake => "Thunder Drake",
            Look::Rimeweaver => "Rimeweaver",
            Look::Wolf => "wolf",
            Look::Skeleton => "skeleton",
            Look::ArmoredSkeleton => "armoured skeleton",
            Look::Zombie => "zombie",
            Look::ArmoredZombie => "armoured zombie",
            Look::Spiderling => "cave spider",
        }
    }

    /// Pets get petted, the rest can be hunted for meat
    pub fn is_pet(self) -> bool {
        matches!(self, Look::Dog | Look::GrayCat | Look::GingerCat)
    }

    /// How much damage it takes to hunt. A bare-handed hit does PUNCH.
    pub fn health(self) -> f64 {
        match self {
            Look::Bear => 140.0,
            Look::Dog => 60.0,
            Look::GrayCat | Look::GingerCat => 30.0,
            Look::Golem => 2600.0,
            Look::FrostGolem => 2800.0,
            Look::MagmaGolem => 3000.0,
            Look::Drake | Look::VenomDrake => 2200.0,
            Look::FrostWyrm => 2400.0,
            Look::Ironhide => 2000.0,
            Look::Frostfang => 2200.0,
            Look::Spider => 1800.0,
            Look::Treant => 3200.0,
            Look::Direwolf => 2000.0,
            Look::WinterAlpha => 2200.0,
            Look::GraveKnight => 3400.0,
            Look::StormDrake => 2300.0,
            Look::Rimeweaver => 2000.0,
            Look::Wolf => 70.0,
            Look::Skeleton => 150.0,
            Look::ArmoredSkeleton => 240.0,
            Look::Zombie => 170.0,
            Look::ArmoredZombie => 270.0,
            Look::Spiderling => 40.0,
            _ => 20.0,
        }
    }

    /// How much meat it gives

    pub fn meat(self) -> u32 {
        match self {
            Look::Bear => 3,
            Look::Squirrel | Look::Wolf => 1,
            Look::Ironhide | Look::Frostfang | Look::Direwolf | Look::WinterAlpha => 6,
            _ => 0,
        }
    }

    // Speeds in meters per second
    pub fn walk_speed(self) -> f64 {
        match self {
            Look::Wolf => return 1.8,
            Look::Direwolf | Look::WinterAlpha => return 2.2,
            Look::Zombie | Look::ArmoredZombie => return 0.8,
            Look::Skeleton | Look::ArmoredSkeleton => return 1.4,
            Look::Spiderling => return 2.6,
            _ => {}
        }
        match self.build() {
            Some(Build::Golem) => 1.3,
            Some(Build::Drake | Build::Bear) => 1.6,
            Some(Build::Spider) => 2.4,
            Some(Build::Treant) => 0.9,
            Some(Build::Wolf) => 1.8,
            Some(Build::Undead) => 1.2,
            None => match self {
                Look::Bear => 1.1,
                Look::Dog => 1.6,
                Look::Squirrel => 1.4,
                _ => 0.9,
            },
        }
    }

    pub fn run_speed(self) -> f64 {
        match self {
            Look::Wolf => return 6.5,
            Look::Direwolf | Look::WinterAlpha => return 8.0,
            Look::Skeleton => return 3.2,
            Look::ArmoredSkeleton | Look::GraveKnight => return 2.8,
            // Shambling, but quicker than they look
            Look::Zombie => return 2.6,
            Look::ArmoredZombie => return 2.2,
            Look::Spiderling => return 7.0,
            _ => {}
        }
        match self.build() {
            Some(Build::Golem) => 2.4,
            Some(Build::Drake) => 4.5,
            // Charging
            Some(Build::Bear) => 10.0,
            Some(Build::Spider) => 6.0,
            Some(Build::Treant) => 1.6,
            Some(Build::Wolf) => 6.5,
            Some(Build::Undead) => 3.0,
            None => match self {
                Look::Bear => 3.2,
                Look::Dog => 4.5,
                Look::Squirrel => 4.0,
                _ => 3.0,
            },
        }
    }

    /// Meters covered by one cycle of its walk
    pub fn stride(self) -> f64 {
        match self.build() {
            Some(Build::Golem) => 2.4,
            Some(Build::Drake | Build::Bear) => 2.6,
            Some(Build::Spider) => 1.8 * self.scale(),
            Some(Build::Treant) => 2.8,
            Some(Build::Wolf) => 1.4 * self.scale(),
            Some(Build::Undead) => 1.3 * self.scale(),
            None => match self {
                Look::Bear => 1.5,
                Look::Dog => 0.9,
                Look::Squirrel => 0.35,
                _ => 0.55,
            },
        }
    }

    /// Highest ledge it can step up, in meters
    fn climb(self) -> f64 {
        match self {
            Look::Dog => 0.4,
            _ if self.is_boss() => 1.0,
            _ => 0.5,
        }
    }
}

/// Frame 0 stands still, 1 to WALK_FRAMES are a walk cycle, then come a boss's poses as it
/// attacks, see POSE_WINDUP and the rest, and frames halfway between them to ease from one to the
/// next, see tween(). Only bosses have the poses.
const WALK_FRAMES: usize = 12;
/// Getting ready to attack
pub const POSE_WINDUP: usize = WALK_FRAMES + 1;
/// Bringing it down
pub const POSE_STRIKE: usize = WALK_FRAMES + 2;
/// Something each boss does its own way: throwing, spreading its wings or lowering its head to charge
pub const POSE_SPECIAL: usize = WALK_FRAMES + 3;
/// The pairs of poses with frames between them, 0 being standing, and how many frames
const TWEENS: [(usize, usize); 5] = [(0, POSE_WINDUP), (0, POSE_SPECIAL), (POSE_WINDUP, POSE_STRIKE), (POSE_SPECIAL, POSE_STRIKE), (POSE_STRIKE, 0)];
const TWEEN_STEPS: usize = 2;
const FRAMES: usize = POSE_SPECIAL + 1 + TWEENS.len() * TWEEN_STEPS;

/// The frame `f` of the way from pose `a` to pose `b`, 0 being standing, if there are frames
/// between them
pub fn tween(a: usize, b: usize, f: f64) -> Option<usize> {
    let (k, f) = match TWEENS.iter().position(|&pair| pair == (a, b)) {
        Some(k) => (k, f),
        None => (TWEENS.iter().position(|&pair| pair == (b, a))?, 1.0 - f),
    };
    let step = ((f * (TWEEN_STEPS + 1) as f64).floor() as usize).clamp(1, TWEEN_STEPS);
    Some(POSE_SPECIAL + 1 + k * TWEEN_STEPS + step - 1)
}

/// How far a boss is into each of its poses, 0 to 1, so it can be drawn partway between them
#[derive(Clone, Copy)]
struct Pose {
    windup: f64,
    strike: f64,
    special: f64,
}

impl Pose {
    const REST: Pose = Pose { windup: 0.0, strike: 0.0, special: 0.0 };

    /// The pose drawn in `frame`, see POSE_WINDUP and tween()
    fn of(frame: usize) -> Pose {
        match frame {
            POSE_WINDUP => Pose { windup: 1.0, ..Pose::REST },
            POSE_STRIKE => Pose { strike: 1.0, ..Pose::REST },
            POSE_SPECIAL => Pose { special: 1.0, ..Pose::REST },
            _ if frame > POSE_SPECIAL => {
                // Partway from one pose of a pair to the other
                let k = frame - POSE_SPECIAL - 1;
                let (pair, step) = (TWEENS[k / TWEEN_STEPS], k % TWEEN_STEPS + 1);
                let t = step as f64 / (TWEEN_STEPS + 1) as f64;
                let (a, b) = (Pose::of(pair.0), Pose::of(pair.1));
                let mix = |x: f64, y: f64| x + (y - x) * t;
                Pose { windup: mix(a.windup, b.windup), strike: mix(a.strike, b.strike), special: mix(a.special, b.special) }
            }
            _ => Pose::REST,
        }
    }

    /// What's `rest` standing, `windup`, `strike` and `special` in each pose, in this one
    fn mix(self, rest: f64, windup: f64, strike: f64, special: f64) -> f64 {
        rest + self.windup * (windup - rest) + self.strike * (strike - rest) + self.special * (special - rest)
    }

    /// Where something that's at `rest` standing and at the others in each pose is in this one
    fn at(self, rest: Vec3, windup: Vec3, strike: Vec3, special: Vec3) -> Vec3 {
        [0, 1, 2].map(|i| self.mix(rest[i], windup[i], strike[i], special[i]))
    }
}

/// Voxels per meter of everything drawn like an animal, weapons and leaves too: the same as the
/// world's, so every voxel is the same size
pub const MODEL_RES: f64 = 16.0;

type Vec3 = [f64; 3];

/// A capsule whose radius goes from `ra` at `a` to `rb` at `b`, stretched by `stretch` around its
/// middle line so it can be an ellipsoid too. Or with `boxed`, the box with corners `a` and `b`.
/// Earlier shapes win where they overlap.
#[derive(Clone, Copy)]
pub struct Part {
    a: Vec3,
    b: Vec3,
    ra: f64,
    rb: f64,
    stretch: Vec3,
    boxed: bool,
    material: u8,
    /// Darker bands across the body, every few voxels along x
    stripes: u8,
}

pub fn ball(c: Vec3, r: f64, material: u8) -> Part {
    Part { a: c, b: c, ra: r, rb: r, stretch: [1.0; 3], boxed: false, material, stripes: AIR }
}

pub fn ellipsoid(c: Vec3, radii: Vec3, material: u8) -> Part {
    let r = radii[0].min(radii[1]).min(radii[2]);
    Part { a: c, b: c, ra: r, rb: r, stretch: radii.map(|v| v / r), boxed: false, material, stripes: AIR }
}

pub fn limb(a: Vec3, b: Vec3, ra: f64, rb: f64, material: u8) -> Part {
    Part { a, b, ra, rb, stretch: [1.0; 3], boxed: false, material, stripes: AIR }
}

pub fn cuboid(lo: Vec3, hi: Vec3, material: u8) -> Part {
    Part { a: lo, b: hi, ra: 0.0, rb: 0.0, stretch: [1.0; 3], boxed: true, material, stripes: AIR }
}

impl Part {
    fn striped(mut self, stripes: u8) -> Self {
        self.stripes = stripes;
        self
    }

    /// `s` times as big, and as far from the origin
    pub fn scaled(&self, s: f64) -> Part {
        Part { a: self.a.map(|v| v * s), b: self.b.map(|v| v * s), ra: self.ra * s, rb: self.rb * s, ..*self }
    }

    /// Squashed or stretched around its middle, `stretch` times as big along each axis
    pub fn stretched(mut self, stretch: Vec3) -> Self {
        self.stretch = [0, 1, 2].map(|i| self.stretch[i] * stretch[i]);
        self
    }

    /// At least a voxel thick at `res` voxels per meter whichever way it's turned, so nothing
    /// thin falls between the voxels
    fn thickened(&self, res: f64) -> Part {
        let (a, b) = (self.a, self.b);
        if self.boxed {
            let half = 0.51 / res;
            let mut lo = a;
            let mut hi = b;
            for i in 0..3 {
                if hi[i] - lo[i] < 2.0 * half {
                    let mid = (lo[i] + hi[i]) * 0.5;
                    lo[i] = mid - half;
                    hi[i] = mid + half;
                }
            }
            return Part { a: lo, b: hi, ..*self };
        }
        let least = 0.72 / res;
        let thinnest = self.ra.min(self.rb).max(1e-6);
        let (ra, rb) = (self.ra.max(least), self.rb.max(least));
        let k = ra.min(rb);
        let stretch = self.stretch.map(|s| (s * thinnest / k).max(least / k));
        Part { a, b, ra, rb, stretch, ..*self }
    }

    fn contains(&self, p: Vec3) -> bool {
        if self.boxed {
            return (0..3).all(|i| p[i] >= self.a[i] && p[i] <= self.b[i]);
        }
        // Squash the point towards the middle so stretched shapes become plain capsules
        let mid = [0, 1, 2].map(|i| (self.a[i] + self.b[i]) * 0.5);
        let q = [0, 1, 2].map(|i| mid[i] + (p[i] - mid[i]) / self.stretch[i]);
        let ab = [0, 1, 2].map(|i| self.b[i] - self.a[i]);
        let aq = [0, 1, 2].map(|i| q[i] - self.a[i]);
        let length2: f64 = ab.iter().map(|v| v * v).sum();
        let t = if length2 > 0.0 { (aq.iter().zip(&ab).map(|(a, b)| a * b).sum::<f64>() / length2).clamp(0.0, 1.0) } else { 0.0 };
        let r = self.ra + (self.rb - self.ra) * t;
        let d2: f64 = (0..3).map(|i| (aq[i] - ab[i] * t).powi(2)).sum();
        d2 <= r * r
    }

    fn reach(&self) -> (Vec3, Vec3) {
        if self.boxed {
            return (self.a, self.b);
        }
        let r = self.ra.max(self.rb);
        let s = self.stretch;
        let mid = [0, 1, 2].map(|i| (self.a[i] + self.b[i]) * 0.5);
        let lo = [0, 1, 2].map(|i| mid[i] + (self.a[i].min(self.b[i]) - r - mid[i]) * s[i]);
        let hi = [0, 1, 2].map(|i| mid[i] + (self.a[i].max(self.b[i]) + r - mid[i]) * s[i]);
        (lo, hi)
    }
}

/// A leg hanging from `hip`, swung forwards by `swing` radians, ending in a paw
fn leg(parts: &mut Vec<Part>, hip: Vec3, length: f64, radius: f64, swing: f64, fur: u8, paw: u8) {
    let foot = [hip[0] + swing.sin() * length, hip[1], hip[2] - swing.cos() * length + radius * 0.6];
    parts.push(ball(foot, radius * 1.1, paw));
    parts.push(limb(hip, foot, radius * 1.15, radius, fur));
}

/// `v` turned up by `angle` radians about `pivot`, round the y axis, so what's in front rises
fn rear(v: Vec3, pivot: Vec3, angle: f64) -> Vec3 {
    let (dx, dz) = (v[0] - pivot[0], v[2] - pivot[2]);
    let (c, s) = (angle.cos(), angle.sin());
    [pivot[0] + dx * c - dz * s, v[1], pivot[2] + dx * s + dz * c]
}

/// The Stone Warden: a hulking golem of rock, moss on its shoulders, crystals growing from its
/// back and a cold light glowing in its chest. It raises its fists overhead to slam them down, and
/// hurls boulders.
fn golem(p: &mut Vec<Part>, swing: f64, pose: Pose, skin: Skin) {
    let (rock, dark, glow) = (skin.body, skin.dark, skin.glow);
    // Leaning into a blow, the higher up the further
    let lean = pose.mix(0.0, 0.0, 0.4, 0.0);
    let up = |v: Vec3| {
        let k = (v[2] - 1.2).max(0.0) / 2.0;
        [v[0] + lean * k, v[1], v[2] - lean * 0.4 * k]
    };
    p.push(ball(up([0.64, 0.15, 2.86]), 0.065, glow));
    p.push(ball(up([0.64, -0.15, 2.86]), 0.065, glow));
    p.push(ellipsoid(up([0.6, 0.0, 2.98]), [0.12, 0.32, 0.06], dark));
    p.push(ellipsoid(up([0.38, 0.0, 2.84]), [0.32, 0.32, 0.28], rock));
    // The core, with light cracking out across its chest
    p.push(ball(up([0.6, 0.0, 2.05]), 0.17, glow));
    p.push(limb(up([0.66, 0.0, 2.05]), up([0.62, 0.38, 2.45]), 0.045, 0.02, glow));
    p.push(limb(up([0.66, 0.0, 2.05]), up([0.62, -0.32, 1.7]), 0.045, 0.02, glow));
    for (y, z, h) in [(0.25, 2.5, 0.55), (-0.22, 2.3, 0.65), (0.05, 2.75, 0.5)] {
        p.push(limb(up([-0.45, y, z]), up([-0.82, y * 1.3, z + h]), 0.1, 0.02, skin.extra));
    }
    p.push(ellipsoid(up([0.35, 0.0, 3.1]), [0.26, 0.26, 0.08], skin.accent));
    for s in [-1.0, 1.0] {
        p.push(ellipsoid(up([0.0, s * 0.95, 2.88]), [0.32, 0.32, 0.1], skin.accent));
    }
    // A boulder held up behind its head, ready to throw
    if pose.special > 0.75 {
        p.push(ball([-0.55, -0.55, 4.05], 0.45, rock));
    }
    for s in [-1.0, 1.0] {
        let shoulder = up([0.05, s * 0.98, 2.45]);
        // Swinging as it walks, raised overhead, brought down, or one lifting the boulder
        let special = if s < 0.0 { ([-0.25, -1.05, 3.1], [-0.45, -0.75, 3.7]) } else { ([0.5, 1.1, 1.9], [0.95, 0.9, 1.6]) };
        let elbow = pose.at([0.2 - 0.25 * swing * s, s * 1.18, 1.65], [0.0, s * 0.95, 3.15], [0.85, s * 0.85, 1.6], special.0);
        let fist = pose.at([0.45 - 0.35 * swing * s, s * 1.12, 0.85], [0.2, s * 0.5, 3.85], [1.45, s * 0.45, 0.4], special.1);
        p.push(ball(fist, 0.36, dark));
        p.push(limb(elbow, fist, 0.25, 0.3, rock));
        p.push(limb(shoulder, elbow, 0.27, 0.24, rock));
        p.push(ball(shoulder, 0.42, dark));
    }
    p.push(ellipsoid(up([0.1, 0.0, 2.05]), [0.62, 0.85, 0.75], rock));
    p.push(ellipsoid([0.0, 0.0, 1.2], [0.45, 0.6, 0.35], dark));
    for (y, s) in [(0.42, 1.0), (-0.42, -1.0)] {
        leg(p, [0.0, y, 1.15], 1.15, 0.27, 0.3 * swing * s, rock, dark);
    }
}

/// The Ember Drake: a red-scaled dragon with fire glowing through its belly, its wings folded.
/// It rears its head back, then breathes fire or spits it, and spreads its wings to roar.
fn drake(p: &mut Vec<Part>, angle: f64, swing: f64, moving: f64, pose: Pose, skin: Skin) {
    let (scales, dark, glow) = (skin.body, skin.dark, skin.glow);
    // Its head reared back, thrust out low, or raised to roar
    let neck_base = pose.at([0.8, 0.0, 1.45], [0.8, 0.0, 1.5], [0.8, 0.0, 1.45], [0.8, 0.0, 1.5]);
    let neck_top = pose.at([1.45, 0.0, 2.0], [1.15, 0.0, 2.45], [1.6, 0.0, 1.65], [1.35, 0.0, 2.3]);
    let head = pose.at([1.75, 0.0, 2.05], [1.35, 0.0, 2.75], [1.95, 0.0, 1.6], [1.65, 0.0, 2.55]);
    let open = pose.strike + pose.special > 0.4;
    let h = Vector3::from(head);
    let fwd = (h - Vector3::from(neck_top)).normalize();
    let upward = Vector3::new(-fwd.z, 0.0, fwd.x);
    let at = |f: f64, u: f64, side: f64| -> Vec3 { (h + fwd * f + upward * u + Vector3::new(0.0, side, 0.0)).into() };
    for side in [-0.13, 0.13] {
        p.push(ball(at(0.16, 0.1, side), 0.045, glow));
        p.push(limb(at(-0.08, 0.12, side), at(-0.45, 0.38, side * 1.6), 0.05, 0.012, skin.extra));
    }
    if open {
        p.push(ball(at(0.24, -0.06, 0.0), 0.13, glow));
        p.push(limb(h.into(), at(0.4, 0.04, 0.0), 0.15, 0.08, scales));
        p.push(limb(at(0.0, -0.14, 0.0), at(0.32, -0.26, 0.0), 0.1, 0.05, scales));
    } else {
        p.push(limb(h.into(), at(0.4, 0.0, 0.0), 0.16, 0.09, scales));
        p.push(limb(at(0.0, -0.09, 0.0), at(0.34, -0.1, 0.0), 0.1, 0.06, scales));
    }
    p.push(ball(head, 0.22, scales));
    p.push(limb(neck_base, neck_top, 0.3, 0.2, scales));
    // Its wings folded, or spread wide, partway between as it opens them
    let spread = pose.special;
    let open_by = |folded: Vec3, wide: Vec3| -> Vec3 { [0, 1, 2].map(|i| folded[i] + (wide[i] - folded[i]) * spread) };
    for s in [-1.0, 1.0] {
        let tip = open_by([-0.7, s * 0.62, 2.35], [-0.1, s * 2.3, 2.75]);
        p.push(limb(open_by([0.45, s * 0.48, 1.75], [0.45, s * 0.45, 1.75]), tip, 0.07, 0.03, dark));
        if spread > 0.0 {
            p.push(limb(tip, open_by([-0.7, s * 0.62, 2.35], [-1.1, s * 1.6, 1.6]), 0.03, 0.02, dark));
        }
        p.push(ellipsoid(open_by([-0.3, s * 0.6, 1.85], [-0.4, s * 1.35, 2.05]), open_by([0.75, 0.06, 0.38], [0.75, 1.0, 0.05]), skin.accent));
    }
    for k in 0..7 {
        let x = 0.7 - k as f64 * 0.3;
        p.push(limb([x, 0.0, 1.66], [x - 0.12, 0.0, 1.98 - k as f64 * 0.03], 0.06, 0.01, dark));
    }
    p.push(ellipsoid([0.05, 0.0, 0.95], [0.85, 0.38, 0.2], glow));
    p.push(ellipsoid([0.0, 0.0, 1.25], [1.05, 0.55, 0.5], scales));
    let sway = 0.25 * (angle + 1.0).sin() * moving;
    let tail = [[-0.95, 0.0, 1.25], [-1.8, sway, 1.0], [-2.55, sway * 2.0, 0.7], [-3.1, sway * 2.5, 0.55]];
    p.push(limb(tail[2], tail[3], 0.12, 0.03, dark));
    p.push(limb(tail[1], tail[2], 0.22, 0.12, scales));
    p.push(limb(tail[0], tail[1], 0.32, 0.22, scales));
    for (x, y, s) in [(0.6, 0.38, 1.0), (0.6, -0.38, -1.0), (-0.55, 0.4, -1.0), (-0.55, -0.4, 1.0)] {
        leg(p, [x, y, 0.95], 0.95, 0.16, 0.4 * swing * s, scales, dark);
    }
}

/// Ironhide: a huge old bear in plates of armor, tusked, with red eyes. It rears up on its hind
/// legs to bring its paws crashing down, and lowers its head to charge.
fn ironhide(p: &mut Vec<Part>, swing: f64, pose: Pose, skin: Skin) {
    let (fur, dark, armor) = (skin.body, skin.dark, skin.accent);
    // Reared up, or its head lowered
    let raised = pose.mix(0.0, 0.75, 0.0, 0.0);
    let low = pose.mix(0.0, 0.0, 0.15, 0.3);
    let pivot = [-0.75, 0.0, 1.0];
    let r = |v: Vec3| rear([v[0], v[1], v[2] - low * (v[0] + 0.75).max(0.0) / 2.25], pivot, raised);
    for s in [-1.0, 1.0] {
        p.push(ball(r([1.82, s * 0.22, 1.98]), 0.065, skin.glow));
        p.push(limb(r([2.0, s * 0.14, 1.52]), r([2.3, s * 0.2, 1.22]), 0.05, 0.015, skin.extra));
        p.push(ball(r([1.3, s * 0.33, 2.27]), 0.12, fur));
    }
    p.push(ball(r([2.2, 0.0, 1.7]), 0.07, EYE));
    if pose.windup + pose.special > 0.4 {
        // Roaring
        p.push(ball(r([2.06, 0.0, 1.5]), 0.11, EYE));
    }
    p.push(ellipsoid(r([1.92, 0.0, 1.68]), [0.32, 0.22, 0.2], dark));
    p.push(ellipsoid(r([1.62, 0.0, 2.14]), [0.32, 0.3, 0.1], armor));
    p.push(ball(r([1.5, 0.0, 1.85]), 0.48, fur));
    for k in 0..4 {
        let x = 0.8 - k as f64 * 0.5;
        let z = 2.5 - k as f64 * 0.07;
        p.push(limb(r([x, 0.0, z + 0.08]), r([x - 0.15, 0.0, z + 0.45]), 0.08, 0.012, skin.extra));
        p.push(ellipsoid(r([x, 0.0, z]), [0.3, 0.55, 0.12], armor));
    }
    p.push(ball(r([0.4, 0.0, 2.05]), 0.62, fur));
    p.push(limb(r([-0.85, 0.0, 1.55]), r([0.75, 0.0, 1.7]), 0.8, 0.85, fur));
    // Its front legs reach forward as it rears and slams, its hind legs stay planted
    let reach = pose.mix(0.0, 0.9, 0.7, 0.0);
    for (x, y, s) in [(0.75, 0.45, 1.0), (0.75, -0.45, -1.0)] {
        leg(p, r([x, y, 1.35]), 1.35, 0.28, reach + 0.35 * swing * s, fur, dark);
    }
    for (x, y, s) in [(-0.75, 0.45, -1.0), (-0.75, -0.45, 1.0)] {
        leg(p, [x, y, 1.35], 1.35, 0.3, 0.35 * swing * s, fur, dark);
    }
}

/// The Broodmother: a giant spider, dark and hairy, venom glowing in the markings on its back and
/// a cluster of red eyes. It rears up to bite, and crouches to leap.
fn spider(p: &mut Vec<Part>, angle: f64, moving: f64, pose: Pose, skin: Skin) {
    // Reared up on its back legs, lunging low, or crouched to spring
    let raise = pose.mix(0.0, 0.5, -0.15, 0.0);
    let drop = pose.mix(0.0, 0.0, 0.25, 0.45);
    let lift = |v: Vec3| {
        let up = raise * (v[0] + 0.6).max(0.0) * 0.8 - drop;
        [v[0], v[1], v[2] + up]
    };
    // Eight eyes, and fangs
    for (y, z, r) in [(0.1, 1.42, 0.06), (-0.1, 1.42, 0.06), (0.2, 1.36, 0.045), (-0.2, 1.36, 0.045), (0.06, 1.52, 0.04), (-0.06, 1.52, 0.04), (0.16, 1.5, 0.03), (-0.16, 1.5, 0.03)] {
        p.push(ball(lift([1.18, y, z]), r, skin.glow));
    }
    for y in [-0.09, 0.09] {
        p.push(limb(lift([1.22, y, 1.2]), lift([1.38, y * 0.6, 0.92]), 0.05, 0.012, skin.extra));
    }
    p.push(ellipsoid(lift([0.85, 0.0, 1.32]), [0.4, 0.38, 0.32], skin.body));
    // The abdomen, with glowing markings down its back
    for k in 0..3 {
        let x = -0.55 - k as f64 * 0.38;
        p.push(ellipsoid(lift([x, 0.0, 1.9 - k as f64 * 0.05]), [0.14, 0.32, 0.06], skin.accent));
    }
    p.push(ellipsoid(lift([-0.85, 0.0, 1.45]), [0.9, 0.75, 0.62], skin.body));
    // Eight legs, in two sets of four lifting by turns, each bent up at the knee
    for side in [-1.0, 1.0] {
        for k in 0..4 {
            let along = 0.95 - k as f64 * 0.45;
            let step = (angle + k as f64 * PI * 0.5 + if side > 0.0 { PI } else { 0.0 }).sin() * moving;
            let hip = lift([along * 0.6 + 0.3, side * 0.3, 1.3]);
            let reach = 1.6 + 0.2 * (1.0 - (k as f64 - 1.5).abs() / 1.5);
            // Its front legs raised as it rears
            let front = if k == 0 { pose.windup } else { 0.0 };
            let foot_x = along * 1.4 + 0.15 * step + 0.6 * front;
            let foot_z = 0.15 * step.max(0.0) + 1.4 * front;
            let foot = [foot_x, side * reach, foot_z];
            let knee = [(hip[0] + foot[0]) * 0.5, side * (reach * 0.65), hip[2] + 0.55 - drop * 0.5];
            p.push(limb(hip, knee, 0.1, 0.08, skin.dark));
            p.push(limb(knee, foot, 0.08, 0.03, skin.dark));
        }
    }
}

/// The Ancient Treant: an old tree walking on its roots, moss on its bark, a crown of leaves and
/// eyes glowing deep in its trunk. It swings its branches round, and calls roots up from below.
fn treant(p: &mut Vec<Part>, swing: f64, pose: Pose, skin: Skin) {
    let (bark, wood, moss, glow, leaves) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    p.push(ball([0.48, 0.17, 3.0], 0.08, glow));
    p.push(ball([0.48, -0.17, 3.0], 0.08, glow));
    p.push(ellipsoid([0.5, 0.0, 2.84], [0.06, 0.2, 0.05], BARK_DARK));
    // The crown
    for (c, r) in [([0.0, 0.0, 4.4], 0.95), ([0.5, 0.6, 4.1], 0.6), ([-0.4, -0.6, 4.2], 0.65), ([0.3, -0.5, 4.6], 0.55), ([-0.3, 0.5, 4.7], 0.55)] {
        p.push(ball(c, r, if r > 0.9 { LEAVES_DARK } else { leaves }));
    }
    // Moss on its trunk
    p.push(ellipsoid([0.38, 0.0, 2.2], [0.12, 0.38, 0.5], moss));
    p.push(limb([0.0, 0.0, 1.0], [0.0, 0.0, 3.7], 0.58, 0.45, bark));
    // Branches for arms, raised wide to sweep, swept down, or swaying as it walks
    for side in [-1.0, 1.0] {
        let shoulder = [0.0, side * 0.45, 3.2];
        let elbow = pose.at([0.2 - 0.2 * swing * side, side * 1.15, 2.6], [0.1, side * 1.3, 3.9], [0.9, side * 1.0, 2.2], [0.3, side * 1.1, 2.6]);
        let hand = pose.at([0.45 - 0.3 * swing * side, side * 1.35, 1.6], [0.4, side * 2.0, 4.4], [1.8, side * 0.4, 0.9], [0.2, side * 1.4, 1.2]);
        p.push(ball(hand, 0.32, leaves));
        p.push(limb(hand, [hand[0] + 0.3, hand[1] + side * 0.2, hand[2] - 0.35], 0.08, 0.02, wood));
        p.push(limb(elbow, hand, 0.17, 0.09, wood));
        p.push(limb(shoulder, elbow, 0.24, 0.17, bark));
    }
    // Roots for legs, spreading out from its foot
    for k in 0..5 {
        let a = k as f64 / 5.0 * TAU + 0.3;
        let lift = 0.35 * (swing * if k % 2 == 0 { 1.0 } else { -1.0 }).max(0.0);
        p.push(limb([a.cos() * 0.35, a.sin() * 0.35, 1.0], [a.cos() * 1.2, a.sin() * 1.2, lift], 0.3, 0.08, bark));
    }
}

/// A wolf, lean and grey, eyes glowing. It crouches to spring, lunges with its jaws open, and
/// throws its head back to howl.
fn wolf(p: &mut Vec<Part>, angle: f64, swing: f64, moving: f64, pose: Pose, skin: Skin) {
    let (fur, dark) = (skin.body, skin.dark);
    let crouch = pose.mix(0.0, 0.14, 0.0, 0.04);
    let lunge = pose.mix(0.0, -0.08, 0.22, 0.0);
    let howl = pose.special;
    let open = pose.strike + pose.special > 0.4;
    let head = [0.74 + lunge - 0.12 * howl, 0.0, 0.78 - crouch + 0.3 * howl];
    // Its muzzle points on ahead, or up at the sky
    let snout = [head[0] + 0.2 - 0.12 * howl, 0.0, head[2] - 0.04 + 0.2 * howl];
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.09, s * 0.07, head[2] + 0.05], 0.025, skin.glow));
        p.push(limb([head[0] - 0.03, s * 0.07, head[2] + 0.1], [head[0] - 0.05, s * 0.09, head[2] + 0.24], 0.04, 0.01, dark));
    }
    p.push(ball(snout, 0.03, EYE));
    if open {
        p.push(limb([head[0] + 0.05, 0.0, head[2] - 0.08], [snout[0], 0.0, snout[2] - 0.12], 0.045, 0.025, dark));
    }
    p.push(limb(head, snout, 0.075, 0.05, fur));
    p.push(ball(head, 0.13, fur));
    p.push(limb([0.42 + lunge * 0.5, 0.0, 0.62 - crouch], head, 0.12, 0.1, fur));
    p.push(ellipsoid([0.28 + lunge * 0.5, 0.0, 0.55 - crouch], [0.22, 0.17, 0.18], skin.accent));
    p.push(ellipsoid([0.0 + lunge * 0.3, 0.0, 0.6 - crouch], [0.5, 0.18, 0.19], fur));
    // Its tail, low as it creeps, up as it howls
    let sway = 0.08 * (angle + 1.0).sin() * moving;
    p.push(limb([-0.45, 0.0, 0.62 - crouch], [-0.82, sway, 0.45 - crouch + 0.35 * howl], 0.06, 0.035, dark));
    for (x, y, s) in [(0.3, 0.12, 1.0), (0.3, -0.12, -1.0), (-0.32, 0.12, -1.0), (-0.32, -0.12, 1.0)] {
        // Its hind legs stretched out behind as it lunges
        let reach = if x < 0.0 { -0.4 * pose.strike } else { 0.5 * pose.strike };
        leg(p, [x + lunge * 0.4, y, 0.52 - crouch], 0.52 - crouch, 0.055, 0.5 * swing * s + reach, fur, dark);
    }
}

/// The walking dead, about 1.8 meters tall: bones or rotting flesh in rags, maybe in armor, eyes
/// burning, a blade in the right hand. It raises the blade overhead and brings it down; zombies
/// shamble with their arms out.
fn undead(p: &mut Vec<Part>, swing: f64, pose: Pose, skin: Skin, bony: bool, armored: bool) {
    let (flesh, dark, glow) = (skin.body, skin.dark, skin.glow);
    // Leaning into a blow, and hunched as zombies are
    let hunch = if bony { 0.0 } else { 0.08 };
    let lean = pose.mix(hunch, hunch - 0.05, hunch + 0.22, hunch + 0.05);
    let up = |v: Vec3| [v[0] + lean * (v[2] - 0.9).max(0.0), v[1], v[2]];
    let limb_r = if bony { 0.045 } else { 0.07 };
    // Its head: a skull with burning sockets, or a rotting face
    let head = up([0.02, 0.0, 1.62]);
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.1, s * 0.045, head[2] + 0.02], 0.028, glow));
    }
    if armored {
        // A helm with a slit
        p.push(ellipsoid([head[0] - 0.01, 0.0, head[2] + 0.04], [0.15, 0.15, 0.15], skin.accent));
    }
    p.push(ellipsoid([head[0] + 0.05, 0.0, head[2] - 0.09], [0.08, 0.08, 0.05], if bony { flesh } else { dark }));
    p.push(ball(head, 0.13, flesh));
    p.push(limb(up([0.0, 0.0, 1.3]), head, 0.05, 0.05, flesh));
    // Its chest: ribs, or rags over flesh, maybe a breastplate
    let chest = up([0.0, 0.0, 1.25]);
    if armored {
        p.push(ellipsoid(chest, [0.15, 0.22, 0.22], skin.accent));
        for s in [-1.0, 1.0] {
            p.push(ellipsoid(up([0.0, s * 0.24, 1.44]), [0.12, 0.12, 0.08], skin.accent));
        }
    }
    if bony {
        p.push(ellipsoid(chest, [0.12, 0.17, 0.2], flesh).striped(dark));
        p.push(limb(up([0.0, 0.0, 0.92]), chest, 0.04, 0.04, flesh));
    } else {
        p.push(ellipsoid(up([0.0, 0.0, 1.12]), [0.15, 0.2, 0.28], skin.accent));
        p.push(ellipsoid(chest, [0.13, 0.2, 0.22], flesh));
    }
    p.push(ellipsoid([0.0, 0.0, 0.92], [0.1, 0.15, 0.07], if armored { skin.accent } else { dark }));
    // The blade arm: hanging, raised overhead behind, or brought down in front
    let shoulder = up([0.0, -0.22, 1.42]);
    let zombie_reach = if bony { 0.0 } else { 0.35 };
    let elbow = pose.at([0.05 + zombie_reach - 0.15 * swing, -0.26, 1.15 + zombie_reach * 0.5], [-0.08, -0.3, 1.65], [0.38, -0.2, 1.3], [0.1, -0.32, 1.55]);
    let hand = pose.at([0.2 + zombie_reach * 1.4 - 0.25 * swing, -0.25, 0.95 + zombie_reach], [-0.18, -0.24, 2.0], [0.62, -0.1, 1.0], [0.3, -0.28, 1.9]);
    let blade = pose.at([0.55, 0.0, -0.75], [-0.5, 0.05, 0.85], [0.92, 0.1, -0.35], [0.3, 0.0, 0.95]);
    let n = (blade[0] * blade[0] + blade[1] * blade[1] + blade[2] * blade[2]).sqrt().max(0.1);
    let tip = [0, 1, 2].map(|i| hand[i] + blade[i] / n * 0.85);
    p.push(limb(hand, tip, 0.03, 0.012, skin.extra));
    p.push(limb([hand[0] - blade[1] / n * 0.1, hand[1] - 0.09, hand[2]], [hand[0] + blade[1] / n * 0.1, hand[1] + 0.09, hand[2]], 0.025, 0.025, DARK_STEEL));
    p.push(ball(hand, limb_r * 1.1, flesh));
    p.push(limb(elbow, hand, limb_r * 0.9, limb_r * 0.8, flesh));
    p.push(limb(shoulder, elbow, limb_r, limb_r * 0.9, if armored { skin.accent } else { flesh }));
    // The other arm swinging, reaching out for a zombie, thrown out for balance as it strikes
    let shoulder = up([0.0, 0.22, 1.42]);
    let elbow = pose.at([0.05 + zombie_reach + 0.15 * swing, 0.26, 1.15 + zombie_reach * 0.5], [0.2, 0.3, 1.3], [-0.12, 0.3, 1.2], [0.1, 0.32, 1.55]);
    let hand = pose.at([0.15 + zombie_reach * 1.4 + 0.25 * swing, 0.27, 0.95 + zombie_reach], [0.42, 0.26, 1.35], [-0.25, 0.32, 1.0], [0.3, 0.28, 1.9]);
    p.push(ball(hand, limb_r * 1.1, flesh));
    p.push(limb(elbow, hand, limb_r * 0.9, limb_r * 0.8, flesh));
    p.push(limb(shoulder, elbow, limb_r, limb_r * 0.9, if armored { skin.accent } else { flesh }));
    // Its legs, in rags or greaves
    for (y, s) in [(0.1, 1.0), (-0.1, -1.0)] {
        leg(p, [0.0, y, 0.92], 0.88, limb_r * 1.1, 0.45 * swing * s, if armored { skin.accent } else if bony { flesh } else { dark }, dark);
    }
}

/// The shapes making up an animal, in meters: facing +x, with its feet on z = 0.
/// `phase` is how far through its walk it is, None standing still. A boss can strike a `pose`.
fn parts(look: Look, phase: Option<f64>, pose: Pose) -> Vec<Part> {
    let angle = phase.map_or(0.0, |p| p * TAU);
    let moving = phase.is_some() as u8 as f64;
    // Diagonal pairs of legs swing together
    let swing = angle.sin() * moving;
    let mut p = Vec::new();
    match look {
        // Bosses and monsters, made at the size of their build, then grown or shrunk to their own
        _ if look.build().is_some() => {
            match look.build() {
                Some(Build::Golem) => golem(&mut p, swing, pose, look.skin()),
                Some(Build::Drake) => drake(&mut p, angle, swing, moving, pose, look.skin()),
                Some(Build::Bear) => ironhide(&mut p, swing, pose, look.skin()),
                Some(Build::Spider) => spider(&mut p, angle, moving, pose, look.skin()),
                Some(Build::Wolf) => wolf(&mut p, angle, swing, moving, pose, look.skin()),
                Some(Build::Undead) => undead(&mut p, swing, pose, look.skin(), look.bony(), look.armored()),
                _ => treant(&mut p, swing, pose, look.skin()),
            }
            if look.scale() != 1.0 {
                p = p.iter().map(|part| part.scaled(look.scale())).collect();
            }
        }
        Look::Bear => {
            let (fur, dark) = (FUR_BROWN, FUR_DARK);
            p.push(ball([0.98, 0.12, 1.08], 0.035, EYE));
            p.push(ball([0.98, -0.12, 1.08], 0.035, EYE));
            p.push(ball([1.2, 0.0, 0.97], 0.05, EYE));
            p.push(ellipsoid([1.06, 0.0, 0.93], [0.16, 0.12, 0.1], FUR_CREAM));
            p.push(ball([0.78, 0.19, 1.24], 0.08, dark));
            p.push(ball([0.78, -0.19, 1.24], 0.08, dark));
            p.push(ball([0.84, 0.0, 1.02], 0.27, fur));
            p.push(ball([0.32, 0.0, 1.06], 0.32, fur));
            p.push(ellipsoid([0.0, 0.0, 0.88], [0.78, 0.42, 0.42], fur));
            p.push(ball([-0.78, 0.0, 0.96], 0.08, fur));
            for (x, y, s) in [(0.48, 0.24, 1.0), (0.48, -0.24, -1.0), (-0.48, 0.24, -1.0), (-0.48, -0.24, 1.0)] {
                leg(&mut p, [x, y, 0.66], 0.66, 0.13, 0.35 * swing * s, fur, dark);
            }
        }
        Look::Dog => {
            let (fur, white) = (FUR_TAN, FUR_WHITE);
            p.push(ball([0.51, 0.06, 0.75], 0.022, EYE));
            p.push(ball([0.51, -0.06, 0.75], 0.022, EYE));
            p.push(ball([0.635, 0.0, 0.685], 0.026, EYE));
            p.push(ellipsoid([0.55, 0.0, 0.66], [0.09, 0.06, 0.055], white));
            p.push(ellipsoid([0.39, 0.115, 0.68], [0.04, 0.025, 0.08], FUR_BROWN));
            p.push(ellipsoid([0.39, -0.115, 0.68], [0.04, 0.025, 0.08], FUR_BROWN));
            p.push(ball([0.42, 0.0, 0.71], 0.115, fur));
            p.push(limb([0.27, 0.0, 0.5], [0.38, 0.0, 0.66], 0.085, 0.07, fur));
            p.push(ellipsoid([0.2, 0.0, 0.41], [0.15, 0.12, 0.13], white));
            p.push(ellipsoid([0.0, 0.0, 0.47], [0.36, 0.15, 0.16], fur));
            // Wags while it walks
            let wag = 0.06 * (angle * 2.0).sin() * moving;
            p.push(limb([-0.32, 0.0, 0.53], [-0.5, wag, 0.7], 0.035, 0.025, fur));
            for (x, y, s) in [(0.22, 0.09, 1.0), (0.22, -0.09, -1.0), (-0.24, 0.09, -1.0), (-0.24, -0.09, 1.0)] {
                leg(&mut p, [x, y, 0.4], 0.4, 0.042, 0.45 * swing * s, fur, white);
            }
        }
        Look::GrayCat | Look::GingerCat => {
            let (fur, stripes) = if look == Look::GrayCat { (FUR_GRAY, FUR_DARK) } else { (FUR_GINGER, FUR_RED) };
            let white = FUR_WHITE;
            p.push(ball([0.3, 0.035, 0.38], 0.015, EYE));
            p.push(ball([0.3, -0.035, 0.38], 0.015, EYE));
            p.push(ball([0.327, 0.0, 0.345], 0.012, NOSE));
            p.push(ball([0.3, 0.0, 0.33], 0.038, white));
            p.push(limb([0.225, 0.045, 0.41], [0.22, 0.052, 0.47], 0.026, 0.006, fur));
            p.push(limb([0.225, -0.045, 0.41], [0.22, -0.052, 0.47], 0.026, 0.006, fur));
            p.push(ball([0.24, 0.0, 0.36], 0.08, fur));
            p.push(ellipsoid([0.03, 0.0, 0.21], [0.15, 0.06, 0.055], white));
            p.push(ellipsoid([0.0, 0.0, 0.25], [0.21, 0.085, 0.09], fur).striped(stripes));
            // The tail sways
            let sway = 0.04 * (angle + 1.0).sin() * moving;
            p.push(limb([-0.19, 0.0, 0.28], [-0.3, sway, 0.42], 0.022, 0.02, fur).striped(stripes));
            p.push(limb([-0.3, sway, 0.42], [-0.28, sway * 1.5, 0.53], 0.02, 0.016, fur).striped(stripes));
            for (x, y, s) in [(0.14, 0.05, 1.0), (0.14, -0.05, -1.0), (-0.13, 0.05, -1.0), (-0.13, -0.05, 1.0)] {
                leg(&mut p, [x, y, 0.21], 0.21, 0.024, 0.45 * swing * s, fur, white);
            }
        }
        Look::Squirrel => {
            // Squirrels bound along: both pairs of legs together, the body bobbing up between
            let fur = FUR_RED;
            let lift = 0.04 * angle.sin().max(0.0) * moving;
            let up = |v: Vec3| [v[0], v[1], v[2] + lift];
            p.push(ball(up([0.13, 0.027, 0.175]), 0.011, EYE));
            p.push(ball(up([0.13, -0.027, 0.175]), 0.011, EYE));
            p.push(ball(up([0.146, 0.0, 0.16]), 0.009, EYE));
            p.push(limb(up([0.09, 0.022, 0.2]), up([0.088, 0.025, 0.23]), 0.012, 0.004, fur));
            p.push(limb(up([0.09, -0.022, 0.2]), up([0.088, -0.025, 0.23]), 0.012, 0.004, fur));
            p.push(ball(up([0.1, 0.0, 0.16]), 0.045, fur));
            p.push(ellipsoid(up([0.03, 0.0, 0.085]), [0.07, 0.035, 0.04], FUR_CREAM));
            p.push(ellipsoid(up([0.0, 0.0, 0.11]), [0.1, 0.05, 0.06], fur));
            // A big bushy tail curling up over its back
            let flick = 0.02 * angle.cos() * moving;
            for (c, r) in [([-0.1, 0.0, 0.11], 0.045), ([-0.15, 0.0, 0.17], 0.055), ([-0.15 + flick, 0.0, 0.25], 0.06), ([-0.11 + flick, 0.0, 0.31], 0.055)] {
                p.push(ball(up(c), r, fur));
            }
            for (x, y, length) in [(0.06, 0.03, 0.07), (0.06, -0.03, 0.07), (-0.06, 0.04, 0.08), (-0.06, -0.04, 0.08)] {
                let s = if x > 0.0 { 1.0 } else { -1.0 };
                leg(&mut p, up([x, y, length]), length, 0.018, 0.5 * swing * s, fur, fur);
            }
        }
        Look::Apple => {
            p.push(limb([0.0, 0.0, 0.11], [0.01, 0.0, 0.145], 0.008, 0.006, WOOD));
            p.push(ellipsoid([0.028, 0.0, 0.138], [0.026, 0.012, 0.008], LEAVES));
            p.push(ellipsoid([0.0, 0.0, 0.062], [0.066, 0.066, 0.06], APPLE));
        }
        Look::Meat => {
            // A drumstick
            p.push(ball([-0.14, 0.022, 0.03], 0.024, BONE));
            p.push(ball([-0.14, -0.022, 0.03], 0.024, BONE));
            p.push(limb([-0.14, 0.0, 0.03], [-0.02, 0.0, 0.045], 0.016, 0.018, BONE));
            p.push(ellipsoid([0.05, 0.0, 0.055], [0.1, 0.07, 0.055], MEAT));
        }
        Look::Candle => {
            // A stubby candle with wax run down its side, and its flame
            p.push(ellipsoid([0.0, 0.0, 0.25], [0.028, 0.028, 0.05], FLAME));
            p.push(limb([0.0, 0.0, 0.18], [0.0, 0.0, 0.205], 0.008, 0.008, BARK_DARK));
            p.push(limb([0.045, 0.0, 0.06], [0.045, 0.0, 0.17], 0.018, 0.012, WAX));
            p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 0.18], 0.05, 0.045, WAX));
        }
        Look::Skull => {
            p.push(ball([0.085, 0.042, 0.115], 0.026, EYE));
            p.push(ball([0.085, -0.042, 0.115], 0.026, EYE));
            p.push(ball([0.105, 0.0, 0.075], 0.015, EYE));
            p.push(limb([0.075, -0.035, 0.045], [0.075, 0.035, 0.045], 0.008, 0.008, BARK_DARK));
            p.push(ellipsoid([0.055, 0.0, 0.045], [0.06, 0.06, 0.04], BONE));
            p.push(ellipsoid([0.0, 0.0, 0.115], [0.1, 0.085, 0.09], BONE));
        }
        Look::Chest | Look::OpenChest => {
            // A wooden chest bound with iron, its lock at the front. Open, its lid stands up
            // at the back and gold glints inside.
            let open = look == Look::OpenChest;
            if open {
                p.push(cuboid([-0.17, -0.3, 0.17], [0.17, 0.3, 0.21], GLOW_HOLY));
                p.push(cuboid([-0.18, -0.31, 0.04], [0.18, 0.31, 0.3], AIR));
            } else {
                p.push(cuboid([0.215, -0.04, 0.17], [0.245, 0.04, 0.26], GOLD));
            }
            for y in [-0.25, 0.25] {
                p.push(cuboid([-0.23, y - 0.025, 0.0], [0.23, y + 0.025, 0.29], DARK_STEEL));
            }
            p.push(cuboid([-0.22, -0.35, 0.0], [0.22, 0.35, 0.28], WOOD));
            let lid = |p: &mut Vec<Part>, lo: Vec3, hi: Vec3| {
                if open {
                    // Swung up and over on its hinge at the back
                    let turn = |q: Vec3| [-0.22 - (q[2] - 0.28), q[1], 0.28 + (q[0] + 0.22)];
                    let (a, b) = (turn(lo), turn(hi));
                    p.push(cuboid([a[0].min(b[0]), a[1], a[2].min(b[2])], [a[0].max(b[0]), b[1], a[2].max(b[2])], WOOD));
                } else {
                    p.push(cuboid(lo, hi, WOOD));
                }
            };
            if !open {
                for y in [-0.25, 0.25] {
                    p.push(cuboid([-0.23, y - 0.025, 0.28], [0.23, y + 0.025, 0.37], DARK_STEEL));
                    p.push(cuboid([-0.18, y - 0.025, 0.36], [0.18, y + 0.025, 0.41], DARK_STEEL));
                }
            }
            lid(&mut p, [-0.22, -0.35, 0.28], [0.22, 0.35, 0.36]);
            lid(&mut p, [-0.17, -0.35, 0.36], [0.17, 0.35, 0.4]);
        }
        // Bosses, all built above
        _ => {}
    }
    p
}

/// A model rasterized into voxels: x + (y + z * size.y) * size.x
pub struct Model {
    pub size: [u32; 3],
    pub voxels: Vec<u8>,
}

/// Turns shapes in meters into a model with `res` voxels per meter, sized to fit them however
/// they're placed. Returns where the shader puts the model in the world, the middle of its bottom,
/// in the same meters as the shapes.
pub fn voxelize(parts: &[Part], res: f64) -> (Model, Vector3<f64>) {
    let parts: Vec<Part> = parts.iter().map(|p| p.thickened(res)).collect();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for part in &parts {
        let (a, b) = part.reach();
        for i in 0..3 {
            lo[i] = lo[i].min(a[i]);
            hi[i] = hi[i].max(b[i]);
        }
    }
    let size = [0, 1, 2].map(|i| (((hi[i] - lo[i]) * res).ceil() as u32).clamp(1, 1023));
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    // Each shape only looks at the voxels around it, and takes those no earlier shape took
    let mut taken = vec![false; voxels.len()];
    for part in &parts {
        let (a, b) = part.reach();
        let from = [0, 1, 2].map(|i| (((a[i] - lo[i]) * res - 0.5).floor().max(0.0) as u32).min(size[i]));
        let to = [0, 1, 2].map(|i| (((b[i] - lo[i]) * res + 0.5).ceil().max(0.0) as u32).min(size[i]));
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let i = (x + (y + z * size[1]) * size[0]) as usize;
                    let p = [lo[0] + (x as f64 + 0.5) / res, lo[1] + (y as f64 + 0.5) / res, lo[2] + (z as f64 + 0.5) / res];
                    if !taken[i] && part.contains(p) {
                        taken[i] = true;
                        voxels[i] = part.material;
                    }
                }
            }
        }
    }
    let anchor = Vector3::new(lo[0] + size[0] as f64 * 0.5 / res, lo[1] + size[1] as f64 * 0.5 / res, lo[2]);
    (Model { size, voxels }, anchor)
}

fn rasterize(look: Look, phase: Option<f64>, pose: Pose) -> Model {
    let res = look.resolution();
    let parts: Vec<Part> = parts(look, phase, pose).iter().map(|p| p.thickened(res)).collect();
    // Wide enough either side of the middle to hold everything, as the shader puts the middle of
    // the model where the animal stands
    let (mut half_x, mut half_y, mut top) = (0.0f64, 0.0f64, 0.0f64);
    for part in &parts {
        let (lo, hi) = part.reach();
        half_x = half_x.max(lo[0].abs()).max(hi[0].abs());
        half_y = half_y.max(lo[1].abs()).max(hi[1].abs());
        top = top.max(hi[2]);
    }
    let size = [
        ((2.0 * half_x * res).ceil() as u32).clamp(1, 255),
        ((2.0 * half_y * res).ceil() as u32).clamp(1, 255),
        ((top * res).ceil() as u32).clamp(1, 255),
    ];
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    // Each shape only looks at the voxels around it, and takes those no earlier shape took
    let mut taken = vec![false; voxels.len()];
    let origin = [-(size[0] as f64) * 0.5, -(size[1] as f64) * 0.5, 0.0];
    for part in &parts {
        let (a, b) = part.reach();
        let from = [0, 1, 2].map(|i| ((a[i] * res - origin[i] - 0.5).floor().max(0.0) as u32).min(size[i]));
        let to = [0, 1, 2].map(|i| ((b[i] * res - origin[i] + 0.5).ceil().max(0.0) as u32).min(size[i]));
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let i = (x + (y + z * size[1]) * size[0]) as usize;
                    let p = [(x as f64 + 0.5 + origin[0]) / res, (y as f64 + 0.5 + origin[1]) / res, (z as f64 + 0.5) / res];
                    if !taken[i] && part.contains(p) {
                        taken[i] = true;
                        let band = part.stripes != AIR && (x / 2) % 3 == 0;
                        voxels[i] = if band { part.stripes } else { part.material };
                    }
                }
            }
        }
    }
    Model { size, voxels }
}

fn model_index(look: Look, frame: usize) -> u32 {
    (Look::ALL.iter().position(|&l| l == look).unwrap() * FRAMES + frame) as u32
}

/// How many models animal_models() makes. Other models come after them.
pub const ANIMAL_MODELS: usize = Look::ALL.len() * FRAMES;

/// Every frame of every animal, and of everything else drawn like one
pub fn animal_models() -> Vec<Model> {
    let mut models = Vec::new();
    for look in Look::ALL {
        models.push(rasterize(look, None, Pose::REST));
        for frame in 0..WALK_FRAMES {
            models.push(rasterize(look, Some(frame as f64 / WALK_FRAMES as f64), Pose::REST));
        }
        // Only bosses and monsters strike poses, so the rest leave them empty
        for frame in POSE_WINDUP..FRAMES {
            models.push(if look.build().is_some() { rasterize(look, None, Pose::of(frame)) } else { Model { size: [1, 1, 1], voxels: vec![AIR] } });
        }
    }
    models
}

/// Models packed for shaders\traverse.comp: two header words per model (where its voxels start, and
/// its size packed x | y << 10 | z << 20), then the voxels, four to a word.
pub fn pack_models(models: &[&Model]) -> Vec<u32> {
    let mut words = vec![0u32; models.len() * 2];
    for (i, model) in models.iter().enumerate() {
        words[i * 2] = words.len() as u32;
        words[i * 2 + 1] = model.size[0] | model.size[1] << 10 | model.size[2] << 20;
        for chunk in model.voxels.chunks(4) {
            words.push(chunk.iter().enumerate().fold(0, |w, (j, &m)| w | (m as u32) << (j * 8)));
        }
    }
    words
}

// How an instance moves, has to match shaders\traverse.comp
pub const STILL: u32 = 0;
pub const MOVING: u32 = 1;
/// In the player's hand
pub const HELD: u32 = 2;

/// What weapons did to an animal, shown on it. Has to match TINT_* in shaders\traverse.comp.
pub const TINT_NONE: u32 = 0;
pub const TINT_BURNING: u32 = 1;
pub const TINT_FROZEN: u32 = 2;
pub const TINT_POISONED: u32 = 3;
pub const TINT_SHOCKED: u32 = 4;
pub const TINT_CHILLED: u32 = 5;
/// Struck a moment ago
pub const TINT_HIT: u32 = 6;

/// One animal as traverse.comp reads it, see the Animal struct there
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
pub struct AnimalInstance {
    position: [f32; 4],
    rotation: [f32; 4],
    scale: [f32; 4],
    info: [u32; 4],
}

impl AnimalInstance {
    /// Model `model` turned by `rotation`, with the middle of its bottom at `position` and its
    /// voxels `scale` voxels of the main world across. `reach` is how far it reaches from there,
    /// in voxels of the main world, and `kind` STILL, MOVING or HELD.
    pub fn new(model: u32, position: Vector3<f64>, rotation: UnitQuaternion<f64>, scale: f64, reach: f64, kind: u32) -> Self {
        let q = rotation.coords;
        AnimalInstance {
            position: [position.x as f32, position.y as f32, position.z as f32, reach as f32],
            rotation: [q.x as f32, q.y as f32, q.z as f32, q.w as f32],
            scale: [scale as f32, 0.0, 0.0, 0.0],
            info: [model, kind, TINT_NONE, 0],
        }
    }

    pub fn tinted(mut self, tint: u32) -> Self {
        self.info[2] = tint;
        self
    }
}

fn facing(heading: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), heading)
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mood {
    Idle,
    Walk,
    Run,
}

/// Positions are in voxels of the main world
pub struct Animal {
    /// Different for every animal
    pub id: u32,
    pub look: Look,
    /// The middle of its feet
    pub position: Vector3<f64>,
    pub(crate) heading: f64,
    pub(crate) target_heading: f64,
    pub(crate) speed: f64,
    pub(crate) mood: Mood,
    timer: f64,
    /// How far through its walk cycle, 0-1
    pub(crate) phase: f64,
    /// Seconds it's been about, for it to breathe and sway as it stands
    age: f64,
    pub health: f64,
    /// Burning, poisoned, frozen and such, see src\combat.rs
    pub status: Status,
    /// What a boss is up to, see src\bosses.rs
    pub boss: Option<Boss>,
    rng: u32,
}

/// How hard a bare-handed hit is
pub const PUNCH: f64 = 35.0;

/// Something in the world drawn like an animal that doesn't move, like food lying on the ground.
/// `position` is in voxels of the main world.
pub fn still_instance(look: Look, position: Vector3<f64>, heading: f64, world_voxels_per_meter: u32) -> AnimalInstance {
    let vpm = world_voxels_per_meter as f64;
    AnimalInstance::new(model_index(look, 0), position, facing(heading), vpm / look.resolution(), look.reach() * vpm, STILL)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

fn random(seed: &mut u32) -> f64 {
    *seed = hash(seed.wrapping_add(0x9e3779b9));
    (*seed >> 8) as f64 / 16777216.0
}

/// Height of the ground an animal standing at height `z` could step onto at x, y: up at most
/// `up` voxels, down at most `down`. None for walls, cliffs, water and outside the world.
fn ground_at(world: &World, water: WaterLevel, x: f64, y: f64, z: f64, up: f64, down: f64) -> Option<f64> {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let top = (z + up).floor() as i64;
    if !is_passable(world.material_at(xi, yi, top)) {
        return None;
    }
    let ground = ((z - down).floor() as i64..top)
        .rev()
        .find(|&zi| !is_passable(world.material_at(xi, yi, zi)))
        .map(|zi| zi as f64 + 1.0)?;
    // Paddling in the shallows is fine
    (water(x, y) < ground + 0.2 * world.voxels_per_meter as f64).then_some(ground)
}

/// The highest dry ground in a column, if it isn't under water
fn surface_at(world: &World, water: WaterLevel, x: f64, y: f64) -> Option<f64> {
    ground_at(world, water, x, y, 0.0, world.size[2] as f64 - 1.0, 1.0)
}

impl Animal {
    fn new(look: Look, position: Vector3<f64>, rng: u32) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        let mut rng = rng;
        let heading = random(&mut rng) * TAU;
        Animal {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            look,
            position,
            heading,
            target_heading: heading,
            speed: 0.0,
            mood: Mood::Idle,
            timer: random(&mut rng) * 3.0,
            phase: 0.0,
            age: random(&mut rng) * 10.0,
            health: look.health(),
            status: Status::default(),
            boss: None,
            rng,
        }
    }

    /// A boss guarding `home`, in voxels. `key` is where its lair is in the whole landscape, to
    /// remember it's been beaten.
    pub fn boss(look: Look, home: Vector3<f64>, key: [i64; 3], seed: u32) -> Self {
        let mut animal = Animal::new(look, home, hash(seed ^ 0xb055));
        animal.boss = Some(Boss::new(look, home, key));
        animal
    }

    pub fn heading(&self) -> f64 {
        self.heading
    }

    /// The middle of its body, to aim at
    pub fn center(&self, world_voxels_per_meter: u32) -> Vector3<f64> {
        self.position + Vector3::new(0.0, 0.0, self.look.body().0 * world_voxels_per_meter as f64)
    }

    /// How much health it has left, 0-1
    pub fn vitality(&self) -> f64 {
        self.health / self.look.health()
    }

    /// Hit bare-handed by the player standing at `from`. Runs off, and returns whether that was
    /// the last hit.
    pub fn hurt(&mut self, from: Vector3<f64>) -> bool {
        self.damage(PUNCH, from)
    }

    /// Takes `amount` of damage from something at `from`, and runs off, or a boss turns on
    /// whoever did it. Returns whether it died.
    pub fn damage(&mut self, amount: f64, from: Vector3<f64>) -> bool {
        self.health = (self.health - amount).max(0.0);
        match &mut self.boss {
            Some(boss) => boss.provoke(),
            None => self.flee(from, 4.0),
        }
        self.health <= 0.0
    }

    pub fn flee(&mut self, from: Vector3<f64>, seconds: f64) {
        let away = (self.position - from).xy();
        self.mood = Mood::Run;
        self.timer = seconds;
        self.target_heading = away.y.atan2(away.x);
    }

    /// Petted by the player at `from`: stops to enjoy it, facing them
    pub fn pet(&mut self, from: Vector3<f64>) {
        let towards = (from - self.position).xy();
        self.mood = Mood::Idle;
        self.timer = 4.0;
        self.target_heading = towards.y.atan2(towards.x);
    }

    /// `player` is where the player is, in voxels, so some animals can come over or run off
    pub fn update(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let look = self.look;
        self.age += dt;

        // Bosses shrug off most of what would knock an animal about, monsters don't
        if look.is_boss() {
            let most = 1.2 * vpm;
            if self.status.push.norm() > most {
                self.status.push = self.status.push.normalize() * most;
            }
            self.status.limit_for_boss();
        }
        if self.boss.is_some() {
            bosses::think(self, world, player, dt);
        }

        self.timer -= dt;
        if self.boss.is_none() && self.timer <= 0.0 {
            let r = random(&mut self.rng);
            let (mood, time) = if r < 0.4 {
                (Mood::Idle, 2.0 + 4.0 * random(&mut self.rng))
            } else if r < 0.9 {
                (Mood::Walk, 3.0 + 5.0 * random(&mut self.rng))
            } else {
                (Mood::Run, 1.5 + 2.0 * random(&mut self.rng))
            };
            self.mood = mood;
            self.timer = time;
            self.target_heading = self.heading + (random(&mut self.rng) - 0.5) * 3.0;

            // Dogs like to come over to the player now and then
            if let Some(player) = player
                && look == Look::Dog
                && random(&mut self.rng) < 0.5
            {
                let offset = (player - self.position).xy() / vpm;
                if offset.norm() > 3.0 && offset.norm() < 25.0 {
                    self.mood = Mood::Walk;
                    self.target_heading = offset.y.atan2(offset.x);
                }
            }
        }

        // Squirrels dart away when the player gets close
        if let Some(player) = player
            && look == Look::Squirrel
        {
            let offset = (self.position - player).xy() / vpm;
            if offset.norm() < 4.0 && self.mood != Mood::Run {
                self.mood = Mood::Run;
                self.timer = 1.5;
                self.target_heading = offset.y.atan2(offset.x);
            }
        }

        // Thrown back by a blow, sliding to a stop
        let push = self.status.push * dt;
        if push.norm() > 1e-3 {
            let (x, y) = (self.position.x + push.x, self.position.y + push.y);
            let reach = look.climb() * vpm * 2.0;
            if let Some(z) = ground_at(world, water, x, y, self.position.z, reach, reach * 2.0) {
                self.position = Vector3::new(x, y, z);
            } else {
                self.status.push = Vector3::zeros();
            }
        }
        self.status.push *= (-6.0 * dt).exp();

        // Frozen solid or knocked off its feet it can't move, slowed it crawls
        let pace = self.status.pace();
        if pace <= 0.0 {
            self.speed = 0.0;
            return;
        }

        // Turn towards where it's headed, the short way round
        let turn = (self.target_heading - self.heading + PI).rem_euclid(TAU) - PI;
        self.heading += turn.clamp(-3.0 * dt * pace, 3.0 * dt * pace);

        let wish = pace
            * match self.mood {
                Mood::Idle => 0.0,
                Mood::Walk => look.walk_speed(),
                Mood::Run => look.run_speed(),
            };
        self.speed += (wish - self.speed) * (1.0 - (-4.0 * dt).exp());

        if self.speed > 0.01 {
            let step = Vector2::new(self.heading.cos(), self.heading.sin()) * self.speed * vpm * dt;
            // Feel ahead a little, so it turns before its nose is in a wall
            let ahead = Vector2::new(self.heading.cos(), self.heading.sin()) * 0.3 * vpm;
            let (x, y) = (self.position.x + step.x, self.position.y + step.y);
            let climb = look.climb() * vpm;
            let ground = ground_at(world, water, x, y, self.position.z, climb, climb * 2.0);
            let clear = ground_at(world, water, x + ahead.x, y + ahead.y, self.position.z, climb, climb * 2.0).is_some();
            match ground {
                Some(z) if clear => {
                    self.position.x = x;
                    self.position.y = y;
                    // Settle onto the new height quickly but not instantly
                    self.position.z += (z - self.position.z) * (1.0 - (-20.0 * dt).exp());
                }
                _ => {
                    // Something's in the way: turn around and try elsewhere. A boss charging
                    // into it is stopped dead, see bosses.rs.
                    if self.boss.is_none() {
                        self.target_heading = self.heading + PI + (random(&mut self.rng) - 0.5) * 1.5;
                    }
                    self.speed = 0.0;
                }
            }
            self.phase = (self.phase + self.speed * dt / look.stride()).fract();
        }
    }

    pub fn instance(&self, world_voxels_per_meter: u32) -> AnimalInstance {
        let walking = if self.speed > 0.15 { 1 + (self.phase * WALK_FRAMES as f64) as usize % WALK_FRAMES } else { 0 };
        let frame = self.boss.as_ref().and_then(Boss::pose).unwrap_or(walking);
        let mut scale = world_voxels_per_meter as f64 / self.look.resolution();
        let mut heading = self.heading;
        // Bosses and monsters breathe, swelling a little and settling, and sway as they stand,
        // so they're never still as statues between frames
        if self.look.build().is_some() {
            let offset = self.id as f64 * 0.7;
            scale *= 1.0 + 0.012 * (self.age * 2.1 + offset).sin();
            if self.speed < 0.15 {
                heading += 0.035 * (self.age * 1.3 + offset).sin();
            }
        }
        let radius = self.look.reach() * world_voxels_per_meter as f64;
        AnimalInstance::new(model_index(self.look, frame), self.position, facing(heading), scale, radius, MOVING).tinted(self.status.tint())
    }
}

/// A boss for the lair at `near`, in voxels with its z on the ground there: on open ground as near
/// it as there is, at about that height with room above, so not on a roof or in a wall. None if
/// there's nowhere within 25 meters.
pub fn spawn_boss(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, key: [i64; 3], seed: u32) -> Option<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    for ring in 0..26 {
        let count = (ring * 6).max(1);
        for i in 0..count {
            let a = i as f64 / count as f64 * TAU;
            let (x, y) = (near.x + a.cos() * ring as f64 * vpm, near.y + a.sin() * ring as f64 * vpm);
            if x < 2.0 || y < 2.0 || x >= sx - 2.0 || y >= sy - 2.0 {
                continue;
            }
            if let Some(z) = ground_at(world, water, x, y, near.z, 4.0 * vpm, 4.0 * vpm) {
                return Some(Animal::boss(look, Vector3::new(x, y, z), key, seed));
            }
        }
    }
    None
}

/// A monster of `look` on the ground near `near`, in voxels, about that high with room above it,
/// within `within` meters: None if there's nowhere
pub fn spawn_monster(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, within: f64, seed: u32) -> Option<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut rng = hash(seed ^ 0x5ca7);
    for _ in 0..24 {
        let a = random(&mut rng) * TAU;
        let r = random(&mut rng).sqrt() * within * vpm;
        let (x, y) = (near.x + a.cos() * r, near.y + a.sin() * r);
        if x < 2.0 || y < 2.0 || x >= sx - 2.0 || y >= sy - 2.0 {
            continue;
        }
        if let Some(z) = ground_at(world, water, x, y, near.z, 2.0 * vpm, 3.0 * vpm) {
            let key = [x as i64, y as i64, -2];
            return Some(Animal::boss(look, Vector3::new(x, y, z), key, hash(rng)));
        }
    }
    None
}

/// Scatters animals over dry land: about half of them close to `near` so there are some to see
/// straight away, the rest anywhere.
pub fn spawn(world: &World, water: WaterLevel, seed: u32, near: Vector3<f64>) -> Vec<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut rng = hash(seed ^ 0xa11a);
    let mut animals = Vec::new();
    for (look, count) in [(Look::Bear, 6), (Look::Dog, 8), (Look::GrayCat, 5), (Look::GingerCat, 5), (Look::Squirrel, 14)] {
        for i in 0..count {
            for _ in 0..64 {
                let (x, y) = if i % 2 == 0 {
                    let angle = random(&mut rng) * TAU;
                    let distance = (4.0 + 40.0 * random(&mut rng)) * vpm;
                    (near.x + angle.cos() * distance, near.y + angle.sin() * distance)
                } else {
                    (sx * (0.1 + 0.8 * random(&mut rng)), sy * (0.1 + 0.8 * random(&mut rng)))
                };
                if x < 1.0 || y < 1.0 || x >= sx - 1.0 || y >= sy - 1.0 {
                    continue;
                }
                if let Some(z) = surface_at(world, water, x, y) {
                    animals.push(Animal::new(look, Vector3::new(x, y, z), hash(rng)));
                    break;
                }
            }
        }
    }
    // Packs of wolves, out on grass or snow, well away from the player to begin with
    for _ in 0..WOLF_PACKS {
        for _ in 0..64 {
            let (x, y) = (sx * (0.1 + 0.8 * random(&mut rng)), sy * (0.1 + 0.8 * random(&mut rng)));
            if (Vector2::new(x, y) - near.xy()).norm() < 35.0 * vpm {
                continue;
            }
            let Some(z) = surface_at(world, water, x, y) else {
                continue;
            };
            if matches!(world.material_at(x as i64, y as i64, z as i64 - 1), SAND | SANDSTONE | SANDSTONE_DARK | JUNGLE_GRASS | VINE) {
                continue;
            }
            let den = Vector3::new(x, y, z);
            for k in 0..3 {
                animals.extend(spawn_monster(world, water, Look::Wolf, den, 4.0, hash(rng ^ k)));
            }
            break;
        }
    }
    animals
}

/// How many packs of wolves roam the world at once
const WOLF_PACKS: usize = 2;
