//! Trees coming down when they're cut, see building.rs: once the last blow lands the tree's taken
//! out of the world and made a model of its own, which topples over about its foot away from the
//! player, slowly at first and quicker as it goes, till it crashes to the ground with a thud, the
//! ground shaking near by and its leaves and splinters bursting off it, and breaks up into the wood
//! it gives. A big tree takes longer to come down and lands heavier; a palm sways back before it
//! goes, bending, and bounces where it lands; a dead tree snaps off above its stump and shatters.
//! What it comes down on is hurt, and a player standing where it'll fall is warned, and thrown
//! clear as it lands.
//!
//! The tree's model is built on a thread of its own from its voxels, at the world's own size of
//! voxel for most trees and coarser for the biggest, so no frame waits on it: the tree stands as it
//! was till its model's ready, a frame or two. It's drawn as the animals are, with the weapons'
//! models, in one of a few slots kept for such one-off models, see Game::set_one_off().

use std::sync::mpsc::{Receiver, channel};

use nalgebra::{UnitQuaternion, Vector2, Vector3};

use crate::animals::{AnimalInstance, MOVING, Model};
use crate::bases::{BLOCK, Materials};
use crate::gather::{Cut, Tree, is_crown, is_trunk};
use crate::worldgen::{World, is_passable, material::*};

/// The most voxels a falling tree's model has, so a big one's made coarser rather than taking long
/// to build, send and trace through: a megabyte and a half
pub const MODEL_MOST: usize = 1_500_000;
/// How many trees can be coming down at once, each its model in a slot of its own
pub const SLOTS: usize = 3;
/// Seconds a tree lies where it fell before it breaks up into its wood, and a dead one
const LIE: f64 = 0.7;
const LIE_DEAD: f64 = 0.12;
/// How quickly it tips at first, in radians a second, as the last blow pushes it over
const PUSH: f64 = 0.22;
/// How far it's turned when it lets go, in radians
const START: f64 = 0.04;
/// It's down for good once it's turned this far, whatever's under it, in radians: a little past
/// lying flat, for a slope falling away
const MOST: f64 = 1.85;
/// How much harder than a uniform rod a tree's pulled over, the weight of its crown being high up
const PULL: f64 = 1.6;
/// How much of what it lands on it hurts, at most, and how far round in meters
const HARM: f64 = 140.0;
const HARM_REACH_M: f64 = 1.6;
/// How far a dead tree snaps off over its foot, in meters, the stump left standing
const SNAP_M: f64 = 0.6;

/// The kinds of tree that come down their own way
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Broad,
    /// Sways back before it goes, bends, and bounces where it lands
    Palm,
    /// Snaps off over its stump, comes down quick and shatters
    Dead,
}

/// A tree about to come down: what's taken out of the world, what it's made of, voxel by voxel in
/// the world as it was, which way it falls, and what it gives. All in voxels of the world it was
/// planned in, `origin` its corner in the whole landscape.
pub struct Plan {
    pub cut: Cut,
    pub voxels: Vec<([i64; 3], u8)>,
    /// What it turns about, the middle of its trunk where it's cut through, and how tall it is
    /// over that
    pub pivot: Vector3<f64>,
    pub height: f64,
    pub dir: Vector2<f64>,
    pub kind: Kind,
    pub gives: Materials,
    pub bark: u8,
    pub leaf: Option<u8>,
    pub origin: [i64; 3],
    /// How many voxels of it there are, for how heavily it lands
    pub size: usize,
}

/// Which way a tree at `foot` falls with the player at `player`: straight away from them, along the
/// way they look if they're right at its foot, or along x if there's no player
pub fn direction(foot: Vector3<f64>, player: Option<Vector3<f64>>, look: Vector3<f64>) -> Vector2<f64> {
    let away = player.map(|p| Vector2::new(foot.x - p.x, foot.y - p.y));
    away.and_then(|a| a.try_normalize(1.0)).or_else(|| Vector2::new(look.x, look.y).try_normalize(1e-6)).unwrap_or(Vector2::x())
}

/// What it takes to bring `tree` down in `world`, the player at `player` looking along `look`
pub fn plan(world: &World, tree: Tree, player: Option<Vector3<f64>>, look: Vector3<f64>) -> Plan {
    let vpm = world.voxels_per_meter as f64;
    let kind = match tree.bark {
        DEAD_WOOD => Kind::Dead,
        PALM_WOOD => Kind::Palm,
        _ => Kind::Broad,
    };
    // A dead tree leaves its stump standing
    let snap = if kind == Kind::Dead { (SNAP_M * vpm).round() } else { 0.0 };
    let above = tree.foot.z + snap;
    let mut cut = Cut::new();
    let mut voxels = Vec::new();
    for (b, list) in tree.cut {
        let mut kept = Vec::with_capacity(list.len());
        for k in list {
            let v = [b[0] * BLOCK + (k as i64 % 8), b[1] * BLOCK + (k as i64 / 8) % 8, b[2] * BLOCK + k as i64 / 64];
            if (v[2] as f64) < above {
                continue;
            }
            let m = world.material_at(v[0], v[1], v[2]);
            if m == AIR {
                continue;
            }
            voxels.push((v, m));
            kept.push(k);
        }
        if !kept.is_empty() {
            cut.insert(b, kept);
        }
    }
    Plan {
        cut,
        size: voxels.len(),
        voxels,
        pivot: Vector3::new(tree.foot.x + 0.5, tree.foot.y + 0.5, above),
        height: (tree.height - snap).max(1.0),
        dir: direction(tree.foot, player, look),
        kind,
        gives: crate::materials::tree_gives(tree.wood, tree.bark, tree.leaf),
        bark: tree.bark,
        leaf: tree.leaf,
        origin: world.origin,
    }
}

/// A tree's model, built from its voxels: the model, how many of the world's voxels across each of
/// its own is, and where its first corner is in the world's voxels
pub struct Built {
    pub model: Model,
    pub scale: f64,
    pub lo: Vector3<f64>,
    /// The model packed by pack_models() alone, see Game::set_one_off(): its voxels are then let go
    pub packed: Vec<u32>,
}

/// Builds a model of `voxels`, of as big voxels as keep it to MODEL_MOST, the trunk and branches
/// winning over the leaves where a voxel of it has both
pub fn build(voxels: &[([i64; 3], u8)]) -> Built {
    let mut lo = [i64::MAX; 3];
    let mut hi = [i64::MIN; 3];
    for (v, _) in voxels {
        for i in 0..3 {
            lo[i] = lo[i].min(v[i]);
            hi[i] = hi[i].max(v[i]);
        }
    }
    if voxels.is_empty() {
        return Built { model: Model { size: [1, 1, 1], voxels: vec![AIR] }, scale: 1.0, lo: Vector3::zeros(), packed: Vec::new() };
    }
    let span = [0, 1, 2].map(|i| (hi[i] - lo[i] + 1) as usize);
    let scale = (1..=8usize).find(|&f| span.iter().map(|&s| s.div_ceil(f)).product::<usize>() <= MODEL_MOST && span.iter().all(|&s| s.div_ceil(f) <= 1023)).unwrap_or(8);
    let size = span.map(|s| s.div_ceil(scale));
    let mut out = vec![AIR; size[0] * size[1] * size[2]];
    for &(v, m) in voxels {
        let c = [0, 1, 2].map(|i| (v[i] - lo[i]) as usize / scale);
        let i = c[0] + (c[1] + c[2] * size[1]) * size[0];
        if out[i] == AIR || (is_trunk(m) && !is_trunk(out[i])) {
            out[i] = m;
        }
    }
    Built { model: Model { size: size.map(|s| s as u32), voxels: out }, scale: scale as f64, lo: Vector3::new(lo[0] as f64, lo[1] as f64, lo[2] as f64), packed: Vec::new() }
}

/// How a tree turns as it comes down, about its pivot: how far over it is, and how quickly it's
/// going, in radians, and once it's down, how long it's lain there
#[derive(Clone, Copy, Debug)]
pub struct Topple {
    pub angle: f64,
    pub speed: f64,
    pub kind: Kind,
    /// How tall it is over its pivot, in meters
    pub length_m: f64,
    pub landed: Option<f64>,
    bounced: bool,
}

impl Topple {
    pub fn new(kind: Kind, length_m: f64) -> Self {
        // A palm sways back the other way first
        let (angle, speed) = if kind == Kind::Palm { (START, -PUSH * 0.9) } else { (START, PUSH) };
        Topple { angle, speed, kind, length_m: length_m.max(1.0), landed: None, bounced: false }
    }

    /// `dt` seconds on, `hits` saying whether it'd be into the ground turned so far over: whether
    /// it hit the ground just now
    pub fn step(&mut self, dt: f64, hits: impl Fn(f64) -> bool) -> bool {
        if let Some(lain) = &mut self.landed {
            *lain += dt;
            return false;
        }
        // A rod tipping over about its foot, pulled over faster the further over it is, the
        // taller the slower; a palm's springier and a dead tree's lighter at the top
        let pull = match self.kind {
            Kind::Broad => PULL,
            Kind::Palm => PULL * 0.75,
            Kind::Dead => PULL * 1.4,
        };
        let substeps = (dt / 0.004).ceil().max(1.0) as usize;
        let h = dt / substeps as f64;
        for _ in 0..substeps {
            let accel = pull * 3.0 * 9.8 / (2.0 * self.length_m) * self.angle.sin().max(0.02);
            // A palm swaying back is pulled forward again, bending
            let spring = if self.kind == Kind::Palm && self.angle < 0.15 { 2.5 * (0.15 - self.angle) } else { 0.0 };
            self.speed += (accel + spring) * h;
            let next = self.angle + self.speed * h;
            if next > 0.2 && (hits(next) || next >= MOST) {
                // A palm bounces once where it lands, the rest come to rest at once
                if self.kind == Kind::Palm && !self.bounced && self.speed > 0.6 {
                    self.bounced = true;
                    self.speed *= -0.22;
                    return true;
                }
                self.speed = 0.0;
                self.landed = Some(0.0);
                return !self.bounced || self.kind != Kind::Palm;
            }
            self.angle = next.max(-0.2);
        }
        false
    }

    /// Whether it's lain long enough to break up into its wood
    pub fn done(&self) -> bool {
        self.landed.is_some_and(|l| l >= if self.kind == Kind::Dead { LIE_DEAD } else { LIE })
    }

    /// How it's turned now, falling along `dir`
    pub fn rotation(&self, dir: Vector2<f64>) -> UnitQuaternion<f64> {
        let axis = Vector3::z().cross(&Vector3::new(dir.x, dir.y, 0.0));
        UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(axis), self.angle)
    }
}

/// A tree coming down
pub struct Fall {
    pub plan: Plan,
    pub built: Built,
    pub topple: Topple,
    pub slot: usize,
    /// Whether the player's been warned, and whether it's hit the ground yet
    pub warned: bool,
    pub struck: bool,
}

impl Fall {
    /// Where its top is, `along` of the way up it from its pivot, turned `angle` over, in voxels of
    /// the whole landscape
    pub fn point(&self, along: f64, angle: f64) -> Vector3<f64> {
        let p = &self.plan;
        let o = Vector3::new(p.origin[0] as f64, p.origin[1] as f64, 0.0);
        let l = p.height * along;
        p.pivot + o + Vector3::new(p.dir.x * angle.sin() * l, p.dir.y * angle.sin() * l, angle.cos() * l)
    }

    /// What's drawn of it, `model` the number of its model among all models, in voxels of the world
    /// whose corner's at `origin` in the whole landscape
    pub fn instance(&self, model: u32, origin: [i64; 3]) -> AnimalInstance {
        let b = &self.built;
        let size = Vector3::new(b.model.size[0] as f64, b.model.size[1] as f64, b.model.size[2] as f64) * b.scale;
        // Its model's turned about the middle of its bottom, so that's put where it is with the tree
        // turned about its pivot
        let shift = Vector3::new((self.plan.origin[0] - origin[0]) as f64, (self.plan.origin[1] - origin[1]) as f64, 0.0);
        let bottom = b.lo + Vector3::new(size.x * 0.5, size.y * 0.5, 0.0);
        let turn = self.topple.rotation(self.plan.dir);
        let at = self.plan.pivot + turn * (bottom - self.plan.pivot) + shift;
        AnimalInstance::new(model, at, turn, b.scale, size.norm() + (bottom - self.plan.pivot).norm(), MOVING)
    }
}

/// The trees coming down, and those whose models are being built
#[derive(Default)]
pub struct Felling {
    pub falls: Vec<Fall>,
    building: Vec<(Plan, Receiver<Built>)>,
    /// The slot the next tree's model goes in
    next_slot: usize,
}

impl Felling {
    /// Starts `plan` off: its model built on a thread of its own, see ready()
    pub fn start(&mut self, mut plan: Plan) {
        let (send, receive) = channel();
        let voxels = std::mem::take(&mut plan.voxels);
        std::thread::spawn(move || {
            let mut built = build(&voxels);
            built.packed = crate::animals::pack_models(&[&built.model]);
            built.model.voxels = Vec::new();
            let _ = send.send(built);
        });
        self.building.push((plan, receive));
    }

    /// Whether a tree's being made ready to come down, while which the next can't be
    pub fn busy(&self) -> bool {
        !self.building.is_empty()
    }

    /// The trees whose models are ready, to come down now, each with the slot its model's to go in
    pub fn ready(&mut self) -> Vec<(Plan, Built, usize)> {
        let mut out = Vec::new();
        let mut k = 0;
        while k < self.building.len() {
            match self.building[k].1.try_recv() {
                Ok(built) => {
                    let (plan, _) = self.building.remove(k);
                    let slot = self.next_slot;
                    self.next_slot = (self.next_slot + 1) % SLOTS;
                    // A tree still coming down in that slot is down at once
                    self.falls.retain(|f| f.slot != slot);
                    out.push((plan, built, slot));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => k += 1,
                Err(_) => {
                    self.building.remove(k);
                }
            }
        }
        out
    }
}

/// Whether the ground, or anything solid but the tree's own stump, is where a tree falling along
/// `fall` would be, turned `angle` over: looked for at a few points up its trunk, in voxels of
/// `world`
pub fn hits_ground(world: &World, fall: &Fall, angle: f64) -> bool {
    let o = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
    [0.35, 0.5, 0.65, 0.8, 0.95].iter().any(|&along| {
        let p = fall.point(along, angle) - o;
        let m = world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64);
        !is_passable(m) && !is_crown(m)
    })
}

/// Whether `at` is where a tree coming down along `fall` will land, within its crown's reach of its
/// trunk, all in voxels of the whole landscape: for warning the player, and hurting what's there
pub fn in_the_way(fall: &Fall, at: Vector3<f64>, vpm: f64) -> bool {
    let foot = fall.point(0.0, 0.0);
    let to = at - foot;
    let along = to.x * fall.plan.dir.x + to.y * fall.plan.dir.y;
    let across = (to.x * fall.plan.dir.y - to.y * fall.plan.dir.x).abs();
    along > 0.5 * vpm && along < fall.plan.height + vpm && across < HARM_REACH_M * vpm && to.z.abs() < 4.0 * vpm
}

/// How much a tree of `voxels` voxels hurts what it lands on, and shakes the ground: more the
/// bigger it is, HARM at most
pub fn harm(voxels: usize) -> f64 {
    HARM * (voxels as f64 / 60_000.0).sqrt().clamp(0.25, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gather::{tests::grove, tree_at};

    // A tree's planned to come down whole, all of its trunk and crown, away from the player, giving
    // the wood it's worth; and taken out of the world, none of it's left
    #[test]
    fn felling_takes_the_tree_and_falls_away_from_the_player() {
        let mut world = grove();
        let none = |_: [i64; 3]| false;
        let tree = tree_at(&world, [40, 63, 30], &none).unwrap();
        let wood = tree.wood;
        let player = Vector3::new(30.0, 61.0, 16.0);
        let plan = plan(&world, tree, Some(player), Vector3::new(1.0, 0.0, 0.0));
        assert!(plan.dir.x > 0.95, "away from the player: {:?}", plan.dir);
        assert_eq!(plan.gives.wood, wood);
        assert_eq!(plan.kind, Kind::Broad);
        for z in 16..90 {
            assert!(plan.voxels.iter().any(|(v, _)| *v == [40, 60, z]), "trunk at {z}");
        }
        // Taken out of the world, as App::cut() does with the edits, nothing of it's left
        for (b, list) in &plan.cut {
            let mut brick = crate::bases::read_brick(&world, *b);
            for &k in list {
                brick[k] = AIR;
            }
            world.set_brick(*b, &brick);
        }
        assert!((16..90).all(|z| world.material_at(40, 60, z) == AIR));
        assert!((70..110).all(|z| world.material_at(40, 50, z) != LEAVES), "its crown with it");
        assert_eq!(world.material_at(66, 60, 40), WOOD, "the tree beside it stays");
        // The other way with the player on the other side
        let tree = tree_at(&grove(), [66, 63, 30], &none).unwrap();
        let other = super::plan(&grove(), tree, Some(Vector3::new(80.0, 60.0, 16.0)), Vector3::x());
        assert!(other.dir.x < -0.95);
        // Its model's of every voxel of it
        let built = build(&plan.voxels);
        assert_eq!(built.scale, 1.0);
        assert_eq!(built.model.voxels.iter().filter(|&&m| m != AIR).count(), plan.voxels.len());
    }

    // A model packed alone and copied in after others is what packing them all at once makes
    #[test]
    fn models_packed_alone_splice_in() {
        use crate::animals::{pack_models, splice_packed};
        let a = Model { size: [2, 1, 1], voxels: vec![WOOD, STONE] };
        let b = Model { size: [1, 2, 2], voxels: vec![AIR, LEAVES, WOOD, AIR] };
        let c = build(&[([0, 0, 0], WOOD), ([3, 1, 2], LEAVES), ([1, 0, 5], WOOD)]).model;
        let d = Model { size: [1, 1, 3], voxels: vec![GLASS, AIR, MARBLE] };
        assert_eq!(splice_packed(&pack_models(&[&a, &b]), &[pack_models(&[&c]), pack_models(&[&d])]), pack_models(&[&a, &b, &c, &d]));
        assert_eq!(splice_packed(&pack_models(&[]), &[pack_models(&[&c])]), pack_models(&[&c]));
    }

    // A big tree's model is made of bigger voxels, kept to MODEL_MOST, its trunk kept over its
    // leaves
    #[test]
    fn big_trees_make_coarser_models() {
        let mut voxels = Vec::new();
        for z in 0..520 {
            for y in 0..80 {
                for x in 0..80 {
                    let trunk = (28..32).contains(&x) && (28..32).contains(&y);
                    if trunk || (z > 200 && (x + y + z) % 5 == 0) {
                        voxels.push(([x, y, z], if trunk { WOOD } else { LEAVES }));
                    }
                }
            }
        }
        let built = build(&voxels);
        assert!(built.scale >= 2.0);
        assert!(built.model.voxels.len() <= MODEL_MOST);
        let s = built.model.size;
        let middle = (30 / built.scale as usize) + (30 / built.scale as usize) * s[0] as usize + (300 / built.scale as usize) * (s[0] * s[1]) as usize;
        assert_eq!(built.model.voxels[middle], WOOD);
    }

    // A tree tips over slow at first and faster as it goes, landing in a couple of seconds, a big
    // one later than a small one; a palm sways back first and bounces; once down it lies a moment
    #[test]
    fn trees_topple_and_land() {
        let flat = |angle: f64| angle >= std::f64::consts::FRAC_PI_2 - 0.05;
        let land = |kind: Kind, length: f64| {
            let mut t = Topple::new(kind, length);
            let mut time = 0.0;
            let mut speeds = Vec::new();
            let mut least: f64 = 1.0;
            while t.landed.is_none() && time < 10.0 {
                t.step(1.0 / 60.0, flat);
                least = least.min(t.angle);
                speeds.push(t.speed);
                time += 1.0 / 60.0;
            }
            (time, speeds, least)
        };
        let (small, speeds, _) = land(Kind::Broad, 6.0);
        let (big, _, _) = land(Kind::Broad, 20.0);
        assert!(small > 0.8 && small < 3.0, "{small}");
        assert!(big > small, "{big} {small}");
        assert!(speeds.windows(2).take(60).all(|w| w[1] >= w[0]), "it speeds up as it goes");
        let (palm, _, least) = land(Kind::Palm, 8.0);
        assert!(least < START, "a palm sways back first");
        assert!(palm > land(Kind::Broad, 8.0).0);
        let (dead, _, _) = land(Kind::Dead, 8.0);
        assert!(dead < land(Kind::Broad, 8.0).0);
        // On a slope falling away, no further than MOST
        let mut t = Topple::new(Kind::Broad, 8.0);
        for _ in 0..1000 {
            t.step(1.0 / 60.0, |_| false);
        }
        assert!(t.landed.is_some() && t.angle <= MOST);
        let mut lying = t;
        lying.step(LIE + 0.1, |_| false);
        assert!(lying.done());
    }

    // It falls away from the player, along the way they look if they're at its foot; what's where
    // it lands is in the way, what's behind it isn't
    #[test]
    fn what_is_in_the_way() {
        assert!(direction(Vector3::new(10.0, 0.0, 0.0), Some(Vector3::new(0.0, 0.0, 0.0)), Vector3::y()).x > 0.99);
        assert!(direction(Vector3::new(10.0, 0.0, 0.0), Some(Vector3::new(10.2, 0.1, 0.0)), Vector3::y()).y > 0.99);
        let mut world = grove();
        let tree = tree_at(&world, [40, 63, 30], &|_| false).unwrap();
        let plan = plan(&world, tree, Some(Vector3::new(30.0, 61.0, 16.0)), Vector3::x());
        let built = build(&plan.voxels);
        for (b, list) in &plan.cut {
            let mut brick = crate::bases::read_brick(&world, *b);
            for &k in list {
                brick[k] = AIR;
            }
            world.set_brick(*b, &brick);
        }
        let fall = Fall { plan, built, topple: Topple::new(Kind::Broad, 4.0), slot: 0, warned: false, struck: false };
        assert!(in_the_way(&fall, Vector3::new(80.0, 61.0, 17.0), 16.0));
        assert!(!in_the_way(&fall, Vector3::new(20.0, 61.0, 17.0), 16.0));
        assert!(!in_the_way(&fall, Vector3::new(80.0, 120.0, 17.0), 16.0));
        // Lying flat, it's into the ground of the grove; standing, it isn't
        assert!(hits_ground(&world, &fall, std::f64::consts::FRAC_PI_2 + 0.05) || hits_ground(&world, &fall, 1.7));
        assert!(!hits_ground(&world, &fall, 0.1));
        assert!(harm(200_000) <= HARM && harm(1000) < harm(100_000));
    }
}
