//! Gathering and building as the player does them, see bases.rs and gather.rs: G gathers what's in
//! front of them, B goes into building, where a click builds the piece picked where the outline
//! shows, a right click takes down something built and gives it back, R turns it and the wheel or
//! the number keys pick another. E opens and shuts a door. What changes goes into the world as it
//! is and to the GPU with the next frame; the strips that come in later are built with it, see
//! stream.rs.

use egui_winit_vulkano::egui::{Align2, Color32, FontId, Pos2, Stroke, pos2, vec2};

use crate::bases::{self, BLOCK, Bases, Kind, Piece};
use crate::gather::{self, Yield};

use super::*;

/// How far from the eye things can be built and gathered, in meters
const REACH_M: f64 = 5.0;
/// Seconds between blows while G's held
const BLOW_SECONDS: f64 = 0.4;
/// Blows it takes to fell a tree
const TREE_BLOWS: u32 = 4;
/// How much a blast a meter round does to what's built right where it is
const BLAST_DAMAGE: f64 = 70.0;

/// What's being gathered
struct Gathering {
    /// The block of the whole landscape struck, or for a tree its column, z left 0
    key: [i64; 3],
    kind: Yield,
    blows: u32,
    wanted: u32,
}

/// The state of building and gathering that isn't saved
#[derive(Default)]
pub struct Building {
    /// In building, with B
    pub on: bool,
    /// Which of Kind::ALL is picked
    pub picked: usize,
    /// Quarter turns on from how it'd face, with R
    pub turn: u8,
    gathering: Option<Gathering>,
    /// Seconds till the next blow can be struck
    wait: f64,
    /// Seconds since the last blow, for the HUD's ring
    since_blow: f64,
    /// How far through Bases::version the world as it is has been brought
    pub applied: u64,
    /// Boxes round bricks just built, in bricks of the whole landscape, whose empty bricks round them
    /// are made to skip no further than them on the GPU with the next frame, and again once the
    /// skips worked out again where strips join are done, as those keep what they found before
    reskip: Vec<([i64; 3], [i64; 3])>,
    redo: Vec<([i64; 3], [i64; 3])>,
    set: Option<(Arc<ImageView>, Arc<DescriptorSet>)>,
    /// For screenshot scripts: the corner of the hut built, see App::bases_script(), how far
    /// through it is, and since which frame
    hut: Option<[i64; 3]>,
    phase: u8,
    mark: usize,
}

/// Has to match the push constants in shaders\builds.comp
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct ReskipConstants {
    lo: [i32; 4],
    hi: [i32; 4],
    built_lo: [i32; 4],
    built_hi: [i32; 4],
    wrap: [i32; 4],
}

/// What a piece would be built as where the player's looking, and whether it can be
pub struct Ghost {
    pub piece: Piece,
    pub ok: Result<(), String>,
}

/// Quarter turns round from +x of the way `look` goes, flat
fn snapped(look: Vector3<f64>) -> u8 {
    let a = look.y.atan2(look.x);
    ((a / FRAC_PI_2).round() as i64).rem_euclid(4) as u8
}

impl App {
    /// The eye and which way it looks, in voxels of the world
    fn eye_look(&self) -> (Vector3<f64>, Vector3<f64>) {
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let look = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
        (eye, look)
    }

    /// The block of the whole landscape voxel `v` of the world is in
    fn block_of(&self, v: [i64; 3]) -> [i64; 3] {
        [(v[0] + self.world.origin[0]).div_euclid(BLOCK), (v[1] + self.world.origin[1]).div_euclid(BLOCK), (v[2] + self.world.origin[2]).div_euclid(BLOCK)]
    }

    /// Whether voxel `v` of the world is of something built
    fn is_built(&self, v: [i64; 3]) -> bool {
        self.bases.piece_at(self.block_of(v)).is_some()
    }

    /// Reads the keys for gathering and building, called each frame while playing. Returns whether
    /// the mouse's buttons went to building, so they don't swing what's in hand too.
    pub(super) fn update_building(&mut self, dt: f64) -> bool {
        self.building.wait -= dt;
        self.building.since_blow += dt;
        if self.input.key_pressed(KeyCode::KeyB) {
            self.building.on = !self.building.on;
            self.game.say(if self.building.on { "Building: click to build, right click to take down, R to turn, wheel to pick." } else { "Stopped building." });
        }
        // Holding G strikes again and again
        if self.input.key_held(KeyCode::KeyG) && self.building.wait <= 0.0 {
            self.building.wait = BLOW_SECONDS;
            self.gather_blow();
        }
        if !self.building.on {
            return false;
        }
        let kinds = Kind::ALL.len();
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8];
        if let Some(k) = digits.iter().position(|&k| self.input.key_pressed(k)) {
            self.building.picked = k.min(kinds - 1);
        }
        let wheel = self.input.scroll_diff().1;
        if wheel != 0.0 {
            self.building.picked = (self.building.picked as i64 + if wheel < 0.0 { 1 } else { -1 }).rem_euclid(kinds as i64) as usize;
        }
        if self.input.key_pressed(KeyCode::KeyR) {
            self.building.turn = (self.building.turn + 1) % 4;
        }
        if self.input.mouse_pressed(MouseButton::Left) {
            self.build_here();
        }
        if self.input.mouse_pressed(MouseButton::Right) {
            self.take_down_here();
        }
        true
    }

    /// The piece picked, put where the player's looking, and whether it can go there
    pub(super) fn ghost(&self) -> Option<Ghost> {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let (hit, normal) = gather::raycast(&self.world, eye, look, REACH_M * vpm)?;
        let front = [hit[0] + normal[0], hit[1] + normal[1], hit[2] + normal[2]];
        let kind = Kind::ALL[self.building.picked];
        // A door faces the player and spans to their right; stairs go up away from them
        let facing = snapped(look);
        let auto = match kind {
            Kind::Door => facing + 2,
            Kind::Stairs => facing,
            _ => 0,
        };
        let piece = Piece::new(0, kind, self.block_of(front), auto + self.building.turn);
        let ok = self.can_build(&piece);
        Some(Ghost { piece, ok })
    }

    /// Whether `piece` can be built: nothing built there, room for it, something under it or beside
    /// it to hold it up, the player not in the way, and enough to build it with
    fn can_build(&self, piece: &Piece) -> Result<(), String> {
        if !self.bases.free_for(piece) {
            return Err("Something's built there.".into());
        }
        let mut held = false;
        for b in piece.blocks() {
            let local = bases::local_block(&self.world, b).ok_or("That's too far out.")?;
            let materials = bases::read_brick(&self.world, local);
            let solid = materials.iter().filter(|&&m| !worldgen::is_passable(m)).count();
            if solid > 256 {
                return Err("There's no room there.".into());
            }
            held |= solid > 0;
            // Standing on what's under it
            if b[2] == piece.at[2] && local[2] > 0 {
                let below = bases::read_brick(&self.world, [local[0], local[1], local[2] - 1]);
                held |= below[448..].iter().any(|&m| !worldgen::is_passable(m));
            }
            // Or built onto something beside it
            for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                held |= self.bases.piece_at([b[0] + d[0], b[1] + d[1], b[2] + d[2]]).is_some();
            }
        }
        // Not where the player stands
        if let Some(player) = &self.player {
            let vpm = self.world.voxels_per_meter as f64;
            let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
            let feet = player.position + origin;
            let half = 0.3 * vpm;
            let (lo, hi) = (feet - Vector3::new(half, half, 0.0), feet + Vector3::new(half, half, 1.75 * vpm));
            for b in piece.blocks() {
                let (blo, bhi) = (b.map(|v| (v * BLOCK) as f64), b.map(|v| ((v + 1) * BLOCK) as f64));
                if (0..3).all(|i| lo[i] < bhi[i] && hi[i] > blo[i]) {
                    return Err("You're in the way.".into());
                }
            }
        }
        if !held {
            return Err("It needs something under it or beside it.".into());
        }
        let cost = piece.kind.cost();
        if !self.bases.materials.covers(cost) {
            return Err(format!("Needs {}.", cost.describe()));
        }
        Ok(())
    }

    /// Builds the piece picked where the outline shows, if it can be
    fn build_here(&mut self) {
        let Some(ghost) = self.ghost() else {
            return;
        };
        if let Err(why) = ghost.ok {
            self.game.say(why);
            return;
        }
        let Piece { kind, at, turn, .. } = ghost.piece;
        if self.bases.build(kind, at, turn).is_ok() {
            self.refresh_bases();
            let middle = self.bases.piece_at(at).map(|p| p.middle());
            self.play_at(audio::Sound::Place, middle, 0.8, if kind == Kind::StoneWall || kind == Kind::ClayWall { 0.8 } else { 1.0 });
            if kind == Kind::Door {
                self.save_game(false);
            }
        }
    }

    /// Takes down what's built where the player's looking, giving back what it's worth
    fn take_down_here(&mut self) {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let Some((hit, _)) = gather::raycast_by(&self.world, eye, look, REACH_M * vpm, |m| m != worldgen::material::AIR) else {
            return;
        };
        let block = self.block_of(hit);
        let Some(id) = self.bases.piece_at(block).map(|p| p.id) else {
            self.game.say("Only what you've built can be taken down.");
            return;
        };
        if let Some((piece, _)) = self.bases.remove(id, true) {
            self.refresh_bases();
            let back = piece.refund();
            self.game.say(format!("{} taken down: {} back.", piece.kind.name(), back.describe()));
            self.debris_at(piece.middle(), piece.kind.material(), 6);
            self.play_at(audio::Sound::Place, Some(piece.middle()), 0.7, 0.75);
        }
    }

    /// Opens or shuts a door the player's looking at, if there's one in reach
    pub(super) fn door_here(&mut self) -> bool {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let mut t = 0.0;
        while t < REACH_M * vpm {
            let p = eye + look * t;
            let v = [p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64];
            let block = self.block_of(v);
            if let Some(door) = self.bases.piece_at(block).filter(|p| p.kind == Kind::Door).map(|p| p.id) {
                if let Some((open, _)) = self.bases.toggle_door(door) {
                    self.refresh_bases();
                    let middle = self.bases.piece(door).map(|p| p.middle());
                    self.play_at(if open { audio::Sound::DoorOpen } else { audio::Sound::DoorClose }, middle, 0.8, 1.0);
                }
                return true;
            }
            // Stopping at anything solid that isn't of a door
            if !worldgen::is_passable(self.world.material_at(v[0], v[1], v[2])) {
                return false;
            }
            t += 0.5;
        }
        false
    }

    /// Mends what's built where the player's looking, if it's damaged, for half what it cost
    pub(super) fn mend_here(&mut self) {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let Some((hit, _)) = gather::raycast_by(&self.world, eye, look, REACH_M * vpm, |m| m != worldgen::material::AIR) else {
            return;
        };
        let Some(id) = self.bases.piece_at(self.block_of(hit)).map(|p| p.id) else {
            return;
        };
        let cost = self.bases.repair_cost(id);
        match self.bases.repair(id) {
            Ok(_) => {
                self.refresh_bases();
                self.game.say(format!("Mended, for {}.", cost.map_or("nothing".into(), |c| c.describe())));
                let middle = self.bases.piece(id).map(|p| p.middle());
                self.play_at(audio::Sound::Place, middle, 0.8, 1.1);
            }
            Err(why) => self.game.say(why),
        }
    }

    /// Blasts and splashes, `struck`, each where in voxels of the world, how far round in meters,
    /// and whether it was a blast: what's built is damaged by a blast, the nearer the worse, doors
    /// and lights far more than walls, rather than having holes blown in it or being stained, so
    /// what's left of it is put back as it was
    pub(super) fn blasted(&mut self, struck: &[(Vector3<f64>, f64, bool)]) {
        if struck.is_empty() || self.bases.pieces().is_empty() {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let mut touched: Vec<[i64; 3]> = Vec::new();
        for &(center, radius, blast) in struck {
            let at = center + origin;
            let reach = (radius + 0.6) * vpm;
            let near: Vec<(u32, f64, bool)> = self
                .bases
                .pieces()
                .iter()
                .filter_map(|p| {
                    let d = (Vector3::from(p.middle()) - at).norm();
                    (d < reach).then(|| (p.id, 1.0 - d / reach, p.kind.weak()))
                })
                .collect();
            for (id, near, weak) in near {
                if let Some(piece) = self.bases.piece(id) {
                    touched.extend(piece.blocks());
                }
                if blast {
                    let amount = BLAST_DAMAGE * radius * near * if weak { 1.0 } else { 0.4 };
                    if let Some((true, _, _)) = self.bases.damage(id, amount) {
                        self.game.say("Something you built was blown apart.");
                    }
                }
            }
        }
        // What's changed goes in as anything built does; the rest of what was hit put back
        self.refresh_bases();
        touched.sort_unstable();
        touched.dedup();
        for block in touched {
            if self.bases.piece_at(block).is_none() {
                continue;
            }
            let Some(local) = bases::local_block(&self.world, block) else { continue };
            let mut materials = bases::read_brick(&self.world, local);
            self.bases.apply_to(block, &mut materials);
            let cell = self.world.set_brick(local, &materials);
            self.edited.push(cell);
        }
    }

    /// One blow at what the player's looking at, gathering it once there's been enough of them
    fn gather_blow(&mut self) {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let Some((hit, normal)) = gather::raycast(&self.world, eye, look, REACH_M * vpm) else {
            self.building.gathering = None;
            return;
        };
        if self.is_built(hit) {
            self.game.say("That's built: take it down while building (B).");
            return;
        }
        let material = self.world.material_at(hit[0], hit[1], hit[2]);
        let kind = gather::yield_of(material);
        if kind == Yield::Nothing {
            self.game.say("There's nothing to gather there.");
            return;
        }
        let block = self.block_of(hit);
        let tree = kind == Yield::Wood && gather::is_trunk(material);
        // A tree's struck anywhere up its trunk
        let key = if tree { [block[0], block[1], 0] } else { block };
        let wanted = if tree { TREE_BLOWS } else { gather::blows_for(kind) };
        let same = self.building.gathering.as_ref().is_some_and(|g| g.key == key && g.kind == kind);
        if !same {
            self.building.gathering = Some(Gathering { key, kind, blows: 0, wanted });
        }
        let gathering = self.building.gathering.as_mut().unwrap();
        gathering.blows += 1;
        let done = gathering.blows >= gathering.wanted;
        self.building.since_blow = 0.0;
        // Chips flying off where it's struck
        let at = Vector3::new(hit[0] as f64 + 0.5 + normal[0] as f64, hit[1] as f64 + 0.5 + normal[1] as f64, hit[2] as f64 + 0.5 + normal[2] as f64);
        self.game.debris(at, &[material; 3], vpm);
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let sound = match kind {
            Yield::Wood => audio::Sound::Chop,
            Yield::Stone => audio::Sound::Mine,
            _ => audio::Sound::Dig,
        };
        self.play_at(sound, Some((at + origin).into()), 0.8, 1.0);
        if !done {
            return;
        }
        self.building.gathering = None;
        let built = |v: [i64; 3]| self.bases.piece_at([(v[0] + self.world.origin[0]).div_euclid(BLOCK), (v[1] + self.world.origin[1]).div_euclid(BLOCK), v[2].div_euclid(BLOCK)]).is_some();
        let started = Instant::now();
        if tree && let Some(felled) = gather::tree_at(&self.world, hit, &built) {
            self.slow("felling a tree", started);
            self.bases.materials.wood += felled.wood;
            self.cut(&felled.cut);
            self.game.say(format!("Timber! +{} wood", felled.wood));
            // It comes apart all the way up, its leaves flying
            let steps = 6;
            for k in 0..steps {
                let up = felled.height * (k as f64 + 0.5) / steps as f64;
                let at = felled.foot + Vector3::new(0.0, 0.0, up);
                let leafy = felled.leaf.filter(|_| k >= steps / 2);
                let materials: Vec<u8> = (0..6).map(|i| if i % 2 == 0 { felled.bark } else { leafy.unwrap_or(felled.bark) }).collect();
                self.game.debris(at, &materials, vpm);
            }
            self.play_at(audio::Sound::Timber, Some((felled.foot + origin + Vector3::new(0.0, 0.0, felled.height * 0.3)).into()), 1.0, 1.0);
            return;
        }
        let local = hit.map(|v| v.div_euclid(BLOCK));
        let Some(broken) = gather::block_at(&self.world, local, &built) else {
            return;
        };
        self.bases.materials.add(broken.gives);
        self.cut(&broken.cut);
        self.game.say(if broken.gives.is_empty() { "Dug out.".to_string() } else { format!("+{}", broken.gives.describe()) });
        self.debris_at((at + origin).into(), broken.material, 5);
    }

    /// Takes what's been gathered, `cut`, out of the world for good
    fn cut(&mut self, cut: &gather::Cut) {
        let corner = [self.world.origin[0].div_euclid(BLOCK), self.world.origin[1].div_euclid(BLOCK), 0];
        for (b, voxels) in cut {
            self.bases.clear_voxels([b[0] + corner[0], b[1] + corner[1], b[2] + corner[2]], voxels);
        }
        self.refresh_bases();
    }

    /// Bits of `material` flying off round `at`, in voxels of the whole landscape
    fn debris_at(&mut self, at: [f64; 3], material: u8, count: usize) {
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let vpm = self.world.voxels_per_meter as f64;
        self.game.debris(Vector3::from(at) - origin, &vec![material; count], vpm);
    }

    /// Plays `sound` at `at`, in voxels of the whole landscape, or right by the player
    fn play_at(&mut self, sound: audio::Sound, at: Option<[f64; 3]>, volume: f32, pitch: f32) {
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let view = self.view();
        let cue = match at {
            Some(at) => audio::Cue::at(sound, Vector3::from(at) - origin, volume),
            None => audio::Cue::here(sound, volume),
        };
        if let Some(audio) = &mut self.audio {
            audio.play(cue.pitched(pitch), &view);
        }
    }

    /// Brings the world as it is up to date with what's been built and dug since last time, and
    /// sends it to the GPU with the next frame
    pub(super) fn refresh_bases(&mut self) {
        let since = self.building.applied;
        if self.bases.version == since {
            return;
        }
        self.building.applied = self.bases.version;
        let edits = self.bases.edits();
        let (cells, filled) = bases::apply_edits(&mut self.world, &edits, Some(since), None);
        // Water flows into what's dug out
        let mut ground: Option<([i64; 2], [i64; 2])> = None;
        for edit in edits.iter().filter(|e| e.version > since) {
            let Some(b) = bases::local_block(&self.world, edit.at) else {
                continue;
            };
            if self.lower_ground(b) {
                let (lo, hi) = ([b[0] * BLOCK, b[1] * BLOCK], [(b[0] + 1) * BLOCK, (b[1] + 1) * BLOCK]);
                ground = Some(ground.map_or((lo, hi), |(a, z)| ([a[0].min(lo[0]), a[1].min(lo[1])], [z[0].max(hi[0]), z[1].max(hi[1])])));
            }
        }
        self.built_into(&filled);
        if let Some(mut stream) = self.stream.take() {
            self.send_edits(&mut stream, cells, ground);
            self.stream = Some(stream);
        }
    }

    /// Bricks of the world, counted from its first, that aren't empty any more: the empty bricks round
    /// them are made to skip no further than them on the GPU with the next frame
    pub(super) fn built_into(&mut self, filled: &[[i64; 3]]) {
        let corner = [self.world.origin[0].div_euclid(BLOCK), self.world.origin[1].div_euclid(BLOCK), 0];
        for (lo, hi) in bases::groups(filled) {
            self.building.reskip.push(([lo[0] + corner[0], lo[1] + corner[1], lo[2]], [hi[0] + corner[0], hi[1] + corner[1], hi[2]]));
        }
    }

    /// Where the water finds the ground in the columns of brick `b`, lowered to what's left of it.
    /// Whether any changed.
    fn lower_ground(&mut self, b: [i64; 3]) -> bool {
        let Some(map) = &self.world.water else {
            return false;
        };
        let mut lowered = Vec::new();
        for y in b[1] * BLOCK..(b[1] + 1) * BLOCK {
            for x in b[0] * BLOCK..(b[0] + 1) * BLOCK {
                let i = self.world.column(x, y);
                let old = map.ground[i] as i64;
                if old < b[2] * BLOCK || old > (b[2] + 1) * BLOCK + 1 {
                    continue;
                }
                let top = (0..old.min(self.world.size[2] as i64)).rev().find(|&z| !worldgen::is_passable(self.world.material_at(x, y, z))).map_or(0, |z| z + 1);
                if top < old {
                    lowered.push((i, top as f32));
                }
            }
        }
        let map = self.world.water.as_mut().unwrap();
        for &(i, top) in &lowered {
            map.ground[i] = top;
        }
        !lowered.is_empty()
    }
}

/// Records with the frame the empty bricks round what's just been built being made to skip no
/// further than it, see shaders\builds.comp, on the main world's grid `grid`: those of the main
/// world `world`. `busy` says whether the skips are being worked out again where strips join, which
/// would put back what they found before: what's done now is done again once they're not.
#[allow(clippy::too_many_arguments)]
pub fn record_reskips(
    building: &mut Building,
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    pipeline: &Arc<ComputePipeline>,
    grid: &Arc<ImageView>,
    allocator: &Arc<StandardDescriptorSetAllocator>,
    world: &World,
    busy: bool,
) {
    let mut boxes = std::mem::take(&mut building.reskip);
    if busy {
        building.redo.extend(boxes.iter().copied());
        building.redo = merged(std::mem::take(&mut building.redo));
    } else {
        boxes.append(&mut building.redo);
    }
    if boxes.is_empty() {
        return;
    }
    // Many built at once, as a whole base coming in, are done together
    let boxes = merged(boxes);
    let set = match &building.set {
        Some((view, set)) if Arc::ptr_eq(view, grid) => set.clone(),
        _ => {
            let layout = pipeline.layout().set_layouts()[0].clone();
            let set = DescriptorSet::new(allocator.clone(), layout, [WriteDescriptorSet::image_view(0, grid.clone())], []).unwrap();
            building.set = Some((grid.clone(), set.clone()));
            set
        }
    };
    let corner = [world.origin[0].div_euclid(BLOCK), world.origin[1].div_euclid(BLOCK)];
    let size = world.size.map(|n| n as i64 / BLOCK);
    // No empty brick skips further than the world's high, as there's ground under every one
    let reach = size[2];
    builder.bind_pipeline_compute(pipeline.clone()).unwrap().bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, vec![set]).unwrap();
    for (lo, hi) in boxes {
        let built_lo = [lo[0] - corner[0], lo[1] - corner[1], lo[2]];
        let built_hi = [hi[0] - corner[0], hi[1] - corner[1], hi[2]];
        let from = [0, 1, 2].map(|i| (built_lo[i] - reach).clamp(0, size[i]));
        let to = [0, 1, 2].map(|i| (built_hi[i] + reach).clamp(0, size[i]));
        if (0..3).any(|i| to[i] <= from[i]) {
            continue;
        }
        let constants = ReskipConstants {
            lo: [from[0] as i32, from[1] as i32, from[2] as i32, 0],
            hi: [to[0] as i32, to[1] as i32, to[2] as i32, 0],
            built_lo: [built_lo[0] as i32, built_lo[1] as i32, built_lo[2] as i32, 0],
            built_hi: [built_hi[0] as i32, built_hi[1] as i32, built_hi[2] as i32, 0],
            wrap: [(world.wrap[0] / 8) as i32, (world.wrap[1] / 8) as i32, 0, 0],
        };
        builder.push_constants(pipeline.layout().clone(), 0, constants).unwrap();
        unsafe {
            builder.dispatch([((to[0] - from[0]) as u32).div_ceil(8), ((to[1] - from[1]) as u32).div_ceil(8), 1]).unwrap();
        }
    }
}

/// Boxes of bricks, from the first to one past the last, those near one another put together into
/// one round them all
fn merged(boxes: Vec<([i64; 3], [i64; 3])>) -> Vec<([i64; 3], [i64; 3])> {
    const NEAR: i64 = 16;
    let mut out: Vec<([i64; 3], [i64; 3])> = Vec::new();
    for b in boxes {
        let mut b = b;
        // Taken in with every box it's near, and they with it, till none is
        loop {
            let near = out.iter().position(|o| (0..3).all(|i| b.0[i] <= o.1[i] + NEAR && o.0[i] <= b.1[i] + NEAR));
            let Some(k) = near else { break };
            let o = out.swap_remove(k);
            b = ([0, 1, 2].map(|i| b.0[i].min(o.0[i])), [0, 1, 2].map(|i| b.1[i].max(o.1[i])));
        }
        out.push(b);
    }
    out
}

/// The lights of what's built, in voxels of `world`, as App::lights has them
pub fn base_lights(bases: &Bases, world: &World) -> Vec<[f32; 8]> {
    let vpm = world.voxels_per_meter as f64;
    let origin = [world.origin[0] as f64, world.origin[1] as f64, 0.0];
    bases
        .lights(vpm)
        .into_iter()
        .map(|(at, reach, color)| [(at[0] - origin[0]) as f32, (at[1] - origin[1]) as f32, (at[2] - origin[2]) as f32, reach, color[0], color[1], color[2], 0.0])
        .collect()
}

/// What's carried to build with, under the clock; in building, the pieces to pick from and the
/// outline of `ghost`, where the one picked would go; the blows struck at what's being gathered
pub fn hud(building: &Building, bases: &Bases, ghost: Option<&Ghost>, world: &World, ctx: &egui::Context, view: &combat::View) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("building")));
    let screen = ctx.screen_rect();
    // What's carried
    let m = bases.materials;
    let items = [("Wood", m.wood, Color32::from_rgb(150, 100, 55)), ("Stone", m.stone, Color32::from_rgb(150, 150, 145)), ("Clay", m.clay, Color32::from_rgb(190, 110, 70))];
    let top = screen.top() + 150.0;
    let right = screen.right() - 16.0;
    painter.rect_filled(egui::Rect::from_min_size(pos2(right - 100.0, top - 14.0), vec2(108.0, 66.0)), 8.0, Color32::from_black_alpha(110));
    for (k, (name, count, color)) in items.iter().enumerate() {
        let y = top + k as f32 * 19.0;
        painter.rect_filled(egui::Rect::from_min_size(pos2(right - 92.0, y - 6.0), vec2(12.0, 12.0)), 2.0, *color);
        painter.text(pos2(right - 74.0, y), Align2::LEFT_CENTER, *name, FontId::proportional(13.0), Color32::from_gray(220));
        painter.text(pos2(right, y), Align2::RIGHT_CENTER, count.to_string(), FontId::proportional(14.0), Color32::WHITE);
    }
    // The blows struck at what's being gathered, round the middle of the screen
    if let Some(g) = &building.gathering
        && building.since_blow < 2.0
    {
        let c = screen.center();
        let fraction = g.blows as f32 / g.wanted.max(1) as f32;
        let points: Vec<Pos2> = (0..=40).map(|k| k as f32 / 40.0 * fraction * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2).map(|a| c + vec2(a.cos(), a.sin()) * 16.0).collect();
        painter.circle_stroke(c, 16.0, Stroke::new(3.0f32, Color32::from_black_alpha(100)));
        painter.add(egui::Shape::line(points, Stroke::new(3.0f32, Color32::from_rgb(255, 210, 120))));
    }
    if !building.on {
        return;
    }
    // The pieces, down the right side
    let x = screen.right() - 236.0;
    let mut y = screen.center().y - 110.0;
    painter.rect_filled(egui::Rect::from_min_size(pos2(x - 10.0, y - 30.0), vec2(230.0, 34.0 + Kind::ALL.len() as f32 * 26.0 + 58.0)), 10.0, Color32::from_black_alpha(140));
    painter.text(pos2(x, y - 14.0), Align2::LEFT_CENTER, "BUILDING  (B)", FontId::proportional(15.0), Color32::from_rgb(255, 215, 130));
    for (k, kind) in Kind::ALL.iter().enumerate() {
        let picked = k == building.picked;
        let cost = kind.cost();
        let afford = bases.materials.covers(cost);
        if picked {
            painter.rect_filled(egui::Rect::from_min_size(pos2(x - 6.0, y + 2.0), vec2(222.0, 24.0)), 6.0, Color32::from_white_alpha(28));
        }
        let color = if afford { Color32::from_gray(if picked { 255 } else { 205 }) } else { Color32::from_rgb(200, 110, 100) };
        painter.text(pos2(x, y + 14.0), Align2::LEFT_CENTER, format!("{} {}", k + 1, kind.name()), FontId::proportional(14.0), color);
        painter.text(pos2(x + 210.0, y + 14.0), Align2::RIGHT_CENTER, cost.describe(), FontId::proportional(12.0), color.gamma_multiply(0.85));
        y += 26.0;
    }
    painter.text(pos2(x, y + 16.0), Align2::LEFT_CENTER, "Click build, right click take down", FontId::proportional(12.0), Color32::from_gray(185));
    painter.text(pos2(x, y + 34.0), Align2::LEFT_CENTER, "R turn, wheel pick, G gather, E door", FontId::proportional(12.0), Color32::from_gray(185));

    // Where it'd go: a box round it, green if it can, red if not, and why not
    let Some(ghost) = ghost else {
        return;
    };
    let origin = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
    let blocks = ghost.piece.blocks();
    let min = |i: usize| blocks.iter().map(|b| b[i]).min().unwrap_or(0);
    let max = |i: usize| blocks.iter().map(|b| b[i]).max().unwrap_or(0) + 1;
    let lo = Vector3::new(min(0), min(1), min(2)).cast::<f64>() * BLOCK as f64 - origin;
    let hi = Vector3::new(max(0), max(1), max(2)).cast::<f64>() * BLOCK as f64 - origin;
    let lo = if ghost.piece.kind == Kind::Floor { Vector3::new(lo.x, lo.y, lo.z + 4.0) } else { lo };
    let corner = |k: usize| Vector3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z });
    let on_screen = |p: Vector3<f64>| view.project(p).map(|[u, v]| pos2(screen.left() + u as f32 * screen.width(), screen.top() + v as f32 * screen.height()));
    let corners: Vec<Option<Pos2>> = (0..8).map(|k| on_screen(corner(k))).collect();
    let color = if ghost.ok.is_ok() { Color32::from_rgb(110, 240, 140) } else { Color32::from_rgb(250, 90, 80) };
    for face in [[0, 1, 3, 2], [4, 5, 7, 6], [0, 1, 5, 4], [2, 3, 7, 6], [0, 2, 6, 4], [1, 3, 7, 5]] {
        let points: Option<Vec<Pos2>> = face.iter().map(|&k| corners[k]).collect();
        if let Some(points) = points {
            painter.add(egui::Shape::convex_polygon(points.clone(), color.gamma_multiply(0.08), Stroke::NONE));
            for i in 0..4 {
                painter.line_segment([points[i], points[(i + 1) % 4]], Stroke::new(1.5f32, color.gamma_multiply(0.9)));
            }
        }
    }
    if let Err(why) = &ghost.ok {
        painter.text(screen.center() + vec2(0.0, 36.0), Align2::CENTER_CENTER, why, FontId::proportional(14.0), color);
    }
}

/// How far the `away` script goes each frame going off a strip at a time and back, in meters, and
/// for how many frames
const AWAY_STEP_M: f64 = 0.05;
const AWAY_STEPS: usize = 3300;

/// The screenshot scripts of building and gathering, see App::bases_script()
pub const SCRIPTS: [&str; 6] = ["chop", "mine", "hut", "away", "moon", "raid"];

impl App {
    /// Open, level ground near `near`, in voxels of the world, `across` blocks each way with room
    /// over it: the block of the whole landscape at its corner, just over the ground
    fn open_ground(&self, near: Vector3<f64>, across: i64) -> Option<[i64; 3]> {
        let vpm = self.world.voxels_per_meter as i64;
        let ground = |x: i64, y: i64| -> Option<i64> {
            let top = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(x, y, z)))?;
            (self.water.depth_at(x as f64, y as f64) < 0.05 * vpm as f64).then_some(top + 1)
        };
        for ring in 0..12i64 {
            for k in 0..(ring * 8).max(1) {
                let a = k as f64 / (ring * 8).max(1) as f64 * TAU;
                let c = near + Vector3::new(a.cos(), a.sin(), 0.0) * (ring * 2 * vpm) as f64;
                let lo = [c.x as i64 - across * BLOCK / 2, c.y as i64 - across * BLOCK / 2];
                let mut heights = Vec::new();
                for j in 0..=across * 2 {
                    for i in 0..=across * 2 {
                        heights.push(ground(lo[0] + i * BLOCK / 2, lo[1] + j * BLOCK / 2));
                    }
                }
                let Some(heights) = heights.into_iter().collect::<Option<Vec<i64>>>() else { continue };
                let (min, max) = (*heights.iter().min().unwrap(), *heights.iter().max().unwrap());
                if max - min > vpm * 3 / 4 {
                    continue;
                }
                // Nothing standing there, like a tree
                let clear = (0..=across).all(|j| (0..=across).all(|i| (0..3 * vpm).all(|z| worldgen::is_passable(self.world.material_at(lo[0] + i * BLOCK, lo[1] + j * BLOCK, max + z)))));
                if clear {
                    return Some(self.block_of([lo[0], lo[1], min]));
                }
            }
        }
        None
    }

    /// Builds `kind` at block `at` facing `turn` for a script, paying for it out of what it's been
    /// given, and puts it in the world
    fn script_build(&mut self, kind: Kind, at: [i64; 3], turn: u8) -> bool {
        let built = self.bases.build(kind, at, turn).is_ok();
        if built {
            self.refresh_bases();
        }
        built
    }

    /// A small hut with its corner at block `at`: stone footings down to the ground, walls of
    /// stone, clay and wood five blocks high round four by four inside, a door in its front, which
    /// faces -y, a torch inside and a lantern by the door, and a roof of wood over it
    fn build_hut(&mut self, at: [i64; 3]) -> usize {
        let side = 6;
        let high = 5;
        let mut count = 0;
        for j in 0..side {
            for i in 0..side {
                let wall = i == 0 || j == 0 || i == side - 1 || j == side - 1;
                if !wall {
                    continue;
                }
                // Footings down to the ground where it's lower
                for down in 1..=3 {
                    let b = [at[0] + i, at[1] + j, at[2] - down];
                    let Some(local) = bases::local_block(&self.world, b) else { continue };
                    let solid = bases::read_brick(&self.world, local).iter().filter(|&&m| !worldgen::is_passable(m)).count();
                    if solid > 384 {
                        break;
                    }
                    count += self.script_build(Kind::StoneWall, b, 0) as usize;
                }
                for k in 0..high {
                    // The door's two blocks wide in the middle of the front, four high
                    if j == 0 && (i == 2 || i == 3) && k < 4 {
                        continue;
                    }
                    let kind = match k {
                        0 | 1 => Kind::StoneWall,
                        2 => Kind::ClayWall,
                        _ => Kind::WoodWall,
                    };
                    count += self.script_build(kind, [at[0] + i, at[1] + j, at[2] + k], 0) as usize;
                }
            }
        }
        // The door, facing out the front, from its left as seen from outside
        count += self.script_build(Kind::Door, [at[0] + 2, at[1], at[2]], 3) as usize;
        for j in 0..side {
            for i in 0..side {
                count += self.script_build(Kind::WoodWall, [at[0] + i, at[1] + j, at[2] + high], 0) as usize;
            }
        }
        // A floor inside, a torch standing on it, and a lantern on a post by the door
        for j in 1..side - 1 {
            for i in 1..side - 1 {
                count += self.script_build(Kind::Floor, [at[0] + i, at[1] + j, at[2]], 0) as usize;
            }
        }
        count += self.script_build(Kind::Torch, [at[0] + 4, at[1] + 4, at[2] + 1], 0) as usize;
        count += self.script_build(Kind::StoneWall, [at[0] + 1, at[1] - 1, at[2]], 0) as usize;
        count += self.script_build(Kind::Lantern, [at[0] + 1, at[1] - 1, at[2] + 1], 0) as usize;
        // And a torch on a post on the door's other side
        count += self.script_build(Kind::StoneWall, [at[0] + 4, at[1] - 1, at[2]], 0) as usize;
        count += self.script_build(Kind::Torch, [at[0] + 4, at[1] - 1, at[2] + 1], 0) as usize;
        count
    }

    /// Plays through gathering and building for screenshot runs, see SCRIPTS: `chop` fells the tree
    /// nearest, `mine` digs down through earth to stone, `hut` builds a hut and opens its door, and
    /// `away` builds one, goes far off and back, by the world built afresh and by strips, and looks
    /// at it again. Each says what it did.
    pub(super) fn bases_script(&mut self, name: &str, frame: usize) {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        match (name, frame) {
            // Looking at the moon from where the player stands, `--set moon_up=1` straight up at the
            // sky round it rather than at it
            ("moon", f) if f >= 3 => {
                let eye = camera_to_voxels(self.camera.position, self.world.size);
                let moon = self.sky().moon;
                let up = settings_from_args().iter().any(|(k, v)| k == "moon_up" && v == "1");
                let toward = if up { Vector3::new(moon.x * 0.3, moon.y * 0.3, 1.0).normalize() } else { moon };
                if f == 3 {
                    println!("frame {frame}: the moon's {:.0} degrees up, its phase {:.2}, {:.0}% lit", moon.z.asin().to_degrees(), self.sky().moon_phase, 100.0 * daynight::lit_part(self.sky().moon_phase));
                }
                self.look_from(eye, eye + toward * 100.0 * vpm);
                self.reset_accum = false;
            }
            // A raid on a hut with the player shut in it: the camera off to one side of its door,
            // a shot as it's first struck, once it's half broken, and as it gives way
            ("raid", 3) => {
                self.bases.materials.add(bases::Materials::new(400, 300, 120));
                let ahead = Vector3::new(-self.camera.rotation.z.sin(), self.camera.rotation.z.cos(), 0.0);
                let Some(at) = self.open_ground(feet + ahead * 9.0 * vpm, 6) else {
                    println!("frame {frame}: nowhere open to build");
                    return;
                };
                let count = self.build_hut(at);
                self.building.hut = Some(at);
                // The player inside, on its floor
                let inside = Vector3::new((at[0] + 3) as f64, (at[1] + 3) as f64, (at[2] + 1) as f64) * BLOCK as f64 - origin;
                let mut player = Player::new(inside + Vector3::new(0.0, 0.0, 1.6 * vpm), &self.world);
                player.position = inside;
                self.player = Some(player);
                self.raid.next_wave = self.raid.next_wave.min(1.0);
                println!("frame {frame}: built {count} pieces, the player shut in at block {:?}", [at[0] + 3, at[1] + 3, at[2] + 1]);
            }
            ("raid", f) if f > 3 => {
                let Some(at) = self.building.hut else { return };
                // Looking at the door from off to its side, a little way out
                let door = Vector3::new((at[0] + 3) as f64, at[1] as f64, at[2] as f64) * BLOCK as f64 - origin;
                let eye = door + Vector3::new(-3.2 * vpm, -4.4 * vpm, 2.2 * vpm);
                self.camera.position = voxels_to_camera(eye, self.world.size);
                self.camera.look_at(voxels_to_camera(door + Vector3::new(0.0, -0.8 * vpm, 0.8 * vpm), self.world.size));
                let door_piece = self.bases.piece_at([at[0] + 2, at[1], at[2]]).map(|p| (p.health / p.kind.health(), p.id));
                match (self.building.phase, door_piece) {
                    (0, Some((left, _))) if left < 0.999 => {
                        self.shoot_soon("the door first struck");
                        self.building.phase = 1;
                    }
                    (1, Some((left, _))) if left < 0.5 => {
                        self.shoot_soon("the door half broken");
                        self.building.phase = 2;
                    }
                    (0..=2, None) => {
                        self.shoot_soon("the door broken down");
                        self.building.phase = 3;
                        self.building.mark = f;
                    }
                    (3, _) if f == self.building.mark + 90 => {
                        self.shoot_soon("in through the door");
                        self.building.phase = 4;
                    }
                    _ => {}
                }
                if f % 60 == 0 {
                    println!("frame {frame}: raid on: {}, {} about, the door {:?}, the player {} at {:.0} health, {}", self.raid.on, self.raid.about, door_piece.map(|d| (d.0 * 100.0).round()), if self.game.dead() { "down" } else { "standing" }, self.game.vitals().0, self.game.last_message().unwrap_or(""));
                }
            }
            ("chop", 5) => {
                let trees = self.trees_near(feet, 30.0);
                let Some(&(foot, height, _)) = trees.iter().find(|t| t.1 > 3.0) else {
                    println!("frame {frame}: no tree near");
                    return;
                };
                // Stood by its trunk, on the side with the most room, looking at it
                let side = (0..8)
                    .map(|k| k as f64 / 8.0 * TAU)
                    .find(|a| {
                        let p = foot + Vector3::new(a.cos(), a.sin(), 0.0) * 2.2 * vpm + Vector3::new(0.0, 0.0, 1.5 * vpm);
                        worldgen::is_passable(self.world.material_at(p.x as i64, p.y as i64, p.z as i64))
                    })
                    .unwrap_or(0.0);
                let eye = foot + Vector3::new(side.cos(), side.sin(), 0.0) * 2.4 * vpm + Vector3::new(0.0, 0.0, 1.6 * vpm);
                self.look_from(eye, foot + Vector3::new(0.0, 0.0, 1.2 * vpm));
                println!("frame {frame}: by a tree {height:.1} m tall at {:.0}, {:.0}, carrying {:?}", (foot.x + origin.x) / vpm, (foot.y + origin.y) / vpm, self.bases.materials);
            }
            ("chop", f) if (30..=30 + 24 * 4).contains(&f) && (f - 30) % 24 == 0 => {
                self.gather_blow();
                println!("frame {frame}: blow, {}, carrying {:?}", self.game.last_message().unwrap_or(""), self.bases.materials);
            }
            ("chop", 200) => {
                // Stepped back to see it gone
                let (eye, look) = self.eye_look();
                self.look_from(eye - look * 6.0 * vpm + Vector3::new(0.0, 0.0, 1.5 * vpm), eye + look * 2.0 * vpm);
            }
            ("mine", 5) => {
                // Looking down at dry ground in front, away from the water
                let Some(spot) = self.open_ground(feet, 2) else {
                    println!("frame {frame}: no dry ground near");
                    return;
                };
                let ground = Vector3::new((spot[0] * BLOCK) as f64, (spot[1] * BLOCK) as f64, (spot[2] * BLOCK) as f64) - origin + Vector3::new(4.0, 4.0, 0.0);
                let top = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(ground.x as i64, ground.y as i64, z))).unwrap_or(0) as f64 + 1.0;
                let at = Vector3::new(ground.x, ground.y, top);
                self.look_from(at + Vector3::new(-0.5 * vpm, -1.2 * vpm, 1.6 * vpm), at);
                println!("frame {frame}: digging at {:.0}, {:.0}, {:.0}", (at.x + origin.x) / vpm, (at.y + origin.y) / vpm, top / vpm);
            }
            ("mine", f) if (20..=320).contains(&f) && f % 12 == 0 => {
                let (eye, look) = self.eye_look();
                let hit = gather::raycast(&self.world, eye, look, REACH_M * vpm).map(|(v, _)| self.world.material_at(v[0], v[1], v[2]));
                self.gather_blow();
                if f % 48 == 0 {
                    println!("frame {frame}: struck {:?}, {}, carrying {:?}", hit, self.game.last_message().unwrap_or(""), self.bases.materials);
                }
            }
            ("hut" | "away", 3) => {
                self.bases.materials.add(bases::Materials::new(400, 300, 120));
                let ahead = Vector3::new(-self.camera.rotation.z.sin(), self.camera.rotation.z.cos(), 0.0);
                let Some(at) = self.open_ground(feet + ahead * 9.0 * vpm, 6) else {
                    println!("frame {frame}: nowhere open to build");
                    return;
                };
                let started = Instant::now();
                let count = self.build_hut(at);
                println!(
                    "frame {frame}: built {count} pieces at block {at:?} in {:.1} ms, {} pieces in all, carrying {:?}",
                    started.elapsed().as_secs_f64() * 1000.0,
                    self.bases.pieces().len(),
                    self.bases.materials
                );
                self.building.hut = Some(at);
                self.look_at_hut(at, -1.0);
            }
            ("hut", 150) => {
                // The door opened, from inside
                if let Some(at) = self.building.hut
                    && let Some(id) = self.bases.piece_at([at[0] + 2, at[1], at[2]]).map(|p| p.id)
                {
                    let open = self.bases.toggle_door(id).map(|(open, _)| open);
                    self.refresh_bases();
                    println!("frame {frame}: the door's open: {open:?}");
                    self.look_at_hut(at, -1.0);
                }
            }
            ("hut", 260) => {
                if let Some(at) = self.building.hut {
                    // Looking in through the door
                    let door = Vector3::new((at[0] + 3) as f64, at[1] as f64, at[2] as f64) * BLOCK as f64 - origin;
                    let eye = door + Vector3::new(0.0, -3.5 * vpm, 1.5 * vpm);
                    self.look_from(eye, door + Vector3::new(0.0, 2.0 * vpm, 0.8 * vpm));
                    for l in base_lights(&self.bases, &self.world) {
                        println!("frame {frame}: a light at {:.0}, {:.0}, {:.0}, {:.1} m from the eye, reaching {:.1} m", l[0], l[1], l[2], (Vector3::new(l[0] as f64, l[1] as f64, l[2] as f64) - eye).norm() / vpm, l[3] as f64 / vpm);
                    }
                }
            }
            // Far off, by the world built afresh round the camera, then back the same way once it's
            // there, then off and back a strip at a time, a shot of the hut each time it's back
            ("away", f) if f >= 100 => {
                let settled = !self.held_still() && self.stream.as_ref().is_none_or(|s| s.teleport.is_none());
                let Some(at) = self.building.hut else { return };
                let since = f - self.building.mark;
                let eye = camera_to_voxels(self.camera.position, self.world.size);
                match self.building.phase {
                    0 => {
                        println!("frame {frame}: going 170 m off, {} pieces built, the world's corner at {:?}", self.bases.pieces().len(), self.world.origin);
                        self.look_from(eye + Vector3::new(170.0 * vpm, 0.0, 0.0), eye + Vector3::new(175.0 * vpm, 0.0, 0.0));
                        (self.building.phase, self.building.mark) = (1, f);
                    }
                    1 if settled && since > 60 => {
                        println!("frame {frame}: far off, the world's corner at {:?}, the hut in it: {}", self.world.origin, bases::local_block(&self.world, at).is_some());
                        self.look_at_hut(at, 1.0);
                        (self.building.phase, self.building.mark) = (2, f);
                    }
                    2 if settled && since > 60 => {
                        println!("frame {frame}: back by the world built afresh, the world's corner at {:?}, blocks of it missing {}", self.world.origin, self.missing_blocks(at));
                        self.shoot_soon("back by the world built afresh");
                        (self.building.phase, self.building.mark) = (3, f);
                    }
                    // Off 165 m and back a strip at a time, no faster than the strips come in: 5 cm a
                    // frame, a few meters a second at the rate screenshot runs go
                    3 if since > 10 => {
                        self.camera.position = voxels_to_camera(eye + Vector3::new(0.0, AWAY_STEP_M * vpm, 0.0), self.world.size);
                        if since > AWAY_STEPS {
                            println!("frame {frame}: 165 m off a strip at a time, the world's corner at {:?}, the hut in it: {}", self.world.origin, bases::local_block(&self.world, at).is_some());
                            (self.building.phase, self.building.mark) = (4, f);
                        }
                    }
                    4 => {
                        self.camera.position = voxels_to_camera(eye + Vector3::new(0.0, -AWAY_STEP_M * vpm, 0.0), self.world.size);
                        if since > AWAY_STEPS {
                            (self.building.phase, self.building.mark) = (5, f);
                        }
                    }
                    5 if settled && since > 30 => {
                        self.look_at_hut(at, -1.0);
                        (self.building.phase, self.building.mark) = (6, f);
                    }
                    6 if since > 40 => {
                        println!("frame {frame}: back by strips, the world's corner at {:?}, blocks of it missing {}", self.world.origin, self.missing_blocks(at));
                        self.shoot_soon("back by strips");
                        (self.building.phase, self.building.mark) = (7, f);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        if frame % 120 == 0 && matches!(name, "hut" | "away") {
            let lost = self.building.hut.map_or(0, |at| self.missing_blocks(at));
            println!("frame {frame}: pieces {}, blocks of the hut missing from the world {lost}", self.bases.pieces().len());
        }
    }

    /// Takes a shot in a moment, in a screenshot run, saying so
    fn shoot_soon(&mut self, what: &str) {
        if let Some(capture) = &mut self.capture {
            let frame = capture.frame_times.len() + 2;
            capture.shots.push(frame);
            println!("SHOT at frame {frame}: {what}");
        }
    }

    /// Looks at the hut at block `at` from in front of it, `side` -1 for its front and 1 for its back
    fn look_at_hut(&mut self, at: [i64; 3], side: f64) {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let middle = Vector3::new((at[0] + 3) as f64, (at[1] + 3) as f64, at[2] as f64) * BLOCK as f64 - origin;
        self.look_from(middle + Vector3::new(4.0 * vpm, side * 9.0 * vpm, 4.5 * vpm), middle + Vector3::new(0.0, 0.0, 1.0 * vpm));
    }

    /// How many blocks of what's built in the hut at `at` aren't in the world as they should be
    fn missing_blocks(&self, at: [i64; 3]) -> usize {
        let mut missing = 0;
        for piece in self.bases.pieces() {
            if (0..2).any(|i| (piece.at[i] - at[i]).abs() > 12) {
                continue;
            }
            for (k, b) in piece.blocks().iter().enumerate() {
                let Some(local) = bases::local_block(&self.world, *b) else {
                    continue;
                };
                let have = bases::read_brick(&self.world, local);
                let want = piece.voxels(k);
                if want.iter().zip(have.iter()).any(|(&w, &h)| w != worldgen::material::AIR && w != h) {
                    missing += 1;
                }
            }
        }
        missing
    }
}
