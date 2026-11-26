//! Gathering and building as the player does them, see bases.rs and gather.rs: holding G at a
//! tree, rock or clay gathers it, bare hands or the best tool carried for it, a ring round the
//! crosshair filling till it's done and what it gave rising off it, straight into the pack. B goes
//! into building, where a click builds the piece picked where the outline shows, a right click takes
//! down something built and gives it back, R turns it and the wheel or the number keys pick another.
//! E opens and shuts a door. What's looked at says what G does there, and the first time the player
//! gathers, gets wood, builds or can't afford something, a hint says what to do next, once in a game.
//! What changes goes into the world as it is and to the GPU with the next frame; the strips that come
//! in later are built with it, see stream.rs.

use std::collections::VecDeque;

use egui_winit_vulkano::egui::{Align2, Color32, FontId, Pos2, Stroke, pos2, vec2};

use crate::bases::{self, BLOCK, Bases, Kind, Materials, Piece, Res};
use crate::gather::{self, Yield};
use crate::pieces::TABS;
use crate::tools::{self, Gear, Tool};

use super::*;

/// How far from the eye things can be built and gathered, in meters
const REACH_M: f64 = 5.0;
/// Seconds between the blows struck while G's held, each with its chips flying and its sound
const BLOW_SECONDS: f64 = 0.4;
/// How far a tool in hand reaches to gather what its blow lands on, in meters: less than G's, so a
/// swing at something nearer doesn't dig the ground far behind it
const TOOL_REACH_M: f64 = 3.2;
/// How much a tool's swing landing gathers, as so many seconds of holding G with it
const SWING_SECONDS: f64 = 0.6;
/// Seconds how far through something the player got is kept after they stop, should they go on
const FORGET_SECONDS: f64 = 3.0;
/// How long what's gathered rises off the crosshair, in seconds
const POPUP_SECONDS: f64 = 2.0;
/// How long a hint shows, and how long at least before the next one waiting takes its place
const HINT_SECONDS: f64 = 12.0;
const HINT_LEAST_SECONDS: f64 = 4.5;
/// How much a blast a meter round does to what's built right where it is
const BLAST_DAMAGE: f64 = 70.0;

/// What's being gathered
struct Gathering {
    /// The block of the whole landscape struck, or for a tree its column, z left 0
    key: [i64; 3],
    kind: Yield,
    /// How far through it, from 0 to 1: holding G at it goes on through it as quickly as what it's
    /// gathered with is, see tools::power() and gather::work_seconds()
    done: f64,
}

/// What G would gather where the player's looking: the voxel, the way out of the face it's looked
/// at through, what it's made of and gives, whether it's a tree's trunk, and the block of the whole
/// landscape it's in, or for a tree its column, z left 0
#[derive(Clone, Copy)]
struct Target {
    hit: [i64; 3],
    normal: [i64; 3],
    material: u8,
    kind: Yield,
    tree: bool,
    key: [i64; 3],
}

/// What the HUD says under the crosshair about what's looked at, see App::update_prompt(): what
/// it is, what it gives, and what the tool for it does or would do and whether one's carried
pub struct Prompt {
    kind: Yield,
    tree: bool,
    key: [i64; 3],
    material: u8,
    tool: Option<(String, bool)>,
}

/// The first-time hints, each shown once in a game: on first gathering, on the first wood, on
/// first going into building, and the first time a piece picked can't be afforded. A bit each in
/// Bases::hints, so they go in the save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    Gather = 1,
    Wood = 2,
    Build = 4,
    Short = 8,
}

/// A hint on the screen, or waiting to be: its title, what it says, and how long it's shown
struct HintCard {
    title: String,
    lines: Vec<String>,
    age: f64,
}

/// What's just been gathered, rising off the crosshair: like "+8 Wood", in its colour
struct Popup {
    text: String,
    color: Color32,
    age: f64,
}

/// The first of what's carried to build with, always shown in the HUD; the rest of materials::Res
/// shows once there's any of it
const ALWAYS: usize = 3;

/// What `have` is short of `cost`, like "2 stone, 1 clay", and where the first of it's to be got
fn shortfall(cost: Materials, have: Materials) -> Option<(String, &'static str)> {
    let short: Vec<(Res, u32)> = Res::ALL.iter().map(|&r| (r, cost.get(r).saturating_sub(have.get(r)))).filter(|&(_, n)| n > 0).collect();
    let first = short.first()?.0;
    let what = short.iter().map(|(r, n)| format!("{n} {}", r.name().to_lowercase())).collect::<Vec<_>>().join(", ");
    Some((what, first.from()))
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
    /// Seconds till the next blow's struck while G's held
    wait: f64,
    /// Seconds since what's being gathered was last worked at, for the HUD's ring
    since_blow: f64,
    /// What's said under the crosshair about what's looked at, see update_prompt()
    pub(super) prompt: Option<Prompt>,
    /// What's just been gathered, rising off the crosshair, and how long each of what's carried
    /// is lit up in the HUD for having just gone up, by materials::Res
    popups: Vec<Popup>,
    flash: [f64; Res::ALL.len()],
    /// Which of pieces::TABS the pieces are picked from, C going on to the next
    pub tab: usize,
    /// Trees coming down, see felling.rs
    pub(super) felling: crate::felling::Felling,
    /// The hint showing, first, and those waiting after it
    hints: VecDeque<HintCard>,
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
    pub(super) phase: u8,
    pub(super) mark: usize,
    /// And for the gate's: where it's built, and the realm to come back to
    pub(super) spot: Option<Vector3<f64>>,
    pub(super) back_to: usize,
    /// Whether the inventory's Craft tab is open, see tools.rs, and how many blows the weapon in
    /// hand had landed when last looked at, a tool's gathering where each lands
    pub(super) craft_open: bool,
    /// The fires' smoke, embers and crackling, see hearths.rs
    pub(super) fires: crate::hearths::Fires,
    landed: u32,
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

/// What would be built where the player's looking, and whether it can be: a piece, or the Innert
/// Gate or a portal home, `home`, with its foot at `at` in voxels of the world, see gates.rs
pub enum Ghost {
    Piece { piece: Piece, ok: Result<(), String> },
    Gate { at: Vector3<f64>, facing: f64, home: bool, ok: Result<(), String> },
}

impl Ghost {
    pub fn ok(&self) -> &Result<(), String> {
        match self {
            Ghost::Piece { ok, .. } | Ghost::Gate { ok, .. } => ok,
        }
    }
}

/// Quarter turns round from +x of the way `look` goes, flat
fn snapped(look: Vector3<f64>) -> u8 {
    let a = look.y.atan2(look.x);
    ((a / FRAC_PI_2).round() as i64).rem_euclid(4) as u8
}

impl App {
    /// The eye and which way it looks, in voxels of the world
    pub(super) fn eye_look(&self) -> (Vector3<f64>, Vector3<f64>) {
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let look = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
        (eye, look)
    }

    /// The block of the whole landscape voxel `v` of the world is in
    fn block_of(&self, v: [i64; 3]) -> [i64; 3] {
        [(v[0] + self.world.origin[0]).div_euclid(BLOCK), (v[1] + self.world.origin[1]).div_euclid(BLOCK), (v[2] + self.world.origin[2]).div_euclid(BLOCK)]
    }

    /// Reads the keys for gathering and building, called each frame while playing. Returns whether
    /// the mouse's buttons went to building, so they don't swing what's in hand too.
    pub(super) fn update_building(&mut self, dt: f64) -> bool {
        // Not while the world's built afresh round the player, behind the dark
        if self.held_still() {
            return false;
        }
        if self.input.key_pressed(KeyCode::KeyB) {
            self.toggle_building();
        }
        // Holding G gathers what's looked at
        if self.input.key_held(KeyCode::KeyG) {
            self.gather_hold(dt, self.input.key_pressed(KeyCode::KeyG));
        }
        if !self.building.on {
            return false;
        }
        // C goes on to the next tab of pieces, shift and C back
        if self.input.key_pressed(KeyCode::KeyC) {
            let back = self.input.key_held(KeyCode::ShiftLeft) || self.input.key_held(KeyCode::ShiftRight);
            self.pick_tab((self.building.tab as i64 + if back { -1 } else { 1 }).rem_euclid(TABS.len() as i64) as usize);
        }
        // The pieces of the tab, and the gate or a portal home after them where they can be built
        let entries = self.tab_entries();
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9, KeyCode::Digit0, KeyCode::Minus];
        if let Some(k) = digits.iter().position(|&k| self.input.key_pressed(k)) {
            self.pick(entries[k.min(entries.len() - 1)]);
        }
        let wheel = self.input.scroll_diff().1;
        if wheel != 0.0 {
            let at = entries.iter().position(|&e| e == self.building.picked).unwrap_or(0) as i64;
            self.pick(entries[(at + if wheel < 0.0 { 1 } else { -1 }).rem_euclid(entries.len() as i64) as usize]);
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
        if self.building.picked >= Kind::ALL.len() {
            return self.gate_ghost();
        }
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let (hit, normal) = gather::raycast(&self.world, eye, look, REACH_M * vpm)?;
        let front = [hit[0] + normal[0], hit[1] + normal[1], hit[2] + normal[2]];
        let kind = Kind::ALL[self.building.picked];
        // A door faces the player and spans to their right; stairs go up away from them
        let facing = snapped(look);
        let auto = match kind {
            // A fireplace and its chimney face the player too, and a gate, an arch, a furnace and
            // a banner, and what's set across the way they look
            Kind::Door | Kind::Hearth | Kind::Chimney | Kind::IronGate | Kind::Arch | Kind::Furnace | Kind::Banner | Kind::HalfWall => facing + 2,
            Kind::Fence | Kind::GlassWindow | Kind::GlassPane | Kind::IronBars => facing,
            // Stairs and roofs go up away from them
            Kind::Stairs | Kind::ThatchRoof | Kind::SlateRoof => facing,
            _ => 0,
        };
        let piece = Piece::new(0, kind, self.block_of(front), auto + self.building.turn);
        let ok = self.can_build(&piece);
        Some(Ghost::Piece { piece, ok })
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
        // What's short, and where to get it
        if let Some((short, from)) = shortfall(piece.kind.cost(), self.bases.materials) {
            return Err(format!("Need {short} more: {from}"));
        }
        Ok(())
    }

    /// Into building or out of it, with B: the first time in, a hint at what each key does
    pub(super) fn toggle_building(&mut self) {
        self.building.on = !self.building.on;
        if !self.building.on {
            self.game.say("Stopped building.");
            return;
        }
        self.game.say("Building: pick a piece, click to build it. B to stop.");
        self.hint(
            Hint::Build,
            "Building",
            vec![
                "C: the next tab of pieces, by what they're made of. 1–9, 0 or the wheel: pick a piece. What it costs is beside it, greyed out till you have enough.".into(),
                "Click: build it where the outline shows, green if it fits, red saying why not.".into(),
                "Right click: take down what you built, and get it all back.".into(),
                "R: turn it  ·  E: open a door, or mend  ·  G: gather  ·  B: stop building".into(),
            ],
        );
        self.short_hint();
    }

    /// Picks piece `k` of those to build, of Kind::ALL, the gate or portal after them
    pub(super) fn pick(&mut self, k: usize) {
        self.building.picked = k;
        if let Some(&kind) = Kind::ALL.get(k) {
            self.building.tab = crate::pieces::tab_of(kind);
        }
        self.short_hint();
    }

    /// Goes to tab `tab` of the pieces, picking its first
    pub(super) fn pick_tab(&mut self, tab: usize) {
        self.building.tab = tab;
        if let Some(&first) = TABS[tab].1.first() {
            self.pick(Kind::ALL.iter().position(|&k| k == first).unwrap_or(0));
        }
    }

    /// What can be picked in the tab that's open, each by its place in Kind::ALL, and the gate or a
    /// portal home after them where they can be built
    fn tab_entries(&self) -> Vec<usize> {
        let mut entries: Vec<usize> = TABS[self.building.tab.min(TABS.len() - 1)].1.iter().filter_map(|k| Kind::ALL.iter().position(|a| a == k)).collect();
        if self.gate_choice().is_some() {
            entries.push(Kind::ALL.len());
        }
        entries
    }

    /// The name of the piece picked to build and what it costs, the gate or a portal home too
    fn picked_cost(&self) -> Option<(&'static str, Materials)> {
        match Kind::ALL.get(self.building.picked) {
            Some(kind) => Some((kind.name(), kind.cost())),
            None => match self.gate_choice()? {
                false => Some(("The Innert Gate", crate::gates::GATE_COST)),
                true => Some(("A portal home", crate::gates::PORTAL_COST)),
            },
        }
    }

    /// The first time the piece picked can't be afforded, a hint at what's short and where to get it
    fn short_hint(&mut self) {
        if self.bases.hints & Hint::Short as u32 != 0 {
            return;
        }
        let Some((name, cost)) = self.picked_cost() else { return };
        let have = self.bases.materials;
        let Some((short, _)) = shortfall(cost, have) else { return };
        let held: Vec<String> = cost.parts().iter().map(|&(r, _)| format!("{} {}", have.get(r), r.name().to_lowercase())).collect();
        let mut lines = vec![format!("{name} costs {}. You have {}: {short} short.", cost.describe(), held.join(", "))];
        for (r, n) in cost.parts() {
            if have.get(r) < n {
                lines.push(format!("{}: {}.", r.name(), r.from()));
            }
        }
        lines.push("What you gather goes straight into your pack, and pieces you can afford light up in the list.".into());
        self.hint(Hint::Short, "Not enough to build that", lines);
    }

    /// Shows hint `hint` if it hasn't been shown in this game: `title` over `lines`
    fn hint(&mut self, hint: Hint, title: &str, lines: Vec<String>) {
        if self.bases.hints & hint as u32 != 0 {
            return;
        }
        self.bases.hints |= hint as u32;
        self.building.hints.push_back(HintCard { title: title.into(), lines, age: 0.0 });
    }

    /// Builds the piece picked where the outline shows, if it can be
    fn build_here(&mut self) {
        let piece = match self.ghost() {
            None => return,
            Some(Ghost::Gate { .. }) => return self.build_gate_here(),
            Some(Ghost::Piece { ok: Err(why), .. }) => {
                self.short_hint();
                return self.game.say(why);
            }
            Some(Ghost::Piece { piece, .. }) => piece,
        };
        let Piece { kind, at, turn, .. } = piece;
        if self.bases.build(kind, at, turn).is_ok() {
            self.refresh_bases();
            let middle = self.bases.piece_at(at).map(|p| p.middle());
            use worldgen::material::{CLOTH_BLUE, PLANK_BIRCH, PLANK_DARK, PLANK_PALM, PLANK_PINE, THATCH, WOOD};
            let soft = matches!(kind.material(), WOOD | THATCH | CLOTH_BLUE | PLANK_BIRCH | PLANK_DARK | PLANK_PINE | PLANK_PALM);
            self.play_at(audio::Sound::Place, middle, 0.8, if soft { 1.0 } else { 0.8 });
            if kind.is_door() {
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
        let Some((id, kind)) = self.bases.piece_at(block).map(|p| (p.id, p.kind)) else {
            self.game.say("Only what you've built can be taken down.");
            return;
        };
        if !kind.player_built() {
            self.game.say("The gate's old foundation stones won't budge.");
            return;
        }
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
            if let Some(door) = self.bases.piece_at(block).filter(|p| p.kind.is_door()).map(|p| p.id) {
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

    /// Smelts what's carried at a furnace the player's looking at, if there's one in reach: ore to
    /// copper and iron and sand to glass, burning wood, see materials::smelt(). Whether there was.
    pub(super) fn furnace_here(&mut self) -> bool {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let Some((hit, _)) = gather::raycast_by(&self.world, eye, look, REACH_M * vpm, |m| m != worldgen::material::AIR) else {
            return false;
        };
        let Some(middle) = self.bases.piece_at(self.block_of(hit)).filter(|p| p.kind == Kind::Furnace).map(|p| p.middle()) else {
            return false;
        };
        let (used, made) = crate::materials::smelt(self.bases.materials);
        if made.is_empty() {
            let has = Res::ALL.iter().any(|r| r.smelts_to().is_some() && self.bases.materials.get(*r) > 0);
            self.game.say(if has { "The furnace needs wood to burn: a log for every four it smelts." } else { "Bring ore or sand to smelt: copper and iron ore, or two sand for a glass." });
            return true;
        }
        self.bases.materials.take(used);
        self.gained(made);
        self.game.say(format!("Smelted {}, burning {} wood.", made.describe(), used.wood));
        self.play_at(audio::Sound::Sear, Some(middle), 0.9, 0.8);
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        for k in 0..6 {
            let up = Vector3::new(0.0, 0.0, 0.6 * vpm + k as f64);
            self.game.wisp(Vector3::from(middle) - origin + up, Vector3::new(0.0, 0.0, 1.2 * vpm), 1.5, k % 2 == 0);
        }
        // Bits of what it made spilling out of its mouth
        let bits: Vec<u8> = made.parts().iter().map(|(r, _)| r.material()).collect();
        self.game.debris(Vector3::from(middle) - origin - Vector3::new(0.0, 0.0, 0.3 * vpm), &bits, vpm);
        true
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

    /// A tool in hand gathers where each of its blows lands, if it's at what the tool's for, see
    /// tools.rs; and what's just been gathered rises off the crosshair and fades, and the hints
    /// come and go. Called each frame after the fighting's moved on, `dt` seconds on.
    pub(super) fn update_tools(&mut self, dt: f64) {
        let b = &mut self.building;
        b.since_blow += dt;
        if b.since_blow > FORGET_SECONDS {
            b.gathering = None;
        }
        b.popups.retain_mut(|p| {
            p.age += dt;
            p.age < POPUP_SECONDS
        });
        b.flash = b.flash.map(|f| (f - dt).max(0.0));
        // A hint stays its time, or once it's been read a while, gives way to the next one waiting
        let waiting = b.hints.len() > 1;
        if let Some(card) = b.hints.front_mut() {
            card.age += dt;
            if card.age > HINT_SECONDS || (waiting && card.age > HINT_LEAST_SECONDS) {
                b.hints.pop_front();
            }
        }
        let landed = self.game.swings_landed();
        if landed == self.building.landed {
            return;
        }
        self.building.landed = landed;
        // Not a blow at an animal, which is fought as with any weapon, and only at what the tool's
        // for, not so far off as G reaches
        if let Some(gear) = self.game.held_gear()
            && !self.game.has_target()
            && !self.building.on
            && let Ok(target) = self.gather_target(TOOL_REACH_M)
            && Tool::for_yield(target.kind) == Some(gear.tool)
        {
            self.work_at(target, Some(gear), SWING_SECONDS, true);
        }
    }

    /// What G would gather where the player's looking, no further than `reach_m`: or what's to be
    /// said for why there's nothing, if anything, as the tool the hardest of it needs
    fn gather_target(&self, reach_m: f64) -> Result<Target, Option<String>> {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        // Stopping at reeds and flax, though they're walked through
        let (hit, normal) = gather::raycast_by(&self.world, eye, look, reach_m * vpm, |m| !worldgen::is_passable(m) || crate::materials::is_crop(m)).ok_or(None)?;
        let block = self.block_of(hit);
        if let Some(piece) = self.bases.piece_at(block) {
            return Err(Some(if piece.kind.player_built() { "That's built: take it down while building (B), with a right click." } else { "The gate's old foundation stones won't budge." }.into()));
        }
        let material = self.world.material_at(hit[0], hit[1], hit[2]);
        let kind = gather::yield_of(material);
        if kind == Yield::Nothing {
            return Err(Some("There's nothing to gather there.".into()));
        }
        if let Some((tool, _)) = tools::needs(material) {
            tools::can_gather(material, self.game.best_tool(tool)).map_err(Some)?;
        }
        let tree = kind == Yield::Wood && gather::is_trunk(material);
        // A tree's struck anywhere up its trunk
        let key = if tree { [block[0], block[1], 0] } else { block };
        Ok(Target { hit, normal, material, kind, tree, key })
    }

    /// G held `dt` seconds more, `pressed` this frame: gathers what's looked at with the best tool
    /// carried for it, or bare hands, as long as it's held at the same thing
    pub(super) fn gather_hold(&mut self, dt: f64, pressed: bool) {
        match self.gather_target(REACH_M) {
            Ok(target) => {
                let gear = Tool::for_yield(target.kind).and_then(|t| self.game.best_gear(t));
                self.building.wait -= dt;
                self.work_at(target, gear, dt, false);
            }
            Err(Some(why)) if pressed => self.game.say(why),
            Err(_) => {}
        }
    }

    /// Works out what's said under the crosshair about what G would gather where the player's
    /// looking, for the HUD, if `shown`: nothing while building, nor at an animal, a chest or what's
    /// lying about, which say what to do with them themselves
    pub(super) fn update_prompt(&mut self, shown: bool) {
        self.building.prompt = if !shown || self.building.on || self.game.has_target() {
            None
        } else {
            self.gather_target(REACH_M).ok().map(|t| Prompt { kind: t.kind, tree: t.tree, key: t.key, material: t.material, tool: tools::faster(t.kind, Tool::for_yield(t.kind).and_then(|tool| self.game.best_gear(tool))) })
        };
    }

    /// `seconds` of work at `target` with `tool`, or bare hands, gathering it once it's done: a
    /// blow struck now and then, its chips flying and its sound, and with `swing`, the tool in
    /// hand's blow landing, one now
    fn work_at(&mut self, target: Target, tool: Option<Gear>, seconds: f64, swing: bool) {
        let Target { hit, normal, material, kind, tree, key } = target;
        let Some(power) = tools::power(tool, kind) else {
            return;
        };
        // Not while the tree's coming down
        if tree && self.building.felling.busy() {
            return;
        }
        // An axe that fells a whole tree at a blow does so with its first
        let tool = tool.filter(|g| Tool::for_yield(kind) == Some(g.tool));
        let power = if tree && swing && tool.is_some_and(|g| g.perks.fell) { 1e6 } else { power };
        // Quicker for the skill web's gathering, see skills::Stat::GatherSpeed
        let power = power * self.game.gather_speed();
        let vpm = self.world.voxels_per_meter as f64;
        let same = self.building.gathering.as_ref().is_some_and(|g| g.key == key && g.kind == kind);
        if !same {
            self.building.gathering = Some(Gathering { key, kind, done: 0.0 });
            self.building.wait = 0.0;
        }
        let gathering = self.building.gathering.as_mut().unwrap();
        gathering.done += seconds * power / gather::work_seconds(kind, tree);
        let done = gathering.done >= 1.0 - 1e-6;
        self.building.since_blow = 0.0;
        self.hint(
            Hint::Gather,
            "Gathering",
            vec![
                "Keep holding G till the ring round the crosshair closes.".into(),
                "What you gather goes straight into your pack: see the panel at the top right.".into(),
                format!("Bare hands gather most things. Tools are {:.0}–{:.1}× faster: craft them in I › Craft.", tools::SPEED[tools::CRUDE], tools::SPEED[tools::OBSIDIAN_TIER]),
            ],
        );
        if swing || self.building.wait <= 0.0 || done {
            self.building.wait = BLOW_SECONDS;
            // Chips flying off where it's struck: more from a tool's, with splinters of wood, dust
            // and grit off stone, and clods of what's dug
            let at = Vector3::new(hit[0] as f64 + 0.5 + normal[0] as f64, hit[1] as f64 + 0.5 + normal[1] as f64, hit[2] as f64 + 0.5 + normal[2] as f64);
            let bits: Vec<u8> = match (tool.is_some(), kind) {
                (false, _) => vec![material; 3],
                (true, Yield::Wood) => vec![material, material, worldgen::material::DEAD_WOOD, material, worldgen::material::DEAD_WOOD],
                (true, Yield::Stone | Yield::Ore | Yield::Cut) => vec![material, worldgen::material::GRAVEL, material, worldgen::material::GRAVEL, worldgen::material::STONE, material],
                (true, _) => vec![material; 5],
            };
            self.game.debris(at, &bits, vpm);
            let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
            let sound = match kind {
                Yield::Wood => audio::Sound::Chop,
                Yield::Stone | Yield::Ore | Yield::Cut => audio::Sound::Mine,
                _ => audio::Sound::Dig,
            };
            // A better tool rings a little brighter
            let pitch = 0.94 + 0.05 * tool.map_or(0.0, |g| g.tier as f32);
            self.play_at(sound, Some((at + origin).into()), if tool.is_some() { 0.9 } else { 0.8 }, pitch);
        }
        if done {
            self.building.gathering = None;
            self.gather_done(target, tool);
        }
    }

    /// What's been gathered into the pack, rising off the crosshair, and the first wood's hint
    fn gained(&mut self, gives: Materials) {
        self.bases.materials.add(gives);
        for (r, n) in gives.parts() {
            self.building.popups.push(Popup { text: format!("+{n} {}", r.name()), color: r.color(), age: 0.0 });
            if let Some(i) = Res::ALL.iter().position(|&o| o == r) {
                self.building.flash[i] = 1.2;
            }
        }
        if gives.wood > 0 {
            self.hint(Hint::Wood, "Wood", vec!["Wood builds walls, floors, doors, stairs and torches, and feeds a fire.".into(), "Press B to build, and C there for the tabs of pieces.".into()]);
        }
    }

    /// Takes what `target` is out of the world, once it's been worked at enough, with `gear` or
    /// bare hands: the whole tree for a tree's trunk, coming down, a crop where it grows, otherwise
    /// the block it's in, and with a wide swing the blocks either side, into the pack, twice over now
    /// and then with a lucky tool, smelted with one that smelts
    fn gather_done(&mut self, target: Target, gear: Option<Gear>) {
        let Target { hit, normal, tree, kind, .. } = target;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let at = Vector3::new(hit[0] as f64 + 0.5 + normal[0] as f64, hit[1] as f64 + 0.5 + normal[1] as f64, hit[2] as f64 + 0.5 + normal[2] as f64);
        // Nothing built near, as is mostly so, and what's built isn't looked up for every voxel
        let block = self.block_of(hit);
        let near = self.bases.pieces().iter().any(|p| (p.at[0] - block[0]).abs() < 40 && (p.at[1] - block[1]).abs() < 40);
        let built = |v: [i64; 3]| near && self.bases.piece_at([(v[0] + self.world.origin[0]).div_euclid(BLOCK), (v[1] + self.world.origin[1]).div_euclid(BLOCK), v[2].div_euclid(BLOCK)]).is_some();
        let started = Instant::now();
        if tree && let Some(felled) = gather::tree_at(&self.world, hit, &built) {
            self.slow("finding the tree", started);
            let player = self.player.as_ref().map(|p| p.position);
            self.fell(felled, player);
            return;
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
        let luck = bases::hash3(hit[0] as u32, hit[1] as u32, (hit[2] as u32) ^ now);
        let twice = gear.is_some_and(|g| (luck >> 8) as f64 / 16_777_216.0 < g.perks.double);
        let mut gives = Materials::default();
        let mut cut = gather::Cut::new();
        let mut material = None;
        if kind == Yield::Plant {
            if let Some(crop) = gather::crop_at(&self.world, hit, &built) {
                gives.add(crop.gives);
                material = Some(crop.material);
                for (b, v) in crop.cut {
                    cut.entry(b).or_default().extend(v);
                }
            }
        } else {
            let local = hit.map(|v| v.div_euclid(BLOCK));
            let mut blocks = vec![local];
            // A wide swing takes the blocks either side of the one struck, across the face
            if gear.is_some_and(|g| g.perks.wide) {
                let (_, look) = self.eye_look();
                let side = if normal[2] != 0 { if look.x.abs() > look.y.abs() { [0, 1, 0] } else { [1, 0, 0] } } else { [normal[1].abs(), normal[0].abs(), 0] };
                blocks.push([local[0] + side[0], local[1] + side[1], local[2]]);
                blocks.push([local[0] - side[0], local[1] - side[1], local[2]]);
            }
            for (k, b) in blocks.into_iter().enumerate() {
                // What's built, and what the tools carried can't yet break, stays
                if k > 0 && (self.bases.piece_at(self.block_of(b.map(|v| v * BLOCK))).is_some() || !self.can_break(b)) {
                    continue;
                }
                let Some(broken) = gather::block_at(&self.world, b, &built) else { continue };
                gives.add(broken.gives);
                material.get_or_insert(broken.material);
                for (b, v) in broken.cut {
                    cut.entry(b).or_default().extend(v);
                }
            }
        }
        let Some(material) = material else { return };
        self.cut(&cut);
        if twice && !gives.is_empty() {
            let again = gives;
            gives.add(again);
            self.building.popups.push(Popup { text: "Twice the yield!".into(), color: Color32::from_rgb(255, 205, 110), age: 0.0 });
        }
        if gear.is_some_and(|g| g.perks.smelt) {
            gives = crate::materials::smelted(gives);
        }
        if gives.is_empty() {
            self.building.popups.push(Popup { text: "Dug out".into(), color: Color32::from_gray(190), age: 0.0 });
        }
        self.gained(gives);
        self.debris_at((at + origin).into(), material, 5);
    }

    /// `tree` cut through: planned to come down away from the player standing at `player`, its
    /// model built meanwhile, see felling.rs and update_felling()
    fn fell(&mut self, tree: gather::Tree, player: Option<Vector3<f64>>) {
        let (_, look) = self.eye_look();
        let started = Instant::now();
        let plan = crate::felling::plan(&self.world, tree, player, look);
        self.slow("planning a tree's fall", started);
        self.building.felling.start(plan);
    }

    /// The trees coming down, `dt` seconds on: those whose models are ready taken out of the world
    /// and set falling, a warning shouted if the player's where one will land; each tipping on over
    /// till it crashes down, hurting what's under it, throwing the player clear, the ground shaking
    /// and its leaves and splinters bursting off it; then broken up into its wood, given
    pub(super) fn update_felling(&mut self, dt: f64) {
        use crate::felling::{self, Fall, Topple};
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let player = self.player.as_ref().map(|p| p.position + origin);
        for (plan, mut built, slot) in self.building.felling.ready() {
            let started = Instant::now();
            // Out of the world, as the world is now, which may have moved on since it was planned
            let shift = [(plan.origin[0] - self.world.origin[0]).div_euclid(BLOCK), (plan.origin[1] - self.world.origin[1]).div_euclid(BLOCK)];
            let cut: gather::Cut = plan.cut.iter().map(|(b, v)| ([b[0] + shift[0], b[1] + shift[1], b[2]], v.clone())).collect();
            self.cut(&cut);
            self.game.set_one_off(slot, std::mem::take(&mut built.packed));
            self.slow("taking a tree out to fall", started);
            let topple = Topple::new(plan.kind, plan.height / vpm);
            let mut fall = Fall { plan, built, topple, slot, warned: false, struck: false };
            // The crack of it going, and a shout if the player's in the way
            let foot = fall.point(0.0, 0.0);
            self.play_at(audio::Sound::Timber, Some((foot + Vector3::new(0.0, 0.0, fall.plan.height * 0.3)).into()), 1.0, if fall.plan.kind == felling::Kind::Dead { 1.25 } else { 1.0 });
            if player.is_some_and(|p| felling::in_the_way(&fall, p, vpm)) {
                fall.warned = true;
                self.game.say("Timber! It's coming down your way: get clear!");
                self.game.jolt(0.15);
            }
            self.building.felling.falls.push(fall);
        }
        let mut k = 0;
        while k < self.building.felling.falls.len() {
            let fall = &self.building.felling.falls[k];
            let mut topple = fall.topple;
            let landed = topple.step(dt, |angle| felling::hits_ground(&self.world, fall, angle));
            self.building.felling.falls[k].topple = topple;
            if landed {
                self.tree_lands(k, player);
            }
            if self.building.felling.falls[k].topple.done() {
                let fall = self.building.felling.falls.remove(k);
                // Broken up where it lies, into the wood it gives
                let angle = fall.topple.angle;
                for s in 0..5 {
                    let at = fall.point((s as f64 + 0.5) / 5.0, angle) - origin;
                    self.game.debris(at, &[fall.plan.bark, worldgen::material::DEAD_WOOD, fall.plan.bark], vpm);
                }
                self.gained(fall.plan.gives);
                continue;
            }
            k += 1;
        }
    }

    /// Tree `k` coming down hits the ground: a thud as heavy as it is, the ground shaking near by,
    /// leaves and splinters bursting off all along it, what's under it hurt, and the player, if
    /// it's coming down on them, thrown clear
    fn tree_lands(&mut self, k: usize, player: Option<Vector3<f64>>) {
        use crate::felling;
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let fall = &self.building.felling.falls[k];
        let first = !fall.struck;
        let angle = fall.topple.angle;
        let heavy = (fall.plan.size as f64 / 60_000.0).sqrt().clamp(0.2, 1.0);
        let middle = fall.point(0.6, angle);
        let (bark, leaf, dir) = (fall.plan.bark, fall.plan.leaf, fall.plan.dir);
        let points: Vec<Vector3<f64>> = (0..8).map(|s| fall.point(0.2 + 0.8 * s as f64 / 7.0, angle)).collect();
        let harm = felling::harm(fall.plan.size) * if first { 1.0 } else { 0.0 };
        let in_way: Vec<usize> = self.game.animals_mut().enumerate().filter(|(_, a)| a.health > 0.0 && felling::in_the_way(fall, a.position + origin, vpm)).map(|(i, _)| i).collect();
        let player_hit = player.is_some_and(|p| felling::in_the_way(fall, p, vpm));
        let foot = fall.point(0.0, 0.0);
        self.building.felling.falls[k].struck = true;
        self.play_at(audio::Sound::Slam, Some(middle.into()), (0.6 + 0.4 * heavy) as f32, (1.05 - 0.35 * heavy) as f32);
        if heavy > 0.5 {
            self.play_at(audio::Sound::Rumble, Some(middle.into()), (0.5 * heavy) as f32, 0.8);
        }
        // The ground shakes, the nearer and bigger the more
        if let Some(p) = player {
            let near = (1.0 - (p - middle).norm() / (25.0 * vpm)).clamp(0.0, 1.0);
            self.game.jolt((0.2 + 0.7 * heavy) * near * if first { 1.0 } else { 0.4 });
        }
        // Leaves and splinters bursting off all along it
        for (s, at) in points.iter().enumerate() {
            let crown = s >= 4;
            let bits: Vec<u8> = (0..if first { 5 } else { 2 }).map(|i| if crown && i % 2 == 0 { leaf.unwrap_or(bark) } else if i % 3 == 0 { worldgen::material::DEAD_WOOD } else { bark }).collect();
            self.game.debris(at - origin, &bits, vpm);
        }
        if harm > 0.0 {
            let mut hit = 0;
            for (i, a) in self.game.animals_mut().enumerate() {
                if in_way.contains(&i) {
                    a.damage(harm, foot - origin);
                    hit += 1;
                }
            }
            if hit > 0 {
                self.game.say(if hit == 1 { "The tree came down on it!".to_owned() } else { format!("The tree came down on {hit} of them!") });
            }
        }
        // Thrown clear, off to the side of the trunk the player's on
        if first && player_hit && let Some(p) = player {
            let to = p - foot;
            let side = if to.x * dir.y - to.y * dir.x > 0.0 { Vector3::new(dir.y, -dir.x, 0.0) } else { Vector3::new(-dir.y, dir.x, 0.0) };
            self.game.shove(side * 7.0 * vpm + Vector3::new(0.0, 0.0, 3.0 * vpm));
            self.game.say("The tree crashed down by you and threw you clear!");
        }
    }

    /// Whether brick `b` of the world, counted from its first, has nothing in it the tools carried
    /// can't break yet, for a wide swing taking it with the one struck
    fn can_break(&self, b: [i64; 3]) -> bool {
        bases::read_brick(&self.world, b).iter().all(|&m| tools::needs(m).is_none_or(|(tool, _)| tools::can_gather(m, self.game.best_tool(tool)).is_ok()))
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
    let mut lights: Vec<[f32; 8]> = bases
        .lights(vpm)
        .into_iter()
        .map(|(at, reach, color)| [(at[0] - origin[0]) as f32, (at[1] - origin[1]) as f32, (at[2] - origin[2]) as f32, reach, color[0], color[1], color[2], 0.0])
        .collect();
    // And the fires burning, see hearths.rs
    lights.extend(crate::hearths::lights(bases, world));
    lights
}

/// What's drawn of the trees coming down, see felling.rs, in voxels of `world`
pub fn felling_instances(building: &Building, game: &crate::game::Game, world: &World) -> Vec<crate::animals::AnimalInstance> {
    building.felling.falls.iter().map(|f| f.instance(game.one_off_model(f.slot), world.origin)).collect()
}

/// The colour of what `kind` gives, in the HUD
fn yield_color(kind: Yield) -> Color32 {
    match kind {
        Yield::Wood => Res::Wood.color(),
        Yield::Stone => Res::Stone.color(),
        Yield::Cut => Res::Sandstone.color(),
        Yield::Ore => Res::IronOre.color(),
        Yield::Clay => Res::Clay.color(),
        Yield::Sand => Res::Sand.color(),
        Yield::Plant => Res::Reed.color(),
        _ => Color32::from_gray(200),
    }
}

/// `text` on a dark backing `backing` opaque, so it reads over the brightest of what's behind it,
/// the middle of its top at `at`
fn boxed(painter: &egui::Painter, at: Pos2, text: &str, size: f32, color: Color32, backing: u8) {
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(size), color);
    let corner = pos2(at.x - galley.size().x * 0.5, at.y);
    painter.rect_filled(egui::Rect::from_min_size(corner, galley.size()).expand2(vec2(11.0, 3.0)), 7.0, Color32::from_black_alpha(backing));
    painter.galley(corner, galley, color);
}

/// What's carried to build with, under the clock, each with what it's for; what G does at what's
/// looked at, under the crosshair, and a ring round it filling as it's gathered, what it gave
/// rising off it; a first-time hint, at the left; and in building, the pieces to pick from with
/// what each costs, and the outline of `ghost`, where the one picked would go
pub fn hud(building: &Building, bases: &Bases, ghost: Option<&Ghost>, gate: Option<bool>, world: &World, ctx: &egui::Context, view: &combat::View) {
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("building")));
    let screen = ctx.screen_rect();
    let c = screen.center();
    // What's carried, and what each is for, lit up for a moment as it goes up: wood, stone and
    // clay always, the rest once there's any, those after the first few in rows of their own a
    // little smaller
    let m = bases.materials;
    let shown: Vec<(usize, Res)> = Res::ALL.iter().copied().enumerate().filter(|&(k, r)| k < ALWAYS || m.get(r) > 0).collect();
    let top = screen.top() + 150.0;
    let right = screen.right() - 16.0;
    let (width, row, small) = (196.0, 34.0, 20.0);
    let high = ALWAYS as f32 * row + (shown.len() - ALWAYS) as f32 * small;
    crate::theme::hud_panel(&painter, egui::Rect::from_min_size(pos2(right - width, top - 14.0), vec2(width + 8.0, high + 6.0)), 8.0);
    let mut y = top;
    for &(k, res) in &shown {
        let full = k < ALWAYS;
        let color = res.color();
        let flash = (building.flash[k] / 1.2).clamp(0.0, 1.0) as f32;
        if flash > 0.0 {
            painter.rect_filled(egui::Rect::from_min_size(pos2(right - width + 3.0, y - 11.0), vec2(width + 2.0, if full { row } else { small } - 2.0)), 6.0, color.gamma_multiply(0.45 * flash));
        }
        painter.rect_filled(egui::Rect::from_min_size(pos2(right - width + 8.0, y - 6.0), vec2(12.0, 12.0)), 2.0, color);
        painter.text(pos2(right - width + 26.0, y), Align2::LEFT_CENTER, res.name(), FontId::proportional(if full { 14.0 } else { 13.0 }), Color32::from_gray(225));
        painter.text(pos2(right, y), Align2::RIGHT_CENTER, m.get(res).to_string(), FontId::proportional(if full { 15.0 } else { 13.5 }), Color32::WHITE);
        if full {
            painter.text(pos2(right - width + 26.0, y + 14.0), Align2::LEFT_CENTER, res.uses(), FontId::proportional(11.0), Color32::from_gray(160));
        }
        y += if full { row } else { small };
    }
    // How far through what's being gathered, as a ring round the crosshair in the colour of what
    // it gives, fading once it's left off
    let working = building.gathering.as_ref().filter(|_| building.since_blow < 1.0);
    if let Some(g) = working {
        let fade = (1.0 - (building.since_blow as f32 - 0.4).max(0.0) / 0.6).clamp(0.0, 1.0);
        let fraction = g.done.clamp(0.0, 1.0) as f32;
        let radius = 22.0;
        let points: Vec<Pos2> = (0..=48).map(|k| k as f32 / 48.0 * fraction * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2).map(|a| c + vec2(a.cos(), a.sin()) * radius).collect();
        let light = yield_color(g.kind);
        let light = Color32::from_rgb(light.r().saturating_add(70), light.g().saturating_add(70), light.b().saturating_add(70));
        painter.circle_stroke(c, radius, Stroke::new(8.0f32, Color32::from_black_alpha((150.0 * fade) as u8)));
        painter.add(egui::Shape::line(points, Stroke::new(5.0f32, light.gamma_multiply(fade))));
        painter.text(c + vec2(0.0, -radius - 14.0), Align2::CENTER_CENTER, format!("{:.0}%", fraction * 100.0), FontId::proportional(12.0), Color32::from_white_alpha((200.0 * fade) as u8));
    }
    // What G does at what's looked at, and what a tool does or would do for it
    if let Some(p) = &building.prompt {
        let res = crate::materials::res_of(p.material).map(|r| r.name());
        let (verb, gives) = match (p.kind, p.tree) {
            (Yield::Wood, true) => (["chop tree", "Chopping tree"], Some("Wood")),
            (Yield::Wood, false) => (["cut wood", "Cutting wood"], Some("Wood")),
            (Yield::Stone, _) => (["break stone", "Breaking stone"], res),
            (Yield::Cut, _) => (["cut stone", "Cutting stone"], res),
            (Yield::Ore, _) => (["mine ore", "Mining ore"], res),
            (Yield::Clay, _) => (["dig clay", "Digging clay"], Some("Clay")),
            (Yield::Sand, _) => (["dig sand", "Digging sand"], Some("Sand")),
            (Yield::Plant, _) => (["cut", "Cutting"], res),
            _ => (["dig earth", "Digging earth"], None),
        };
        let busy = working.is_some_and(|g| g.key == p.key && building.since_blow < 0.3);
        let line = match (busy, gives) {
            (true, _) => format!("{}…", verb[1]),
            (false, Some(gives)) => format!("Hold G — {} ({gives})", verb[0]),
            (false, None) => format!("Hold G — {} (gives nothing)", verb[0]),
        };
        let at = c + vec2(0.0, 36.0);
        if gives.is_some() {
            // On a dark backing, a swatch of what it gives before it, and under it what the tool
            // for it does, or would
            let main = painter.layout_no_wrap(line, FontId::proportional(18.0), Color32::WHITE);
            let tool = p.tool.as_ref().map(|(text, carried)| painter.layout_no_wrap(text.clone(), FontId::proportional(13.5), if *carried { Color32::from_rgb(150, 235, 125) } else { Color32::from_gray(205) }));
            let swatch = 18.0;
            let wide = (main.size().x + swatch).max(tool.as_ref().map_or(0.0, |t| t.size().x));
            let high = main.size().y + tool.as_ref().map_or(0.0, |t| t.size().y + 2.0);
            painter.rect_filled(egui::Rect::from_min_size(pos2(c.x - wide * 0.5 - 12.0, at.y - 4.0), vec2(wide + 24.0, high + 8.0)), 8.0, Color32::from_black_alpha(140));
            let left = c.x - (main.size().x + swatch) * 0.5;
            painter.rect_filled(egui::Rect::from_min_size(pos2(left, at.y + main.size().y * 0.5 - 5.0), vec2(10.0, 10.0)), 2.0, crate::materials::res_of(p.material).map_or(yield_color(p.kind), |r| r.color()));
            let main_high = main.size().y;
            painter.galley(pos2(left + swatch, at.y), main, Color32::WHITE);
            if let Some(tool) = tool {
                painter.galley(pos2(c.x - tool.size().x * 0.5, at.y + main_high + 2.0), tool, Color32::WHITE);
            }
        } else {
            // Earth's said quieter, there being so much of it, and nothing in it to keep
            boxed(&painter, at, &line, 14.0, Color32::from_white_alpha(185), 80);
        }
    }
    // What's just been gathered, rising off the crosshair and fading
    for (k, popup) in building.popups.iter().enumerate() {
        let t = (popup.age / POPUP_SECONDS) as f32;
        let alpha = (1.0 - (t - 0.6).max(0.0) / 0.4).clamp(0.0, 1.0);
        let grow = 1.0 + 0.25 * (1.0 - (t * 6.0).min(1.0));
        let at = c + vec2(0.0, -70.0 - 70.0 * t.sqrt() - 40.0 * (building.popups.len() - 1 - k) as f32);
        let color = Color32::from_rgba_unmultiplied(popup.color.r().saturating_add(60), popup.color.g().saturating_add(60), popup.color.b().saturating_add(60), (255.0 * alpha) as u8);
        boxed(&painter, at, &popup.text, 30.0 * grow, color, (150.0 * alpha) as u8);
    }
    // The hint showing, at the left, faded in and out
    if let Some(card) = building.hints.front() {
        let alpha = ((card.age / 0.35).min(1.0) * ((HINT_SECONDS - card.age) / 0.8).clamp(0.0, 1.0)) as f32;
        let wrap = 380.0;
        let tint = |c: Color32| c.gamma_multiply(alpha);
        let title = painter.layout(card.title.clone(), crate::theme::display(17.0), tint(crate::theme::HEADING), wrap);
        let lines: Vec<_> = card.lines.iter().map(|l| painter.layout(l.clone(), FontId::proportional(14.5), tint(Color32::from_gray(230)), wrap)).collect();
        let high = title.size().y + 8.0 + lines.iter().map(|g| g.size().y + 6.0).sum::<f32>();
        let at = pos2(screen.left() + 24.0, (c.y - 60.0 - high * 0.5).max(screen.top() + 200.0));
        let card_rect = egui::Rect::from_min_size(at - vec2(14.0, 12.0), vec2(wrap + 28.0, high + 22.0));
        painter.rect(card_rect, 10.0, crate::theme::panel_fill().gamma_multiply(alpha * 0.9), Stroke::new(1.0f32, crate::theme::BRONZE.gamma_multiply(alpha)), egui::StrokeKind::Inside);
        painter.rect_filled(egui::Rect::from_min_size(card_rect.min, vec2(4.0, card_rect.height())), 2.0, tint(Color32::from_rgb(255, 215, 130)));
        painter.text(card_rect.right_top() + vec2(-10.0, 8.0), Align2::RIGHT_TOP, "hint", FontId::proportional(11.0), tint(Color32::from_gray(140)));
        let mut y = at.y;
        let height = title.size().y;
        painter.galley(pos2(at.x, y), title, Color32::WHITE);
        y += height + 8.0;
        for line in lines {
            let height = line.size().y;
            painter.galley(pos2(at.x, y), line, Color32::WHITE);
            y += height + 6.0;
        }
    }
    if !building.on {
        return;
    }
    // The tabs of pieces along the top of the list, by what they're made of, the one open lit up
    // and each saying how many of its pieces can be afforded now
    let tab = building.tab.min(TABS.len() - 1);
    // Left of what's carried, which runs down the right side
    let x = screen.right() - 240.0 - 360.0;
    let width = 354.0;
    // The pieces of the tab, down the right side, and the gate or a portal home after them where
    // they can be built, see gates.rs
    let mut entries: Vec<(usize, &str, Materials)> = TABS[tab].1.iter().map(|k| (Kind::ALL.iter().position(|a| a == k).unwrap_or(0), k.name(), k.cost())).collect();
    match gate {
        Some(false) => entries.push((Kind::ALL.len(), "Innert Gate", crate::gates::GATE_COST)),
        Some(true) => entries.push((Kind::ALL.len(), "Portal Home", crate::gates::PORTAL_COST)),
        None => {}
    }
    let mut y = (screen.center().y - 170.0).max(screen.top() + 260.0);
    let tab_rows = TABS.len().div_ceil(3) as f32;
    let list = egui::Rect::from_min_size(pos2(x - 10.0, y - 30.0), vec2(width + 10.0, 34.0 + tab_rows * 26.0 + 8.0 + entries.len() as f32 * 26.0 + 58.0));
    painter.rect(list, 10.0, Color32::from_rgba_unmultiplied(14, 11, 8, 190), Stroke::new(1.0f32, crate::theme::BRONZE), egui::StrokeKind::Inside);
    crate::theme::accent(&painter, list, 1.0);
    painter.text(pos2(x, y - 14.0), Align2::LEFT_CENTER, "Building", crate::theme::display(16.0), crate::theme::HEADING);
    painter.text(pos2(x + width - 14.0, y - 14.0), Align2::RIGHT_CENTER, "C next tab  ·  B to stop", FontId::proportional(12.0), crate::theme::WEAK);
    let cell = (width - 14.0) / 3.0;
    for (k, (title, kinds)) in TABS.iter().enumerate() {
        let at = pos2(x + (k % 3) as f32 * cell, y + (k / 3) as f32 * 26.0 + 2.0);
        let open = k == tab;
        let rect = egui::Rect::from_min_size(at + vec2(-4.0, 0.0), vec2(cell - 4.0, 22.0));
        // The open one as what's picked is everywhere, the rest as buttons
        if open {
            painter.rect(rect, 5.0, Color32::from_rgb(112, 80, 36), Stroke::new(1.2f32, crate::theme::ACCENT), egui::StrokeKind::Inside);
        } else {
            painter.rect(rect, 5.0, Color32::from_rgba_unmultiplied(52, 42, 32, 200), Stroke::new(1.0f32, Color32::from_rgb(98, 76, 50)), egui::StrokeKind::Inside);
        }
        let can = kinds.iter().filter(|kind| bases.materials.covers(kind.cost())).count();
        painter.text(rect.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, *title, FontId::proportional(13.0), if open { Color32::WHITE } else { crate::theme::TEXT.gamma_multiply(0.85) });
        painter.text(rect.right_center() - vec2(6.0, 0.0), Align2::RIGHT_CENTER, format!("{can}/{}", kinds.len()), FontId::proportional(11.0), if can > 0 { Color32::from_rgb(150, 225, 140) } else { Color32::from_gray(120) });
    }
    y += tab_rows * 26.0 + 8.0;
    // Each with its key and what it costs, greyed out till there's enough for it, the colour of
    // each of what it costs by it
    let have = bases.materials;
    for (k, &(index, name, cost)) in entries.iter().enumerate() {
        let picked = index == building.picked;
        let afford = have.covers(cost);
        if picked {
            painter.rect_filled(egui::Rect::from_min_size(pos2(x - 6.0, y + 2.0), vec2(width - 4.0, 24.0)), 6.0, Color32::from_rgba_unmultiplied(255, 220, 160, if afford { 34 } else { 16 }));
            painter.rect_filled(egui::Rect::from_min_size(pos2(x - 6.0, y + 2.0), vec2(3.0, 24.0)), 1.5, if afford { Color32::from_rgb(110, 240, 140) } else { Color32::from_rgb(250, 90, 80) });
        }
        let grey = Color32::from_gray(125);
        let color = if afford { Color32::from_gray(if picked { 255 } else { 210 }) } else { grey };
        // The gate in gold, being what all of it's for
        let color = if index >= Kind::ALL.len() && afford { Color32::from_rgb(255, 215, 130) } else { color };
        let key = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"].get(k).copied().unwrap_or(" ");
        painter.text(pos2(x, y + 14.0), Align2::LEFT_CENTER, format!("{key}  {name}"), FontId::proportional(14.0), color);
        // What it costs, from the right: each in its colour if there's enough of it, grey if not
        let mut cx = x + width - 14.0;
        for (res, n) in cost.parts().into_iter().rev() {
            let text = format!("{n} {}", res.name().to_lowercase());
            let enough = have.get(res) >= n;
            let galley = painter.layout_no_wrap(text, FontId::proportional(12.5), if enough { Color32::from_gray(225) } else { Color32::from_rgb(225, 135, 120) });
            let w = galley.size().x;
            painter.galley(pos2(cx - w, y + 14.0 - galley.size().y * 0.5), galley, Color32::WHITE);
            painter.rect_filled(egui::Rect::from_center_size(pos2(cx - w - 8.0, y + 14.0), vec2(8.0, 8.0)), 1.5, if enough { res.color() } else { res.color().gamma_multiply(0.45) });
            cx -= w + 20.0;
        }
        y += 26.0;
    }
    painter.text(pos2(x, y + 16.0), Align2::LEFT_CENTER, "Click build  ·  Right click take down  ·  C tab", FontId::proportional(12.5), crate::theme::WEAK);
    painter.text(pos2(x, y + 34.0), Align2::LEFT_CENTER, "R turn  ·  Wheel pick  ·  E door, mend, smelt  ·  G gather", FontId::proportional(12.5), crate::theme::WEAK);

    // Where it'd go: a box round it, green if it can, red if not, and why not
    let Some(ghost) = ghost else {
        return;
    };
    let origin = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
    let on_screen = |p: Vector3<f64>| view.project(p).map(|[u, v]| pos2(screen.left() + u as f32 * screen.width(), screen.top() + v as f32 * screen.height()));
    let color = if ghost.ok().is_ok() { Color32::from_rgb(110, 240, 140) } else { Color32::from_rgb(250, 90, 80) };
    // A box by its eight corners, faint over its faces and drawn round their edges
    let draw_box = |corner: &dyn Fn(usize) -> Vector3<f64>| {
        let corners: Vec<Option<Pos2>> = (0..8).map(|k| on_screen(corner(k))).collect();
        for face in [[0, 1, 3, 2], [4, 5, 7, 6], [0, 1, 5, 4], [2, 3, 7, 6], [0, 2, 6, 4], [1, 3, 7, 5]] {
            let points: Option<Vec<Pos2>> = face.iter().map(|&k| corners[k]).collect();
            if let Some(points) = points {
                painter.add(egui::Shape::convex_polygon(points.clone(), color.gamma_multiply(0.08), Stroke::NONE));
                for i in 0..4 {
                    painter.line_segment([points[i], points[(i + 1) % 4]], Stroke::new(1.5f32, color.gamma_multiply(0.9)));
                }
            }
        }
    };
    match ghost {
        Ghost::Piece { piece, .. } => {
            let blocks = piece.blocks();
            let min = |i: usize| blocks.iter().map(|b| b[i]).min().unwrap_or(0);
            let max = |i: usize| blocks.iter().map(|b| b[i]).max().unwrap_or(0) + 1;
            let lo = Vector3::new(min(0), min(1), min(2)).cast::<f64>() * BLOCK as f64 - origin;
            let hi = Vector3::new(max(0), max(1), max(2)).cast::<f64>() * BLOCK as f64 - origin;
            let lo = if piece.kind == Kind::Floor { Vector3::new(lo.x, lo.y, lo.z + 4.0) } else { lo };
            draw_box(&|k: usize| Vector3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }));
        }
        // Its dais on the ground, the arch standing on it, and a step out in front of it
        &Ghost::Gate { at, facing, home, .. } => {
            let vpm = world.voxels_per_meter as f64;
            let (c, s) = (facing.cos(), facing.sin());
            let turned = |x: f64, y: f64, z: f64| at + Vector3::new(x * c - y * s, x * s + y * c, z) * vpm;
            let shape = |lo: [f64; 3], hi: [f64; 3]| move |k: usize| turned(if k & 1 == 0 { lo[0] } else { hi[0] }, if k & 2 == 0 { lo[1] } else { hi[1] }, if k & 4 == 0 { lo[2] } else { hi[2] });
            let [fx, fy] = crate::gates::FOOT;
            let (arch, high) = if home { (1.5, 5.6) } else { (1.75, 6.6) };
            draw_box(&shape([-fx, -fy, 0.0], [fx, fy, 0.3]));
            draw_box(&shape([-0.45, -arch, 0.3], [0.45, arch, high]));
            draw_box(&shape([fx, -0.9, 0.0], [fx + 0.6, 0.9, 0.15]));
        }
    }
    if let Err(why) = ghost.ok() {
        boxed(&painter, screen.center() + vec2(0.0, 30.0), why, 15.0, Color32::from_rgb(255, 150, 135), 170);
    }
}

/// How far the `away` script goes each frame going off a strip at a time and back, in meters, and
/// for how many frames
const AWAY_STEP_M: f64 = 0.05;
const AWAY_STEPS: usize = 3300;

/// The screenshot scripts of building and gathering, see App::bases_script()
pub const SCRIPTS: [&str; 15] = ["chop", "mine", "hut", "away", "moon", "raid", "saved", "clay", "gate", "portal", "tools", "hearth", "gather", "manor", "kit"];

impl App {
    /// Open, level ground near `near`, in voxels of the world, `across` blocks each way with room
    /// over it: the block of the whole landscape at its corner, just over the ground
    pub(super) fn open_ground(&self, near: Vector3<f64>, across: i64) -> Option<[i64; 3]> {
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

    /// A house of the new materials with its corner at block `at`, six blocks a side, its front
    /// facing -y: marble pillars at its corners, cut stone walls with glass windows in them, birch
    /// over them and dark oak over the windows, an iron gate in its front, a thatched roof sloping
    /// up from front and back to a ridge, its gable ends of pine; a cobbled floor inside with a rug
    /// on it, a banner over the gate, a lamp post and a fence out front and a furnace at its side
    fn build_manor(&mut self, at: [i64; 3]) -> usize {
        let (side, high) = (6, 5);
        let mut count = 0;
        let mut put = |app: &mut App, kind: Kind, b: [i64; 3], turn: u8| count += app.script_build(kind, b, turn) as usize;
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
                    if bases::read_brick(&self.world, local).iter().filter(|&&m| !worldgen::is_passable(m)).count() > 384 {
                        break;
                    }
                    put(self, Kind::CutStoneWall, b, 0);
                }
                let corner = (i == 0 || i == side - 1) && (j == 0 || j == side - 1);
                if corner {
                    put(self, Kind::MarblePillar, [at[0] + i, at[1] + j, at[2]], 0);
                    put(self, Kind::MarbleWall, [at[0] + i, at[1] + j, at[2] + 4], 0);
                    continue;
                }
                // Facing out of the wall it's in, for the windows
                let turn = if j == 0 { 3 } else if j == side - 1 { 1 } else if i == 0 { 2 } else { 0 };
                let window = (i == 1 || i == side - 2 || j == 1 || j == side - 2) && !(j == 0 && (2..=3).contains(&i));
                for k in 0..high {
                    if j == 0 && (i == 2 || i == 3) && k < 4 {
                        continue;
                    }
                    let kind = match k {
                        0 => Kind::CutStoneWall,
                        1 | 2 if window => Kind::GlassWindow,
                        1 => Kind::CutStoneWall,
                        2 => Kind::BirchWall,
                        3 if window => Kind::DarkWall,
                        _ => Kind::BirchWall,
                    };
                    put(self, kind, [at[0] + i, at[1] + j, at[2] + k], turn);
                }
            }
        }
        put(self, Kind::IronGate, [at[0] + 2, at[1], at[2]], 3);
        // The roof: thatch rising from front and back to a ridge, a block out past the walls each
        // way, its gable ends of pine boards
        for r in 0..=side / 2 {
            for i in -1..=side {
                put(self, Kind::ThatchRoof, [at[0] + i, at[1] - 1 + r, at[2] + high + r], 1);
                put(self, Kind::ThatchRoof, [at[0] + i, at[1] + side - r, at[2] + high + r], 3);
            }
            for i in [0, side - 1] {
                for j in r..side - r {
                    put(self, Kind::PineWall, [at[0] + i, at[1] + j, at[2] + high + r], 0);
                }
            }
        }
        // Inside, a cobbled floor and a rug on it
        for j in 1..side - 1 {
            for i in 1..side - 1 {
                put(self, Kind::CobbleFloor, [at[0] + i, at[1] + j, at[2]], 0);
            }
        }
        for (i, j) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            put(self, Kind::Rug, [at[0] + i, at[1] + j, at[2] + 1], 0);
        }
        // Out front: a banner over the gate, a lamp post, a fence, iron bars in a stone sill; and a
        // furnace by its side
        put(self, Kind::Banner, [at[0] + 3, at[1] - 1, at[2] + 3], 3);
        put(self, Kind::LampPost, [at[0] + 1, at[1] - 2, at[2]], 0);
        for i in [-2, -1, 4, 5, 6, 7] {
            put(self, Kind::Fence, [at[0] + i, at[1] - 3, at[2]], 1);
            put(self, Kind::Fence, [at[0] + i, at[1] - 3, at[2] + 1], 1);
        }
        put(self, Kind::Furnace, [at[0] + side, at[1] + 2, at[2]], 0);
        put(self, Kind::SlateRoof, [at[0] + side, at[1] + 2, at[2] + 2], 0);
        count
    }

    /// See bases_script(): `kit`, with `--set gui=1`, the Craft tab with plenty of everything to
    /// make every tool at every tier, then a legendary steel axe's card
    fn kit_script(&mut self, frame: usize) {
        match frame {
            5 => {
                let all = Res::ALL.iter().fold(Materials::new(0, 0, 0), |m, &r| m.with(r, 40));
                self.bases.materials.add(all);
                self.inventory_open = true;
                self.building.craft_open = true;
            }
            60 => self.shoot_soon("the Craft tab, every tool at every tier"),
            80 => {
                self.building.craft_open = false;
                let rarity = settings_from_args().into_iter().find(|(k, _)| k == "kit_rarity").and_then(|(_, v)| v.parse().ok()).unwrap_or(4);
                if let Some(axe) = tools::make_rare(self.game.library(), Tool::Hatchet, tools::STEEL_TIER, rarity, 0x5eed_1e9e) {
                    println!("frame {frame}: made {}: {:?}", axe.name, tools::gear(self.game.library(), &axe));
                    self.game.give_weapon(axe);
                    let slot = self.game.slot_count() - 1;
                    self.game.inspect(slot);
                }
            }
            140 => self.shoot_soon("a legendary tool's card"),
            _ => {}
        }
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
            // Going on from a save: once the world's built round the player, says what's built, what's
            // carried and the time, whether every block of what's built is in the world, and takes
            // a shot of it
            ("saved", f) if f >= 30 && self.building.phase == 0 => {
                let settled = !self.held_still() && self.stream.as_ref().is_none_or(|s| s.teleport.is_none());
                if !settled {
                    return;
                }
                let first = self.bases.pieces().first().map(|p| p.at);
                println!(
                    "frame {frame}: loaded {} pieces, carrying {:?}, {:.2} o'clock on day {}, the time {}, blocks missing {}",
                    self.bases.pieces().len(),
                    self.bases.materials,
                    self.cycle.hour(),
                    self.cycle.clock.day,
                    if self.cycle.clock.running { "going on" } else { "held" },
                    first.map_or(0, |at| self.missing_blocks(at))
                );
                if let Some(at) = first {
                    self.look_at_hut(at, -1.0);
                }
                self.building.phase = 1;
                self.building.mark = f;
            }
            ("saved", f) if self.building.phase == 1 && f == self.building.mark + 40 => {
                self.shoot_soon("what was built, back from the save");
                self.building.phase = 2;
            }
            // Clay where it lies, for its colour, the nearest of the kind `--set clay=` says: `beach`
            // on a shore just over the water, `shallows` under shallow water, `cave` in a cave
            ("clay", f) if f >= 20 && self.building.phase == 0 => {
                if self.held_still() {
                    return;
                }
                let want = settings_from_args().into_iter().find(|(k, _)| k == "clay").map_or("beach".to_owned(), |(_, v)| v);
                match self.find_clay(feet, &want) {
                    Some((at, from)) => {
                        println!("frame {frame}: clay ({want}) at {:.1}, {:.1}, {:.1} m, {:.0} m off", (at.x + origin.x) / vpm, (at.y + origin.y) / vpm, at.z / vpm, (at - feet).xy().norm() / vpm);
                        // A torch stood on it in a cave, as the player would to see by
                        if want == "cave" {
                            self.bases.materials.add(bases::Materials::new(2, 0, 0));
                            let block = self.block_of([at.x as i64, at.y as i64, at.z as i64 + 1]);
                            self.script_build(Kind::Torch, block, 0);
                        }
                        self.look_from(from, at);
                    }
                    None => println!("frame {frame}: no clay ({want}) found near"),
                }
                self.building.phase = 1;
                self.building.mark = f;
            }
            ("clay", f) if self.building.phase == 1 && f == self.building.mark + 60 => {
                self.shoot_soon("clay");
                self.building.phase = 2;
            }
            // A raid on a hut with the player shut in it: the camera off to one side of its door,
            // a shot as it's first struck, once it's half broken, and as it gives way
            // A fireplace in a hut, its chimney up through the roof, stocked with four logs, see
            // hearths.rs: `--set hearth_view=out` looks at the hut from outside, `in` at the fire
            // from just inside the door, `cook` roasts meat over it from there and `fish` cooks fish
            ("hearth", 3) => {
                self.bases.materials.add(bases::Materials::new(400, 300, 120));
                let ahead = Vector3::new(-self.camera.rotation.z.sin(), self.camera.rotation.z.cos(), 0.0);
                let Some(at) = self.open_ground(feet + ahead * 9.0 * vpm, 6) else {
                    println!("frame {frame}: nowhere open to build");
                    return;
                };
                let count = self.build_hut(at);
                // The torch inside taken away, so the fire lights the room, and the roof where the
                // chimney goes up through it
                for b in [[at[0] + 4, at[1] + 4, at[2] + 1], [at[0] + 2, at[1] + 4, at[2] + 5], [at[0] + 3, at[1] + 4, at[2] + 5]] {
                    if let Some(id) = self.bases.piece_at(b).map(|p| p.id) {
                        self.bases.remove(id, false);
                    }
                }
                let hearth = [at[0] + 2, at[1] + 4, at[2] + 1];
                let built = [self.bases.build(Kind::Hearth, hearth, 3).is_ok(), self.bases.build(Kind::Chimney, [hearth[0], hearth[1], hearth[2] + 2], 3).is_ok(), self.bases.build(Kind::Chimney, [hearth[0], hearth[1], hearth[2] + 4], 3).is_ok()];
                // `--set hearth_logs=0` leaves it unlit, to see what the fire costs
                let logs = settings_from_args().into_iter().find(|(k, _)| k == "hearth_logs").and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(4.0);
                if let Some(id) = self.bases.piece_at(hearth).map(|p| p.id).filter(|_| logs > 0.0) {
                    self.bases.add_fuel(id, logs * crate::hearths::LOG_SECONDS, crate::hearths::MOST_SECONDS);
                }
                self.refresh_bases();
                self.building.hut = Some(at);
                println!("frame {frame}: a hut of {count} pieces, its fireplace and chimney built {built:?}, burning {:?} s", self.bases.piece_at(hearth).map(|p| p.fuel));
                let view = settings_from_args().into_iter().find(|(k, _)| k == "hearth_view").map_or("in".to_owned(), |(_, v)| v);
                if view == "out" {
                    self.look_at_hut(at, -1.0);
                } else {
                    // Just inside the door, looking at the fire
                    let inside = Vector3::new(((at[0] + 4) * BLOCK) as f64 + 4.0, ((at[1] + 1) * BLOCK) as f64 + 2.0, ((at[2] + 1) * BLOCK) as f64) - origin;
                    let fire = crate::hearths::fire_at(self.bases.piece_at(hearth).unwrap()) - origin;
                    self.look_from(inside + Vector3::new(0.0, 0.0, 1.35 * vpm), fire + Vector3::new(0.0, 0.0, 0.45 * vpm));
                }
                self.building.phase = 1;
                self.building.mark = frame;
            }
            ("hearth", f) if self.building.phase == 1 => {
                let since = f - self.building.mark;
                // `hearth_view=fish` cooks raw fish instead, see animals\sealife.rs
                let fish = settings_from_args().iter().any(|(k, v)| k == "hearth_view" && v == "fish");
                let cook = fish || settings_from_args().iter().any(|(k, v)| k == "hearth_view" && v == "cook");
                match since {
                    40 if cook => {
                        self.game.hand(if fish { crate::game::Item::RawFish } else { crate::game::Item::Meat }, 3);
                    }
                    // Every one of the fish, so the cooked are in hand after
                    70 | 90 if cook => {
                        self.hearth_here();
                        println!("frame {frame}: {}", self.game.last_message().unwrap_or(""));
                    }
                    110 if fish => {
                        self.hearth_here();
                        println!("frame {frame}: {}", self.game.last_message().unwrap_or(""));
                    }
                    120 => {
                        let fire = self.bases.pieces().iter().find(|p| p.kind == Kind::Hearth).map(|p| p.fuel);
                        println!("frame {frame}: the fire has {fire:?} s left, the note {:?}", self.hearth_note());
                        self.shoot_soon(if cook { "roasting over the fire" } else { "the fire burning" });
                    }
                    _ => {}
                }
            }
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
            // Gathering with tools in hand, see tools.rs: a crude hatchet at a tree, a sturdy
            // pickaxe at bare rock and a fine spade at clay, each swung till it's gathered, then the
            // inventory's Craft tab
            ("tools", f) => self.tools_script(f),
            ("chop", 5) => {
                let trees = self.trees_near(feet, 30.0);
                // `--set chop_tree=big` the biggest near, rather than the nearest
                let big = settings_from_args().iter().any(|(k, v)| k == "chop_tree" && v == "big");
                let pick = if big { trees.iter().filter(|t| t.1 > 3.0).max_by(|a, b| a.1.total_cmp(&b.1)) } else { trees.iter().find(|t| t.1 > 3.0) };
                let Some(&(foot, height, _)) = pick else {
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
            // G held at it, bare handed, till it comes down
            ("chop", f) if (30..=150).contains(&f) && self.building.felling.falls.is_empty() && self.building.phase == 0 => {
                if !self.building.felling.busy() {
                    self.gather_hold(1.0 / 30.0, f == 30);
                }
                if f % 24 == 6 {
                    println!("frame {frame}: holding G, {:.0}% through, {}, carrying {:?}", self.building.gathering.as_ref().map_or(0.0, |g| g.done * 100.0), self.last_popup(), self.bases.materials);
                }
            }
            // Once it's falling, seen from off to its side, a shot every few frames as it comes down
            // and after it's down, see felling.rs
            ("chop", f) if self.building.phase == 0 && !self.building.felling.falls.is_empty() => {
                let fall = &self.building.felling.falls[0];
                let foot = fall.point(0.0, 0.0) - origin;
                let (dir, height, size, model, scale) = (fall.plan.dir, fall.plan.height, fall.plan.size, fall.built.model.size, fall.built.scale);
                let side = Vector3::new(-dir.y, dir.x, 0.0);
                let middle = foot + Vector3::new(dir.x, dir.y, 0.0) * height * 0.45 + Vector3::new(0.0, 0.0, height * 0.35);
                let eye = middle + side * (height * 0.95 + 3.0 * vpm) - Vector3::new(dir.x, dir.y, 0.0) * height * 0.2 + Vector3::new(0.0, 0.0, 0.5 * vpm);
                self.look_from(eye, middle);
                println!("frame {frame}: a tree {:.1} m tall of {size} voxels starts coming down, its model {model:?} voxels at {scale} to one", height / vpm);
                self.building.phase = 1;
                self.building.mark = f;
            }
            ("chop", f) if self.building.phase == 1 => {
                let since = f - self.building.mark;
                if let Some(fall) = self.building.felling.falls.first() {
                    if since % 6 == 0 && since <= 90 {
                        println!("frame {frame}: {:.2} rad over, going {:.2} rad/s{}", fall.topple.angle, fall.topple.speed, if fall.topple.landed.is_some() { ", down" } else { "" });
                    }
                }
                // `--set chop_shots=1` for the shots, which hold up the frames they're taken in
                if since <= 84 && since % 12 == 0 && settings_from_args().iter().any(|(k, v)| k == "chop_shots" && v == "1") {
                    self.shoot_soon("the tree coming down");
                }
                if since == 130 {
                    println!("frame {frame}: carrying {:?}, {}", self.bases.materials, self.last_popup());
                    self.building.phase = 2;
                }
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
            ("mine", f) if (20..=320).contains(&f) => {
                let (eye, look) = self.eye_look();
                let hit = gather::raycast(&self.world, eye, look, REACH_M * vpm).map(|(v, _)| self.world.material_at(v[0], v[1], v[2]));
                // Bare hands at first, then with a pickaxe, quicker, see tools.rs
                if f == 170 {
                    self.game.give_tool(Tool::Pickaxe, tools::CRUDE);
                }
                self.gather_hold(1.0 / 30.0, f == 20);
                if f % 48 == 0 {
                    println!("frame {frame}: struck {:?}, {}, carrying {:?}", hit, self.last_popup(), self.bases.materials);
                }
            }
            // What gathering and building say to a new player, see gather_script()
            ("gather", f) => self.gather_script(f),
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
            // A house of the new materials, see manor(): `--set manor_view=in` from inside its gate,
            // `menu` the build menu over it, open at its stone tab
            ("manor", 3) => {
                let all = Res::ALL.iter().fold(Materials::new(0, 0, 0), |m, &r| m.with(r, 500));
                self.bases.materials.add(all);
                let ahead = Vector3::new(-self.camera.rotation.z.sin(), self.camera.rotation.z.cos(), 0.0);
                let Some(at) = self.open_ground(feet + ahead * 9.0 * vpm, 8) else {
                    println!("frame {frame}: nowhere open to build");
                    return;
                };
                let started = Instant::now();
                let count = self.build_manor(at);
                println!("frame {frame}: a house of {count} pieces at block {at:?} in {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
                self.building.hut = Some(at);
                let view = settings_from_args().into_iter().find(|(k, _)| k == "manor_view").map_or("out".to_owned(), |(_, v)| v);
                match view.as_str() {
                    "in" => {
                        let gate = Vector3::new((at[0] + 3) as f64, at[1] as f64, at[2] as f64) * BLOCK as f64 - origin;
                        self.look_from(gate + Vector3::new(0.0, -2.5 * vpm, 1.6 * vpm), gate + Vector3::new(0.0, 3.0 * vpm, 1.0 * vpm));
                    }
                    "side" => {
                        let middle = Vector3::new((at[0] + 3) as f64, (at[1] + 3) as f64, at[2] as f64) * BLOCK as f64 - origin;
                        self.look_from(middle + Vector3::new(-7.0 * vpm, -6.0 * vpm, 3.5 * vpm), middle + Vector3::new(0.0, 0.0, 1.5 * vpm));
                    }
                    _ => self.look_at_hut(at, -1.0),
                }
                if view == "menu" {
                    self.building.on = true;
                    self.pick_tab(2);
                }
                self.building.phase = 1;
                self.building.mark = frame;
            }
            // Its gate opened, to see in
            ("manor", f) if self.building.phase == 1 && f == self.building.mark + 60 && settings_from_args().iter().any(|(k, v)| k == "manor_view" && v == "in") => {
                if let Some(at) = self.building.hut
                    && let Some(id) = self.bases.piece_at([at[0] + 2, at[1], at[2]]).map(|p| p.id)
                {
                    let open = self.bases.toggle_door(id).map(|(open, _)| open);
                    self.refresh_bases();
                    println!("frame {frame}: the gate's open: {open:?}");
                }
            }
            ("manor", f) if self.building.phase == 1 && f == self.building.mark + 150 => {
                self.shoot_soon("the house of the new materials");
                self.building.phase = 2;
            }
            // The Craft tab with every tool at every tier, then a legendary axe's card
            ("kit", f) => self.kit_script(f),
            // Building the gate and a portal home, see gates.rs
            ("gate" | "portal", f) => self.gate_script(name, f),
            _ => {}
        }
        if frame % 120 == 0 && matches!(name, "hut" | "away") {
            let lost = self.building.hut.map_or(0, |at| self.missing_blocks(at));
            println!("frame {frame}: pieces {}, blocks of the hut missing from the world {lost}", self.bases.pieces().len());
        }
    }

    /// Takes a shot in a moment, in a screenshot run, saying so
    pub(super) fn shoot_soon(&mut self, what: &str) {
        if let Some(capture) = &mut self.capture {
            let frame = capture.frame_times.len() + 2;
            capture.shots.push(frame);
            println!("SHOT at frame {frame}: {what}");
        }
    }

    /// See bases_script(): `tools`, played at `--step 0.0333`
    fn tools_script(&mut self, frame: usize) {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        // Each looked for near where the player first stood
        let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        if frame <= 5 {
            self.building.spot = Some(feet);
        }
        let feet = self.building.spot.unwrap_or(feet);
        // Up close to it, from the way `from` is, and the tool to swing at it in hand
        let take_up = |app: &mut App, tool: Tool, grade: usize, at: Vector3<f64>, from: Vector3<f64>| {
            // Ground's looked down on from over it, a trunk from beside it
            let back = Vector3::new(from.x - at.x, from.y - at.y, 0.0).normalize();
            let eye = if from.z - at.z > vpm { at + back * 0.5 * vpm + Vector3::new(0.0, 0.0, 1.7 * vpm) } else { at + (from - at).normalize() * 2.3 * vpm };
            app.look_from(eye, at);
            app.game.give_tool(tool, grade);
            app.game.select_family(tool.family());
            println!("frame {frame}: a {} {} in hand {:?}, at {:.1}, {:.1}, {:.1} m, carrying {:?}", tools::TIERS[grade], tool.name(), app.game.held_tool(), (at.x + origin.x) / vpm, (at.y + origin.y) / vpm, at.z / vpm, app.bases.materials);
        };
        let swinging = |f: usize, from: usize, to: usize| f > from + 10 && f < to - 10;
        match frame {
            5 => {
                let trees = self.trees_near(feet, 30.0);
                let Some(&(foot, height, _)) = trees.iter().find(|t| t.1 > 3.0) else {
                    println!("frame {frame}: no tree near");
                    return;
                };
                println!("frame {frame}: a tree {height:.1} m tall");
                let side = (0..8)
                    .map(|k| k as f64 / 8.0 * TAU)
                    .find(|a| {
                        let p = foot + Vector3::new(a.cos(), a.sin(), 0.0) * 2.2 * vpm + Vector3::new(0.0, 0.0, 1.5 * vpm);
                        worldgen::is_passable(self.world.material_at(p.x as i64, p.y as i64, p.z as i64))
                    })
                    .unwrap_or(0.0);
                let at = foot + Vector3::new(0.0, 0.0, 1.1 * vpm);
                take_up(self, Tool::Hatchet, tools::CRUDE, at, at + Vector3::new(side.cos(), side.sin(), 0.25) * vpm);
            }
            205 => match self.find_clay(feet, "stone") {
                Some((at, from)) => take_up(self, Tool::Pickaxe, tools::STURDY, at, from),
                None => println!("frame {frame}: no bare stone near"),
            },
            505 => match self.find_clay(feet, "beach") {
                Some((at, from)) => take_up(self, Tool::Spade, tools::FINE, at, from),
                None => println!("frame {frame}: no clay near"),
            },
            705 => {
                self.bases.materials.add(bases::Materials::new(30, 30, 12));
                self.inventory_open = true;
                self.building.craft_open = true;
            }
            f if f == 72 || f == 372 || f == 572 => self.shoot_soon("a tool's blow landing"),
            745 => self.shoot_soon("the Craft tab"),
            _ => {}
        }
        let swing = swinging(frame, 5, 205) || swinging(frame, 205, 505) || swinging(frame, 505, 705);
        self.script_controls = combat::Controls { attack: swing, holding: swing, ..Default::default() };
        if frame % 50 == 0 && frame < 700 {
            println!("frame {frame}: blows landed {}, carrying {:?}, {}", self.game.swings_landed(), self.bases.materials, self.game.last_message().unwrap_or(""));
        }
    }

    /// What was last gathered, as it rose off the crosshair, for the scripts to say
    fn last_popup(&self) -> &str {
        self.building.popups.last().map_or("", |p| p.text.as_str())
    }

    /// Looks down at the ground at `at` from a couple of meters off, as a player standing there to
    /// gather it would, from the way `from` is if the view's clear from there, otherwise from
    /// another way round that is, both in voxels of the world
    fn look_close(&mut self, at: Vector3<f64>, from: Vector3<f64>) {
        let vpm = self.world.voxels_per_meter as f64;
        let back = (from.y - at.y).atan2(from.x - at.x);
        // Nothing at all between the eye and it, not leaves nor tall grass
        let clear = |eye: Vector3<f64>| {
            (0..40).all(|k| {
                let p = eye + (at - eye) * (k as f64 / 40.0) * 0.92;
                self.world.material_at(p.x as i64, p.y as i64, p.z as i64) == worldgen::material::AIR
            })
        };
        let eye = (0..8)
            .map(|k| back + k as f64 * TAU / 8.0)
            .map(|a| at + Vector3::new(a.cos() * 2.0 * vpm, a.sin() * 2.0 * vpm, 1.9 * vpm))
            .find(|&eye| clear(eye))
            .unwrap_or(at + Vector3::new(back.cos() * 2.0 * vpm, back.sin() * 2.0 * vpm, 1.9 * vpm));
        self.look_from(eye, at);
    }

    /// See bases_script(): `gather`, played at `--step 0.0333` with `--set gui=1`, as a new player
    /// would first meet gathering and building, a shot of each thing the HUD says: bare handed at a
    /// tree, what G does there, the ring filling as it's held and the first hint, the wood rising
    /// off it and the hint about wood; at bare rock with a pickaxe carried, and at clay; then into
    /// building, its hint and the pieces with what they cost, and one picked that can't be afforded
    fn gather_script(&mut self, frame: usize) {
        let vpm = self.world.voxels_per_meter as f64;
        let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        if frame <= 5 {
            self.building.spot = Some(feet);
        }
        let feet = self.building.spot.unwrap_or(feet);
        let say = |app: &App, what: &str| {
            let card = app.building.hints.front().map_or(String::new(), |c| c.title.clone());
            println!("frame {frame}: {what}: carrying {:?}, the prompt {:?}, the hint {card:?}, {}", app.bases.materials, app.building.prompt.as_ref().map(|p| (p.kind, &p.tool)), app.last_popup());
        };
        match frame {
            5 => {
                self.bases.hints = 0;
                let trees = self.trees_near(feet, 30.0);
                let Some(&(foot, height, _)) = trees.iter().find(|t| t.1 > 3.0) else {
                    println!("frame {frame}: no tree near");
                    return;
                };
                let side = (0..8)
                    .map(|k| k as f64 / 8.0 * TAU)
                    .find(|a| {
                        let p = foot + Vector3::new(a.cos(), a.sin(), 0.0) * 2.2 * vpm + Vector3::new(0.0, 0.0, 1.5 * vpm);
                        worldgen::is_passable(self.world.material_at(p.x as i64, p.y as i64, p.z as i64))
                    })
                    .unwrap_or(0.0);
                let eye = foot + Vector3::new(side.cos(), side.sin(), 0.0) * 2.4 * vpm + Vector3::new(0.0, 0.0, 1.6 * vpm);
                self.look_from(eye, foot + Vector3::new(0.0, 0.0, 1.2 * vpm));
                println!("frame {frame}: by a tree {height:.1} m tall");
            }
            40 => {
                say(self, "looking at a tree");
                self.shoot_soon("what G does at a tree");
            }
            // Held at it till it comes down, bare handed
            f if (50..=150).contains(&f) && (f == 50 || self.building.gathering.is_some()) => {
                self.gather_hold(1.0 / 30.0, f == 50);
                if f == 95 {
                    say(self, "half way");
                    self.shoot_soon("the ring filling, and the first hint");
                }
            }
            146 => {
                say(self, "felled");
                self.shoot_soon("the wood rising off it");
            }
            200 => {
                say(self, "the hint after");
                self.shoot_soon("the hint about wood");
            }
            // Bare rock with a pickaxe carried
            210 => match self.find_clay(feet, "bare").or_else(|| self.find_clay(feet, "stone")) {
                Some((at, from)) => {
                    self.game.give_tool(Tool::Pickaxe, tools::CRUDE);
                    self.look_close(at, from);
                }
                None => println!("frame {frame}: no bare stone near"),
            },
            240 => {
                say(self, "looking at rock");
                self.shoot_soon("what G does at rock, a pickaxe carried");
            }
            250 => match self.find_clay(feet, "beach") {
                Some((at, from)) => self.look_close(at, from),
                None => println!("frame {frame}: no clay near"),
            },
            280 => {
                say(self, "looking at clay");
                self.shoot_soon("what G does at clay");
            }
            // Into building, looking at open ground, a fireplace picked with no stone
            290 => {
                if let Some(spot) = self.open_ground(feet, 2) {
                    let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
                    let at = Vector3::new((spot[0] * BLOCK) as f64, (spot[1] * BLOCK) as f64, (spot[2] * BLOCK) as f64) - origin + Vector3::new(4.0, 4.0, 0.0);
                    self.look_from(at + Vector3::new(-1.0 * vpm, -3.0 * vpm, 1.7 * vpm), at);
                }
                // The hint about wood read, gone
                self.building.hints.clear();
                self.toggle_building();
                self.pick(Kind::ALL.iter().position(|&k| k == Kind::Hearth).unwrap_or(0));
            }
            320 => {
                say(self, "building");
                self.shoot_soon("building, the pieces and what they cost, and its hint");
            }
            440 => {
                say(self, "short");
                self.shoot_soon("a piece that can't be afforded, and its hint");
            }
            // And what the menu says of it all
            455 => self.menu_open = true,
            470 => self.shoot_soon("the menu, gathering and building in it"),
            _ => {}
        }
    }

    /// The nearest clay of kind `want` to `near`, as the `clay` script has it, and where to look at
    /// it from, both in voxels of the world
    fn find_clay(&self, near: Vector3<f64>, want: &str) -> Option<(Vector3<f64>, Vector3<f64>)> {
        let vpm = self.world.voxels_per_meter as f64;
        let [sx, sy, sz] = self.world.size.map(|n| n as i64);
        let solid = |x: i64, y: i64, z: i64| !worldgen::is_passable(self.world.material_at(x, y, z));
        let top = |x: i64, y: i64| (0..sz).rev().find(|&z| solid(x, y, z));
        let mut best: Option<(f64, Vector3<f64>, Vector3<f64>)> = None;
        let reach = (60.0 * vpm) as i64;
        let (cx, cy) = (near.x as i64, near.y as i64);
        let step = 3;
        for y in ((cy - reach).max(8)..(cy + reach).min(sy - 8)).step_by(step) {
            for x in ((cx - reach).max(8)..(cx + reach).min(sx - 8)).step_by(step) {
                let d = ((x - cx) as f64).hypot((y - cy) as f64);
                if d < 5.0 * vpm || best.as_ref().is_some_and(|b| d >= b.0) {
                    continue;
                }
                let Some(z) = top(x, y) else { continue };
                let depth = self.water.depth_at(x as f64, y as f64);
                let at = Vector3::new(x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
                let back = (Vector3::new(cx as f64, cy as f64, 0.0) - Vector3::new(x as f64, y as f64, 0.0)).normalize();
                let found = match want {
                    "cave" => {
                        // Under the ground, clay in the floor or walls of a hollow
                        let mut z = z - 2 * vpm as i64;
                        let mut hit = None;
                        while z > 4 && hit.is_none() {
                            if !solid(x, y, z) {
                                let floor = (0..z).rev().find(|&k| solid(x, y, k));
                                if let Some(k) = floor
                                    && self.world.material_at(x, y, k) == worldgen::material::CLAY_BANK
                                    && (1..(1.4 * vpm) as i64).all(|h| !solid(x, y, k + h))
                                {
                                    hit = Some(k);
                                }
                                z = floor.unwrap_or(0);
                            }
                            z -= 1;
                        }
                        // Looked at from as far off across the hollow as it's open, up to 4 m
                        hit.map(|k| {
                            let clay = Vector3::new(x as f64 + 0.5, y as f64 + 0.5, k as f64 + 0.5);
                            let eye_z = k as f64 + 1.3 * vpm;
                            let open = |a: f64| {
                                let mut far = 0.0;
                                while far < 4.0 * vpm {
                                    let p = clay + Vector3::new(a.cos() * (far + 1.0), a.sin() * (far + 1.0), 0.0);
                                    if solid(p.x as i64, p.y as i64, eye_z as i64) || solid(p.x as i64, p.y as i64, eye_z as i64 + 4) {
                                        break;
                                    }
                                    far += 1.0;
                                }
                                far
                            };
                            let (a, far) = (0..16).map(|i| i as f64 * TAU / 16.0).map(|a| (a, open(a))).max_by(|p, q| p.1.total_cmp(&q.1)).unwrap();
                            (clay, Vector3::new(clay.x + a.cos() * far, clay.y + a.sin() * far, eye_z))
                        })
                    }
                    // Bare stone or rock on top, dry
                    "stone" => {
                        // A patch of it, not a speck in the grass
                        let rock = |x: i64, y: i64| top(x, y).is_some_and(|z| matches!(self.world.material_at(x, y, z), worldgen::material::STONE | worldgen::material::ROCK) && (1..5).all(|h| self.world.material_at(x, y, z + h) == worldgen::material::AIR));
                        let patch = [(-4, 0), (4, 0), (0, -4), (0, 4), (-3, -3), (3, 3), (0, 0)].iter().all(|&(dx, dy)| rock(x + dx, y + dy));
                        (patch && depth < 0.02 * vpm).then(|| (at, at + back * 5.0 * vpm + Vector3::new(0.0, 0.0, 2.5 * vpm)))
                    }
                    // Bare rock with nothing growing on it for a meter round, not even the shader's
                    // grass, open to the sky
                    "bare" => {
                        let grassless = |x: i64, y: i64| x >= 0 && y >= 0 && x < sx && y < sy && self.world.grass.as_ref().is_none_or(|g| (g[self.world.column(x, y)] >> 16) & 0xff == 0);
                        let rock = |x: i64, y: i64| grassless(x, y) && top(x, y).is_some_and(|z| matches!(self.world.material_at(x, y, z), worldgen::material::STONE | worldgen::material::ROCK) && (1..32).all(|h| self.world.material_at(x, y, z + h) == worldgen::material::AIR));
                        let r = vpm as i64;
                        let patch = (-2..=2).all(|j| (-2..=2).all(|i| rock(x + i * r / 2, y + j * r / 2)));
                        (patch && depth < 0.02 * vpm).then(|| (at, at + back * 5.0 * vpm + Vector3::new(0.0, 0.0, 2.5 * vpm)))
                    }
                    _ if self.world.material_at(x, y, z) != worldgen::material::CLAY_BANK => None,
                    "shallows" => (depth > 0.15 * vpm && depth < 0.8 * vpm).then(|| (at, at + back * 5.0 * vpm + Vector3::new(0.0, 0.0, 3.0 * vpm))),
                    _ => (depth < 0.02 * vpm).then(|| (at, at + back * 5.0 * vpm + Vector3::new(0.0, 0.0, 2.5 * vpm))),
                };
                if let Some((at, from)) = found {
                    best = Some((d, at, from));
                }
            }
        }
        // Looking from over the ground, not inside it
        best.map(|(_, at, mut from)| {
            if let Some(ground) = top(from.x as i64, from.y as i64).filter(|_| want != "cave") {
                from.z = from.z.max(ground as f64 + 1.6 * vpm);
            }
            (at, from)
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    // What's short of a cost says how much of each, and where the first of it's got
    #[test]
    fn shortfall_says_what_and_where() {
        assert_eq!(shortfall(Materials::new(1, 0, 0), Materials::new(3, 0, 0)), None);
        assert_eq!(shortfall(Kind::Hearth.cost(), Materials::new(0, 3, 0)), Some(("5 stone, 4 clay".to_owned(), Res::Stone.from())));
        assert_eq!(shortfall(Kind::Door.cost(), Materials::new(1, 9, 9)), Some(("3 wood".to_owned(), Res::Wood.from())));
        assert_eq!(shortfall(Kind::LampPost.cost(), Materials::new(1, 9, 9).with(Res::Iron, 1)), Some(("1 iron, 1 glass".to_owned(), Res::Iron.from())));
        // The first few say what they're for and where they're got
        for r in &Res::ALL[..ALWAYS] {
            assert!(!r.uses().is_empty() && r.from().starts_with("hold G"), "{r:?}");
        }
    }

    // Each hint has a bit of its own, so each is shown once and stays shown in the save
    #[test]
    fn hints_have_a_bit_each() {
        let all = [Hint::Gather, Hint::Wood, Hint::Build, Hint::Short];
        for (i, a) in all.iter().enumerate() {
            assert!((*a as u32).is_power_of_two(), "{a:?}");
            assert!(all[i + 1..].iter().all(|b| *b as u32 != *a as u32), "{a:?}");
        }
    }

    // Everything there is to build can be paid for from what bare hands gather, each piece in
    // seconds of holding G, even counting the smallest tree's wood
    #[test]
    fn every_piece_is_gathered_bare_handed_in_seconds() {
        let smallest_tree = 2.0;
        // The pieces of wood, stone and clay alone: the rest are of what's got further on
        for kind in Kind::ALL.into_iter().filter(|k| k.cost().parts().iter().all(|(r, _)| Res::ALL[..ALWAYS].contains(r))) {
            let m = kind.cost();
            let [wood, stone, clay] = [m.wood, m.stone, m.clay].map(f64::from);
            let seconds = (wood / smallest_tree).ceil() * gather::work_seconds(Yield::Wood, true)
                + (stone / gather::PER_BLOCK as f64).ceil() * gather::work_seconds(Yield::Stone, false)
                + (clay / gather::PER_BLOCK as f64).ceil() * gather::work_seconds(Yield::Clay, false);
            assert!(seconds <= 10.0, "{kind:?} takes {seconds} s");
            for (y, n) in [(Yield::Wood, wood), (Yield::Stone, stone), (Yield::Clay, clay)] {
                assert!(n == 0.0 || tools::power(None, y).is_some(), "{kind:?}: {y:?} by hand");
            }
        }
    }
}
