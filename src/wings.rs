//! Wings worn on the back, see Game::worn: the kinds there are, each with a look of its own and a
//! feel of its own to glide on, and how they're seen in first person: spread out at the edges of
//! the view while gliding, flexing as they ride the air and beating hard on a flap, folded down the
//! back otherwise, seen looking down, with a trail of sparks, frost or motes behind the elemental
//! kinds. Gliding itself is the player's, see Player::glide_feel.

use std::f64::consts::{PI, TAU};

use crate::animals::{AnimalInstance, HELD, MODEL_RES, MOVING, Model, Part, ball, ellipsoid, limb, voxelize};
use crate::worldgen::material::*;

use super::*;

/// What a kind of wings does besides gliding
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Perk {
    None,
    /// Crackling with sparks of lightning behind
    Sparks,
    /// Frost drifting off behind
    Frost,
    /// Mending the wearer as they glide, a little health a second
    Mending,
    /// Each flap throws off a burst of embers
    Embers,
    /// Gold glittering behind
    Gilded,
}

pub struct WingKind {
    /// What it's called in saves
    pub id: &'static str,
    pub name: &'static str,
    /// Of the weapons' rarities, 0 common to 5 mythic, for its colour on its card
    pub rarity: usize,
    /// How fast they carry the player on ahead and how fast they let them sink at most, in meters
    /// a second, and how quickly they come round to where the player steers, the share of the
    /// way a second, about
    pub speed: f64,
    pub sink: f64,
    pub turn: f64,
    /// A flap with jump, in the air: how fast it lifts the player, in meters a second, and how
    /// many seconds till the next
    pub flap: Option<(f64, f64)>,
    pub perk: Perk,
    /// Gold a merchant asks
    pub price: u32,
    pub about: &'static str,
    /// Whether they're ever found in chests, left by bosses or sold by travelling merchants: all
    /// but the royal ones, which only royal armouries sell
    pub found: bool,
    /// What they're made of: the bone or frame along the leading edge, the feathers or the
    /// membrane, and their tips or the veins through them, and whether it's a membrane
    frame: u8,
    vane: u8,
    tip: u8,
    membrane: bool,
}

/// How much health Perk::Mending gives back a second while gliding
const MENDING: f64 = 2.5;

/// What merchants and chests ask for each rarity of wings, common to mythic: plain ones for what
/// a few fights bring in, the rarest for a fortune
const PRICES: [u32; 6] = [120, 450, 1100, 2600, 6500, 14000];

pub static KINDS: [WingKind; 6] = [
    WingKind {
        id: "gull",
        name: "Gullfeather Wings",
        rarity: 0,
        speed: 11.0,
        sink: 2.2,
        turn: 4.0,
        flap: None,
        perk: Perk::None,
        price: PRICES[0],
        found: true,
        about: "Plain white wings on a bone frame. They glide well and ask nothing.",
        frame: BONE,
        vane: FUR_WHITE,
        tip: FUR_GRAY,
        membrane: false,
    },
    WingKind {
        id: "stormcaller",
        name: "Stormcaller Wings",
        rarity: 2,
        speed: 15.0,
        sink: 2.6,
        turn: 6.0,
        flap: None,
        perk: Perk::Sparks,
        price: PRICES[2],
        found: true,
        about: "Slate feathers tipped with lightning: fast and nimble, sparks crackling off behind.",
        frame: DARK_STEEL,
        vane: FUR_DARK,
        tip: GLOW_STORM,
        membrane: false,
    },
    WingKind {
        id: "rimeglass",
        name: "Rimeglass Wings",
        rarity: 3,
        speed: 10.0,
        sink: 1.2,
        turn: 2.5,
        flap: None,
        perk: Perk::Frost,
        price: PRICES[3],
        found: true,
        about: "Feathers of clear ice on a steel frame. Slow and steady, they sink hardly at all, frost drifting off them.",
        frame: STEEL,
        vane: ICE,
        tip: CRYSTAL,
        membrane: false,
    },
    WingKind {
        id: "veilmoth",
        name: "Veilmoth Wings",
        rarity: 3,
        speed: 11.5,
        sink: 1.8,
        turn: 4.5,
        flap: None,
        perk: Perk::Mending,
        price: PRICES[3],
        found: true,
        about: "A lilac membrane from the dreaming realm, shedding pale motes. Gliding on them mends the wearer.",
        frame: BONE,
        vane: FLOWER_PURPLE,
        tip: GLOW_HOLY,
        membrane: true,
    },
    WingKind {
        id: "cinderwing",
        name: "Cinderwing",
        rarity: 3,
        speed: 12.0,
        sink: 2.4,
        turn: 5.0,
        flap: Some((7.0, 3.0)),
        perk: Perk::Embers,
        price: PRICES[3],
        found: true,
        about: "A membrane of black glass veined with fire. Jump in the air to beat them once for height, throwing off embers.",
        frame: DARK_STEEL,
        vane: OBSIDIAN,
        tip: GLOW_FIRE,
        membrane: true,
    },
    WingKind {
        id: "aurelian",
        name: "Aurelian Wings",
        rarity: 4,
        speed: 14.0,
        sink: 1.5,
        turn: 5.5,
        flap: Some((6.0, 2.2)),
        perk: Perk::Gilded,
        price: 9000,
        found: false,
        about: "Royal wings of white and gold, fast, steady and quick to beat, gold glittering behind.",
        frame: GOLD,
        vane: LINEN,
        tip: GOLD,
        membrane: false,
    },
];

pub const GULL: u8 = 0;
pub const STORMCALLER: u8 = 1;
pub const RIMEGLASS: u8 = 2;
pub const VEILMOTH: u8 = 3;
pub const CINDERWING: u8 = 4;
pub const AURELIAN: u8 = 5;

/// The kind with id `id`, by its number
pub fn by_id(id: &str) -> Option<u8> {
    KINDS.iter().position(|k| k.id == id).map(|k| k as u8)
}

/// The kind of its own a raid boss of realm `realm` leaves, if it has one: the sky isles' stormy
/// ones, the dreaming realm's moth wings, the frozen realm's ice, the Cinder Crown's
pub fn of_realm(realm: usize) -> Option<u8> {
    match realm {
        crate::realms::WINGS_REALM => Some(STORMCALLER),
        2 => Some(VEILMOTH),
        3 => Some(RIMEGLASS),
        5 => Some(CINDERWING),
        _ => None,
    }
}

/// How likely wings of each rarity are to turn up next to the plain ones, before luck
const RARITY_ODDS: [f64; 6] = [1.0, 0.75, 0.5, 0.25, 0.09, 0.025];

/// A kind of wings found, for `roll` from 0 to 1, as chests and bosses roll their weapons: the
/// plainer kinds most often, the rarer ones the more often the luckier it is. What `has` says the
/// player has already is left out, and so are the royal ones, so each pair found is new. None
/// once there's nothing left to find.
pub fn pick(roll: f64, luck: f64, has: impl Fn(u8) -> bool) -> Option<u8> {
    let luck = luck.max(0.25);
    let mut odds: Vec<(u8, f64)> = Vec::new();
    for (rarity, base) in RARITY_ODDS.iter().enumerate() {
        let kinds: Vec<u8> = (0..KINDS.len() as u8).filter(|&k| KINDS[k as usize].rarity == rarity && KINDS[k as usize].found && !has(k)).collect();
        let share = base * luck.powf(0.6 * rarity as f64) / kinds.len().max(1) as f64;
        odds.extend(kinds.into_iter().map(|k| (k, share)));
    }
    let total: f64 = odds.iter().map(|&(_, w)| w).sum();
    let mut left = roll.clamp(0.0, 0.999_999) * total;
    for &(k, w) in &odds {
        if left < w {
            return Some(k);
        }
        left -= w;
    }
    odds.last().map(|&(k, _)| k)
}

/// The plainest wings a travelling merchant sells that the player hasn't: the gull's first, priced
/// for what a few fights bring in, then the next cheapest
pub fn plainest(has: impl Fn(u8) -> bool) -> Option<u8> {
    (0..KINDS.len() as u8).filter(|&k| KINDS[k as usize].found && !has(k)).min_by_key(|&k| (KINDS[k as usize].price, k))
}

/// What a perk does, for the card of the wings with it, and the colour it's told in
pub fn perk_text(perk: Perk) -> Option<(&'static str, [u8; 3])> {
    Some(match perk {
        Perk::None => return None,
        Perk::Sparks => ("Sparks of lightning crackle off behind", [120, 200, 255]),
        Perk::Frost => ("Frost drifts off behind", [190, 230, 255]),
        Perk::Mending => ("Mends you while you glide", [120, 230, 120]),
        Perk::Embers => ("Each flap throws off a burst of embers", [255, 150, 60]),
        Perk::Gilded => ("Gold glitters behind", [255, 205, 70]),
    })
}

/// The colours of kind `k`'s icon: its feathers or membrane, and its tips or veins
pub fn icon_colors(k: u8) -> ([u8; 3], [u8; 3]) {
    match k {
        STORMCALLER => ([70, 76, 92], [120, 200, 255]),
        RIMEGLASS => ([190, 228, 244], [235, 250, 255]),
        VEILMOTH => ([196, 150, 226], [255, 244, 200]),
        CINDERWING => ([40, 34, 44], [255, 130, 40]),
        AURELIAN => ([250, 246, 232], [255, 200, 60]),
        _ => ([244, 240, 228], [170, 170, 170]),
    }
}

/// Voxels per meter of a wing's model: finer than the world, being right by the eye
const RES: f64 = 6.0 * MODEL_RES;
/// How far a wing reaches out from its root, in meters
const SPAN: f64 = 1.3;

/// What its trail's made of, if it leaves one, by the trail models after the wings', see models()
fn trail(perk: Perk) -> Option<usize> {
    match perk {
        Perk::Sparks => Some(0),
        Perk::Frost => Some(1),
        Perk::Mending => Some(2),
        Perk::Embers => Some(3),
        Perk::Gilded => Some(4),
        Perk::None => None,
    }
}
const TRAIL_MATERIALS: [u8; 5] = [GLOW_STORM, CRYSTAL, GLOW_HOLY, GLOW_FIRE, GOLD];
/// How big a mote of trail is, in meters across at first, shrinking away as it goes, and how many
/// seconds it lasts
const MOTE: f64 = 0.1;
const MOTE_LIFE: f64 = 1.0;

/// The right wing of kind `k`, flat, out along x from its root at the middle of the model's x and
/// y, its leading edge along y = 0 and the rest trailing back down -y, and a mote of each trail
fn wing(k: &WingKind) -> Vec<Part> {
    let mut p = Vec::new();
    // Along the leading edge, from the root out to the wrist and on to the tip, `f` of the way
    let edge = |f: f64| -> [f64; 3] {
        if f < 0.42 {
            let t = f / 0.42;
            [0.55 * t, 0.06 * t, 0.0]
        } else {
            let t = (f - 0.42) / 0.58;
            [0.55 + 0.75 * t, 0.06 - 0.18 * t, 0.0]
        }
    };
    if k.membrane {
        // Fingers of bone fanning back from the wrist, the membrane stretched between them,
        // veined along its trailing edge with the tip material
        let wrist = edge(0.42);
        let fingers = [[1.12, -0.52, 0.0], [0.86, -0.7, 0.0], [0.52, -0.6, 0.0]];
        for f in fingers {
            p.push(limb(wrist, f, 0.016, 0.008, k.frame));
            p.push(ball(f, 0.022, k.tip));
        }
        p.push(limb([0.02, 0.0, 0.0], [0.08, -0.38, 0.0], 0.014, 0.008, k.frame));
        for (c, r) in [([1.08, -0.28, 0.0], [0.2, 0.22]), ([0.86, -0.4, 0.0], [0.22, 0.28]), ([0.58, -0.36, 0.0], [0.24, 0.26]), ([0.28, -0.24, 0.0], [0.24, 0.22])] {
            p.push(ellipsoid(c, [r[0], r[1], 0.012], k.vane));
        }
    } else {
        // Long flight feathers down the back of the wing, longest out at its end, their tips first
        // so they show, and rows of small coverts over their roots
        for n in 0..10 {
            let t = n as f64 / 9.0;
            let [x, y, _] = edge(0.06 + 0.92 * t);
            let long = 0.26 + 0.2 * t;
            p.push(ellipsoid([x, y - 0.06 - long * 0.85, 0.0], [0.05, long * 0.18, 0.012], k.tip));
            p.push(ellipsoid([x, y - 0.06 - long * 0.5, 0.0], [0.055, long * 0.5, 0.013], k.vane));
        }
        for n in 0..7 {
            let [x, y, _] = edge(0.05 + 0.14 * n as f64);
            p.insert(0, ellipsoid([x, y - 0.07, 0.006], [0.1, 0.08, 0.016], k.vane));
        }
    }
    p.insert(0, limb(edge(0.42), edge(1.0), 0.026, 0.012, k.frame));
    p.insert(0, limb(edge(0.0), edge(0.42), 0.036, 0.026, k.frame));
    p
}

/// The models of every kind of wings, a right wing each, then a mote of each trail, see trail()
pub fn models() -> Vec<(Model, Vector3<f64>)> {
    let mut models: Vec<(Model, Vector3<f64>)> = KINDS.iter().map(|k| voxelize(&wing(k), RES)).collect();
    models.extend(TRAIL_MATERIALS.iter().map(|&m| voxelize(&[ball([0.0; 3], 0.5 / RES * 4.0, m)], RES)));
    models
}

/// A mote of a wing's trail, in voxels of the world
struct Mote {
    at: Vector3<f64>,
    drift: Vector3<f64>,
    age: f64,
    trail: usize,
}

/// How the wings worn are moving, see App::update_wings()
#[derive(Default)]
pub struct Flight {
    /// How far they're spread, 0 folded down the back to 1 out wide
    spread: f64,
    /// Seconds since they were last beaten, while it's under a beat's length
    beat: Option<f64>,
    /// Seconds till the next flap can be
    flap_wait: f64,
    /// Seconds into gliding, for their flexing
    time: f64,
    motes: Vec<Mote>,
    /// Seconds till the next motes of trail
    shed: f64,
    rng: u32,
}

/// How long a flap's beat takes, in seconds
const BEAT: f64 = 0.5;

impl Flight {
    /// The world's corner moved on `shift` voxels, see App::shift_everything()
    pub fn shift(&mut self, shift: Vector3<f64>) {
        for mote in &mut self.motes {
            mote.at -= shift;
        }
    }
}

impl Flight {
    fn random(&mut self) -> f64 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let mut x = self.rng;
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        (x >> 8) as f64 / 16_777_216.0
    }
}

impl App {
    /// The wings worn, spreading as the player glides and folding as they land, a flap when jump's
    /// pressed in the air if they have one, mending if theirs mend, and the trail they leave.
    /// Before the player moves each frame; `jumped` is jump pressed just now.
    pub(super) fn update_wings(&mut self, dt: f64, gliding: bool, jumped: bool) {
        let vpm = self.world.voxels_per_meter as f64;
        let view = self.view();
        self.wings.flap_wait = (self.wings.flap_wait - dt).max(0.0);
        let airborne = self.player.as_ref().is_some_and(|p| !p.on_ground());
        let Some(k) = self.game.worn_wings() else {
            self.wings.spread = 0.0;
            self.wings.motes.clear();
            self.game.set_aloft(None);
            return;
        };
        let kind = &KINDS[k as usize];
        if let Some(player) = &mut self.player {
            player.glide_feel = [kind.speed, kind.sink, kind.turn];
        }
        let flying = gliding && airborne;
        let flight = &mut self.wings;
        let target = if flying { 1.0 } else { 0.0 };
        flight.spread += (target - flight.spread) * (1.0 - (-7.0 * dt).exp());
        flight.time = if flying { flight.time + dt } else { 0.0 };
        if let Some(t) = &mut flight.beat {
            *t += dt;
            if *t > BEAT {
                flight.beat = None;
            }
        }
        // A flap: a hard beat lifting the player, embers thrown off for the cinder wings
        if jumped
            && airborne
            && self.wings.flap_wait <= 0.0
            && let Some((lift, wait)) = kind.flap
            && let Some(player) = &mut self.player
        {
            player.flap(lift * vpm);
            let carry = player.velocity() * 0.85;
            self.wings.flap_wait = wait;
            self.wings.beat = Some(0.0);
            if let Some(sound) = &mut self.audio {
                sound.play(audio::Cue::here(audio::Sound::Dodge, 0.8).pitched(0.7), &view);
            }
            // Embers thrown off along both wings, carried on with the player at first
            if kind.perk == Perk::Embers {
                for (_, tip) in self.wing_tips(&view, self.wings.spread) {
                    for _ in 0..8 {
                        let flight = &mut self.wings;
                        let along = flight.random();
                        let scatter = Vector3::new(flight.random() - 0.5, flight.random() - 0.5, flight.random() - 0.8) * 2.5 * vpm;
                        let at = view.eye + (tip - view.eye) * (0.4 + 0.6 * along);
                        flight.motes.push(Mote { at, drift: carry + scatter, age: 0.0, trail: 3 });
                    }
                }
            }
        }
        // Mending as they glide
        if flying && kind.perk == Perk::Mending {
            self.game.mend(MENDING * dt);
        }
        // The trail: motes shed from both wingtips while gliding, drifting and shrinking away
        let flight = &mut self.wings;
        for mote in &mut flight.motes {
            mote.age += dt;
            mote.at += mote.drift * dt;
        }
        flight.motes.retain(|m| m.age < MOTE_LIFE);
        flight.shed -= dt;
        if let Some(trail) = trail(kind.perk).filter(|_| flying && self.wings.spread > 0.6 && self.wings.shed <= 0.0) {
            self.wings.shed = 0.035;
            // Carried on with the player nearly as fast at first, so the trail streams off the
            // wingtips past the edges of the view rather than being left behind out of sight
            let carry = self.player.as_ref().map_or(Vector3::zeros(), |p| p.velocity()) * 0.92;
            for (_, tip) in self.wing_tips(&view, self.wings.spread) {
                let flight = &mut self.wings;
                let drift = carry + Vector3::new(flight.random() - 0.5, flight.random() - 0.5, -0.8) * 0.6 * vpm;
                flight.motes.push(Mote { at: tip, drift, age: 0.0, trail });
            }
        }
        let flight = &mut self.wings;
        if flight.motes.len() > 80 {
            let extra = flight.motes.len() - 80;
            flight.motes.drain(..extra);
        }
        // What's shown of them by the quick bar: whether they're gliding, and how ready a flap is
        let ready = kind.flap.map(|(_, wait)| ((1.0 - flight.flap_wait / wait).clamp(0.0, 1.0) as f32, "Space  flap"));
        let glided = flying && flight.time > 1.0;
        self.game.set_aloft(Some(game::Aloft { gliding: flying, glided, ready, note: None }));
    }
    /// Where each wing's root is and how it's turned, by the way the body faces rather than the
    /// view, as worn on the back: spread `spread` of the way from folded down the back to out
    /// wide, swept forward, flexing as it rides the air and beating down hard on a flap. The right
    /// wing first, then the left, the same model turned over.
    fn wing_poses(&self, view: &combat::View, spread: f64) -> [(Vector3<f64>, UnitQuaternion<f64>, f64); 2] {
        let m = self.world.voxels_per_meter as f64;
        let up = Vector3::z();
        let ahead = (view.rotation * Vector3::y()).xy().try_normalize(1e-6).map_or(Vector3::y(), |a| Vector3::new(a.x, a.y, 0.0));
        let right = ahead.cross(&up);
        let s = spread * spread * (3.0 - 2.0 * spread);
        let flight = &self.wings;
        // Riding the air, the tips rise and fall a little; on a flap, up and then hard down
        let flex = 0.07 * s * (flight.time * TAU * 0.6).sin();
        let beat = flight.beat.map_or(0.0, |t| {
            let u = t / BEAT;
            -0.6 * (u * PI).sin()
        });
        let sweep = -1.35 + (1.1 + 1.35) * s;
        let rise = -0.95 + (0.08 + 0.95) * s + flex + beat;
        // Spread, pitched up at the front so their tops face back towards the eye, rather than
        // being seen edge on
        let bank = 0.55 * s;
        [1.0, -1.0].map(|side: f64| {
            let root = view.eye + (right * 0.32 * side - ahead * 0.1 - up * 0.32) * m;
            let span = (right * side * sweep.cos() * rise.cos() + ahead * sweep.sin() * rise.cos() + up * rise.sin()).normalize();
            let forward_flat = ahead * sweep.cos() - right * side * sweep.sin();
            let chord = (forward_flat - span * forward_flat.dot(&span)).try_normalize(1e-6).unwrap_or(ahead);
            let top = span.cross(&chord);
            let top = if top.z < 0.0 { -top } else { top };
            let chord = (chord * bank.cos() + top * bank.sin()).normalize();
            let turn = UnitQuaternion::from_rotation_matrix(&Rotation3::from_basis_unchecked(&[span, chord, span.cross(&chord)]));
            (root, turn, side)
        })
    }

    /// Where the wingtips are, in voxels of the world, with which side each is
    fn wing_tips(&self, view: &combat::View, spread: f64) -> Vec<(f64, Vector3<f64>)> {
        let m = self.world.voxels_per_meter as f64;
        self.wing_poses(view, spread).into_iter().map(|(root, turn, side)| (side, root + turn * Vector3::new(SPAN * 0.95, -0.15, 0.0) * m)).collect()
    }

    /// The wings worn and their trail, to draw
    pub(super) fn wing_instances(&self, view: &combat::View) -> Vec<AnimalInstance> {
        let Some(kind) = self.game.worn_wings() else {
            return Vec::new();
        };
        let m = self.world.voxels_per_meter as f64;
        let first = self.game.wing_model(0);
        let models = self.game.wing_models();
        let mut out = Vec::new();
        // Folded right down the back they're out of sight, unless looking down
        for (root, turn, _) in self.wing_poses(view, self.wings.spread) {
            let (model, anchor) = &models[kind as usize];
            let [x, y, z] = model.size.map(|v| v as f64 / RES);
            let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * m;
            out.push(AnimalInstance::new(first + kind as u32, root + turn * anchor * m, turn, m / RES, reach, HELD));
        }
        for mote in &self.wings.motes {
            let size = 1.0 - mote.age / MOTE_LIFE;
            let model = KINDS.len() + mote.trail;
            let (_, anchor) = &models[model];
            // The mote's model's 4 voxels across, see models()
            let scale = size.max(0.05) * MOTE * RES / 4.0;
            out.push(AnimalInstance::new(first + model as u32, mote.at + anchor * m * scale, UnitQuaternion::identity(), m / RES * scale, MOTE * m, MOVING));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind has a whole model, small enough for the shader, and a name, an id of its own and
    /// sensible numbers to glide on
    #[test]
    fn every_kind_of_wings_is_whole_and_sensible() {
        let models = models();
        assert_eq!(models.len(), KINDS.len() + TRAIL_MATERIALS.len());
        for (k, kind) in KINDS.iter().enumerate() {
            let (model, _) = &models[k];
            assert!(model.size.iter().all(|&s| s < 255), "{} is {:?} voxels", kind.name, model.size);
            let solid = model.voxels.iter().filter(|&&v| v != AIR).count();
            assert!(solid > 300, "{} has only {solid} voxels", kind.name);
            assert_eq!(by_id(kind.id), Some(k as u8));
            assert!((8.0..=16.0).contains(&kind.speed) && (1.0..=3.0).contains(&kind.sink) && kind.turn > 1.0, "{}", kind.name);
            assert!(kind.rarity <= 5 && kind.price > 0);
        }
    }

    /// Those found come plainest first, never one already had nor a royal one, the rarer the
    /// luckier
    #[test]
    fn wings_are_found_new_and_the_luckier_the_rarer() {
        assert_eq!(plainest(|_| false), Some(GULL));
        assert_eq!(KINDS[GULL as usize].price, PRICES[0]);
        assert_eq!(plainest(|_| true), None);
        assert_eq!(pick(0.5, 1.0, |_| true), None);
        let rarity = |luck: f64| (0..400).map(|n| KINDS[pick(n as f64 / 400.0, luck, |_| false).unwrap() as usize].rarity as f64).sum::<f64>() / 400.0;
        assert!(rarity(6.0) > rarity(1.0) + 0.5, "{} {}", rarity(1.0), rarity(6.0));
        for n in 0..200 {
            let roll = n as f64 / 200.0;
            let k = pick(roll, 2.0, |k| k % 3 == 0).unwrap();
            assert!(k % 3 != 0 && KINDS[k as usize].found);
            assert_ne!(pick(roll, 9.0, |_| false), Some(AURELIAN));
        }
    }
}
