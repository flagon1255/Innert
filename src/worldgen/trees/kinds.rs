//! The kinds of tree that came after the first: gnarled oaks, tall grey beeches and maples in
//! autumn reds in the broad-leaved woods, larches going gold and dead snags among the pines and in
//! the snow, baobabs at the edge of the desert and Joshua trees in it, kapok giants on their
//! buttresses and banyans on their pillars of roots in the jungle, mangroves on their stilts along
//! warm shores, bald cypresses with knees standing round them by the rivers, and the realms' own:
//! dreamblooms hung with blossom in the dreaming realm, bloodthorns in its bleeding half and rime
//! trees in the frozen one. Each grows from rolls of its own, see Terrain::species(), so where
//! none of them is picked a tree grows just as it always did.

use super::*;

/// The newer kinds of tree, see the top of this file
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Species {
    Oak,
    Beech,
    Maple,
    Larch,
    Snag,
    Baobab,
    Joshua,
    Kapok,
    Banyan,
    Mangrove,
    Cypress,
    Dreambloom,
    Bloodthorn,
    Rime,
}

impl Species {
    pub(crate) const ALL: [Species; 14] = [
        Species::Oak,
        Species::Beech,
        Species::Maple,
        Species::Larch,
        Species::Snag,
        Species::Baobab,
        Species::Joshua,
        Species::Kapok,
        Species::Banyan,
        Species::Mangrove,
        Species::Cypress,
        Species::Dreambloom,
        Species::Bloodthorn,
        Species::Rime,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Species::Oak => "gnarled oak",
            Species::Beech => "beech",
            Species::Maple => "maple",
            Species::Larch => "larch",
            Species::Snag => "dead snag",
            Species::Baobab => "baobab",
            Species::Joshua => "Joshua tree",
            Species::Kapok => "kapok",
            Species::Banyan => "banyan",
            Species::Mangrove => "mangrove",
            Species::Cypress => "bald cypress",
            Species::Dreambloom => "dreambloom",
            Species::Bloodthorn => "bloodthorn",
            Species::Rime => "rime tree",
        }
    }
}

/// How old a tree of the newer kinds is, as a size next to most of its kind: now and then a
/// sapling, some young, most grown, and the odd old giant, rolled on its own from `r`
pub(super) fn age(r: u32) -> f32 {
    let (a, b) = (unit(hash(r ^ 0xa6e1)), unit(hash(r ^ 0xa6e2)));
    if a < 0.08 {
        0.42 + 0.14 * b
    } else if a < 0.28 {
        0.66 + 0.2 * b
    } else if a < 0.9 {
        0.9 + 0.2 * b
    } else {
        1.12 + 0.16 * b
    }
}

/// A slab of wood, a flat ellipsoid `half` meters along `along`, across and up from `center`: a
/// buttress root, whose lower half is in the ground
fn slab(center: Vec3, along: Vec3, half: Vec3, bark: (u8, u8)) -> Shape {
    let along = normalize([along[0], along[1], 0.0]);
    let side = [-along[1], along[0], 0.0];
    let reach = half[0].max(half[1]).max(half[2]);
    Shape {
        a: center,
        b: center,
        radius_a: reach,
        radius_b: reach,
        flat: Some(([along, side, [0.0, 0.0, 1.0]], half)),
        boxed: None,
        material: bark.0,
        accent: bark.1,
        ragged: false,
        placement: Placement::Above,
        pattern: Pattern::Plain,
        rough: 0.0,
        floor: f32::MIN,
        min: center.map(|v| v - reach),
        max: center.map(|v| v + reach),
    }
}

impl Terrain {
    /// Which of the newer kinds of tree grows at `x`, `y`, `base` meters high, where the climate is
    /// `c`, if one does, `wild` out in the wild rather than in a garden: from a roll of its own on
    /// `r`, so where it's none of them the tree's what it always was. Following grow_tree()'s lands
    /// in its order: the realms' own first, the jungle's giants, nothing new in the desert itself
    /// (its Joshua trees stand among the cacti, see joshua()), the snow's, snags up high, mangroves
    /// on warm shores and cypresses on mild ones, larches among the pines, baobabs at the desert's
    /// edge, and oaks, beeches and maples in the broad-leaved woods, beeches in groves of their own
    /// and maples most where the leaves have turned.
    pub(in crate::worldgen) fn species(&self, x: f32, y: f32, base: f32, r: u32, wild: bool, c: &Climate) -> Option<Species> {
        let mut roll = unit(hash(r ^ 0x5bec));
        let seed = self.settings.seed;
        let land = land_of(seed);
        let pick = |roll: f32, shares: &[(Species, f32)]| {
            let mut left = roll;
            shares.iter().find(|(_, share)| {
                left -= share;
                left < 0.0
            })
            .map(|s| s.0)
        };
        if land == Land::Dream {
            let blood = dream_blood(x, y);
            return if blood > 0.6 {
                pick(roll, &[(Species::Bloodthorn, 0.55)])
            } else if blood < 0.3 {
                pick(roll, &[(Species::Dreambloom, 0.32)])
            } else {
                None
            };
        }
        if c.jungle > 0.5 {
            return if wild { pick(roll, &[(Species::Kapok, 0.13), (Species::Banyan, 0.12)]) } else { None };
        }
        if c.desert > 0.5 {
            return None;
        }
        if c.snow > 0.5 {
            let shares = if land == Land::Ice {
                [(Species::Rime, 0.24), (Species::Larch, 0.1), (Species::Snag, if wild { 0.06 } else { 0.0 })]
            } else {
                [(Species::Larch, 0.16), (Species::Snag, if wild { 0.07 } else { 0.0 }), (Species::Rime, 0.03)]
            };
            return pick(roll, &shares);
        }
        let sea_level = sea_level_m(&self.settings);
        let altitude = (base - sea_level) / (self.settings.height_m as f32 - sea_level);
        let turned = smoothstep(-0.4, 0.4, autumn(&self.settings, x, y));
        if wild && altitude > 0.5 && roll < 0.05 {
            return Some(Species::Snag);
        }
        if base - sea_level < 2.5 {
            let warm = c.desert + c.jungle > 0.25 || land == Land::Water;
            return if warm {
                pick(roll, &[(Species::Mangrove, 0.42)])
            } else if wild {
                pick(roll, &[(Species::Cypress, 0.4)])
            } else {
                None
            };
        }
        let pines = self.pines(x, y, base);
        let larch = pines * (0.14 + 0.22 * turned);
        if roll < larch {
            return Some(Species::Larch);
        }
        roll -= larch;
        let dry = smoothstep(0.15, 0.45, c.desert);
        let baobab = if wild { 0.16 * dry } else { 0.0 };
        if roll < baobab {
            return Some(Species::Baobab);
        }
        roll -= baobab;
        let leafy = (1.0 - pines) * (1.0 - dry * 0.85);
        let beeches = smoothstep(0.1, 0.5, fbm(x / 60.0, y / 60.0, 2, seed.wrapping_add(13)));
        pick(roll, &[(Species::Oak, 0.1 * leafy), (Species::Beech, (0.04 + 0.24 * beeches) * leafy), (Species::Maple, (0.05 + 0.17 * turned) * leafy)])
    }

    /// A tree of `species` at `x`, `y` on ground `base` meters high, grown from `r`, `wild` out in
    /// the wild rather than in a garden, where it's no bigger than most
    pub(in crate::worldgen) fn grow_species(&self, species: Species, x: f32, y: f32, base: f32, r: u32, wild: bool) -> Object {
        let mut rng = Rng(hash(r ^ 0x9a0e));
        let size = if wild { age(r) } else { age(r).min(1.0) };
        let lean = self.downhill(x, y);
        let snowy = climate(&self.settings, x, y).snow > 0.5;
        let shapes = match species {
            Species::Oak => self.oak(x, y, base, size, lean, &mut rng),
            Species::Beech => self.beech(x, y, base, size, lean, &mut rng),
            Species::Maple => self.maple(x, y, base, size, lean, &mut rng),
            Species::Larch => self.larch(x, y, base, size, snowy, &mut rng),
            Species::Snag => self.snag(x, y, base, size, snowy, &mut rng),
            Species::Baobab => self.baobab(x, y, base, size, &mut rng),
            Species::Joshua => self.joshua(x, y, base, &mut rng),
            Species::Kapok => self.kapok(x, y, base, size, &mut rng),
            Species::Banyan => self.banyan(x, y, base, size, &mut rng),
            Species::Mangrove => self.mangrove(x, y, base, size, &mut rng),
            Species::Cypress => self.cypress(x, y, base, size, &mut rng),
            Species::Dreambloom => self.dreambloom(x, y, base, size, &mut rng),
            Species::Bloodthorn => self.bloodthorn(x, y, base, size, lean, &mut rng),
            Species::Rime => self.rime(x, y, base, size, lean, &mut rng),
        };
        Object::new(shapes, rng.0).of(species, [x, y])
    }

    /// A gnarled old oak: a short, massive trunk, its bark flecked dark, its limbs going out nearly
    /// level, kinked once on the way, then up into masses of leaves apart from each other, with sky
    /// between
    #[allow(clippy::too_many_arguments)]
    fn oak(&self, x: f32, y: f32, base: f32, size: f32, lean: [f32; 2], rng: &mut Rng) -> Vec<Shape> {
        let s = size.min(1.15);
        let height = rng.range(1.8, 2.5) * s.max(0.6);
        let radius = rng.range(0.36, 0.48) * s.max(0.5);
        let bark = (WOOD, if rng.next() < 0.5 { BARK_DARK } else { DEAD_WOOD });
        let foot = [x, y, base];
        let top = [x + lean[0] * height + rng.range(-0.3, 0.3) * s, y + lean[1] * height + rng.range(-0.3, 0.3) * s, base + height];
        let canopy = self.canopy(x, y, rng);
        let mut t = Parts::default();
        self.trunk(&mut t.wood, foot, top, radius, 0.75, bark, if s > 0.6 { 5 } else { 0 }, rng);
        let limbs = 4 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.3, 0.3)) / limbs as f32 * TAU;
            let out = [angle.cos(), angle.sin()];
            let start = lerp3(foot, top, rng.range(0.75, 1.0));
            let reach = rng.range(2.3, 3.2) * s;
            let elbow = add(start, [out[0] * reach * 0.5 + rng.range(-0.4, 0.4) * s, out[1] * reach * 0.5 + rng.range(-0.4, 0.4) * s, rng.range(-0.1, 0.5) * s]);
            let end = add(start, [out[0] * reach, out[1] * reach, rng.range(0.6, 1.3) * s]);
            let r0 = radius * rng.range(0.5, 0.65);
            t.wood.push(Shape::capsule(start, elbow, r0, r0 * 0.75, bark.0, bark.1, false));
            t.wood.push(Shape::capsule(elbow, end, r0 * 0.75, r0 * 0.4, bark.0, bark.1, false));
            let r = rng.range(1.25, 1.6) * s.max(0.55);
            self.lobe(&mut t, add(end, [0.0, 0.0, r * 0.2]), r, 0.8, canopy, bark.0, rng);
            if rng.next() < 0.6 {
                self.lobe(&mut t, add(elbow, [0.0, 0.0, r * 0.55]), r * rng.range(0.6, 0.75), 0.8, canopy, bark.0, rng);
            }
        }
        let r = rng.range(1.3, 1.6) * s.max(0.55);
        let crown = add(top, [0.0, 0.0, r * 0.9 + 0.4 * s]);
        t.wood.push(Shape::wood(top, crown, radius * 0.5, radius * 0.3, bark.0));
        self.lobe(&mut t, crown, r, 0.8, canopy, bark.0, rng);
        t.shapes()
    }

    /// A beech: a tall, smooth grey trunk standing clear, limbs sweeping steeply up from high on
    /// it into a tall crown, fresh green, or copper where the leaves have turned
    #[allow(clippy::too_many_arguments)]
    fn beech(&self, x: f32, y: f32, base: f32, size: f32, lean: [f32; 2], rng: &mut Rng) -> Vec<Shape> {
        let s = size;
        let height = rng.range(4.2, 5.6) * s.max(0.5);
        let radius = rng.range(0.26, 0.36) * s.max(0.5);
        let bark = (DEAD_WOOD, BIRCH_BARK);
        let foot = [x, y, base];
        let top = [x + lean[0] * height * 0.5 + rng.range(-0.2, 0.2), y + lean[1] * height * 0.5 + rng.range(-0.2, 0.2), base + height];
        let turned = smoothstep(-0.4, 0.4, autumn(&self.settings, x, y));
        let canopy = if rng.next() < turned * 0.9 {
            [Canopy::new(LEAVES_CRIMSON, LEAVES_ORANGE, LEAVES_YELLOW), autumns(0), Canopy::new(LEAVES_RED, LEAVES_ORANGE, LEAVES_ORANGE)][(rng.next() * 2.99) as usize]
        } else if rng.next() < 0.5 {
            Canopy::new(LEAVES_DARK, LEAVES, LEAVES_LIME)
        } else {
            greens(1)
        };
        let mut t = Parts::default();
        self.trunk(&mut t.wood, foot, top, radius, 0.55, bark, if s > 0.6 { 4 } else { 0 }, rng);
        let limbs = 4 + (rng.next() * 1.99) as usize;
        let spin = rng.range(0.0, TAU);
        let mut highest = top[2];
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.25, 0.25)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.9, 1.35)]);
            let start = lerp3(foot, top, rng.range(0.6, 0.95));
            let end = self.limb(&mut t.wood, start, dir, rng.range(2.0, 2.7) * s, radius * 0.5, bark, rng);
            let r = rng.range(1.2, 1.45) * s.max(0.55);
            highest = highest.max(end[2]);
            self.lobe(&mut t, add(end, [0.0, 0.0, r * 0.2]), r, 1.15, canopy, bark.0, rng);
        }
        let r = rng.range(1.25, 1.45) * s.max(0.55);
        let crown = add(top, [rng.range(-0.3, 0.3) * s, rng.range(-0.3, 0.3) * s, (highest - top[2]) * 0.7 + r * 0.6]);
        t.wood.push(Shape::wood(top, crown, radius * 0.5, radius * 0.25, bark.0));
        self.lobe(&mut t, crown, r * 1.1, 1.15, canopy, bark.0, rng);
        self.lobe(&mut t, add(crown, [0.0, 0.0, r * 1.2]), r * 0.8, 1.15, canopy, AIR, rng);
        t.shapes()
    }

    /// A maple: a short trunk under a crown shaped like an egg, of many smaller masses of leaves
    /// all round it, most often blazing red, orange or gold where the leaves have turned
    #[allow(clippy::too_many_arguments)]
    fn maple(&self, x: f32, y: f32, base: f32, size: f32, lean: [f32; 2], rng: &mut Rng) -> Vec<Shape> {
        let s = size;
        let height = rng.range(2.2, 3.0) * s.max(0.6);
        let radius = rng.range(0.22, 0.3) * s.max(0.5);
        let bark = (WOOD, BARK_DARK);
        let foot = [x, y, base];
        let top = [x + lean[0] * height + rng.range(-0.2, 0.2), y + lean[1] * height + rng.range(-0.2, 0.2), base + height];
        let turned = smoothstep(-0.6, 0.3, autumn(&self.settings, x, y));
        let canopy = if rng.next() < 0.2 + 0.75 * turned {
            [
                Canopy::new(LEAVES_CRIMSON, LEAVES_RED, LEAVES_ORANGE),
                Canopy::new(LEAVES_CRIMSON, LEAVES_RED, FLOWER_RED),
                Canopy::new(LEAVES_RED, LEAVES_ORANGE, LEAVES_YELLOW),
                Canopy::new(LEAVES_RED, LEAVES_RED, LEAVES_ORANGE),
                autumns(0),
            ][(rng.next() * 4.99) as usize]
        } else if rng.next() < 0.5 {
            Canopy::new(LEAVES, LEAVES_LIME, LEAVES_LIME)
        } else {
            greens(2)
        };
        let mut t = Parts::default();
        self.trunk(&mut t.wood, foot, top, radius, 0.6, bark, if s > 0.6 { 4 } else { 0 }, rng);
        let (wide, tall) = (rng.range(2.0, 2.4) * s.max(0.55), rng.range(2.4, 2.9) * s.max(0.55));
        let middle = add(top, [0.0, 0.0, tall * 0.72]);
        t.wood.push(Shape::wood(top, middle, radius * 0.6, radius * 0.3, bark.0));
        self.lobe(&mut t, middle, wide * 0.7, 1.1, canopy.deep(), AIR, rng);
        let count = 9 + (rng.next() * 3.99) as usize;
        let spin = rng.range(0.0, TAU);
        for k in 0..count {
            // Spread round it from the top down, as lobe() spreads its clumps
            let z = 0.9 - 1.6 * (k as f32 + 0.5) / count as f32;
            let ring = (1.0 - z * z).max(0.0).sqrt();
            let a = spin + k as f32 * 2.399_963;
            let at = add(middle, [ring * a.cos() * wide * 0.75, ring * a.sin() * wide * 0.75, z * tall * 0.7]);
            if k % 3 == 0 {
                t.wood.push(Shape::wood(top, at, radius * 0.35, radius * 0.15, bark.0));
            }
            self.lobe(&mut t, at, rng.range(0.85, 1.1) * s.max(0.55), 1.0, canopy, AIR, rng);
        }
        t.shapes()
    }

    /// A larch: a pine that sheds its needles, slender, its boughs few, short and sweeping up at
    /// their tips, with sky between them, fresh light green, or gold where the leaves have turned
    /// and always in the snow
    pub(super) fn larch(&self, x: f32, y: f32, base: f32, size: f32, snowy: bool, rng: &mut Rng) -> Vec<Shape> {
        let s = size;
        let height = rng.range(9.0, 13.0) * s;
        let crown = rng.range(1.4, 1.9) * s.max(0.5);
        let radius = rng.range(0.16, 0.22) * s.max(0.5);
        let turned = smoothstep(-0.5, 0.3, autumn(&self.settings, x, y));
        let needles = if snowy || rng.next() < 0.25 + 0.75 * turned {
            [Canopy::new(LEAVES_ORANGE, LEAVES_YELLOW, LEAVES_YELLOW), Canopy::new(LEAVES_YELLOW, LEAVES_YELLOW, FLOWER_YELLOW), Canopy::new(LEAVES_ORANGE, LEAVES_ORANGE, LEAVES_YELLOW)][(rng.next() * 2.99) as usize]
        } else {
            Canopy::new(LEAVES, LEAVES_LIME, LEAVES_LIME)
        };
        let top = [x + rng.range(-0.25, 0.25), y + rng.range(-0.25, 0.25), base + height];
        let mut wood = Vec::new();
        self.trunk(&mut wood, [x, y, base], top, radius * 1.2, 0.2, (WOOD, BARK_DARK), if s > 0.6 { 3 } else { 0 }, rng);
        let (mut boughs, mut snow) = (Vec::new(), Vec::new());
        let spacing = (0.85 * s).max(0.5);
        let lowest = base + height * rng.range(0.25, 0.35);
        let spin = rng.range(0.0, TAU);
        let mut z = lowest;
        let mut i = 0;
        while z < base + height * 0.95 {
            let t = (z - lowest) / (base + height - lowest);
            i += 1;
            let reach = (crown * (1.0 - 0.85 * t) * rng.range(0.8, 1.1)).max(0.3);
            let around = lerp3([x, y, base], top, (z - base) / height);
            let count = if reach > 0.8 { 4 } else { 3 };
            for k in 0..count {
                let a = spin + i as f32 * 1.1 + (k as f32 + rng.range(-0.25, 0.25)) / count as f32 * TAU;
                let length = reach * rng.range(0.85, 1.1);
                let root = [around[0], around[1], z];
                // Down a little, then sweeping up at the tip
                let mid = [root[0] + a.cos() * length * 0.6, root[1] + a.sin() * length * 0.6, z - length * 0.12];
                let tip = [root[0] + a.cos() * length, root[1] + a.sin() * length, z + length * 0.08];
                let thick = (length * 0.22).clamp(0.14, 0.32);
                boughs.push(leafy_shape(Shape::capsule(root, mid, thick, thick * 0.8, needles.body, AIR, false), needles));
                boughs.push(leafy_shape(Shape::capsule(mid, tip, thick * 0.8, 0.08, needles.body, AIR, false), needles));
                if snowy {
                    snow.push(Shape::capsule(add(root, [0.0, 0.0, thick * 0.6]), add(mid, [0.0, 0.0, thick * 0.5]), thick * 0.55, thick * 0.45, SNOW, AIR, false));
                }
            }
            z += spacing * (1.0 - 0.4 * t) + rng.range(-0.1, 0.15);
        }
        boughs.push(leafy_shape(Shape::capsule(top, add(top, [0.0, 0.0, 0.7 * s]), 0.14, 0.04, needles.body, AIR, false), needles));
        wood.append(&mut boughs);
        wood.append(&mut snow);
        wood
    }

    /// A dead snag: a tall dead pine, grey, its top broken off in splinters, stubs of its branches
    /// all the way up and the odd longer one hanging down, fungus on its side now and then, and in
    /// the snow a cap of it on top
    pub(super) fn snag(&self, x: f32, y: f32, base: f32, size: f32, snowy: bool, rng: &mut Rng) -> Vec<Shape> {
        let s = size.max(0.6);
        let height = rng.range(5.5, 10.0) * s;
        let radius = rng.range(0.16, 0.26) * s;
        let top = [x + rng.range(-0.4, 0.4), y + rng.range(-0.4, 0.4), base + height];
        // Straight, so the stubs of its branches all start in it, on roots
        let mut shapes = vec![Shape::capsule([x, y, base - 0.6], top, radius * 1.15, radius * 0.5, DEAD_WOOD, WOOD, false)];
        let spin = rng.range(0.0, TAU);
        for k in 0..4 {
            let a = spin + (k as f32 + rng.range(-0.3, 0.3)) / 4.0 * TAU;
            let out = radius * rng.range(2.0, 3.0);
            let end = [x + a.cos() * out, y + a.sin() * out, base - 0.2];
            shapes.push(Shape::capsule([x, y, base + radius * rng.range(0.8, 1.4)], end, radius * 0.55, radius * 0.15, DEAD_WOOD, WOOD, false));
        }
        // Splinters where the top broke off
        for _ in 0..3 {
            let up = add(top, [rng.range(-0.15, 0.15), rng.range(-0.15, 0.15), rng.range(0.3, 0.8) * s]);
            shapes.push(Shape::wood(top, up, radius * 0.4, 0.04, DEAD_WOOD));
        }
        let stubs = 8 + (rng.next() * 6.99) as usize;
        for _ in 0..stubs {
            let along = rng.range(0.25, 0.95);
            let start = lerp3([x, y, base], top, along);
            let a = rng.range(0.0, TAU);
            let dir = normalize([a.cos(), a.sin(), rng.range(-0.5, 0.35)]);
            let mut length = rng.range(0.25, 0.7) * s;
            if rng.next() < 0.2 {
                length *= 2.5;
            }
            shapes.push(Shape::wood(start, add(start, scale(dir, length)), (radius * 0.3 * (1.1 - along)).max(0.05), 0.045, DEAD_WOOD));
        }
        if rng.next() < 0.4 {
            for _ in 0..2 + (rng.next() * 2.99) as usize {
                let a = rng.range(0.0, TAU);
                let out = [a.cos(), a.sin(), 0.0];
                let at = [x + out[0] * radius * 1.05, y + out[1] * radius * 1.05, base + rng.range(0.6, 2.2)];
                shapes.insert(0, Shape::leaf(at, out, [0.0, 0.0, 1.0], 0.3, 0.24, MUSHROOM_CAP, MUSHROOM_STEM));
            }
        }
        if snowy {
            shapes.push(Shape::sphere(add(top, [0.0, 0.0, 0.1]), radius * 0.8, SNOW, AIR, false));
        }
        shapes
    }

    /// A baobab, at the edge of the desert: a great grey trunk swelling like a bottle, crowned
    /// with stubby crooked branches like roots, small masses of leaves at their ends, fewer and
    /// smaller in the dry season
    fn baobab(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let s = size.clamp(0.55, 1.1);
        let height = rng.range(4.2, 5.4) * s;
        let girth = rng.range(0.7, 0.9) * s;
        let bark = (DEAD_WOOD, WOOD);
        let swell = [x, y, base + height * 0.35];
        let neck = [x + rng.range(-0.2, 0.2), y + rng.range(-0.2, 0.2), base + height];
        let mut t = Parts::default();
        t.wood.push(Shape::capsule([x, y, base - 0.4], swell, girth * 0.92, girth, bark.0, bark.1, false));
        t.wood.push(Shape::capsule(swell, neck, girth, girth * 0.4, bark.0, bark.1, false));
        let dry = rng.next() < 0.3;
        let canopy = Canopy::new(LEAVES_DARK, LEAVES, LEAVES_LIME);
        let branches = 5 + (rng.next() * 3.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..branches {
            let a = spin + (i as f32 + rng.range(-0.3, 0.3)) / branches as f32 * TAU;
            let dir = normalize([a.cos(), a.sin(), rng.range(0.5, 1.2)]);
            let start = add(neck, [a.cos() * girth * 0.2, a.sin() * girth * 0.2, -0.2]);
            let length = rng.range(1.4, 2.3) * s;
            let elbow = add(start, add(scale(dir, length * 0.55), [rng.range(-0.25, 0.25), rng.range(-0.25, 0.25), 0.0]));
            let end = add(elbow, scale(normalize(add(dir, [0.0, 0.0, 0.4])), length * 0.45));
            let r = girth * 0.24;
            t.wood.push(Shape::capsule(start, elbow, r, r * 0.65, bark.0, bark.1, false));
            t.wood.push(Shape::capsule(elbow, end, r * 0.65, r * 0.3, bark.0, AIR, false));
            for side in [-1.0f32, 1.0] {
                let twig = add(end, [(a + side * 0.7).cos() * 0.5 * s, (a + side * 0.7).sin() * 0.5 * s, rng.range(0.2, 0.5)]);
                t.wood.push(Shape::wood(end, twig, r * 0.3, 0.045, bark.0));
            }
            if !dry || i % 2 == 0 {
                let r = rng.range(0.6, 0.85) * s.max(0.7) * if dry { 0.7 } else { 1.0 };
                self.lobe(&mut t, add(end, [0.0, 0.0, 0.3]), r, 0.55, canopy, AIR, rng);
            }
        }
        t.shapes()
    }

    /// A Joshua tree, out in the desert among the cacti: a shaggy trunk forking into arms that
    /// fork again, each ending in a spiky tuft of stiff leaves with a skirt of dead ones under it
    pub(in crate::worldgen) fn joshua(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let s = rng.range(0.7, 1.25);
        let (mut wood, mut leaves) = (Vec::new(), Vec::new());
        let top = [x + rng.range(-0.15, 0.15), y + rng.range(-0.15, 0.15), ground + rng.range(1.2, 2.2) * s];
        wood.push(Shape::capsule([x, y, ground - 0.4], top, 0.2 * s, 0.16 * s, PALM_WOOD, DEAD_WOOD, false));
        let depth = if s > 0.85 { 2 } else { 1 };
        self.joshua_arm(&mut wood, &mut leaves, top, [0.0, 0.0, 1.0], 0.9 * s, 0.15 * s, depth, true, rng);
        wood.append(&mut leaves);
        wood
    }

    #[allow(clippy::too_many_arguments)]
    fn joshua_arm(&self, wood: &mut Vec<Shape>, leaves: &mut Vec<Shape>, start: Vec3, dir: Vec3, length: f32, radius: f32, depth: u32, first: bool, rng: &mut Rng) {
        let end = add(start, scale(dir, length));
        if !first {
            wood.push(Shape::capsule(start, end, radius, radius * 0.85, PALM_WOOD, DEAD_WOOD, false));
        }
        if depth == 0 || !first && rng.next() < 0.15 {
            // A skirt of dead leaves hanging under the tuft, then the tuft
            for k in 0..6 {
                let a = k as f32 / 6.0 * TAU + rng.range(-0.3, 0.3);
                wood.push(Shape::wood(end, add(end, [a.cos() * 0.16, a.sin() * 0.16, -0.32]), 0.05, 0.04, DEAD_WOOD));
            }
            for _ in 0..16 {
                let d = normalize(add(rng.direction(0.0), scale(dir, 0.9)));
                leaves.push(Shape::capsule(end, add(end, scale(d, rng.range(0.35, 0.55))), 0.04, 0.012, PALM_LEAF, LEAVES, false));
            }
            return;
        }
        let forks = if rng.next() < 0.3 { 3 } else { 2 };
        let spin = rng.range(0.0, TAU);
        for k in 0..forks {
            let mut next = bend(dir, rng.range(0.5, 0.9), spin + k as f32 / forks as f32 * TAU);
            next[2] += 0.35;
            let from = if first { start } else { end };
            self.joshua_arm(wood, leaves, from, normalize(next), length * 0.85, radius * 0.8, depth - 1 + first as u32, false, rng);
        }
    }

    /// A kapok, a giant of the jungle standing over all the rest: a pale trunk straight up out of
    /// tall buttresses flaring from its foot like fins, and up top limbs going out nearly level
    /// under flat masses of leaves, an umbrella over the jungle, sometimes in flower
    fn kapok(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let s = size.clamp(0.6, 1.12);
        let height = rng.range(15.0, 20.0) * s;
        let radius = rng.range(0.42, 0.55) * s;
        let bark = (DEAD_WOOD, MUSHROOM_STEM);
        let foot = [x, y, base];
        let top = [x + rng.range(-0.5, 0.5), y + rng.range(-0.5, 0.5), base + height];
        let mut t = Parts::default();
        self.trunk(&mut t.wood, foot, top, radius, 0.55, bark, 0, rng);
        // Buttresses: slabs across the foot, each a fin out either side
        let slabs = 2 + (rng.next() < 0.5) as usize;
        let spin = rng.range(0.0, TAU);
        for k in 0..slabs {
            let a = spin + k as f32 * std::f32::consts::PI / slabs as f32 + rng.range(-0.2, 0.2);
            t.wood.push(slab([x, y, base - 0.2], [a.cos(), a.sin(), 0.0], [rng.range(1.8, 2.6) * s, 0.1 * s.max(0.8), rng.range(2.2, 3.2) * s], bark));
        }
        let mut canopy = Canopy::new(LEAVES_DARK, LEAVES, LEAVES_LIME);
        if rng.next() < 0.25 {
            canopy = canopy.speckled(if rng.next() < 0.5 { FLOWER_WHITE } else { FLOWER_PINK }, rng.range(16.0, 30.0));
        }
        let limbs = 5 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let a = spin + (i as f32 + rng.range(-0.25, 0.25)) / limbs as f32 * TAU;
            let dir = normalize([a.cos(), a.sin(), rng.range(0.1, 0.35)]);
            let start = lerp3(foot, top, rng.range(0.8, 1.0));
            let end = self.limb(&mut t.wood, start, dir, rng.range(3.0, 4.0) * s, radius * 0.4, bark, rng);
            self.lobe(&mut t, add(end, [0.0, 0.0, 0.3]), rng.range(1.4, 1.75) * s, 0.5, canopy, AIR, rng);
        }
        let r = 1.9 * s;
        let crown = add(top, [0.0, 0.0, 0.9]);
        t.wood.push(Shape::wood(top, crown, radius * 0.5, radius * 0.3, bark.0));
        self.lobe(&mut t, crown, r, 0.5, canopy, AIR, rng);
        // Lianas hanging from the limbs
        for _ in 0..5 {
            let a = rng.next() * TAU;
            let d = rng.range(1.5, 4.0) * s;
            let start = [top[0] + a.cos() * d, top[1] + a.sin() * d, top[2] - 0.4];
            let end = [start[0], start[1], start[2] - rng.range(3.0, height * 0.5)];
            t.wood.push(Shape::capsule(start, end, 0.06, 0.04, VINE, LEAVES_DARK, false));
        }
        t.shapes()
    }

    /// A banyan: several stems grown together into one, long limbs going out nearly level, and
    /// from them roots dropping straight to the ground, standing like pillars under its broad,
    /// dark crown
    fn banyan(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let s = size.clamp(0.6, 1.1);
        let height = rng.range(4.0, 5.5) * s;
        let bark = (WOOD, BARK_DARK);
        let foot = [x, y, base];
        let top = [x + rng.range(-0.3, 0.3), y + rng.range(-0.3, 0.3), base + height];
        let mut t = Parts::default();
        self.trunk(&mut t.wood, foot, top, 0.4 * s, 0.7, bark, 5, rng);
        let spin = rng.range(0.0, TAU);
        for k in 0..3 {
            let a = spin + k as f32 / 3.0 * TAU;
            let o = 0.35 * s;
            t.wood.push(Shape::capsule([x + a.cos() * o, y + a.sin() * o, base - 0.5], add(top, [a.cos() * 0.15, a.sin() * 0.15, -0.3]), 0.28 * s, 0.2 * s, bark.0, bark.1, false));
        }
        let mut canopy = Canopy::new(LEAVES_DARK, LEAVES_DARK, LEAVES);
        if rng.next() < 0.3 {
            canopy = canopy.speckled(FLOWER_RED, 18.0);
        }
        let limbs = 5 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let a = spin + (i as f32 + rng.range(-0.25, 0.25)) / limbs as f32 * TAU;
            let start = lerp3(foot, top, rng.range(0.7, 1.0));
            let reach = rng.range(3.0, 4.4) * s;
            let end = add(start, [a.cos() * reach, a.sin() * reach, rng.range(0.3, 1.0)]);
            t.wood.push(Shape::capsule(start, end, 0.22 * s, 0.1 * s, bark.0, bark.1, false));
            // Its roots dropping to the ground, but not onto a road
            for _ in 0..1 + (rng.next() * 2.99) as usize {
                let p = lerp3(start, end, rng.range(0.4, 0.95));
                if self.roads.near(p[0], p[1], 1.0) {
                    continue;
                }
                let ground = height_at(&self.settings, p[0], p[1]).min(base) - 0.6;
                t.wood.push(Shape::capsule([p[0], p[1], ground], p, rng.range(0.1, 0.16) * s, 0.07 * s, bark.0, bark.1, false));
            }
            let r = rng.range(1.3, 1.7) * s;
            self.lobe(&mut t, add(end, [0.0, 0.0, 0.4]), r, 0.6, canopy, AIR, rng);
            self.lobe(&mut t, add(lerp3(start, end, 0.5), [0.0, 0.0, r * 0.6]), r * 0.8, 0.6, canopy, AIR, rng);
        }
        self.lobe(&mut t, add(top, [0.0, 0.0, 1.2]), 1.8 * s, 0.6, canopy, AIR, rng);
        t.shapes()
    }

    /// A mangrove on a warm shore: its trunk held up off the ground on arching stilt roots, a
    /// dense, dark crown, and thin roots hanging from its limbs
    fn mangrove(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let s = size.clamp(0.5, 1.1);
        let lift = rng.range(0.9, 1.5) * s;
        let height = rng.range(2.2, 3.2) * s;
        let bark = (BARK_DARK, WOOD);
        let foot = [x, y, base + lift];
        let top = [x + rng.range(-0.3, 0.3), y + rng.range(-0.3, 0.3), base + lift + height];
        let mut t = Parts::default();
        t.wood.push(Shape::capsule(foot, top, 0.17 * s.max(0.6), 0.12 * s.max(0.6), bark.0, bark.1, false));
        let roots = 7 + (rng.next() * 3.99) as usize;
        let spin = rng.range(0.0, TAU);
        for k in 0..roots {
            let a = spin + (k as f32 + rng.range(-0.3, 0.3)) / roots as f32 * TAU;
            let start = [x, y, base + lift * rng.range(0.6, 1.1)];
            let out = rng.range(1.2, 2.0) * s.max(0.7);
            let knee = [x + a.cos() * out * 0.55, y + a.sin() * out * 0.55, start[2] + rng.range(0.1, 0.35)];
            let end = [x + a.cos() * out, y + a.sin() * out, base - 0.4];
            t.wood.push(Shape::capsule(start, knee, 0.08, 0.07, bark.0, bark.1, false));
            t.wood.push(Shape::capsule(knee, end, 0.07, 0.05, bark.0, bark.1, false));
        }
        let canopy = if rng.next() < 0.5 { Canopy::new(LEAVES_DARK, LEAVES_DARK, LEAVES) } else { greens(0) };
        let limbs = 3 + (rng.next() * 1.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let a = spin + i as f32 / limbs as f32 * TAU;
            let dir = normalize([a.cos(), a.sin(), rng.range(0.5, 0.9)]);
            let start = lerp3(foot, top, rng.range(0.55, 0.9));
            let end = add(start, scale(dir, rng.range(1.0, 1.6) * s));
            t.wood.push(Shape::wood(start, end, 0.08, 0.05, bark.0));
            self.lobe(&mut t, end, rng.range(1.0, 1.3) * s.max(0.6), 0.75, canopy, AIR, rng);
            // A root or two hanging from it
            if rng.next() < 0.6 {
                let from = lerp3(start, end, rng.range(0.4, 0.8));
                t.wood.push(Shape::wood(from, [from[0], from[1], base - 0.3], 0.04, 0.04, bark.0));
            }
        }
        self.lobe(&mut t, add(top, [0.0, 0.0, 0.5]), 1.2 * s.max(0.6), 0.75, canopy, AIR, rng);
        t.shapes()
    }

    /// A bald cypress by the water: its foot flared wide, a tall straight trunk, level feathery
    /// boughs over its top half flattening out at the top, soft green or russet where the leaves
    /// have turned, moss hanging from them, and knees of wood standing up out of the ground round it
    fn cypress(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let s = size.clamp(0.5, 1.2);
        let height = rng.range(9.0, 13.0) * s;
        let radius = rng.range(0.22, 0.3) * s.max(0.6);
        let bark = (WOOD, DEAD_WOOD);
        let top = [x + rng.range(-0.25, 0.25), y + rng.range(-0.25, 0.25), base + height];
        let mut t = Parts::default();
        t.wood.push(Shape::capsule([x, y, base - 0.4], [x, y, base + 1.4 * s], radius * 2.4, radius * 1.05, bark.0, bark.1, false));
        self.trunk(&mut t.wood, [x, y, base], top, radius, 0.3, bark, if s > 0.6 { 5 } else { 0 }, rng);
        // Knees standing up round it, not on a road
        let knees = if s > 0.6 { 4 + (rng.next() * 5.99) as usize } else { 0 };
        for _ in 0..knees {
            let a = rng.range(0.0, TAU);
            let d = rng.range(1.3, 2.8) * s.max(0.7);
            let p = [x + a.cos() * d, y + a.sin() * d];
            if self.roads.near(p[0], p[1], 0.8) {
                continue;
            }
            let g = height_at(&self.settings, p[0], p[1]).min(base + 0.3);
            t.wood.push(Shape::capsule([p[0], p[1], g - 0.3], [p[0], p[1], g + rng.range(0.25, 0.6)], 0.12, 0.04, bark.0, bark.1, false));
        }
        let turned = smoothstep(-0.4, 0.4, autumn(&self.settings, x, y));
        let needles = if rng.next() < 0.15 + 0.7 * turned { Canopy::new(LEAVES_CRIMSON, LEAVES_ORANGE, LEAVES_RED) } else { Canopy::new(LEAVES_DARK, FERN, LEAVES_LIME) };
        let crown = rng.range(1.6, 2.2) * s.max(0.6);
        let lowest = base + height * rng.range(0.42, 0.5);
        let spin = rng.range(0.0, TAU);
        let (mut z, mut i) = (lowest, 0);
        let mut moss = Vec::new();
        while z < base + height * 0.97 {
            let t_up = (z - lowest) / (base + height - lowest);
            i += 1;
            // Widest a little below the middle, then drawn in to a flattish top
            let reach = (crown * (0.55 + 0.45 * (std::f32::consts::PI * (0.15 + 0.75 * t_up)).sin()) * rng.range(0.85, 1.1)).max(0.4);
            let around = lerp3([x, y, base], top, (z - base) / height);
            for k in 0..4 {
                let a = spin + i as f32 * 0.8 + (k as f32 + rng.range(-0.2, 0.2)) / 4.0 * TAU;
                let length = reach * rng.range(0.85, 1.1);
                let root = [around[0], around[1], z];
                let tip = [root[0] + a.cos() * length, root[1] + a.sin() * length, z - length * 0.1];
                let thick = (length * 0.32).clamp(0.22, 0.45);
                t.leaves.push(leafy_shape(Shape::capsule(root, tip, thick, 0.12, needles.body, AIR, false), needles));
                if rng.next() < 0.12 {
                    let from = lerp3(root, tip, rng.range(0.5, 0.9));
                    moss.push(Shape::capsule(from, add(from, [0.0, 0.0, -rng.range(0.8, 1.6)]), 0.06, 0.03, VINE, WILLOW, false));
                }
            }
            z += (0.7 * s).max(0.45) + rng.range(-0.08, 0.12);
        }
        self.lobe(&mut t, add(top, [0.0, 0.0, 0.2]), 1.2 * s.max(0.6), 0.5, needles, AIR, rng);
        t.leaves.append(&mut moss);
        t.shapes()
    }

    /// A dreambloom, the dreaming realm's: a pale trunk twisting up in a slow spiral to great soft
    /// puffs of blossom, strands of it hanging down from them like wisteria
    fn dreambloom(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let s = size.clamp(0.5, 1.2);
        let height = rng.range(4.0, 6.5) * s;
        let bark = (BIRCH_BARK, MUSHROOM_STEM);
        let mut t = Parts::default();
        let (spin, turns, wide) = (rng.range(0.0, TAU), rng.range(0.6, 1.1), 0.35 * s);
        let mut from = [x, y, base - 0.5];
        for i in 1..=6 {
            let f = i as f32 / 6.0;
            let a = spin + f * turns * TAU;
            let to = [x + a.cos() * wide * f, y + a.sin() * wide * f, base + height * f];
            t.wood.push(Shape::capsule(from, to, 0.22 * s * (1.0 - 0.45 * (f - 1.0 / 6.0)), 0.22 * s * (1.0 - 0.45 * f), bark.0, bark.1, false));
            from = to;
        }
        let top = from;
        // Blossom deep inside, so it comes down with the tree when it's felled, see gather.rs
        let colours = [(BLOSSOM, FLOWER_PURPLE, FLOWER_WHITE), (BLOSSOM, FLOWER_PINK, FLOWER_WHITE), (BLOSSOM, FLOWER_BLUE, FLOWER_PURPLE)];
        let (inner, body, tip) = colours[(rng.next() * 2.99) as usize];
        let canopy = Canopy::new(inner, body, tip).speckled(if rng.next() < 0.5 { FLOWER_YELLOW } else { FLOWER_WHITE }, 20.0);
        let mut puffs = vec![(add(top, [0.0, 0.0, 1.0 * s]), rng.range(1.6, 2.1) * s)];
        for k in 0..1 + (rng.next() * 2.99) as usize {
            let a = spin + k as f32 * 2.3;
            let start = lerp3([x, y, base], top, rng.range(0.6, 0.9));
            let end = self.limb(&mut t.wood, start, normalize([a.cos(), a.sin(), 0.6]), rng.range(1.6, 2.4) * s, 0.12 * s, bark, rng);
            puffs.push((end, rng.range(1.1, 1.5) * s));
        }
        let mut strands = Vec::new();
        for &(at, r) in &puffs {
            self.lobe(&mut t, at, r, 1.0, canopy, AIR, rng);
            for _ in 0..5 + (rng.next() * 3.99) as usize {
                let a = rng.range(0.0, TAU);
                let mut point = add(at, [a.cos() * r * 0.7, a.sin() * r * 0.7, -r * 0.45]);
                let length = rng.range(1.2, 2.4).min(point[2] - base - 0.5).max(0.3);
                let shade = if rng.next() < 0.5 { body } else { inner };
                for _ in 0..3 {
                    let next = add(point, [0.0, 0.0, -length / 3.0]);
                    strands.push(Shape::capsule(point, next, 0.07, 0.05, shade, tip, false));
                    point = next;
                }
            }
        }
        t.leaves.append(&mut strands);
        t.shapes()
    }

    /// A bloodthorn, in the dreaming realm's bleeding half: a black, twisted trunk leaning over,
    /// jagged limbs bristling with long thorns, and a few crimson leaves at their ends
    #[allow(clippy::too_many_arguments)]
    fn bloodthorn(&self, x: f32, y: f32, base: f32, size: f32, lean: [f32; 2], rng: &mut Rng) -> Vec<Shape> {
        let s = size.clamp(0.5, 1.15);
        let height = rng.range(2.5, 3.8) * s;
        let radius = rng.range(0.2, 0.28) * s;
        let bark = (BARK_DARK, DEAD_WOOD);
        let tilt = rng.range(0.0, TAU);
        let top = [x + (lean[0] + tilt.cos() * 0.25) * height, y + (lean[1] + tilt.sin() * 0.25) * height, base + height];
        let mut t = Parts::default();
        self.trunk(&mut t.wood, [x, y, base], top, radius, 0.6, bark, if s > 0.6 { 4 } else { 0 }, rng);
        let canopy = Canopy::new(LEAVES_CRIMSON, LEAVES_CRIMSON, LEAVES_RED).speckled(FLOWER_RED, 30.0);
        let limbs = 3 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let a = spin + i as f32 / limbs as f32 * TAU;
            let mut dir = normalize([a.cos(), a.sin(), rng.range(0.4, 1.0)]);
            let mut at = lerp3([x, y, base], top, rng.range(0.7, 1.0));
            let mut r = radius * 0.55;
            for _ in 0..3 {
                let length = rng.range(0.9, 1.4) * s;
                let next = add(at, scale(dir, length));
                t.wood.push(Shape::capsule(at, next, r, r * 0.7, bark.0, bark.1, false));
                // Thorns along it
                for _ in 0..2 + (rng.next() * 1.99) as usize {
                    let p = lerp3(at, next, rng.range(0.15, 0.9));
                    let d = normalize(add(rng.direction(0.2), scale(dir, 0.4)));
                    t.wood.push(Shape::wood(p, add(p, scale(d, rng.range(0.3, 0.6) * s)), 0.06, 0.04, bark.0));
                }
                at = next;
                r *= 0.7;
                dir = bend(dir, rng.range(0.4, 0.8), rng.range(0.0, TAU));
                dir[2] = dir[2].max(-0.2);
                dir = normalize(dir);
            }
            if rng.next() < 0.6 {
                self.lobe(&mut t, at, rng.range(0.6, 0.9) * s.max(0.7), 0.8, canopy, AIR, rng);
            }
        }
        t.shapes()
    }

    /// A rime tree, of the frozen realm and now and then out in the snow: pale bark and a crown
    /// all crusted white with frost, dark in its depths
    #[allow(clippy::too_many_arguments)]
    fn rime(&self, x: f32, y: f32, base: f32, size: f32, lean: [f32; 2], rng: &mut Rng) -> Vec<Shape> {
        let s = size;
        let height = rng.range(2.2, 3.0) * s.max(0.6);
        let radius = rng.range(0.18, 0.25) * s.max(0.5);
        let bark = (BIRCH_BARK, DEAD_WOOD);
        let top = [x + lean[0] * height + rng.range(-0.3, 0.3), y + lean[1] * height + rng.range(-0.3, 0.3), base + height];
        let canopy = Canopy::new(PINE, FLOWER_WHITE, FLOWER_WHITE);
        let mut t = Parts::default();
        self.trunk(&mut t.wood, [x, y, base], top, radius, 0.6, bark, if s > 0.6 { 4 } else { 0 }, rng);
        let limbs = 4 + (rng.next() * 1.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let a = spin + (i as f32 + rng.range(-0.25, 0.25)) / limbs as f32 * TAU;
            let dir = normalize([a.cos(), a.sin(), rng.range(0.6, 1.1)]);
            let start = lerp3([x, y, base], top, rng.range(0.7, 1.0));
            let end = self.limb(&mut t.wood, start, dir, rng.range(1.6, 2.2) * s, radius * 0.55, bark, rng);
            let r = rng.range(0.8, 1.1) * s.max(0.6);
            self.lobe(&mut t, end, r, 0.85, canopy, BIRCH_BARK, rng);
            if rng.next() < 0.5 {
                self.lobe(&mut t, add(lerp3(start, end, 0.5), [0.0, 0.0, r * 0.4]), r * 0.65, 0.85, canopy, AIR, rng);
            }
        }
        self.lobe(&mut t, add(top, [0.0, 0.0, 1.0 * s.max(0.6)]), 1.1 * s.max(0.6), 0.85, canopy, BIRCH_BARK, rng);
        t.shapes()
    }
}
