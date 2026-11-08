//! The signature special attacks: sixteen that give the families that had the fewest, or shared
//! the same set, an identity of their own. Each animates, plays out over time, is seen whether
//! anything's about or not, and from a bow, a crossbow, a gun, bombs or a weapon that casts it
//! leaves the weapon as something flying. What they shoot, set off and leave going is the fight's
//! own machinery, see combat.rs: projectiles with a trick of their own, parts of special attacks
//! still to come and zones going on a while, each kind of those being one of Sig, SigAct and
//! SigZone here, so all of it lives here, out of the way of the rest.

use super::*;

/// Every one of them
pub(super) const SIGNATURES: [Special; 16] = [
    Special::Rimeskip,
    Special::Scuttlers,
    Special::Wickfire,
    Special::Lantern,
    Special::Chainshot,
    Special::Caltrops,
    Special::Riftshot,
    Special::GloamingKnives,
    Special::SeveringDraw,
    Special::ViperThrusts,
    Special::SoulGleaning,
    Special::Stormanvil,
    Special::Stormstakes,
    Special::Litany,
    Special::EmberKnell,
    Special::Bloodthread,
];

/// What one of their shots is, and does where it gets to
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Sig {
    /// A rimeskip charge, with this many skips left before it shatters
    Skip(u8),
    /// The bomb scuttling charges come out of, cracking open where it lands
    Brood,
    /// A wickfire arrow, sticking fast where it strikes
    Wick,
    /// An arrow bursting into a lantern of light where it strikes
    Lantern,
    /// Two balls chained together, spinning apart as they fly
    Chain,
    /// A barbed caltrop, lying where it lands
    Caltrop,
    /// A ball of void, tearing a rift open where it strikes
    Rift,
    /// A shard a rift spits out
    RiftShard,
    /// A knife of shadow
    Knife,
    /// A viper's fang, through this many already
    Fang(u8),
    /// A gleaned wisp flung back out
    Wisp,
    /// A stake for the lightning to string between
    Stake,
    /// A page of light, a litany circle written where it lands
    Script,
    /// A ball of flame, a knell rolling out where it lands
    Knell,
    /// A hex of blood, binding what's round where it strikes together
    Thread,
}

/// Parts of them still to come
#[derive(Clone, Copy, Debug)]
pub(super) enum SigAct {
    /// The `k`th wickfire arrow loosed, from where the bow was, `local` meters off the eye in the
    /// view, with arrowhead `head`
    Wick { k: u32, local: Vector3<f64>, head: usize },
    /// The `k`th stake loosed, from `local` meters off the eye in the view
    Stake { k: u32, local: Vector3<f64> },
    /// The katana drawn: the dash, and the line of frost left hanging
    Draw,
    /// The `k`th viper's fang flung on
    Fang(u32),
    /// The gleaning sweep, and every wisp gleaned flung back out
    Glean,
    Fling,
    /// Lightning striking the raised weapon, it slammed down, a bolt leaping off the ring onto the
    /// animal with this id, or up off the ground here
    Charge,
    Slam,
    Leap(u32),
    Fork(Vector3<f64>),
    /// An ember knell's ring reaching the animal with this id, which rings out in turn, `depth`
    /// rings on from the first
    Echo { id: u32, depth: u32 },
}

impl SigAct {
    pub(super) fn recenter(&mut self, shift: Vector3<f64>) {
        if let SigAct::Fork(at) = self {
            *at -= shift;
        }
    }
}

/// What they leave going a while
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum SigZone {
    /// A clockwork charge scuttling after what it's after: `target`, clung on once `dwell` is set,
    /// `velocity` its way, or once it's clung, where it's clung on next to what it's on
    Scuttler,
    /// A wickfire arrow stuck fast, on `target` with `velocity` its place on it, or in the ground,
    /// pointing `way`
    Wick { way: [f32; 3] },
    /// A lantern of light hanging, searing what's in `hit` for `dwell` more seconds
    Lantern,
    /// Caltrops lying on the ground at each of `path`
    Caltrops,
    /// A rift in the air spitting shards
    Rift,
    /// Knives of shadow fanned out over the player, `owed` of them loosed so far
    Knives,
    /// A line of frost hanging from `position` along `velocity`, shattering once `tick` is out,
    /// `dwell` set once it has
    Severance,
    /// Wisps streaming back to the player, at each of `path`, `owed` of them come
    Gleaning,
    /// Lightning crackling round the raised weapon till it comes down
    Anvil,
    /// Stakes stuck in the ground at each of `path`, lightning strung between them
    Stakes,
    /// A circle of runes on the ground, `dwell` of them written, blazed up once `owed` is set
    Litany,
    /// An ember knell rolling out, `hit` what's rung out already
    Knell,
    /// A ring of `element` rolling out along the ground from `position` to `radius` meters, as
    /// fast as Combat::ripple()'s, glowing, to be seen going
    Ring(Element),
    /// A hex of blood binding what's in `hit` together, and what health each had last frame
    Bloodthread { last: [f32; THREADS] },
}

/// How many skips a rimeskip charge makes, how far apart, in meters, how long each takes, and how
/// much faster than things do it falls between, so each is a quick low hop that's seen
const SKIPS: u8 = 4;
const SKIP_HOP: f64 = 3.4;
const SKIP_TIME: f64 = 0.3;
const SKIP_GRAVITY: f64 = 3.0;
/// When a bomb's let go of in its own moves, as part of them
const SKIM_RELEASE: f64 = 0.42;
const BOWL_RELEASE: f64 = 0.4;
/// How many charges a scuttling brood cracks into, and how fast they scuttle, in meters a second
const SCUTTLERS: usize = 6;
const SCUTTLE_SPEED: f64 = 10.0;
/// How big a scuttling charge is drawn, next to a bomb
const SCUTTLER_SIZE: f64 = 2.2;
/// A wickfire volley: how long its move is, when its first arrow's loosed and between each, as
/// parts of it, and seconds a wick burns down
const WICK_LENGTH: f64 = 1.15;
const WICK_FIRST: f64 = 0.24;
const WICK_EVERY: f64 = 0.24;
const WICK_SECONDS: f64 = 2.0;
/// A lantern: seconds it hangs, how far off it sears, and how far round its glow heals, in meters
const LANTERN_SECONDS: f64 = 7.0;
const LANTERN_REACH: f64 = 8.0;
const LANTERN_GLOW: f64 = 6.0;
/// Chainshot: how fast it flies, in meters a second, how far its balls spin apart either side,
/// in meters, and how fast they spin round, in radians a second
const CHAIN_SPEED: f64 = 32.0;
const CHAIN_HALF: f64 = 1.0;
const CHAIN_SPIN: f64 = 14.0;
/// How many caltrops are blasted out, seconds they lie, and how big each of a caltrop's spikes is
/// drawn, next to a thorn as long as an arrow
const CALTROPS: usize = 14;
const CALTROP_SECONDS: f64 = 8.0;
const CALTROP_SPIKE: f64 = 0.3;
/// How big round chainshot's balls are drawn, in meters
const CHAINSHOT_BALL: f64 = 0.26;
/// A rift: seconds it stays open, and how far off it spits at things, in meters
const RIFT_SECONDS: f64 = 5.0;
const RIFT_REACH: f64 = 14.0;
/// Gloaming knives: how many, how long the move is, and when the first is loosed and between
/// each, in seconds
const KNIVES: usize = 6;
const KNIVES_LENGTH: f64 = 1.25;
const KNIFE_FIRST: f64 = 0.42;
const KNIFE_GAP: f64 = 0.1;
/// Severing draw: how long its move is, when the blade comes out, as part of it, and seconds the
/// line of frost hangs before it shatters
const DRAW_LENGTH: f64 = 1.15;
const DRAW_AT: f64 = 0.42;
const SEVER_DELAY: f64 = 0.45;
/// How far it lunges on at most, how far the cut reaches on ahead, and how long the line takes to
/// fly out there, in meters and seconds
const SEVER_LUNGE: f64 = 2.0;
const SEVER_REACH: f64 = 14.0;
const SEVER_GROW: f64 = 0.12;
/// Viper thrusts: how many, how long the move is, and when the first lands and between each, as
/// parts of it
const FANGS: u32 = 8;
const VIPER_LENGTH: f64 = 1.1;
const VIPER_FIRST: f64 = 0.14;
const VIPER_GAP: f64 = 0.085;
/// Soul gleaning: how long its move is, and when the sweep comes through and the wisps are flung,
/// as parts of it
const GLEAN_LENGTH: f64 = 1.45;
const GLEAN_REAP: f64 = 0.34;
const GLEAN_FLING: f64 = 0.8;
/// Stormanvil: how long its move is, when the lightning strikes the weapon and when it comes down,
/// as parts of it, and how far its ring rolls out, in meters
const ANVIL_LENGTH: f64 = 1.3;
const ANVIL_CHARGE: f64 = 0.28;
const ANVIL_SLAM: f64 = 0.56;
const ANVIL_REACH: f64 = 14.0;
/// Where the raised weapon's head is, meters off the eye in the view, at the top of it, for the
/// lightning to strike
const ANVIL_TIP: [f64; 3] = [0.19, 1.1, 0.46];
/// How fast a ring rolls out, as Combat::ripple() has it, in meters a second
const RING_SPEED: f64 = 13.0;
/// Stormstakes: how many, seconds between each loosed, and seconds the lightning's strung
const STAKES: u32 = 3;
const STAKE_GAP: f64 = 0.14;
const STAKE_SECONDS: f64 = 6.0;
/// A litany circle: how many runes, how far round it is, in meters, seconds between each rune
/// written, and when it blazes up
const RUNES: usize = 8;
const LITANY_RADIUS: f64 = 4.5;
const RUNE_GAP: f64 = 0.22;
const LITANY_BLAZE: f64 = 2.0;
/// An ember knell: how far its first ring rolls, and each it sets ringing, in meters, and how many
/// rings on from the first they go at most
const KNELL_REACH: f64 = 10.0;
const ECHO_REACH: f64 = 4.5;
const ECHO_DEPTH: u32 = 3;
/// A bloodthread hex: how many it binds at most, how far round, in meters, seconds it holds, and
/// how much of what one suffers the rest do
const THREADS: usize = 8;
const THREAD_REACH: f64 = 12.0;
const THREAD_SECONDS: f64 = 6.0;
const THREAD_SHARE: f64 = 0.5;
/// When a weapon that casts lets go of each, as part of its move
const LITANY_RELEASE: f64 = 0.36;
const KNELL_RELEASE: f64 = 0.4;
const THREAD_RELEASE: f64 = 0.38;

/// When each is let go of: the blades' play out with their moves from the start, the rest leave
/// the weapon as the move lets go
pub(super) fn signature_release(special: Special, _profile: &Profile) -> Option<Release> {
    match special {
        Special::GloamingKnives | Special::SeveringDraw | Special::ViperThrusts | Special::SoulGleaning | Special::Stormanvil => Some(Release::AtOnce),
        s if SIGNATURES.contains(&s) => Some(Release::Struck),
        _ => None,
    }
}

/// The moves each goes through with a weapon of `kind` held in `style`, or None for one that
/// doesn't roll it, see special_move()
pub(super) fn signature_move(special: Special, kind: Kind, style: Style) -> Option<SpecialMove> {
    let style = moves_like(style);
    let shouldered = matches!(style, Style::Crank | Style::Powder);
    Some(match (special, kind) {
        (Special::Rimeskip, Kind::Melee) if style == Style::Lob => skim_move(),
        (Special::Scuttlers, Kind::Melee) if style == Style::Lob => bowl_move(),
        (Special::Wickfire, Kind::Ranged) if style == Style::Draw => wickfire_move(),
        (Special::Lantern, Kind::Ranged) if style == Style::Draw => lantern_move(),
        (Special::Chainshot, Kind::Ranged) if shouldered => chainshot_move(),
        (Special::Caltrops, Kind::Ranged) if shouldered => caltrop_move(),
        (Special::Riftshot, Kind::Ranged) if shouldered => riftshot_move(),
        (Special::Stormstakes, Kind::Ranged) if shouldered => stakes_move(),
        (Special::GloamingKnives, Kind::Melee) if style == Style::Stab => knives_move(),
        (Special::SeveringDraw, Kind::Melee) if style == Style::Slash => draw_move(),
        (Special::ViperThrusts, Kind::Melee) if style == Style::Thrust => viper_move(),
        (Special::SoulGleaning, Kind::Melee) if matches!(style, Style::Reap | Style::Thrust) => gleaning_move(style),
        (Special::Stormanvil, Kind::Melee) if matches!(style, Style::Smash | Style::Whirl) => anvil_move(style),
        (Special::Litany, Kind::Magic) => litany_move(style),
        (Special::EmberKnell, Kind::Magic) => knell_move(style),
        (Special::Bloodthread, Kind::Magic) => thread_move(style),
        _ => return None,
    })
}

/// A bomb held in the hand at `hand`, pointing `along`
fn in_palm(hand: [f64; 3], along: [f64; 3]) -> Pose {
    pose(hand, along, [1.0, 0.0, 0.0])
}

/// A rimeskip charge's: crouched, drawn back low out to the right, and skimmed across flat and
/// low, sidearm, the hand following through to the left
fn skim_move() -> SpecialMove {
    (
        0.85,
        vec![
            (0.16, in_palm([0.36, 0.36, -0.36], [0.55, -0.15, 0.82]), false),
            (0.32, in_palm([0.42, 0.24, -0.44], [0.72, -0.4, 0.56]), false),
            (SKIM_RELEASE, in_palm([0.06, 0.62, -0.42], [-0.3, 0.9, 0.3]), true),
            (0.64, in_palm([-0.16, 0.52, -0.38], [-0.75, 0.55, 0.36]), false),
        ],
    )
}

/// Scuttling charges': crouched right down, swung back low and rolled out along the ground,
/// rising after it
fn bowl_move() -> SpecialMove {
    (
        0.85,
        vec![
            (0.16, in_palm([0.3, 0.32, -0.46], [0.1, -0.42, 0.9]), false),
            (0.3, in_palm([0.31, 0.24, -0.54], [0.12, -0.62, 0.78]), false),
            (BOWL_RELEASE, in_palm([0.16, 0.66, -0.52], [0.0, 0.96, 0.28]), true),
            (0.62, in_palm([0.2, 0.58, -0.3], [0.0, 0.5, 0.86]), false),
        ],
    )
}

/// A bow held against the string, about to loose, aimed `up` and `yaw` off the crosshair
fn bow_held(up: f64, yaw: f64) -> Pose {
    let mut p = bow_aimed(up, yaw);
    p.hand += Vector3::new(0.004, 0.015, 0.004);
    p
}

/// The bow kicked as the string snaps forward
fn bow_loosed(up: f64, yaw: f64) -> Pose {
    let mut p = bow_aimed(up, yaw);
    p.hand += Vector3::new(0.03, -0.045, -0.01);
    p.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.16) * p.turn;
    p
}

/// Wickfire's: drawn and loosed three times, a little left, on, then a little right, each arrow
/// at the next of what's nearest, then lowered
fn wickfire_move() -> SpecialMove {
    let mut keys = Vec::new();
    for (k, yaw) in [-0.12, 0.0, 0.12].into_iter().enumerate() {
        let at = WICK_FIRST + WICK_EVERY * k as f64;
        keys.push((at - 0.12, bow_aimed(0.08, yaw), false));
        keys.push((at - 0.05, bow_held(0.08, yaw), false));
        keys.push((at, bow_loosed(0.08, yaw), true));
    }
    let mut lowered = bow_aimed(0.04, 0.05);
    lowered.hand.z -= 0.05;
    keys.push((0.9, lowered, false));
    (WICK_LENGTH, keys)
}

/// A lantern arrow's: the bow raised high to the sky, brought down onto the crosshair, drawn a
/// long moment and loosed a little high, then lowered
fn lantern_move() -> SpecialMove {
    let mut lowered = bow_aimed(0.14, 0.0);
    lowered.hand.z -= 0.05;
    (1.0, vec![(0.12, bow_aimed(0.6, 0.0), false), (0.3, bow_aimed(0.24, 0.0), false), (0.45, bow_held(0.24, 0.0), false), (0.52, bow_loosed(0.24, 0.0), true), (0.74, lowered, false)])
}

/// `p` settled into the shoulder, steadying
fn steadied(p: &Pose, by: f64) -> Pose {
    let mut s = p.clone();
    s.hand += Vector3::new(-0.004, 0.012, -0.005) * by;
    s
}

/// `p` rocked back after a hard shot
fn rocked(p: &Pose, kick: f64) -> Pose {
    let mut r = p.clone();
    r.hand += Vector3::new(0.0, -0.014 * kick, 0.006 * kick);
    r.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.08 * kick) * r.turn;
    r
}

/// Chainshot's: the gun tipped up as the chained balls are rammed home, levelled, settled into
/// the shoulder, then kicking back hard enough to rock the shooter
fn chainshot_move() -> SpecialMove {
    let aimed = levelled(0.04, 0.0);
    let mut lowered = levelled(0.02, 0.0);
    lowered.hand.z -= 0.02;
    (1.0, vec![(0.1, levelled(0.3, 0.12), false), (0.24, aimed.clone(), false), (0.32, steadied(&aimed, 1.0), false), (0.38, gun_kicked(&aimed, 1.7), true), (0.5, rocked(&aimed, 1.7), false), (0.7, lowered, false)])
}

/// A caltrop blast's: the gun brought down to the hip, swung over and aimed down at the ground
/// ahead, and let go, kicking right up
fn caltrop_move() -> SpecialMove {
    let hip = |up: f64, yaw: f64| {
        let mut p = levelled(up, yaw);
        p.hand += Vector3::new(-0.02, -0.07, -0.07);
        p
    };
    let aimed = hip(-0.16, 0.0);
    (0.95, vec![(0.1, hip(-0.04, -0.18), false), (0.24, aimed.clone(), false), (0.29, steadied(&aimed, 0.8), false), (0.33, gun_kicked(&aimed, 1.9), true), (0.46, rocked(&aimed, 1.9), false), (0.7, hip(-0.06, 0.04), false)])
}

/// Riftshot's: shouldered and aimed a long while, steadying, then the shot kicking back
fn riftshot_move() -> SpecialMove {
    let aimed = levelled(0.03, 0.0);
    let mut lowered = levelled(0.02, 0.0);
    lowered.hand.z -= 0.02;
    (1.05, vec![(0.14, aimed.clone(), false), (0.3, steadied(&aimed, 1.0), false), (0.42, steadied(&aimed, 1.4), false), (0.5, gun_kicked(&aimed, 1.35), true), (0.6, rocked(&aimed, 1.35), false), (0.8, lowered, false)])
}

/// Stormstakes': levelled out to the left, shot, swung on and shot, swung right and shot, then
/// lowered, as STAKE_GAP has them go
fn stakes_move() -> SpecialMove {
    let length = 1.0;
    let mut keys = vec![(0.14, levelled(-0.04, -0.22), false)];
    for (k, yaw) in [-0.22, 0.0, 0.22].into_iter().enumerate() {
        let at = 0.3 + STAKE_GAP / length * k as f64;
        let aimed = levelled(-0.04, yaw);
        keys.push((at - 0.05, steadied(&aimed, 0.6), false));
        keys.push((at, gun_kicked(&aimed, 0.9), true));
    }
    keys.push((0.8, levelled(0.0, 0.05), false));
    (length, keys)
}

/// Gloaming knives': the dagger raised point up at the right as they come, swept across overhead
/// as they fan out, then flicked out at what's ahead once for each as it goes
fn knives_move() -> SpecialMove {
    let edge = [-1.0, 0.0, 0.0];
    let s = |t: f64| t / KNIVES_LENGTH;
    let mut keys = vec![(s(0.14), pose([0.34, 0.38, -0.02], [0.18, 0.12, 0.98], edge), false), (s(0.3), pose([0.06, 0.44, 0.06], [-0.35, 0.25, 0.9], edge), false)];
    for k in 0..KNIVES {
        let at = KNIFE_FIRST + KNIFE_GAP * k as f64;
        let side = [0.0, -0.03, 0.03, -0.04, 0.04, 0.0][k];
        if k > 0 {
            keys.push((s(at - 0.05), pose([0.16 + side, 0.56, -0.14], [-0.06 + side, 0.9, 0.42], edge), false));
        }
        keys.push((s(at), pose([0.12 + side, 0.66, -0.16], [-0.1 + side, 0.97, 0.18], edge), true));
    }
    keys.push((s(1.08), pose([0.24, 0.5, -0.24], [-0.05, 0.8, 0.6], edge), false));
    (KNIVES_LENGTH, keys)
}

/// Severing draw's: the hand at the left hip on the sheathed blade, held there crouched, then the
/// blade drawn out across to the right in one cut, following through up high, and flicked clean
fn draw_move() -> SpecialMove {
    let hip = [0.0, 1.0, 0.0];
    let flat = [0.0, 0.0, 1.0];
    let back = [-1.0, 0.0, 0.0];
    (
        DRAW_LENGTH,
        vec![
            (0.1, pose([-0.1, 0.46, -0.27], [-0.6, -0.25, -0.76], hip), false),
            (0.3, pose([-0.12, 0.44, -0.3], [-0.62, -0.27, -0.74], hip), false),
            (0.37, pose([-0.18, 0.46, -0.3], [-0.85, 0.35, -0.38], hip), false),
            (DRAW_AT, pose([0.24, 0.64, -0.14], [0.85, 0.5, 0.15], flat), true),
            (0.56, pose([0.42, 0.48, 0.02], [0.75, 0.15, 0.64], flat), false),
            (0.74, pose([0.3, 0.46, -0.14], [0.2, 0.4, 0.9], back), false),
            (0.84, pose([0.3, 0.44, -0.2], [0.1, 0.5, 0.85], back), false),
        ],
    )
}

/// Viper thrusts': drawn back, then jabbed out eight times as fast as it'll go, a little one side
/// then the other, drawn back halfway between each
fn viper_move() -> SpecialMove {
    let edge = [-1.0, 0.0, 0.0];
    let mut keys = vec![(0.06, pose([0.3, 0.3, -0.26], [0.05, 0.86, 0.32], edge), false)];
    for k in 0..FANGS {
        let at = VIPER_FIRST + VIPER_GAP * k as f64;
        let side = if k % 2 == 0 { 0.03 } else { -0.03 };
        keys.push((at - 0.042, pose([0.22 + side, 0.46, -0.22], [-0.04 + side, 0.94, 0.3], edge), false));
        keys.push((at, pose([0.12 + side, 0.74, -0.18], [-0.08 + side * 2.0, 0.99, 0.06], edge), true));
    }
    keys.push((0.9, pose([0.22, 0.52, -0.24], [-0.04, 0.9, 0.4], edge), false));
    (VIPER_LENGTH, keys)
}

/// Soul gleaning's: wound right back to the right, swept across low through everything in front,
/// raised up as the wisps stream in, then thrust out as they're flung. A halberd sweeps lower.
fn gleaning_move(style: Style) -> SpecialMove {
    let edge = [-1.0, 0.0, 0.0];
    let flat = [0.0, 0.0, 1.0];
    let low = if style == Style::Thrust { -0.06 } else { 0.0 };
    (
        GLEAN_LENGTH,
        vec![
            (0.12, pose([0.42, 0.42, -0.18 + low], [0.85, 0.45, 0.25], flat), false),
            (0.26, pose([0.47, 0.38, -0.21 + low], [0.9, 0.3, 0.3], flat), false),
            (GLEAN_REAP, pose([0.0, 0.66, -0.3 + low], [0.05, 0.99, 0.05], flat), true),
            (0.46, pose([-0.42, 0.44, -0.32 + low], [-0.9, 0.32, 0.1], flat), false),
            (0.62, pose([0.3, 0.46, -0.06], [0.15, 0.5, 0.85], edge), false),
            (0.73, pose([0.3, 0.45, -0.03], [0.12, 0.45, 0.88], edge), false),
            (GLEAN_FLING, pose([0.12, 0.68, -0.16], [-0.1, 0.97, 0.2], edge), true),
            (0.92, pose([0.16, 0.6, -0.22], [-0.05, 0.9, 0.35], edge), false),
        ],
    )
}

/// Stormanvil's: a hammer dipped, raised up at the sky and held there trembling as the lightning
/// strikes it, drawn back over and slammed down; a flail whirled round overhead faster and faster
/// as it's struck, then its head brought down
fn anvil_move(style: Style) -> SpecialMove {
    let keys = if style == Style::Whirl {
        let mut keys = vec![(0.04, pose([0.3, 0.42, -0.2], [0.2, 0.6, 0.77], [1.0, 0.0, 0.0]), false)];
        for k in 0..12 {
            let a = k as f64 / 6.0 * TAU;
            let t = 0.08 + 0.4 * (k as f64 / 11.0).powf(0.9);
            keys.push((t, pose([0.18, 0.42, 0.04], [a.cos() * 0.85, a.sin() * 0.85, 0.5], [-a.sin(), a.cos(), 0.0]), false));
        }
        keys.push((ANVIL_SLAM, pose([0.18, 0.62, -0.3], [0.0, 0.66, -0.75], [1.0, 0.0, 0.0]), true));
        keys.push((0.74, pose([0.22, 0.52, -0.32], [-0.05, 0.6, -0.8], [1.0, 0.0, 0.0]), false));
        keys
    } else {
        // Held up before the face, its head up at the top of the view for the lightning to be
        // seen striking it, see ANVIL_TIP
        let up = [0.0, 0.92, 0.4];
        let slammed = pose([0.05, 0.62, -0.42], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]);
        let mut settled = slammed.clone();
        settled.hand.z -= 0.04;
        vec![
            (0.08, pose([0.3, 0.42, -0.36], [0.05, 0.85, -0.5], [0.0, 0.5, 0.85]), false),
            (ANVIL_CHARGE - 0.12, pose([0.18, 0.48, -0.16], [0.02, 0.66, 0.75], up), false),
            (ANVIL_CHARGE, pose([0.18, 0.5, -0.14], [0.02, 0.7, 0.71], up), false),
            (0.4, pose([0.185, 0.49, -0.135], [0.04, 0.68, 0.73], up), false),
            (0.48, pose([0.16, 0.28, 0.1], [0.05, -0.4, 0.92], up), false),
            (ANVIL_SLAM, slammed, true),
            (0.74, settled, false),
        ]
    };
    (ANVIL_LENGTH, keys)
}

/// Where a weapon that casts, held in `style`, is held `by` meters off its rest and pointing
/// `toward`, as spell_special() moves it: an orb, a tome, a bell or a totem only leaning
fn spell_at(style: Style) -> impl Fn([f64; 3], [f64; 3]) -> Pose {
    let rest = spell_rest(style);
    let tip = match style {
        Style::Channel => 0.3,
        Style::Summon => 0.45,
        Style::Toll | Style::Censer => 0.35,
        Style::Hex => 0.8,
        _ => 1.0,
    };
    let side = if style == Style::Summon { -1.0 } else { 1.0 };
    let reach = match style {
        Style::Flick => 0.8,
        Style::Reap => 1.3,
        _ => 1.0,
    };
    let (out, lift) = if style == Style::Summon { (-0.06, 0.5) } else { (0.0, 1.0) };
    move |by: [f64; 3], toward: [f64; 3]| shifted(&rest, [by[0] * side * reach + out, by[1] * reach, by[2] * reach * lift], [toward[0] * side, toward[1], toward[2]], tip)
}

/// A litany circle's: the tome raised to the chest, pushed out as the page of light leaves it,
/// then traced round in a circle as the runes are written
fn litany_move(style: Style) -> SpecialMove {
    let at = spell_at(style);
    let mut keys = vec![(0.14, at([0.02, -0.04, 0.08], [0.02, 0.1, 1.0]), false), (0.28, at([0.03, -0.07, 0.11], [0.04, 0.0, 1.0]), false), (LITANY_RELEASE, at([-0.08, 0.15, 0.06], [-0.1, 0.9, 0.45]), true)];
    for k in 0..6 {
        let a = k as f64 / 6.0 * TAU;
        keys.push((0.46 + 0.07 * k as f64, at([-0.08 + 0.035 * a.cos(), 0.13, 0.06 + 0.035 * a.sin()], [-0.1 + 0.08 * a.cos(), 0.9, 0.45 + 0.08 * a.sin()]), false));
    }
    keys.push((0.9, at([-0.02, 0.04, 0.02], [-0.03, 0.5, 0.86]), false));
    (1.3, keys)
}

/// An ember knell's: the bell or censer raised high, held trembling, swung down and out hard as
/// it rings and flings its flame, swung back and out again as it rings on
fn knell_move(style: Style) -> SpecialMove {
    let at = spell_at(style);
    (
        1.1,
        vec![
            (0.14, at([0.0, -0.05, 0.12], [0.0, -0.35, 0.94]), false),
            (0.3, at([0.01, -0.07, 0.14], [0.02, -0.45, 0.9]), false),
            (KNELL_RELEASE, at([-0.06, 0.12, -0.02], [-0.04, 0.88, 0.45]), true),
            (0.56, at([0.0, 0.0, 0.07], [0.0, -0.2, 0.98]), false),
            (0.7, at([-0.04, 0.08, 0.01], [-0.03, 0.65, 0.75]), false),
            (0.86, at([0.0, 0.01, 0.02], [0.0, 0.1, 1.0]), false),
        ],
    )
}

/// A bloodthread hex's: the totem or orb raised and shaken one way and the other, thrust out as
/// the hex leaves it, held there, then drawn back as the threads go taut
fn thread_move(style: Style) -> SpecialMove {
    let at = spell_at(style);
    (
        1.0,
        vec![
            (0.1, at([0.02, -0.02, 0.06], [0.1, 0.3, 0.95]), false),
            (0.18, at([-0.02, -0.01, 0.07], [-0.12, 0.32, 0.94]), false),
            (0.26, at([0.02, -0.03, 0.08], [0.12, 0.28, 0.95]), false),
            (THREAD_RELEASE, at([-0.1, 0.16, 0.04], [-0.12, 0.86, 0.5]), true),
            (0.6, at([-0.09, 0.14, 0.03], [-0.1, 0.82, 0.55]), false),
            (0.8, at([-0.03, 0.05, 0.01], [-0.05, 0.6, 0.8]), false),
        ],
    )
}

/// `profile` turned to `element`
fn tinted(profile: &Rc<Profile>, element: Element) -> Rc<Profile> {
    if profile.element == Some(element) { profile.clone() } else { Rc::new(Profile { element: Some(element), ..(**profile).clone() }) }
}

/// `v` along the ground
fn flat_of(v: Vector3<f64>) -> Vector3<f64> {
    Vector3::new(v.x, v.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x)
}

/// How fast to throw something from `from` so it gets to `to` in `time` seconds, falling
/// `gravity` as fast as things do, in voxels a second
fn thrown(from: Vector3<f64>, to: Vector3<f64>, time: f64, gravity: f64, vpm: f64) -> Vector3<f64> {
    let g = Vector3::new(0.0, 0.0, -9.8 * gravity * vpm);
    (to - from - g * 0.5 * time * time) / time
}

/// Which way to shoot from `from` at `speed` meters a second to come down on `to`, falling
/// `gravity` as fast as things do: a little over it
fn lead(from: Vector3<f64>, to: Vector3<f64>, speed: f64, gravity: f64, vpm: f64) -> Vector3<f64> {
    let t = (to - from).norm() / (speed * vpm);
    let drop = 0.5 * 9.8 * gravity * vpm * t * t;
    (to + Vector3::new(0.0, 0.0, drop) - from).try_normalize(1e-6).unwrap_or_else(Vector3::y)
}

/// How far `p` is from the line from `a` to `b`
fn off_line(p: Vector3<f64>, a: Vector3<f64>, b: Vector3<f64>) -> f64 {
    let line = b - a;
    let along = ((p - a).dot(&line) / line.norm_squared().max(1e-9)).clamp(0.0, 1.0);
    (p - (a + line * along)).norm()
}

/// Whether `p` is inside the triangle `a`, `b`, `c` seen from above
fn inside_triangle(p: Vector3<f64>, a: Vector3<f64>, b: Vector3<f64>, c: Vector3<f64>) -> bool {
    let cross = |o: Vector3<f64>, u: Vector3<f64>, v: Vector3<f64>| (u.x - o.x) * (v.y - o.y) - (u.y - o.y) * (v.x - o.x);
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

/// Where the `k`th gloaming knife hangs `age` seconds in, fanned out over the view from the hand
fn knife_slot(view: &View, k: usize, age: f64) -> Vector3<f64> {
    let s = (k as f64 - 2.5) / 2.5;
    let out = smooth((age - 0.04 * k as f64) / 0.3);
    let hand = Vector3::new(0.25, 0.55, -0.2);
    let slot = Vector3::new(s * 0.95, 2.0 - 0.3 * s * s, 0.62 - 0.22 * s * s);
    view.eye + view.rotation * hand.lerp(&slot, out) * view.vpm
}

/// How far a chainshot's balls have spun apart either side, in meters, `age` seconds out
fn chain_half(age: f64) -> f64 {
    0.15 + (CHAIN_HALF - 0.15) * smooth(age / 0.3)
}

/// Where a chainshot's two balls are now
fn chain_balls(p: &Projectile, vpm: f64) -> (Vector3<f64>, Vector3<f64>) {
    let age = p.flown / (CHAIN_SPEED * vpm);
    let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
    let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::y);
    let over = across.cross(&way);
    let spin = age * CHAIN_SPIN;
    let arm = (across * spin.cos() + over * spin.sin()) * chain_half(age) * vpm;
    (p.position + arm, p.position - arm)
}

/// Where the `k`th rune of a litany circle round `center` is written
fn rune_at(center: Vector3<f64>, k: usize, vpm: f64) -> Vector3<f64> {
    let a = k as f64 / RUNES as f64 * TAU + center.x * 0.01;
    center + Vector3::new(a.cos(), a.sin(), 0.0) * LITANY_RADIUS * 0.82 * vpm + Vector3::new(0.0, 0.0, 0.08 * vpm)
}

/// How far a ring rolling out to `reach` meters has got `age` seconds in, in meters
fn ring_reached(age: f64, reach: f64) -> f64 {
    (0.4 + RING_SPEED * age).min(reach)
}

/// How many of a litany circle's runes are written `age` seconds in
fn runes_written(age: f64) -> usize {
    if age < 0.15 { 0 } else { (((age - 0.15) / RUNE_GAP) as usize + 1).min(RUNES) }
}

impl Combat {
    /// The last projectile set off given `sig`. Like anything in flight it flies on till it
    /// strikes something or leaves the world.
    fn signed(&mut self, sig: Sig) {
        if let Some(p) = self.projectiles.last_mut() {
            p.trick = Trick::Signature(sig);
        }
    }

    /// The living animals within `within` meters of `from`, in front along `forward` by `cos`,
    /// the nearest first, by index
    fn foes_ahead(arena: &Arena, from: Vector3<f64>, forward: Vector3<f64>, within: f64, cos: f64) -> Vec<usize> {
        let vpm = arena.view.vpm;
        let mut near: Vec<(f64, usize)> = (0..arena.animals.len())
            .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
            .filter_map(|i| {
                let to = arena.animals[i].center(vpm as u32) - from;
                let d = to.norm();
                (d < within * vpm && to.dot(&forward) > cos * d).then_some((d, i))
            })
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.into_iter().map(|(_, i)| i).collect()
    }

    /// The living animals within `within` meters of `at`, the nearest first, by index
    fn foes_round(arena: &Arena, at: Vector3<f64>, within: f64) -> Vec<usize> {
        let vpm = arena.view.vpm;
        let mut near: Vec<(f64, usize)> = (0..arena.animals.len())
            .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
            .map(|i| ((arena.animals[i].center(vpm as u32) - at).norm() / vpm - arena.animals[i].look.body().1, i))
            .filter(|&(d, _)| d < within)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.into_iter().map(|(_, i)| i).collect()
    }

    /// Where something aimed at `target` comes down: there, if it's within `most` meters, or else
    /// on the ground `most` meters on ahead
    fn ground_aim(&self, arena: &Arena, target: Vector3<f64>, most: f64) -> Vector3<f64> {
        let view = arena.view;
        let vpm = view.vpm;
        if (target - view.eye).norm() <= most * vpm {
            return target;
        }
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        ground_under(arena.world, feet + flat_of(view.forward()) * most * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm), vpm)
    }

    /// What's under `at` to land on or run along: the ground, or the top of the water over it
    fn floor_at(arena: &Arena, at: Vector3<f64>) -> Vector3<f64> {
        let vpm = arena.view.vpm;
        let ground = ground_under(arena.world, at + Vector3::new(0.0, 0.0, 0.6 * vpm), vpm);
        let surface = (arena.water)(ground.x, ground.y);
        if surface.is_finite() && surface > ground.z { Vector3::new(ground.x, ground.y, surface) } else { ground }
    }

    /// A glowing streak of `look`, its head at `at`, pointing `way`, `size` as big as a spell's,
    /// shrunk away to nothing as it comes up against `eye`, so one passing right by the eye never
    /// fills the view; None where it's gone
    fn streak_at(&self, eye: Vector3<f64>, at: Vector3<f64>, way: Vector3<f64>, look: usize, size: f64) -> Option<(Vector3<f64>, AnimalInstance)> {
        let vpm = self.vpm;
        let clear = (((at - eye).norm() / vpm - 0.8) / 1.5).clamp(0.0, 1.0);
        if clear <= 0.0 {
            return None;
        }
        let (model, bolt) = self.effect_model(SLOT_BOLT, look);
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
        Some((at, place(model, bolt, BOLT_RES, Vector3::zeros(), at, rotation, vpm, size * clear, MOVING)))
    }

    /// A ring of glowing streaks `radius` meters round `center`, lying flat, turned `turn` round,
    /// each `size` as big as a spell's: a circle of light that reads at any distance, seen from
    /// `eye`
    fn circle_instances(&self, eye: Vector3<f64>, center: Vector3<f64>, radius: f64, look: usize, size: f64, turn: f64) -> Vec<(Vector3<f64>, AnimalInstance)> {
        self.ring_instances(eye, center, Vector3::z(), radius, look, size, turn)
    }

    /// The same, standing square to `normal`
    #[allow(clippy::too_many_arguments)]
    fn ring_instances(&self, eye: Vector3<f64>, center: Vector3<f64>, normal: Vector3<f64>, radius: f64, look: usize, size: f64, turn: f64) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = self.vpm;
        let normal = normal.try_normalize(1e-6).unwrap_or_else(Vector3::z);
        let u = normal.cross(&if normal.z.abs() < 0.9 { Vector3::z() } else { Vector3::x() }).normalize();
        let v = normal.cross(&u);
        // Few and long rather than many, as there's only so much drawn at once, see
        // game::MAX_INSTANCES
        let n = ((TAU * radius / 1.4).ceil() as usize).clamp(10, 28);
        (0..n)
            .filter_map(|k| {
                let a = (k as f64 + 0.5) / n as f64 * TAU + turn;
                let at = center + (u * a.cos() + v * a.sin()) * radius * vpm;
                self.streak_at(eye, at, v * a.cos() - u * a.sin(), look, size)
            })
            .collect()
    }

    /// A line of glowing streaks from `from` to `to`, end to end, about `spacing` meters each and
    /// `size` as big as a spell's, at most a dozen, so a beam or a strand of lightning costs little
    /// to draw; with `jag` meters of kinks jumping about from one frame to the next, for lightning.
    /// Seen from `eye`.
    #[allow(clippy::too_many_arguments)]
    fn line_bolts(&self, eye: Vector3<f64>, from: Vector3<f64>, to: Vector3<f64>, look: usize, spacing: f64, size: f64, jag: f64) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = self.vpm;
        let n = (((to - from).norm() / vpm / spacing).ceil() as usize).clamp(1, 12);
        let frame = (self.time * 30.0) as u32;
        let mut last = from;
        let mut out = Vec::with_capacity(n);
        for k in 1..=n {
            let mut at = from.lerp(&to, k as f64 / n as f64);
            if jag > 0.0 && k < n {
                let h = hash(frame.wrapping_mul(97) ^ k as u32 ^ hash(from.x as u32));
                let wiggle = Vector3::new((h & 255) as f64 / 127.5 - 1.0, ((h >> 8) & 255) as f64 / 127.5 - 1.0, ((h >> 16) & 255) as f64 / 127.5 - 1.0);
                at += wiggle * jag * vpm;
            }
            out.extend(self.streak_at(eye, at, at - last, look, size));
            last = at;
        }
        out
    }

    /// How far the player can dash on ahead along `flat`, in meters, before running into something
    fn room_ahead(arena: &Arena, feet: Vector3<f64>, flat: Vector3<f64>) -> f64 {
        let vpm = arena.view.vpm;
        arena
            .animals
            .iter()
            .filter(|a| a.health > 0.0 && !a.look.is_pet())
            .filter_map(|a| {
                let to = a.position - feet;
                let d = to.xy().norm() / vpm;
                (d < 14.0 && to.xy().try_normalize(1e-6).is_some_and(|v| v.dot(&flat.xy()) > 0.8)).then_some(d - a.look.body().1 - 1.5)
            })
            .fold(f64::MAX, f64::min)
            .max(0.0)
    }

    /// What `special` does, let go of with `weapon` aimed at `target`, doing `base` damage, see
    /// unleash_at()
    pub(super) fn signature_at(&mut self, arena: &mut Arena, weapon: &Weapon, special: Special, target: Vector3<f64>, base: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let profile = weapon.profile.clone();
        let up = Vector3::z();
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        let later = |pending: &mut Vec<Pending>, delay: f64, act: SigAct, profile: &Rc<Profile>, damage: f64| pending.push(Pending { delay, act: Act::Signature(act), profile: profile.clone(), damage });
        let local = |at: Vector3<f64>| view.rotation.inverse() * (at - view.eye) / vpm;
        match special {
            // Skimmed out low onto the ground a few meters ahead, to skip on from there
            Special::Rimeskip => {
                let frost = tinted(&profile, Element::Frost);
                let from = self.held_point(weapon, view, false);
                let first = Combat::floor_at(arena, feet + flat * 4.5 * vpm + up * 0.6 * vpm) + up * 0.2 * vpm;
                self.launch(&frost, Some(Missile { shot: Shot::Shell, head: 0, power: true, size: 1.6 }), Form::Bolt, from, thrown(from, first, 0.34, BOMB_GRAVITY, vpm), base * 0.9, BOMB_GRAVITY);
                self.signed(Sig::Skip(SKIPS));
                self.cues.push(Cue::here(Sound::Swoop, 0.9).pitched(1.35));
                self.cues.push(Cue::here(Sound::Crack, 0.35).pitched(1.7));
            }
            // Rolled out onto the ground a few meters ahead, where it cracks open
            Special::Scuttlers => {
                let arcane = tinted(&profile, Element::Arcane);
                let from = self.held_point(weapon, view, false);
                let to = Combat::floor_at(arena, feet + flat * 5.5 * vpm + up * 0.6 * vpm) + up * 0.2 * vpm;
                self.launch(&arcane, Some(Missile { shot: Shot::Shell, head: 0, power: true, size: 1.6 }), Form::Bolt, from, thrown(from, to, 0.55, BOMB_GRAVITY, vpm), base * 0.5, BOMB_GRAVITY);
                self.signed(Sig::Brood);
                self.cues.push(Cue::here(Sound::Swoop, 0.8));
                self.cues.push(Cue::here(Sound::Skitter, 0.3).pitched(1.3));
            }
            // The first arrow now, the other two as the bow's drawn again
            Special::Wickfire => {
                let fire = tinted(&profile, Element::Fire);
                let from = self.held_point(weapon, view, true);
                let head = weapon.arrowhead();
                self.wick_arrow(arena, &fire, from, 0, base * 0.5, head);
                for k in 1..3 {
                    later(&mut self.pending, WICK_EVERY * WICK_LENGTH * k as f64, SigAct::Wick { k, local: local(from), head }, &fire, base * 0.5);
                }
            }
            Special::Lantern => {
                let holy = tinted(&profile, Element::Holy);
                let from = self.held_point(weapon, view, true);
                let to = self.ground_aim(arena, target, 30.0);
                self.launch(&holy, Some(Missile::arrow(weapon.arrowhead(), true)), Form::Bolt, from, lead(from, to, 55.0, ARROW_GRAVITY * 0.4, vpm) * 55.0 * vpm, base * 0.8, ARROW_GRAVITY * 0.4);
                self.signed(Sig::Lantern);
                self.cues.push(Cue::here(Sound::Twang, 0.9).pitched(1.15));
                self.cues.push(Cue::here(Sound::Chime, 0.5).pitched(1.7));
            }
            Special::Chainshot => {
                let from = self.held_point(weapon, view, false);
                let way = lead(from, target, CHAIN_SPEED, BALL_GRAVITY * 0.6, vpm);
                self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 2.0 }), Form::Bolt, from, way * CHAIN_SPEED * vpm, base * 1.5, BALL_GRAVITY * 0.6);
                self.signed(Sig::Chain);
                // Going off as a gun's shot does, see gun_report(), but harder
                self.gun_report(weapon, view, &profile, from, way, 1.6);
                self.kick += 0.06;
                self.cues.push(Cue::here(Sound::Chain, 0.9).pitched(0.75));
                self.cues.push(Cue::here(Sound::Whirr, 0.7).pitched(1.4));
            }
            // A cone of them across the ground ahead, each coming down a little further on
            Special::Caltrops => {
                let from = self.held_point(weapon, view, false);
                // Plain iron, so they trail no more than they should, all eighteen at once; drawn
                // glinting red, see signature_shot_instances()
                let iron = Rc::new(Profile { element: None, ..(*profile).clone() });
                self.zones.retain(|z| z.kind != ZoneKind::Signature(SigZone::Caltrops));
                self.zone(ZoneKind::Signature(SigZone::Caltrops), &profile, feet, Vector3::zeros(), 0.7, CALTROP_SECONDS + 1.6, base * 0.25);
                for _ in 0..CALTROPS {
                    let a = (random(&mut self.rng) - 0.5) * 1.05;
                    let d = 3.0 + 8.5 * random(&mut self.rng).powf(0.7);
                    let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * flat;
                    let to = ground_under(arena.world, feet + out * d * vpm + up * 1.5 * vpm, vpm) + up * 0.1 * vpm;
                    self.launch(&iron, Some(Missile { shot: Shot::Ball, head: 0, power: false, size: 1.5 }), Form::Bolt, from, thrown(from, to, 0.22 + 0.03 * d, BOMB_GRAVITY, vpm), base * 0.35, BOMB_GRAVITY);
                    self.signed(Sig::Caltrop);
                }
                self.gun_report(weapon, view, &profile, from, forward, 1.8);
                self.kick += 0.07;
                self.cues.push(Cue::here(Sound::Coins, 0.9).pitched(1.35));
            }
            Special::Riftshot => {
                let void = tinted(&profile, Element::Void);
                let from = self.held_point(weapon, view, false);
                let way = lead(from, target, 48.0, BALL_GRAVITY * 0.4, vpm);
                self.launch(&void, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 2.2 }), Form::Bolt, from, way * 48.0 * vpm, base * 1.2, BALL_GRAVITY * 0.4);
                self.signed(Sig::Rift);
                self.gun_report(weapon, view, &profile, from, way, 1.3);
                self.kick += 0.05;
                self.cues.push(Cue::here(Sound::Hex, 0.8).pitched(0.6));
                self.cues.push(Cue::here(Sound::Wail, 0.25).pitched(1.8));
            }
            // Fanned out over the player as the move begins, loosed one after another, see knives()
            Special::GloamingKnives => {
                let void = tinted(&profile, Element::Void);
                self.zones.retain(|z| z.kind != ZoneKind::Signature(SigZone::Knives));
                self.zone(ZoneKind::Signature(SigZone::Knives), &void, feet, Vector3::zeros(), 0.0, KNIVES_LENGTH, base * 0.9);
                self.cues.push(Cue::here(Sound::Summon, 0.8).pitched(1.7));
                self.cues.push(Cue::here(Sound::Shing, 0.4).pitched(0.6));
            }
            Special::SeveringDraw => {
                let frost = tinted(&profile, Element::Frost);
                later(&mut self.pending, DRAW_AT * DRAW_LENGTH, SigAct::Draw, &frost, base * 2.4);
                self.cues.push(Cue::here(Sound::Clang, 0.3).pitched(1.9));
            }
            Special::ViperThrusts => {
                let poison = tinted(&profile, Element::Poison);
                for k in 0..FANGS {
                    later(&mut self.pending, (VIPER_FIRST + VIPER_GAP * k as f64) * VIPER_LENGTH, SigAct::Fang(k), &poison, base * 0.55);
                }
                self.cues.push(Cue::here(Sound::Hiss, 0.6).pitched(1.4));
            }
            // The sweep in the weapon's own way, the wisps arcane
            Special::SoulGleaning => {
                let arcane = tinted(&profile, Element::Arcane);
                self.zones.retain(|z| z.kind != ZoneKind::Signature(SigZone::Gleaning));
                self.zone(ZoneKind::Signature(SigZone::Gleaning), &arcane, feet, Vector3::zeros(), 0.0, GLEAN_FLING * GLEAN_LENGTH + 0.4, base * 0.8);
                later(&mut self.pending, GLEAN_REAP * GLEAN_LENGTH, SigAct::Glean, &profile, base * 1.8);
                later(&mut self.pending, GLEAN_FLING * GLEAN_LENGTH, SigAct::Fling, &arcane, base * 0.8);
                self.cues.push(Cue::here(Sound::Wail, 0.25).pitched(1.6));
            }
            Special::Stormanvil => {
                let storm = tinted(&profile, Element::Storm);
                later(&mut self.pending, ANVIL_CHARGE * ANVIL_LENGTH, SigAct::Charge, &storm, base);
                later(&mut self.pending, ANVIL_SLAM * ANVIL_LENGTH, SigAct::Slam, &storm, base * 1.6);
                self.cues.push(Cue::here(Sound::Rumble, 0.5).pitched(1.4));
            }
            // The first stake now, the other two as the crossbow's swung on to each
            Special::Stormstakes => {
                let storm = tinted(&profile, Element::Storm);
                self.zones.retain(|z| z.kind != ZoneKind::Signature(SigZone::Stakes));
                self.zone(ZoneKind::Signature(SigZone::Stakes), &storm, feet, Vector3::zeros(), 0.9, STAKE_SECONDS + 0.6, base * 0.45);
                let from = self.held_point(weapon, view, false);
                self.stake(arena, &storm, from, 0, base * 0.8);
                for k in 1..STAKES {
                    later(&mut self.pending, STAKE_GAP * k as f64, SigAct::Stake { k, local: local(from) }, &storm, base * 0.8);
                }
            }
            Special::Litany => {
                let holy = tinted(&profile, Element::Holy);
                let from = self.held_point(weapon, view, false);
                let to = self.ground_aim(arena, target, 24.0);
                self.launch(&holy, None, Form::Bolt, from, (to - from).try_normalize(1e-6).unwrap_or(forward) * 30.0 * vpm, base * 0.5, 0.0);
                self.signed(Sig::Script);
                if let Some(p) = self.projectiles.last_mut() {
                    p.size = 1.9;
                }
                self.cues.push(Cue::here(Sound::Cast(Element::look(Some(Element::Holy)) as u8), 0.9).pitched(1.2));
                self.cues.push(Cue::here(Sound::Chime, 0.6).pitched(1.5));
            }
            // A ball of flame flung out of the bell as it rings, coming down where it's aimed
            Special::EmberKnell => {
                let fire = tinted(&profile, Element::Fire);
                let from = self.held_point(weapon, view, false);
                let to = self.ground_aim(arena, target, 18.0);
                let time = 0.45 + 0.025 * (to - from).norm() / vpm;
                self.launch(&fire, None, Form::Fireball, from, thrown(from, to, time, 1.0, vpm), base * 0.8, 1.0);
                self.signed(Sig::Knell);
                if let Some(p) = self.projectiles.last_mut() {
                    p.size = 1.7;
                }
                self.cues.push(Cue::here(Sound::Toll, 1.0).pitched(1.3));
                self.cues.push(Cue::here(Sound::Breath, 0.35).pitched(1.7));
            }
            // At what's aimed at, turning after it, or at the ground there
            Special::Bloodthread => {
                let blood = tinted(&profile, Element::Blood);
                let from = self.held_point(weapon, view, false);
                let aimed = Combat::aimed_foe(arena, target, 30.0).map(|i| arena.animals[i].center(vpm as u32));
                let to = aimed.unwrap_or_else(|| self.ground_aim(arena, target, 22.0));
                self.launch(&blood, None, Form::Lance, from, (to - from).try_normalize(1e-6).unwrap_or(forward) * 26.0 * vpm, base * 0.8, 0.0);
                self.signed(Sig::Thread);
                if let Some(p) = self.projectiles.last_mut() {
                    p.size = 1.4;
                    p.seek = if aimed.is_some() { 2.5 } else { 0.0 };
                }
                self.cues.push(Cue::here(Sound::Hex, 0.9).pitched(0.65));
                self.cues.push(Cue::here(Sound::Squelch, 0.4).pitched(0.75));
            }
            _ => {}
        }
    }

    /// The `k`th of a wickfire volley's arrows loosed from `from`: at the `k`th nearest ahead, or the
    /// nearest if there are fewer, or else at the crosshair, a little to one side
    fn wick_arrow(&mut self, arena: &Arena, profile: &Rc<Profile>, from: Vector3<f64>, k: u32, damage: f64, head: usize) {
        let view = arena.view;
        let vpm = view.vpm;
        let ahead = Combat::foes_ahead(arena, view.eye, view.forward(), 40.0, 0.55);
        let to = match ahead.get(k as usize).or(ahead.first()) {
            Some(&i) => arena.animals[i].center(vpm as u32),
            None => {
                let aim = self.ground_aim(arena, self.aim(arena), 32.0);
                view.eye + UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (k as f64 - 1.0) * 0.07) * (aim - view.eye)
            }
        };
        let way = lead(from, to, 52.0, ARROW_GRAVITY * 0.35, vpm);
        self.launch(profile, Some(Missile::arrow(head, true)), Form::Bolt, from, way * 52.0 * vpm, damage, ARROW_GRAVITY * 0.35);
        self.signed(Sig::Wick);
        self.cues.push(Cue::here(Sound::Twang, 0.9).pitched(0.95 + 0.07 * k as f32));
        self.cues.push(Cue::here(Sound::Fuse, 0.35).pitched(1.5));
    }

    /// The `k`th stake loosed from `from` onto the ground ahead: the middle one furthest on, then
    /// one to the left, then one to the right
    fn stake(&mut self, arena: &Arena, profile: &Rc<Profile>, from: Vector3<f64>, k: u32, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let flat = flat_of(view.forward());
        let side = flat.cross(&up);
        // Far enough on to be seen over the crossbow in hand
        let (on, across) = match k {
            0 => (13.0, 0.0),
            1 => (9.5, -4.5),
            _ => (9.5, 4.5),
        };
        let spot = ground_under(arena.world, feet + (flat * on + side * across) * vpm + up * 1.5 * vpm, vpm) + up * 0.05 * vpm;
        let way = lead(from, spot, 62.0, QUARREL_GRAVITY * 0.3, vpm);
        self.launch(profile, Some(Missile { shot: Shot::Quarrel, head: 0, power: true, size: 1.6 }), Form::Bolt, from, way * 62.0 * vpm, damage, QUARREL_GRAVITY * 0.3);
        self.signed(Sig::Stake);
        self.flashes.push(Flash { position: from, color: Element::Storm.light(), reach: 5.0, life: 0.15, lifetime: 0.15 });
        self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.85 + 0.08 * k as f32));
        self.cues.push(Cue::here(Sound::Zap, 0.5).pitched(1.3));
        self.kick += 0.025;
    }

    /// One of their parts whose time has come
    pub(super) fn signature_act(&mut self, arena: &mut Arena, act: SigAct, profile: &Rc<Profile>, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        match act {
            SigAct::Wick { k, local, head } => {
                let from = view.eye + view.rotation * local * vpm;
                self.wick_arrow(arena, profile, from, k, damage, head);
            }
            SigAct::Stake { k, local } => {
                let from = view.eye + view.rotation * local * vpm;
                self.stake(arena, profile, from, k, damage);
            }
            // A lunge as the blade comes out, and the cut flying on ahead out of it, a line of
            // frost hanging in the air through everything in the way, each thing along it held a
            // beat
            SigAct::Draw => {
                let lunge = Combat::room_ahead(arena, feet, flat).min(SEVER_LUNGE);
                self.dash = flat * (lunge / 0.31) * vpm + up * 1.0 * vpm;
                // Out from the chest straight through the nearest thing in front, if there's one
                // in reach, or else on along where it's looked, though never much up or down, so
                // it cuts what's down a slope or up one as well as what's level
                let chest = feet + up * 1.1 * vpm;
                let way = match Combat::foes_ahead(arena, chest, flat, SEVER_REACH, 0.9).first() {
                    Some(&i) => (arena.animals[i].center(vpm as u32) - chest).try_normalize(1e-6).unwrap_or(flat),
                    None => {
                        let pitch = forward.z.clamp(-0.14, 0.1);
                        flat * (1.0 - pitch * pitch).sqrt() + up * pitch
                    }
                };
                let from = chest + way * 1.0 * vpm;
                let mut to = chest + way * SEVER_REACH * vpm;
                if let Some(t) = solid_along(arena.world, from, to) {
                    to = from + (to - from) * t;
                }
                self.zone(ZoneKind::Signature(SigZone::Severance), profile, from, to - from, 1.3, SEVER_DELAY + 0.55, damage);
                if let Some(z) = self.zones.last_mut() {
                    z.tick = SEVER_DELAY;
                }
                for i in 0..arena.animals.len() {
                    let a = &arena.animals[i];
                    if a.health > 0.0 && !a.look.is_pet() && off_line(a.center(vpm as u32), from, to) / vpm - a.look.body().1 < 1.3 {
                        let s = &mut arena.animals[i].status;
                        s.staggered = s.staggered.max(0.5);
                        s.flash = s.flash.max(0.08);
                    }
                }
                self.flashes.push(Flash { position: view.eye, color: Element::Frost.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Shing, 1.0).pitched(1.25));
                self.cues.push(Cue::here(Sound::Dodge, 0.8).pitched(0.75));
            }
            // Out of the spear's point, at full stretch, on through what's in front and down onto
            // the ground some way on
            SigAct::Fang(k) => {
                let side = if k % 2 == 0 { 1.0 } else { -1.0 } * (0.02 + 0.015 * (k % 3) as f64);
                let tip = view.eye + view.rotation * Vector3::new(0.1 + side, 1.35, -0.14) * vpm;
                let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), side) * flat;
                let to = ground_under(arena.world, feet + out * 13.0 * vpm + up * 1.5 * vpm, vpm) + up * 0.5 * vpm;
                self.launch(profile, None, Form::Bolt, tip, (to - tip).try_normalize(1e-6).unwrap_or(forward) * 32.0 * vpm, damage, 0.0);
                self.signed(Sig::Fang(0));
                if let Some(p) = self.projectiles.last_mut() {
                    p.size = 1.4;
                }
                self.cues.push(Cue::here(Sound::Stab, 0.6).pitched(1.25 + 0.03 * k as f32));
                if k % 3 == 0 {
                    self.cues.push(Cue::here(Sound::Hiss, 0.3).pitched(1.5));
                }
            }
            // Everything in front cut, a wisp torn out of each, or out of the ground if nothing's
            // there to cut
            SigAct::Glean => {
                let reach = profile.stat("range").max(1.5) + 2.5;
                let look = Element::look(Some(Element::Arcane));
                for j in 0..20 {
                    let a = (0.5 - j as f64 / 19.0) * 2.0;
                    let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * flat;
                    let round = Vector3::new(out.y, -out.x, 0.0);
                    self.sparks.push(Spark { position: feet + out * 2.2 * vpm + up * 1.0 * vpm, velocity: round * 7.0 * vpm, look, life: 0.4, lifetime: 0.4, gravity: 0.0, slot: SLOT_SPARK });
                }
                self.cap_sparks();
                self.cues.push(Cue::here(Sound::HeavySwing, 1.0).pitched(0.75));
                self.cues.push(Cue::here(Sound::Shing, 0.6).pitched(0.7));
                let cut: Vec<usize> = Combat::foes_ahead(arena, view.eye, forward, reach + 1.0, 0.25).into_iter().filter(|&i| (arena.animals[i].center(vpm as u32) - view.eye).norm() / vpm - arena.animals[i].look.body().1 < reach).collect();
                let mut wisps = Vec::new();
                for &i in &cut {
                    let center = arena.animals[i].center(vpm as u32);
                    self.element_hit(center, Some(Element::Arcane), 1.4);
                    self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: feet, depth: 0, element: Some(Element::Arcane) });
                    if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                        let away = flat_of(arena.animals[i].position - feet);
                        let s = &mut arena.animals[i].status;
                        s.push = away * 5.0 * vpm;
                        s.staggered = s.staggered.max(0.4);
                    }
                    wisps.push(center);
                }
                if wisps.is_empty() {
                    let side = flat.cross(&up);
                    for j in [-1.0, 0.0, 1.0] {
                        wisps.push(ground_under(arena.world, feet + (flat * 5.0 + side * j * 1.8) * vpm + up * vpm, vpm) + up * 0.4 * vpm);
                    }
                }
                if let Some(z) = self.zones.iter_mut().rev().find(|z| z.kind == ZoneKind::Signature(SigZone::Gleaning)) {
                    z.path.extend(wisps);
                }
                self.cues.push(Cue::here(Sound::Summon, 0.5).pitched(1.8));
            }
            // Every wisp gleaned flung back out ahead, each turning after what's near
            SigAct::Fling => {
                let mut n = 3;
                if let Some(z) = self.zones.iter_mut().rev().find(|z| z.kind == ZoneKind::Signature(SigZone::Gleaning)) {
                    n = (z.owed as usize + z.path.len()).clamp(3, 8);
                    z.path.clear();
                    z.life = 0.0;
                }
                let from = view.eye + view.rotation * Vector3::new(0.12, 0.85, -0.1) * vpm;
                for j in 0..n {
                    let a = (j as f64 / (n - 1) as f64 - 0.5) * 0.9;
                    let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * flat;
                    let to = ground_under(arena.world, feet + out * 12.0 * vpm + up * vpm, vpm) + up * 0.6 * vpm;
                    self.launch(profile, None, Form::Missile, from, (to - from).try_normalize(1e-6).unwrap_or(forward) * 24.0 * vpm, damage, 0.0);
                    self.signed(Sig::Wisp);
                    if let Some(p) = self.projectiles.last_mut() {
                        p.size = 1.3;
                    }
                }
                self.cues.push(Cue::here(Sound::Cast(Element::look(Some(Element::Arcane)) as u8), 0.9).pitched(1.4));
                self.cues.push(Cue::here(Sound::Summon, 0.5).pitched(1.9));
            }
            // Lightning out of the sky onto the raised weapon's head, over the view, sparks raining
            // down off it, and crackling round it till it comes down
            SigAct::Charge => {
                let tip = view.eye + view.rotation * Vector3::from(ANVIL_TIP) * vpm;
                self.bolt_line(tip + (flat * 3.0 + up * 16.0) * vpm, tip, Element::look(Some(Element::Storm)));
                self.flashes.push(Flash { position: tip, color: Element::Storm.light(), reach: 14.0, life: 0.45, lifetime: 0.45 });
                self.sparks(tip, 10, 3.0, Element::look(Some(Element::Storm)), 1.0, 0.45);
                self.cues.push(Cue::here(Sound::Thunder, 1.0).pitched(1.15));
                self.cues.push(Cue::here(Sound::Zap, 0.7).pitched(0.8));
                self.shake = self.shake.max(0.45);
                self.zones.retain(|z| z.kind != ZoneKind::Signature(SigZone::Anvil));
                self.zone(ZoneKind::Signature(SigZone::Anvil), profile, tip, Vector3::zeros(), 0.0, (ANVIL_SLAM - ANVIL_CHARGE) * ANVIL_LENGTH, 0.0);
            }
            // Slammed down: a ring of lightning rolling out along the ground, a bolt leaping off it
            // onto each thing it reaches, or up off the ground if there's nothing
            SigAct::Slam => {
                // Far enough on to be seen, where the head comes down
                let center = ground_under(arena.world, feet + flat * 3.0 * vpm + up * vpm, vpm);
                let look = Element::look(Some(Element::Storm));
                self.blasts.push(Blast { position: center + up * 0.4 * vpm, look, radius: 1.6, life: 0.35, lifetime: 0.35 });
                self.sparks(center, 18, 6.0, look, 0.2, 0.3);
                self.flashes.push(Flash { position: center + up * vpm, color: Element::Storm.light(), reach: 16.0, life: 0.4, lifetime: 0.4 });
                self.ripple(profile, center, ANVIL_REACH, damage, Vec::new(), true, false);
                self.roll_ring(profile, center, ANVIL_REACH, Element::Storm);
                let near = Combat::foes_round(arena, center, ANVIL_REACH);
                for &i in &near {
                    let d = ((arena.animals[i].position - center).xy().norm() / vpm - arena.animals[i].look.body().1 - 0.4).max(0.0);
                    self.pending.push(Pending { delay: d / RING_SPEED, act: Act::Signature(SigAct::Leap(arena.animals[i].id)), profile: profile.clone(), damage: damage * 0.35 });
                }
                if near.is_empty() {
                    for j in 0..5 {
                        let r = 3.0 + 1.6 * j as f64;
                        let a = random(&mut self.rng) * TAU;
                        let at = ground_under(arena.world, center + Vector3::new(a.cos(), a.sin(), 0.0) * r * vpm + up * vpm, vpm);
                        self.pending.push(Pending { delay: r / RING_SPEED, act: Act::Signature(SigAct::Fork(at)), profile: profile.clone(), damage: 0.0 });
                    }
                }
                self.cues.push(Cue::at(Sound::Slam, center, 1.0).pitched(0.75));
                self.cues.push(Cue::at(Sound::Thunder, center, 0.8).pitched(1.5));
                self.cues.push(Cue::at(Sound::Quake, center, 0.6).pitched(1.2));
                self.shake = self.shake.max(1.0);
            }
            SigAct::Leap(id) => {
                let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
                    return;
                };
                let center = arena.animals[i].center(vpm as u32);
                self.arc(arena.animals[i].position + up * 0.1 * vpm, center + up * 0.3 * vpm);
                self.element_hit(center, Some(Element::Storm), 1.0);
                self.strike(arena, profile, i, Blow { damage, crit: false, heavy: false, from: center - up * vpm, depth: 1, element: Some(Element::Storm) });
                arena.animals[i].status.shocked = 0.6;
                self.cues.push(Cue::at(Sound::Crackle, center, 0.5).pitched(1.3));
            }
            SigAct::Fork(at) => {
                let to = at + (self.any_direction() * 0.8 + up * 2.2) * 1.2 * vpm;
                self.arc(at, to);
                self.sparks(at, 4, 3.0, Element::look(Some(Element::Storm)), 0.3, 0.25);
            }
            // Rung out in turn, a smaller ring rolling on from it to what's round it
            SigAct::Echo { id, depth } => {
                let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
                    return;
                };
                match self.zones.iter_mut().rev().find(|z| z.kind == ZoneKind::Signature(SigZone::Knell)) {
                    Some(z) if z.hit.contains(&id) => return,
                    Some(z) => z.hit.push(id),
                    None if depth > 1 => return,
                    None => {}
                }
                let at = arena.animals[i].position;
                self.ripple(profile, at, ECHO_REACH, damage, vec![id], false, false);
                self.roll_ring(profile, at, ECHO_REACH, Element::Fire);
                let look = Element::look(Some(Element::Fire));
                for j in 0..14 {
                    let a = j as f64 / 14.0 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.05);
                    self.sparks.push(Spark { position: at + out * 0.6 * vpm + up * 0.8 * vpm, velocity: out * 6.0 * vpm, look, life: 0.45, lifetime: 0.45, gravity: 0.0, slot: SLOT_SPARK });
                }
                self.cap_sparks();
                self.flashes.push(Flash { position: at + up * vpm, color: Element::Fire.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::at(Sound::Toll, at, 0.55).pitched(1.5 + 0.2 * depth as f32));
                if depth < ECHO_DEPTH {
                    for j in Combat::foes_round(arena, at, ECHO_REACH) {
                        if arena.animals[j].id == id {
                            continue;
                        }
                        let d = ((arena.animals[j].position - at).xy().norm() / vpm - arena.animals[j].look.body().1 - 0.4).max(0.0);
                        self.pending.push(Pending { delay: d / RING_SPEED, act: Act::Signature(SigAct::Echo { id: arena.animals[j].id, depth: depth + 1 }), profile: profile.clone(), damage: damage * 0.8 });
                    }
                }
            }
        }
    }

    /// One of their shots in flight: what trails off it, and for chainshot its balls scything
    /// through what's either side. True if it's gone. Coming to the end of its time, it comes
    /// down where it is, so nothing ever just vanishes.
    pub(super) fn signature_flight(&mut self, arena: &mut Arena, p: &mut Projectile, sig: Sig, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let at = p.position;
        // Shots from guns and crossbows, and knives of shadow, leave a glowing streak behind them, so
        // they're seen whatever's behind, sky or ground, never reaching back past the muzzle
        let tracer: Option<(Element, f64)> = match sig {
            Sig::Caltrop => Some((Element::Blood, 0.03)),
            Sig::Rift => Some((Element::Arcane, 0.05)),
            Sig::Stake => Some((Element::Storm, 0.05)),
            Sig::Knife => Some((Element::Void, 0.03)),
            _ => None,
        };
        if let Some((element, seconds)) = tracer
            && p.flown > 0.4 * vpm
        {
            let back = (p.velocity.norm() * seconds).min(p.flown);
            self.rays.push((at - p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x) * back, at, Element::look(Some(element))));
        }
        match sig {
            Sig::Chain => return self.chain_flight(arena, p, dt),
            // Glinting red as they fly
            Sig::Caltrop if random(&mut self.rng) < dt * 10.0 => self.mote(at, Some(Element::Blood), 1.2),
            // Bright violet flecks off the dark ball
            Sig::Rift if random(&mut self.rng) < dt * 30.0 => {
                let v = self.any_direction() * 0.5 * vpm;
                self.fine(at, v, Element::look(Some(Element::Arcane)), 0.3, 0.0);
            }
            // Crackling round the bolt
            Sig::Stake if random(&mut self.rng) < dt * 30.0 => {
                let off = self.any_direction() * 0.15 * vpm;
                self.fine(at + off, Vector3::zeros(), Element::look(Some(Element::Storm)), 0.15, 0.0);
            }
            // Glowing with what's in it as it flies, the fuse sputtering
            Sig::Skip(_) | Sig::Brood if random(&mut self.rng) < dt * 25.0 => {
                self.mote(at, p.profile.element, 1.5);
                let v = (self.any_direction() * 0.6 + Vector3::z()) * vpm;
                self.fine(at, v, Element::look(Some(Element::Fire)), 0.3, 0.3);
            }
            _ => {}
        }
        false
    }

    /// A chainshot's two balls spinning apart as it flies, each glowing hot and trailing sparks,
    /// smoke behind, and what's within the swath between them battered and knocked off its feet.
    /// Something big it wraps round, stopping there.
    fn chain_flight(&mut self, arena: &mut Arena, p: &mut Projectile, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let (a, b) = chain_balls(p, vpm);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let look = Element::look(Some(Element::Fire));
        let age = p.flown / (CHAIN_SPEED * vpm);
        for ball in [a, b] {
            // A streak of glowing tracer behind each ball as it goes round
            if age > 0.02 {
                self.rays.push((ball - p.velocity * age.min(0.04), ball, look));
            }
            if random(&mut self.rng) < dt * 35.0 {
                let v = (-way * 2.0 + self.any_direction() * 0.5) * vpm;
                self.sparks.push(Spark { position: ball, velocity: v, look, life: 0.25, lifetime: 0.25, gravity: 0.3, slot: SLOT_SPARK });
            }
        }
        if random(&mut self.rng) < dt * 20.0 {
            let v = self.any_direction() * 0.3 * vpm;
            self.sparks.push(Spark { position: p.position, velocity: v, look: 0, life: 0.9, lifetime: 0.9, gravity: -0.04, slot: SLOT_SMOKE });
        }
        self.cap_sparks();
        let half = chain_half(age);
        let step = p.velocity.norm() * dt;
        let caught: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let animal = &arena.animals[i];
                if animal.health <= 0.0 || animal.look.is_pet() || p.hit.contains(&animal.id) {
                    return false;
                }
                let body = animal.look.body().1 * vpm;
                let rel = animal.center(vpm as u32) - p.position;
                let along = rel.dot(&way);
                along > -body && along < step + body && (rel - way * along).norm() < half * vpm + body
            })
            .collect();
        let profile = p.profile.clone();
        for i in caught {
            let animal = &arena.animals[i];
            let center = animal.center(vpm as u32);
            let big = animal.boss.is_some() || animal.look.body().1 > 1.4;
            p.hit.push(animal.id);
            self.sparks(center, 10, 3.5, Element::look(None), 0.8, 0.35);
            self.cues.push(Cue::at(Sound::Clang, center, 0.7).pitched(0.85));
            self.strike(arena, &profile, i, Blow { damage: p.damage * if big { 1.6 } else { 1.0 }, crit: false, heavy: true, from: p.position - way * vpm, depth: 0, element: profile.element });
            if arena.animals[i].health > 0.0 {
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(if big { 0.25 } else { 1.0 });
                if !big {
                    s.push = way * 7.0 * vpm;
                }
            }
            // Wrapped round something too big to scythe through, and dropped there
            if big {
                self.cues.push(Cue::at(Sound::Chain, center, 0.9).pitched(0.6));
                self.signature_lands(arena, p, Sig::Chain, center);
                return true;
            }
            p.damage *= 0.9;
        }
        false
    }

    /// One of their shots striking animal `i` at `at`. True if it's gone, false if it goes on.
    pub(super) fn signature_hit(&mut self, arena: &mut Arena, p: &mut Projectile, sig: Sig, i: usize, at: Vector3<f64>) -> bool {
        let vpm = arena.view.vpm;
        let id = arena.animals[i].id;
        let profile = p.profile.clone();
        let center = arena.animals[i].center(vpm as u32);
        let from = p.position - p.velocity * 0.02;
        let hit = |c: &mut Combat, arena: &mut Arena, damage: f64, element: Option<Element>| {
            c.element_hit(at, element, 1.2);
            c.cues.push(Cue::at(Sound::Hit, at, 0.7));
            c.strike(arena, &profile, i, Blow { damage, crit: false, heavy: false, from, depth: 0, element });
        };
        match sig {
            // Its swath does what it does, as it flies, see chain_flight()
            Sig::Chain => return false,
            Sig::Skip(left) => self.skip(arena, p, at, left),
            Sig::Brood => self.brood(arena, p, at),
            Sig::Wick => {
                hit(self, arena, p.damage, profile.element);
                self.wick_stick(p, at, Some((id, at - center)));
            }
            Sig::Lantern => {
                hit(self, arena, p.damage, Some(Element::Holy));
                let over = arena.animals[i].position + Vector3::new(0.0, 0.0, (arena.animals[i].look.body().0 + 1.0) * vpm);
                self.hang_lantern(arena, p, over);
            }
            // A caltrop that hits something sticks in, then drops at its feet
            Sig::Caltrop => {
                hit(self, arena, p.damage, Some(Element::Blood));
                if arena.animals[i].health > 0.0 {
                    let s = &mut arena.animals[i].status;
                    s.bleed = s.bleed.max(4.0);
                    s.bleed_stacks = (s.bleed_stacks + 1).min(5);
                    s.bleed_dps = s.bleed_dps.max(p.damage * 0.25);
                }
                let feet = ground_under(arena.world, arena.animals[i].position + Vector3::new(0.0, 0.0, 0.5 * vpm), vpm);
                self.lay_caltrop(feet);
            }
            Sig::Rift => {
                hit(self, arena, p.damage, Some(Element::Void));
                self.open_rift(p, at);
            }
            Sig::RiftShard => hit(self, arena, p.damage, Some(Element::Void)),
            // Bursting into darkness in what it strikes
            Sig::Knife => {
                hit(self, arena, p.damage, Some(Element::Void));
                self.scarless = true;
                self.burst(arena, &profile, at, 1.1, p.damage * 0.35, 1, false, Some(Element::Void), Some(i));
                self.scarless = false;
            }
            // Through two, poisoning each, and stopping in the third
            Sig::Fang(n) => {
                hit(self, arena, p.damage, Some(Element::Poison));
                if arena.animals[i].health > 0.0 {
                    let s = &mut arena.animals[i].status;
                    s.poison = s.poison.max(4.0);
                    s.poison_stacks = (s.poison_stacks + 1).min(5);
                    s.poison_dps = s.poison_dps.max(p.damage * 0.12);
                }
                self.cues.push(Cue::at(Sound::Bite, at, 0.4).pitched(1.6));
                p.hit.push(id);
                if n < 2 {
                    p.trick = Trick::Signature(Sig::Fang(n + 1));
                    return false;
                }
            }
            Sig::Wisp => hit(self, arena, p.damage, Some(Element::Arcane)),
            // Through it, shocking it, and down into the ground at its feet
            Sig::Stake => {
                hit(self, arena, p.damage, Some(Element::Storm));
                arena.animals[i].status.shocked = 0.6;
                let feet = ground_under(arena.world, arena.animals[i].position + Vector3::new(0.0, 0.0, 0.5 * vpm), vpm);
                self.plant_stake(feet, Vector3::new(0.0, 0.0, -1.0));
            }
            Sig::Script => {
                hit(self, arena, p.damage, Some(Element::Holy));
                let feet = ground_under(arena.world, arena.animals[i].position + Vector3::new(0.0, 0.0, 0.5 * vpm), vpm);
                self.write_litany(p, feet);
            }
            Sig::Knell => {
                hit(self, arena, p.damage, Some(Element::Fire));
                let feet = ground_under(arena.world, arena.animals[i].position + Vector3::new(0.0, 0.0, 0.5 * vpm), vpm);
                self.knell(arena, p, feet);
            }
            Sig::Thread => {
                hit(self, arena, p.damage, Some(Element::Blood));
                self.bind_threads(arena, p, center);
            }
        }
        true
    }

    /// One of their shots coming down at `at`, on the ground, a wall or the water, or wherever it
    /// was when its time ran out
    pub(super) fn signature_lands(&mut self, arena: &mut Arena, p: &Projectile, sig: Sig, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        match sig {
            // The balls slamming into the ground or a wall with a crack, dirt and sparks flying
            Sig::Chain => {
                self.sparks(at, 14, 4.0, Element::look(None), 0.9, 0.4);
                for _ in 0..6 {
                    let v = (self.any_direction() + up * 1.3) * 3.0 * vpm;
                    self.drop(at, v, DROP_MUD, 0.9);
                }
                self.blasts.push(Blast { position: at, look: Element::look(p.profile.element.or(Some(Element::Fire))), radius: 0.5, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::at(Sound::Clang, at, 0.9).pitched(0.6));
                self.cues.push(Cue::at(Sound::Chain, at, 0.6).pitched(0.9));
                self.scarless = true;
                let profile = p.profile.clone();
                self.burst(arena, &profile, at, 1.1, p.damage * 0.5, 1, false, None, None);
                self.scarless = false;
            }
            Sig::Skip(left) => self.skip(arena, p, at, left),
            Sig::Brood => self.brood(arena, p, at),
            Sig::Wick => self.wick_stick(p, at, None),
            Sig::Lantern => {
                let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let mut over = at - way * 0.4 * vpm + up * 1.7 * vpm;
                if let Some(t) = solid_along(arena.world, at - way * 0.4 * vpm, over) {
                    over = at - way * 0.4 * vpm + (over - at + way * 0.4 * vpm) * t * 0.8;
                }
                self.hang_lantern(arena, p, over);
            }
            Sig::Caltrop => {
                let ground = ground_under(arena.world, at + up * 0.4 * vpm, vpm);
                self.lay_caltrop(ground);
            }
            Sig::Rift => {
                let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
                self.open_rift(p, at - way * 0.4 * vpm + up * 0.9 * vpm);
            }
            Sig::RiftShard => self.sparks(at, 6, 2.0, Element::look(Some(Element::Void)), 0.0, 0.35),
            // A knife stuck where it struck, a puff of darkness off it
            Sig::Knife => {
                let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
                self.sparks(at, 8, -2.0, Element::look(Some(Element::Void)), 0.0, 0.4);
                self.blasts.push(Blast { position: at, look: Element::look(Some(Element::Void)), radius: 0.45, life: 0.35, lifetime: 0.35 });
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                self.stuck.push(Stuck { position: at + way * 0.1 * vpm, rotation, look: Element::look(Some(Element::Void)), head: Some(0), quarrel: true, element: Some(Element::Void), life: 2.0, glowing: 1.5 });
                self.keep_stuck_few();
                self.cues.push(Cue::at(Sound::ArrowHit, at, 0.5).pitched(1.3));
            }
            // A splash of venom where it goes into the ground
            Sig::Fang(_) => {
                for _ in 0..6 {
                    let v = (self.any_direction() + up * 1.2) * 2.5 * vpm;
                    self.drop(at, v, DROP_VENOM, 0.9);
                }
                self.sparks(at, 4, 1.5, Element::look(Some(Element::Poison)), 0.4, 0.3);
                self.cues.push(Cue::at(Sound::Squelch, at, 0.3).pitched(1.6));
            }
            Sig::Wisp => self.sparks(at, 8, 2.0, Element::look(Some(Element::Arcane)), 0.0, 0.4),
            Sig::Stake => self.plant_stake(at, p.velocity.try_normalize(1e-6).unwrap_or(-up)),
            Sig::Script => {
                let ground = ground_under(arena.world, at + up * 0.8 * vpm, vpm);
                self.write_litany(p, ground);
            }
            Sig::Knell => {
                let ground = ground_under(arena.world, at + up * 0.8 * vpm, vpm);
                self.knell(arena, p, ground);
            }
            Sig::Thread => self.bind_threads(arena, p, at + up * 0.8 * vpm),
        }
    }

    /// A rimeskip charge coming down at `at` with `left` skips to go: ice bursting up, everything
    /// near chilled, bigger every skip, then skipping on, or at the last a great freezing burst
    fn skip(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>, left: u8) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let k = (SKIPS - left.min(SKIPS)) as f64;
        let last = left == 0;
        let radius = if last { 3.4 } else { 1.4 + 0.4 * k };
        let damage = p.damage * if last { 2.4 } else { 1.0 + 0.3 * k };
        let profile = p.profile.clone();
        // Skipping off the water as off the ground, freezing it where it touches
        let ground = Combat::floor_at(arena, at);
        self.scarless = true;
        self.burst(arena, &profile, ground + up * 0.3 * vpm, radius, damage, 0, false, Some(Element::Frost), None);
        self.scarless = false;
        // Spikes of ice thrust up where it touches, a ring of frost shards skating out over the
        // ground, and cold mist rolling off
        let frost = Element::look(Some(Element::Frost));
        self.swells.push(Swell { model: ICE_BURST, position: ground, radius: radius * 1.15, life: 1.0, lifetime: 1.0 });
        let turn = random(&mut self.rng) * TAU;
        for j in 0..18 {
            let a = turn + j as f64 / 18.0 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.12);
            self.sparks.push(Spark { position: ground + out * 0.4 * vpm + up * 0.2 * vpm, velocity: out * (5.0 + 2.0 * radius) * vpm, look: frost, life: 0.45, lifetime: 0.45, gravity: 0.4, slot: SLOT_SPARK });
        }
        for _ in 0..2 {
            let v = (self.any_direction() * 0.6 + up * 0.4) * vpm;
            self.sparks.push(Spark { position: ground + up * 0.5 * vpm, velocity: v, look: frost, life: 1.1, lifetime: 1.1, gravity: -0.02, slot: SLOT_CLOUD });
        }
        self.cap_sparks();
        self.flashes.push(Flash { position: ground + up * vpm, color: Element::Frost.light(), reach: 6.0 + 2.0 * radius, life: 0.3, lifetime: 0.3 });
        let caught = Combat::under(arena, ground, radius, 3.0);
        for &i in &caught {
            let boss = arena.animals[i].boss.is_some();
            let s = &mut arena.animals[i].status;
            s.slowed = s.slowed.max(2.5);
            s.slow = s.slow.max(0.45);
            if last && !boss {
                s.frozen = s.frozen.max(1.6);
            }
        }
        self.cues.push(Cue::at(Sound::Crack, ground, 0.8).pitched(1.35 - 0.12 * k as f32));
        if last {
            for j in 0..5 {
                let a = j as f64 / 5.0 * TAU + random(&mut self.rng);
                self.swells.push(Swell { model: ICE_BURST, position: ground + Vector3::new(a.cos(), a.sin(), 0.0) * 2.2 * vpm, radius: 1.3, life: 1.1, lifetime: 1.1 });
            }
            self.cues.push(Cue::at(Sound::Crack, ground, 1.0).pitched(0.6));
            self.shake = self.shake.max(0.6);
            return;
        }
        // On along the way it was going, low, past what the burst caught
        let way = flat_of(p.velocity);
        let next = Combat::floor_at(arena, ground + way * SKIP_HOP * vpm + up * vpm) + up * 0.15 * vpm;
        let from = ground + up * 0.35 * vpm;
        let mut hop = self.offshoot(p, from, thrown(from, next, SKIP_TIME, SKIP_GRAVITY, vpm), p.damage, Trick::Signature(Sig::Skip(left - 1)));
        hop.size = p.size;
        hop.gravity = SKIP_GRAVITY;
        hop.hit.extend(caught.iter().map(|&i| arena.animals[i].id));
        self.projectiles.push(hop);
    }

    /// Scuttling charges' bomb cracking open at `at`: six charges spilling out, each going after
    /// one of the nearest, or scattering if there's nothing near
    fn brood(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let ground = Combat::floor_at(arena, at);
        let look = Element::look(Some(Element::Arcane));
        self.sparks(ground + up * 0.3 * vpm, 14, 3.0, look, 0.4, 0.4);
        self.blasts.push(Blast { position: ground + up * 0.3 * vpm, look, radius: 0.7, life: 0.35, lifetime: 0.35 });
        self.flashes.push(Flash { position: ground + up * 0.5 * vpm, color: Element::Arcane.light(), reach: 5.0, life: 0.25, lifetime: 0.25 });
        self.cues.push(Cue::at(Sound::Clang, ground, 0.7).pitched(1.6));
        self.cues.push(Cue::at(Sound::Skitter, ground, 1.0));
        let near: Vec<(u32, Vector3<f64>)> = Combat::foes_round(arena, ground, 18.0).into_iter().take(SCUTTLERS).map(|i| (arena.animals[i].id, arena.animals[i].position)).collect();
        let away = flat_of(ground - arena.view.eye);
        for k in 0..SCUTTLERS {
            // Off in a fan at what each is after, or if there's nothing, scattering in a wide fan
            // on away from the thrower, going off one after another where they're to be seen
            let target = (!near.is_empty()).then(|| near[k % near.len()]);
            let out = match target {
                Some((_, at)) => UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (k as f64 - 2.5) * 0.2) * flat_of(at - ground),
                None => UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (k as f64 / (SCUTTLERS - 1) as f64 - 0.5) * 1.5) * away,
            };
            let target = target.map(|t| t.0);
            let life = if target.is_some() { 3.4 } else { 0.45 + 0.07 * k as f64 };
            self.zone(ZoneKind::Signature(SigZone::Scuttler), &p.profile, ground + out * 0.35 * vpm + up * 0.12 * vpm, out * SCUTTLE_SPEED * vpm, 1.7, life, p.damage * 2.2);
            if let Some(z) = self.zones.last_mut() {
                z.target = target;
                // A moment spilling out before they're off
                z.tick = 0.15;
            }
        }
    }

    /// A wickfire arrow stuck fast at `at`, in what it struck, `on` its id and where on it, or in
    /// the ground
    fn wick_stick(&mut self, p: &Projectile, at: Vector3<f64>, on: Option<(u32, Vector3<f64>)>) {
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let kind = ZoneKind::Signature(SigZone::Wick { way: [way.x as f32, way.y as f32, way.z as f32] });
        self.zone(kind, &p.profile, at, on.map_or(Vector3::zeros(), |o| o.1), 2.2, WICK_SECONDS, p.damage * 3.2);
        if let Some(z) = self.zones.last_mut() {
            z.target = on.map(|o| o.0);
        }
        self.cues.push(Cue::at(Sound::Fuse, at, 0.6).pitched(1.4));
        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.6));
    }

    /// A lantern of light hung at `at`
    fn hang_lantern(&mut self, arena: &Arena, p: &Projectile, at: Vector3<f64>) {
        let holy = tinted(&p.profile, Element::Holy);
        self.zone(ZoneKind::Signature(SigZone::Lantern), &holy, at, Vector3::zeros(), LANTERN_REACH, LANTERN_SECONDS, p.damage * 1.1);
        let ground = Combat::floor_at(arena, at);
        if let Some(z) = self.zones.last_mut() {
            z.tick = 0.3;
            // Where its glow lies on the ground, for its circle to be drawn round
            z.path = vec![ground];
        }
        self.swells.push(Swell { model: PILLAR, position: ground, radius: 0.8, life: 0.7, lifetime: 0.7 });
        self.flashes.push(Flash { position: at, color: Element::Holy.light(), reach: 10.0, life: 0.35, lifetime: 0.35 });
        self.sparks(at, 12, 2.5, Element::look(Some(Element::Holy)), -0.1, 0.5);
        self.cues.push(Cue::at(Sound::Summon, at, 0.8).pitched(1.5));
        self.cues.push(Cue::at(Sound::Chime, at, 0.7).pitched(1.2));
    }

    /// A glowing ring of `element` set rolling out from `center` along the ground to `reach`
    /// meters, alongside a ripple doing what it does
    fn roll_ring(&mut self, profile: &Rc<Profile>, center: Vector3<f64>, reach: f64, element: Element) {
        self.zone(ZoneKind::Signature(SigZone::Ring(element)), profile, center, Vector3::zeros(), reach, (reach - 0.4) / RING_SPEED + 0.25, 0.0);
    }

    /// A caltrop come to rest at `at`, among the rest
    fn lay_caltrop(&mut self, at: Vector3<f64>) {
        let vpm = self.vpm;
        if let Some(z) = self.zones.iter_mut().rev().find(|z| z.kind == ZoneKind::Signature(SigZone::Caltrops))
            && z.path.len() < CALTROPS + 6
        {
            z.path.push(at + Vector3::new(0.0, 0.0, 0.06 * vpm));
        }
        self.sparks(at, 2, 1.2, Element::look(Some(Element::Blood)), 0.6, 0.25);
        if random(&mut self.rng) < 0.3 {
            self.cues.push(Cue::at(Sound::ArrowHit, at, 0.3).pitched(1.9));
        }
    }

    /// A rift torn open at `at`
    fn open_rift(&mut self, p: &Projectile, at: Vector3<f64>) {
        self.zone(ZoneKind::Signature(SigZone::Rift), &p.profile, at, Vector3::zeros(), RIFT_REACH, RIFT_SECONDS, p.damage * 0.4);
        if let Some(z) = self.zones.last_mut() {
            z.tick = 0.45;
        }
        let look = Element::look(Some(Element::Void));
        self.blasts.push(Blast { position: at, look, radius: 1.0, life: 0.5, lifetime: 0.5 });
        self.sparks(at, 18, -3.0, look, 0.0, 0.5);
        self.flashes.push(Flash { position: at, color: Element::Void.light(), reach: 8.0, life: 0.3, lifetime: 0.3 });
        self.cues.push(Cue::at(Sound::Summon, at, 0.8).pitched(0.45));
        self.cues.push(Cue::at(Sound::Wail, at, 0.35).pitched(1.5));
    }

    /// A stake stuck in the ground at `at`, pointing `way`, for the lightning to string to
    fn plant_stake(&mut self, at: Vector3<f64>, way: Vector3<f64>) {
        let vpm = self.vpm;
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
        let look = Element::look(Some(Element::Storm));
        self.stuck.push(Stuck { position: at + way * 0.12 * vpm, rotation, look, head: Some(0), quarrel: true, element: Some(Element::Storm), life: STAKE_SECONDS + 0.4, glowing: STAKE_SECONDS });
        self.keep_stuck_few();
        if let Some(z) = self.zones.iter_mut().rev().find(|z| z.kind == ZoneKind::Signature(SigZone::Stakes))
            && z.path.len() < STAKES as usize
        {
            z.path.push(at + Vector3::new(0.0, 0.0, 0.05 * vpm));
        }
        self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 5.0, life: 0.2, lifetime: 0.2 });
        self.sparks(at, 8, 3.0, look, 0.2, 0.25);
        self.cues.push(Cue::at(Sound::Crackle, at, 0.6));
        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.6).pitched(0.8));
    }

    /// A litany circle begun round `center`
    fn write_litany(&mut self, p: &Projectile, center: Vector3<f64>) {
        self.zone(ZoneKind::Signature(SigZone::Litany), &p.profile, center, Vector3::zeros(), LITANY_RADIUS, LITANY_BLAZE + 0.7, p.damage * 5.0);
        self.flashes.push(Flash { position: center, color: Element::Holy.light(), reach: 8.0, life: 0.3, lifetime: 0.3 });
        self.sparks(center, 10, 2.0, Element::look(Some(Element::Holy)), -0.1, 0.5);
        self.cues.push(Cue::at(Sound::Hex, center, 0.6).pitched(1.7));
    }

    /// An ember knell going off at `center`: its ring rolling out, and each thing it reaches set
    /// to ring out in turn as it gets there
    fn knell(&mut self, arena: &mut Arena, p: &Projectile, center: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = p.profile.clone();
        self.zones.retain(|z| z.kind != ZoneKind::Signature(SigZone::Knell));
        self.zone(ZoneKind::Signature(SigZone::Knell), &profile, center, Vector3::zeros(), KNELL_REACH, 3.2, p.damage * 0.75);
        self.ripple(&profile, center, KNELL_REACH, p.damage * 1.3, Vec::new(), true, false);
        self.roll_ring(&profile, center, KNELL_REACH, Element::Fire);
        let look = Element::look(Some(Element::Fire));
        self.blasts.push(Blast { position: center + up * 0.5 * vpm, look, radius: 1.3, life: 0.45, lifetime: 0.45 });
        self.sparks(center + up * 0.5 * vpm, 16, 4.0, look, -0.1, 0.5);
        self.flashes.push(Flash { position: center + up * vpm, color: Element::Fire.light(), reach: 14.0, life: 0.4, lifetime: 0.4 });
        self.cues.push(Cue::at(Sound::Toll, center, 1.0).pitched(0.95));
        self.cues.push(Cue::at(Sound::Burst, center, 0.6).pitched(1.2));
        self.shake = self.shake.max(0.4);
        for i in Combat::foes_round(arena, center, KNELL_REACH) {
            let d = ((arena.animals[i].position - center).xy().norm() / vpm - arena.animals[i].look.body().1 - 0.4).max(0.0);
            self.pending.push(Pending { delay: d / RING_SPEED, act: Act::Signature(SigAct::Echo { id: arena.animals[i].id, depth: 1 }), profile: profile.clone(), damage: p.damage * 0.75 });
        }
    }

    /// A hex of blood hanging at `hub`, bound to the nearest round it
    fn bind_threads(&mut self, arena: &Arena, p: &Projectile, hub: Vector3<f64>) {
        let linked: Vec<usize> = Combat::foes_round(arena, hub, THREAD_REACH).into_iter().take(THREADS).collect();
        let mut last = [0.0f32; THREADS];
        for (k, &i) in linked.iter().enumerate() {
            last[k] = arena.animals[i].health as f32;
        }
        let life = if linked.is_empty() { 2.5 } else { THREAD_SECONDS };
        self.zone(ZoneKind::Signature(SigZone::Bloodthread { last }), &p.profile, hub, Vector3::zeros(), THREAD_REACH, life, p.damage * 0.6);
        if let Some(z) = self.zones.last_mut() {
            z.hit = linked.iter().map(|&i| arena.animals[i].id).collect();
        }
        self.flashes.push(Flash { position: hub, color: Element::Blood.light(), reach: 8.0, life: 0.3, lifetime: 0.3 });
        self.sparks(hub, 12, 2.5, Element::look(Some(Element::Blood)), 0.8, 0.5);
        self.cues.push(Cue::at(Sound::Hex, hub, 0.8).pitched(0.6));
        self.cues.push(Cue::at(Sound::Squelch, hub, 0.5).pitched(0.7));
    }

    /// One of their zones going on, see zones()
    pub(super) fn signature_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: SigZone, dt: f64) {
        match kind {
            SigZone::Scuttler => self.scuttle(arena, z, dt),
            SigZone::Wick { way } => self.wick_burns(arena, z, Vector3::new(way[0] as f64, way[1] as f64, way[2] as f64), dt),
            SigZone::Lantern => self.lantern(arena, z, dt),
            SigZone::Caltrops => self.caltrops(arena, z, dt),
            SigZone::Rift => self.rift(arena, z, dt),
            SigZone::Knives => self.knives(arena, z, dt),
            SigZone::Severance => self.severance(arena, z, dt),
            SigZone::Gleaning => self.gleaning(arena, z, dt),
            SigZone::Anvil => {
                let view = arena.view;
                z.position = view.eye + view.rotation * Vector3::from(ANVIL_TIP) * view.vpm;
                if random(&mut self.rng) < dt * 45.0 {
                    let off = self.any_direction() * 0.25 * view.vpm;
                    self.fine(z.position + off, -off * 2.0, Element::look(Some(Element::Storm)), 0.15, 0.0);
                }
            }
            SigZone::Stakes => self.stakes(arena, z, dt),
            SigZone::Litany => self.litany(arena, z, dt),
            SigZone::Knell => {}
            // A ring rolling out glowing, drawn in signature_zone_instances(); flames licking up
            // off a ring of fire here and there as it goes
            SigZone::Ring(element) => {
                let age = z.lifetime - z.life;
                if element == Element::Fire && random(&mut self.rng) < dt * 30.0 * (z.radius / KNELL_REACH) {
                    let a = random(&mut self.rng) * TAU;
                    let at = z.position + Vector3::new(a.cos(), a.sin(), 0.0) * ring_reached(age, z.radius) * arena.view.vpm + Vector3::new(0.0, 0.0, 0.3 * arena.view.vpm);
                    self.blasts.push(Blast { position: at, look: Element::look(Some(Element::Fire)), radius: 0.35, life: 0.35, lifetime: 0.35 });
                }
            }
            SigZone::Bloodthread { last } => self.threads(arena, z, last, dt),
        }
    }

    /// A clockwork charge spilling out, scuttling along the ground after what it's after, clinging
    /// on once it's there, and going off a moment after
    fn scuttle(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        if z.tick > 0.0 {
            return;
        }
        let goal = z.target.and_then(|id| arena.animals.iter().position(|a| a.id == id && a.health > 0.0));
        if z.dwell > 0.0 {
            z.dwell += dt;
            if let Some(i) = goal {
                z.position = arena.animals[i].center(vpm as u32) + z.velocity;
            }
            if random(&mut self.rng) < dt * 40.0 {
                let v = self.any_direction() * 1.5 * vpm;
                self.fine(z.position, v, Element::look(Some(Element::Arcane)), 0.25, 0.0);
            }
            // Blinking faster as it's about to go
            if random(&mut self.rng) < dt * 10.0 {
                self.flashes.push(Flash { position: z.position, color: Element::Arcane.light(), reach: 3.0, life: 0.06, lifetime: 0.06 });
            }
            if z.dwell > 0.5 {
                z.life = 0.0;
            }
            return;
        }
        let mut way = flat_of(z.velocity);
        match goal {
            Some(i) => {
                let a = &arena.animals[i];
                let to = a.position - z.position;
                if to.xy().norm() < (a.look.body().1 + 0.4) * vpm {
                    // Clung on, a little in from where it reached
                    let center = a.center(vpm as u32);
                    z.dwell = 1e-3;
                    z.velocity = (z.position - center) * 0.6;
                    self.cues.push(Cue::at(Sound::Clang, z.position, 0.4).pitched(2.0));
                    return;
                }
                way = way.lerp(&flat_of(to), (6.0 * dt).min(1.0)).try_normalize(1e-6).unwrap_or(way);
            }
            // What it was after is gone: off on its own a moment, then it goes
            None if z.target.is_some() => {
                z.target = None;
                z.life = z.life.min(0.6);
            }
            None => {}
        }
        let next = z.position + way * SCUTTLE_SPEED * vpm * dt;
        z.position = Combat::floor_at(arena, next) + up * 0.15 * vpm;
        z.velocity = way * SCUTTLE_SPEED * vpm;
        // A glowing trail kicked up behind it, to be seen running
        if random(&mut self.rng) < dt * 20.0 {
            let v = (-way * 1.5 + up * 1.2 + self.any_direction() * 0.4) * vpm;
            self.sparks.push(Spark { position: z.position, velocity: v, look: Element::look(Some(Element::Arcane)), life: 0.35, lifetime: 0.35, gravity: 0.8, slot: SLOT_SPARK });
            self.cap_sparks();
        }
        if random(&mut self.rng) < dt * 0.8 {
            self.cues.push(Cue::at(Sound::Skitter, z.position, 0.3).pitched(1.4 + 0.3 * random(&mut self.rng) as f32));
        }
    }

    /// A wickfire arrow's wick burning down, spitting sparks, on what it's in or in the ground
    fn wick_burns(&mut self, arena: &mut Arena, z: &mut Zone, way: Vector3<f64>, dt: f64) {
        let vpm = arena.view.vpm;
        if let Some(id) = z.target {
            match arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                Some(i) => z.position = arena.animals[i].center(vpm as u32) + z.velocity,
                None => z.target = None,
            }
        }
        let tail = z.position - way * 0.55 * vpm;
        let fire = Element::look(Some(Element::Fire));
        // A ring on the ground round it, how far it'll throw fire when it goes
        let ground = ground_under(arena.world, z.position, vpm);
        self.marks.push((ground + Vector3::new(0.0, 0.0, 0.1 * vpm), z.radius));
        if random(&mut self.rng) < dt * 40.0 {
            let v = (self.any_direction() * 1.2 + Vector3::z() * 1.5) * vpm;
            self.fine(tail, v, fire, 0.35, 0.6);
        }
        if random(&mut self.rng) < dt * 14.0 {
            let v = (self.any_direction() * 0.8 + Vector3::z() * 2.0) * vpm;
            self.sparks.push(Spark { position: tail, velocity: v, look: fire, life: 0.5, lifetime: 0.5, gravity: 0.5, slot: SLOT_SPARK });
            self.cap_sparks();
        }
        // Flaring faster as it's about to go
        if z.life < 0.5 && random(&mut self.rng) < dt * 14.0 {
            self.flashes.push(Flash { position: tail, color: Element::Fire.light(), reach: 3.0, life: 0.08, lifetime: 0.08 });
        }
    }

    /// A lantern hanging: its glow on the ground round it healing the player in it, and every
    /// second a ray of light searing each of the nearest, or reaching down to the ground if
    /// there's nothing near
    fn lantern(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let ground = z.path.first().copied().unwrap_or(z.position - up * 1.7 * vpm);
        let holy = Element::look(Some(Element::Holy));
        // Motes of light drifting down off it
        if random(&mut self.rng) < dt * 12.0 {
            let off = self.any_direction() * 0.35 * vpm;
            self.sparks.push(Spark { position: z.position + off, velocity: Vector3::new(off.x, off.y, -0.8 * vpm), look: holy, life: 0.8, lifetime: 0.8, gravity: 0.0, slot: SLOT_SPARK });
            self.cap_sparks();
        }
        if random(&mut self.rng) < dt * 10.0 {
            let a = random(&mut self.rng) * TAU;
            let r = random(&mut self.rng).sqrt() * LANTERN_GLOW * vpm;
            self.mote(ground + Vector3::new(a.cos() * r, a.sin() * r, 0.15 * vpm), Some(Element::Holy), 1.5);
        }
        if let Some(feet) = arena.player
            && (feet - ground).xy().norm() < LANTERN_GLOW * vpm
            && (feet.z - ground.z).abs() < 4.0 * vpm
        {
            self.heal += 3.0 * dt;
        }
        // Where its rays reach while they're searing, after where its glow lies, for them to be
        // drawn to, see signature_zone_instances(): what's being seared, or the ground under it
        z.path.truncate(1);
        if z.dwell > 0.0 {
            z.dwell -= dt;
            let to: Vec<Vector3<f64>> = z.hit.iter().filter_map(|&id| arena.animals.iter().find(|a| a.id == id && a.health > 0.0)).map(|a| a.center(vpm as u32)).collect();
            if z.hit.is_empty() {
                z.path.push(ground + up * 0.05 * vpm);
            }
            z.path.extend(to);
        }
        if z.tick > 0.0 {
            return;
        }
        z.tick = 1.0;
        z.dwell = 0.22;
        let seen: Vec<usize> = Combat::foes_round(arena, z.position, LANTERN_REACH).into_iter().filter(|&i| solid_along(arena.world, z.position, arena.animals[i].center(vpm as u32)).is_none()).take(4).collect();
        z.hit = seen.iter().map(|&i| arena.animals[i].id).collect();
        let profile = z.profile.clone();
        for i in seen {
            let center = arena.animals[i].center(vpm as u32);
            self.element_hit(center, Some(Element::Holy), 1.2);
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: 0, element: Some(Element::Holy) });
        }
        self.flashes.push(Flash { position: z.position, color: Element::Holy.light(), reach: 9.0, life: 0.2, lifetime: 0.2 });
        self.cues.push(Cue::at(Sound::Chime, z.position, 0.5).pitched(1.3));
    }

    /// Caltrops lying on the ground: every quarter second what's standing on any of them slowed,
    /// and every half second cut, bleeding
    fn caltrops(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        // A glint off one now and then
        if !z.path.is_empty() && random(&mut self.rng) < dt * 4.0 {
            let at = z.path[(random(&mut self.rng) * z.path.len() as f64) as usize % z.path.len()];
            self.fine(at + Vector3::new(0.0, 0.0, 0.1 * vpm), Vector3::new(0.0, 0.0, 0.3 * vpm), Element::look(Some(Element::Holy)), 0.3, 0.0);
        }
        if z.tick > 0.0 || z.life < 0.3 {
            return;
        }
        z.tick = 0.25;
        z.dwell = 1.0 - z.dwell;
        let cut = z.dwell > 0.5;
        let on: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                a.health > 0.0 && !a.look.is_pet() && z.path.iter().any(|c| (a.position - c).xy().norm() < (0.6 + a.look.body().1 * 0.6) * vpm && (a.position.z - c.z).abs() < 1.5 * vpm)
            })
            .collect();
        let profile = z.profile.clone();
        for i in on {
            {
                let s = &mut arena.animals[i].status;
                s.slowed = s.slowed.max(0.6);
                s.slow = s.slow.max(0.55);
            }
            if cut {
                let at = arena.animals[i].position + Vector3::new(0.0, 0.0, 0.2 * vpm);
                self.sparks(at, 3, 1.5, Element::look(Some(Element::Blood)), 1.0, 0.3);
                self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: at, depth: 1, element: Some(Element::Blood) });
                if arena.animals[i].health > 0.0 {
                    let s = &mut arena.animals[i].status;
                    s.bleed = s.bleed.max(3.0);
                    s.bleed_stacks = (s.bleed_stacks + 1).min(4);
                    s.bleed_dps = s.bleed_dps.max(z.damage * 0.4);
                }
            }
        }
    }

    /// A rift hanging open: darkness drawn into it, and a shard spat out of it every so often at
    /// the nearest it can see, or at the ground round it
    fn rift(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let look = Element::look(Some(Element::Void));
        if random(&mut self.rng) < dt * 18.0 {
            self.sparks(z.position, 1, -2.5, look, 0.0, 0.45);
        }
        if z.tick > 0.0 {
            return;
        }
        z.tick = 0.45;
        let target = Combat::foes_round(arena, z.position, RIFT_REACH).into_iter().find(|&i| solid_along(arena.world, z.position, arena.animals[i].center(vpm as u32)).is_none());
        let to = match target {
            Some(i) => arena.animals[i].center(vpm as u32),
            None => {
                let a = random(&mut self.rng) * TAU;
                let r = (1.5 + 3.0 * random(&mut self.rng)) * vpm;
                ground_under(arena.world, z.position + Vector3::new(a.cos() * r, a.sin() * r, 0.0), vpm)
            }
        };
        let way = (to - z.position).try_normalize(1e-6).unwrap_or(-Vector3::z());
        self.launch(&z.profile, None, Form::Bolt, z.position + way * 0.4 * vpm, way * 30.0 * vpm, z.damage, 0.0);
        self.signed(Sig::RiftShard);
        if let Some(p) = self.projectiles.last_mut() {
            p.size = 1.5;
        }
        self.cues.push(Cue::at(Sound::Hex, z.position, 0.35).pitched(1.6));
    }

    /// Gloaming knives fanning out over the player, then loosed one after another at the nearest
    /// ahead, or on ahead down onto the ground if there's nothing
    fn knives(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let age = z.lifetime - z.life;
        let mut loosed = z.owed as usize;
        while loosed < KNIVES && age >= KNIFE_FIRST + KNIFE_GAP * loosed as f64 {
            let from = knife_slot(view, loosed, age);
            let ahead = Combat::foes_ahead(arena, view.eye, view.forward(), 24.0, 0.4);
            let to = match ahead.get(loosed % ahead.len().max(1)) {
                Some(&i) => arena.animals[i].center(vpm as u32),
                None => {
                    let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
                    let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (2.5 - loosed as f64) * 0.1) * flat_of(view.forward());
                    ground_under(arena.world, feet + out * 14.0 * vpm + Vector3::new(0.0, 0.0, vpm), vpm)
                }
            };
            let way = (to - from).try_normalize(1e-6).unwrap_or(view.forward());
            self.launch(&z.profile, None, Form::Bolt, from, way * 40.0 * vpm, z.damage, 0.0);
            self.signed(Sig::Knife);
            if let Some(p) = self.projectiles.last_mut() {
                p.size = 1.4;
            }
            self.cues.push(Cue::here(Sound::Shing, 0.55).pitched(1.35 + 0.06 * loosed as f32));
            loosed += 1;
        }
        z.owed = loosed as f64;
        for k in loosed..KNIVES {
            if random(&mut self.rng) < dt * 12.0 {
                let v = self.any_direction() * 0.3 * vpm;
                self.fine(knife_slot(view, k, age), v, Element::look(Some(Element::Void)), 0.3, 0.0);
            }
        }
    }

    /// A line of frost hanging in the air where a severing draw went, then shattering, cutting
    /// and freezing everything along it
    fn severance(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let (a, b) = (z.position, z.position + z.velocity);
        let frost = Element::look(Some(Element::Frost));
        if z.dwell > 0.0 {
            return;
        }
        // Flying out from the blade to its full length, then hanging there, drawn in
        // signature_zone_instances()
        let grown = smooth((z.lifetime - z.life) / SEVER_GROW);
        if random(&mut self.rng) < dt * 40.0 {
            let at = a.lerp(&b, random(&mut self.rng) * grown);
            let v = self.any_direction() * 0.2 * vpm;
            self.fine(at, v, frost, 0.5, 0.05);
        }
        if z.tick > 0.0 {
            return;
        }
        z.dwell = 1.0;
        let profile = z.profile.clone();
        let cut: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let animal = &arena.animals[i];
                animal.health > 0.0 && !animal.look.is_pet() && off_line(animal.center(vpm as u32), a, b) / vpm - animal.look.body().1 < z.radius
            })
            .collect();
        for i in cut {
            let center = arena.animals[i].center(vpm as u32);
            self.element_hit(center, Some(Element::Frost), 1.6);
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: true, from: a, depth: 0, element: Some(Element::Frost) });
            if arena.animals[i].health > 0.0 {
                let boss = arena.animals[i].boss.is_some();
                let s = &mut arena.animals[i].status;
                if boss {
                    s.chill += 0.5;
                } else {
                    s.frozen = s.frozen.max(1.0);
                }
            }
        }
        // Spikes of ice bursting out along it where it shatters, none within a few meters of the
        // eye so none of them fills the view, wherever the lunge has carried it to by now
        let length = (b - a).norm() / vpm;
        let eye = arena.view.eye;
        let mut on = 2.5;
        while on < length {
            let at = a.lerp(&b, on / length);
            if (at - eye).norm() > 4.5 * vpm {
                self.swells.push(Swell { model: ICE_BURST, position: at, radius: 0.8, life: 0.8, lifetime: 0.8 });
            }
            on += 2.2;
        }
        for j in 0..16 {
            let at = a.lerp(&b, j as f64 / 15.0);
            let v = self.any_direction() * 4.0 * vpm;
            self.sparks.push(Spark { position: at, velocity: v, look: frost, life: 0.5, lifetime: 0.5, gravity: 1.2, slot: SLOT_SPARK });
        }
        self.cap_sparks();
        let middle = a.lerp(&b, 0.5);
        self.cues.push(Cue::at(Sound::Crack, middle, 1.0).pitched(1.45));
        self.cues.push(Cue::at(Sound::Shing, middle, 0.7).pitched(0.8));
        self.shake = self.shake.max(0.4);
    }

    /// Wisps streaming back from what was gleaned to the player's hand, each healing as it comes
    fn gleaning(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let hand = view.eye + view.rotation * Vector3::new(0.12, 0.45, -0.12) * vpm;
        let look = Element::look(Some(Element::Arcane));
        let mut k = 0;
        while k < z.path.len() {
            let w = z.path[k];
            let to = hand - w;
            let d = to.norm();
            if d < 0.35 * vpm {
                z.path.swap_remove(k);
                z.owed += 1.0;
                self.heal += 3.0;
                self.sparks(hand, 3, 1.0, look, 0.0, 0.25);
                continue;
            }
            let wobble = to.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x) * (self.time * 9.0 + k as f64 * 1.7).sin() * 0.6 * vpm * dt;
            z.path[k] = w + to / d * d.min(10.0 * vpm * dt) + wobble;
            if random(&mut self.rng) < dt * 20.0 {
                self.fine(z.path[k], Vector3::zeros(), look, 0.35, 0.0);
            }
            k += 1;
        }
    }

    /// Stakes in the ground, lightning strung between them, flickering, and every half second
    /// what's inside or crossing it shocked
    fn stakes(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let tops: Vec<Vector3<f64>> = z.path.iter().map(|p| p + Vector3::new(0.0, 0.0, 1.0 * vpm)).collect();
        let pairs: &[(usize, usize)] = match tops.len() {
            0 | 1 => &[],
            2 => &[(0, 1)],
            _ => &[(0, 1), (1, 2), (2, 0)],
        };
        // The lightning strung between them is drawn in signature_zone_instances(); sparks
        // crackling along one strand or another now and then
        z.owed += dt;
        if z.owed > 0.12 && !pairs.is_empty() {
            z.owed = 0.0;
            let (m, n) = pairs[(random(&mut self.rng) * pairs.len() as f64) as usize % pairs.len()];
            self.arc(tops[m], tops[n]);
        }
        z.dwell += dt;
        if z.dwell > 0.7 && !tops.is_empty() {
            z.dwell = 0.0;
            self.cues.push(Cue::at(Sound::Crackle, tops[0], 0.35).pitched(0.9));
        }
        if z.tick > 0.0 || pairs.is_empty() {
            return;
        }
        z.tick = 0.5;
        let caught: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                if a.health <= 0.0 || a.look.is_pet() || (a.position.z - z.path[0].z).abs() > 2.5 * vpm {
                    return false;
                }
                let center = a.center(vpm as u32);
                let near = pairs.iter().any(|&(m, n)| off_line(center, tops[m], tops[n]) / vpm - a.look.body().1 < 0.9);
                near || (tops.len() >= 3 && inside_triangle(a.position, z.path[0], z.path[1], z.path[2]))
            })
            .collect();
        let profile = z.profile.clone();
        for i in caught {
            let center = arena.animals[i].center(vpm as u32);
            self.element_hit(center, Some(Element::Storm), 1.0);
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: tops[0], depth: 1, element: Some(Element::Storm) });
            if arena.animals[i].health > 0.0 {
                let s = &mut arena.animals[i].status;
                s.shocked = 0.6;
                s.staggered = s.staggered.max(0.3);
            }
            self.cues.push(Cue::at(Sound::Zap, center, 0.5).pitched(1.2));
        }
    }

    /// A litany circle being written, a rune at a time, slowing what's inside more as it fills,
    /// then blazing up in a column of light
    fn litany(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let written = runes_written(age);
        let profile = z.profile.clone();
        let inside = Combat::under(arena, z.position, z.radius, 3.0);
        while (z.dwell as usize) < written {
            let k = z.dwell as usize;
            z.dwell += 1.0;
            let at = rune_at(z.position, k, vpm);
            self.sparks(at, 6, 1.5, Element::look(Some(Element::Holy)), -0.3, 0.5);
            self.cues.push(Cue::at(Sound::Chime, at, 0.6).pitched(1.0 + 0.07 * k as f32));
            for &i in &inside {
                self.strike(arena, &profile, i, Blow { damage: z.damage * 0.02, crit: false, heavy: false, from: at, depth: 1, element: Some(Element::Holy) });
            }
        }
        for &i in &inside {
            if arena.animals[i].health > 0.0 {
                let s = &mut arena.animals[i].status;
                s.slowed = s.slowed.max(0.3);
                s.slow = s.slow.max(0.3 + 0.05 * written as f64);
            }
        }
        if random(&mut self.rng) < dt * 12.0 {
            let a = random(&mut self.rng) * TAU;
            self.mote(z.position + Vector3::new(a.cos(), a.sin(), 0.0) * z.radius * vpm + up * 0.1 * vpm, Some(Element::Holy), 1.5);
        }
        if age >= LITANY_BLAZE && z.owed == 0.0 {
            z.owed = 1.0;
            self.swells.push(Swell { model: PILLAR, position: z.position, radius: 2.6, life: 0.9, lifetime: 0.9 });
            self.burst(arena, &profile, z.position + up * 0.8 * vpm, z.radius * 0.9, z.damage, 0, false, Some(Element::Holy), None);
            for i in Combat::under(arena, z.position, z.radius, 3.0) {
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(0.6);
            }
            self.flashes.push(Flash { position: z.position + up * 2.0 * vpm, color: Element::Holy.light(), reach: 18.0, life: 0.6, lifetime: 0.6 });
            self.cues.push(Cue::at(Sound::Toll, z.position, 1.0).pitched(1.3));
            self.cues.push(Cue::at(Sound::Summon, z.position, 0.7).pitched(0.8));
            self.shake = self.shake.max(0.6);
        }
    }

    /// A hex of blood holding what it binds together: whatever any of them has suffered since
    /// last frame, half of it each of the rest suffer too, blood running along the threads to them
    fn threads(&mut self, arena: &mut Arena, z: &mut Zone, mut last: [f32; THREADS], dt: f64) {
        let vpm = arena.view.vpm;
        let ids = z.hit.clone();
        let index: Vec<Option<usize>> = ids.iter().map(|&id| arena.animals.iter().position(|a| a.id == id && a.health > 0.0)).collect();
        let lost: Vec<f64> = index.iter().enumerate().map(|(k, i)| i.map_or(0.0, |i| (last[k] as f64 - arena.animals[i].health).max(0.0))).collect();
        let profile = z.profile.clone();
        let look = Element::look(Some(Element::Blood));
        z.owed -= dt;
        for (j, &i) in index.iter().enumerate() {
            let Some(i) = i else {
                continue;
            };
            let share: f64 = lost.iter().enumerate().filter(|&(k, _)| k != j).map(|(_, l)| l * THREAD_SHARE).sum();
            if share < 0.5 || arena.animals[i].health <= 0.0 {
                continue;
            }
            let center = arena.animals[i].center(vpm as u32);
            // Blood running along the thread to it
            for _ in 0..3 {
                let from = z.position.lerp(&center, random(&mut self.rng) * 0.6);
                self.sparks.push(Spark { position: from, velocity: (center - from) * 2.5, look, life: 0.35, lifetime: 0.35, gravity: 0.0, slot: SLOT_SPARK });
            }
            self.cap_sparks();
            let v = self.any_direction() * 1.5 * vpm;
            self.drop(center, v, DROP_BLOOD, 0.8);
            self.strike(arena, &profile, i, Blow { damage: share, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Blood) });
            if z.owed <= 0.0 {
                z.owed = 0.25;
                self.cues.push(Cue::at(Sound::Squelch, center, 0.35).pitched(1.2));
            }
        }
        // What it holds as it is now, a ring under each, and where each is for its thread to be
        // drawn to, see signature_zone_instances()
        z.path.clear();
        for (k, &id) in ids.iter().enumerate().take(THREADS) {
            last[k] = arena.animals.iter().find(|a| a.id == id && a.health > 0.0).map_or(-1.0, |a| a.health as f32);
            if let Some(a) = arena.animals.iter().find(|a| a.id == id && a.health > 0.0) {
                self.marks.push((a.position + Vector3::new(0.0, 0.0, 0.05 * vpm), a.look.body().1 + 0.3));
                z.path.push(a.center(vpm as u32));
            }
        }
        z.kind = ZoneKind::Signature(SigZone::Bloodthread { last });
        if random(&mut self.rng) < dt * 10.0 {
            self.mote(z.position, Some(Element::Blood), 1.5);
        }
    }

    /// One of their zones ending, see zone_ends()
    pub(super) fn signature_zone_ends(&mut self, arena: &mut Arena, z: &Zone, kind: SigZone) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = z.profile.clone();
        self.scarless = true;
        match kind {
            // Going off, clung on or wherever it got to
            SigZone::Scuttler => {
                self.burst(arena, &profile, z.position + up * 0.2 * vpm, z.radius, z.damage, 0, false, Some(Element::Arcane), None);
                for i in Combat::under(arena, z.position, z.radius, 2.5) {
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(0.4);
                }
            }
            SigZone::Wick { .. } => {
                self.burst(arena, &profile, z.position, z.radius, z.damage, 0, false, Some(Element::Fire), None);
                for i in Combat::under(arena, z.position, z.radius, 3.0) {
                    let s = &mut arena.animals[i].status;
                    s.burn = s.burn.max(3.0);
                    s.burn_dps = s.burn_dps.max(z.damage * 0.12);
                }
            }
            SigZone::Lantern => {
                self.sparks(z.position, 16, 3.0, Element::look(Some(Element::Holy)), -0.1, 0.6);
                self.flashes.push(Flash { position: z.position, color: Element::Holy.light(), reach: 10.0, life: 0.4, lifetime: 0.4 });
                self.cues.push(Cue::at(Sound::Toll, z.position, 0.5).pitched(1.8));
            }
            // Snapping shut: everything near dragged in, then torn at
            SigZone::Rift => {
                self.burst(arena, &profile, z.position, 4.0, 0.0, 1, true, Some(Element::Void), None);
                self.burst(arena, &profile, z.position, 2.6, z.damage * 3.0, 0, false, Some(Element::Void), None);
                self.cues.push(Cue::at(Sound::Wail, z.position, 0.5).pitched(1.2));
            }
            // Each stake blowing apart
            SigZone::Stakes => {
                for &at in &z.path {
                    self.burst(arena, &profile, at + up * 0.4 * vpm, 1.6, z.damage * 1.8, 0, false, Some(Element::Storm), None);
                }
                if let Some(&at) = z.path.first() {
                    self.cues.push(Cue::at(Sound::Thunder, at, 0.7).pitched(1.3));
                }
            }
            // The hex bursting, a last spray of blood out of each it held
            SigZone::Bloodthread { .. } => {
                for &id in &z.hit {
                    let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
                        continue;
                    };
                    let center = arena.animals[i].center(vpm as u32);
                    self.blasts.push(Blast { position: center, look: Element::look(Some(Element::Blood)), radius: 0.6, life: 0.4, lifetime: 0.4 });
                    self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Blood) });
                    self.heal += 2.0;
                }
                self.sparks(z.position, 14, 3.0, Element::look(Some(Element::Blood)), 1.0, 0.5);
                self.cues.push(Cue::at(Sound::Squelch, z.position, 0.7).pitched(0.8));
            }
            SigZone::Caltrops | SigZone::Knives | SigZone::Severance | SigZone::Gleaning | SigZone::Anvil | SigZone::Litany | SigZone::Knell | SigZone::Ring(_) => {}
        }
        self.scarless = false;
    }

    /// How one of their shots is drawn, if not as what it is would be: bombs tumbling, chainshot's
    /// balls and chain, caltrops glinting red, a rift's dark ball and a hex of blood churning. Shown
    /// however much else is about, as if they were right at the eye.
    pub(super) fn signature_shot_instances(&self, p: &Projectile, sig: Sig, view: &View) -> Option<Vec<(Vector3<f64>, AnimalInstance)>> {
        let vpm = view.vpm;
        let direction = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
        let look = Element::look(p.profile.element);
        let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 7.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), p.position.x * 0.3);
        // Drawn a little bigger the further off it is past 15 m, as other shots are, so it's
        // never lost to the eye
        let far = ((p.position - view.eye).norm() / vpm / 15.0).max(1.0).sqrt();
        Some(match sig {
            Sig::Skip(_) | Sig::Brood => {
                let (model, bomb) = self.effect_model(SLOT_BOMB, look);
                vec![(view.eye, place(model, bomb, BOMB_RES, Vector3::zeros(), p.position, spin, vpm, p.size * far, MOVING))]
            }
            // Iron balls heated red-hot, churning, the chain between
            Sig::Chain => {
                let (a, b) = chain_balls(p, vpm);
                let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Fire)));
                let mut out = vec![(view.eye, place(model, ball, MODEL_RES, Vector3::zeros(), a, spin, vpm, CHAINSHOT_BALL * far, MOVING)), (view.eye, place(model, ball, MODEL_RES, Vector3::zeros(), b, spin, vpm, CHAINSHOT_BALL * far, MOVING))];
                out.extend(self.line_instances(a, b, 0.0, LASH_CHAIN, 0.05).into_iter().map(|(_, i)| (view.eye, i)));
                out
            }
            // A barbed caltrop glowing red, tumbling
            Sig::Caltrop => self.caltrop_instances(p.position, spin, 1.2 * far),
            Sig::Rift | Sig::Thread => {
                let element = if sig == Sig::Rift { Element::Void } else { Element::Blood };
                let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(element)));
                let churn = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 6.0) * rotation;
                vec![(view.eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, churn, vpm, 0.09 * p.size * far, MOVING))]
            }
            // As the arrows and bolts they are, but shown however much else is about
            Sig::Stake | Sig::Lantern | Sig::Wick if p.arrow => {
                let instance = if p.shot == Shot::Quarrel {
                    let (model, quarrel) = self.effect_model(SLOT_QUARREL + p.head.min(QUARREL_HEADS - 1), look);
                    place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), p.position, rotation, vpm, p.size * far, MOVING)
                } else {
                    let (model, arrow) = self.effect_model(SLOT_ARROW + p.head.min(ARROW_HEADS - 1), look);
                    place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, far, MOVING)
                };
                vec![(view.eye, instance)]
            }
            _ => return None,
        })
    }

    /// A caltrop at `at`, turned `turn`, `size` as big as it's made: barbed spikes crossed through
    /// a glowing red heart
    fn caltrop_instances(&self, at: Vector3<f64>, turn: UnitQuaternion<f64>, size: f64) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = self.vpm;
        let (model, heart) = self.effect_model(SLOT_SHOT, Element::look(Some(Element::Blood)));
        let mut out = vec![(at, place(model, heart, SHOT_RES, Vector3::zeros(), at, turn, vpm, 4.0 * size, MOVING))];
        let (model, thorn) = self.model_at(THORN);
        // Two spikes crossed through it, as few as will read as barbed at a glance
        for way in [Vector3::new(1.0, 0.0, -0.35), Vector3::new(-0.3, 0.95, 0.35)] {
            let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &(turn * way).normalize()).unwrap_or_else(UnitQuaternion::identity);
            out.push((at, place(model, thorn, MODEL_RES, Vector3::new(ARROW_HEAD * 0.5, 0.0, 0.0), at, rotation, vpm, CALTROP_SPIKE * size, MOVING)));
        }
        out
    }

    /// How one of their zones is drawn, see flying_instances()
    pub(super) fn signature_zone_instances(&self, z: &Zone, kind: SigZone, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let age = z.lifetime - z.life;
        let grow = (age / 0.3).min(1.0) * (z.life / 0.3).min(1.0);
        let up = Vector3::z();
        let mut out = Vec::new();
        match kind {
            // A little clockwork bomb pattering along, bobbing, or clung on and swelling
            SigZone::Scuttler => {
                let arcane = Element::look(Some(Element::Arcane));
                let (model, bomb) = self.effect_model(SLOT_BOMB, arcane);
                let phase = z.position.x * 0.7 + z.position.y * 0.3;
                // Drawn a little bigger the further off it runs past 6 m, so it's never lost
                // against the ground
                let far = ((z.position - view.eye).norm() / vpm / 6.0).max(1.0).sqrt();
                let (at, size) = if z.dwell > 0.0 {
                    (z.position, SCUTTLER_SIZE * far * (1.0 + 0.2 * (self.time * 25.0).sin()))
                } else {
                    (z.position + up * (self.time * 30.0 + phase).sin().abs() * 0.08 * vpm, SCUTTLER_SIZE * far)
                };
                let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), z.velocity.y.atan2(z.velocity.x) + phase);
                out.push((view.eye, place(model, bomb, BOMB_RES, Vector3::zeros(), at, turn, vpm, size, MOVING)));
                // Its fuse glowing, a spark standing up off its back, flickering, to pick it out
                let flicker = 0.85 + 0.15 * (self.time * 40.0 + phase).sin();
                out.extend(self.streak_at(view.eye, at + up * 0.3 * far * vpm, up, arcane, 0.9 * far * flicker).map(|(_, i)| (view.eye, i)));
            }
            // The arrow stuck fast, its head in
            SigZone::Wick { way } => {
                let way = Vector3::new(way[0] as f64, way[1] as f64, way[2] as f64);
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                let (model, arrow) = self.effect_model(SLOT_ARROW, Element::look(Some(Element::Fire)));
                out.push((z.position, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), z.position + way * 0.1 * vpm, rotation, vpm, 1.0, MOVING)));
            }
            // A ball of light, bobbing and pulsing, a ring of light turning round it, and its glow
            // lying in a circle on the ground under it
            SigZone::Lantern => {
                let holy = Element::look(Some(Element::Holy));
                let pulse = 1.0 + 0.1 * (self.time * 6.0).sin();
                let at = z.position + up * (self.time * 2.0).sin() * 0.08 * vpm;
                let (model, ball) = self.effect_model(SLOT_FIREBALL, holy);
                out.push((view.eye, place(model, ball, MODEL_RES, Vector3::zeros(), at, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 2.0), vpm, 0.5 * pulse * grow.max(0.05), MOVING)));
                out.extend(self.circle_instances(view.eye, at, 0.75 * grow.max(0.05), holy, 0.7, self.time * 2.5).into_iter().map(|(_, i)| (view.eye, i)));
                if let Some(&ground) = z.path.first() {
                    out.extend(self.circle_instances(view.eye, ground + up * 0.15 * vpm, LANTERN_GLOW * grow.max(0.05), holy, 2.2, self.time * 0.3));
                }
                // Its rays, while they sear
                for &to in z.path.iter().skip(1) {
                    out.extend(self.line_bolts(view.eye, at, to, holy, 1.4, 2.0, 0.0).into_iter().map(|(_, i)| (view.eye, i)));
                }
            }
            // The line of frost flying out from the blade and hanging there till it shatters
            SigZone::Severance if z.dwell <= 0.0 => {
                let grown = smooth(age / SEVER_GROW).max(0.05);
                out.extend(self.line_bolts(view.eye, z.position, z.position + z.velocity * grown, Element::look(Some(Element::Frost)), 1.3, 2.0, 0.0).into_iter().map(|(_, i)| (view.eye, i)));
            }
            // Lightning running up each stake, and strung between them, jagged, flickering
            SigZone::Stakes => {
                let look = Element::look(Some(Element::Storm));
                let tops: Vec<Vector3<f64>> = z.path.iter().map(|p| p + up * vpm).collect();
                for (k, &top) in tops.iter().enumerate() {
                    out.extend(self.line_bolts(view.eye, z.path[k], top, look, 0.6, 1.0, 0.0).into_iter().map(|(_, i)| (view.eye, i)));
                }
                let pairs: &[(usize, usize)] = match tops.len() {
                    0 | 1 => &[],
                    2 => &[(0, 1)],
                    _ => &[(0, 1), (1, 2), (2, 0)],
                };
                for &(m, n) in pairs {
                    out.extend(self.line_bolts(view.eye, tops[m], tops[n], look, 1.5, 2.3, 0.3).into_iter().map(|(_, i)| (view.eye, i)));
                }
            }
            SigZone::Caltrops => {
                let fade = (z.life / 0.6).min(1.0);
                for (k, &at) in z.path.iter().enumerate() {
                    let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), k as f64 * 1.3);
                    out.extend(self.caltrop_instances(at + up * 0.04 * vpm, turn, 1.3 * fade.max(0.1)));
                }
            }
            // A churning ball of darkness, a ring round it facing the eye, turning
            SigZone::Rift => {
                let pulse = 1.0 + 0.12 * (self.time * 8.0).sin();
                let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Void)));
                let churn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 5.0) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 3.0);
                out.push((z.position, place(model, ball, MODEL_RES, Vector3::zeros(), z.position, churn, vpm, 0.3 * pulse * grow.max(0.05), MOVING)));
                let facing = view.eye - z.position;
                out.extend(self.ring_instances(view.eye, z.position, facing, 0.95 * grow.max(0.05), Element::look(Some(Element::Arcane)), 0.9, self.time * 2.5));
            }
            // The knives not loosed yet, blades of shadow hanging in their fan, pointing on ahead,
            // quivering
            SigZone::Knives => {
                let (model, knife) = self.effect_model(SLOT_BOLT, Element::look(Some(Element::Void)));
                for k in (z.owed as usize)..KNIVES {
                    let at = knife_slot(view, k, age);
                    let quiver = view.rotation * Vector3::new((self.time * 23.0 + k as f64).sin() * 0.03, 0.0, -0.06);
                    let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &(view.forward() + quiver).normalize()).unwrap_or_else(UnitQuaternion::identity);
                    // Small as it comes out of the hand, right in front of the eye, full size out
                    // in its place
                    let grown = 0.3 + 0.7 * smooth((age - 0.04 * k as f64) / 0.3);
                    out.push((view.eye, place(model, knife, BOLT_RES, Vector3::zeros(), at, rotation, vpm, 1.1 * grown, MOVING)));
                }
            }
            SigZone::Gleaning => {
                let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Arcane)));
                for &at in &z.path {
                    out.push((view.eye, place(model, ball, MODEL_RES, Vector3::zeros(), at, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 4.0), vpm, 0.15, MOVING)));
                }
            }
            // The runes written so far, turning slowly, flaring as the circle blazes and fading
            SigZone::Litany => {
                let (model, rune) = self.model_at(RUNE);
                let blaze = if age >= LITANY_BLAZE { 1.0 + 0.5 * (1.0 - (age - LITANY_BLAZE) / 0.3).max(0.0) } else { 1.0 };
                let fade = (z.life / 0.5).min(1.0);
                // The circle itself, traced out as the runes are written round it
                let traced = ((age - 0.05) / (LITANY_BLAZE - 0.3)).clamp(0.0, 1.0);
                let circle = self.circle_instances(view.eye, z.position + up * 0.25 * vpm, z.radius, Element::look(Some(Element::Holy)), 2.4 * blaze * fade.max(0.1), self.time * 0.4);
                let n = circle.len();
                out.extend(circle.into_iter().take(((n as f64 * traced).ceil() as usize).max(1)).map(|(_, i)| (view.eye, i)));
                for k in 0..runes_written(age) {
                    let at = rune_at(z.position, k, vpm);
                    let born = 0.15 + RUNE_GAP * k as f64;
                    let size = (1.4 + 0.1 * (self.time * 5.0 + k as f64).sin()) * ((age - born) / 0.15).clamp(0.2, 1.0) * blaze * fade.max(0.1);
                    out.push((view.eye, place(model, rune, RING_RES, Vector3::zeros(), at, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 0.6 + k as f64), vpm, size, MOVING)));
                }
            }
            // A ring rolling out, glowing, fading as it gets as far as it goes
            SigZone::Ring(element) => {
                let fade = (z.life / 0.25).clamp(0.0, 1.0);
                let ring = self.circle_instances(view.eye, z.position + up * 0.3 * vpm, ring_reached(age, z.radius), Element::look(Some(element)), 2.4 * fade.max(0.1), age * 0.5);
                out.extend(ring.into_iter().map(|(_, i)| (view.eye, i)));
            }
            // Threads of blood from the hex out to each it holds, pulsing along, or feeling about
            // round it if it holds nothing
            SigZone::Bloodthread { .. } => {
                let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Blood)));
                let pulse = 1.0 + 0.15 * (self.time * 7.0).sin();
                out.push((view.eye, place(model, ball, MODEL_RES, Vector3::zeros(), z.position, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 3.0), vpm, 0.3 * pulse * grow.max(0.05), MOVING)));
                let blood = Element::look(Some(Element::Blood));
                let ends: Vec<Vector3<f64>> = if z.hit.is_empty() {
                    (0..4)
                        .map(|k| {
                            let a = k as f64 / 4.0 * TAU + self.time * 0.8;
                            z.position + Vector3::new(a.cos(), a.sin(), -0.4) * 1.5 * vpm
                        })
                        .collect()
                } else {
                    // Where each it holds was this frame, as threads() saw it
                    z.path.clone()
                };
                for to in ends {
                    let n = (((to - z.position).norm() / vpm / 1.6).ceil() as usize).clamp(2, 6);
                    let flow = (self.time * 1.5).fract();
                    for j in 0..n {
                        let at = z.position.lerp(&to, ((j as f64 + flow) / n as f64).min(1.0));
                        // A little thicker the further off, so the threads still read across a field
                        let far = ((at - view.eye).norm() / vpm / 8.0).max(1.0).sqrt();
                        out.extend(self.streak_at(view.eye, at, to - z.position, blood, 1.5 * far * grow.max(0.1)).map(|(_, i)| (view.eye, i)));
                    }
                }
            }
            // Lightning crackling all round the raised head once it's been struck, as long as it's
            // held up, jumping about from one frame to the next
            SigZone::Anvil => {
                let look = Element::look(Some(Element::Storm));
                for k in 0..3 {
                    let a = self.time * 7.0 + k as f64 * TAU / 3.0;
                    let to = z.position + view.rotation * Vector3::new(a.cos() * 0.9, 0.35, 0.35 + a.sin() * 0.55) * vpm;
                    out.extend(self.line_bolts(view.eye, z.position, to, look, 0.35, 0.7, 0.12).into_iter().map(|(_, i)| (view.eye, i)));
                }
            }
            SigZone::Severance | SigZone::Knell => {}
        }
        out
    }

    /// What their zones light round them: where, how far, what color and how bright
    pub(super) fn signature_lights(&self) -> Vec<(Vector3<f64>, f64, [f32; 3], f32)> {
        let vpm = self.vpm;
        let mut out = Vec::new();
        for z in &self.zones {
            let ZoneKind::Signature(kind) = z.kind else {
                continue;
            };
            let fade = (z.life / 0.4).min(1.0) as f32;
            match kind {
                SigZone::Lantern => out.push((z.position, 9.0, Element::Holy.light(), 1.1 * fade)),
                SigZone::Rift => out.push((z.position, 6.0, Element::Void.light(), 0.9 * fade)),
                SigZone::Wick { .. } => out.push((z.position, 3.0, Element::Fire.light(), 0.7)),
                SigZone::Anvil => out.push((z.position, 6.0, Element::Storm.light(), 1.0)),
                SigZone::Stakes if !z.path.is_empty() => out.push((z.path[0] + Vector3::new(0.0, 0.0, 0.8 * vpm), 6.0, Element::Storm.light(), 0.6 * fade)),
                SigZone::Litany => {
                    let written = runes_written(z.lifetime - z.life) as f32;
                    out.push((z.position + Vector3::new(0.0, 0.0, 0.5 * vpm), LITANY_RADIUS + 2.0, Element::Holy.light(), (0.15 + 0.06 * written) * fade));
                }
                SigZone::Bloodthread { .. } => out.push((z.position, 5.0, Element::Blood.light(), 0.7 * fade)),
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Which families each is on, the families that had fewest, or shared the same set, getting
    /// signatures of their own
    const FAMILIES: [(Special, &[&str]); 16] = [
        (Special::Rimeskip, &["bombs"]),
        (Special::Scuttlers, &["bombs"]),
        (Special::Wickfire, &["bow"]),
        (Special::Lantern, &["bow"]),
        (Special::Chainshot, &["pistol", "handcannon"]),
        (Special::Caltrops, &["blunderbuss"]),
        (Special::Riftshot, &["musket"]),
        (Special::GloamingKnives, &["dagger"]),
        (Special::SeveringDraw, &["katana"]),
        (Special::ViperThrusts, &["spear"]),
        (Special::SoulGleaning, &["scythe", "halberd"]),
        (Special::Stormanvil, &["hammer", "maul", "flail"]),
        (Special::Stormstakes, &["crossbow", "arbalest", "repeater", "handcrossbow", "twinprod", "siegecrossbow", "harpoon", "boltthrower"]),
        (Special::Litany, &["tome"]),
        (Special::EmberKnell, &["bell", "censer"]),
        (Special::Bloodthread, &["totem", "orb"]),
    ];

    /// Each is named and described, on a legendary or mythic part of each family it's for, and
    /// turns up on weapons that family rolls, so it drops
    #[test]
    fn signatures_are_on_their_families_and_drop() {
        let library = crate::weapons::Library::load();
        for (special, families) in FAMILIES {
            assert!(SIGNATURES.contains(&special));
            assert!(!special.name().is_empty() && special.description().len() > 40, "{special:?}");
            for &family in families {
                assert!(library.components.iter().any(|c| c.special() == Some(special) && c.tier >= 4 && c.fits(family)), "{special:?} isn't on a legendary or mythic {family} part");
                let f = library.family(family).expect("a family");
                let rolled = (0..60_000u32).any(|seed| library.special_of(hash(seed ^ 0x51a7), Some(f), 3.0) == Some(special));
                assert!(rolled, "no {family} rolled with {special:?}");
            }
        }
        // None of the names is one of the others'
        let names: Vec<&str> = SIGNATURES.iter().map(|s| s.name()).collect();
        for (k, name) in names.iter().enumerate() {
            assert!(!names[k + 1..].contains(name), "{name} twice");
        }
    }

    /// The moves line up with what they do: the blades' parts come on their moves' quick keys, and
    /// what's shot leaves on the first
    #[test]
    fn signature_moves_line_up() {
        let check = |special: Special, kind: Kind, style: Style, times: &[f64]| {
            let (length, keys) = special_move(special, kind, style);
            let quick: Vec<f64> = keys.iter().filter(|k| k.2).map(|k| k.0 * length).collect();
            for t in times {
                assert!(quick.iter().any(|q| (q - t).abs() < 1e-6), "{special:?} with {style:?}: nothing lets go at {t:.3}, only {quick:?}");
            }
        };
        check(Special::GloamingKnives, Kind::Melee, Style::Stab, &(0..KNIVES).map(|k| KNIFE_FIRST + KNIFE_GAP * k as f64).collect::<Vec<_>>());
        check(Special::SeveringDraw, Kind::Melee, Style::Slash, &[DRAW_AT * DRAW_LENGTH]);
        check(Special::ViperThrusts, Kind::Melee, Style::Thrust, &(0..FANGS).map(|k| (VIPER_FIRST + VIPER_GAP * k as f64) * VIPER_LENGTH).collect::<Vec<_>>());
        for style in [Style::Reap, Style::Thrust] {
            check(Special::SoulGleaning, Kind::Melee, style, &[GLEAN_REAP * GLEAN_LENGTH, GLEAN_FLING * GLEAN_LENGTH]);
        }
        for style in [Style::Smash, Style::Whirl, Style::Quake] {
            check(Special::Stormanvil, Kind::Melee, style, &[ANVIL_SLAM * ANVIL_LENGTH]);
        }
        check(Special::Wickfire, Kind::Ranged, Style::Draw, &(0..3).map(|k| (WICK_FIRST + WICK_EVERY * k as f64) * WICK_LENGTH).collect::<Vec<_>>());
        let (length, keys) = special_move(Special::Stormstakes, Kind::Ranged, Style::Crank);
        let first = first_release(length, &keys);
        check(Special::Stormstakes, Kind::Ranged, Style::Crank, &(0..STAKES).map(|k| first + STAKE_GAP * k as f64).collect::<Vec<_>>());
        for (special, style, at) in [(Special::Litany, Style::Summon, LITANY_RELEASE), (Special::EmberKnell, Style::Toll, KNELL_RELEASE), (Special::EmberKnell, Style::Censer, KNELL_RELEASE), (Special::Bloodthread, Style::Hex, THREAD_RELEASE), (Special::Bloodthread, Style::Channel, THREAD_RELEASE)] {
            let (length, keys) = special_move(special, Kind::Magic, style);
            assert!((first_release(length, &keys) - at * length).abs() < 1e-9, "{special:?} with {style:?}");
        }
    }

    /// Where each of a family's signature shots goes, to look at:
    /// cargo test --release signature_paths -- --ignored --nocapture
    #[test]
    #[ignore]
    fn signature_paths() {
        let library = crate::weapons::Library::load();
        let size = [384u32, 384, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 8;
        let vpm = 8.0;
        let only = std::env::var("SIG").unwrap_or_default();
        for (special, families) in FAMILIES {
            if !only.is_empty() && !format!("{special:?}").eq_ignore_ascii_case(&only) {
                continue;
            }
            let family = library.family(families[0]).unwrap();
            let Some(seed) = (0..200_000u32).find(|&s| library.special_of(hash(s ^ 0x51a7), Some(family), 8.0) == Some(special)) else {
                continue;
            };
            let weapon = library.generate(hash(seed ^ 0x51a7), Some(family), 8.0);
            let water = |_: f64, _: f64| f64::NEG_INFINITY;
            let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
            let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.12), tan: [0.75, 0.42], vpm, speed: 0.0, stride: 0.0 };
            // With PACK=1, a pack of bandits 10 m ahead
            let mut animals = if std::env::var("PACK").is_ok() { crate::animals::spawn_pack(&world, &water, &[Look::Bandit; 4], feet + Vector3::new(0.0, 10.0 * vpm, 0.0), crate::animals::PACK_SPREAD, 3) } else { Vec::new() };
            let mut combat = Combat::new(1, 0);
            combat.charge = 1.0;
            let dt = 1.0 / 60.0;
            println!("== {special:?} with a {}", families[0]);
            for frame in 0..200 {
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, Some((0, &weapon)), None, Controls { special: frame == 20, ..Default::default() }, dt);
                if frame % 6 == 0 && frame >= 20 {
                    let shots: Vec<String> = combat
                        .projectiles
                        .iter()
                        .filter_map(|p| if let Trick::Signature(s) = p.trick { Some((s, p)) } else { None })
                        .map(|(s, p)| format!("{s:?} ({:.1}, {:.1}, {:.1})", (p.position.x - feet.x) / vpm, (p.position.y - feet.y) / vpm, (p.position.z - feet.z) / vpm))
                        .collect();
                    let zones: Vec<String> = combat.zones.iter().map(|z| format!("{:?} ({:.1}, {:.1}, {:.1}) {} points", z.kind, (z.position.x - feet.x) / vpm, (z.position.y - feet.y) / vpm, (z.position.z - feet.z) / vpm, z.path.len())).collect();
                    let drawn = combat.zones.iter().filter_map(|z| if let ZoneKind::Signature(k) = z.kind { Some(combat.signature_zone_instances(z, k, &view).len()) } else { None }).sum::<usize>();
                    println!("{:.2} s: shots {shots:?} zones {zones:?} swells {} drawn {drawn} of {} sparks {}", (frame - 20) as f64 * dt, combat.swells.len(), combat.flying_instances(&view).len(), combat.sparks.len());
                    // With SPHERES=1, where what's drawn for their zones is, off the eye in meters
                    if std::env::var("SPHERES").is_ok() {
                        for z in &combat.zones {
                            if let ZoneKind::Signature(k) = z.kind {
                                let at: Vec<String> = combat.signature_zone_instances(z, k, &view).iter().map(|(_, i)| i.sphere()).map(|(c, r)| format!("({:.1}, {:.1}, {:.1}) r {:.2}", (c.x - view.eye.x) / vpm, (c.y - view.eye.y) / vpm, (c.z - view.eye.z) / vpm, r / vpm)).collect();
                                println!("    {k:?}: {}", at.join(" "));
                            }
                        }
                    }
                }
            }
        }
    }

    /// A litany circle writes every rune before it blazes, and a chainshot's balls spin out to
    /// their full spread soon after they leave the gun
    #[test]
    fn litany_fills_before_it_blazes() {
        assert_eq!(runes_written(LITANY_BLAZE - 0.01), RUNES);
        assert_eq!(runes_written(0.1), 0);
        assert!(chain_half(0.4) > CHAIN_HALF * 0.99 && chain_half(0.0) < 0.2);
        // The triangle a stake's lightning closes round
        let (a, b, c) = (Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0), Vector3::new(0.0, 10.0, 0.0));
        assert!(inside_triangle(Vector3::new(2.0, 2.0, 5.0), a, b, c));
        assert!(!inside_triangle(Vector3::new(8.0, 8.0, 0.0), a, b, c));
    }
}
