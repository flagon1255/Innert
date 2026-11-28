//! Wings worn on the back, see Game::worn: the kinds there are, each with a look of its own, a
//! feel of its own to glide on and a perk, where they're found, and how they're seen in first
//! person: spread out at the edges of the view while gliding, flexing as they ride the air and
//! beating hard on a flap, swept back on a dash, folded down the back otherwise, seen looking down,
//! with a trail of sparks, frost, feathers or motes behind them. Gliding itself is the player's,
//! see Player::glide_feel. The grandest kinds truly fly, see Lift: beaten from the ground they
//! take off and climb, steered by looking, as long as their wind lasts, see Wingbeats and
//! Player::fly(), each beat a whole stroke up and down, tucked back for a dive and thrown forward
//! flaring to land.

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
    /// After a flap they hold the wearer's height a while before they sink again, see SOAR
    Soar,
    /// Shift while gliding rushes the wearer on ahead, see DASH
    Dash,
    /// Coming down from a glide the wearer lands like falling stone, hurting what's round, see
    /// SLAM_DAMAGE
    Slam,
    /// Faster by night, trailing dusk behind
    Nightwing,
    /// Gliding on them fills the wearer's mana
    Manaflow,
    /// Each flap sends a ring of holy light out round the wearer, searing what's near
    Halo,
    /// Low over water they ride its surface rather than sink into it, throwing up spray
    Skim,
    /// Burning never takes hold of the wearer: not lava, not dragonfire
    Fireproof,
    /// What strikes the wearer is pricked back
    Thorns,
    /// Each monster slain while they're worn gives back a little health
    Leech,
}

/// What a kind of wings is like to look at, see wing()
#[derive(Clone, Copy, PartialEq)]
enum Form {
    /// Long flight feathers down the back of the wing and coverts over their roots, banded across
    /// with the accent if `bars`, the outermost `long` meters longer
    Feathers { bars: bool, long: f64 },
    /// Tier on tier of feathers, the long ones out at the end, gold-edged coverts over them
    Plumes,
    /// Fingers of bone fanning back from the wrist with a membrane stretched between, `fingers`
    /// of them, as thick as stone if `heavy`
    Membrane { fingers: usize, heavy: bool },
    /// Long leaves fanning back from a stem, thorns along their edges
    Leaves,
    /// A butterfly's or a moth's: a great forewing and a hindwing behind it, banded to their rims
    /// and spotted
    Lobes,
    /// A fish's fin: rays fanning out from the root with a membrane between them
    Fin,
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
    /// Whether they truly fly, taking off from the ground and climbing, and how, see Lift: the
    /// grandest kinds only, the rest glide
    pub flight: Option<Lift>,
    pub perk: Perk,
    /// Gold a merchant asks
    pub price: u32,
    pub about: &'static str,
    /// Whether they're ever found in chests, left by bosses or sold by travelling merchants: all
    /// but the royal ones, which only royal armouries sell
    pub found: bool,
    /// What they're made of: the bone or frame along the leading edge, the feathers or the
    /// membrane, their tips, rims or the veins through them, bands or spikes, and a detail of
    /// their own, like spots or claws
    frame: u8,
    vane: u8,
    tip: u8,
    accent: u8,
    detail: u8,
    form: Form,
    /// How big they are next to the plain ones
    size: f64,
    /// The colours of their icon: the feathers or membrane, and the tips or veins
    icon: ([u8; 3], [u8; 3]),
}

/// How wings that truly fly do it, see WingKind::flight and Player::fly(): jump held off the
/// ground, or pressed again in the air, beats them, and each beat takes some of their wind, which
/// comes back on the ground and, slowly, gliding. Heavy wings climb slowly but fly fast and long,
/// light ones climb quickly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lift {
    /// How much faster than gliding beating them steadily climbs, in meters a second
    pub climb: f64,
    /// Seconds between beats while jump's held, and how long a beat's stroke takes
    pub every: f64,
    /// How fast flying level comes to, and the fastest a dive goes, in meters a second
    pub cruise: f64,
    pub dive: f64,
    /// How quickly they come round to where the player looks, the share of the way a second
    pub turn: f64,
    /// How many beats a whole wind lasts
    pub beats: f64,
}

/// Seconds jump's held off the ground before the wings beat by themselves, so a plain jump stays
/// one, see Wingbeats
const TAKE_OFF: f64 = 0.2;
/// How fast the wind comes back, the share of all of it a second: on the ground, and gliding
const WIND_GROUND: f64 = 0.4;
const WIND_GLIDING: f64 = 0.02;
/// Wings that have tired beat again once this much of their wind's back
const RESTED: f64 = 0.3;

/// Flying on wings that truly fly, see Lift, a frame at a time: whether the player's up on them,
/// how much wind's left in them, 0 to 1, whether they've tired, beating again only once they've
/// rested, and whether they've beaten since last on the ground
#[derive(Clone, Copy, Debug)]
pub struct Wingbeats {
    pub up: bool,
    pub wind: f64,
    pub tired: bool,
    pub flown: bool,
    /// Seconds till a held jump beats them again
    next: f64,
}

impl Default for Wingbeats {
    fn default() -> Self {
        Wingbeats { up: false, wind: 1.0, tired: false, flown: false, next: TAKE_OFF }
    }
}

impl Wingbeats {
    /// One frame of `dt` seconds: `aloft` in the air and out of the water, jump `pressed` just now
    /// or `held`, and `gliding` on them as wings that don't fly do. Whether they beat now. Up on
    /// them, letting go of jump they glide on; tired, they fold, and the player falls unless they
    /// hold jump to glide.
    pub fn step(&mut self, lift: &Lift, dt: f64, aloft: bool, pressed: bool, held: bool, gliding: bool) -> bool {
        self.next -= dt;
        if !aloft {
            self.up = false;
            self.flown = false;
            self.next = TAKE_OFF;
            self.wind = (self.wind + WIND_GROUND * dt).min(1.0);
            self.tired &= self.wind < RESTED;
            return false;
        }
        let cost = 1.0 / lift.beats.max(1.0);
        if self.wind < cost - 1e-9 {
            self.tired = true;
        } else if self.tired && self.wind >= RESTED {
            self.tired = false;
        }
        // A press beats once most of a beat's gone by, holding beats on and on
        let beat = !self.tired && ((pressed && self.next <= lift.every * 0.5) || (held && self.next <= 0.0));
        if beat {
            self.up = true;
            self.flown = true;
            self.wind = (self.wind - cost).max(0.0);
            self.next = lift.every;
        } else if self.tired {
            self.up = false;
        }
        if !beat && ((self.up && !held) || (!self.up && gliding)) {
            self.wind = (self.wind + WIND_GLIDING * dt).min(1.0);
        }
        beat
    }
}

/// One frame of `dt` seconds of wings of kind `kind` that truly fly as `lift` says, for `player`,
/// before they move: jump `pressed` just now, `held` and `gliding` on them as wings that don't fly
/// do, see Wingbeats::step(); up on them steering by `look`, as fast as the skill web's gliding
/// makes them, `boost`, holding their height while `soaring` after a roc's beat, and lifted by a
/// beat. Whether they beat now.
#[allow(clippy::too_many_arguments)]
pub fn fly_frame(player: &mut crate::player::Player, beats: &mut Wingbeats, kind: &WingKind, lift: &Lift, dt: f64, [pressed, held, gliding]: [bool; 3], look: Vector3<f64>, boost: f64, soaring: bool, vpm: f64) -> bool {
    let aloft = !player.on_ground() && !player.swimming();
    let beat = beats.step(lift, dt, aloft, pressed, held, gliding);
    let sink = if soaring { 0.0 } else { kind.sink / boost };
    player.look = look;
    player.flight = beats.up.then_some(crate::player::Soaring { cruise: lift.cruise * boost, dive: lift.dive, sink, turn: lift.turn });
    player.gliding |= beats.up;
    if beat {
        player.beat(lift.climb * vpm, lift.every);
    }
    beat
}

/// How much health Perk::Mending gives back a second while gliding, and Perk::Leech for each
/// monster slain, and for each boss
const MENDING: f64 = 2.5;
pub const LEECH: f64 = 6.0;
pub const LEECH_BOSS: f64 = 25.0;
/// How much mana Perk::Manaflow gives back a second while gliding
const MANAFLOW: f64 = 7.0;
/// Seconds Perk::Soar holds the wearer's height after a flap
const SOAR: f64 = 1.6;
/// How hard Perk::Dash throws the wearer on ahead, and up, in meters a second, how long it sweeps
/// the wings back and how long till the next
const DASH: f64 = 15.0;
const DASH_LIFT: f64 = 2.0;
const DASH_SWEEP: f64 = 0.45;
const DASH_WAIT: f64 = 2.5;
/// Perk::Slam: how long the wearer has to have glided before landing for it, how far round it
/// reaches in meters, how hard it hits, before a realm's toughness, and how long till the next
const SLAM_AFTER: f64 = 0.7;
const SLAM_REACH: f64 = 4.5;
pub const SLAM_DAMAGE: f64 = 45.0;
const SLAM_WAIT: f64 = 2.0;
/// Perk::Halo: how far round its ring reaches in meters, and how hard it sears
const HALO_REACH: f64 = 5.0;
pub const HALO_DAMAGE: f64 = 22.0;
/// Perk::Skim: how far over the water in meters it holds the wearer
const SKIM_OVER: f64 = 0.9;
/// Perk::Nightwing: how much faster by night
const NIGHT_SPEED: f64 = 1.3;
/// Perk::Thorns: the share of a blow pricked back
pub const THORNS: f64 = 0.5;
/// How fast gravity pulls, in meters a second a second, as for the player: what holding their
/// height takes each frame
const GRAVITY: f64 = 22.0;

/// What merchants and chests ask for each rarity of wings, common to mythic: plain ones for what
/// a few fights bring in, the rarest for a fortune
const PRICES: [u32; 6] = [120, 450, 1100, 2600, 6500, 14000];

pub static KINDS: [WingKind; 16] = [
    WingKind {
        id: "gull",
        name: "Gullfeather Wings",
        rarity: 0,
        speed: 11.0,
        sink: 2.2,
        turn: 4.0,
        flap: None,
        flight: None,
        perk: Perk::None,
        price: PRICES[0],
        about: "Plain white wings on a bone frame. They glide well and ask nothing.",
        found: true,
        frame: BONE,
        vane: FUR_WHITE,
        tip: FUR_GRAY,
        accent: FUR_WHITE,
        detail: FUR_GRAY,
        form: Form::Feathers { bars: false, long: 0.0 },
        size: 1.0,
        icon: ([244, 240, 228], [170, 170, 170]),
    },
    WingKind {
        id: "stormcaller",
        name: "Stormcaller Wings",
        rarity: 3,
        speed: 15.0,
        sink: 2.6,
        turn: 6.0,
        flap: None,
        flight: Some(Lift { climb: 5.0, every: 0.42, cruise: 18.0, dive: 27.0, turn: 3.4, beats: 18.0 }),
        perk: Perk::Sparks,
        price: PRICES[3],
        about: "Slate feathers tipped with lightning, from the sky isles' storms: they truly fly, fast and nimble, quick to climb and quick to tire, sparks crackling off behind.",
        found: true,
        frame: DARK_STEEL,
        vane: FUR_DARK,
        tip: GLOW_STORM,
        accent: FUR_DARK,
        detail: GLOW_STORM,
        form: Form::Feathers { bars: false, long: 0.0 },
        size: 1.0,
        icon: ([70, 76, 92], [120, 200, 255]),
    },
    WingKind {
        id: "rimeglass",
        name: "Rimeglass Wings",
        rarity: 3,
        speed: 10.0,
        sink: 1.2,
        turn: 2.5,
        flap: None,
        flight: None,
        perk: Perk::Frost,
        price: PRICES[3],
        about: "Feathers of clear ice on a steel frame. Slow and steady, they sink hardly at all, frost drifting off them.",
        found: true,
        frame: STEEL,
        vane: ICE,
        tip: CRYSTAL,
        accent: ICE,
        detail: CRYSTAL,
        form: Form::Feathers { bars: false, long: 0.0 },
        size: 1.0,
        icon: ([190, 228, 244], [235, 250, 255]),
    },
    WingKind {
        id: "veilmoth",
        name: "Veilmoth Wings",
        rarity: 3,
        speed: 11.5,
        sink: 1.8,
        turn: 4.5,
        flap: None,
        flight: None,
        perk: Perk::Mending,
        price: PRICES[3],
        about: "A lilac membrane from the dreaming realm, shedding pale motes. Gliding on them mends the wearer.",
        found: true,
        frame: BONE,
        vane: FLOWER_PURPLE,
        tip: GLOW_HOLY,
        accent: FLOWER_PURPLE,
        detail: GLOW_HOLY,
        form: Form::Membrane { fingers: 3, heavy: false },
        size: 1.0,
        icon: ([196, 150, 226], [255, 244, 200]),
    },
    WingKind {
        id: "cinderwing",
        name: "Cinderwing",
        rarity: 3,
        speed: 12.0,
        sink: 2.4,
        turn: 5.0,
        flap: Some((7.0, 3.0)),
        flight: None,
        perk: Perk::Embers,
        price: PRICES[3],
        about: "A membrane of black glass veined with fire. Jump in the air to beat them once for height, throwing off embers.",
        found: true,
        frame: DARK_STEEL,
        vane: OBSIDIAN,
        tip: GLOW_FIRE,
        accent: OBSIDIAN,
        detail: GLOW_FIRE,
        form: Form::Membrane { fingers: 3, heavy: false },
        size: 1.0,
        icon: ([40, 34, 44], [255, 130, 40]),
    },
    WingKind {
        id: "aurelian",
        name: "Aurelian Wings",
        rarity: 4,
        speed: 14.0,
        sink: 1.5,
        turn: 5.5,
        flap: Some((6.0, 2.2)),
        flight: Some(Lift { climb: 5.0, every: 0.4, cruise: 16.0, dive: 25.0, turn: 3.0, beats: 18.0 }),
        perk: Perk::Gilded,
        price: 9000,
        about: "Royal wings of white and gold that truly fly: steady and quick to beat, climbing readily, gold glittering behind.",
        found: false,
        frame: GOLD,
        vane: LINEN,
        tip: GOLD,
        accent: LINEN,
        detail: GOLD,
        form: Form::Feathers { bars: false, long: 0.0 },
        size: 1.0,
        icon: ([250, 246, 232], [255, 200, 60]),
    },
    WingKind {
        id: "roc",
        name: "Roc Pinions",
        rarity: 4,
        speed: 15.0,
        sink: 1.7,
        turn: 3.0,
        flap: Some((8.5, 3.6)),
        flight: Some(Lift { climb: 3.0, every: 0.62, cruise: 20.0, dive: 30.0, turn: 1.8, beats: 26.0 }),
        perk: Perk::Soar,
        price: PRICES[4],
        about: "The great barred pinions of a roc, brown and tipped with gold. They truly fly: slow, heavy beats that climb unhurried but carry the wearer fast and far, each one holding their height a while, feathers drifting down behind.",
        found: true,
        frame: BRONZE,
        vane: FUR_BROWN,
        tip: GOLD,
        accent: FUR_TAN,
        detail: GOLD,
        form: Form::Feathers { bars: true, long: 0.1 },
        size: 1.28,
        icon: ([140, 96, 56], [255, 200, 60]),
    },
    WingKind {
        id: "harpy",
        name: "Harpy Plumes",
        rarity: 2,
        speed: 13.5,
        sink: 2.4,
        turn: 7.5,
        flap: None,
        flight: None,
        perk: Perk::Dash,
        price: PRICES[2],
        about: "Storm-grey plumes ending in long teal streamers, quick to turn. Press Shift while gliding to dash on ahead on a gust.",
        found: true,
        frame: STEEL,
        vane: FUR_GRAY,
        tip: TEMPLE_TEAL,
        accent: FUR_DARK,
        detail: GLOW_FROST,
        form: Form::Feathers { bars: true, long: 0.2 },
        size: 1.0,
        icon: ([128, 132, 140], [60, 190, 180]),
    },
    WingKind {
        id: "gargoyle",
        name: "Gargoyle Wings",
        rarity: 3,
        speed: 12.5,
        sink: 2.9,
        turn: 2.2,
        flap: None,
        flight: None,
        perk: Perk::Slam,
        price: PRICES[3],
        about: "Wings of carved stone, mossed and cracked. They sink fast and turn slow, and coming down from a glide the wearer lands like a falling statue, hurting and scattering everything round.",
        found: true,
        frame: STONE,
        vane: ROCK,
        tip: DARK_STEEL,
        accent: GRAVEL,
        detail: MOSS,
        form: Form::Membrane { fingers: 3, heavy: true },
        size: 1.1,
        icon: ([120, 118, 112], [70, 74, 80]),
    },
    WingKind {
        id: "duskbat",
        name: "Duskbat Wings",
        rarity: 2,
        speed: 11.5,
        sink: 2.6,
        turn: 7.0,
        flap: Some((4.5, 1.1)),
        flight: None,
        perk: Perk::Nightwing,
        price: PRICES[2],
        about: "Leathery bat's wings, clawed at the wrist. Quick little flaps, the quickest turns, and by night they fly faster still, trailing dusk.",
        found: true,
        frame: BARK_DARK,
        vane: LEATHER,
        tip: BONE,
        accent: FUR_DARK,
        detail: BONE,
        form: Form::Membrane { fingers: 4, heavy: false },
        size: 0.95,
        icon: ([100, 70, 52], [230, 220, 200]),
    },
    WingKind {
        id: "prism",
        name: "Prismwing",
        rarity: 3,
        speed: 10.5,
        sink: 1.6,
        turn: 5.0,
        flap: Some((4.0, 1.6)),
        flight: None,
        perk: Perk::Manaflow,
        price: PRICES[3],
        about: "A butterfly's wings banded pink to blue to a glowing violet rim. Gliding on them fills your mana, light of every colour streaming off behind.",
        found: true,
        frame: DARK_STEEL,
        vane: FLOWER_PINK,
        tip: GLOW_ARCANE,
        accent: FLOWER_BLUE,
        detail: CRYSTAL,
        form: Form::Lobes,
        size: 1.0,
        icon: ([120, 160, 255], [200, 110, 255]),
    },
    WingKind {
        id: "seraph",
        name: "Seraph Wings",
        rarity: 5,
        speed: 14.5,
        sink: 1.4,
        turn: 5.0,
        flap: Some((7.0, 1.6)),
        flight: Some(Lift { climb: 5.5, every: 0.38, cruise: 15.0, dive: 24.0, turn: 2.8, beats: 20.0 }),
        perk: Perk::Halo,
        price: PRICES[5],
        about: "Tier on tier of white feathers edged in gold. They truly fly, light and quick to climb, and a beat of them now and then sends a ring of holy light out round you, searing what's near.",
        found: true,
        frame: GOLD,
        vane: FUR_WHITE,
        tip: GOLD,
        accent: LINEN,
        detail: GLOW_HOLY,
        form: Form::Plumes,
        size: 1.15,
        icon: ([255, 252, 240], [255, 214, 90]),
    },
    WingKind {
        id: "tidefin",
        name: "Tidefin Wings",
        rarity: 3,
        speed: 12.5,
        sink: 2.0,
        turn: 4.5,
        flap: None,
        flight: None,
        perk: Perk::Skim,
        price: PRICES[3],
        about: "Fins of the Sunken Reach, teal between their rays. Low over water they skim its surface instead of sinking in, so you can glide across any sea.",
        found: true,
        frame: BONE,
        vane: TEMPLE_TEAL,
        tip: CRYSTAL,
        accent: FLOWER_BLUE,
        detail: CRYSTAL,
        form: Form::Fin,
        size: 1.0,
        icon: ([50, 160, 160], [200, 240, 255]),
    },
    WingKind {
        id: "drakeskin",
        name: "Drakeskin Wings",
        rarity: 4,
        speed: 13.5,
        sink: 2.3,
        turn: 4.5,
        flap: Some((7.5, 2.6)),
        flight: Some(Lift { climb: 3.6, every: 0.55, cruise: 19.0, dive: 29.0, turn: 2.4, beats: 22.0 }),
        perk: Perk::Fireproof,
        price: PRICES[4],
        about: "The red hide of a Cinder Crown drake on black bone, spiked in bronze. They truly fly, as a dragon does, with strong, heavy beats, fast once they're up. Fire never takes hold of whoever wears them, not lava, not dragonfire.",
        found: true,
        frame: OBSIDIAN,
        vane: FUR_RED,
        tip: GLOW_FIRE,
        accent: BRONZE,
        detail: OBSIDIAN,
        form: Form::Membrane { fingers: 4, heavy: false },
        size: 1.08,
        icon: ([170, 40, 30], [255, 140, 40]),
    },
    WingKind {
        id: "thornleaf",
        name: "Thornleaf Wings",
        rarity: 1,
        speed: 10.5,
        sink: 2.0,
        turn: 5.0,
        flap: None,
        flight: None,
        perk: Perk::Thorns,
        price: PRICES[1],
        about: "Long leaves on a dark stem, thorned along their edges. Whatever strikes you while you wear them is pricked back, and leaves drift off behind as you glide.",
        found: true,
        frame: BARK_DARK,
        vane: LEAVES,
        tip: LEAVES_LIME,
        accent: LEAVES_DARK,
        detail: BONE,
        form: Form::Leaves,
        size: 1.0,
        icon: ([70, 140, 50], [180, 220, 90]),
    },
    WingKind {
        id: "bloodmoth",
        name: "Bloodmoth Wings",
        rarity: 3,
        speed: 11.0,
        sink: 1.9,
        turn: 5.0,
        flap: Some((4.5, 1.8)),
        flight: None,
        perk: Perk::Leech,
        price: PRICES[3],
        about: "A great moth's wings from the Dreaming Red, crimson with staring eyes. Every monster slain while you wear them gives you back a little health.",
        found: true,
        frame: FUR_DARK,
        vane: TEMPLE_RED,
        tip: FUR_DARK,
        accent: FUR_RED,
        detail: GLOW_BLOOD,
        form: Form::Lobes,
        size: 1.05,
        icon: ([160, 30, 40], [255, 70, 70]),
    },
];

pub const GULL: u8 = 0;
pub const STORMCALLER: u8 = 1;
pub const RIMEGLASS: u8 = 2;
pub const CINDERWING: u8 = 4;
pub const AURELIAN: u8 = 5;
pub const DUSKBAT: u8 = 9;
pub const TIDEFIN: u8 = 12;
pub const DRAKESKIN: u8 = 13;
pub const BLOODMOTH: u8 = 15;

/// The kind with id `id`, by its number
pub fn by_id(id: &str) -> Option<u8> {
    KINDS.iter().position(|k| k.id == id).map(|k| k as u8)
}

/// The kind of its own a raid boss of realm `realm` leaves: the normal world's the dusk bat's
/// leathery ones, the sky isles' stormy ones, the dreaming realm's blood-red moth wings, the
/// frozen realm's ice, the sunken reach's fins and the Cinder Crown's drakeskin
pub fn of_realm(realm: usize) -> Option<u8> {
    match realm {
        0 => Some(DUSKBAT),
        crate::realms::WINGS_REALM => Some(STORMCALLER),
        2 => Some(BLOODMOTH),
        3 => Some(RIMEGLASS),
        4 => Some(TIDEFIN),
        5 => Some(DRAKESKIN),
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

/// The colours of kind `k`'s icon: its feathers or membrane, and its tips or veins
pub fn icon_colors(k: u8) -> ([u8; 3], [u8; 3]) {
    KINDS[k as usize % KINDS.len()].icon
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
        Perk::Soar => ("Soar: after a flap they hold your height a while", [235, 190, 110]),
        Perk::Dash => ("Dash: Shift while gliding rushes you on ahead", [90, 220, 210]),
        Perk::Slam => ("Slam: land from a glide like falling stone, hurting all round", [200, 190, 170]),
        Perk::Nightwing => ("Nightwing: faster by night", [170, 140, 255]),
        Perk::Manaflow => ("Manaflow: gliding fills your mana", [110, 150, 255]),
        Perk::Halo => ("Halo: each flap sears what's round you with holy light", [255, 235, 150]),
        Perk::Skim => ("Skim: glide across water on its surface", [120, 220, 230]),
        Perk::Fireproof => ("Fireproof: burning never takes hold of you", [255, 120, 50]),
        Perk::Thorns => ("Thorns: what strikes you is pricked back", [150, 220, 90]),
        Perk::Leech => ("Leech: each monster slain gives back health", [235, 60, 70]),
    })
}

/// What a kind that truly flies does, for its card, see Lift: how it's done, by the key it's on
/// now, how fast it climbs and flies, and how long its wind lasts, and the colour it's told in
pub fn flight_text(lift: &Lift) -> ([String; 2], [u8; 3]) {
    let jump = crate::bindings::words(crate::bindings::Action::Jump);
    let how = format!("Flight: hold {jump}, or press it twice, to take off; press or hold it in the air to beat, and steer by looking");
    let feel = format!("Climbs {:.0} m/s beating, flies {:.0} m/s level, dives at {:.0}; its wind lasts {:.0} beats", lift.climb, lift.cruise, lift.dive, lift.beats);
    ([how, feel], [200, 225, 245])
}

/// Voxels per meter of a wing's model: finer than the world, being right by the eye
const RES: f64 = 6.0 * MODEL_RES;
/// How far a wing reaches out from its root, in meters, at the plain ones' size
const SPAN: f64 = 1.3;

/// What trails are made of, by the trail models after the wings', see models(): the motes'
/// material, how fast they drift down in meters a second, as feathers and leaves do, or up, as
/// smoke, and how much of the wearer's speed they go on with at first, streaming off the
/// wingtips rather than being left behind
const TRAILS: [(u8, f64, f64); 14] = [
    (GLOW_STORM, 0.0, 0.92),
    (CRYSTAL, 0.0, 0.92),
    (GLOW_HOLY, 0.0, 0.92),
    (GLOW_FIRE, 0.0, 0.92),
    (GOLD, 0.0, 0.92),
    // A roc's feathers, a harpy's gusts, a gargoyle's grit, dusk
    (FUR_TAN, 1.0, 0.6),
    (GLOW_FROST, 0.0, 0.85),
    (GRAVEL, 2.5, 0.7),
    (GLOW_VOID, 0.0, 0.9),
    // Prism light, spray, smoke, blood and leaves
    (GLOW_ARCANE, 0.0, 0.92),
    (GLOW_POISON, 0.0, 0.92),
    (FUR_WHITE, 1.8, 0.5),
    (OBSIDIAN, -0.4, 0.8),
    (GLOW_BLOOD, 0.0, 0.9),
];
const LEAF_TRAIL: (u8, f64, f64) = (LEAVES_LIME, 0.8, 0.55);
const HOLY: usize = 2;
const GUST: usize = 6;
const GRIT: usize = 7;
const SPRAY: usize = 11;

/// The motes each perk's trail is made of, going round them, see TRAILS
fn trail(perk: Perk) -> &'static [usize] {
    match perk {
        Perk::Sparks => &[0],
        Perk::Frost => &[1],
        Perk::Mending => &[2],
        Perk::Embers => &[3],
        Perk::Gilded => &[4],
        Perk::Soar => &[5],
        Perk::Dash => &[6],
        Perk::Slam => &[7],
        Perk::Nightwing => &[8],
        Perk::Manaflow => &[9, 6, 10, 2],
        Perk::Halo => &[2, 4],
        Perk::Skim => &[1],
        Perk::Fireproof => &[3, 12],
        Perk::Thorns => &[14],
        Perk::Leech => &[13],
        Perk::None => &[],
    }
}

/// A trail's material, drift and carry, see TRAILS
fn trail_of(k: usize) -> (u8, f64, f64) {
    TRAILS.get(k).copied().unwrap_or(LEAF_TRAIL)
}

/// How many trails there are, the leaves' last
const TRAIL_COUNT: usize = TRAILS.len() + 1;

/// How big a mote of trail is, in meters across at first, shrinking away as it goes, and how many
/// seconds it lasts
const MOTE: f64 = 0.1;
const MOTE_LIFE: f64 = 1.0;
/// The most motes there are at once
const MOTES: usize = 80;

/// `part` flattened into the wing's plane, `half` meters thick either side of it at its widest
/// `widest`: a shape of any outline as seen from above, however it's turned in the plane
fn flat(part: Part, half: f64, widest: f64) -> Part {
    part.stretched([1.0, 1.0, (half / widest.max(1e-6)).min(1.0)])
}

/// A flat blade from `a` to `b`, `r0` wide at `a` and `r1` at `b`, `half` thick
fn blade(a: [f64; 3], b: [f64; 3], r0: f64, r1: f64, half: f64, material: u8) -> Part {
    flat(limb(a, b, r0, r1, material), half, r0.max(r1))
}

/// A membrane stretched from `from` to between `a` and `b`, as a fan of flat blades, its edge
/// sagging in `scallop` of the way between them
fn fan(p: &mut Vec<Part>, from: [f64; 3], a: [f64; 3], b: [f64; 3], scallop: f64, half: f64, material: u8) {
    let n = 6;
    let gap = ((b[0] - a[0]).hypot(b[1] - a[1]) / n as f64).max(0.02);
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let sag = 1.0 - scallop * (t * PI).sin();
        let q = [0, 1, 2].map(|j| from[j] + (a[j] + (b[j] - a[j]) * t - from[j]) * sag);
        p.push(blade(from, q, gap * 0.4, gap * 0.85, half, material));
    }
}

/// Along the leading edge of a wing, from the root out to the wrist and on to the tip, `f` of
/// the way
fn edge(f: f64) -> [f64; 3] {
    if f < 0.42 {
        let t = f / 0.42;
        [0.55 * t, 0.06 * t, 0.0]
    } else {
        let t = (f - 0.42) / 0.58;
        [0.55 + 0.75 * t, 0.06 - 0.18 * t, 0.0]
    }
}

/// The right wing of kind `k`, flat, out along x from its root at the middle of the model's x and
/// y, its leading edge along y = 0 and the rest trailing back down -y. Earlier shapes win where
/// they overlap, so what should show goes first.
fn wing(k: &WingKind) -> Vec<Part> {
    let mut p = Vec::new();
    match k.form {
        Form::Feathers { bars, long: longer } => {
            // Long flight feathers down the back of the wing, longest out at its end, their tips
            // first so they show, banded across for a roc or a harpy, and rows of small coverts
            // over their roots
            for n in 0..10 {
                let t = n as f64 / 9.0;
                let [x, y, _] = edge(0.06 + 0.92 * t);
                let long = 0.26 + 0.2 * t + longer * t * t;
                p.push(ellipsoid([x, y - 0.06 - long * 0.85, 0.0], [0.05, long * 0.18, 0.012], k.tip));
                if bars {
                    for along in [0.62, 0.38] {
                        p.push(ellipsoid([x, y - 0.06 - long * along, 0.001], [0.056, 0.022 + 0.02 * longer, 0.014], k.accent));
                    }
                }
                p.push(ellipsoid([x, y - 0.06 - long * 0.5, 0.0], [0.055, long * 0.5, 0.013], k.vane));
            }
            for n in 0..7 {
                let [x, y, _] = edge(0.05 + 0.14 * n as f64);
                p.insert(0, ellipsoid([x, y - 0.07, 0.006], [0.1, 0.08, 0.016], if bars && n % 2 == 1 { k.accent } else { k.vane }));
            }
        }
        Form::Plumes => {
            // Gold-edged little coverts along the leading edge, over a row of greater coverts, over
            // the long primaries out at the end and the secondaries in by the root
            for n in 0..9 {
                let [x, y, _] = edge(0.04 + 0.115 * n as f64);
                p.push(ellipsoid([x, y - 0.05, 0.012], [0.07, 0.05, 0.014], if n % 2 == 0 { k.tip } else { k.detail }));
            }
            for n in 0..9 {
                let [x, y, _] = edge(0.05 + 0.115 * n as f64);
                p.push(ellipsoid([x, y - 0.13, 0.008], [0.075, 0.1, 0.014], k.accent));
            }
            for n in 0..13 {
                let t = n as f64 / 12.0;
                let [x, y, _] = edge(0.04 + 0.95 * t);
                let long = 0.3 + 0.26 * t * t;
                p.push(ellipsoid([x, y - 0.08 - long * 0.9, 0.0], [0.04, long * 0.12, 0.012], k.tip));
                p.push(ellipsoid([x, y - 0.08 - long * 0.5, 0.0], [0.045, long * 0.5, 0.012], k.vane));
            }
        }
        Form::Membrane { fingers, heavy } => {
            // Fingers of bone fanning back from the wrist, clawed at their ends, the membrane
            // stretched between them, sagging between their tips, and back to the body. A
            // gargoyle's thick as stone, cracked and mossed; a bat's clawed at the wrist; a
            // drake's spiked along its leading edge.
            let wrist = edge(0.42);
            let ends: Vec<[f64; 3]> = match fingers {
                3 => vec![[1.12, -0.52, 0.0], [0.86, -0.7, 0.0], [0.52, -0.6, 0.0]],
                _ => vec![[1.22, -0.36, 0.0], [1.04, -0.66, 0.0], [0.78, -0.8, 0.0], [0.48, -0.66, 0.0]],
            };
            let bone = if heavy { 0.026 } else { 0.016 };
            let half = if heavy { 0.022 } else { 0.012 };
            let tip = edge(1.0);
            for &f in &ends {
                p.push(ball(f, bone * 1.4, k.tip));
                p.push(limb(wrist, f, bone, bone * 0.5, k.frame));
            }
            p.push(ball(tip, bone * 1.2, k.tip));
            if fingers == 4 {
                // A claw at the wrist
                p.push(limb([wrist[0], wrist[1] + 0.02, 0.0], [wrist[0] + 0.05, wrist[1] + 0.1, 0.0], bone * 0.9, 0.004, k.detail));
            }
            if heavy {
                // Cracks across the stone and moss in patches over it
                for (a, b) in [([0.7, -0.2], [0.82, -0.42]), ([0.4, -0.3], [0.3, -0.5]), ([0.95, -0.25], [1.05, -0.38])] {
                    p.push(limb([a[0], a[1], half * 0.6], [b[0], b[1], half * 0.6], 0.008, 0.005, k.accent));
                }
                for (c, r) in [([0.6, -0.45], 0.07), ([0.25, -0.2], 0.06), ([0.98, -0.3], 0.05)] {
                    p.push(ellipsoid([c[0], c[1], half * 0.5], [r, r * 0.8, half * 0.7], k.detail));
                }
            }
            if k.accent != k.vane && !heavy {
                // Spikes along the leading edge
                for n in 0..6 {
                    let [x, y, _] = edge(0.12 + 0.15 * n as f64);
                    p.push(limb([x, y, 0.0], [x - 0.03, y + 0.07, 0.0], 0.014, 0.003, k.accent));
                }
            }
            let body = [0.08, -0.38, 0.0];
            p.push(limb([0.02, 0.0, 0.0], body, bone * 0.9, bone * 0.5, k.frame));
            // The membrane, tip to first finger, between the fingers, and from the last back to
            // the body, a darker vein along each finger's middle for a bat's
            fan(&mut p, wrist, tip, ends[0], 0.12, half, k.vane);
            for pair in ends.windows(2) {
                fan(&mut p, wrist, pair[0], pair[1], 0.16, half, k.vane);
            }
            fan(&mut p, wrist, ends[ends.len() - 1], body, 0.1, half, k.vane);
            fan(&mut p, [0.02, 0.0, 0.0], wrist, body, 0.0, half, k.vane);
        }
        Form::Leaves => {
            // Leaves fanning back from the stem along the leading edge: each a blade swelling out
            // and narrowing to its point, a paler rib down its middle and veins off it, thorns
            // along its forward edge
            let leaves: [(f64, f64, f64, f64); 5] = [(0.95, -0.3, 0.44, 0.13), (0.76, -0.72, 0.52, 0.15), (0.54, -1.02, 0.55, 0.16), (0.32, -1.28, 0.5, 0.15), (0.1, -1.5, 0.42, 0.13)];
            for (n, &(at, angle, length, width)) in leaves.iter().enumerate() {
                let base = edge(at);
                let d = [angle.cos(), angle.sin(), 0.0];
                let side = [-d[1], d[0], 0.0];
                let along = |t: f64, off: f64| [base[0] + d[0] * length * t + side[0] * off, base[1] + d[1] * length * t + side[1] * off, 0.0];
                for t in [0.3, 0.55] {
                    p.push(limb([along(t, 0.0)[0], along(t, 0.0)[1], 0.012], along(t + 0.15, width * 0.75), 0.006, 0.004, k.tip));
                    p.push(limb([along(t, 0.0)[0], along(t, 0.0)[1], 0.012], along(t + 0.15, -width * 0.75), 0.006, 0.004, k.tip));
                }
                for t in [0.25, 0.45, 0.65] {
                    let from = along(t, width * 0.85);
                    p.push(limb(from, along(t + 0.06, width * 1.3), 0.012, 0.003, k.detail));
                }
                p.push(limb(base, along(0.95, 0.0), 0.009, 0.004, k.frame));
                let shade = if n % 2 == 0 { k.vane } else { k.accent };
                p.push(blade(base, along(0.45, 0.0), 0.02, width, 0.012, shade));
                p.push(blade(along(0.45, 0.0), along(1.0, 0.0), width, 0.01, 0.012, shade));
            }
            p.insert(0, limb(edge(0.0), edge(1.0), 0.02, 0.01, k.frame));
        }
        Form::Lobes => {
            // A great forewing out to the tip and a hindwing trailing back behind it, each spotted,
            // veined out from the root and banded from the middle to its rim: inner, then middle,
            // then rim, the earlier of them showing where they overlap
            let root = [0.04, -0.02, 0.0];
            let (fore, hind) = ([0.9, -0.2, 0.0], [0.4, -0.6, 0.0]);
            for (c, r) in [([0.86, -0.24], 0.07), ([0.38, -0.66], 0.06), ([1.12, -0.1], 0.035), ([0.62, -0.08], 0.03)] {
                p.push(ellipsoid([c[0], c[1], 0.004], [r * 0.5, r * 0.5, 0.013], k.detail));
                p.push(ellipsoid([c[0], c[1], 0.003], [r, r, 0.013], k.tip));
            }
            p.push(limb([0.0, 0.0, 0.004], [1.18, -0.1, 0.004], 0.012, 0.006, k.frame));
            for end in [[1.1, -0.34], [0.95, -0.46], [0.72, -0.48], [0.62, -0.8], [0.4, -0.88], [0.18, -0.74]] {
                p.push(limb([root[0], root[1], 0.004], [end[0], end[1], 0.004], 0.006, 0.003, k.frame));
            }
            for (to, rim) in [(fore, 0.3), (hind, 0.27)] {
                p.push(blade(root, to, 0.04, rim * 0.5, 0.012, k.vane));
                p.push(blade(root, to, 0.05, rim * 0.8, 0.012, k.accent));
                p.push(blade(root, to, 0.06, rim, 0.012, k.tip));
            }
        }
        Form::Fin => {
            // Rays fanning out from the root, beaded at their ends, a membrane between them,
            // deeper coloured out to its scalloped edge
            let root = [0.03, -0.01, 0.0];
            let rays: Vec<[f64; 3]> = (0..7)
                .map(|n| {
                    let t = n as f64 / 6.0;
                    let (angle, length) = (0.06 - 1.5 * t, 1.26 - 0.66 * t);
                    [root[0] + angle.cos() * length, root[1] + angle.sin() * length, 0.0]
                })
                .collect();
            for &end in &rays {
                p.push(ball(end, 0.022, k.tip));
                p.push(limb(root, end, 0.016, 0.006, k.frame));
            }
            for pair in rays.windows(2) {
                let inner = |e: [f64; 3]| [root[0] + (e[0] - root[0]) * 0.8, root[1] + (e[1] - root[1]) * 0.8, 0.0];
                fan(&mut p, root, inner(pair[0]), inner(pair[1]), 0.08, 0.011, k.vane);
                fan(&mut p, root, pair[0], pair[1], 0.14, 0.011, k.accent);
            }
        }
    }
    // The bone along the leading edge, out to the wrist and on to the tip, over all the rest,
    // as thick as stone for a gargoyle's
    let heavy = matches!(k.form, Form::Membrane { heavy: true, .. });
    let thick = if heavy { 1.5 } else { 1.0 };
    if !matches!(k.form, Form::Leaves | Form::Lobes | Form::Fin) {
        p.insert(0, limb(edge(0.42), edge(1.0), 0.026 * thick, 0.012 * thick, k.frame));
        p.insert(0, limb(edge(0.0), edge(0.42), 0.036 * thick, 0.026 * thick, k.frame));
    }
    p.into_iter().map(|part| part.scaled(k.size)).collect()
}

/// The models of every kind of wings, a right wing each, then a mote of each trail, see TRAILS
pub fn models() -> Vec<(Model, Vector3<f64>)> {
    let mut models: Vec<(Model, Vector3<f64>)> = KINDS.iter().map(|k| voxelize(&wing(k), RES)).collect();
    models.extend((0..TRAIL_COUNT).map(|t| voxelize(&[ball([0.0; 3], 0.5 / RES * 4.0, trail_of(t).0)], RES)));
    models
}

/// A pair of wings of kind `kind` lying on the ground, dropped or in a shrine's nest: spread either
/// side of `at`, in voxels of the world, their leading edges toward `heading` and their tips lifted a
/// little, both drawn with the kind's own model, the first of which is model `first`, as `models()`
/// made them
pub fn lying(first: u32, models: &[(Model, Vector3<f64>)], kind: u8, at: Vector3<f64>, heading: f64, vpm: f64) -> [AnimalInstance; 2] {
    let k = (kind as usize).min(KINDS.len() - 1);
    let (model, anchor) = &models[k];
    let ahead = Vector3::new(heading.cos(), heading.sin(), 0.0);
    let across = Vector3::new(-heading.sin(), heading.cos(), 0.0);
    let lift: f64 = 0.2;
    [1.0, -1.0].map(|side: f64| {
        let span = across * side * lift.cos() + Vector3::z() * lift.sin();
        let turn = UnitQuaternion::from_rotation_matrix(&Rotation3::from_basis_unchecked(&[span, ahead, span.cross(&ahead)]));
        let [x, y, z] = model.size.map(|v| v as f64 / RES);
        let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
        let root = at + across * side * 0.06 * vpm + Vector3::new(0.0, 0.0, 0.06 * vpm);
        AnimalInstance::new(first + k as u32, root + turn * anchor * vpm, turn, vpm / RES, reach, MOVING)
    })
}

/// A mote of a wing's trail, in voxels of the world
struct Mote {
    at: Vector3<f64>,
    drift: Vector3<f64>,
    age: f64,
    trail: usize,
    /// How much bigger than the trail's motes, as a slam's or a halo's are
    big: f64,
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
    /// Seconds till the next motes of trail, and which of the trail's materials they're of
    shed: f64,
    turn: usize,
    rng: u32,
    /// Seconds a dash has left to sweep the wings back, and till the next can be
    dash: f64,
    dash_wait: f64,
    /// Seconds a soar after a flap goes on holding the wearer's height
    soar: f64,
    /// Seconds spent gliding since last on the ground, for coming down like stone, whether they
    /// were in the air last frame, and seconds till the next slam can be
    aloft: f64,
    was_airborne: bool,
    slam_wait: f64,
    /// Skimming over water just now, for spray
    skimming: bool,
    /// Flying on wings that truly fly, see Wingbeats, how long the beat going on takes, a whole
    /// stroke up and down if it's theirs, and how far the wings are tucked back for a dive and
    /// thrown forward flaring to touch down, 0 to 1
    pub beats: Wingbeats,
    stroke: Option<f64>,
    tuck: f64,
    flare: f64,
    /// The highest the feet have been since the wings last held the player up, and how fast they
    /// were coming down last frame, in voxels a second, for a fall
    top: f64,
    falling: f64,
}

/// How long a flap's beat takes, in seconds
const BEAT: f64 = 0.5;
/// How far a fall from flying, past which it hurts, in meters, and how much for each meter more,
/// at most FALL_MOST; and how fast they have to be coming down for it to, in meters a second. As a
/// fall from a roc's talons, see game\aerie.rs.
pub const FALL_FROM: f64 = 4.0;
pub const FALL_HURT: f64 = 2.6;
pub const FALL_MOST: f64 = 60.0;
const FALL_SPEED: f64 = 9.0;

/// How hard a fall of `height` meters from flying hurts, see FALL_FROM
pub fn fall_damage(height: f64) -> f64 {
    ((height - FALL_FROM) * FALL_HURT).clamp(0.0, FALL_MOST)
}

/// Where a whole stroke of wings that truly fly is, `u` of the way through it: up a little, then
/// hard down and forward, then back to where they ride the air. How far they rise, in radians, and
/// how far they're swept forward.
pub fn stroke(u: f64) -> (f64, f64) {
    let ease = |t: f64| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let rise = if u < 0.3 {
        0.5 * ease(u / 0.3)
    } else if u < 0.75 {
        0.5 - 1.25 * ease((u - 0.3) / 0.45)
    } else {
        -0.75 + 0.75 * ease((u - 0.75) / 0.25)
    };
    let sweep = 0.22 * (PI * ((u - 0.3) / 0.45).clamp(0.0, 1.0)).sin();
    (rise, sweep)
}

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

    /// A ring of `count` big motes of trail `trail` round `at`, flying out to `reach` voxels as
    /// they fade, going on with the wearer at `carry` voxels a second, as a slam's dust or a
    /// halo's light. Made room for among the trail's motes, the oldest going first.
    fn ring(&mut self, at: Vector3<f64>, reach: f64, count: usize, trail: usize, carry: Vector3<f64>) {
        if self.motes.len() + count > MOTES {
            let extra = (self.motes.len() + count - MOTES).min(self.motes.len());
            self.motes.drain(..extra);
        }
        for n in 0..count {
            let a = (n as f64 + 0.5 * self.random()) / count as f64 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.0);
            self.motes.push(Mote { at: at + out * reach * 0.35, drift: carry + out * reach * 0.65 / MOTE_LIFE, age: 0.0, big: 1.6, trail });
        }
    }
}

impl App {
    /// The wings worn, spreading as the player glides and folding as they land, a flap when jump's
    /// pressed in the air if they have one, a dash on Shift if they have that, flying on them if
    /// they truly fly, see Wingbeats, what their perk does, and the trail they leave. Before the
    /// player moves each frame; `jumped` is jump pressed just now, `held` held, `dashed` Shift.
    pub(super) fn update_wings(&mut self, dt: f64, gliding: bool, jumped: bool, held: bool, dashed: bool) {
        let vpm = self.world.voxels_per_meter as f64;
        let view = self.view();
        {
            let flight = &mut self.wings;
            flight.flap_wait = (flight.flap_wait - dt).max(0.0);
            flight.dash_wait = (flight.dash_wait - dt).max(0.0);
            flight.slam_wait = (flight.slam_wait - dt).max(0.0);
            flight.dash = (flight.dash - dt).max(0.0);
            flight.soar = (flight.soar - dt).max(0.0);
        }
        let airborne = self.player.as_ref().is_some_and(|p| !p.on_ground());
        let Some(k) = self.game.worn_wings() else {
            self.wings.spread = 0.0;
            self.wings.motes.clear();
            self.wings.aloft = 0.0;
            self.wings.beats = Wingbeats::default();
            if let Some(player) = &mut self.player {
                player.flight = None;
            }
            self.game.set_aloft(None);
            return;
        };
        let kind = &KINDS[k as usize];
        // The dusk bat's faster by night
        let night = kind.perk == Perk::Nightwing && self.cycle.is_night();
        if let Some(player) = &mut self.player {
            // Quicker and sinking less for the skill web's gliding, see skills::Stat::Glide
            let boost = self.game.glide_boost();
            let feel = if night { [kind.speed * NIGHT_SPEED, kind.sink, kind.turn * 1.25] } else { [kind.speed, kind.sink, kind.turn] };
            player.glide_feel = [feel[0] * boost, feel[1] / boost, feel[2]];
        }
        // Wings that truly fly: up on them, beating them, and a fall if they folded high up
        let mut beat = false;
        if let Some(lift) = kind.flight {
            let (feet, vz, swimming) = self.player.as_ref().map_or((Vector3::zeros(), 0.0, false), |p| (p.position, p.velocity().z, p.swimming()));
            let aloft = airborne && !swimming && self.player.is_some();
            if !airborne && self.wings.was_airborne && self.wings.beats.flown && self.player.is_some() {
                let height = (self.wings.top - feet.z) / vpm;
                let damage = fall_damage(height);
                if self.wings.falling < -FALL_SPEED * vpm && damage > 0.0 {
                    self.game.fell(damage, feet, &self.world);
                    println!("FELL {height:.1} m from flying, {damage:.0} hurt");
                }
                // Folded as they touch down
                if let Some(sound) = &mut self.audio {
                    sound.play(audio::Cue::here(audio::Sound::Flap, 0.35).pitched((0.9 / kind.size) as f32), &view);
                }
            }
            // Fallen dead, they fold
            let alive = !self.game.dead();
            if !alive {
                self.wings.beats.up = false;
            }
            match &mut self.player {
                Some(player) => {
                    let boost = self.game.glide_boost();
                    let soaring = self.wings.soar > 0.0;
                    beat = fly_frame(player, &mut self.wings.beats, kind, &lift, dt, [jumped && alive, held && alive, gliding], view.forward(), boost, soaring, vpm);
                }
                None => {
                    self.wings.beats.step(&lift, dt, aloft, false, false, false);
                }
            }
            let up = self.wings.beats.up;
            // Held up by the wings, a fall's counted from here
            let held_up = up || !airborne || (gliding && vz > -kind.sink * 1.3 * vpm);
            self.wings.top = if held_up { feet.z } else { self.wings.top.max(feet.z) };
            self.wings.falling = vz;
            // Tucked back diving, thrown forward flaring to touch down
            let (tuck, flare) = match &self.player {
                Some(p) if up => (((-view.forward().z.clamp(-1.0, 1.0).asin() - 0.25) / 0.6).clamp(0.0, 1.0), p.flaring()),
                _ => (0.0, 0.0),
            };
            let ease = 1.0 - (-6.0 * dt).exp();
            self.wings.tuck += (tuck - self.wings.tuck) * ease;
            self.wings.flare += (flare - self.wings.flare) * ease;
        }
        let flying = (gliding || self.wings.beats.up) && airborne;
        let flight = &mut self.wings;
        let target = if flying { 1.0 } else { 0.0 };
        flight.spread += (target - flight.spread) * (1.0 - (-7.0 * dt).exp());
        flight.time = if flying { flight.time + dt } else { 0.0 };
        if let Some(t) = &mut flight.beat {
            *t += dt;
            if *t > flight.stroke.unwrap_or(BEAT) {
                flight.beat = None;
            }
        }
        // Come down to the ground: a gargoyle's land like falling stone after a glide
        let landed = !airborne && flight.was_airborne && self.player.is_some();
        flight.was_airborne = airborne;
        if flying && flight.spread > 0.5 {
            flight.aloft += dt;
        }
        if landed && kind.perk == Perk::Slam && flight.aloft > SLAM_AFTER && flight.slam_wait <= 0.0 {
            flight.slam_wait = SLAM_WAIT;
            if let Some(feet) = self.player.as_ref().map(|p| p.position) {
                let hit = self.game.wing_blast(feet + Vector3::new(0.0, 0.0, 0.5 * vpm), SLAM_REACH, SLAM_DAMAGE, true);
                self.wings.ring(feet + Vector3::new(0.0, 0.0, 0.3 * vpm), SLAM_REACH * vpm, 28, GRIT, Vector3::zeros());
                println!("SLAM landing, {hit} hurt");
            }
        }
        if !airborne {
            self.wings.aloft = 0.0;
        }
        // A flap: a hard beat lifting the player, embers thrown off for the cinder wings, holding
        // their height a while for a roc's, a ring of holy light for a seraph's
        // Wings that truly fly beat with every press of jump, or on and on while it's held, a whole
        // stroke each, its sound the deeper the bigger they are; what their perk does on a flap
        // they do no oftener than they would on one
        let mut perk_flap = false;
        if beat && let Some(lift) = kind.flight {
            self.wings.beat = Some(0.0);
            self.wings.stroke = Some(lift.every.clamp(0.36, 0.6));
            if let Some(sound) = &mut self.audio {
                let first = self.wings.beats.wind > 1.0 - 1.5 / lift.beats;
                sound.play(audio::Cue::here(audio::Sound::Flap, if first { 0.85 } else { 0.6 }).pitched((1.25 / kind.size) as f32), &view);
            }
            if self.wings.flap_wait <= 0.0 {
                self.wings.flap_wait = kind.flap.map_or(2.0, |(_, wait)| wait);
                perk_flap = true;
            }
        }
        if jumped
            && airborne
            && kind.flight.is_none()
            && self.wings.flap_wait <= 0.0
            && let Some((lift, wait)) = kind.flap
            && let Some(player) = &mut self.player
        {
            player.flap(lift * vpm);
            self.wings.flap_wait = wait;
            self.wings.beat = Some(0.0);
            self.wings.stroke = None;
            if let Some(sound) = &mut self.audio {
                let leathery = matches!(kind.form, Form::Membrane { .. });
                let cue = if leathery { audio::Cue::here(audio::Sound::Flap, 0.7).pitched((1.6 / kind.size) as f32) } else { audio::Cue::here(audio::Sound::Dodge, 0.8).pitched(0.7) };
                sound.play(cue, &view);
            }
            perk_flap = true;
        }
        if perk_flap && let Some(player) = &self.player {
            let carry = player.velocity() * 0.85;
            let feet = player.position;
            // Embers thrown off along both wings, carried on with the player at first
            if kind.perk == Perk::Embers {
                for (_, tip) in self.wing_tips(&view, self.wings.spread) {
                    for _ in 0..8 {
                        let flight = &mut self.wings;
                        let along = flight.random();
                        let scatter = Vector3::new(flight.random() - 0.5, flight.random() - 0.5, flight.random() - 0.8) * 2.5 * vpm;
                        let at = view.eye + (tip - view.eye) * (0.4 + 0.6 * along);
                        flight.motes.push(Mote { at, drift: carry + scatter, age: 0.0, big: 1.0, trail: 3 });
                    }
                }
            }
            if kind.perk == Perk::Soar {
                self.wings.soar = SOAR;
            }
            if kind.perk == Perk::Halo {
                let hit = self.game.wing_blast(feet + Vector3::new(0.0, 0.0, 1.0 * vpm), HALO_REACH, HALO_DAMAGE, false);
                // At chest height, going out round the wearer below the view's middle
                self.wings.ring(feet + Vector3::new(0.0, 0.0, 0.8 * vpm), HALO_REACH * vpm, 30, HOLY, carry);
                println!("HALO flap, {hit} seared");
            }
        }
        // A dash: thrown on ahead the way the view looks, the wings swept back, a gust off them
        if dashed
            && flying
            && kind.perk == Perk::Dash
            && self.wings.dash_wait <= 0.0
            && let Some(player) = &mut self.player
        {
            let ahead = (view.rotation * Vector3::y()).xy().try_normalize(1e-6).map_or(Vector3::y(), |a| Vector3::new(a.x, a.y, 0.0));
            player.push(ahead * DASH * vpm + Vector3::new(0.0, 0.0, DASH_LIFT * vpm));
            let carry = player.velocity() * 0.3;
            self.wings.dash = DASH_SWEEP;
            self.wings.dash_wait = DASH_WAIT;
            if let Some(sound) = &mut self.audio {
                sound.play(audio::Cue::here(audio::Sound::Dodge, 1.0).pitched(1.3), &view);
            }
            for (_, tip) in self.wing_tips(&view, self.wings.spread) {
                for _ in 0..10 {
                    let flight = &mut self.wings;
                    let scatter = Vector3::new(flight.random() - 0.5, flight.random() - 0.5, flight.random() - 0.5) * 1.5 * vpm;
                    flight.motes.push(Mote { at: tip, drift: carry - ahead * 6.0 * vpm + scatter, age: 0.0, big: 1.0, trail: GUST });
                }
            }
        }
        // A roc's holding their height after a flap, and tidefins over water riding its surface
        self.wings.skimming = false;
        if flying && let Some(player) = &mut self.player {
            let hold = GRAVITY * dt * vpm;
            if self.wings.soar > 0.0 && !self.wings.beats.up {
                player.flap(hold);
            }
            if kind.perk == Perk::Skim {
                let feet = player.position;
                let surface = self.water.surface_at(feet.x, feet.y);
                if surface.is_finite() && feet.z < surface + SKIM_OVER * vpm && feet.z > surface - 0.6 * vpm {
                    // Lifted back up out of it if they've dipped in, held there otherwise
                    player.flap(if feet.z < surface + 0.35 * vpm { hold + 1.5 * vpm } else { hold });
                    self.wings.skimming = true;
                }
            }
        }
        // Mending as they glide, or filling mana
        if flying && kind.perk == Perk::Mending {
            self.game.mend(MENDING * dt);
        }
        if flying && kind.perk == Perk::Manaflow {
            self.game.fill_mana(MANAFLOW * dt);
        }
        // The trail: motes shed from both wingtips while gliding, drifting and shrinking away,
        // and spray off the water while skimming it
        let flight = &mut self.wings;
        for mote in &mut flight.motes {
            mote.age += dt;
            let fall = trail_of(mote.trail).1;
            mote.at += (mote.drift - Vector3::new(0.0, 0.0, fall * vpm)) * dt;
            // Feathers and leaves slow as they're left behind
            if fall > 0.0 {
                mote.drift *= (-1.5 * dt).exp();
            }
        }
        flight.motes.retain(|m| m.age < MOTE_LIFE);
        flight.shed -= dt;
        let trails = trail(kind.perk);
        // The dusk bat's trails dusk only by night
        let trails = if kind.perk == Perk::Nightwing && !night { &[] } else { trails };
        if !trails.is_empty() && flying && self.wings.spread > 0.6 && self.wings.shed <= 0.0 {
            // Feathers and leaves now and then, the rest streaming
            let slow = trail_of(trails[0]).1 > 0.0;
            self.wings.shed = if slow { 0.12 } else { 0.035 };
            let velocity = self.player.as_ref().map_or(Vector3::zeros(), |p| p.velocity());
            let skimming = self.wings.skimming;
            let feet = self.player.as_ref().map(|p| p.position);
            for (_, tip) in self.wing_tips(&view, self.wings.spread) {
                let flight = &mut self.wings;
                flight.turn = (flight.turn + 1) % trails.len();
                let trail = trails[flight.turn];
                // Carried on with the player nearly as fast at first, so the trail streams off
                // the wingtips past the edges of the view rather than being left behind out of
                // sight
                let carry = velocity * trail_of(trail).2;
                let drift = carry + Vector3::new(flight.random() - 0.5, flight.random() - 0.5, -0.8) * 0.6 * vpm;
                flight.motes.push(Mote { at: tip, drift, age: 0.0, big: 1.0, trail });
            }
            if skimming && let Some(feet) = feet {
                for _ in 0..3 {
                    let flight = &mut self.wings;
                    let up = Vector3::new(flight.random() - 0.5, flight.random() - 0.5, 1.6 + flight.random()) * 1.8 * vpm;
                    flight.motes.push(Mote { at: feet, drift: velocity * 0.4 + up, age: 0.0, big: 1.0, trail: SPRAY });
                }
            }
        }
        let flight = &mut self.wings;
        if flight.motes.len() > MOTES {
            let extra = flight.motes.len() - MOTES;
            flight.motes.drain(..extra);
        }
        // What's shown of them by the quick bar: whether they're gliding, how ready their move in
        // the air is, and what their perk's doing
        let move_ready = |wait: f64, of: f64| (1.0 - wait / of.max(1e-6)).clamp(0.0, 1.0) as f32;
        let ready = match (kind.perk, kind.flap) {
            _ if kind.flight.is_some() => None,
            (Perk::Dash, _) => Some((move_ready(flight.dash_wait, DASH_WAIT), "Shift  dash")),
            (Perk::Slam, _) => Some((move_ready(flight.slam_wait, SLAM_WAIT), "Land  slam")),
            (_, Some((_, wait))) => Some((move_ready(flight.flap_wait, wait), "Space  flap")),
            _ => None,
        };
        let note = if flight.beats.tired && kind.flight.is_some() {
            None
        } else if flight.beats.up && flying {
            Some("Flying")
        } else if flight.soar > 0.0 && flying {
            Some("Soaring")
        } else if flight.skimming {
            Some("Skimming")
        } else if night && flying {
            Some("Nightwing")
        } else {
            None
        };
        let glided = flying && flight.time > 1.0;
        let wind = kind.flight.map(|_| (flight.beats.wind as f32, flight.beats.tired));
        self.game.set_aloft(Some(game::Aloft { gliding: flying, glided, ready, note, wind }));
    }

    /// Where each wing's root is and how it's turned, by the way the body faces rather than the
    /// view, as worn on the back: spread `spread` of the way from folded down the back to out
    /// wide, swept forward, flexing as it rides the air, beating down hard on a flap and swept
    /// back on a dash. The right wing first, then the left, the same model turned over.
    fn wing_poses(&self, view: &combat::View, spread: f64) -> [(Vector3<f64>, UnitQuaternion<f64>, f64); 2] {
        let m = self.world.voxels_per_meter as f64;
        let up = Vector3::z();
        let ahead = (view.rotation * Vector3::y()).xy().try_normalize(1e-6).map_or(Vector3::y(), |a| Vector3::new(a.x, a.y, 0.0));
        let right = ahead.cross(&up);
        let s = spread * spread * (3.0 - 2.0 * spread);
        let flight = &self.wings;
        // Riding the air, the tips rise and fall a little, held still while soaring; on a flap,
        // up and then hard down
        let flex = if flight.soar > 0.0 { 0.0 } else { 0.07 * s * (flight.time * TAU * 0.6).sin() };
        // A flap's beat hard down; a whole stroke for wings that truly fly, see stroke()
        let (beat, forward) = flight.beat.map_or((0.0, 0.0), |t| match flight.stroke {
            Some(length) => stroke(t / length),
            None => (-0.6 * (t / BEAT * PI).sin(), 0.0),
        });
        let swept = (flight.dash / DASH_SWEEP).min(1.0);
        // Flying, tucked back for a dive, and thrown up and forward flaring to touch down
        let (tuck, flare) = (flight.tuck * s, flight.flare * s);
        // Seen by a camera following behind, see App::chase_camera(), they're spread out to the
        // sides from the shoulders, as they're worn, rather than swept forward round the view
        let behind = self.game.seen_from_behind();
        let wide = if behind { 0.12 } else { 1.1 };
        let sweep = -1.35 + (wide + 1.35) * s - 0.75 * swept * (2.0 - swept) + forward * s - 0.7 * tuck + 0.3 * flare;
        let rise = -0.95 + (0.08 + 0.95) * s + flex + beat * s.max(0.4) - 0.2 * tuck + 0.35 * flare;
        // Spread, pitched up at the front so their tops face back towards the eye, rather than
        // being seen edge on
        let bank = if behind { 0.15 } else { 0.55 } * s;
        let (out, back, down) = if behind { (0.1, 0.16, 0.24) } else { (0.32, 0.1, 0.32) };
        [1.0, -1.0].map(|side: f64| {
            let root = view.eye + (right * out * side - ahead * back - up * down) * m;
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
        let size = self.game.worn_wings().map_or(1.0, |k| KINDS[k as usize].size);
        self.wing_poses(view, spread).into_iter().map(|(root, turn, side)| (side, root + turn * Vector3::new(SPAN * 0.95 * size, -0.15, 0.0) * m)).collect()
    }

    /// The wings worn and their trail, to draw. Seen by a camera following behind, see
    /// App::chase, they're on the player's back where it is, lit as the world is, rather than
    /// held before the eye.
    pub(super) fn wing_instances(&self, view: &combat::View) -> Vec<AnimalInstance> {
        let Some(kind) = self.game.worn_wings() else {
            return Vec::new();
        };
        let m = self.world.voxels_per_meter as f64;
        let first = self.game.wing_model(0);
        let models = self.game.wing_models();
        let mut out = Vec::new();
        let on_back = combat::View { eye: view.eye - self.chase.map_or_else(Vector3::zeros, |(off, _)| off), rotation: view.rotation, tan: view.tan, vpm: view.vpm, speed: view.speed, stride: view.stride };
        let how = if self.chase.is_some() { MOVING } else { HELD };
        // Folded right down the back they're out of sight, unless looking down
        for (root, turn, _) in self.wing_poses(&on_back, self.wings.spread) {
            let (model, anchor) = &models[kind as usize];
            let [x, y, z] = model.size.map(|v| v as f64 / RES);
            let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * m;
            out.push(AnimalInstance::new(first + kind as u32, root + turn * anchor * m, turn, m / RES, reach, how));
        }
        for mote in &self.wings.motes {
            let size = 1.0 - mote.age / MOTE_LIFE;
            let model = KINDS.len() + mote.trail;
            let (_, anchor) = &models[model];
            // The mote's model's 4 voxels across, see models()
            let scale = size.max(0.05) * mote.big * MOTE * RES / 4.0;
            out.push(AnimalInstance::new(first + model as u32, mote.at + anchor * m * scale, UnitQuaternion::identity(), m / RES * scale, MOTE * mote.big * m, MOVING));
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
        assert_eq!(models.len(), KINDS.len() + TRAIL_COUNT);
        for (k, kind) in KINDS.iter().enumerate() {
            let (model, _) = &models[k];
            assert!(model.size.iter().all(|&s| s < 255), "{} is {:?} voxels", kind.name, model.size);
            let solid = model.voxels.iter().filter(|&&v| v != AIR).count();
            assert!(solid > 300, "{} has only {solid} voxels", kind.name);
            assert_eq!(by_id(kind.id), Some(k as u8));
            assert_eq!(KINDS.iter().filter(|o| o.name == kind.name).count(), 1, "{}", kind.name);
            assert!((8.0..=16.0).contains(&kind.speed) && (1.0..=3.0).contains(&kind.sink) && kind.turn > 1.0, "{}", kind.name);
            assert!(kind.rarity <= 5 && kind.price > 0);
            assert!(kind.flap.is_none_or(|(lift, wait)| lift > 0.0 && wait >= 1.0), "{}", kind.name);
            // Only the grander kinds truly fly, and sensibly
            assert!(kind.flight.is_none_or(|l| kind.rarity >= 3 && l.climb > 1.0 && (0.3..=0.8).contains(&l.every) && l.cruise >= kind.speed && l.dive > l.cruise && l.beats >= 10.0), "{}", kind.name);
            // What it does is said on its card, and seen as it glides
            assert!(kind.perk == Perk::None || perk_text(kind.perk).is_some() && !trail(kind.perk).is_empty(), "{}", kind.name);
        }
        // The kinds the avians' and gargoyles' bosses leave are there by those ids
        for id in ["roc", "harpy", "gargoyle"] {
            assert!(by_id(id).is_some(), "{id}");
        }
        let fliers: Vec<&str> = KINDS.iter().filter(|k| k.flight.is_some()).map(|k| k.id).collect();
        assert!((4..=6).contains(&fliers.len()), "{fliers:?}");
        assert!(KINDS.iter().any(|k| k.flight.is_none() && k.rarity >= 3), "grand wings that only glide are still there");
        // Every trail's a mote of its own
        for t in 0..TRAIL_COUNT {
            assert!(models[KINDS.len() + t].0.voxels.iter().any(|&v| v != AIR));
        }
        for perk in KINDS.iter().map(|k| k.perk) {
            assert!(trail(perk).iter().all(|&t| t < TRAIL_COUNT));
        }
    }

    /// Each realm's raid bosses leave a kind of their own, never the royal ones, and those found
    /// come plainest first, never one already had, the rarer the luckier
    #[test]
    fn wings_are_found_new_and_the_luckier_the_rarer() {
        let own: Vec<u8> = (0..crate::realms::REALM_COUNT).map(|r| of_realm(r).expect("every realm has its own")).collect();
        for (r, &k) in own.iter().enumerate() {
            assert!(KINDS[k as usize].found, "realm {r}'s");
            assert_eq!(own.iter().filter(|&&o| o == k).count(), 1, "realm {r}'s is its own");
        }
        assert_eq!(plainest(|_| false), Some(GULL));
        assert_eq!(KINDS[GULL as usize].price, PRICES[0]);
        assert_eq!(plainest(|k| k == GULL), by_id("thornleaf"));
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
        // Every kind but the royal ones can turn up somewhere
        for (k, kind) in KINDS.iter().enumerate().filter(|(_, kind)| kind.found) {
            assert!((0..2000).any(|n| pick(n as f64 / 2000.0, 8.0, |_| false) == Some(k as u8)), "{}", kind.name);
        }
    }

    /// A flat world at 4 voxels a meter, 128 m across and 64 m high, the ground 4 m deep
    fn flat() -> crate::worldgen::World {
        let size = [512u32, 512, 256];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 2].fill(u128::MAX);
        let mut world = crate::worldgen::World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 4;
        world
    }

    /// How a flight went, see fly()
    struct Flown {
        player: crate::player::Player,
        beats: Wingbeats,
        /// The highest the feet got over the ground, in meters, and how high they were after
        /// `at` seconds
        highest: f64,
        at: Vec<f64>,
        /// The least wind they had, whether they ever tired, and the fastest they flew along
        least_wind: f64,
        tired: bool,
        fastest: f64,
        /// How fast they were coming down as they touched the ground, in meters a second, the last
        /// time they did
        touched: Option<f64>,
    }

    /// Wings of kind `k` worn for `seconds`, 30 frames a second, from standing on flat ground, or
    /// from `from` meters up in the air: jump held when `jump` says so, by the seconds gone and how
    /// the flight's gone so far, looking as `look` says, as App::update_wings() has it
    fn fly(k: u8, seconds: f64, from: Option<f64>, jump: impl Fn(f64, &Flown) -> bool, look: impl Fn(f64) -> Vector3<f64>) -> Flown {
        let world = flat();
        let vpm = 4.0;
        let dt = 1.0 / 30.0;
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut player = crate::player::Player::new(Vector3::new(256.0, 256.0, 7.0 * vpm), &world);
        for _ in 0..60 {
            player.update(&world, &dry, Vector2::zeros(), false, false, dt);
        }
        let ground = player.position.z;
        if let Some(up) = from {
            player.position.z += up * vpm;
            player.push(Vector3::new(0.0, 0.0, 0.1));
        }
        let kind = &KINDS[k as usize];
        let mut flown = Flown { player, beats: Wingbeats::default(), highest: 0.0, at: Vec::new(), least_wind: 1.0, tired: false, fastest: 0.0, touched: None };
        let (mut was, mut falling, mut aloft) = (false, 0.0, 0.0);
        for n in 0..(seconds / dt) as usize {
            let t = n as f64 * dt;
            let held = jump(t, &flown);
            let player = &mut flown.player;
            let pressed = held && !was;
            was = held;
            let airborne = !player.on_ground();
            player.gliding = held && airborne;
            player.glide_feel = [kind.speed, kind.sink, kind.turn];
            if let Some(lift) = kind.flight {
                fly_frame(player, &mut flown.beats, kind, &lift, dt, [pressed, held, held && airborne], look(t).normalize(), 1.0, false, vpm);
            } else if pressed && airborne && let Some((lift, _)) = kind.flap {
                player.flap(lift * vpm);
            }
            player.update(&world, &dry, Vector2::zeros(), held, false, dt);
            // Coming down after a while in the air, not settling a hair onto the ground
            aloft = if airborne { aloft + dt } else { 0.0 };
            if airborne && player.on_ground() && aloft > 0.3 {
                flown.touched = Some(falling / vpm);
            }
            falling = player.velocity().z;
            let up = (player.position.z - ground) / vpm;
            flown.highest = flown.highest.max(up);
            flown.at.push(up);
            flown.least_wind = flown.least_wind.min(flown.beats.wind);
            flown.tired |= flown.beats.tired;
            flown.fastest = flown.fastest.max(player.airspeed() / vpm);
        }
        flown
    }

    /// Looking `degrees` up, turning slowly round so they fly in circles rather than into the
    /// edge of the world
    fn circling(degrees: f64) -> impl Fn(f64) -> Vector3<f64> {
        move |t: f64| {
            let (yaw, pitch) = (0.5 * t, degrees.to_radians());
            Vector3::new(yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos(), pitch.sin())
        }
    }

    /// Holding jump from the ground, wings that truly fly take off and climb high, the light ones
    /// quickest and the heavy ones flying fastest, while wings that only glide go no higher than
    /// a jump
    #[test]
    fn flying_wings_climb_and_gliding_ones_dont() {
        let mut climbed = Vec::new();
        for (k, kind) in KINDS.iter().enumerate() {
            let flown = fly(k as u8, 10.0, None, |_, _| true, circling(10.0));
            match kind.flight {
                Some(lift) => {
                    assert!(flown.highest > 15.0, "{} climbed only {:.1} m", kind.name, flown.highest);
                    assert!(flown.beats.flown && flown.least_wind < 0.8, "{}", kind.name);
                    climbed.push((kind.id, flown.at[5 * 30], lift.cruise));
                }
                None => assert!(flown.highest < 1.5, "{} climbed {:.1} m", kind.name, flown.highest),
            }
        }
        println!("climbed in 5 s: {climbed:?}");
        let of = |id: &str| climbed.iter().find(|c| c.0 == id).copied().unwrap();
        // The seraph's light, the roc's heavy
        assert!(of("seraph").1 > of("roc").1 + 4.0, "{climbed:?}");
        assert!(of("roc").2 > of("seraph").2);
        // Pressing jump twice takes off too, but pressing it once only jumps
        let seraph = by_id("seraph").unwrap();
        let twice = fly(seraph, 4.0, None, |t, _| t < 0.1 || (0.3..0.4).contains(&t) || (0.7..0.8).contains(&t) || (1.1..1.2).contains(&t), circling(10.0));
        assert!(twice.beats.flown && twice.highest > 2.0, "{:.1} m", twice.highest);
        let once = fly(seraph, 4.0, None, |t, _| t < 0.1, circling(10.0));
        assert!(!once.beats.flown && once.highest < 1.5, "{:.1} m", once.highest);
        // Flying on level they speed up to their cruise, and a dive's faster still
        let level = fly(by_id("roc").unwrap(), 12.0, Some(30.0), |t, _| t < 0.1, circling(0.0));
        let dive = fly(by_id("roc").unwrap(), 3.0, Some(40.0), |t, _| t < 0.1, circling(-45.0));
        assert!(level.fastest > 17.0 && dive.fastest > level.fastest, "{:.1} {:.1}", level.fastest, dive.fastest);
    }

    /// Beating drains the wings' wind till they tire, gliding gives a little back and the ground
    /// all of it; tired, holding jump glides them down gently, and letting go they fall hard
    #[test]
    fn the_wind_drains_and_comes_back() {
        let storm = by_id("stormcaller").unwrap();
        // Held till they've tired and glided back down to the ground, then let go
        let flown = fly(storm, 32.0, None, |_, f| !(f.tired && f.touched.is_some()), circling(12.0));
        assert!(flown.tired && flown.least_wind < 0.06, "{:.2}", flown.least_wind);
        // Tired, held, they glide down, gently, and once down get their wind back
        assert!(flown.touched.is_some_and(|v| v > -FALL_SPEED), "{:?}", flown.touched);
        assert!(flown.player.on_ground() && flown.beats.wind > 0.95, "{:.2}", flown.beats.wind);
        // Up on them, letting go of jump glides on, the wind slowly coming back
        let mut beats = Wingbeats { up: true, wind: 0.5, ..Default::default() };
        let lift = KINDS[storm as usize].flight.unwrap();
        for _ in 0..60 {
            assert!(!beats.step(&lift, 1.0 / 30.0, true, false, false, false));
        }
        assert!(beats.up && beats.wind > 0.5 && beats.wind < 0.6, "{:.3}", beats.wind);
        for _ in 0..90 {
            beats.step(&lift, 1.0 / 30.0, false, false, false, false);
        }
        assert!(!beats.up && beats.wind > 0.99, "{:.2}", beats.wind);
        // Tired high up, letting go they fold and fall hard, and it hurts
        let fell = fly(storm, 16.0, None, |_, f| !f.tired, circling(25.0));
        let speed = fell.touched.expect("came down");
        assert!(fell.highest > 15.0 && speed < -FALL_SPEED, "{:.1} m, {speed:.1} m/s", fell.highest);
        assert!(fall_damage(fell.highest) > 20.0);
        assert_eq!(fall_damage(FALL_FROM), 0.0);
        assert_eq!(fall_damage(500.0), FALL_MOST);
    }

    /// However long they beat, the feet never go higher than the sky's limit, under the top of
    /// the world
    #[test]
    fn the_sky_has_a_ceiling() {
        let world = flat();
        let ceiling = (world.size[2] as f64 / 4.0) - crate::player::CEILING;
        for id in ["seraph", "roc"] {
            let flown = fly(by_id(id).unwrap(), 20.0, Some(40.0), |_, _| true, circling(30.0));
            // From over the ground, 4 m deep
            let top = flown.highest + 4.0;
            assert!(top <= ceiling + 0.1 && top > ceiling - 1.5, "{id}: {top:.2} m, the limit {ceiling} m");
        }
    }

    /// Each kind's wing seen from above, the top voxel of each column and how high it is, written
    /// to the folder `WINGS_DUMP` names, for a sheet of them all
    #[test]
    #[ignore]
    fn dump_wings_seen_from_above() {
        let dir = std::env::var("WINGS_DUMP").expect("WINGS_DUMP");
        for (kind, (model, _)) in KINDS.iter().zip(models()) {
            let [sx, sy, sz] = model.size.map(|v| v as usize);
            let mut text = format!("{sx} {sy} {sz}\n");
            for y in 0..sy {
                let row: Vec<String> = (0..sx)
                    .map(|x| (0..sz).rev().find_map(|z| Some(model.voxels[x + (y + z * sy) * sx]).filter(|&m| m != AIR).map(|m| format!("{m}:{z}"))).unwrap_or_else(|| "0:0".into()))
                    .collect();
                text += &row.join(" ");
                text += "\n";
            }
            std::fs::write(format!("{dir}\\{}.txt", kind.id), text).unwrap();
        }
    }

    /// The wings' models together, sent to the graphics card at the start: how big they are.
    /// `cargo test --release wings_fit -- --nocapture`
    #[test]
    fn wings_fit_in_memory() {
        let models = models();
        let bytes = crate::animals::pack_models(&models.iter().map(|(m, _)| m).collect::<Vec<_>>()).len() * 4;
        // The six kinds there first were and their five trails, for how much more the rest take
        let first: Vec<&Model> = models[..6].iter().chain(&models[KINDS.len()..KINDS.len() + 5]).map(|(m, _)| m).collect();
        let before = crate::animals::pack_models(&first).len() * 4;
        println!("{} wing models, {:.2} MB packed, the first six kinds' {:.2} MB", models.len(), bytes as f64 / 1e6, before as f64 / 1e6);
        assert!(bytes < 4_000_000, "the wings take {bytes} bytes");
    }
}
