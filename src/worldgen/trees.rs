//! Trees, and what lies about on the forest floor under them. Crowns are built of clumps of
//! leaves, a big one in the middle of each mass of them and smaller ones bulging out all over it,
//! so their outline against the sky is lumpy, like broccoli. Each clump is solid deep inside, so
//! rays stop there, and all leaves outside that: leaf-sized bits standing out at different depths,
//! with clefts between them going into the shade, sunlit tips lighter, the deep inside darker,
//! and now and then speckled with blossom. Trunks bend a little as they go up, and flare out into
//! roots at the foot.

use super::*;

/// The leaves of a crown in three shades, deep inside, the body of it and the sunlit tips, and the
/// odd speckle of blossom or berries among them, AIR for none, on `bloom` leaves in 256
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Canopy {
    inner: u8,
    body: u8,
    tip: u8,
    speckle: u8,
    bloom: u8,
    /// How deep in the shade of the rest of the crown they are, from 0 for none
    shade: u8,
}

impl Canopy {
    const fn new(inner: u8, body: u8, tip: u8) -> Self {
        Canopy { inner, body, tip, speckle: AIR, bloom: 0, shade: 0 }
    }

    /// Speckled with `speckle` on `bloom` leaves in 256
    fn speckled(self, speckle: u8, bloom: f32) -> Self {
        Canopy { speckle, bloom: bloom.clamp(0.0, 255.0) as u8, ..self }
    }

    /// The same leaves in the shade of the rest of the crown, fewer of them sunlit
    fn shaded(self) -> Self {
        Canopy { shade: 1, ..self }
    }

    /// The same leaves deep in the middle of the crown, darker still
    fn deep(self) -> Self {
        Canopy { shade: 2, bloom: self.bloom / 2, ..self }
    }
}

/// Green crowns: deep, fresh and lime
const GREENS: [Canopy; 3] = [
    Canopy::new(LEAVES_DARK, LEAVES, FERN),
    Canopy::new(LEAVES_DARK, LEAVES, LEAVES_LIME),
    Canopy::new(LEAVES, FERN, LEAVES_LIME),
];
/// Crowns turned in autumn: golden, orange, red and crimson
const AUTUMN: [Canopy; 4] = [
    Canopy::new(LEAVES_ORANGE, LEAVES_YELLOW, FLOWER_YELLOW),
    Canopy::new(LEAVES_RED, LEAVES_ORANGE, LEAVES_YELLOW),
    Canopy::new(LEAVES_CRIMSON, LEAVES_RED, FLOWER_RED),
    Canopy::new(LEAVES_CRIMSON, LEAVES_RED, LEAVES_ORANGE),
];
/// Shrubs under the trees where the leaves have turned, as bright as the crowns over them or
/// brighter: scarlet, orange, golden, yellowish lime and lime
const SHRUBS: [Canopy; 5] = [
    Canopy::new(LEAVES_CRIMSON, LEAVES_RED, FLOWER_RED),
    Canopy::new(LEAVES_RED, LEAVES_ORANGE, LEAVES_YELLOW),
    Canopy::new(LEAVES_ORANGE, LEAVES_YELLOW, FLOWER_YELLOW),
    Canopy::new(LEAVES, LEAVES_LIME, FLOWER_YELLOW),
    Canopy::new(LEAVES, FERN, LEAVES_LIME),
];
/// Moss over the tops of rocks: dark down in it, bright green, and lighter at the tips
const MOSSY: Canopy = Canopy::new(MOSS, FERN, LEAVES_LIME);

/// Leaves stand out of a clump's surface by up to LEAF_OUT meters, and the clefts between them go
/// in up to LEAF_IN, below which it's solid
const LEAF_OUT: f32 = 0.14;
const LEAF_IN: f32 = 0.22;
/// How big a leaf is, and a sprig of them, in meters
const LEAF: f32 = 0.125;
const SPRIG: f32 = 0.3;

/// How far `p` is outside `shape`, a sphere or a capsule, in meters, below 0 inside, how deep the
/// leaves go there, and which way is out
fn outside(shape: &Shape, p: Vec3) -> (f32, f32, Vec3) {
    let d = sub(p, shape.a);
    let ab = sub(shape.b, shape.a);
    let length2 = dot(ab, ab);
    let t = if length2 > 0.0 { (dot(d, ab) / length2).clamp(0.0, 1.0) } else { 0.0 };
    let r = shape.radius_a + (shape.radius_b - shape.radius_a) * t;
    let e = sub(d, scale(ab, t));
    let length = dot(e, e).sqrt().max(1e-4);
    (length - r, LEAF_IN.min(r * 0.5), scale(e, 1.0 / length))
}

/// Which leaf `p` is on, a hash, and which sprig of leaves, from 0 to 1. Their grids are skewed,
/// so the leaves don't line up in rows.
fn leaf_at(p: Vec3) -> (u32, f32) {
    // A voxel is looked at for each of the clumps reaching it in turn, and then for which shade
    // it is, so the last one is kept
    thread_local! {
        static LAST: std::cell::Cell<(Vec3, u32, f32)> = const { std::cell::Cell::new(([f32::NAN; 3], 0, 0.0)) };
    }
    LAST.with(|last| {
        let (q, leaf, sprig) = last.get();
        if q == p {
            return (leaf, sprig);
        }
        let q = [p[0] + p[1] * 0.37, p[1] + p[2] * 0.37, p[2] + p[0] * 0.37];
        let cell = |size: f32, salt: u32| {
            let c = q.map(|v| (v / size).floor() as i32 as u32);
            hash3(c[0], c[1] ^ salt, c[2])
        };
        let found = (cell(LEAF, 0x1eaf), unit(cell(SPRIG, 0x5b71)));
        last.set((p, found.0, found.1));
        found
    })
}

/// How far out of a clump's surface `leaf` on `sprig` stands, in meters, `deep` being how deep
/// the leaves go there: some leaves stand out, others are further in, and between some there's a
/// gap going right down into the shade
fn leaf_reach(leaf: u32, sprig: f32, deep: f32) -> f32 {
    let l = unit(leaf);
    // Not on thin boughs, where they'd go right through
    if l < 0.2 && deep > 0.14 {
        return -deep;
    }
    ((sprig - 0.45) * 0.16 + (l - 0.6) * 0.3) * (deep / LEAF_IN)
}

/// Whether a leafy shape holds `p`, `depth` being as Shape::depth() works it out: everything deep
/// inside it, and the leaves round that
pub(super) fn leafy_holds(shape: &Shape, p: Vec3, depth: f32) -> bool {
    // Halfway in it's solid, as the leaves go in at most half its radius, and it's empty further
    // out than they reach even where it's thinnest
    if depth < 0.25 {
        return true;
    }
    let reach = 1.0 + LEAF_OUT / shape.radius_a.min(shape.radius_b).max(0.01);
    if depth > reach * reach {
        return false;
    }
    // For a ball, how far out it is follows from its depth, as most clumps are
    let (out, deep) = if shape.a == shape.b {
        (depth.sqrt() * shape.radius_a - shape.radius_a, LEAF_IN.min(shape.radius_a * 0.5))
    } else {
        let (out, deep, _) = outside(shape, p);
        (out, deep)
    };
    if out < -deep {
        return true;
    }
    if out > LEAF_OUT {
        return false;
    }
    let (leaf, sprig) = leaf_at(p);
    out < leaf_reach(leaf, sprig, deep)
}

/// Which shade of `canopy` the leaves at `p` of a leafy shape are: lighter the further out they
/// stand and the more they face up, darker in the clefts and underneath, each leaf a shade of its
/// own, and some of them blossom. `h` is a hash of where it is.
pub(super) fn leafy(shape: &Shape, canopy: Canopy, p: Vec3, h: u32) -> u8 {
    let (out, deep, normal) = outside(shape, p);
    if out < -deep - 0.1 {
        return canopy.inner;
    }
    let (leaf, sprig) = leaf_at(p);
    if canopy.speckle != AIR && hash(leaf ^ 0xb705) & 0xFF < canopy.bloom as u32 && out > -deep * 0.5 {
        return canopy.speckle;
    }
    let outward = ((out + deep) / (deep + LEAF_OUT)).clamp(0.0, 1.0);
    let light = normal[2] * 0.7 + outward * 0.9 + (unit(leaf >> 16) - 0.5) * 0.4 + (sprig - 0.5) * 0.3 + (unit(h) - 0.5) * 0.2 - canopy.shade as f32 * 0.3;
    if light > 0.8 {
        canopy.tip
    } else if light > 0.2 {
        canopy.body
    } else {
        canopy.inner
    }
}

/// `shape`, a sphere or capsule, made of leaves
fn leafy_shape(mut shape: Shape, canopy: Canopy) -> Shape {
    shape.material = canopy.body;
    shape.accent = AIR;
    shape.ragged = false;
    shape.pattern = Pattern::Leafy(canopy);
    shape.min = shape.min.map(|v| v - LEAF_OUT);
    shape.max = shape.max.map(|v| v + LEAF_OUT);
    shape
}

/// A round clump of leaves `radius` meters round `at`
fn clump(at: Vec3, radius: f32, canopy: Canopy) -> Shape {
    leafy_shape(Shape::sphere(at, radius, canopy.body, AIR, false), canopy)
}

/// The pieces of a tree as it's built. Earlier shapes win where they overlap, so the solid middles
/// of its clumps of leaves go first, so all of the inside of its crown is one material, twigs and
/// limbs and the leaves of other clumps going through it and all, and needn't be stored voxel by
/// voxel. Then its wood, then its leaves.
#[derive(Default)]
struct Parts {
    cores: Vec<Shape>,
    wood: Vec<Shape>,
    leaves: Vec<Shape>,
}

impl Parts {
    fn shapes(mut self) -> Vec<Shape> {
        self.cores.append(&mut self.wood);
        self.cores.append(&mut self.leaves);
        self.cores
    }

    /// A clump of leaves, see clump(), with its solid middle, as deep as its leaves go and more
    fn clump(&mut self, at: Vec3, radius: f32, canopy: Canopy) {
        let solid = radius - LEAF_IN.min(radius * 0.5) - 0.1;
        if solid > 0.12 {
            self.cores.push(Shape::sphere(at, solid, canopy.inner, AIR, false));
        }
        self.leaves.push(clump(at, radius, canopy));
    }
}

/// Which kind of broad-leaved tree, see Terrain::leafy()
#[derive(Clone, Copy, PartialEq)]
enum Form {
    /// Oaks and maples of the deep woods: a stout trunk forking low into many limbs that reach
    /// far out, under a great dome of leaves much wider than it is high
    Broad,
    /// Short and sturdy, with limbs spreading wide and low into a broad dome
    Spreading,
    /// Taller, with limbs reaching up into a round crown
    Upright,
    /// Crooked, dark limbs and a flat, wide crown, all in flower
    Blossom,
}

/// Trees are scattered at most one in each TREE_CELL meter square, see Terrain::place_objects(),
/// at spots picked with TREE_SALT
pub(super) const TREE_CELL: f32 = 7.5;
pub(super) const TREE_SALT: u32 = 0x7a3e;

/// How far the leaves have turned at `x`, `y`: they turn in patches, warmer the higher this is
pub(super) fn autumn(settings: &TerrainSettings, x: f32, y: f32) -> f32 {
    fbm(x / 120.0, y / 120.0, 2, settings.seed.wrapping_add(6))
}

/// Whether `x`, `y` is in thick woods where the leaves have turned, for finding some to look at
pub(super) fn autumn_woods(settings: &TerrainSettings, x: f32, y: f32) -> bool {
    autumn(settings, x, y) > 0.35 && fbm(x / 80.0, y / 80.0, 3, settings.seed.wrapping_add(4)) > 0.3
}

/// The kinds of broad-leaved tree in the milder lands, see Terrain::grow_tree()
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Broad,
    Upright,
    Spreading,
    Birch,
    Blossom,
    Poplar,
    Willow,
}

impl Terrain {
    /// A tree in a garden or by a town, no bigger than most, see grow_tree()
    pub(super) fn tree(&self, x: f32, y: f32, base: f32, r: u32) -> Object {
        self.grow_tree(x, y, base, r, false)
    }

    /// A tree out in the wild: in the thick of the broad-leaved woods they grow big and spreading,
    /// their crowns meeting overhead, see grow_tree()
    pub(super) fn wild_tree(&self, x: f32, y: f32, base: f32, r: u32) -> Object {
        self.grow_tree(x, y, base, r, true)
    }

    /// How likely a tree at `x`, `y`, standing `base` meters high, is a pine: mostly they are up
    /// in the highlands, with a stand of them here and there lower down, but hardly any where the
    /// leaves have turned
    pub(super) fn pines(&self, x: f32, y: f32, base: f32) -> f32 {
        let sea_level = sea_level_m(&self.settings);
        let altitude = (base - sea_level) / (self.settings.height_m as f32 - sea_level);
        let region = fbm(x / 90.0, y / 90.0, 2, self.settings.seed.wrapping_add(8));
        let highland = smoothstep(0.36, 0.56, altitude + region * 0.2);
        let turned = smoothstep(-0.2, 0.3, autumn(&self.settings, x, y));
        let stands = smoothstep(0.3, 0.6, region) * 0.45 * (1.0 - turned);
        highland.max(stands)
    }

    /// How much `x`, `y`, `base` meters high, is in the thick of the broad-leaved woods, 0 to 1,
    /// for how thick the undergrowth is there
    pub(super) fn broadleaf_woods(&self, x: f32, y: f32, base: f32) -> f32 {
        let c = climate(&self.settings, x, y);
        let forest = smoothstep(-0.15, 0.25, fbm(x / 80.0, y / 80.0, 3, self.settings.seed.wrapping_add(4)));
        forest * (1.0 - self.pines(x, y, base)) * (1.0 - c.jungle.max(c.desert).max(c.snow) * 2.0).max(0.0)
    }

    /// Which kind of tree grows where: pines up high and in the snow, palms in the desert, giants
    /// in the jungle, willows and palms by the water, and broad-leaved woods elsewhere, mostly of
    /// great spreading oaks and maples with groves of one or the other. `wild` trees grow bigger
    /// in the thick of the woods.
    fn grow_tree(&self, x: f32, y: f32, base: f32, r: u32, wild: bool) -> Object {
        let seed = self.settings.seed;
        let mut rng = Rng(r);

        let sea_level = sea_level_m(&self.settings);
        let altitude = (base - sea_level) / (self.settings.height_m as f32 - sea_level);
        let pines = self.pines(x, y, base);
        let grove = fbm(x / 50.0, y / 50.0, 2, seed.wrapping_add(9)) * 0.5 + 0.5;
        let forest = fbm(x / 80.0, y / 80.0, 3, seed.wrapping_add(4));

        let c = climate(&self.settings, x, y);
        if c.jungle > 0.5 {
            return Object::new(self.jungle_tree(x, y, base, &mut rng), rng.0);
        }
        if c.desert > 0.5 {
            return Object::new(self.palm(x, y, base, &mut rng), rng.0);
        }
        if c.snow > 0.5 {
            let spruce = rng.next() < 0.5;
            return Object::new(self.conifer(x, y, base, spruce, true, &mut rng), rng.0);
        }

        // Trees on a slope lean out over it a little
        let lean = self.downhill(x, y);
        let pick = rng.next();
        let shapes = if altitude > 0.55 && pick < 0.12 {
            self.broadleaf(x, y, base, 1.0, 2, DEAD_WOOD, BARE, &mut rng)
        } else if pick < pines {
            let spruce = rng.next() < 0.5;
            self.conifer(x, y, base, spruce, false, &mut rng)
        } else if base - sea_level < 2.5 {
            if c.desert + c.jungle > 0.25 && rng.next() < 0.6 { self.palm(x, y, base, &mut rng) } else { self.willow(x, y, base, &mut rng) }
        } else if rng.next() < smoothstep(0.15, 0.45, c.desert) * 0.85 {
            // Towards the desert, flat-topped trees of the dry lands
            self.acacia(x, y, base, &mut rng)
        } else {
            // Mostly grown, some young, and in the thick of the woods bigger, where the leaves have
            // turned bigger still, and the odd ancient giant
            let age = rng.next();
            let turned = smoothstep(-0.4, 0.4, autumn(&self.settings, x, y));
            let thick = if wild { smoothstep(-0.15, 0.3, forest) } else { 0.0 };
            let size = if age < 0.12 { rng.range(0.45, 0.65) } else { rng.range(0.8, 1.0) + (0.04 + 0.08 * turned) * thick };
            if wild && forest > 0.3 && age > 0.965 + 0.015 * turned {
                return Object::new(self.ancient(x, y, base, &mut rng), rng.0);
            }
            // Groves where oaks or maples have the upper hand, few blossoming where the leaves
            // have turned, and only now and then a poplar
            let kinds = [
                (Kind::Broad, 0.3 + 0.35 * grove),
                (Kind::Upright, 0.38 - 0.25 * grove),
                (Kind::Spreading, 0.06),
                (Kind::Birch, 0.07),
                (Kind::Blossom, 0.05 - 0.04 * turned),
                (Kind::Poplar, 0.03),
                (Kind::Willow, 0.02),
            ];
            let mut roll = rng.next() * kinds.iter().map(|k| k.1).sum::<f32>();
            let kind = kinds.iter().find(|k| {
                roll -= k.1;
                roll < 0.0
            });
            match kind.map_or(Kind::Broad, |k| k.0) {
                Kind::Broad => {
                    let canopy = self.canopy(x, y, &mut rng);
                    let size = if wild { size * (0.93 + 0.07 * turned) } else { size * 0.8 };
                    self.leafy(x, y, base, size, Form::Broad, lean, WOOD, canopy, &mut rng)
                }
                Kind::Upright => {
                    let canopy = self.canopy(x, y, &mut rng);
                    self.leafy(x, y, base, size, Form::Upright, lean, WOOD, canopy, &mut rng)
                }
                Kind::Spreading => {
                    let canopy = self.canopy(x, y, &mut rng);
                    self.leafy(x, y, base, size, Form::Spreading, lean, WOOD, canopy, &mut rng)
                }
                Kind::Birch => self.birch(x, y, base, size, &mut rng),
                Kind::Blossom => {
                    // In flower: blossom all over the leaves, rather than a cloud of pink
                    let flower = [BLOSSOM, FLOWER_PINK, FLOWER_PINK, FLOWER_WHITE][(rng.next() * 3.99) as usize];
                    let canopy = GREENS[(rng.next() * 2.99) as usize].speckled(flower, rng.range(36.0, 56.0));
                    self.leafy(x, y, base, size * 0.85, Form::Blossom, lean, BARK_DARK, canopy, &mut rng)
                }
                Kind::Poplar => {
                    let canopy = self.canopy(x, y, &mut rng);
                    self.poplar(x, y, base, size, canopy, &mut rng)
                }
                Kind::Willow => self.willow(x, y, base, &mut rng),
            }
        };
        Object::new(shapes, rng.0)
    }

    /// Which way and how far a tree at `x`, `y` leans: out over the slope it's on, a little
    fn downhill(&self, x: f32, y: f32) -> [f32; 2] {
        let h = |x: f32, y: f32| height_at(&self.settings, x, y);
        let slope = [h(x - 1.0, y) - h(x + 1.0, y), h(x, y - 1.0) - h(x, y + 1.0)].map(|d| d * 0.5);
        let steep = (slope[0] * slope[0] + slope[1] * slope[1]).sqrt();
        let lean = (steep * 0.6).min(0.35);
        if steep < 1e-3 { [0.0, 0.0] } else { [slope[0] / steep * lean, slope[1] / steep * lean] }
    }

    /// The shades of a crown. Most are green, but where the leaves have turned, in patches, most
    /// are red, crimson, orange or golden, reds the most, with bright green ones among them, and
    /// now and then one is in flower.
    fn canopy(&self, x: f32, y: f32, rng: &mut Rng) -> Canopy {
        let turned = smoothstep(-0.4, 0.4, autumn(&self.settings, x, y)) * 0.85;
        let canopy = if rng.next() < turned {
            AUTUMN[[0, 1, 2, 2, 3, 3][(rng.next() * 5.99) as usize]]
        } else if rng.next() < turned * 1.5 {
            GREENS[1 + (rng.next() * 1.99) as usize]
        } else {
            GREENS[(rng.next() * 2.99) as usize]
        };
        // Red ones often have pale blossom or berries among them
        let flowering = if canopy.body == LEAVES_RED { 0.35 } else { 0.12 };
        if rng.next() < flowering {
            let speckle = if rng.next() < 0.6 { FLOWER_WHITE } else { FLOWER_PINK };
            return canopy.speckled(speckle, rng.range(16.0, 36.0));
        }
        canopy
    }

    /// A mass of leaves about `radius` meters round `at`, `flat` times as high as it's wide: a big
    /// clump in the middle, in shade, and smaller ones bulging out of it all over its top and
    /// sides, fewer underneath, so its outline is lumpy and there are clefts going deep into the
    /// shade between them. Twigs of `bark` reach out into some of them, AIR for none.
    #[allow(clippy::too_many_arguments)]
    fn lobe(&self, t: &mut Parts, at: Vec3, radius: f32, flat: f32, canopy: Canopy, bark: u8, rng: &mut Rng) {
        let middle = radius * 0.6 * flat.max(0.65);
        if middle > LEAF_IN + 0.2 {
            t.cores.push(Shape::sphere(at, middle - LEAF_IN - 0.04, canopy.inner, AIR, false));
        }
        t.leaves.push(clump(at, middle, canopy.deep()));
        let count = (4.0 + 7.0 * (radius / 1.5).powi(2)).round() as usize;
        let spin = rng.next() * TAU;
        for k in 0..count {
            // Spread evenly over it from the top down, then jostled about
            let z = 0.95 - 1.45 * (k as f32 + 0.5) / count as f32;
            let ring = (1.0 - z * z).max(0.0).sqrt();
            let a = spin + k as f32 * 2.399_963;
            let dir = normalize(add([ring * a.cos(), ring * a.sin(), z], scale(rng.direction(0.0), 0.25)));
            let size = (radius * rng.range(0.32, 0.48) * (0.5 + 0.5 * flat)).max(0.22);
            let reach = radius - size * rng.range(0.7, 1.0);
            let center = add(at, [dir[0] * reach, dir[1] * reach, dir[2] * reach * flat]);
            // Those underneath, and some others, in the shade of the rest
            let shade = if dir[2] < -0.2 || rng.next() < 0.2 { canopy.shaded() } else { canopy };
            t.clump(center, size, shade);
            if bark != AIR && rng.next() < 0.35 {
                t.wood.push(Shape::wood(at, center, (radius * 0.04).max(0.04), 0.03, bark));
            }
        }
    }

    /// A trunk from `foot` up to `top`, bending a little this way and that as it goes, `radius`
    /// thick at the foot and `taper` times that at the top, flaring out into roots at the foot
    #[allow(clippy::too_many_arguments)]
    fn trunk(&self, wood: &mut Vec<Shape>, foot: Vec3, top: Vec3, radius: f32, taper: f32, bark: (u8, u8), roots: usize, rng: &mut Rng) {
        let segments = 3;
        let wobble = (top[2] - foot[2]) * 0.05;
        let mut from = [foot[0], foot[1], foot[2] - 0.6];
        let mut r = radius;
        for i in 1..=segments {
            let t = i as f32 / segments as f32;
            let mut to = lerp3(foot, top, t);
            if i < segments {
                let a = rng.range(0.0, TAU);
                to[0] += a.cos() * wobble;
                to[1] += a.sin() * wobble;
            }
            let next = radius * (1.0 - (1.0 - taper) * t);
            wood.push(Shape::capsule(from, to, r, next, bark.0, bark.1, false));
            from = to;
            r = next;
        }
        let spin = rng.range(0.0, TAU);
        for k in 0..roots {
            let a = spin + (k as f32 + rng.range(-0.3, 0.3)) / roots as f32 * TAU;
            let out = radius * rng.range(2.0, 3.2);
            let start = [foot[0], foot[1], foot[2] + (radius * rng.range(0.8, 1.6)).min((top[2] - foot[2]) * 0.6)];
            let end = [foot[0] + a.cos() * out, foot[1] + a.sin() * out, foot[2] - 0.2];
            wood.push(Shape::capsule(start, end, radius * 0.5, radius * 0.12, bark.0, bark.1, false));
        }
    }

    /// A limb from `start` going `dir` for about `length` meters, curving up towards the light as
    /// it goes. Returns where it ends.
    #[allow(clippy::too_many_arguments)]
    fn limb(&self, wood: &mut Vec<Shape>, start: Vec3, dir: Vec3, length: f32, radius: f32, bark: (u8, u8), rng: &mut Rng) -> Vec3 {
        let elbow = add(start, scale(dir, length * 0.55));
        let mut up = add(dir, [0.0, 0.0, rng.range(0.3, 0.7)]);
        up = normalize(up);
        let end = add(elbow, scale(up, length * 0.5));
        wood.push(Shape::capsule(start, elbow, radius, radius * 0.75, bark.0, bark.1, false));
        wood.push(Shape::capsule(elbow, end, radius * 0.75, radius * 0.45, bark.0, bark.1, false));
        end
    }

    /// A broad-leaved tree, `size` times as big as most: a trunk standing clear of the leaves,
    /// splitting into limbs, each ending in a mass of leaves, with another over the top, so the
    /// crown is a cluster of them with clefts between
    #[allow(clippy::too_many_arguments)]
    fn leafy(&self, x: f32, y: f32, base: f32, size: f32, form: Form, lean: [f32; 2], bark: u8, canopy: Canopy, rng: &mut Rng) -> Vec<Shape> {
        let (height, spread, rise, lobe) = match form {
            Form::Broad => (rng.range(2.0, 2.7), rng.range(2.9, 3.6), rng.range(0.22, 0.45), rng.range(1.45, 1.8)),
            Form::Spreading => (rng.range(2.2, 3.0), rng.range(1.8, 2.6), rng.range(0.35, 0.8), rng.range(1.3, 1.7)),
            Form::Upright => (rng.range(2.6, 3.4), rng.range(1.7, 2.3), rng.range(0.8, 1.2), rng.range(1.35, 1.65)),
            Form::Blossom => (rng.range(1.8, 2.4), rng.range(1.8, 2.4), rng.range(0.25, 0.6), rng.range(1.0, 1.3)),
        };
        let (height, spread, lobe) = (height * size, spread * size, lobe * size);
        let flat = match form {
            Form::Blossom => 0.75,
            Form::Broad => 0.78,
            _ => 1.0,
        };
        let radius = if form == Form::Broad { rng.range(0.3, 0.4) } else { rng.range(0.22, 0.32) } * size;
        let bark = (bark, if bark == WOOD { DEAD_WOOD } else { AIR });
        let tilt = rng.range(0.0, 0.25);
        let tilt_angle = rng.range(0.0, TAU);
        let foot = [x, y, base];
        let top = [x + (lean[0] + tilt_angle.cos() * tilt) * height, y + (lean[1] + tilt_angle.sin() * tilt) * height, base + height];

        let mut t = Parts::default();
        self.trunk(&mut t.wood, foot, top, radius, 0.7, bark, if size > 0.7 { 4 + (rng.next() * 1.99) as usize } else { 0 }, rng);

        // Broad ones fork lower into more limbs, with more masses of leaves along them, so the dome
        // is full rather than a ring of them round an empty middle
        let (limbs, low, along) = if form == Form::Broad { (5 + (rng.next() * 2.99) as usize, 0.7, 0.75) } else { (3 + (rng.next() * 2.99) as usize, 0.8, 0.4) };
        let spin = rng.range(0.0, TAU);
        let mut highest: f32 = top[2];
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.25, 0.25)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rise * rng.range(0.8, 1.2)]);
            let start = lerp3(foot, top, rng.range(low, 1.0));
            let end = self.limb(&mut t.wood, start, dir, spread * rng.range(0.85, 1.15), radius * 0.6, bark, rng);
            let r = lobe * rng.range(0.85, 1.15);
            let at = add(end, [0.0, 0.0, r * 0.15]);
            highest = highest.max(at[2]);
            self.lobe(&mut t, at, r, flat, canopy, bark.0, rng);
            // Now and then a smaller one partway along the limb
            if rng.next() < along {
                let mid = add(lerp3(start, end, rng.range(0.45, 0.65)), [0.0, 0.0, r * 0.3]);
                self.lobe(&mut t, mid, r * rng.range(0.55, 0.7), flat, canopy, bark.0, rng);
            }
        }
        if form == Form::Broad {
            // A ring of them over the middle, between the limbs and the top
            let spin = rng.range(0.0, TAU);
            for k in 0..3 {
                let a = spin + k as f32 / 3.0 * TAU;
                let r = lobe * rng.range(0.8, 0.95);
                let at = add(top, [a.cos() * spread * 0.42, a.sin() * spread * 0.42, (highest - top[2]) * 0.75 + r * 0.45]);
                t.wood.push(Shape::wood(top, at, radius * 0.45, radius * 0.25, bark.0));
                self.lobe(&mut t, at, r, flat, canopy, bark.0, rng);
            }
        }
        // Over the top, where the limbs come together
        let r = lobe * if form == Form::Blossom { 1.0 } else { rng.range(1.05, 1.3) };
        let up = if form == Form::Broad { (highest - top[2]) * 0.9 + r * 0.55 } else { (highest - top[2]) * 0.6 + r * 0.4 };
        let at = add(top, [rng.range(-0.3, 0.3) * size, rng.range(-0.3, 0.3) * size, up]);
        t.wood.push(Shape::wood(top, at, radius * 0.5, radius * 0.25, bark.0));
        self.lobe(&mut t, at, r, flat, canopy, bark.0, rng);
        t.shapes()
    }

    /// Now and then in the thick of the woods, a great old tree: a massive, mossy trunk on roots
    /// sprawling out over the ground, limbs as thick as most trees, and a huge crown
    fn ancient(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let size = rng.range(1.5, 1.85);
        let height = rng.range(3.0, 3.8) * size;
        let radius = rng.range(0.32, 0.4) * size;
        let bark = (WOOD, MOSS);
        let foot = [x, y, base];
        let top = [x + rng.range(-0.4, 0.4), y + rng.range(-0.4, 0.4), base + height];
        let mut canopy = Canopy::new(LEAVES_DARK, LEAVES_DARK, LEAVES);
        if rng.next() < 0.5 {
            canopy = canopy.speckled(FERN, 40.0);
        }
        // Where the leaves have turned, as red or golden as the rest
        if rng.next() < smoothstep(-0.2, 0.4, autumn(&self.settings, x, y)) {
            canopy = AUTUMN[[0, 2, 2, 3][(rng.next() * 3.99) as usize]];
        }

        let mut t = Parts::default();
        self.trunk(&mut t.wood, foot, top, radius, 0.6, bark, 6, rng);
        let limbs = 5 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.3, 0.3)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.25, 0.7)]);
            let start = lerp3(foot, top, rng.range(0.65, 1.0));
            let end = self.limb(&mut t.wood, start, dir, rng.range(3.0, 4.2) * size * 0.75, radius * 0.5, bark, rng);
            let r = rng.range(1.5, 2.0) * size * 0.75;
            self.lobe(&mut t, add(end, [0.0, 0.0, r * 0.15]), r, 1.0, canopy, WOOD, rng);
        }
        let r = 2.2 * size * 0.75;
        let crown = add(top, [0.0, 0.0, r * 0.9]);
        t.wood.push(Shape::wood(top, crown, radius * 0.5, radius * 0.3, WOOD));
        self.lobe(&mut t, crown, r, 1.0, canopy, WOOD, rng);
        t.shapes()
    }

    /// Tall and slender with white bark, and small masses of light leaves close round its top half,
    /// with sky showing here and there between them
    fn birch(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(6.0, 8.5) * size;
        let radius = rng.range(0.11, 0.16) * size.max(0.7);
        let top = [x + rng.range(-0.4, 0.4), y + rng.range(-0.4, 0.4), base + height];
        let mut t = Parts::default();
        self.trunk(&mut t.wood, [x, y, base], top, radius * 1.2, 0.35, (BIRCH_BARK, BARK_DARK), if size > 0.7 { 3 } else { 0 }, rng);

        let canopy = if autumn(&self.settings, x, y) > 0.1 { AUTUMN[0] } else { GREENS[2] };
        let branches = 6 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        let small = size.max(0.6);
        for i in 0..branches {
            let along = 0.45 + 0.45 * (i as f32 + rng.next()) / branches as f32;
            let angle = spin + i as f32 * 2.4;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.6, 1.2)]);
            let start = lerp3([x, y, base], top, along);
            let length = rng.range(0.5, 0.9) * (1.3 - along * 0.6) * size;
            let end = add(start, scale(dir, length));
            t.wood.push(Shape::wood(start, end, radius * 0.4, radius * 0.2, BIRCH_BARK));
            self.lobe(&mut t, end, rng.range(0.6, 0.85) * (1.15 - along * 0.4) * small, 1.0, canopy, AIR, rng);
        }
        self.lobe(&mut t, add(top, [0.0, 0.0, 0.2]), 0.6 * small, 1.0, canopy, AIR, rng);
        t.shapes()
    }

    /// A narrow flame of leaves on a trunk: limbs sweeping steeply up all round it, each a long
    /// spray of leaves, the low ones bulging out and the high ones drawn in to the tip, so it's
    /// widest a third of the way up and its outline is streaked upwards rather than lumpy
    fn poplar(&self, x: f32, y: f32, base: f32, size: f32, canopy: Canopy, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(7.5, 10.0) * size;
        let width = rng.range(1.0, 1.3) * size.max(0.6);
        let mut t = Parts::default();
        let top = [x + rng.range(-0.2, 0.2), y + rng.range(-0.2, 0.2), base + height * 0.8];
        self.trunk(&mut t.wood, [x, y, base], top, 0.22 * size.max(0.6), 0.4, (WOOD, DEAD_WOOD), if size > 0.7 { 4 } else { 0 }, rng);
        let bottom = base + 1.6 * size.max(0.6);
        let summit = base + height;
        let at = |z: f32| {
            let p = lerp3([x, y, base], top, ((z - base) / (height * 0.8)).min(1.0));
            [p[0], p[1], z]
        };
        // How far out it reaches `along` the way up from the bottom of the leaves to the tip
        let profile = |along: f32| width * (std::f32::consts::PI * (0.16 + 0.84 * along.clamp(0.0, 1.0))).sin().max(0.0).powf(0.65);
        let up = |along: f32| bottom + (summit - bottom) * along;
        // The middle, in shade, so no sky shows through, going nearly up to the tip
        let core = width * 0.5;
        let (low, high) = (up(0.06), up(0.9));
        t.cores.push(Shape::capsule(at(low), at(high), (core - LEAF_IN - 0.04).max(0.05), 0.05, canopy.inner, AIR, false));
        t.leaves.push(leafy_shape(Shape::capsule(at(low), at(high), core, (profile(0.9) * 0.6).max(0.22), canopy.body, AIR, false), canopy.deep()));
        let sprays = 8 + (rng.next() * 3.99) as usize;
        let spin = rng.range(0.0, TAU);
        for k in 0..sprays {
            let a = spin + k as f32 * 2.399_963;
            let out = [a.cos(), a.sin(), 0.0];
            // From low down, or partway up, to somewhere up towards the tip, its outer side
            // following the outline, and one of them right up to the tip
            let from = if k == 0 { rng.range(0.3, 0.45) } else { rng.range(0.0, 0.45) };
            let to = if k == 0 { 0.97 } else { (from + rng.range(0.45, 0.7)).min(rng.range(0.9, 0.97)) };
            let (w0, w1) = (profile(from), profile(to).max(0.3));
            let (r0, r1) = (w0 * rng.range(0.45, 0.55), (w1 * 0.5).max(0.2));
            let a0 = add(at(up(from) + r0 * 0.5), scale(out, w0 - r0));
            let a1 = add(at(up(to)), scale(out, (w1 - r1).max(0.0)));
            let shade = if from < 0.12 || rng.next() < 0.2 { canopy.shaded() } else { canopy };
            let solid = |r: f32| r - LEAF_IN.min(r * 0.5) - 0.1;
            if solid(r0) > 0.12 {
                t.cores.push(Shape::capsule(a0, a1, solid(r0), solid(r1).max(0.05), canopy.inner, AIR, false));
            }
            t.leaves.push(leafy_shape(Shape::capsule(a0, a1, r0, r1, canopy.body, AIR, false), shade));
        }
        t.shapes()
    }

    /// A thick, short trunk with limbs arching out, each ending in a mass of leaves, from which
    /// long curtains of leaves hang down nearly to the ground
    pub(super) fn willow(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(2.4, 3.2);
        let radius = rng.range(0.3, 0.42);
        let top = [x + rng.range(-0.4, 0.4), y + rng.range(-0.4, 0.4), base + height];
        let mut t = Parts::default();
        let mut strands = Vec::new();
        self.trunk(&mut t.wood, [x, y, base], top, radius * 1.2, 0.75, (WOOD, DEAD_WOOD), 5, rng);
        let canopy = Canopy::new(LEAVES, WILLOW, LEAVES_LIME);

        let limbs = 4 + (rng.next() * 2.99) as usize;
        for i in 0..limbs {
            let angle = (i as f32 + rng.range(-0.3, 0.3)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.5, 0.9)]);
            let end = add(top, scale(dir, rng.range(1.6, 2.4)));
            t.wood.push(Shape::wood(top, end, radius * 0.5, radius * 0.25, WOOD));
            self.lobe(&mut t, end, rng.range(0.8, 1.1), 0.85, canopy, AIR, rng);

            // Strands hanging down from round the mass at its end, drifting outwards a little,
            // with narrow leaves along them
            for _ in 0..(7 + (rng.next() * 3.99) as usize) {
                let a = angle + rng.range(-0.9, 0.9);
                let mut point = add(lerp3(top, end, rng.range(0.7, 1.0)), [a.cos() * rng.range(0.4, 1.0), a.sin() * rng.range(0.4, 1.0), -0.4]);
                let length = rng.range(1.6, 3.0).min(point[2] - base - 0.3).max(0.5);
                let shade = if rng.next() < 0.6 { WILLOW } else { LEAVES };
                let out = [a.cos(), a.sin(), 0.0];
                for _ in 0..3 {
                    let next = add(point, [out[0] * 0.06, out[1] * 0.06, -length / 3.0]);
                    strands.push(Shape::capsule(point, next, 0.06, 0.05, shade, FERN, false));
                    let middle = lerp3(point, next, 0.5);
                    for side in [-1.0, 1.0] {
                        let across = [-out[1] * side, out[0] * side, 0.0];
                        strands.push(Shape::leaf(add(middle, scale(across, 0.07)), add(across, [0.0, 0.0, -1.5]), out, 0.32, 0.09, shade, FERN));
                    }
                    point = next;
                }
            }
        }
        t.leaves.append(&mut strands);
        t.shapes()
    }

    /// A pine: a straight trunk tapering to a spire, with whorls of branches all the way up it,
    /// each branch a bough thick with needles, lighter at their tips, longest at the bottom.
    /// Spruces are narrow, their boughs drooping, firs broader and darker with level boughs. In the
    /// snow, snow lies along the top of each bough.
    fn conifer(&self, x: f32, y: f32, base: f32, spruce: bool, snowy: bool, rng: &mut Rng) -> Vec<Shape> {
        let young = rng.next() < 0.15;
        let size = if young { rng.range(0.4, 0.6) } else { rng.range(0.85, 1.15) };
        let (height, crown, droop, needles) = if spruce {
            (rng.range(8.0, 13.0), rng.range(1.5, 2.0), 0.4, Canopy::new(PINE, PINE, LEAVES_DARK))
        } else {
            (rng.range(7.0, 11.0), rng.range(2.0, 2.7), 0.15, Canopy::new(PINE, LEAVES_DARK, LEAVES))
        };
        let (height, crown) = (height * size, crown * size);
        let radius = rng.range(0.18, 0.26) * size.max(0.6);
        let top = [x + rng.range(-0.2, 0.2), y + rng.range(-0.2, 0.2), base + height];
        let mut wood = Vec::new();
        self.trunk(&mut wood, [x, y, base], top, radius * 1.2, 0.25, (WOOD, BARK_DARK), if young { 0 } else { 4 }, rng);
        let (mut boughs, mut snow) = (Vec::new(), Vec::new());

        let spacing = (0.75 * size).max(0.45);
        let lowest = base + height * rng.range(0.14, 0.24);
        let spin = rng.range(0.0, TAU);
        let mut z = lowest;
        let mut i = 0;
        // Whorls closer together towards the top, so the spire is thick with needles
        while z < base + height * 0.96 {
            let t = (z - lowest) / (base + height - lowest);
            i += 1;
            let reach = (crown * (1.0 - 0.9 * t) * rng.range(0.85, 1.1)).max(0.3);
            let around = lerp3([x, y, base], top, (z - base) / height);
            let count = if reach > 1.0 { 6 } else if reach > 0.6 { 5 } else { 4 };
            for k in 0..count {
                let a = spin + i as f32 * 0.7 + (k as f32 + rng.range(-0.2, 0.2)) / count as f32 * TAU;
                let root = [around[0], around[1], z];
                let length = reach * rng.range(0.85, 1.1);
                let tip = [root[0] + a.cos() * length, root[1] + a.sin() * length, z - length * droop + 0.1];
                let thick = (length * 0.3).clamp(0.2, 0.5);
                boughs.push(leafy_shape(Shape::capsule(root, tip, thick, 0.1, needles.body, AIR, false), needles));
                if snowy {
                    let up = [0.0, 0.0, thick * 0.55];
                    snow.push(Shape::capsule(add(root, up), add(tip, [0.0, 0.0, 0.08]), thick * 0.75, 0.07, SNOW, AIR, false));
                }
            }
            z += spacing * (1.0 - 0.45 * t) + rng.range(-0.1, 0.1);
        }
        // The spire
        boughs.push(leafy_shape(Shape::capsule(top, add(top, [0.0, 0.0, 0.8 * size]), 0.16, 0.04, needles.body, AIR, false), needles));
        if snowy {
            snow.push(Shape::sphere(add(top, [0.0, 0.0, 0.2]), 0.14, SNOW, AIR, false));
        }
        wood.append(&mut boughs);
        wood.append(&mut snow);
        wood
    }

    /// A jungle giant: a tall trunk on buttress roots, a broad, flat-topped crown of glossy leaves
    /// spread wide over everything else, now and then in flower, and vines hanging down from it
    pub(super) fn jungle_tree(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(9.0, 15.0);
        let radius = rng.range(0.35, 0.55);
        let spread = rng.range(3.0, 4.5);
        let top = [x + rng.range(-0.8, 0.8), y + rng.range(-0.8, 0.8), base + height];
        let mut t = Parts::default();
        self.trunk(&mut t.wood, [x, y, base], top, radius * 1.3, 0.6, (BARK_DARK, DEAD_WOOD), 0, rng);
        // Buttress roots, tall fins of wood down the foot of the trunk
        for i in 0..5 {
            let angle = i as f32 / 5.0 * TAU + rng.range(-0.4, 0.4);
            let out = rng.range(1.4, 2.0);
            let foot = [x + angle.cos() * out, y + angle.sin() * out, base - 0.3];
            for k in 0..3 {
                let up = [x, y, base + 0.8 + k as f32 * 0.5];
                t.wood.push(Shape::wood(up, lerp3(foot, [x, y, base - 0.3], k as f32 * 0.15), radius * 0.45, 0.1, BARK_DARK));
            }
        }
        let mut canopy = Canopy::new(LEAVES_DARK, LEAVES, if rng.next() < 0.4 { LEAVES_LIME } else { FERN });
        let flower = [AIR, AIR, FLOWER_RED, FLOWER_PINK, FLOWER_YELLOW][(rng.next() * 4.99) as usize];
        if flower != AIR {
            canopy = canopy.speckled(flower, rng.range(14.0, 30.0));
        }
        let around = 5 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..=around {
            let angle = spin + i as f32 / around as f32 * TAU + rng.range(-0.3, 0.3);
            let d = if i == around { 0.0 } else { spread * rng.range(0.7, 1.0) };
            let r = if i == around { rng.range(1.8, 2.2) } else { rng.range(1.4, 1.8) };
            let at = [top[0] + angle.cos() * d, top[1] + angle.sin() * d, top[2] + rng.range(-0.3, 0.5) - d * 0.12];
            if i < around {
                t.wood.push(Shape::wood([top[0], top[1], top[2] - 1.5], [at[0], at[1], at[2] - 0.5], 0.22, 0.1, BARK_DARK));
            }
            // Flatter than most, lying along the top of the jungle
            self.lobe(&mut t, at, r, 0.6, canopy, AIR, rng);
        }
        for _ in 0..9 {
            let angle = rng.next() * TAU;
            let d = spread * rng.range(0.5, 1.2);
            let start = [top[0] + angle.cos() * d, top[1] + angle.sin() * d, top[2] - 0.6];
            let end = [start[0] + rng.range(-0.2, 0.2), start[1] + rng.range(-0.2, 0.2), start[2] - rng.range(2.5, height * 0.6)];
            t.wood.push(Shape::capsule(start, end, 0.07, 0.04, VINE, LEAVES_DARK, false));
        }
        t.shapes()
    }

    /// A tree of the dry lands at the edge of the desert: a crooked trunk forking low into a few
    /// limbs leaning out and up, under one wide, flat crown like a parasol
    fn acacia(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let size = rng.range(0.8, 1.15);
        let radius = rng.range(0.16, 0.24) * size;
        let fork = [x + rng.range(-0.3, 0.3), y + rng.range(-0.3, 0.3), base + rng.range(1.2, 2.0) * size];
        let crown = base + rng.range(3.8, 5.0) * size;
        let mut t = Parts::default();
        self.trunk(&mut t.wood, [x, y, base], fork, radius, 0.8, (WOOD, DEAD_WOOD), 3, rng);
        let mut canopy = Canopy::new(LEAVES_DARK, LEAVES, WILLOW);
        if rng.next() < 0.25 {
            canopy = canopy.speckled(FLOWER_YELLOW, 40.0);
        }

        let limbs = 2 + (rng.next() * 1.99) as usize;
        let spin = rng.range(0.0, TAU);
        let mut ends = Vec::new();
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.2, 0.2)) / limbs as f32 * TAU;
            let out = rng.range(1.2, 2.4) * size;
            let end = [fork[0] + angle.cos() * out, fork[1] + angle.sin() * out, crown + rng.range(-0.3, 0.3)];
            // Crooked: out one way, then up another
            let bend = angle + rng.range(-0.6, 0.6);
            let elbow = add(lerp3(fork, end, 0.5), [bend.cos() * 0.4, bend.sin() * 0.4, -0.3]);
            t.wood.push(Shape::wood(fork, elbow, radius * 0.75, radius * 0.55, WOOD));
            t.wood.push(Shape::wood(elbow, end, radius * 0.55, radius * 0.3, WOOD));
            ends.push(end);
        }
        // A wide, flat mass of leaves over each limb, and more filling out the parasol between them
        let middle = scale(ends.iter().fold([0.0; 3], |a, &e| add(a, e)), 1.0 / ends.len() as f32);
        for &end in &ends {
            self.lobe(&mut t, add(end, [0.0, 0.0, 0.3]), rng.range(1.4, 1.9) * size, 0.45, canopy, WOOD, rng);
        }
        self.lobe(&mut t, add(middle, [rng.range(-0.5, 0.5), rng.range(-0.5, 0.5), 0.45]), rng.range(1.3, 1.7) * size, 0.45, canopy, WOOD, rng);
        t.shapes()
    }

    /// A smaller tree of the jungle, growing up in the shade between the giants: a slim, crooked
    /// trunk and a few masses of glossy leaves
    pub(super) fn jungle_sapling(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let mut canopy = Canopy::new(LEAVES_DARK, LEAVES, if rng.next() < 0.5 { LEAVES_LIME } else { FERN });
        if rng.next() < 0.3 {
            canopy = canopy.speckled(FERN, 40.0);
        }
        let height = rng.range(2.5, 4.0);
        let top = [x + rng.range(-0.5, 0.5), y + rng.range(-0.5, 0.5), base + height];
        let mut t = Parts::default();
        self.trunk(&mut t.wood, [x, y, base], top, rng.range(0.1, 0.15), 0.6, (BARK_DARK, AIR), 0, rng);
        let spin = rng.range(0.0, TAU);
        let limbs = 2 + (rng.next() * 1.99) as usize;
        for i in 0..limbs {
            let angle = spin + i as f32 / limbs as f32 * TAU;
            let start = lerp3([x, y, base], top, rng.range(0.6, 0.85));
            let end = add(start, [angle.cos() * rng.range(0.7, 1.1), angle.sin() * rng.range(0.7, 1.1), rng.range(0.4, 0.9)]);
            t.wood.push(Shape::wood(start, end, 0.07, 0.04, BARK_DARK));
            self.lobe(&mut t, end, rng.range(0.7, 1.0), 0.75, canopy, AIR, rng);
        }
        self.lobe(&mut t, add(top, [0.0, 0.0, 0.3]), rng.range(0.9, 1.2), 0.75, canopy, AIR, rng);
        t.shapes()
    }

    /// A bush: a few lumpy masses of leaves huddled together, in the colors of the trees round it,
    /// dark with red berries in the woods, or covered in flowers in the meadows. In the thick of
    /// the broad-leaved woods, `woods` from 0 to 1, mostly a bigger shrub, see shrub().
    pub(super) fn bush(&self, x: f32, y: f32, ground: f32, meadow: f32, woods: f32, r: u32) -> Object {
        let mut rng = Rng(r);
        if rng.next() < woods * 0.85 {
            let shapes = self.shrub(x, y, ground, &mut rng);
            return Object::new(shapes, rng.0);
        }
        let size = rng.range(0.45, 0.9);
        let flowering = rng.next() < meadow;
        let canopy = if flowering {
            let flower = [FLOWER_BLUE, FLOWER_PINK, FLOWER_WHITE, FLOWER_YELLOW][(rng.next() * 3.99) as usize];
            GREENS[(rng.next() * 2.99) as usize].speckled(flower, rng.range(50.0, 80.0))
        } else if rng.next() < 0.35 {
            Canopy::new(LEAVES_DARK, LEAVES, FERN).speckled(FLOWER_RED, 22.0)
        } else {
            self.canopy(x, y, &mut rng)
        };
        let mut t = Parts::default();
        for _ in 0..(2 + (rng.next() * 2.99) as usize) {
            let center = [x + rng.range(-0.7, 0.7) * size, y + rng.range(-0.7, 0.7) * size, ground + size * 0.4];
            self.lobe(&mut t, center, size * rng.range(0.6, 1.0), 1.0, canopy, AIR, &mut rng);
        }
        Object::new(t.shapes(), rng.0)
    }

    /// A shrub of the woods' undergrowth, filling the gaps between the trunks: a big mass of leaves
    /// with smaller ones round it, a little wider than high, in patches of one bright color or
    /// another where the leaves have turned, and fresh lime green elsewhere
    fn shrub(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let size = rng.range(0.65, 1.1);
        let turned = smoothstep(-0.4, 0.4, autumn(&self.settings, x, y));
        // Patches of a color, reds the most common, with others mixed in
        let patch = unit(hash3((x / 9.0).floor() as i32 as u32, (y / 9.0).floor() as i32 as u32, self.settings.seed ^ 0x5b0b));
        let pick = if rng.next() < 0.6 { patch } else { rng.next() };
        let mut canopy = if rng.next() < turned * 0.9 {
            SHRUBS[[0, 0, 0, 1, 1, 2, 2, 3, 4][(pick * 8.99) as usize]]
        } else {
            [GREENS[2], SHRUBS[4], SHRUBS[3]][(pick * 2.99) as usize]
        };
        // Red ones speckled pale, like the berries and the last of the blossom on them
        if canopy.body == LEAVES_RED && rng.next() < 0.6 {
            canopy = canopy.speckled(if rng.next() < 0.5 { FLOWER_PINK } else { FLOWER_WHITE }, rng.range(18.0, 34.0));
        }
        let mut t = Parts::default();
        let lobes = 3 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for k in 0..lobes {
            let a = spin + k as f32 * 2.4;
            let (d, r) = if k == 0 { (0.0, size * rng.range(0.85, 1.05)) } else { (size * rng.range(0.7, 1.15), size * rng.range(0.55, 0.8)) };
            self.lobe(&mut t, [x + a.cos() * d, y + a.sin() * d, ground + r * 0.5], r, 0.85, canopy, AIR, rng);
        }
        t.shapes()
    }

    /// Where the tree in the cell `cx`, `cy` of the scatter trees are placed by would stand, if
    /// there is one, in meters
    pub(super) fn tree_spot(&self, cx: i32, cy: i32) -> [f32; 2] {
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ TREE_SALT);
        [(cx as f32 + unit(r)) * TREE_CELL, (cy as f32 + unit(r >> 16)) * TREE_CELL]
    }

    /// Whether a tree could be standing within `room` meters of `x`, `y`
    pub(super) fn near_tree(&self, x: f32, y: f32, room: f32) -> bool {
        let (cx, cy) = ((x / TREE_CELL).floor() as i32, (y / TREE_CELL).floor() as i32);
        (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let p = self.tree_spot(cx + dx, cy + dy);
                (p[0] - x).hypot(p[1] - y) < room
            })
        })
    }

    /// Something lying on the forest floor: a fallen log, mossy and with mushrooms growing on it,
    /// sometimes with the roots it was torn up by, or the stump of a tree long gone
    pub(super) fn deadwood(&self, x: f32, y: f32, ground: f32, r: u32) -> Option<Object> {
        let mut rng = Rng(r);
        let mut s = Vec::new();
        if rng.next() < 0.6 {
            let length = rng.range(2.5, 5.0);
            let angle = rng.range(0.0, TAU);
            let along = [angle.cos(), angle.sin(), 0.0];
            let half = length * 0.5;
            // Keeps clear of standing trees all along it
            if [-half, 0.0, half].iter().any(|&d| self.near_tree(x + along[0] * d, y + along[1] * d, 1.6)) {
                return None;
            }
            let radius = rng.range(0.18, 0.32);
            let end = |d: f32| {
                let (px, py) = (x + along[0] * d, y + along[1] * d);
                [px, py, height_at(&self.settings, px, py).min(ground + 0.6) + radius * 0.4]
            };
            let (a, b) = (end(-half), end(half));
            let wood = if rng.next() < 0.5 { WOOD } else { DEAD_WOOD };
            s.push(Shape::capsule(a, b, radius, radius * 0.85, wood, BARK_DARK, false));
            // Moss along its top, and the odd bracket of mushrooms on its side
            s.push(Shape::capsule(add(a, [0.0, 0.0, radius * 0.45]), add(b, [0.0, 0.0, radius * 0.4]), radius * 0.75, radius * 0.6, MOSS, FERN, true));
            for _ in 0..(rng.next() * 3.5) as usize {
                let t = rng.range(0.15, 0.85);
                let at = lerp3(a, b, t);
                let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
                let out = [-along[1] * side, along[0] * side, 0.0];
                let cap = rng.range(0.08, 0.14);
                s.insert(0, Shape::leaf(add(at, add(scale(out, radius * 0.95), [0.0, 0.0, rng.range(-0.05, 0.1)])), out, [0.0, 0.0, 1.0], cap * 2.0, cap * 1.6, MUSHROOM_STEM, MUSHROOM_CAP));
            }
            // A broken-off branch or two
            for _ in 0..(rng.next() * 2.5) as usize {
                let at = lerp3(a, b, rng.range(0.2, 0.8));
                let dir = normalize([along[0] * 0.5 + rng.range(-0.6, 0.6), along[1] * 0.5 + rng.range(-0.6, 0.6), rng.range(0.4, 1.0)]);
                s.push(Shape::wood(at, add(at, scale(dir, rng.range(0.5, 1.0))), radius * 0.35, radius * 0.15, wood));
            }
            // Torn up by the roots, which stand up on end at its foot with earth still in them
            if rng.next() < 0.35 {
                let plate = add(a, scale(along, -radius * 0.6));
                s.push(Shape::leaf(add(plate, [0.0, 0.0, radius * 0.8]), [0.0, 0.0, 1.0], along, radius * 5.0, radius * 4.5, DIRT, WOOD));
            }
        } else {
            if self.near_tree(x, y, 1.4) {
                return None;
            }
            let radius = rng.range(0.25, 0.45);
            let height = rng.range(0.4, 0.9);
            let top = [x, y, ground + height];
            self.trunk(&mut s, [x, y, ground], top, radius, 0.9, (WOOD, MOSS), 4, &mut rng);
            // Its sawn or rotted top, paler, and mushrooms round its foot
            s.insert(0, Shape::leaf(add(top, [0.0, 0.0, 0.02]), [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], radius * 1.8, radius * 1.8, DEAD_WOOD, WOOD));
            if rng.next() < 0.5 {
                s.extend(self.mushrooms(x + radius * 1.4, y, ground, &mut rng));
            }
        }
        Some(Object::new(s, rng.0))
    }

    /// A boulder or two half sunk in the forest floor, or, `big`, an outcrop of great lumps of
    /// rock huddled together. Over the top of each lies a thick, lumpy blanket of moss, as leafy
    /// as the crowns, with tufts of grass growing out of it, and on outcrops ferns and a few
    /// flowers. `moss` in 255 is how far down their sides it creeps.
    pub(super) fn mossy_rock(&self, x: f32, y: f32, ground: f32, big: bool, moss: u8, r: u32) -> Object {
        let mut rng = Rng(r);
        let heading = rng.range(0.0, TAU);
        let along = [heading.cos(), heading.sin(), 0.0];
        let side = [-along[1], along[0], 0.0];
        // Where each lump is, how big along, across and up, and how far it's tipped up
        let mut lumps = Vec::new();
        let count = if big { 3 + (rng.next() * 2.99) as usize } else { 1 + (rng.next() * 1.7) as usize };
        let length = if big { rng.range(4.0, 6.5) } else { rng.range(1.0, 1.8) };
        for k in 0..count {
            let radii = if big {
                let tall = if k == count / 2 { rng.range(2.3, 3.0) } else { rng.range(1.6, 2.4) };
                [rng.range(1.3, 1.9), rng.range(1.1, 1.5), tall]
            } else {
                let s = if k == 0 { rng.range(1.0, 1.6) } else { rng.range(0.55, 0.9) };
                [s * rng.range(1.0, 1.3), s * rng.range(0.8, 1.0), s * rng.range(0.8, 1.05)]
            };
            let u = if count == 1 { 0.0 } else { (k as f32 / (count - 1) as f32 - 0.5) * length } + rng.range(-0.3, 0.3);
            let v = rng.range(-0.6, 0.6) * if big { 1.0 } else { 0.5 };
            let (px, py) = (x + along[0] * u + side[0] * v, y + along[1] * u + side[1] * v);
            // Sunk into the ground, most of it showing, on a slope by its lower side
            let g = if k == 0 && !big { ground } else { height_at(&self.settings, px, py).min(ground + 0.4).max(ground - 0.6) };
            let center = [px, py, g + radii[2] * rng.range(0.25, 0.45)];
            let turn = rng.range(-0.5, 0.5);
            let dir = [along[0] * turn.cos() - along[1] * turn.sin(), along[0] * turn.sin() + along[1] * turn.cos(), 0.0];
            lumps.push(stone(center, dir, radii, rng.range(-0.25, 0.25), moss));
        }

        let mut blanket = Vec::new();
        let mut plants = Vec::new();
        let mut tops = Vec::new();
        for lump in &lumps {
            let Some((axes, radii)) = lump.flat else { continue };
            let reach = lump.radius_a;
            // Clumps of moss on its top, sitting low on its lumpy surface and running together into
            // a thick carpet, about enough of them to cover what faces up
            let clumps = (radii[0] * radii[1] * 2.0 / 0.1).round().clamp(6.0, 56.0) as usize;
            let scale_of = (reach / 1.5).clamp(0.8, 1.15);
            for _ in 0..clumps {
                let z: f32 = rng.range(0.5, 1.0);
                let a = rng.range(0.0, TAU);
                let ring = (1.0 - z * z).sqrt();
                let local = [ring * a.cos() * radii[0], ring * a.sin() * radii[1], z * radii[2]];
                let offset = (0..3).fold([0.0; 3], |o, i| add(o, scale(axes[i], local[i])));
                let normal = normalize((0..3).fold([0.0; 3], |n, i| add(n, scale(axes[i], local[i] / (radii[i] * radii[i])))));
                if normal[2] < 0.74 - moss as f32 / 255.0 * 0.2 {
                    continue;
                }
                let point = add(lump.a, offset);
                let surface = add(point, scale(offset, rock_noise(point) * lump.rough / reach));
                let size = rng.range(0.3, 0.46) * scale_of;
                let shade = if normal[2] < 0.8 { MOSSY.shaded() } else { MOSSY };
                blanket.push(clump(sub(surface, scale(normal, size * 0.78)), size, shade));
                let top = add(surface, scale(normal, size * 0.2));
                if normal[2] > 0.75 {
                    tops.push(top);
                }
            }
        }
        // Grass growing out of the moss in tufts, and on outcrops a fern or two and flowers
        for (k, &top) in tops.iter().enumerate() {
            let roll = rng.next();
            if roll < 0.2 {
                for _ in 0..(5 + (rng.next() * 5.99) as usize) {
                    let lean = rng.direction(0.0);
                    let foot = add(top, [rng.range(-0.1, 0.1), rng.range(-0.1, 0.1), -0.08]);
                    let tip = add(foot, scale(normalize([lean[0] * 0.4, lean[1] * 0.4, 1.0]), rng.range(0.2, 0.45)));
                    plants.push(Shape::capsule(foot, tip, 0.035, 0.02, GRASS_BLADE, GRASS_TIP, false));
                }
            } else if roll < 0.26 {
                let color = [FLOWER_YELLOW, FLOWER_WHITE, FLOWER_PINK][k % 3];
                for _ in 0..3 {
                    let foot = add(top, [rng.range(-0.2, 0.2), rng.range(-0.2, 0.2), -0.05]);
                    let head = add(foot, [0.0, 0.0, rng.range(0.12, 0.25)]);
                    plants.push(Shape::sphere(head, 0.05, color, AIR, false));
                    plants.push(Shape::capsule(foot, head, 0.02, 0.02, GRASS_BLADE, AIR, false));
                }
            } else if big && roll > 0.97 {
                plants.extend(self.fern(top[0], top[1], top[2] - 0.1, &mut rng));
            }
        }
        lumps.append(&mut blanket);
        lumps.append(&mut plants);
        Object::new(lumps, rng.0)
    }
}

/// A lump of rock `radii` meters along, across and up from `center`, its long side going `along`
/// tipped up by `tilt`, lumpy all over, weathered as rock() says, with `moss` on its top
fn stone(center: Vec3, along: Vec3, radii: Vec3, tilt: f32, moss: u8) -> Shape {
    let side = normalize([-along[1], along[0], 0.0]);
    let along = normalize([along[0] * tilt.cos(), along[1] * tilt.cos(), tilt.sin()]);
    let up = cross(along, side);
    let reach = radii[0].max(radii[1]).max(radii[2]);
    let rough = (0.15 + 0.12 * reach).min(0.45);
    Shape {
        a: center,
        b: center,
        radius_a: reach,
        radius_b: reach,
        flat: Some(([along, side, up], radii)),
        boxed: None,
        material: ROCK,
        accent: TEMPLE_DARK,
        ragged: false,
        placement: Placement::Above,
        pattern: Pattern::Rock(moss),
        rough: 0.0,
        floor: f32::MIN,
        min: center.map(|v| v - reach),
        max: center.map(|v| v + reach),
    }
    .roughened(rough, center[2] - reach - 1.0)
}

/// How far a rock's surface at `p` is pushed out or in, -1 to 1 of its roughness: swellings and
/// hollows about as big as a boulder, smaller knobs over them, and here and there a crack
fn rock_noise(p: Vec3) -> f32 {
    let swell = noise3(p[0] / 1.1, p[1] / 1.1, p[2] / 0.9, 0x70c4) * 0.6;
    let knobs = noise3(p[0] / 0.35, p[1] / 0.35, p[2] / 0.3, 0x4b0b) * 0.3;
    let crack = noise3(p[0] / 0.8 + p[2] * 0.3, p[1] / 0.8, p[2] / 1.6, 0xc4ac).abs();
    (swell + knobs - (0.12 - crack).max(0.0) * 6.0).clamp(-1.0, 1.0)
}

/// Whether a rock holds `p`, `depth` being as Shape::depth() works it out: everything well inside
/// it, and out to its lumpy, cracked surface, see rock_noise()
pub(super) fn rock_holds(shape: &Shape, p: Vec3, depth: f32) -> bool {
    let margin = shape.rough / shape.radius_a.max(0.1);
    let d = depth.sqrt();
    if d < 1.0 - margin {
        return true;
    }
    if d > 1.0 + margin {
        return false;
    }
    d < 1.0 + rock_noise(p) * margin
}

/// What a voxel `p` of a rock is: weathered stone, pale on its knobs, browner and darker down in
/// its hollows, darkest in the cracks, flecked with other stone, and with a film of moss over what
/// faces up, as far down its sides as `moss` in 255 says. Deep inside it's all one stone, so
/// bricks there needn't be stored voxel by voxel. `h` is a hash of where it is.
pub(super) fn rock(shape: &Shape, moss: u8, p: Vec3, voxel: f32, h: u32) -> u8 {
    let reach = shape.radius_a.max(0.1);
    let d = shape.depth(p, voxel).sqrt();
    if (1.0 - d) * reach > shape.rough + 0.2 {
        return shape.material;
    }
    let lump = rock_noise(p);
    let normal = match &shape.flat {
        Some((axes, half)) => {
            let e = sub(p, shape.a);
            normalize((0..3).fold([0.0; 3], |n, i| add(n, scale(axes[i], dot(axes[i], e) / (half[i] * half[i]).max(1e-4)))))
        }
        None => outside(shape, p).2,
    };
    let grain = unit(h >> 8);
    let up = normal[2] + lump * 0.2 + (grain - 0.5) * 0.25;
    if moss > 0 && up > 1.0 - moss as f32 / 255.0 * 0.4 {
        return if grain < 0.55 { MOSS } else { JUNGLE_GRASS };
    }
    // Faint streaks along it, as if it had been laid down in layers long ago and worn since
    let streak = ((p[2] + lump * 0.35) * 7.0).sin() * 0.1;
    let worn = lump + streak + (grain - 0.5) * 0.15;
    if worn < -0.5 {
        DEAD_WOOD
    } else if worn < -0.05 {
        shape.accent
    } else if worn > 0.3 {
        TEMPLE_STONE
    } else if grain < 0.05 {
        STONE
    } else {
        shape.material
    }
}
