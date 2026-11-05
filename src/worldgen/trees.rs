//! Trees, and what lies about on the forest floor under them. Crowns are built of rounded masses
//! of leaves, each solid in the middle, so rays stop at its outside, and lumpy all over that with
//! smaller puffs of leaves: lighter where the sun catches them on top, darker underneath and deep
//! inside, and now and then speckled with blossom. Trunks bend a little as they go up, and flare
//! out into roots at the foot.

use super::*;

/// The leaves of a crown in three shades, deep inside, the body of it and the sunlit tips, and the
/// odd speckle of blossom or berries among them, AIR for none
#[derive(Clone, Copy)]
pub(super) struct Canopy {
    inner: u8,
    body: u8,
    tip: u8,
    speckle: u8,
}

/// Which kind of broad-leaved tree, see Terrain::leafy()
#[derive(Clone, Copy, PartialEq)]
enum Form {
    /// Short and sturdy, with limbs spreading wide and low into a broad dome
    Spreading,
    /// Taller, with limbs reaching up into an egg-shaped crown
    Upright,
    /// Crooked, dark limbs and a flat, wide crown of blossom
    Blossom,
}

/// Trees are scattered at most one in each TREE_CELL meter square, see Terrain::place_objects(),
/// at spots picked with TREE_SALT
pub(super) const TREE_CELL: f32 = 7.5;
pub(super) const TREE_SALT: u32 = 0x7a3e;

impl Terrain {
    /// Which kind of tree grows where: pines up high and in the snow, palms in the desert, giants
    /// in the jungle, willows and palms by the water, and groves of one kind or another elsewhere
    pub(super) fn tree(&self, x: f32, y: f32, base: f32, r: u32) -> Object {
        let seed = self.settings.seed;
        let mut rng = Rng(r);

        let sea_level = sea_level_m(&self.settings);
        let altitude = (base - sea_level) / (self.settings.height_m as f32 - sea_level);
        let pine_region = fbm(x / 90.0, y / 90.0, 2, seed.wrapping_add(8));
        let pines = smoothstep(0.25, 0.45, altitude + pine_region * 0.3);
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
            // Mostly grown, some young, and in the thick of the woods the odd ancient giant
            let age = rng.next();
            let size = if age < 0.14 { rng.range(0.45, 0.65) } else { rng.range(0.8, 1.2) };
            if forest > 0.3 && age > 0.965 {
                return Object::new(self.ancient(x, y, base, &mut rng), rng.0);
            }
            let choice = grove * 0.7 + rng.next() * 0.3;
            if choice < 0.3 {
                let canopy = self.canopy(x, y, &mut rng);
                self.leafy(x, y, base, size, Form::Spreading, lean, WOOD, canopy, &mut rng)
            } else if choice < 0.45 {
                self.birch(x, y, base, size, &mut rng)
            } else if choice < 0.51 {
                let canopy = Canopy { inner: FLOWER_PINK, body: BLOSSOM, tip: BLOSSOM, speckle: FLOWER_WHITE };
                self.leafy(x, y, base, size * 0.85, Form::Blossom, lean, BARK_DARK, canopy, &mut rng)
            } else if choice < 0.61 {
                let canopy = self.canopy(x, y, &mut rng);
                self.poplar(x, y, base, size, canopy, &mut rng)
            } else if choice < 0.65 {
                self.willow(x, y, base, &mut rng)
            } else {
                let canopy = self.canopy(x, y, &mut rng);
                self.leafy(x, y, base, size, Form::Upright, lean, WOOD, canopy, &mut rng)
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

    /// The shades of a crown. They come in regions, so forests turn in patches as in autumn, and
    /// now and then a tree is in flower.
    fn canopy(&self, x: f32, y: f32, rng: &mut Rng) -> Canopy {
        let autumn = fbm(x / 120.0, y / 120.0, 2, self.settings.seed.wrapping_add(6));
        let pick = (rng.next() * 0.7 + autumn * 0.8 + 0.15).clamp(0.0, 0.999);
        let mut canopy = [
            Canopy { inner: LEAVES_DARK, body: LEAVES, tip: FERN, speckle: AIR },
            Canopy { inner: LEAVES_DARK, body: LEAVES_DARK, tip: LEAVES, speckle: AIR },
            Canopy { inner: LEAVES_DARK, body: LEAVES, tip: FERN, speckle: LEAVES_YELLOW },
            Canopy { inner: LEAVES_ORANGE, body: LEAVES_YELLOW, tip: FLOWER_YELLOW, speckle: AIR },
            Canopy { inner: LEAVES_RED, body: LEAVES_ORANGE, tip: LEAVES_YELLOW, speckle: AIR },
            Canopy { inner: LEAVES_RED, body: LEAVES_RED, tip: FLOWER_RED, speckle: LEAVES_ORANGE },
        ][(pick * 6.0) as usize];
        if rng.next() < 0.12 {
            canopy.speckle = if rng.next() < 0.6 { FLOWER_WHITE } else { FLOWER_PINK };
        }
        canopy
    }

    /// A rounded mass of leaves `radius` meters round `at`: solid and dark in the middle, and lumpy
    /// all over its outside with smaller puffs of leaves, sunlit on top and shadowy underneath,
    /// some bulging out of the rest so its outline against the sky is lumpy. The middle goes in
    /// `core`, before the puffs, so it's all one material and doesn't need storing voxel by voxel.
    pub(super) fn lobe(&self, core: &mut Vec<Shape>, puffs: &mut Vec<Shape>, at: Vec3, radius: f32, canopy: Canopy, rng: &mut Rng) {
        self.flat_lobe(core, puffs, at, radius, 1.0, canopy, rng);
    }

    /// The same, but only `flat` times as high as it's wide
    #[allow(clippy::too_many_arguments)]
    fn flat_lobe(&self, core: &mut Vec<Shape>, puffs: &mut Vec<Shape>, at: Vec3, radius: f32, flat: f32, canopy: Canopy, rng: &mut Rng) {
        let inside = radius * 0.72;
        core.push(if flat < 1.0 {
            Shape::ellipsoid(at, [inside, inside, inside * flat], canopy.inner, AIR)
        } else {
            Shape::sphere(at, inside, canopy.inner, AIR, false)
        });
        let puff = (radius * 0.4).clamp(0.22, 0.7);
        let count = (5.0 + 2.0 * (radius / puff).powi(2)) as usize;
        let spin = rng.next() * TAU;
        for k in 0..count {
            // Spread evenly over it, top to bottom, then jostled about
            let z = 1.0 - 2.0 * (k as f32 + 0.5) / count as f32;
            let ring = (1.0 - z * z).sqrt();
            let a = spin + k as f32 * 2.399_963;
            let dir = normalize(add([ring * a.cos(), ring * a.sin(), z], scale(rng.direction(0.0), 0.3)));
            let size = puff * rng.range(0.7, 1.3);
            let reach = radius - size * rng.range(0.25, 0.8);
            // Flatter underneath
            let squash = (if dir[2] < 0.0 { 0.7 } else { 1.0 }) * flat;
            let center = add(at, [dir[0] * reach, dir[1] * reach, dir[2] * reach * squash]);
            let (material, accent) = if dir[2] > 0.4 {
                (canopy.tip, canopy.body)
            } else if dir[2] < -0.35 {
                (canopy.inner, canopy.body)
            } else if canopy.speckle != AIR && rng.next() < 0.45 {
                (canopy.body, canopy.speckle)
            } else {
                (canopy.body, canopy.tip)
            };
            puffs.push(Shape::sphere(center, size, material, accent, true));
            if dir[2] > -0.3 && rng.next() < 0.6 {
                let out = normalize(add(dir, scale(rng.direction(0.0), 0.7)));
                let bump = size * rng.range(0.35, 0.6);
                puffs.push(Shape::sphere(add(center, scale(out, size * 0.8)), bump, canopy.tip, canopy.body, true));
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

    /// A broad-leaved tree, `size` times as big as most: a short trunk splitting into limbs, each
    /// ending in a mass of leaves, and another over the top, so the crown is a cluster of them with
    /// clefts between
    #[allow(clippy::too_many_arguments)]
    fn leafy(&self, x: f32, y: f32, base: f32, size: f32, form: Form, lean: [f32; 2], bark: u8, canopy: Canopy, rng: &mut Rng) -> Vec<Shape> {
        let (height, spread, rise, lobe) = match form {
            Form::Spreading => (rng.range(1.6, 2.6), rng.range(1.8, 2.6), rng.range(0.35, 0.8), rng.range(1.3, 1.8)),
            Form::Upright => (rng.range(2.4, 3.4), rng.range(1.4, 2.0), rng.range(0.9, 1.5), rng.range(1.2, 1.6)),
            Form::Blossom => (rng.range(1.4, 2.0), rng.range(1.8, 2.4), rng.range(0.25, 0.6), rng.range(1.0, 1.4)),
        };
        let (height, spread, lobe) = (height * size, spread * size, lobe * size);
        let radius = rng.range(0.22, 0.32) * size;
        let bark = (bark, if bark == WOOD { DEAD_WOOD } else { AIR });
        let tilt = rng.range(0.0, 0.25);
        let tilt_angle = rng.range(0.0, TAU);
        let foot = [x, y, base];
        let top = [x + (lean[0] + tilt_angle.cos() * tilt) * height, y + (lean[1] + tilt_angle.sin() * tilt) * height, base + height];

        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        self.trunk(&mut wood, foot, top, radius, 0.7, bark, if size > 0.7 { 4 + (rng.next() * 1.99) as usize } else { 0 }, rng);

        let limbs = 3 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        let mut highest: f32 = top[2];
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.25, 0.25)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rise * rng.range(0.8, 1.2)]);
            let start = lerp3(foot, top, rng.range(0.8, 1.0));
            let end = self.limb(&mut wood, start, dir, spread * rng.range(0.85, 1.15), radius * 0.6, bark, rng);
            let r = lobe * rng.range(0.85, 1.15);
            let at = add(end, [0.0, 0.0, r * 0.3]);
            highest = highest.max(at[2]);
            self.lobe(&mut core, &mut puffs, at, r, canopy, rng);
            // Smaller ones huddled round it, so the crown's a cluster of lumps of all sizes
            for _ in 0..(1 + (rng.next() * 1.6) as usize) {
                let side = normalize([-dir[1] + dir[0] * 0.5, dir[0] + dir[1] * 0.5, rng.range(-0.3, 0.5)]);
                let s = if rng.next() < 0.5 { 1.0 } else { -1.0 };
                let off = add(at, scale([side[0] * s, side[1] * s, side[2]], r * rng.range(0.7, 1.0)));
                self.lobe(&mut core, &mut puffs, off, r * rng.range(0.5, 0.75), canopy, rng);
            }
        }
        // Over the top, where the limbs come together
        let crown = match form {
            Form::Spreading => 1,
            Form::Upright => 2,
            Form::Blossom => 1,
        };
        let mut at = top;
        for k in 0..crown {
            let r = lobe * if form == Form::Blossom { 1.0 } else { rng.range(1.05, 1.3) } * (1.0 - 0.2 * k as f32);
            at = add(at, [rng.range(-0.3, 0.3) * size, rng.range(-0.3, 0.3) * size, if k == 0 { (highest - top[2]) * 0.6 + r * 0.4 } else { r * 1.1 }]);
            wood.push(Shape::wood(top, at, radius * 0.5, radius * 0.25, bark.0));
            self.lobe(&mut core, &mut puffs, at, r, canopy, rng);
        }

        wood.append(&mut core);
        wood.append(&mut puffs);
        wood
    }

    /// Now and then in the thick of the woods, a great old tree: a massive, mossy trunk on roots
    /// sprawling out over the ground, limbs as thick as most trees, and a huge crown
    fn ancient(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let size = rng.range(1.5, 1.85);
        let height = rng.range(2.6, 3.4) * size;
        let radius = rng.range(0.32, 0.4) * size;
        let bark = (WOOD, MOSS);
        let foot = [x, y, base];
        let top = [x + rng.range(-0.4, 0.4), y + rng.range(-0.4, 0.4), base + height];
        let canopy = Canopy { inner: LEAVES_DARK, body: LEAVES_DARK, tip: LEAVES, speckle: if rng.next() < 0.5 { FERN } else { AIR } };

        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        self.trunk(&mut wood, foot, top, radius, 0.6, bark, 6, rng);
        let limbs = 5 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.3, 0.3)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.25, 0.7)]);
            let start = lerp3(foot, top, rng.range(0.65, 1.0));
            let end = self.limb(&mut wood, start, dir, rng.range(3.0, 4.2) * size * 0.75, radius * 0.5, bark, rng);
            let r = rng.range(1.5, 2.0) * size * 0.75;
            self.lobe(&mut core, &mut puffs, add(end, [0.0, 0.0, r * 0.25]), r, canopy, rng);
        }
        let r = 2.2 * size * 0.75;
        let crown = add(top, [0.0, 0.0, r * 0.9]);
        wood.push(Shape::wood(top, crown, radius * 0.5, radius * 0.3, WOOD));
        self.lobe(&mut core, &mut puffs, crown, r, canopy, rng);

        wood.append(&mut core);
        wood.append(&mut puffs);
        wood
    }

    /// Tall and slender with white bark, and small masses of light leaves close round its top half,
    /// with sky showing here and there between them
    fn birch(&self, x: f32, y: f32, base: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(6.0, 8.5) * size;
        let radius = rng.range(0.11, 0.16) * size.max(0.7);
        let top = [x + rng.range(-0.4, 0.4), y + rng.range(-0.4, 0.4), base + height];
        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        self.trunk(&mut wood, [x, y, base], top, radius * 1.2, 0.35, (BIRCH_BARK, BARK_DARK), if size > 0.7 { 3 } else { 0 }, rng);

        let autumn = fbm(x / 120.0, y / 120.0, 2, self.settings.seed.wrapping_add(6));
        let canopy = if autumn > 0.1 {
            Canopy { inner: LEAVES_ORANGE, body: LEAVES_YELLOW, tip: FLOWER_YELLOW, speckle: AIR }
        } else {
            Canopy { inner: LEAVES, body: FERN, tip: WILLOW, speckle: AIR }
        };
        let branches = 6 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        let small = size.max(0.6);
        for i in 0..branches {
            let t = 0.4 + 0.5 * (i as f32 + rng.next()) / branches as f32;
            let angle = spin + i as f32 * 2.4;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.6, 1.2)]);
            let start = lerp3([x, y, base], top, t);
            let length = rng.range(0.4, 0.8) * (1.3 - t * 0.6) * size;
            let end = add(start, scale(dir, length));
            wood.push(Shape::wood(start, end, radius * 0.4, radius * 0.2, BIRCH_BARK));
            self.lobe(&mut core, &mut puffs, end, rng.range(0.6, 0.9) * (1.15 - t * 0.4) * small, canopy, rng);
        }
        self.lobe(&mut core, &mut puffs, add(top, [0.0, 0.0, 0.2]), 0.6 * small, canopy, rng);

        wood.append(&mut core);
        wood.append(&mut puffs);
        wood
    }

    /// A narrow column of leaves, one mass on another, smaller towards the top
    fn poplar(&self, x: f32, y: f32, base: f32, size: f32, canopy: Canopy, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(7.0, 10.0) * size;
        let width = rng.range(0.9, 1.25) * size.max(0.6);
        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        let top = [x + rng.range(-0.2, 0.2), y + rng.range(-0.2, 0.2), base + height * 0.85];
        self.trunk(&mut wood, [x, y, base], top, 0.22 * size.max(0.6), 0.4, (WOOD, DEAD_WOOD), if size > 0.7 { 4 } else { 0 }, rng);
        let mut z = base + 1.6 * size + width;
        let mut r = width;
        while z < base + height - r * 0.5 {
            let t = (z - base) / height;
            let at = lerp3([x, y, base], top, t.min(1.0));
            // Each a little to one side of the last, so it's not a stack of balls
            let a = rng.range(0.0, TAU);
            let off = r * rng.range(0.15, 0.35);
            self.lobe(&mut core, &mut puffs, [at[0] + a.cos() * off, at[1] + a.sin() * off, z], r, canopy, rng);
            z += r * rng.range(0.75, 0.95);
            r *= 0.9;
        }
        wood.append(&mut core);
        wood.append(&mut puffs);
        wood
    }

    /// A thick, short trunk with limbs arching out, each ending in a mass of leaves, from which
    /// long curtains of leaves hang down nearly to the ground
    pub(super) fn willow(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(2.0, 3.0);
        let radius = rng.range(0.3, 0.42);
        let top = [x + rng.range(-0.4, 0.4), y + rng.range(-0.4, 0.4), base + height];
        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        let mut strands = Vec::new();
        self.trunk(&mut wood, [x, y, base], top, radius * 1.2, 0.75, (WOOD, DEAD_WOOD), 5, rng);
        let canopy = Canopy { inner: LEAVES, body: WILLOW, tip: FERN, speckle: AIR };

        let limbs = 4 + (rng.next() * 2.99) as usize;
        for i in 0..limbs {
            let angle = (i as f32 + rng.range(-0.3, 0.3)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.5, 0.9)]);
            let end = add(top, scale(dir, rng.range(1.6, 2.4)));
            wood.push(Shape::wood(top, end, radius * 0.5, radius * 0.25, WOOD));
            self.lobe(&mut core, &mut puffs, end, rng.range(0.8, 1.1), canopy, rng);

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
        wood.append(&mut core);
        wood.append(&mut puffs);
        wood.append(&mut strands);
        wood
    }

    /// A pine: a straight trunk tapering to a spire, with whorls of branches all the way up it,
    /// each branch a ragged bough of needles, longest at the bottom. Spruces are narrow, their
    /// boughs drooping, firs broader and darker with level boughs. In the snow, snow lies along
    /// the top of each bough.
    fn conifer(&self, x: f32, y: f32, base: f32, spruce: bool, snowy: bool, rng: &mut Rng) -> Vec<Shape> {
        let young = rng.next() < 0.15;
        let size = if young { rng.range(0.4, 0.6) } else { rng.range(0.85, 1.15) };
        let (height, crown, droop, needles) = if spruce {
            (rng.range(8.0, 13.0), rng.range(1.5, 2.0), 0.4, (PINE, LEAVES_DARK))
        } else {
            (rng.range(7.0, 11.0), rng.range(2.0, 2.7), 0.15, (LEAVES_DARK, PINE))
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
                boughs.push(Shape::capsule(root, tip, thick, 0.1, needles.0, needles.1, true));
                if snowy {
                    let up = [0.0, 0.0, thick * 0.55];
                    snow.push(Shape::capsule(add(root, up), add(tip, [0.0, 0.0, 0.08]), thick * 0.75, 0.07, SNOW, AIR, false));
                }
            }
            z += spacing * (1.0 - 0.45 * t) + rng.range(-0.1, 0.1);
        }
        // The spire
        boughs.push(Shape::capsule(top, add(top, [0.0, 0.0, 0.8 * size]), 0.16, 0.04, needles.0, needles.1, true));
        if snowy {
            snow.push(Shape::sphere(add(top, [0.0, 0.0, 0.2]), 0.14, SNOW, AIR, false));
        }
        wood.append(&mut boughs);
        wood.append(&mut snow);
        wood
    }

    /// A jungle giant: a tall trunk on buttress roots, a broad, flat-topped crown of dark leaves
    /// spread wide over everything else, now and then in flower, and vines hanging down from it
    pub(super) fn jungle_tree(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(9.0, 15.0);
        let radius = rng.range(0.35, 0.55);
        let spread = rng.range(3.0, 4.5);
        let top = [x + rng.range(-0.8, 0.8), y + rng.range(-0.8, 0.8), base + height];
        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        self.trunk(&mut wood, [x, y, base], top, radius * 1.3, 0.6, (BARK_DARK, DEAD_WOOD), 0, rng);
        // Buttress roots, tall fins of wood down the foot of the trunk
        for i in 0..5 {
            let angle = i as f32 / 5.0 * TAU + rng.range(-0.4, 0.4);
            let out = rng.range(1.4, 2.0);
            let foot = [x + angle.cos() * out, y + angle.sin() * out, base - 0.3];
            for k in 0..3 {
                let up = [x, y, base + 0.8 + k as f32 * 0.5];
                wood.push(Shape::wood(up, lerp3(foot, [x, y, base - 0.3], k as f32 * 0.15), radius * 0.45, 0.1, BARK_DARK));
            }
        }
        let canopy = Canopy {
            inner: LEAVES_DARK,
            body: LEAVES_DARK,
            tip: LEAVES,
            speckle: [FERN, FERN, FLOWER_RED, FLOWER_PINK, AIR][(rng.next() * 4.99) as usize],
        };
        let around = 5 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..=around {
            let angle = spin + i as f32 / around as f32 * TAU + rng.range(-0.3, 0.3);
            let d = if i == around { 0.0 } else { spread * rng.range(0.7, 1.0) };
            let r = if i == around { rng.range(1.8, 2.2) } else { rng.range(1.4, 1.8) };
            let at = [top[0] + angle.cos() * d, top[1] + angle.sin() * d, top[2] + rng.range(-0.3, 0.5) - d * 0.12];
            if i < around {
                wood.push(Shape::wood([top[0], top[1], top[2] - 1.5], [at[0], at[1], at[2] - 0.5], 0.22, 0.1, BARK_DARK));
            }
            // Flatter than most, lying along the top of the jungle
            self.flat_lobe(&mut core, &mut puffs, at, r, 0.6, canopy, rng);
        }
        for _ in 0..9 {
            let angle = rng.next() * TAU;
            let d = spread * rng.range(0.5, 1.2);
            let start = [top[0] + angle.cos() * d, top[1] + angle.sin() * d, top[2] - 0.6];
            let end = [start[0] + rng.range(-0.2, 0.2), start[1] + rng.range(-0.2, 0.2), start[2] - rng.range(2.5, height * 0.6)];
            wood.push(Shape::capsule(start, end, 0.07, 0.04, VINE, LEAVES_DARK, false));
        }
        wood.append(&mut core);
        wood.append(&mut puffs);
        wood
    }

    /// A tree of the dry lands at the edge of the desert: a crooked trunk forking low into a few
    /// limbs leaning out and up, under one wide, flat crown like a parasol
    fn acacia(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let size = rng.range(0.8, 1.15);
        let radius = rng.range(0.16, 0.24) * size;
        let fork = [x + rng.range(-0.3, 0.3), y + rng.range(-0.3, 0.3), base + rng.range(1.2, 2.0) * size];
        let crown = base + rng.range(3.8, 5.0) * size;
        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        self.trunk(&mut wood, [x, y, base], fork, radius, 0.8, (WOOD, DEAD_WOOD), 3, rng);
        let canopy = Canopy { inner: LEAVES_DARK, body: LEAVES, tip: WILLOW, speckle: if rng.next() < 0.25 { FLOWER_YELLOW } else { AIR } };

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
            wood.push(Shape::wood(fork, elbow, radius * 0.75, radius * 0.55, WOOD));
            wood.push(Shape::wood(elbow, end, radius * 0.55, radius * 0.3, WOOD));
            ends.push(end);
        }
        // A wide, flat mass of leaves over each limb, and more filling out the parasol between them
        let middle = scale(ends.iter().fold([0.0; 3], |a, &e| add(a, e)), 1.0 / ends.len() as f32);
        for &end in &ends {
            self.flat_lobe(&mut core, &mut puffs, add(end, [0.0, 0.0, 0.3]), rng.range(1.4, 1.9) * size, 0.45, canopy, rng);
        }
        self.flat_lobe(&mut core, &mut puffs, add(middle, [rng.range(-0.5, 0.5), rng.range(-0.5, 0.5), 0.45]), rng.range(1.3, 1.7) * size, 0.45, canopy, rng);

        wood.append(&mut core);
        wood.append(&mut puffs);
        wood
    }

    /// A smaller tree of the jungle, growing up in the shade between the giants: a slim, crooked
    /// trunk and a few masses of glossy leaves
    pub(super) fn jungle_sapling(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let canopy = Canopy { inner: LEAVES_DARK, body: LEAVES_DARK, tip: LEAVES, speckle: if rng.next() < 0.3 { FERN } else { AIR } };
        let height = rng.range(2.5, 4.0);
        let top = [x + rng.range(-0.5, 0.5), y + rng.range(-0.5, 0.5), base + height];
        let mut wood = Vec::new();
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        self.trunk(&mut wood, [x, y, base], top, rng.range(0.1, 0.15), 0.6, (BARK_DARK, AIR), 0, rng);
        let spin = rng.range(0.0, TAU);
        let limbs = 2 + (rng.next() * 1.99) as usize;
        for i in 0..limbs {
            let angle = spin + i as f32 / limbs as f32 * TAU;
            let start = lerp3([x, y, base], top, rng.range(0.6, 0.85));
            let end = add(start, [angle.cos() * rng.range(0.7, 1.1), angle.sin() * rng.range(0.7, 1.1), rng.range(0.4, 0.9)]);
            wood.push(Shape::wood(start, end, 0.07, 0.04, BARK_DARK));
            self.flat_lobe(&mut core, &mut puffs, end, rng.range(0.7, 1.0), 0.75, canopy, rng);
        }
        self.flat_lobe(&mut core, &mut puffs, add(top, [0.0, 0.0, 0.3]), rng.range(0.9, 1.2), 0.75, canopy, rng);
        wood.append(&mut core);
        wood.append(&mut puffs);
        wood
    }

    /// A bush: a few lumpy masses of leaves huddled together, in the colors of the trees round it,
    /// dark with red berries in the woods, or covered in flowers in the meadows
    pub(super) fn bush(&self, x: f32, y: f32, ground: f32, meadow: f32, r: u32) -> Object {
        let mut rng = Rng(r);
        let size = rng.range(0.45, 0.9);
        let flowering = rng.next() < meadow;
        let mut canopy = if flowering || rng.next() < 0.35 {
            Canopy { inner: LEAVES_DARK, body: LEAVES_DARK, tip: LEAVES, speckle: FLOWER_RED }
        } else {
            self.canopy(x, y, &mut rng)
        };
        if flowering {
            canopy.speckle = [FLOWER_BLUE, FLOWER_PINK, FLOWER_WHITE][(rng.next() * 2.99) as usize];
            canopy.tip = canopy.speckle;
        }
        let (mut core, mut puffs) = (Vec::new(), Vec::new());
        for _ in 0..(2 + (rng.next() * 2.99) as usize) {
            let center = [x + rng.range(-0.7, 0.7) * size, y + rng.range(-0.7, 0.7) * size, ground + size * 0.4];
            self.lobe(&mut core, &mut puffs, center, size * rng.range(0.6, 1.0), canopy, &mut rng);
        }
        core.append(&mut puffs);
        Object::new(core, rng.0)
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
}
