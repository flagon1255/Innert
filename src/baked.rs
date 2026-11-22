//! The monsters, the animals and the folk of the towns, drawn in baked frames as they always were,
//! one model a frame, so dozens of them cost no more to draw than ever: but the frames baked from
//! jointed pieces, as a boss is made of, see src\rig.rs. A head, a torso, an upper arm, a forearm,
//! a thigh, a shin, a tail, what's held in a hand: each its own model, voxelized once as the code
//! shapes it standing, and every frame of walking, every pose and every frame between poses baked
//! by putting each piece where that frame has it, see bake().
//!
//! Where the pieces go in each frame comes from the code that drew each frame whole before, see
//! animals::parts(): each of its shapes says which bone it's part of, and each bone's place in a
//! frame is worked out from where its shapes are there, the turn that best takes them there from
//! where they are standing, see fit(). Limbs whose code bends them further than they reach are bent
//! at their middle joint so their ends get where the code has them, see reach().
//!
//! So a piece edited in MagicaVoxel, assets\models\<look>\<piece>.vox, goes with its limb in every
//! frame, and every bone can be turned and moved in the model editor, its clips saved to
//! assets\anims\<build>\<look>\<clip>.json as the jointed bosses' are, see src\anim.rs: idle, walk,
//! and each of its poses, windup, strike and special. The frames are baked again whenever a piece's
//! file or a clip's is saved, in the editor and in a game running beside it. A standing frame
//! edited before there were pieces, `<look>/base.vox`, has what was changed in it put on the pieces
//! holding those places, see legacy(), so it goes with them too.

use std::path::Path;
use std::sync::{Mutex, OnceLock};

use nalgebra::{Matrix3, UnitQuaternion, Vector3};

use crate::anim;
use crate::animals::{self, AnimalInstance, Look, MODEL_RES, MOVING, Model, Part};
use crate::worldgen::material::AIR;

pub type V = Vector3<f64>;
pub type Q = UnitQuaternion<f64>;

/// Every look drawn in frames baked from pieces, in the order their pieces are among the models
pub const KINDS: [Look; 26] = [
    Look::Bear,
    Look::Dog,
    Look::GrayCat,
    Look::GingerCat,
    Look::Squirrel,
    Look::Villager,
    Look::Townswoman,
    Look::Wolf,
    Look::Skeleton,
    Look::ArmoredSkeleton,
    Look::Zombie,
    Look::ArmoredZombie,
    Look::Spiderling,
    Look::Bandit,
    Look::BanditBrute,
    Look::Crossbowman,
    Look::Shieldbearer,
    Look::DuneBurrower,
    Look::RimeWraith,
    Look::MireLurker,
    Look::Gloomwing,
    Look::Tusker,
    Look::Bloater,
    Look::Ghoul,
    Look::GraveBrute,
    Look::BoneArcher,
];

/// Pieces each has room for among the models, those it doesn't have left empty
pub const PIECES: usize = 32;

/// How many models the pieces are, see animals::BAKED_MODELS_AT
pub const MODELS: usize = KINDS.len() * PIECES;

/// Which of KINDS `look` is, if its frames are baked from pieces
pub fn kind(look: Look) -> Option<usize> {
    KINDS.iter().position(|&k| k == look)
}

/// Whether `look`'s frames are baked from pieces
pub fn baked(look: Look) -> bool {
    kind(look).is_some()
}

/// Which of four legs one at `x` ahead and `y` to the left is: the right fore leg, the left fore
/// leg, the right hind leg or the left hind leg
pub fn leg_of(x: f64, y: f64) -> usize {
    (x < 0.0) as usize * 2 + (y > 0.0) as usize
}

// The bones of each build, as its shapes are tagged with them in animals.rs: one more than their
// place in its skeleton(), 0 being none. Two of a kind are the right and the left.

pub(crate) mod beast {
    pub const BODY: u8 = 1;
    pub const HEAD: u8 = 2;
    pub const TAIL: u8 = 3;
    pub const TAIL_TIP: u8 = 4;
    /// The fore right, fore left, hind right and hind left, see super::leg_of()
    pub const UPPER: [u8; 4] = [5, 6, 7, 8];
    pub const LOWER: [u8; 4] = [9, 10, 11, 12];
}

pub(crate) mod wolf {
    pub const BODY: u8 = 1;
    pub const CHEST: u8 = 2;
    pub const NECK: u8 = 3;
    pub const HEAD: u8 = 4;
    pub const MUZZLE: u8 = 5;
    pub const JAW: u8 = 6;
    pub const TAIL: u8 = 7;
    pub const UPPER: [u8; 4] = [8, 9, 10, 11];
    pub const LOWER: [u8; 4] = [12, 13, 14, 15];
}

pub(crate) mod undead {
    pub const PELVIS: u8 = 1;
    pub const TORSO: u8 = 2;
    pub const HEAD: u8 = 3;
    pub const ARM: [u8; 2] = [4, 6];
    pub const FORE: [u8; 2] = [5, 7];
    pub const THIGH: [u8; 2] = [8, 10];
    pub const SHIN: [u8; 2] = [9, 11];
    /// The blade, the mace, the crossbow, the bow or the gravestone
    pub const HELD: u8 = 12;
    pub const SHIELD: u8 = 13;
    /// A bow's string as it hangs, and drawn back with an arrow on it
    pub const STRING: u8 = 14;
    pub const DRAWN: u8 = 15;
    /// A crossbow's bolt, till it's loosed
    pub const BOLT: u8 = 16;
}

pub(crate) mod spider {
    pub const BODY: u8 = 1;
    pub const ABDOMEN: u8 = 2;
    /// Four legs down the right, then four down the left
    pub const FEMUR: [u8; 8] = [3, 4, 5, 6, 7, 8, 9, 10];
    pub const TIBIA: [u8; 8] = [11, 12, 13, 14, 15, 16, 17, 18];
}

pub(crate) mod bloater {
    pub const BELLY: u8 = 1;
    pub const CHEST: u8 = 2;
    pub const HEAD: u8 = 3;
    pub const ARM: [u8; 2] = [4, 6];
    pub const FORE: [u8; 2] = [5, 7];
    pub const THIGH: [u8; 2] = [8, 10];
    pub const SHIN: [u8; 2] = [9, 11];
}

pub(crate) mod ghoul {
    pub const PELVIS: u8 = 1;
    pub const TORSO: u8 = 2;
    pub const HEAD: u8 = 3;
    pub const ARM: [u8; 2] = [4, 6];
    pub const FORE: [u8; 2] = [5, 7];
    pub const THIGH: [u8; 2] = [8, 10];
    pub const SHIN: [u8; 2] = [9, 11];
}

pub(crate) mod burrower {
    pub const BODY: u8 = 1;
    pub const HEAD: u8 = 2;
    pub const ARM: [u8; 2] = [3, 6];
    pub const CLAW: [u8; 2] = [4, 7];
    /// The finger of a pincer that closes
    pub const PINCER: [u8; 2] = [5, 8];
    /// Three legs down the right, then three down the left
    pub const THIGH: [u8; 6] = [9, 10, 11, 12, 13, 14];
    pub const SHIN: [u8; 6] = [15, 16, 17, 18, 19, 20];
    pub const TAIL: [u8; 5] = [21, 22, 23, 24, 25];
    pub const STINGER: u8 = 26;
}

pub(crate) mod wraith {
    pub const BODY: u8 = 1;
    pub const SHROUD: u8 = 2;
    pub const HEAD: u8 = 3;
    pub const ARM: [u8; 2] = [4, 6];
    pub const FORE: [u8; 2] = [5, 7];
    pub const RAG: [u8; 7] = [8, 9, 10, 11, 12, 13, 14];
}

pub(crate) mod lurker {
    pub const BODY: u8 = 1;
    pub const HEAD: u8 = 2;
    pub const JAW: u8 = 3;
    pub const MOUTH: u8 = 4;
    pub const TONGUE: u8 = 5;
    pub const TAIL: u8 = 6;
    pub const FROND: [u8; 4] = [7, 8, 9, 10];
    pub const UPPER: [u8; 4] = [11, 12, 13, 14];
    pub const LOWER: [u8; 4] = [15, 16, 17, 18];
}

pub(crate) mod gloomwing {
    pub const BODY: u8 = 1;
    pub const HEAD: u8 = 2;
    pub const INNER: [u8; 2] = [3, 5];
    pub const OUTER: [u8; 2] = [4, 6];
}

pub(crate) mod villager {
    pub const TORSO: u8 = 1;
    pub const HEAD: u8 = 2;
    pub const ARM: [u8; 2] = [3, 5];
    pub const FORE: [u8; 2] = [4, 6];
    pub const SKIRT: u8 = 7;
    pub const THIGH: [u8; 2] = [8, 10];
    pub const SHIN: [u8; 2] = [9, 11];
}

/// How a look's built, which decides its bones
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Build {
    Beast,
    Wolf,
    Undead,
    Spider,
    Bloater,
    Ghoul,
    Burrower,
    Wraith,
    Lurker,
    Gloomwing,
    Villager,
}

fn build_of(look: Look) -> Build {
    match look {
        Look::Bear | Look::Dog | Look::GrayCat | Look::GingerCat | Look::Squirrel | Look::Tusker => Build::Beast,
        Look::Wolf => Build::Wolf,
        Look::Spiderling => Build::Spider,
        Look::Bloater => Build::Bloater,
        Look::Ghoul => Build::Ghoul,
        Look::DuneBurrower => Build::Burrower,
        Look::RimeWraith => Build::Wraith,
        Look::MireLurker => Build::Lurker,
        Look::Gloomwing => Build::Gloomwing,
        Look::Villager | Look::Townswoman => Build::Villager,
        _ => Build::Undead,
    }
}

/// What a build's animation files are kept under, the first part of its rigs' ids, see id_of(): a
/// file there's for every look of that build without one of its own
fn build_name(b: Build) -> &'static str {
    match b {
        Build::Beast => "beast",
        Build::Wolf => "wolf",
        Build::Undead => "undead",
        Build::Spider => "spider",
        Build::Bloater => "bloater",
        Build::Ghoul => "ghoul",
        Build::Burrower => "burrower",
        Build::Wraith => "wraith",
        Build::Lurker => "lurker",
        Build::Gloomwing => "gloomwing",
        Build::Villager => "villager",
    }
}

/// What `look`'s animation files are kept under, assets\anims\<id>, as "undead/armored_skeleton"
pub fn id_of(look: Look) -> String {
    format!("{}/{}", build_name(build_of(look)), crate::assets::snake(&format!("{look:?}")))
}

/// A bone of a look's skeleton
#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// The tag its shapes carry, see animals::Part::on()
    tag: u8,
    /// The lower half of a limb: bent at the joint between it and the bone it hangs from so its
    /// end gets where the code puts it, see reach()
    reaches: bool,
    /// Carried by the bone it hangs from, as what's held in a hand is, rather than put where the
    /// code puts it, so it never comes away from the hand
    held: bool,
    /// Swelling and squashing as the code has it, rather than only turning
    swells: bool,
    /// Its joint, where the code has it standing, in meters at its build's size, if it isn't the
    /// end of it nearest what it hangs from
    joint: Option<V>,
}

/// The bones of `look`, in the order of the tags their shapes carry, parents first
fn skeleton(look: Look) -> Vec<Bone> {
    let mut v: Vec<Bone> = Vec::new();
    let mut add = |name: &str, parent: Option<u8>| {
        let tag = v.len() as u8 + 1;
        v.push(Bone { name: name.to_string(), parent: parent.map(|p| p as usize - 1), tag, reaches: false, held: false, swells: false, joint: None });
        v.len() - 1
    };
    let sides = ["right", "left"];
    let legs = ["fore_right", "fore_left", "hind_right", "hind_left"];
    let mut reach = Vec::new();
    let mut held = Vec::new();
    let mut swells = Vec::new();
    match build_of(look) {
        Build::Beast => {
            use beast::*;
            add("body", None);
            add("head", Some(BODY));
            add("tail", Some(BODY));
            add("tail_tip", Some(TAIL));
            for leg in legs {
                add(&format!("upper_{leg}"), Some(BODY));
            }
            for (k, leg) in legs.iter().enumerate() {
                reach.push(add(&format!("lower_{leg}"), Some(UPPER[k])));
            }
        }
        Build::Wolf => {
            use wolf::*;
            add("body", None);
            add("chest", Some(BODY));
            add("neck", Some(CHEST));
            add("head", Some(NECK));
            add("muzzle", Some(HEAD));
            add("jaw", Some(HEAD));
            add("tail", Some(BODY));
            for leg in legs {
                add(&format!("upper_{leg}"), Some(BODY));
            }
            for (k, leg) in legs.iter().enumerate() {
                reach.push(add(&format!("lower_{leg}"), Some(UPPER[k])));
            }
        }
        Build::Undead => {
            use undead::*;
            add("pelvis", None);
            add("torso", Some(PELVIS));
            add("head", Some(TORSO));
            for s in 0..2 {
                add(&format!("arm_{}", sides[s]), Some(TORSO));
                reach.push(add(&format!("forearm_{}", sides[s]), Some(ARM[s])));
            }
            for s in 0..2 {
                add(&format!("thigh_{}", sides[s]), Some(PELVIS));
                reach.push(add(&format!("shin_{}", sides[s]), Some(THIGH[s])));
            }
            // What's in its hands, held in one hand or both
            let (name, hand, one) = match look {
                Look::Crossbowman => ("crossbow", FORE[0], false),
                Look::Shieldbearer => ("mace", FORE[0], true),
                Look::BoneArcher => ("bow", FORE[1], true),
                Look::GraveBrute => ("gravestone", FORE[0], false),
                _ => ("blade", FORE[0], true),
            };
            let h = add(name, Some(hand));
            if one {
                held.push(h);
            }
            add("shield", Some(FORE[1]));
            add("string", Some(HELD));
            add("drawn_string", Some(HELD));
            add("bolt", Some(HELD));
        }
        Build::Spider => {
            use spider::*;
            add("body", None);
            add("abdomen", Some(BODY));
            let names: Vec<String> = (0..8).map(|k| format!("{}_{}", sides[k / 4], k % 4 + 1)).collect();
            for n in &names {
                add(&format!("femur_{n}"), Some(BODY));
            }
            for (k, n) in names.iter().enumerate() {
                reach.push(add(&format!("tibia_{n}"), Some(FEMUR[k])));
            }
        }
        Build::Bloater | Build::Ghoul => {
            let (root, chest) = if build_of(look) == Build::Bloater { ("belly", "chest") } else { ("pelvis", "torso") };
            let r = add(root, None);
            if build_of(look) == Build::Bloater {
                swells.push(r);
            }
            add(chest, Some(1));
            add("head", Some(2));
            for s in 0..2 {
                add(&format!("arm_{}", sides[s]), Some(2));
                reach.push(add(&format!("forearm_{}", sides[s]), Some(bloater::ARM[s])));
            }
            for s in 0..2 {
                add(&format!("thigh_{}", sides[s]), Some(1));
                reach.push(add(&format!("shin_{}", sides[s]), Some(bloater::THIGH[s])));
            }
        }
        Build::Burrower => {
            use burrower::*;
            add("body", None);
            add("head", Some(BODY));
            for s in 0..2 {
                add(&format!("arm_{}", sides[s]), Some(BODY));
                reach.push(add(&format!("claw_{}", sides[s]), Some(ARM[s])));
                add(&format!("pincer_{}", sides[s]), Some(CLAW[s]));
            }
            let names: Vec<String> = (0..6).map(|k| format!("{}_{}", sides[k / 3], k % 3 + 1)).collect();
            for n in &names {
                add(&format!("thigh_{n}"), Some(BODY));
            }
            for (k, n) in names.iter().enumerate() {
                reach.push(add(&format!("shin_{n}"), Some(THIGH[k])));
            }
            for k in 0..5 {
                add(&format!("tail_{}", k + 1), Some(if k == 0 { BODY } else { TAIL[k - 1] }));
            }
            add("stinger", Some(TAIL[4]));
        }
        Build::Wraith => {
            use wraith::*;
            add("body", None);
            add("shroud", Some(BODY));
            add("head", Some(BODY));
            for s in 0..2 {
                add(&format!("arm_{}", sides[s]), Some(BODY));
                reach.push(add(&format!("forearm_{}", sides[s]), Some(ARM[s])));
            }
            for k in 0..7 {
                add(&format!("rag_{}", k + 1), Some(SHROUD));
            }
        }
        Build::Lurker => {
            use lurker::*;
            add("body", None);
            add("head", Some(BODY));
            add("jaw", Some(HEAD));
            let m = add("mouth", Some(HEAD));
            swells.push(m);
            add("tongue", Some(JAW));
            add("tail", Some(BODY));
            for k in 0..4 {
                add(&format!("frond_{}", k + 1), Some(BODY));
            }
            for leg in legs {
                add(&format!("upper_{leg}"), Some(BODY));
            }
            for (k, leg) in legs.iter().enumerate() {
                reach.push(add(&format!("lower_{leg}"), Some(UPPER[k])));
            }
        }
        Build::Gloomwing => {
            use gloomwing::*;
            add("body", None);
            add("head", Some(BODY));
            for s in 0..2 {
                add(&format!("wing_{}_inner", sides[s]), Some(BODY));
                add(&format!("wing_{}_outer", sides[s]), Some(INNER[s]));
            }
        }
        Build::Villager => {
            use villager::*;
            add("torso", None);
            add("head", Some(TORSO));
            for s in 0..2 {
                add(&format!("arm_{}", sides[s]), Some(TORSO));
                reach.push(add(&format!("forearm_{}", sides[s]), Some(ARM[s])));
            }
            add("skirt", Some(TORSO));
            for s in 0..2 {
                add(&format!("thigh_{}", sides[s]), Some(TORSO));
                reach.push(add(&format!("shin_{}", sides[s]), Some(THIGH[s])));
            }
        }
    }
    for i in reach {
        v[i].reaches = true;
    }
    for i in held {
        v[i].held = true;
    }
    for i in swells {
        v[i].swells = true;
    }
    // A tusker's head nods about its neck
    if look == Look::Tusker {
        v[beast::HEAD as usize - 1].joint = Some(V::new(0.55, 0.0, 0.95));
    }
    v
}

/// Where a bone's joint is, which it's turned about
#[derive(Clone, Copy, Debug)]
enum Joint {
    /// One of its points, see points_of()
    Point(usize),
    /// The middle of its points
    Middle,
    /// The middle of the box round its shapes, for what swells
    Swelling,
    /// Somewhere set, in meters
    At(V),
}

/// Where a bone goes in a frame, in meters from the middle of its feet with +x ahead: where its
/// joint is, how it's turned from how it stands, how much it's swollen along each of its own
/// directions, and whether it's drawn
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub at: V,
    pub turn: Q,
    pub scale: V,
    pub shown: bool,
}

impl Pose {
    fn hidden() -> Pose {
        Pose { at: V::zeros(), turn: Q::identity(), scale: V::repeat(1.0), shown: false }
    }

    /// `f` of the way from `self` to `b`
    fn mix(&self, b: &Pose, f: f64) -> Pose {
        let turn = self.turn.try_slerp(&b.turn, f, 1e-9).unwrap_or(if f < 0.5 { self.turn } else { b.turn });
        Pose { at: self.at.lerp(&b.at, f), turn, scale: self.scale.lerp(&b.scale, f), shown: if f < 0.5 { self.shown } else { b.shown } }
    }
}

/// A bone's voxels, as the code shapes it standing: each a material, AIR for one cut away by a
/// shape of air, with which of the frame's shapes took it, lower drawn over higher, so pieces put
/// back where they were make the frame they came from voxel for voxel
#[derive(Clone)]
pub struct Piece {
    pub model: Model,
    /// u16::MAX for a voxel no shape took
    order: Vec<u16>,
    /// Where its first voxel is in the frame it came from, which is `size` big
    lo: [i32; 3],
    size: [u32; 3],
    /// Its joint in that frame, in meters from the middle of the feet
    pub joint: V,
}

impl Piece {
    fn empty() -> Piece {
        Piece { model: Model { size: [1, 1, 1], voxels: vec![AIR] }, order: vec![u16::MAX], lo: [0; 3], size: [1, 1, 1], joint: V::zeros() }
    }

    pub fn is_empty(&self) -> bool {
        self.order.iter().all(|&o| o == u16::MAX)
    }

    /// Where its first voxel's corner is, in meters from the middle of the feet in the frame it
    /// came from
    fn corner(&self) -> V {
        V::new((self.lo[0] as f64 - self.size[0] as f64 * 0.5) / MODEL_RES, (self.lo[1] as f64 - self.size[1] as f64 * 0.5) / MODEL_RES, self.lo[2] as f64 / MODEL_RES)
    }

    /// Where the middle of its model's bottom is from its joint, in meters: what its file's placed
    /// by, as a jointed boss's pieces are
    pub fn anchor(&self) -> V {
        let s = self.model.size;
        self.corner() + V::new(s[0] as f64 * 0.5, s[1] as f64 * 0.5, 0.0) / MODEL_RES - self.joint
    }

    /// The same with its voxels `model`, whose first voxel's `lo` from where this one's is, as read
    /// from a file: voxels it had keep their order, new ones drawn over the rest
    pub fn with(&self, model: Model, lo: [i32; 3]) -> Piece {
        let first = self.order.iter().copied().filter(|&o| o != u16::MAX).min().unwrap_or(0);
        let mut order = vec![u16::MAX; model.voxels.len()];
        let [sx, sy, sz] = model.size;
        let [ox, oy, oz] = self.model.size.map(|v| v as i32);
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let i = (x + (y + z * sy) * sx) as usize;
                    if model.voxels[i] == AIR {
                        continue;
                    }
                    let q = [x as i32 + lo[0], y as i32 + lo[1], z as i32 + lo[2]];
                    let inside = q[0] >= 0 && q[1] >= 0 && q[2] >= 0 && q[0] < ox && q[1] < oy && q[2] < oz;
                    let was = if inside { self.order[(q[0] + (q[1] + q[2] * oy) * ox) as usize] } else { u16::MAX };
                    order[i] = if was == u16::MAX { first } else { was };
                }
            }
        }
        Piece { model, order, lo: [0, 1, 2].map(|a| self.lo[a] + lo[a]), size: self.size, joint: self.joint }
    }

    /// The same in other materials
    pub fn recolored(&self, map: &[u8; 256]) -> Piece {
        Piece { model: Model { size: self.model.size, voxels: self.model.voxels.iter().map(|&v| map[v as usize]).collect() }, ..self.clone() }
    }
}

/// A look's frames as baked from its pieces: its bones, their pieces as the code shapes them,
/// where each goes in every frame, and the frames baked from them
pub struct Rig {
    pub look: Look,
    pub bones: Vec<Bone>,
    pub pieces: Vec<Piece>,
    /// Each frame's size and where each bone goes in it, None for a frame it's never drawn in
    poses: Vec<Option<([u32; 3], Vec<Pose>)>>,
    /// The frames as the code has them, every one, those it's never drawn in empty
    pub frames: Vec<Model>,
}

/// The points of `part` that are bone `tag`'s, by which it's followed from frame to frame: its
/// ends and its middle, the half of it that's the bone's for one shared by two
fn points_of(part: &Part, tag: u8) -> Vec<V> {
    let (own, far) = part.bones();
    if own != tag && far != tag {
        return Vec::new();
    }
    let (a, b, _) = part.ends();
    let (a, b) = (V::from(a), V::from(b));
    let mid = (a + b) * 0.5;
    if far != 0 {
        return if own == tag { vec![a, mid] } else { vec![mid, b] };
    }
    if (a - b).norm() < 1e-9 { vec![a] } else { vec![a, b, mid] }
}

fn points(parts: &[Part], tag: u8) -> Vec<V> {
    parts.iter().flat_map(|p| points_of(p, tag)).collect()
}

/// The box round the shapes of bone `tag`, as they reach at least a voxel thick, as they're drawn
fn bounds(parts: &[Part], tag: u8) -> Option<(V, V)> {
    let mut out: Option<(V, V)> = None;
    for p in parts.iter().filter(|p| p.bones().0 == tag || p.bones().1 == tag) {
        let (a, b) = p.thickened(MODEL_RES).reach();
        let (a, b) = (V::from(a), V::from(b));
        out = Some(match out {
            None => (a, b),
            Some((lo, hi)) => (lo.inf(&a), hi.sup(&b)),
        });
    }
    out
}

/// How far `q` is from the surface of `part`, 0 inside it
fn distance(part: &Part, q: V) -> f64 {
    let (a, b, boxed) = part.ends();
    let (a, b) = (V::from(a), V::from(b));
    if boxed {
        let d = V::new((a.x - q.x).max(q.x - b.x).max(0.0), (a.y - q.y).max(q.y - b.y).max(0.0), (a.z - q.z).max(q.z - b.z).max(0.0));
        return d.norm();
    }
    let ab = b - a;
    let t = if ab.norm_squared() > 0.0 { ((q - a).dot(&ab) / ab.norm_squared()).clamp(0.0, 1.0) } else { 0.0 };
    ((q - (a + ab * t)).norm() - part.girth()).max(0.0)
}

/// The turn that best takes points `from` to points `to`, about their middles: for points all in a
/// line, as a limb's, the least turn that lays the line along where it's gone, so it doesn't twist
/// about its length
fn fit(from: &[V], to: &[V]) -> Q {
    let n = from.len().min(to.len());
    if n == 0 {
        return Q::identity();
    }
    let middle = |v: &[V]| v[..n].iter().sum::<V>() / n as f64;
    let (cp, cq) = (middle(from), middle(to));
    let mut h = Matrix3::zeros();
    for (p, q) in from.iter().zip(to).take(n) {
        h += (p - cp) * (q - cq).transpose();
    }
    let svd = h.svd(true, true);
    let (Some(u), Some(v_t)) = (svd.u, svd.v_t) else { return Q::identity() };
    let s = svd.singular_values;
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| s[j].total_cmp(&s[i]));
    if s[order[0]] < 1e-12 {
        return Q::identity();
    }
    if s[order[1]] < 1e-4 * s[order[0]] {
        // In a line: the point furthest from the middle shows which way it lies
        let far = (0..n).max_by(|&i, &j| (from[i] - cp).norm_squared().total_cmp(&(from[j] - cp).norm_squared())).unwrap_or(0);
        let (a, b) = (from[far] - cp, to[far] - cq);
        return Q::rotation_between(&a, &b).unwrap_or_else(|| {
            let side = a.cross(&V::z()).try_normalize(1e-9).unwrap_or_else(V::x);
            Q::from_axis_angle(&nalgebra::Unit::new_normalize(side), std::f64::consts::PI)
        });
    }
    let v = v_t.transpose();
    let mut d = Matrix3::identity();
    if (v * u.transpose()).determinant() < 0.0 {
        d[(order[2], order[2])] = -1.0;
    }
    let r = v * d * u.transpose();
    Q::from_rotation_matrix(&nalgebra::Rotation3::from_matrix_unchecked(r))
}

/// The voxels of bone `tag`'s shapes in a frame `size` big, `parts` thickened: each taken by the
/// first of the bone's shapes it's in, as the whole frame's are by the first of all its shapes
fn cut(parts: &[Part], size: [u32; 3], tag: u8, joint: V) -> Piece {
    let res = MODEL_RES;
    let n = (size[0] * size[1] * size[2]) as usize;
    let mut voxels = vec![AIR; n];
    let mut order = vec![u16::MAX; n];
    let (mut lo, mut hi) = ([u32::MAX; 3], [0u32; 3]);
    for (k, part) in parts.iter().enumerate() {
        let (own, far) = part.bones();
        if own != tag && far != tag {
            continue;
        }
        // A shape shared by two bones, a leg's thigh and shin, is split across its middle, each
        // half keeping the round of it there so a bent knee stays whole
        let (a, b, _) = part.ends();
        let mid = V::from(a).lerp(&V::from(b), 0.5);
        let (ra, rb) = part.radii();
        let round = (ra + rb) * 0.5;
        let (from, to) = animals::frame_span(part, size, res);
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let i = (x + (y + z * size[1]) * size[0]) as usize;
                    if order[i] != u16::MAX {
                        continue;
                    }
                    let p = animals::frame_voxel(size, res, x, y, z);
                    if !part.contains(p) {
                        continue;
                    }
                    if far != 0 {
                        let t = part.along(p);
                        let near = (V::from(p) - mid).norm() <= round;
                        if (own == tag && t > 0.5 && !near) || (far == tag && t < 0.5 && !near) {
                            continue;
                        }
                    }
                    order[i] = k as u16;
                    voxels[i] = animals::banded(part, x);
                    for (axis, c) in [x, y, z].into_iter().enumerate() {
                        lo[axis] = lo[axis].min(c);
                        hi[axis] = hi[axis].max(c + 1);
                    }
                }
            }
        }
    }
    if lo[0] == u32::MAX {
        return Piece::empty();
    }
    let s = [0, 1, 2].map(|a| hi[a] - lo[a]);
    let mut model = Model { size: s, voxels: vec![AIR; (s[0] * s[1] * s[2]) as usize] };
    let mut kept = vec![u16::MAX; model.voxels.len()];
    for z in 0..s[2] {
        for y in 0..s[1] {
            for x in 0..s[0] {
                let from = ((x + lo[0]) + ((y + lo[1]) + (z + lo[2]) * size[1]) * size[0]) as usize;
                let to = (x + (y + z * s[1]) * s[0]) as usize;
                model.voxels[to] = voxels[from];
                kept[to] = order[from];
            }
        }
    }
    Piece { model, order: kept, lo: lo.map(|v| v as i32), size, joint }
}

/// Makes `look`'s rig: its bones, their pieces as the code shapes them standing, where they go in
/// every frame, and every frame baked from them
pub fn build(look: Look) -> Rig {
    let res = MODEL_RES;
    let frames: Vec<Option<Vec<Part>>> = (0..animals::FRAMES).map(|f| animals::shown_in(look, f).then(|| animals::frame_parts(look, f))).collect();
    let all = skeleton(look);
    // Only the bones it has shapes for
    let used: Vec<bool> = all.iter().map(|b| frames.iter().flatten().any(|parts| parts.iter().any(|p| p.bones().0 == b.tag || p.bones().1 == b.tag))).collect();
    let mut index = vec![None; all.len()];
    let mut bones: Vec<Bone> = Vec::new();
    for (i, b) in all.iter().enumerate() {
        if used[i] {
            index[i] = Some(bones.len());
            bones.push(b.clone());
        }
    }
    for b in &mut bones {
        let mut up = b.parent;
        b.parent = loop {
            match up {
                None => break None,
                Some(p) if index[p].is_some() => break index[p],
                Some(p) => up = all[p].parent,
            }
        };
    }
    let n = bones.len();
    assert!(n <= PIECES, "{look:?} has {n} bones");
    let scale = look.scale();
    // Each bone stands as it does in the first frame it's in
    let rest: Vec<usize> = bones.iter().map(|b| (0..frames.len()).find(|&f| frames[f].as_ref().is_some_and(|parts| !points(parts, b.tag).is_empty())).unwrap_or(0)).collect();
    let rest_points: Vec<Vec<V>> = bones.iter().zip(&rest).map(|(b, &f)| points(frames[f].as_deref().unwrap_or(&[]), b.tag)).collect();
    let joints: Vec<Joint> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if let Some(j) = b.joint {
                return Joint::At(j * scale);
            }
            if b.swells {
                return Joint::Swelling;
            }
            let Some(parent) = b.parent else { return Joint::Middle };
            // The end of it nearest what it hangs from, standing
            let parts = frames[rest[i]].as_deref().unwrap_or(&[]);
            let theirs: Vec<&Part> = parts.iter().filter(|p| p.bones().0 == bones[parent].tag || p.bones().1 == bones[parent].tag).collect();
            let near = |q: &V| theirs.iter().map(|p| distance(p, *q)).fold(f64::MAX, f64::min);
            let best = (0..rest_points[i].len()).min_by(|&x, &y| near(&rest_points[i][x]).total_cmp(&near(&rest_points[i][y]))).unwrap_or(0);
            Joint::Point(best)
        })
        .collect();
    let joint_in = |i: usize, parts: &[Part], pts: &[V], turn: &Q| -> V {
        let middle = |v: &[V]| if v.is_empty() { V::zeros() } else { v.iter().sum::<V>() / v.len() as f64 };
        match joints[i] {
            Joint::Point(k) => pts.get(k).copied().unwrap_or_else(|| middle(pts)),
            Joint::Middle => middle(pts),
            Joint::Swelling => bounds(parts, bones[i].tag).map_or_else(|| middle(pts), |(lo, hi)| (lo + hi) * 0.5),
            Joint::At(j) => middle(pts) + turn * (j - middle(&rest_points[i])),
        }
    };
    let rest_joint: Vec<V> = (0..n).map(|i| joint_in(i, frames[rest[i]].as_deref().unwrap_or(&[]), &rest_points[i], &Q::identity())).collect();
    // The pieces, each cut from the frame it stands in
    let sizes: Vec<[u32; 3]> = frames.iter().map(|f| f.as_ref().map_or([1, 1, 1], |parts| animals::frame_size(&parts.iter().map(|p| p.thickened(res)).collect::<Vec<_>>(), res))).collect();
    let thick: Vec<Option<Vec<Part>>> = (0..frames.len()).map(|f| rest.contains(&f).then(|| frames[f].as_ref().map(|parts| parts.iter().map(|p| p.thickened(res)).collect())).flatten()).collect();
    let pieces: Vec<Piece> = bones.iter().enumerate().map(|(i, b)| thick[rest[i]].as_ref().map_or_else(Piece::empty, |parts| cut(parts, sizes[rest[i]], b.tag, rest_joint[i]))).collect();
    // Where each bone goes in every frame
    let poses: Vec<Option<([u32; 3], Vec<Pose>)>> = frames
        .iter()
        .enumerate()
        .map(|(f, parts)| {
            let parts = parts.as_ref()?;
            let mut out: Vec<Pose> = vec![Pose::hidden(); n];
            let mut now: Vec<Vec<V>> = Vec::with_capacity(n);
            for i in 0..n {
                let pts = points(parts, bones[i].tag);
                let ok = !pts.is_empty() && pts.len() == rest_points[i].len();
                if f == rest[i] {
                    out[i] = Pose { at: rest_joint[i], turn: Q::identity(), scale: V::repeat(1.0), shown: true };
                } else if ok {
                    let (turn, scale) = if bones[i].swells {
                        let grown = match (bounds(parts, bones[i].tag), bounds(frames[rest[i]].as_deref().unwrap_or(&[]), bones[i].tag)) {
                            (Some((a, b)), Some((c, d))) => (b - a).component_div(&(d - c).sup(&V::repeat(1e-6))),
                            _ => V::repeat(1.0),
                        };
                        (Q::identity(), grown)
                    } else {
                        (fit(&rest_points[i], &pts), V::repeat(1.0))
                    };
                    out[i] = Pose { at: joint_in(i, parts, &pts, &turn), turn, scale, shown: true };
                } else if let Some(p) = bones[i].parent.filter(|_| !pts.is_empty()) {
                    // Shaped differently than standing: carried by what it hangs from
                    let up = out[p];
                    out[i] = Pose { at: up.at + up.turn * (rest_joint[i] - rest_joint[p]), turn: up.turn, scale: V::repeat(1.0), shown: true };
                }
                now.push(pts);
            }
            // Limbs bent so their ends get where the code has them
            for i in 0..n {
                if !bones[i].reaches || f == rest[i] || !out[i].shown {
                    continue;
                }
                let Some(u) = bones[i].parent.filter(|&u| out[u].shown && rest[u] == rest[i]) else { continue };
                reach(&mut out, u, i, &rest_joint, &rest_points[i], &now[i]);
            }
            // What's held in a hand goes with it
            for i in 0..n {
                if let Some(p) = bones[i].parent.filter(|_| bones[i].held && out[i].shown && f != rest[i]) {
                    let up = out[p];
                    out[i].at = up.at + up.turn * (rest_joint[i] - rest_joint[p]);
                }
            }
            Some((sizes[f], out))
        })
        .collect();
    let mut rig = Rig { look, bones, pieces, poses, frames: Vec::new() };
    rig.frames = rig.bake(&rig.pieces, None);
    rig
}

/// Bends limb `lower`, hanging from `upper`, at the joint between them, keeping their lengths as
/// they stand, so the end of `lower`, the point of it furthest from its joint, gets as near as it
/// can to where the code has it, `now`, bending the way the code bends it
fn reach(out: &mut [Pose], upper: usize, lower: usize, rest_joint: &[V], rest_points: &[V], now: &[V]) {
    let Some(end) = (0..rest_points.len()).max_by(|&a, &b| (rest_points[a] - rest_joint[lower]).norm_squared().total_cmp(&(rest_points[b] - rest_joint[lower]).norm_squared())) else { return };
    let (root, elbow, target) = (out[upper].at, out[lower].at, now[end]);
    let (up_len, low_len) = ((rest_joint[lower] - rest_joint[upper]).norm(), (rest_points[end] - rest_joint[lower]).norm());
    if up_len < 1e-6 || low_len < 1e-6 {
        return;
    }
    // Already where it should be
    let carried = root + out[upper].turn * (rest_joint[lower] - rest_joint[upper]);
    let reached = elbow + out[lower].turn * (rest_points[end] - rest_joint[lower]);
    if (carried - elbow).norm() < 1e-3 && (reached - target).norm() < 1e-3 {
        return;
    }
    let pole = elbow - (root + target) * 0.5;
    let (knee, foot) = crate::rig::bend(root, target, up_len, low_len, pole);
    let aim = |turn: Q, from: V, to: V| Q::rotation_between(&(turn * from), &to).map_or(turn, |r| r * turn);
    out[upper].turn = aim(out[upper].turn, rest_joint[lower] - rest_joint[upper], knee - root);
    out[lower].at = knee;
    out[lower].turn = aim(out[lower].turn, rest_points[end] - rest_joint[lower], foot - knee);
}

/// Puts `piece` where `pose` puts it into a frame `size` big, voxel by voxel, each going where no
/// voxel drawn over it is, and saying whose it is in `owner` if asked
#[allow(clippy::too_many_arguments)]
fn place(piece: &Piece, pose: &Pose, size: [u32; 3], voxels: &mut [u8], order: &mut [u16], owner: &mut Option<&mut Vec<usize>>, who: usize) {
    let res = MODEL_RES;
    let [sx, sy, sz] = piece.model.size;
    let mut put = |d: usize, m: u8, o: u16, voxels: &mut [u8], order: &mut [u16]| {
        if o < order[d] {
            order[d] = o;
            voxels[d] = m;
            if let Some(w) = owner.as_deref_mut() {
                w[d] = who;
            }
        }
    };
    let corner = piece.corner();
    let half = V::new(size[0] as f64 * 0.5, size[1] as f64 * 0.5, 0.0);
    if pose.turn.angle() < 1e-9 && (pose.scale - V::repeat(1.0)).norm() < 1e-9 {
        // Only moved: copied over whole
        let shift = (pose.at - piece.joint + corner) * res + half;
        let shift = [shift.x.round() as i64, shift.y.round() as i64, shift.z.round() as i64];
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let i = (x + (y + z * sy) * sx) as usize;
                    let o = piece.order[i];
                    if o == u16::MAX {
                        continue;
                    }
                    let d = [x as i64 + shift[0], y as i64 + shift[1], z as i64 + shift[2]];
                    if d.iter().zip(size).any(|(&c, s)| c < 0 || c >= s as i64) {
                        continue;
                    }
                    put((d[0] + (d[1] + d[2] * size[1] as i64) * size[0] as i64) as usize, piece.model.voxels[i], o, voxels, order);
                }
            }
        }
        return;
    }
    // Turned: each voxel of the frame its piece covers takes the voxel of the piece nearest it
    let back = pose.turn.inverse();
    let local = |w: V| piece.joint + (back * (w - pose.at)).component_div(&pose.scale);
    let world = |l: V| pose.at + pose.turn * (l - piece.joint).component_mul(&pose.scale);
    let (mut lo, mut hi) = (V::repeat(f64::MAX), V::repeat(f64::MIN));
    for c in 0..8 {
        let l = corner + V::new(if c & 1 == 0 { 0.0 } else { sx as f64 }, if c & 2 == 0 { 0.0 } else { sy as f64 }, if c & 4 == 0 { 0.0 } else { sz as f64 }) / res;
        let w = world(l);
        lo = lo.inf(&w);
        hi = hi.sup(&w);
    }
    let to_voxel = |w: V| (w * res + half).map(|v| v.floor());
    let (from, to) = (to_voxel(lo), to_voxel(hi));
    let range = |a: usize| (from[a].max(0.0) as u32)..((to[a] + 1.0).min(size[a] as f64).max(0.0) as u32);
    for z in range(2) {
        for y in range(1) {
            for x in range(0) {
                let w = V::from(animals::frame_voxel(size, res, x, y, z));
                let q = (local(w) - corner) * res;
                let (i, j, k) = (q.x.floor(), q.y.floor(), q.z.floor());
                if i < 0.0 || j < 0.0 || k < 0.0 || i >= sx as f64 || j >= sy as f64 || k >= sz as f64 {
                    continue;
                }
                let at = (i as u32 + (j as u32 + k as u32 * sy) * sx) as usize;
                let o = piece.order[at];
                if o != u16::MAX {
                    put((x + (y + z * size[1]) * size[0]) as usize, piece.model.voxels[at], o, voxels, order);
                }
            }
        }
    }
}

/// A frame `size` big at least, with `pieces` put where `poses` has them, grown the same either
/// side of its middle if they reach past it, and whose voxel each is in `owner` if asked
fn compose(pieces: &[Piece], poses: &[Pose], size: [u32; 3], mut owner: Option<&mut Vec<usize>>) -> Model {
    let res = MODEL_RES;
    let mut size = size;
    for (piece, pose) in pieces.iter().zip(poses) {
        if !pose.shown || piece.is_empty() {
            continue;
        }
        let corner = piece.corner();
        let s = piece.model.size.map(|v| v as f64 / res);
        for c in 0..8 {
            let l = corner + V::new(if c & 1 == 0 { 0.0 } else { s[0] }, if c & 2 == 0 { 0.0 } else { s[1] }, if c & 4 == 0 { 0.0 } else { s[2] });
            let w = pose.at + pose.turn * (l - piece.joint).component_mul(&pose.scale);
            for (a, need) in [(0, 2.0 * w.x.abs()), (1, 2.0 * w.y.abs())] {
                let need = (need * res - 0.01).ceil() as u32;
                if need > size[a] {
                    size[a] += (need - size[a]).div_ceil(2) * 2;
                }
            }
            size[2] = size[2].max((w.z * res - 0.01).ceil() as u32);
        }
    }
    let size = size.map(|v| v.clamp(1, 255));
    let n = (size[0] * size[1] * size[2]) as usize;
    let mut voxels = vec![AIR; n];
    let mut order = vec![u16::MAX; n];
    if let Some(w) = owner.as_deref_mut() {
        *w = vec![usize::MAX; n];
    }
    for (who, (piece, pose)) in pieces.iter().zip(poses).enumerate() {
        if pose.shown && !piece.is_empty() {
            place(piece, pose, size, &mut voxels, &mut order, &mut owner, who);
        }
    }
    Model { size, voxels }
}

impl Rig {
    /// The skeleton as the editor and the animation files see it
    pub fn bone_defs(&self) -> Vec<anim::BoneDef> {
        self.bones.iter().map(|b| anim::BoneDef { name: b.name.clone(), parent: b.parent }).collect()
    }

    /// Where each bone goes in frame `frame` as the code has it, if it's drawn in it
    pub fn code_pose(&self, frame: usize) -> Option<&[Pose]> {
        self.poses.get(frame)?.as_ref().map(|(_, p)| p.as_slice())
    }

    /// Every frame baked from `pieces`, with `deltas` on top of where the code puts each bone in
    /// each, see deltas(), as the animation files have them unless given
    pub fn bake(&self, pieces: &[Piece], deltas: Option<&Deltas>) -> Vec<Model> {
        let read;
        let deltas = match deltas {
            Some(d) => Some(d),
            None => {
                read = self.deltas();
                read.as_ref()
            }
        };
        let defs = self.bone_defs();
        self.poses
            .iter()
            .enumerate()
            .map(|(f, frame)| match frame {
                None => Model { size: [1, 1, 1], voxels: vec![AIR] },
                Some((size, poses)) => {
                    let poses = match deltas.and_then(|d| d.get(f)) {
                        Some(d) => moved(&defs, poses, d),
                        None => poses.clone(),
                    };
                    compose(pieces, &poses, *size, None)
                }
            })
            .collect()
    }

    /// The standing frame from `pieces`, and whose each of its voxels is, usize::MAX for none
    fn standing(&self, pieces: &[Piece]) -> (Model, Vec<usize>) {
        let mut owner = Vec::new();
        let (size, poses) = self.poses[0].as_ref().expect("every look stands");
        let model = compose(pieces, poses, *size, Some(&mut owner));
        (model, owner)
    }

    /// How far each bone's turned and moved in each frame by the animation files, if there are any
    /// for it, see deltas_of()
    pub fn deltas(&self) -> Option<Deltas> {
        if !anim::active() {
            return None;
        }
        let id = id_of(self.look);
        let clips: Vec<Option<std::sync::Arc<anim::Clip>>> = CLIPS.iter().map(|c| anim::load(&id, c)).collect();
        if clips.iter().all(Option::is_none) {
            return None;
        }
        Some(self.deltas_of(&|c, name, t| clips[c].as_ref().and_then(|clip| clip.sample(name, t))))
    }

    /// How far each bone's turned and moved in each frame, `sample` giving it in clip c of CLIPS at
    /// a moment of it: a walking frame from its moment of the walk, a pose from the end of its
    /// clip, a frame on the way from standing into a pose from its moment along that clip, and a
    /// frame between two poses partway from the one's to the other's
    pub fn deltas_of(&self, sample: &dyn Fn(usize, &str, f64) -> Option<([f64; 3], [f64; 3])>) -> Deltas {
        let walk = walk_seconds(self.look);
        let mix = |a: Option<([f64; 3], [f64; 3])>, b: Option<([f64; 3], [f64; 3])>, f: f64| {
            if a.is_none() && b.is_none() {
                return None;
            }
            let (a, b) = (a.unwrap_or_default(), b.unwrap_or_default());
            Some(([0, 1, 2].map(|i| a.0[i] + (b.0[i] - a.0[i]) * f), [0, 1, 2].map(|i| a.1[i] + (b.1[i] - a.1[i]) * f)))
        };
        (0..self.poses.len())
            .map(|f| {
                self.bones
                    .iter()
                    .map(|b| {
                        let name = b.name.as_str();
                        let pose_clip = |p: usize| [animals::POSE_WINDUP, animals::POSE_STRIKE, animals::POSE_SPECIAL].iter().position(|&q| q == p).map(|k| k + 2);
                        match f {
                            0 => sample(0, name, 0.0),
                            _ if f <= WALK => sample(1, name, (f - 1) as f64 / WALK as f64 * walk),
                            _ => {
                                if let Some(c) = pose_clip(f) {
                                    return sample(c, name, 1.0);
                                }
                                let (from, to, along) = animals::tween_ends(f)?;
                                match (pose_clip(from), pose_clip(to)) {
                                    (None, Some(c)) if from == 0 => sample(c, name, along),
                                    (Some(c), None) if to == 0 => sample(c, name, 1.0 - along),
                                    (Some(a), Some(c)) => mix(sample(a, name, 1.0), sample(c, name, 1.0), along),
                                    _ => None,
                                }
                            }
                        }
                    })
                    .collect()
            })
            .collect()
    }
}

/// How far each bone's turned and moved from the code's pose, in degrees and meters, frame by frame
pub type Deltas = Vec<Vec<Option<([f64; 3], [f64; 3])>>>;

/// `poses` with each bone turned and moved by `deltas`, what hangs from it going with it, see
/// anim::apply_deltas()
fn moved(defs: &[anim::BoneDef], poses: &[Pose], deltas: &[Option<([f64; 3], [f64; 3])>]) -> Vec<Pose> {
    if deltas.iter().all(Option::is_none) {
        return poses.to_vec();
    }
    let mut joints: Vec<anim::Joint> = poses.iter().map(|p| anim::Joint { at: p.at, turn: p.turn, shown: p.shown }).collect();
    anim::apply_deltas(defs, &mut joints, &|b| deltas.get(b).copied().flatten(), 1.0);
    poses.iter().zip(joints).map(|(p, j)| Pose { at: j.at, turn: j.turn, ..*p }).collect()
}

/// Frames of walking, see animals::WALK_FRAMES
const WALK: usize = animals::POSE_WINDUP - 1;

/// A look's clips, in the editor and its files: standing, walking, and the three poses a monster
/// strikes, see animals::POSE_WINDUP. A gloomwing's walk is its wings beating, and a wraith's its
/// rags stirring, even as it stands.
pub const CLIPS: [&str; 5] = ["idle", "walk", "windup", "strike", "special"];

/// How many of CLIPS `look` has: animals and the folk of the towns only stand and walk
fn clips(look: Look) -> usize {
    if look.build().is_some() { CLIPS.len() } else { 2 }
}

/// Seconds a cycle of `look`'s walk takes at its walking pace, the length of its walk clip
fn walk_seconds(look: Look) -> f64 {
    look.stride() / look.walk_speed().max(0.1)
}

// Every look's rig, made once

static RIGS: OnceLock<Vec<Rig>> = OnceLock::new();

/// Whether the rigs have been made yet, see animals::animal_models(), which makes them alongside the
/// rest of the frames
pub fn made() -> bool {
    RIGS.get().is_some()
}

/// Keeps the rigs `rigs`, in the order of KINDS, made by animals::animal_models()
pub fn keep(rigs: Vec<Rig>) {
    let _ = RIGS.set(rigs);
}

/// Every look's rig, in the order of KINDS: made here, on every core at once, if they haven't been
pub fn rigs() -> &'static [Rig] {
    RIGS.get_or_init(|| {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
        let mut made: Vec<Rig> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut out = Vec::new();
                        loop {
                            let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if k >= KINDS.len() {
                                break out;
                            }
                            out.push(build(KINDS[k]));
                        }
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().expect("making a rig failed")).collect()
        });
        made.sort_by_key(|r| kind(r.look));
        made
    })
}

/// `look`'s rig, if its frames are baked from pieces
pub fn rig(look: Look) -> Option<&'static Rig> {
    rigs().get(kind(look)?)
}

/// The pieces' models, to go after the avians', see animals::BAKED_MODELS_AT: each look's in the
/// order of its bones, PIECES to a look
pub fn models() -> Vec<Model> {
    let mut out = Vec::with_capacity(MODELS);
    for rig in rigs() {
        for p in 0..PIECES {
            out.push(rig.pieces.get(p).map_or_else(|| Piece::empty().model, |piece| piece.model.clone()));
        }
    }
    out
}

/// Where each look's pieces are drawn from their joints as they are now, files and all, by KINDS,
/// for the editor: the middle of each one's bottom from its joint, and its size
static LAYOUT: Mutex<Vec<Option<Vec<(V, [u32; 3])>>>> = Mutex::new(Vec::new());

/// Tells the editor `look`'s pieces are now `pieces`, as read from their files
pub fn laid_out(look: Look, pieces: &[Piece]) {
    let Some(k) = kind(look) else { return };
    let mut layout = LAYOUT.lock().unwrap_or_else(|e| e.into_inner());
    if layout.len() < KINDS.len() {
        layout.resize(KINDS.len(), None);
    }
    layout[k] = Some(pieces.iter().map(|p| (p.anchor(), p.model.size)).collect());
}

fn layout_of(rig: &Rig) -> Vec<(V, [u32; 3])> {
    let layout = LAYOUT.lock().unwrap_or_else(|e| e.into_inner());
    kind(rig.look).and_then(|k| layout.get(k).cloned().flatten()).unwrap_or_else(|| rig.pieces.iter().map(|p| (p.anchor(), p.model.size)).collect())
}

// Files

/// What piece `p` of `look` is called, for its file, `<look>/<name>.vox`
pub fn piece_name(look: Look, p: usize) -> Option<String> {
    rig(look)?.bones.get(p).map(|b| b.name.clone())
}

/// The look `look`'s pieces are drawn from in its own colours, if they're another's in other
/// materials, as the ginger cat's are the grey cat's: the first such in KINDS, with the materials
/// swapped
pub fn skin_of(look: Look) -> Option<(Look, [u8; 256])> {
    let mine = rig(look)?;
    fn models(r: &Rig) -> Vec<&Model> {
        r.pieces.iter().map(|p| &p.model).collect()
    }
    for &other in KINDS.iter().take_while(|&&l| l != look) {
        let theirs = rig(other)?;
        if theirs.bones.len() != mine.bones.len() || theirs.bones.iter().zip(&mine.bones).any(|(a, b)| a.name != b.name) {
            continue;
        }
        if let Some(map) = crate::assets::skin_map(&models(theirs), &models(mine))
            && skin_of(other).is_none()
        {
            return Some((other, map));
        }
    }
    None
}

/// Whether there's an animation file for any of `look`'s clips
pub fn has_anims(look: Look) -> bool {
    anim::active() && CLIPS.iter().any(|c| anim::load(&id_of(look), c).is_some())
}

/// Whether the animation file `file`, its path from assets\anims without .json, is one of
/// `look`'s clips: its own, or its build's for every look of that build without one of its own
pub fn clip_file_of(look: Look, file: &str) -> bool {
    let id = id_of(look);
    let build = id.split('/').next().unwrap_or_default();
    file.strip_prefix(&format!("{id}/")).is_some_and(|c| !c.contains('/')) || file.rsplit_once('/').is_some_and(|(d, _)| d == build)
}

/// The files of `look`'s that are among `files`, the files edited: its pieces' and its old
/// standing frame's
pub fn files_of(look: Look, files: &std::collections::HashSet<String>) -> Vec<String> {
    let Some(rig) = rig(look) else { return Vec::new() };
    let folder = crate::assets::look_folder(look);
    rig.bones.iter().map(|b| b.name.as_str()).chain(["base"]).map(|n| format!("{folder}/{n}")).filter(|f| files.contains(f)).collect()
}

/// `look`'s pieces with the files edited among `files` in `dir` read in, from `folder`: each piece's
/// own file used whole, and what was changed in an old standing frame, `<folder>/base.vox`, put on
/// the pieces holding those places, see legacy()
pub fn read_pieces(look: Look, dir: &Path, files: &std::collections::HashSet<String>, folder: &str) -> Vec<Piece> {
    let Some(rig) = rig(look) else { return Vec::new() };
    let mut pieces = rig.pieces.clone();
    for (p, bone) in rig.bones.iter().enumerate() {
        let name = format!("{folder}/{}", bone.name);
        if !files.contains(&name) || pieces[p].is_empty() {
            continue;
        }
        let piece = &pieces[p];
        match crate::assets::read_edit(&crate::assets::file_of(dir, &name), crate::assets::pivot_of(piece.model.size, Some(piece.anchor()), MODEL_RES)) {
            Ok(edit) => {
                let (m, lo) = crate::assets::conform(&edit, piece.model.size, crate::assets::Grow::Free);
                pieces[p] = piece.with(m, lo);
            }
            Err(e) => crate::assets::log(format!("{name}.vox: {e}")),
        }
    }
    let base = format!("{folder}/base");
    if files.contains(&base)
        && let Err(e) = legacy(rig, &mut pieces, dir, &base)
    {
        crate::assets::log(format!("{base}.vox: {e}"));
    }
    pieces
}

/// The model that an old standing frame's file, written before its look's frames were baked from
/// pieces, was written from: `look`'s standing frame, or the frame the old export wrote there,
/// counting every look's frames as taking up the jointed bosses' pieces too, if the file's .json
/// says that's what was written
/// How many models apart each look's files were put by the export that counted the jointed bosses'
/// pieces in with the frames, see assets::frames(): every frame of every look and the jointed
/// bosses' pieces, over how many looks there were, as it was then. Kept as it was, as more looks and
/// more jointed bosses since would put an old file on yet another frame.
const LEGACY_STRIDE: usize = 56;

fn written_from(look: Look, sidecar: Option<&crate::assets::Sidecar>) -> Option<Model> {
    let mut candidates = vec![animals::drawn_frame(look, 0)];
    let all = Look::ALL.len();
    let stride = LEGACY_STRIDE;
    if let Some(i) = Look::ALL.iter().position(|&l| l == look).map(|i| i * stride).filter(|&i| i < all * animals::FRAMES) {
        candidates.push(animals::drawn_frame(Look::ALL[i / animals::FRAMES], i % animals::FRAMES));
    }
    let written = sidecar.and_then(|s| s.written.clone());
    let first = candidates[0].clone();
    Some(candidates.into_iter().find(|m| written.as_ref().is_some_and(|w| crate::assets::written_as(m, MODEL_RES) == *w)).unwrap_or(first))
}

/// What was changed in `name`.vox, `look`'s standing frame as it was edited before its frames were
/// baked from pieces, put on the pieces holding those places in the standing frame: a voxel taken
/// away or recoloured on the piece that has it there, one added on the piece nearest it. So an old
/// edit goes with the limb it was made to, in every frame.
pub fn legacy(rig: &Rig, pieces: &mut [Piece], dir: &Path, name: &str) -> Result<usize, String> {
    let path = crate::assets::file_of(dir, name);
    let sidecar: Option<crate::assets::Sidecar> = std::fs::read_to_string(path.with_extension("json")).ok().and_then(|t| serde_json::from_str(&t).ok());
    let base = written_from(rig.look, sidecar.as_ref()).ok_or("nothing to compare it with")?;
    let res = MODEL_RES;
    let edit = crate::assets::read_edit(&path, crate::assets::pivot_of(base.size, None, res))?;
    let (edited, lo) = crate::assets::conform(&edit, base.size, crate::assets::Grow::Fixed { cap: 255 });
    let changes = crate::assets::changes(&base, &edited, lo);
    if changes.is_empty() {
        return Ok(0);
    }
    let (standing, owner) = rig.standing(pieces);
    let size = standing.size;
    let at = |q: [i64; 3]| -> Option<usize> { (q.iter().zip(size).all(|(&c, s)| c >= 0 && c < s as i64)).then(|| (q[0] + (q[1] + q[2] * size[1] as i64) * size[0] as i64) as usize) };
    // The piece whose voxel nearest `q` is
    let nearest = |q: [i64; 3]| -> Option<usize> {
        for r in 0..6i64 {
            let mut best: Option<(i64, usize)> = None;
            for dz in -r..=r {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != r {
                            continue;
                        }
                        if let Some(i) = at([q[0] + dx, q[1] + dy, q[2] + dz]).filter(|&i| owner[i] != usize::MAX) {
                            let d = dx * dx + dy * dy + dz * dz;
                            if best.is_none_or(|(b, _)| d < b) {
                                best = Some((d, owner[i]));
                            }
                        }
                    }
                }
            }
            if let Some((_, who)) = best {
                return Some(who);
            }
        }
        None
    };
    let mut edits: Vec<Vec<([i32; 3], u8)>> = vec![Vec::new(); pieces.len()];
    for (p, now, was) in changes {
        // Where it is in the standing frame, by where it is from the middle of the feet
        let q = [0, 1, 2].map(|a| {
            let m = if a < 2 { p[a] as f64 + 0.5 - base.size[a] as f64 * 0.5 } else { p[a] as f64 + 0.5 };
            (m + if a < 2 { size[a] as f64 * 0.5 } else { 0.0 }).floor() as i64
        });
        let here = at(q).map(|i| owner[i]).filter(|&o| o != usize::MAX);
        let who = match (here, was != AIR, now != AIR) {
            (Some(o), true, _) => Some(o),
            (_, _, true) => here.or_else(|| nearest(q)),
            _ => None,
        };
        let Some(who) = who.filter(|&w| rig.poses[0].as_ref().is_some_and(|(_, poses)| poses[w].shown)) else { continue };
        // In the piece's own voxels: the standing frame may have grown either side to hold
        // pieces read from files
        let piece = &pieces[who];
        let grown = [0, 1].map(|a| (size[a] as i64 - piece.size[a] as i64) / 2);
        edits[who].push(([(q[0] - grown[0]) as i32 - piece.lo[0], (q[1] - grown[1]) as i32 - piece.lo[1], q[2] as i32 - piece.lo[2]], now));
    }
    let mut changed = 0;
    for (who, list) in edits.into_iter().enumerate() {
        if list.is_empty() {
            continue;
        }
        changed += list.len();
        let (m, lo) = crate::assets::place(&pieces[who].model, &list, crate::assets::Grow::Free);
        pieces[who] = pieces[who].with(m, lo);
    }
    Ok(changed)
}

// The editor

/// A look baked from pieces as the model and animation editor sees it, see src\anim.rs: its bones,
/// its clips played as the game plays its frames, and its pieces drawn where they go
pub struct EditorRig(pub Look);

impl EditorRig {
    fn rig(&self) -> &'static Rig {
        rig(self.0).expect("a look baked from pieces")
    }

    /// The frames clip `clip` goes through, each at its moment: for the editor's timeline
    pub fn frames(&self, clip: usize) -> Vec<(f64, usize)> {
        let look = self.0;
        match clip {
            0 => vec![(0.0, 0)],
            1 => (0..WALK).map(|k| (k as f64 / WALK as f64 * walk_seconds(look), k + 1)).collect(),
            c => animals::path_to(look, [animals::POSE_WINDUP, animals::POSE_STRIKE, animals::POSE_SPECIAL][(c - 2).min(2)]),
        }
    }
}

impl anim::Rig for EditorRig {
    fn id(&self) -> String {
        id_of(self.0)
    }

    fn bones(&self) -> Vec<anim::BoneDef> {
        self.rig().bone_defs()
    }

    fn clips(&self) -> Vec<anim::ClipDef> {
        let look = self.0;
        (0..clips(look))
            .map(|c| {
                let (length, looping) = match c {
                    0 => (1.0, true),
                    1 => (walk_seconds(look), true),
                    _ => (1.0, false),
                };
                anim::ClipDef { name: CLIPS[c].to_string(), length, looping }
            })
            .collect()
    }

    /// Where the code puts each bone `t` seconds into clip `clip`: partway between the frames on
    /// either side of that moment, as the frames are baked from
    fn pose(&self, clip: usize, t: f64) -> Vec<anim::Joint> {
        let rig = self.rig();
        let frames = self.frames(clip);
        let length = if clip == 1 { walk_seconds(self.0) } else { 1.0 };
        let t = if clip == 1 { t.rem_euclid(length) } else { t.clamp(0.0, length) };
        let k = frames.iter().rposition(|(at, _)| *at <= t + 1e-9).unwrap_or(0);
        let (t0, f0) = frames[k];
        let (t1, f1) = if k + 1 < frames.len() { frames[k + 1] } else if clip == 1 { (length, frames[0].1) } else { frames[k] };
        let along = if t1 > t0 { ((t - t0) / (t1 - t0)).clamp(0.0, 1.0) } else { 0.0 };
        let (Some(a), Some(b)) = (rig.code_pose(f0), rig.code_pose(f1)) else { return Vec::new() };
        a.iter().zip(b).map(|(a, b)| a.mix(b, along)).map(|p| anim::Joint { at: p.at, turn: p.turn, shown: p.shown }).collect()
    }

    fn draw(&self, joints: &[anim::Joint], at: V, yaw: f64, vpm: f64, lit: Option<usize>, out: &mut Vec<AnimalInstance>) {
        let rig = self.rig();
        let Some(k) = kind(self.0) else { return };
        let layout = layout_of(rig);
        let turn = Q::from_axis_angle(&V::z_axis(), yaw);
        for (p, j) in joints.iter().enumerate().take(PIECES) {
            let Some(&(anchor, size)) = layout.get(p) else { continue };
            if !j.shown || size == [1, 1, 1] {
                continue;
            }
            let [x, y, z] = size.map(|v| v as f64 / MODEL_RES);
            let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
            let piece = turn * j.turn;
            let place = at + turn * j.at * vpm + piece * anchor * vpm;
            let mut instance = AnimalInstance::new((animals::BAKED_MODELS_AT + k * PIECES + p) as u32, place, piece, vpm / MODEL_RES, reach, MOVING);
            if lit == Some(p) {
                instance = instance.skinned(0xA0_30_D0_FF);
            }
            out.push(instance);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(a: &Model, b: &Model) -> bool {
        a.size == b.size && a.voxels == b.voxels
    }

    /// How much of what's solid in `a` and `b` is solid in both, of what's solid in either, with
    /// their middles over the same spot
    fn overlap(a: &Model, b: &Model) -> f64 {
        let solid = |m: &Model| -> std::collections::HashSet<[i64; 3]> {
            let mut out = std::collections::HashSet::new();
            let [sx, sy, sz] = m.size;
            for z in 0..sz {
                for y in 0..sy {
                    for x in 0..sx {
                        if m.voxels[(x + (y + z * sy) * sx) as usize] != AIR {
                            out.insert([2 * x as i64 - sx as i64, 2 * y as i64 - sy as i64, z as i64]);
                        }
                    }
                }
            }
            out
        };
        let (a, b) = (solid(a), solid(b));
        // Within a voxel is near enough: frames a voxel apart in size are half a voxel apart in where
        // their voxels are, and a limb drawn whole was a voxel thicker or thinner from frame to frame
        // as it fell between them
        let near = |s: &std::collections::HashSet<[i64; 3]>, p: &[i64; 3]| (-2..=2).any(|dx| (-2..=2).any(|dy| (-1..=1).any(|dz| s.contains(&[p[0] + dx, p[1] + dy, p[2] + dz]))));
        let both = a.iter().filter(|p| near(&b, p)).count() + b.iter().filter(|p| near(&a, p)).count();
        both as f64 / (a.len() + b.len()).max(1) as f64
    }

    /// Every look's pieces put back where they stand make its standing frame exactly as it was
    /// drawn whole, and every other frame comes out much as it was drawn: the same shape, its limbs
    /// where they were, near enough
    #[test]
    fn baked_frames_are_the_frames_drawn() {
        let mut unlike = Vec::new();
        for &look in &KINDS {
            let rig = rig(look).unwrap();
            assert!(same(&rig.frames[0], &animals::drawn_frame(look, 0)), "{look:?} standing");
            assert!(rig.pieces.iter().filter(|p| !p.is_empty()).count() >= 4, "{look:?} has only a few pieces");
            let mut worst = (1.0f64, 0);
            for f in 1..animals::FRAMES {
                let drawn = animals::drawn_frame(look, f);
                assert_eq!(rig.frames[f].size == [1, 1, 1], drawn.size == [1, 1, 1], "{look:?} frame {f}");
                if drawn.size == [1, 1, 1] {
                    continue;
                }
                let o = overlap(&rig.frames[f], &drawn);
                if o < worst.0 {
                    worst = (o, f);
                }
            }
            println!("{look:?}: {} bones, worst frame {} at {:.2}", rig.bones.len(), worst.1, worst.0);
            if worst.0 < 0.85 {
                unlike.push(format!("{look:?} frame {} is only {:.2} like it was drawn", worst.1, worst.0));
            }
        }
        assert!(unlike.is_empty(), "{unlike:?}");
    }

    /// The colours of every material, read out of the shader, see PALETTE in shaders\traverse.comp
    fn palette() -> Vec<[f64; 3]> {
        crate::assets::materials().iter().map(|m| m.rgb.map(|c| (c as f64 / 255.0).powf(2.2))).collect()
    }

    /// Frames drawn on the processor into `image` at `ox`, `oy`, `size` pixels square, seen from in
    /// front, round to its left and a little above, as the jointed bosses' sheets are, see
    /// rig::preview
    fn draw(model: &Model, reach: f64, size: u32, ox: u32, oy: u32, palette: &[[f64; 3]], image: &mut image::RgbImage) {
        let (yaw, pitch) = (1.0f64, 0.22f64);
        let forward = V::new(-yaw.cos() * pitch.cos(), -yaw.sin() * pitch.cos(), -pitch.sin());
        let right = V::new(-yaw.sin(), yaw.cos(), 0.0);
        let up = right.cross(&forward);
        let scale = size as f64 / (2.2 * reach);
        let middle = V::new(0.0, 0.0, reach * 0.55);
        let light = V::new(0.3, -0.5, 0.8).normalize();
        let n = (size * size) as usize;
        let mut depth = vec![f64::MAX; n];
        let mut color = vec![[0.3f64, 0.34, 0.4]; n];
        for (i, c) in color.iter_mut().enumerate() {
            let f = (i as u32 / size) as f64 / size as f64;
            *c = [0.32 - 0.12 * f, 0.36 - 0.12 * f, 0.42 - 0.1 * f];
        }
        let [sx, sy, sz] = model.size;
        let at = |x: i64, y: i64, z: i64| if x < 0 || y < 0 || z < 0 || x >= sx as i64 || y >= sy as i64 || z >= sz as i64 { AIR } else { model.voxels[(x + (y + z * sy as i64) * sx as i64) as usize] };
        let splat = (scale / MODEL_RES * 1.25).ceil().max(1.0) as i64;
        for z in 0..sz as i64 {
            for y in 0..sy as i64 {
                for x in 0..sx as i64 {
                    let m = at(x, y, z);
                    if m == AIR {
                        continue;
                    }
                    let mut normal = V::zeros();
                    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        if at(x + dx, y + dy, z + dz) == AIR {
                            normal += V::new(dx as f64, dy as f64, dz as f64);
                        }
                    }
                    if normal == V::zeros() {
                        continue;
                    }
                    let w = V::from(animals::frame_voxel(model.size, MODEL_RES, x as u32, y as u32, z as u32)) - middle;
                    let (px, py, d) = (size as f64 * 0.5 + w.dot(&right) * scale, size as f64 * 0.55 - w.dot(&up) * scale, w.dot(&forward));
                    let base = palette.get(m as usize).copied().unwrap_or([1.0, 0.0, 1.0]);
                    let shade = 0.42 + 0.75 * normal.normalize().dot(&light).max(0.0);
                    for oy2 in 0..splat {
                        for ox2 in 0..splat {
                            let (qx, qy) = (px as i64 + ox2 - splat / 2, py as i64 + oy2 - splat / 2);
                            if qx >= 0 && qy >= 0 && qx < size as i64 && qy < size as i64 {
                                let i = (qx + qy * size as i64) as usize;
                                if d < depth[i] {
                                    depth[i] = d;
                                    color[i] = base.map(|v| v * shade);
                                }
                            }
                        }
                    }
                }
            }
        }
        for (i, c) in color.iter().enumerate() {
            let to = |v: f64| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            image.put_pixel(ox + i as u32 % size, oy + i as u32 / size, image::Rgb([to(c[0]), to(c[1]), to(c[2])]));
        }
    }

    /// Contact sheets of every look baked from pieces, its frames as they were drawn whole above
    /// and as they're baked from its pieces below, walking and in each pose, to judge them by:
    /// `BAKED_SHEETS=<folder> cargo test --release baked_sheets -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn baked_sheets() {
        let Ok(folder) = std::env::var("BAKED_SHEETS") else {
            println!("set BAKED_SHEETS to a folder to draw them into");
            return;
        };
        let palette = palette();
        // `BAKED_ONLY=<look>` draws only that one, `BAKED_SIZE=<pixels>` each frame that big
        let only = std::env::var("BAKED_ONLY").ok();
        let size: u32 = std::env::var("BAKED_SIZE").ok().and_then(|v| v.parse().ok()).unwrap_or(200);
        for &look in &KINDS {
            if only.as_ref().is_some_and(|o| !format!("{look:?}").eq_ignore_ascii_case(o)) {
                continue;
            }
            let rig = rig(look).unwrap();
            let mut frames: Vec<usize> = (0..=WALK).step_by(3).collect();
            frames.extend([animals::POSE_WINDUP, animals::POSE_STRIKE, animals::POSE_SPECIAL].into_iter().filter(|&f| animals::shown_in(look, f)));
            let mut image = image::RgbImage::new(size * frames.len() as u32, size * 2);
            for (c, &f) in frames.iter().enumerate() {
                draw(&animals::drawn_frame(look, f), look.reach() * 0.75, size, c as u32 * size, 0, &palette, &mut image);
                draw(&rig.frames[f], look.reach() * 0.75, size, c as u32 * size, size, &palette, &mut image);
            }
            let path = std::path::Path::new(&folder).join(format!("{look:?}.png"));
            image.save(&path).unwrap();
            println!("{}", path.display());
        }
        // The armoured skeleton with the player's old edit of its standing frame put on its
        // pieces, below, as it's drawn without it, above
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("tests").join("armored_skeleton_base.vox");
        let dir = std::env::temp_dir().join(format!("innert-legacy-sheet-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("armored_skeleton")).unwrap();
        std::fs::copy(&source, dir.join("armored_skeleton").join("base.vox")).unwrap();
        std::fs::copy(source.with_extension("json"), dir.join("armored_skeleton").join("base.json")).unwrap();
        let rig = rig(Look::ArmoredSkeleton).unwrap();
        let mut pieces = rig.pieces.clone();
        legacy(rig, &mut pieces, &dir, "armored_skeleton/base").unwrap();
        let edited = rig.bake(&pieces, None);
        let mut frames: Vec<usize> = (0..=WALK).step_by(3).collect();
        frames.extend([animals::POSE_WINDUP, animals::POSE_STRIKE, animals::POSE_SPECIAL]);
        let mut image = image::RgbImage::new(size * frames.len() as u32, size * 2);
        for (c, &f) in frames.iter().enumerate() {
            draw(&rig.frames[f], 1.6, size, c as u32 * size, 0, &palette, &mut image);
            draw(&edited[f], 1.6, size, c as u32 * size, size, &palette, &mut image);
        }
        let path = std::path::Path::new(&folder).join("ArmoredSkeleton_old_edit.png");
        image.save(&path).unwrap();
        println!("{}", path.display());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every bone's tagged on some shape, and every shape of a baked look is part of a bone
    #[test]
    fn every_shape_has_a_bone() {
        for &look in &KINDS {
            let n = skeleton(look).len();
            for f in (0..animals::FRAMES).filter(|&f| animals::shown_in(look, f)) {
                for part in animals::frame_parts(look, f) {
                    let (own, far) = part.bones();
                    assert!(own > 0 && own as usize <= n && far as usize <= n, "{look:?} frame {f} has a shape of bone {own}");
                }
            }
        }
    }

    /// A clip keyed turns its bone in the frames baked from that clip, carrying what hangs from it
    /// along, and leaves the frames of other clips as they were
    #[test]
    fn a_clip_rebakes_its_frames() {
        let rig = rig(Look::Skeleton).unwrap();
        let arm = rig.bones.iter().position(|b| b.name == "arm_right").unwrap();
        let blade = rig.bones.iter().position(|b| b.name == "blade").unwrap();
        let mut walk = anim::Clip { length: walk_seconds(Look::Skeleton), looping: true, ..Default::default() };
        walk.set_key("arm_right", anim::Key { time: 0.0, rotate: [0.0, -80.0, 0.0], offset: [0.0; 3], ease: anim::Ease::Linear });
        let deltas = rig.deltas_of(&|c, name, t| (c == 1).then(|| walk.sample(name, t)).flatten());
        let frames = rig.bake(&rig.pieces, Some(&deltas));
        assert!(same(&frames[0], &rig.frames[0]), "standing's from idle, which isn't keyed");
        assert!(same(&frames[animals::POSE_STRIKE], &rig.frames[animals::POSE_STRIKE]));
        assert!(!same(&frames[5], &rig.frames[5]), "walking's keyed");
        let defs = rig.bone_defs();
        let code = rig.code_pose(5).unwrap();
        let now = moved(&defs, code, &deltas[5]);
        assert!(now[arm].turn.angle_to(&code[arm].turn) > 1.0);
        assert!((now[blade].at - code[blade].at).norm() > 0.1, "the blade goes with the arm");
    }

    /// A clip saved, in a running game, bakes again the looks it's a clip of: its own look's, or
    /// every look of its build's
    #[test]
    fn a_saved_clip_is_known_by_its_looks() {
        assert!(clip_file_of(Look::ArmoredSkeleton, "undead/armored_skeleton/walk"));
        assert!(!clip_file_of(Look::Skeleton, "undead/armored_skeleton/walk"));
        assert!(clip_file_of(Look::Skeleton, "undead/strike") && clip_file_of(Look::Bandit, "undead/strike"));
        assert!(!clip_file_of(Look::Wolf, "undead/strike"));
        assert!(!clip_file_of(Look::Wolf, "boss/direwolf/walk") && !clip_file_of(Look::Wolf, "wolf/wolf/extra/walk"));
        assert!(clip_file_of(Look::GingerCat, "beast/ginger_cat/idle"));
    }

    /// A piece's file edited goes with its limb: a voxel added to the end of a skeleton's blade is
    /// at the end of the blade raised over its head as it winds up, not where the blade was
    #[test]
    fn a_piece_file_goes_with_its_limb() {
        let dir = std::env::temp_dir().join(format!("innert-baked-test-{}", std::process::id()));
        let look = Look::Skeleton;
        let rig = rig(look).unwrap();
        let p = rig.bones.iter().position(|b| b.name == "blade").unwrap();
        let piece = &rig.pieces[p];
        let name = format!("{}/blade", crate::assets::look_folder(look));
        crate::assets::write_model(&dir, &name, &piece.model, MODEL_RES, Some(piece.anchor()), "a test", "").unwrap();
        // Read back as it was, it changes nothing
        let files: std::collections::HashSet<String> = [name.clone()].into_iter().collect();
        let back = read_pieces(look, &dir, &files, &crate::assets::look_folder(look));
        assert!(same(&back[p].model, &piece.model) && (back[p].anchor() - piece.anchor()).norm() < 1e-9);
        // A voxel of flame added under its lowest one, past the tip of the blade hanging down
        let mut grown = piece.model.clone();
        let tip = (0..grown.voxels.len()).find(|&i| grown.voxels[i] != AIR).unwrap();
        let [sx, sy, _] = grown.size;
        let (x, y) = (tip as u32 % sx, tip as u32 / sx % sy);
        let edits = vec![([x as i32, y as i32, -1], crate::worldgen::material::FLAME)];
        let (m, lo) = crate::assets::place(&grown, &edits, crate::assets::Grow::Free);
        grown = m;
        // Saved as MagicaVoxel saves it: the file's grown down in its scene, its .json as it was
        let origin = crate::assets::origin_of(crate::assets::pivot_of(piece.model.size, Some(piece.anchor()), MODEL_RES));
        let grid = crate::vox::Grid { size: grown.size, voxels: grown.voxels.clone(), origin: [0, 1, 2].map(|a| origin[a] + lo[a]) };
        std::fs::write(crate::assets::file_of(&dir, &name), crate::vox::write(&grid, &crate::assets::palette())).unwrap();
        let back = read_pieces(look, &dir, &files, &crate::assets::look_folder(look));
        assert_eq!(back[p].model.size[2], piece.model.size[2] + 1);
        let frames = rig.bake(&back, None);
        let flame = |m: &Model| -> Vec<V> {
            let [sx, sy, sz] = m.size;
            let mut out = Vec::new();
            for z in 0..sz {
                for y in 0..sy {
                    for x in 0..sx {
                        if m.voxels[(x + (y + z * sy) * sx) as usize] == crate::worldgen::material::FLAME {
                            out.push(V::from(animals::frame_voxel(m.size, MODEL_RES, x, y, z)));
                        }
                    }
                }
            }
            out
        };
        let standing = flame(&frames[0]);
        let raised = flame(&frames[animals::POSE_WINDUP]);
        assert_eq!(standing.len(), 1, "standing, the flame's where it was put");
        assert!(!raised.is_empty(), "winding up, it's still there");
        assert!(raised[0].z > standing[0].z + 0.6, "and it's gone up with the blade: {:?} from {:?}", raised[0], standing[0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The armoured skeleton's standing frame as the player edited it, before its frames were baked
    /// from pieces: its blade painted red and thickened. The file was written from the wrong frame,
    /// a bandit's, as the old export counted frames wrongly, and its .json says so; what was changed
    /// in it is put on the blade, and goes up with the blade as it winds up.
    #[test]
    fn an_old_standing_frame_goes_with_its_limbs() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("tests").join("armored_skeleton_base.vox");
        if !source.exists() {
            println!("no {}", source.display());
            return;
        }
        let dir = std::env::temp_dir().join(format!("innert-legacy-test-{}", std::process::id()));
        let folder = dir.join("armored_skeleton");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::copy(&source, folder.join("base.vox")).unwrap();
        std::fs::copy(source.with_extension("json"), folder.join("base.json")).unwrap();
        let look = Look::ArmoredSkeleton;
        let rig = rig(look).unwrap();
        let sidecar: crate::assets::Sidecar = serde_json::from_str(&std::fs::read_to_string(folder.join("base.json")).unwrap()).unwrap();
        let from = written_from(look, Some(&sidecar)).unwrap();
        assert!(!same(&from, &rig.frames[0]), "it was written from another look's frame");
        let mut pieces = rig.pieces.clone();
        let changed = legacy(rig, &mut pieces, &dir, "armored_skeleton/base").unwrap();
        assert_eq!(changed, 9, "six voxels added and three recoloured");
        let blade = rig.bones.iter().position(|b| b.name == "blade").unwrap();
        let red = |m: &Model| m.voxels.iter().filter(|&&v| v == crate::worldgen::material::APPLE).count();
        assert_eq!(red(&pieces[blade].model), 9, "all of it on the blade");
        let frames = rig.bake(&pieces, None);
        let high = |m: &Model| -> f64 {
            let [sx, sy, sz] = m.size;
            let mut top = 0.0f64;
            for z in 0..sz {
                for y in 0..sy {
                    for x in 0..sx {
                        if m.voxels[(x + (y + z * sy) * sx) as usize] == crate::worldgen::material::APPLE {
                            top = top.max(animals::frame_voxel(m.size, MODEL_RES, x, y, z)[2]);
                        }
                    }
                }
            }
            top
        };
        assert!(red(&frames[0]) >= 8, "standing, the red's on the blade: {}", red(&frames[0]));
        assert!(high(&frames[animals::POSE_WINDUP]) > high(&frames[0]) + 0.6, "and it's raised with the blade as it winds up");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
