//! Bosses: a few huge creatures guarding ruins, each fighting its own way. As far as everything
//! else goes they're animals, hit by weapons the same way. This is what they think and do: what
//! they do to the player is left to src\game.rs, as Deeds.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use nalgebra::{Vector2, Vector3};

use crate::animals::{self, Animal, Build, Look, Mood, POSE_COIL, POSE_RAISE, POSE_SPECIAL, POSE_SPIKES, POSE_STRIKE, POSE_SWEEP, POSE_WINDUP};
use crate::weapons::Element;
use crate::worldgen::{World, is_passable};

/// How near the player has to come before a boss turns on them, and how far off they can get
/// before it gives up, in meters
const NOTICE: f64 = 24.0;
const GIVE_UP: f64 = 50.0;
/// The furthest from home it'll follow them
const LEASH: f64 = 60.0;
/// Seconds a boss or monster leaves the player be after beating them, so they can get up and
/// get away
const SPARE_SECONDS: f64 = 10.0;
/// The same for the walking dead
const MONSTER_NOTICE: f64 = 16.0;
const MONSTER_GIVE_UP: f64 = 28.0;
const MONSTER_LEASH: f64 = 35.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Move {
    /// Fists or paws brought down on the ground in front, hurting everything near
    Slam,
    /// A boulder hurled at the player
    Throw,
    /// Fire breathed out in a stream
    Breath,
    /// A fireball spat
    Spit,
    /// Charging headlong at the player
    Charge,
    /// A roar, on first seeing the player and again on getting angry
    Roar,
    /// Leaping through the air to come down on the player
    Leap,
    /// Roots bursting up from under the player, a moment after it calls them
    Roots,
    /// A volley of thorns
    Volley,
    /// The ground pounded so a shockwave runs out across it, to be jumped
    Quake,
    /// Its element called down out of the sky round the player
    Storm,
    /// Three lunging swipes, the last the hardest
    Maul,
    /// A ball of web spat, leaving the ground sticky
    Web,
    /// A line of roots bursting up one after another, dragging the player in
    RootWave,
    /// Calling up monsters of its kind to help it: a howl, the dead rising, spiderlings
    Summon,
    /// A raid boss bringing the sky down: its element falling all over, all round the player
    Cataclysm,
    /// A crossbow bolt loosed at the player, from well back
    Shoot,
    /// Digging in under the sand, to come at the player unseen
    Burrow,
    /// Bursting up out of the sand right under the player
    Erupt,
    /// Vanishing in a swirl of frost and coming back a few meters off to one side
    Blink,
    /// A tongue lashed out at the player, dragging them in to be bitten
    Lash,
    /// A knight's greatsword swept round low in a full circle, to be jumped: twice over, angry
    Sweep,
    /// A knight's greatsword driven into the ground, bone spikes bursting up all round it
    Spikes,
}

const MOVES: usize = 23;

/// Where the rest of a monster's pack is, so it keeps with them: gathered up each frame by
/// src\game.rs, see Game::gather_packs()
#[derive(Clone, Copy, Default)]
pub struct Flock {
    /// The middle of the pack, in voxels, and how many of it are left, this one too
    pub middle: Vector3<f64>,
    pub size: u8,
    /// The middle of those of the pack that close in to fight, leaving out those that hang back
    /// to shoot, see hangs_back(): the whole pack's middle if they all do. And how many close in.
    pub front: Vector3<f64>,
    pub fighters: u8,
    /// Its place among those closing in, 0 to fighters - 1, for which side of the player it comes
    /// round to
    pub slot: u8,
    /// One of the pack is after the player and can see them
    pub alarmed: bool,
    /// Which way it's crowded off by the rest of the pack, and how hard, 0 to 1 or so for each of
    /// them right on top of it: so they keep close but don't stand in each other
    pub crowd: Vector2<f64>,
}

/// How near the rest of its pack a monster lets them come, in meters, before it's crowded off
pub const ELBOW_ROOM: f64 = 1.1;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Wandering round home
    Roam,
    /// Going after the player
    Chase,
    /// Getting ready, so the player can see it coming: with how long it's been at it
    WindUp(Move, f64),
    /// Doing it
    Act(Move, f64),
    /// Getting its breath back
    Recover(f64),
    /// Dazed from running into something
    Stunned(f64),
}

/// What a boss did this frame, for src\game.rs to see to. Positions are in voxels.
#[derive(Clone, Copy)]
pub enum Deed {
    /// Getting ready to make a move, here
    WindUp(Move, Vector3<f64>),
    /// Roaring on first seeing the player, or getting `angry`
    Roar { at: Vector3<f64>, angry: bool },
    /// A blow landing on the ground at `at`: hurting what's within `radius` meters, and blowing a
    /// hole `crater` meters round
    Slam { at: Vector3<f64>, radius: f64, damage: f64, crater: f64 },
    Throw { from: Vector3<f64>, to: Vector3<f64> },
    Spit { from: Vector3<f64>, to: Vector3<f64> },
    /// Breathing fire for `dt` seconds
    Breath { from: Vector3<f64>, direction: Vector3<f64>, dt: f64 },
    /// Charging into the player, who's thrown along `direction`
    Ram { damage: f64, direction: Vector3<f64> },
    /// A heavy footfall
    Stomp(Vector3<f64>),
    /// Roots called up from under `at`: they burst up a moment later
    Roots { at: Vector3<f64> },
    /// Thorns shot from `from` at `to`
    Volley { from: Vector3<f64>, to: Vector3<f64> },
    /// Showing on the ground where it's coming down, `radius` meters round, `closing` from 1 to 0
    /// as it comes
    Mark { at: Vector3<f64>, radius: f64, closing: f64 },
    /// A shockwave running out from `at`, its front gone from `from` to `to` meters out this frame
    /// of `dt` seconds
    Quake { at: Vector3<f64>, from: f64, to: f64, dt: f64 },
    /// The `k`th of its element coming down out of the sky on the player at `to`
    Storm { to: Vector3<f64>, k: usize },
    /// Web spat from `from` at `to`
    Web { from: Vector3<f64>, to: Vector3<f64> },
    /// A line of roots called up from `from` to `to`, `lanes` of them fanned out
    RootWave { from: Vector3<f64>, to: Vector3<f64>, lanes: usize },
    /// The `k`th swipe of a maul starting, here
    Swipe(Vector3<f64>, usize),
    /// `count` monsters of `look` called up round `at`, to fight in `pack`, see Flock
    Summon { at: Vector3<f64>, look: Look, count: usize, pack: u32 },
    /// A crossbow bolt loosed from `from` at `to`
    Bolt { from: Vector3<f64>, to: Vector3<f64> },
    /// Sand churned up over something tunnelling along under it at `at`
    Burrow { at: Vector3<f64> },
    /// Something bursting up out of the sand at `at`
    Erupt { at: Vector3<f64> },
    /// Vanishing at `from` in a swirl of frost, and coming back at `to`
    Blink { from: Vector3<f64>, to: Vector3<f64> },
    /// A tongue lashed out from `from` at `to`, dragging back whoever it catches
    Lash { from: Vector3<f64>, to: Vector3<f64> },
    /// The `k`th of a raid boss's element coming down out of the sky somewhere round the player at
    /// `to`
    Cataclysm { to: Vector3<f64>, k: usize },
    /// A raid boss going into its `stage`th phase, 1 or 2, here
    Phase { at: Vector3<f64>, stage: u8 },
}

pub struct Boss {
    look: Look,
    /// Where its lair is in the whole landscape, to remember it's been beaten
    pub key: [i64; 3],
    /// Where it guards, in voxels
    pub home: Vector3<f64>,
    phase: Phase,
    /// Fighting the player
    pub engaged: bool,
    /// Hurt badly enough to attack faster
    pub enraged: bool,
    /// Seconds until each move can be made again
    cooldowns: [f64; MOVES],
    /// Where its move's aimed
    aim: Vector3<f64>,
    /// Whether its charge has hit already
    rammed: bool,
    /// Seconds it leaves the player be after beating them, unless they come at it first
    grace: f64,
    /// Where a leap started and where it's coming down
    leap: (Vector3<f64>, Vector3<f64>),
    /// How far through its walk it was at the last footfall
    last_step: f64,
    /// The pose it's in and the one it was in before, see animals::POSE_WINDUP, None to walk or
    /// stand, and seconds since it changed, to ease from one to the other
    posed: (Option<usize>, Option<usize>, f64),
    /// For a raid boss, which phase of the fight it's in, 0 to 2, see RAID_PHASES
    pub stage: u8,
    /// Blows it has left to follow up the one it's making with, one after another
    combo: u8,
    /// Seconds it's holding the blow it's winding up back, longer than it usually would
    hold: f64,
    /// How its whole body is moving on top of its pose, eased towards where its move puts it:
    /// radians tipped forward, and meters moved on and up, see Boss::sway()
    sway: [f64; 3],
    /// Seconds since it last saw the player, while fighting them
    unseen: f64,
    /// Whether it saw the player this frame, while fighting them, to alarm the rest of its pack
    sighted: bool,
    /// Which pack it goes about with, 0 for none, and where the rest of them are
    pub pack: u32,
    pub flock: Flock,
    /// A burrower dug in under the sand
    buried: bool,
    /// A lurker lying in wait, sunk low, for something to come close
    lurking: bool,
    /// Seconds it goes on backing off from the player to shoot from further back, and seconds
    /// until it'll do it again
    retreat: f64,
    retreat_wait: f64,
    /// Hit since it last thought, for a wraith to vanish from under the blow
    struck: bool,
    /// Meters it's sunk into the ground or dived down from where it flies, eased towards where its
    /// move puts it
    sink: f64,
    /// Seconds it's been about, for what rises and falls as it flies or drifts
    clock: f64,
    /// Seconds until it next churns up the sand over it, tunnelling
    trail: f64,
    /// The last move it made, for how it gets over it: a crossbowman spans its crossbow again
    last: Option<Move>,
    /// Which way a knight faced as it started sweeping its greatsword round
    spun_from: f64,
    /// For looking at them: the frame it's held in, see animals::POSE_WINDUP, whatever it's doing
    pub fixed_pose: Option<usize>,
    pub deeds: Vec<Deed>,
}

/// Seconds a boss goes on after the player once it can't see them, and a monster
const LOST: f64 = 8.0;
const MONSTER_LOST: f64 = 6.0;

/// Whether `a` can see the player standing at `feet`, in voxels: nothing solid in the way from the
/// middle of its body to their eyes or their middle, looking a quarter of a meter at a time
fn sees(a: &Animal, feet: Vector3<f64>, world: &World) -> bool {
    let vpm = world.voxels_per_meter as f64;
    // From where its middle is standing, not sunk under the sand or the water
    let from = a.position + Vector3::new(0.0, 0.0, a.look.body().0 * vpm);
    [1.6, 0.9].into_iter().any(|up| {
        let to = feet + Vector3::new(0.0, 0.0, up * vpm);
        let steps = ((to - from).norm() / (0.25 * vpm)).ceil() as usize;
        (1..steps).all(|s| {
            let p = from + (to - from) * (s as f64 / steps as f64);
            is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
        })
    })
}

/// How long each move takes to get ready for, to do, and to get over, in seconds, and then how
/// long until it can be made again
fn windup(look: Look, m: Move) -> f64 {
    // Monsters are quick about it, but not so quick it can't be seen coming
    if look.is_monster() {
        return match (look, m) {
            (_, Move::Roar) => 0.0,
            (Look::Wolf, _) => 0.5,
            (Look::Spiderling, Move::Leap) => 0.45,
            (Look::Spiderling, _) => 0.4,
            (Look::Zombie, _) => 0.75,
            (Look::Skeleton, _) => 0.8,
            (Look::Bandit, Move::Leap) => 0.5,
            (Look::Bandit, _) => 0.65,
            (Look::BanditBrute, _) => 1.0,
            // Taking aim long enough to be seen and stepped out of the way of
            (Look::Crossbowman, Move::Shoot) => 0.95,
            (Look::Crossbowman, _) => 0.6,
            (Look::Shieldbearer, _) => 0.9,
            (Look::DuneBurrower, Move::Erupt) => 0.6,
            (Look::DuneBurrower, _) => 0.55,
            (Look::RimeWraith, Move::Blink) => 0.2,
            (Look::RimeWraith, Move::Spit) => 0.8,
            (Look::RimeWraith, _) => 0.6,
            (Look::MireLurker, Move::Lash) => 0.7,
            (Look::MireLurker, _) => 0.55,
            (Look::Gloomwing, _) => 0.35,
            (Look::Tusker, Move::Charge) => 0.9,
            (Look::Tusker, _) => 0.65,
            // Swelling up fit to burst, long enough to get away from
            (Look::Bloater, _) => 1.1,
            (Look::Ghoul, Move::Leap) => 0.5,
            (Look::Ghoul, _) => 0.45,
            // The gravestone hauled up high, slowly: the moment to get out from under it
            (Look::GraveBrute, _) => 1.25,
            (Look::BoneArcher, Move::Shoot) => 1.0,
            (Look::BoneArcher, _) => 0.6,
            (Look::Necromancer, Move::Summon) => 1.3,
            (Look::Necromancer, Move::Spit) => 0.85,
            (Look::Necromancer, _) => 0.7,
            _ => 0.85,
        };
    }
    // A knight crouching to sweep, raising its sword to drive it in, gathering itself to leap,
    // and drawing back to fling the void off its blade
    if look.is_knight() {
        match m {
            Move::Sweep => return 0.95,
            Move::Spikes => return 1.1,
            Move::Leap => return 0.8,
            Move::Spit => return 0.7,
            _ => {}
        }
    }
    match (look.build(), m) {
        (_, Move::Roar) => 0.0,
        (_, Move::Summon) => 1.2,
        (_, Move::Sweep | Move::Spikes) => 1.0,
        (Some(Build::Wolf), Move::Slam) => 0.55,
        (Some(Build::Drake), Move::Slam) => 0.6,
        (Some(Build::Bear), Move::Slam) => 0.8,
        (Some(Build::Spider), Move::Slam) => 0.7,
        (_, Move::Slam | Move::Throw | Move::Charge) => 0.9,
        (_, Move::Breath) => 0.7,
        (_, Move::Spit) => 0.55,
        (_, Move::Leap) => 0.6,
        (_, Move::Roots) => 0.8,
        (_, Move::Volley) => 0.7,
        (_, Move::Quake) => 1.1,
        (_, Move::Storm | Move::RootWave) => 1.0,
        (_, Move::Maul) => 0.7,
        (_, Move::Web) => 0.6,
        (_, Move::Cataclysm) => 1.8,
        (_, Move::Shoot | Move::Lash) => 0.8,
        (_, Move::Burrow | Move::Erupt) => 0.6,
        (_, Move::Blink) => 0.3,
    }
}

fn duration(look: Look, m: Move) -> f64 {
    match m {
        // Once round, see Boss::spins() for how many times
        Move::Sweep => SPIN + SPIN_SETTLE,
        Move::Spikes => 0.6,
        Move::Shoot => 0.25,
        Move::Burrow => 0.6,
        Move::Erupt => 0.35,
        Move::Blink => 0.2,
        Move::Lash => 0.45,
        Move::Cataclysm => CATACLYSM_START + CATACLYSM_EVERY * CATACLYSM_FALLS as f64,
        Move::Slam if look.is_monster() => 0.35,
        Move::Slam => 0.55,
        Move::Throw => 0.45,
        Move::Breath => 1.8,
        Move::Spit => 0.4,
        Move::Charge => 2.4,
        Move::Roar => 1.4,
        Move::Leap => leap(look).0,
        Move::Summon => 1.0,
        Move::Roots => 0.6,
        Move::Volley => 0.5,
        Move::Quake => quake_reach(look) / QUAKE_PACE,
        Move::Storm => 1.5,
        Move::Maul => MAUL_SWIPE * 3.0,
        Move::Web => 0.4,
        Move::RootWave => 0.6,
    }
}

fn recovery(look: Look, m: Move) -> f64 {
    match m {
        // Unsteady after spinning, and working its sword back out of the ground: the moments to
        // get in close
        Move::Sweep => 1.0,
        Move::Spikes => 1.1,
        Move::Leap if look.is_knight() => 0.9,
        // Spanning the crossbow again
        Move::Shoot => 0.7,
        Move::Burrow => 0.2,
        // Out in the open and slow to get its bearings: the moment to hit it
        Move::Erupt => 1.0,
        Move::Blink => 0.3,
        Move::Lash => 0.6,
        Move::Slam if look == Look::Shieldbearer => 1.1,
        Move::Slam if look == Look::Gloomwing => 0.45,
        Move::Slam if matches!(look, Look::Skeleton | Look::ArmoredSkeleton) => 1.0,
        Move::Slam if matches!(look, Look::Zombie | Look::ArmoredZombie) => 0.9,
        Move::Slam if look == Look::GraveBrute => 1.4,
        Move::Slam if look == Look::Bandit => 0.75,
        Move::Slam if look == Look::BanditBrute => 1.1,
        Move::Slam | Move::Leap if look.is_monster() => 0.5,
        Move::Summon => 0.8,
        // A long stagger after bringing the sky down: the moment to get in close
        Move::Cataclysm => 2.2,
        // Heavy blows leave it open a while, the bigger it is the longer
        Move::Slam => 0.9 * look.size().sqrt(),
        Move::Throw => 0.7,
        Move::Breath => 1.0,
        Move::Spit => 0.5,
        Move::Charge => 0.8,
        Move::Roar => 0.2,
        Move::Leap => 0.7,
        Move::Roots => 0.6,
        Move::Volley => 0.6,
        Move::Quake | Move::Storm => 1.0,
        Move::Maul => 1.2,
        Move::Web => 0.6,
        Move::RootWave => 0.8,
    }
}

fn cooldown(look: Look, m: Move) -> f64 {
    match m {
        Move::Sweep => 7.5,
        Move::Spikes => 11.0,
        Move::Leap if look.is_knight() => 8.0,
        Move::Spit if look.is_knight() => 3.6,
        Move::Shoot => 3.2,
        Move::Burrow => 6.0,
        Move::Erupt => 2.0,
        Move::Blink => 4.5,
        Move::Lash => 6.0,
        Move::Charge if look == Look::Tusker => 6.5,
        Move::Slam if matches!(look, Look::Wolf | Look::MireLurker | Look::Ghoul) => 1.6,
        Move::Slam if look == Look::GraveBrute => 3.0,
        Move::Leap if look == Look::Ghoul => 4.5,
        Move::Summon if look == Look::Necromancer => 15.0,
        Move::Slam if matches!(look, Look::Spiderling | Look::Gloomwing) => 1.4,
        Move::Slam if look.is_monster() => 2.4,
        Move::Leap if look == Look::Wolf => 6.0,
        Move::Leap if look.is_monster() => 5.0,
        Move::Cataclysm => 16.0,
        Move::Summon if look.is_raid() => 18.0,
        Move::Summon if look.build() == Some(Build::Spider) => 14.0,
        Move::Summon if look.build() == Some(Build::Wolf) => 20.0,
        Move::Summon => 24.0,
        Move::Slam => 2.6,
        Move::Throw => 4.5,
        Move::Breath => 6.5,
        Move::Spit => 3.0,
        Move::Charge => 5.5,
        Move::Roar => 0.0,
        Move::Leap => 5.0,
        Move::Roots => 4.0,
        Move::Volley => 4.5,
        Move::Quake | Move::RootWave => 8.0,
        Move::Storm => 9.0,
        Move::Maul => 7.0,
        Move::Web => 10.0,
    }
}

/// A leap: seconds in the air, how high it goes and how far round it hurts where it comes down in
/// meters, and how hard
fn leap(look: Look) -> (f64, f64, f64, f64) {
    match look {
        Look::Wolf => (0.6, 1.4, 1.8, 12.0),
        Look::Spiderling => (0.5, 1.0, 1.2, 8.0),
        Look::Bandit => (0.55, 1.1, 1.4, 14.0),
        // A ghoul pouncing, claws first
        Look::Ghoul => (0.55, 1.3, 1.4, 13.0),
        // A gloomwing swooping down
        Look::Gloomwing => (0.45, 0.5, 1.1, 7.0),
        Look::Direwolf | Look::WinterAlpha => (0.7, 3.0, 3.0, 24.0),
        // The Grave Knight's leap, its greatsword raised, to cleave down where the player stood
        Look::GraveKnight => (0.85, 3.4, 3.2, 30.0),
        _ => (0.8, 4.0, 3.0, 26.0),
    }
}

/// How near it comes before stopping to strike, in meters, and how far short of the player a
/// leap of its comes down
fn closeness(look: Look) -> (f64, f64) {
    match look {
        Look::Wolf => (1.3, 0.8),
        Look::Skeleton | Look::ArmoredSkeleton | Look::Bandit | Look::BanditBrute | Look::Shieldbearer | Look::Crossbowman => (1.6, 0.5),
        Look::Zombie | Look::ArmoredZombie => (1.4, 0.5),
        Look::Bloater | Look::Ghoul => (1.2, 0.6),
        Look::GraveBrute => (1.9, 0.6),
        Look::BoneArcher | Look::Necromancer => (1.6, 0.5),
        Look::Spiderling => (0.9, 0.5),
        Look::DuneBurrower => (1.3, 0.5),
        Look::RimeWraith => (1.6, 0.5),
        Look::MireLurker => (1.5, 0.6),
        Look::Gloomwing => (0.8, 0.6),
        Look::Tusker => (1.9, 0.8),
        _ if look.build() == Some(Build::Drake) => (6.0, look.body().1 + 1.0),
        _ => (3.0, look.body().1 + 1.0),
    }
}

/// A blow at what's in front: how far ahead it lands and how far round it reaches in meters, how
/// hard it hits, and how big a hole it blows
fn slam(look: Look) -> (f64, f64, f64, f64) {
    match look {
        Look::Wolf => (1.0, 1.3, 8.0, 0.0),
        Look::Spiderling => (0.6, 1.0, 5.0, 0.0),
        Look::Skeleton => (1.4, 1.6, 20.0, 0.0),
        Look::ArmoredSkeleton => (1.4, 1.7, 23.0, 0.0),
        Look::Zombie => (1.1, 1.5, 14.0, 0.0),
        Look::ArmoredZombie => (1.1, 1.6, 17.0, 0.0),
        Look::Bandit => (1.4, 1.5, 15.0, 0.0),
        Look::BanditBrute => (1.5, 1.8, 26.0, 0.0),
        // A crossbowman's stock, a mace over the top of a shield, pincers and a sting, icy claws,
        // a bite, a gloomwing's teeth and a tusker's goring
        Look::Crossbowman => (1.2, 1.3, 9.0, 0.0),
        Look::Shieldbearer => (1.4, 1.6, 18.0, 0.0),
        Look::DuneBurrower => (1.2, 1.5, 12.0, 0.0),
        Look::RimeWraith => (1.2, 1.4, 12.0, 0.0),
        Look::MireLurker => (1.3, 1.6, 15.0, 0.0),
        Look::Gloomwing => (0.5, 1.0, 5.0, 0.0),
        Look::Tusker => (1.6, 1.8, 22.0, 0.0),
        // A bloater bursting where it stands, a ghoul's claws, a brute's gravestone, an archer's
        // bow swung like a club and a necromancer's staff
        Look::Bloater => (0.0, 2.4, 14.0, 0.0),
        Look::Ghoul => (1.1, 1.4, 10.0, 0.0),
        Look::GraveBrute => (1.9, 2.1, 25.0, 0.0),
        Look::BoneArcher => (1.1, 1.2, 8.0, 0.0),
        Look::Necromancer => (1.2, 1.3, 9.0, 0.0),
        Look::GraveKnight => (2.8, 3.6, 32.0, 1.0),
        Look::Warlord => (2.2, 2.9, 30.0, 0.5),
        // A raid boss's blows reach as far further as it's bigger, and hit harder still
        _ if look.is_raid() => {
            let base = if look == Look::CryptTitan { Look::GraveKnight } else if look.build() == Some(Build::Golem) { Look::Golem } else { Look::Drake };
            let (reach, radius, damage, crater) = slam(base);
            let s = look.size();
            (reach * s, radius * s, damage * 1.3, crater * s)
        }
        _ => match look.build() {
            Some(Build::Golem) => (2.0, 3.4, 32.0, 1.1),
            Some(Build::Bear) => (2.2, 2.8, 26.0, 0.7),
            Some(Build::Spider) => (1.8, 2.4, 20.0, 0.3),
            Some(Build::Treant) => (2.4, 3.6, 30.0, 0.8),
            Some(Build::Wolf) => (2.2, 3.0, 24.0, 0.4),
            _ => (2.0, 2.6, 22.0, 0.5),
        },
    }
}

/// How far its quake's shockwave runs out, in meters
fn quake_reach(look: Look) -> f64 {
    QUAKE_REACH * look.size().sqrt()
}

/// What a raid boss can do in each phase, from 0 to 2: more and worse as it goes, see Boss::stage
fn unlocked(look: Look, m: Move, stage: u8) -> bool {
    !look.is_raid()
        || match m {
            Move::Cataclysm | Move::Summon | Move::Spikes => stage >= 1,
            Move::Storm | Move::Throw => stage >= 2 || look != Look::Colossus,
            _ => true,
        }
}
/// Radians a second it can turn while breathing, slow enough to run out of the way of
const BREATH_TURN: f64 = 0.7;
/// How far a quake's shockwave runs out in meters, and how fast
const QUAKE_REACH: f64 = 14.0;
const QUAKE_PACE: f64 = 10.0;
/// Seconds each swipe of a maul takes, the first part of it lunging
const MAUL_SWIPE: f64 = 0.65;
const MAUL_LUNGE: f64 = 0.4;
/// Seconds between things coming down in a storm, after it starts
const STORM_EVERY: f64 = 0.22;
const STORM_START: f64 = 0.3;
/// The same for a raid boss's cataclysm, and how many come down
const CATACLYSM_EVERY: f64 = 0.14;
const CATACLYSM_START: f64 = 0.4;
const CATACLYSM_FALLS: usize = 18;
/// How far through a raid boss's health it goes into its second and third phases
const RAID_PHASES: [f64; 2] = [0.66, 0.33];
/// Seconds at most a heavy blow's held back, wound up, so it can't be timed by rhythm alone
const MOST_HOLD: f64 = 0.55;
/// How far round a burrower bursting up out of the sand hurts, in meters
const ERUPT_RADIUS: f64 = 1.5;
/// How far a wraith goes when it vanishes, in meters
const BLINK: f64 = 6.0;
/// Seconds a knight takes to sweep its greatsword once round, and to stop after
const SPIN: f64 = 0.55;
const SPIN_SETTLE: f64 = 0.12;
/// Where a knight's blade points, radians round to the left of where it faces, drawn back to sweep
/// and swept out, see animals::POSE_COIL and POSE_SWEEP, and seconds it takes to swing from one
/// to the other in front of it before its whole body turns with it
const COILED: f64 = -2.7;
const SWEPT: f64 = 1.55;
const SWING: f64 = 0.07;
/// How much further than its blade reaches a sweep or the spikes are shown on the ground, so
/// standing just outside the ring is safe, in meters
const RING_SPARE: f64 = 0.4;
/// How hard a knight's bone spikes hurt
const SPIKES_DAMAGE: f64 = 28.0;
/// How far apart the void a knight flings off its blade lands, either side of the player once
/// it's angry, in meters
const SLASH_SPREAD: f64 = 3.4;

impl Boss {
    pub fn new(look: Look, home: Vector3<f64>, key: [i64; 3]) -> Self {
        Boss {
            look,
            key,
            home,
            phase: Phase::Roam,
            engaged: false,
            enraged: false,
            cooldowns: [0.0; MOVES],
            aim: home,
            rammed: false,
            grace: 0.0,
            leap: (home, home),
            last_step: 0.0,
            posed: (None, None, f64::MAX),
            stage: 0,
            combo: 0,
            hold: 0.0,
            sway: [0.0; 3],
            unseen: 0.0,
            sighted: false,
            pack: 0,
            flock: Flock::default(),
            // Burrowers wait dug in, lurkers sunk low in the shallows
            buried: look == Look::DuneBurrower,
            lurking: look == Look::MireLurker,
            retreat: 0.0,
            retreat_wait: 0.0,
            struck: false,
            sink: 0.0,
            clock: 0.0,
            trail: 0.0,
            last: None,
            spun_from: 0.0,
            fixed_pose: None,
            deeds: Vec::new(),
        }
    }

    /// Hit by the player: it turns on them
    pub fn provoke(&mut self) {
        self.engaged = true;
        self.grace = 0.0;
        self.unseen = 0.0;
        self.struck = true;
        self.lurking = false;
    }

    /// Whether it's after the player and saw them this frame
    pub fn sighted(&self) -> bool {
        self.engaged && self.sighted
    }

    /// Dug in under the sand
    pub fn buried(&self) -> bool {
        self.buried
    }

    /// Holding up its guard, for a shieldbearer: whenever it isn't attacking or getting over it
    pub fn guarding(&self) -> bool {
        matches!(self.phase, Phase::Roam | Phase::Chase)
    }

    /// How much faster than it walks or runs it's going: a burrower tunnelling along, a tusker
    /// charging
    pub fn haste(&self) -> f64 {
        match self.phase {
            Phase::Act(Move::Charge, _) if self.look == Look::Tusker => 1.7,
            _ if self.buried => 1.8,
            _ => 1.0,
        }
    }

    /// For looking at it: leaves the player be for good, so it stays where it is
    pub fn leave_be(&mut self) {
        self.grace = f64::MAX;
    }

    /// Having beaten the player, it goes home and leaves them be for a while to get up
    pub fn spare(&mut self) {
        self.calm();
        self.grace = SPARE_SECONDS;
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: Vector3<f64>) {
        self.home -= shift;
        self.aim -= shift;
        self.leap.0 -= shift;
        self.leap.1 -= shift;
    }

    /// Gives up the fight, as when the player's beaten, and goes home. Partway through a leap, it
    /// comes down first.
    pub fn calm(&mut self) {
        self.engaged = false;
        self.enraged = false;
        self.retreat = 0.0;
        if !matches!(self.phase, Phase::Act(Move::Leap, _)) {
            self.phase = Phase::Roam;
        }
    }

    /// Which frame to draw it in, see animals::POSE_WINDUP, or None to walk or stand: easing through
    /// the frames between its last pose and a new one, see animals::tween()
    pub fn pose(&self) -> Option<usize> {
        if self.fixed_pose.is_some() {
            return self.fixed_pose;
        }
        let (now, before, since) = self.posed;
        animals::tween(self.look, before.unwrap_or(0), now.unwrap_or(0), since).or(now)
    }

    /// How many times round a knight sweeps its greatsword: twice, once it's angry
    fn spins(&self) -> f64 {
        if self.enraged { 2.0 } else { 1.0 }
    }

    /// How its whole body moves on top of its pose: radians it's tipped forward, and meters it's
    /// moved on and up, for drawing. It leans back into a heavy blow as it winds up, trembling the
    /// longer it holds it back, lunges into it, and settles slowly after, so the blow has weight.
    pub fn sway(&self) -> [f64; 3] {
        let [pitch, on, up] = self.sway;
        let tremble = match self.phase {
            Phase::WindUp(m, t) if t > windup(self.look, m) => 0.012 * (t * 38.0).sin(),
            Phase::Stunned(t) => 0.03 * (t * 6.0).sin(),
            _ => 0.0,
        };
        // Fliers bob up and down as they beat their wings, a wraith rises and falls as it drifts,
        // and either way sinks as it dives or digs in
        let hover = match self.look.build() {
            Some(Build::Bat) => 0.07 * (self.clock * 6.8).sin() + 0.12 * (self.clock * 0.9 + self.key[0] as f64).sin(),
            Some(Build::Wraith) => 0.06 * (self.clock * 1.6).sin(),
            _ => 0.0,
        };
        [pitch + tremble, on, up + hover - self.sink]
    }

    /// How far it's sunk into the ground or dived down from where it flies, in meters: dug right
    /// in under the sand but for the tip of its tail, lying low in wait with only its eyes up, or
    /// down to bite
    fn sink_target(&self) -> (f64, f64) {
        let diving = matches!(self.phase, Phase::WindUp(Move::Slam | Move::Leap, _) | Phase::Act(Move::Slam | Move::Leap, _));
        match self.look {
            // Held in a pose to be looked at, all of it out in the open
            _ if self.fixed_pose.is_some() => (0.0, 6.0),
            Look::DuneBurrower if self.buried => (1.1, 2.5),
            Look::MireLurker if self.lurking => (0.3, 2.0),
            Look::Gloomwing if diving => (0.85, 7.0),
            Look::Gloomwing => (0.0, 3.0),
            _ => (0.0, 6.0),
        }
    }

    /// Where the way it's going about its move puts its whole body, see sway(), and how quickly it
    /// gets there
    fn sway_target(&self) -> ([f64; 3], f64) {
        let heavy = |m: Move| matches!(m, Move::Slam | Move::Quake | Move::Maul);
        match self.phase {
            // A knight crouched to sweep, and low as it spins, staying where it is so it spins on
            // the spot; and rising up to drive its sword in, then not tipped at all so the spikes
            // round it stand straight
            Phase::WindUp(Move::Sweep, _) => ([0.04, -0.08, -0.03], 5.0),
            Phase::Act(Move::Sweep, _) => ([0.0, 0.0, -0.04], 12.0),
            Phase::WindUp(Move::Spikes, _) => ([-0.04, -0.06, 0.06], 5.0),
            Phase::Act(Move::Spikes, _) => ([0.0, 0.0, 0.0], 25.0),
            Phase::WindUp(m, t) if heavy(m) => {
                let held = ((t - windup(self.look, m)) / MOST_HOLD).clamp(0.0, 1.0);
                ([-0.13 - 0.05 * held, -0.18, 0.06], 5.0)
            }
            Phase::WindUp(Move::Roar, _) | Phase::Act(Move::Roar, _) => ([-0.1, -0.05, 0.1], 6.0),
            Phase::WindUp(Move::Cataclysm, _) | Phase::Act(Move::Cataclysm, _) => ([-0.16, -0.1, 0.12], 3.0),
            Phase::WindUp(..) => ([-0.06, -0.06, 0.02], 5.0),
            // Lunging into the blow, quick
            Phase::Act(m, _) if heavy(m) => ([0.17, 0.55, -0.06], 22.0),
            Phase::Act(Move::Charge, _) => ([0.09, 0.0, 0.0], 8.0),
            Phase::Act(Move::Breath | Move::Spit | Move::Throw | Move::Volley | Move::Web, _) => ([0.06, 0.12, 0.0], 10.0),
            Phase::Stunned(_) => ([0.12, 0.05, -0.08], 4.0),
            // Settling back slowly after, heavily
            _ => ([0.0; 3], 3.0),
        }
    }

    /// Which pose it's striking as it goes about its move, or None to walk or stand
    fn target_pose(&self) -> Option<usize> {
        // A knight: its greatsword raised overhead to cleave, whether close or leaping, and
        // brought down as it lands; crouched with it drawn back low, then swept out, whether it's
        // sweeping round or flinging the void off it from afar; raised point down, then driven
        // into the ground
        if self.look.is_knight() {
            match self.phase {
                Phase::WindUp(Move::Leap, _) => return Some(POSE_WINDUP),
                Phase::Act(Move::Leap, t) => return Some(if t < leap(self.look).0 * 0.6 { POSE_WINDUP } else { POSE_STRIKE }),
                Phase::WindUp(Move::Sweep | Move::Spit, _) => return Some(POSE_COIL),
                Phase::Act(Move::Sweep | Move::Spit, _) => return Some(POSE_SWEEP),
                Phase::WindUp(Move::Spikes, _) => return Some(POSE_RAISE),
                Phase::Act(Move::Spikes, _) => return Some(POSE_SPIKES),
                _ => {}
            }
        }
        match (self.phase, self.look.build()) {
            // Taking aim and loosing, then spanning the crossbow again
            (Phase::WindUp(Move::Shoot, _), _) => Some(POSE_WINDUP),
            (Phase::Act(Move::Shoot, _), _) => Some(POSE_STRIKE),
            (Phase::Recover(_), _) if self.last == Some(Move::Shoot) => Some(POSE_SPECIAL),
            // Nosing down to dig in, then bursting up out of the sand
            (Phase::WindUp(Move::Burrow | Move::Erupt, _) | Phase::Act(Move::Burrow, _), _) => Some(POSE_SPECIAL),
            (Phase::Act(Move::Erupt, _), _) => Some(POSE_STRIKE),
            // Drawing in on itself to vanish
            (Phase::WindUp(Move::Blink, _) | Phase::Act(Move::Blink, _), _) => Some(POSE_SPECIAL),
            // Gaping, then flinging its mouth wide as its tongue lashes out
            (Phase::WindUp(Move::Lash, _), _) => Some(POSE_WINDUP),
            (Phase::Act(Move::Lash, _), _) => Some(POSE_SPECIAL),
            // A tusker paws the ground before it charges
            (Phase::WindUp(Move::Charge, _), Some(Build::Boar)) => Some(POSE_WINDUP),
            (Phase::Act(Move::Charge, _), Some(Build::Boar)) => Some(POSE_SPECIAL),
            (Phase::Act(Move::Roar, _), Some(Build::Drake)) => Some(POSE_SPECIAL),
            (Phase::Act(Move::Roar, _), _) => Some(POSE_WINDUP),
            // A wolf crouches to spring; howling, the dead raising their arms, calling the rest up
            (Phase::WindUp(Move::Leap, _), Some(Build::Wolf)) => Some(POSE_WINDUP),
            (Phase::WindUp(Move::Summon, _) | Phase::Act(Move::Summon, _), _) => Some(POSE_SPECIAL),
            (Phase::WindUp(Move::Throw | Move::Charge | Move::Leap | Move::Roots | Move::Storm | Move::Maul, _), _) => Some(POSE_SPECIAL),
            (Phase::WindUp(Move::Slam | Move::Breath | Move::Spit | Move::Volley | Move::Quake | Move::Web | Move::RootWave, _), _) => Some(POSE_WINDUP),
            (Phase::Act(Move::Roots | Move::Storm, _), _) => Some(POSE_SPECIAL),
            // Arms raised to the sky as it calls it down
            (Phase::WindUp(Move::Cataclysm, _) | Phase::Act(Move::Cataclysm, _), _) => Some(POSE_SPECIAL),
            // Rearing back as it lunges, then bringing it down, three times over
            (Phase::Act(Move::Maul, t), _) if t % MAUL_SWIPE < MAUL_LUNGE => Some(POSE_WINDUP),
            (Phase::Act(Move::Slam | Move::Throw | Move::Breath | Move::Spit | Move::Leap | Move::Volley | Move::Quake | Move::Maul | Move::Web | Move::RootWave, _), _) => Some(POSE_STRIKE),
            _ => None,
        }
    }

    /// The move it can make from `distance` meters off, if any's ready
    fn choose(&self, distance: f64) -> Option<Move> {
        let ready = |m: Move| self.cooldowns[m as usize] <= 0.0 && unlocked(self.look, m, self.stage);
        // A raid boss reaches as far further as it's bigger
        let size = self.look.size();
        let within = |m: Move, near: f64, far: f64| (near * size..far * size).contains(&distance) && ready(m);
        let monster = self.look.is_monster();
        let moves: &[(Move, f64, f64)] = match self.look.build() {
            // Raid bosses: what their kind does, and bringing the sky down once they're hurt
            _ if self.look == Look::Colossus => &[(Move::Slam, 0.0, 4.6), (Move::Cataclysm, 0.0, 30.0), (Move::Quake, 3.0, 13.0), (Move::Throw, 4.6, 32.0)],
            _ if self.look == Look::ElderWyrm => &[(Move::Slam, 0.0, 4.0), (Move::Cataclysm, 0.0, 30.0), (Move::Breath, 0.0, 9.0), (Move::Storm, 8.0, 30.0), (Move::Spit, 4.0, 35.0)],
            _ if self.look == Look::CryptTitan => &[
                (Move::Spikes, 0.0, 3.0),
                (Move::Sweep, 0.0, 3.0),
                (Move::Slam, 0.0, 4.6),
                (Move::Summon, 0.0, 30.0),
                (Move::Cataclysm, 0.0, 30.0),
                (Move::Quake, 3.0, 12.0),
                (Move::Spit, 4.6, 30.0),
            ],
            // The Grave Knight: bone spikes for whoever crowds it, its greatsword swept round or
            // brought down on them; the dead raised to fight for it; and from further off a leap
            // to cleave down on the player, or the void flung off its blade at them
            _ if self.look == Look::GraveKnight => &[(Move::Spikes, 0.0, 3.0), (Move::Sweep, 0.0, 3.0), (Move::Slam, 0.0, 4.6), (Move::Summon, 0.0, 30.0), (Move::Leap, 7.0, 20.0), (Move::Spit, 4.6, 30.0)],
            // Monsters bite or swing at what's in reach, and some pounce
            Some(Build::Wolf) if monster => &[(Move::Slam, 0.0, 2.2), (Move::Leap, 4.0, 11.0)],
            Some(Build::Spider) if monster => &[(Move::Slam, 0.0, 1.5), (Move::Leap, 3.5, 9.0)],
            _ if matches!(self.look, Look::Skeleton | Look::ArmoredSkeleton | Look::BanditBrute) => &[(Move::Slam, 0.0, 2.8)],
            // A bandit lunges in from a few steps off
            _ if self.look == Look::Bandit => &[(Move::Slam, 0.0, 2.6), (Move::Leap, 3.5, 8.0)],
            // The warlord charges, throws firebombs and calls the gang
            _ if self.look == Look::Warlord => &[(Move::Slam, 0.0, 3.8), (Move::Summon, 0.0, 30.0), (Move::Charge, 4.5, 22.0), (Move::Spit, 4.0, 26.0)],
            // A crossbowman shoots from well back, and clubs whatever gets right up to it
            _ if self.look == Look::Crossbowman => &[(Move::Slam, 0.0, 1.8), (Move::Shoot, 3.0, 28.0)],
            _ if self.look == Look::Shieldbearer => &[(Move::Slam, 0.0, 2.5)],
            // A burrower comes up under the player, and digs back in once they've got away from it
            _ if self.look == Look::DuneBurrower && self.buried => &[(Move::Erupt, 0.0, 2.0)],
            _ if self.look == Look::DuneBurrower => &[(Move::Slam, 0.0, 2.3), (Move::Burrow, 5.0, 40.0)],
            // A wraith vanishes from under the player's nose, claws them if it can't, and throws
            // frost from further off
            _ if self.look == Look::RimeWraith => &[(Move::Blink, 0.0, 3.0), (Move::Slam, 0.0, 2.1), (Move::Spit, 4.5, 24.0)],
            _ if self.look == Look::MireLurker => &[(Move::Slam, 0.0, 2.3), (Move::Lash, 3.0, 9.0)],
            _ if self.look == Look::Gloomwing => &[(Move::Slam, 0.0, 1.4), (Move::Leap, 3.0, 7.5)],
            _ if self.look == Look::Tusker => &[(Move::Slam, 0.0, 2.5), (Move::Charge, 5.0, 18.0)],
            // A bloater bursts once it's close; a ghoul claws, or leaps on the player from a few
            // steps off; a brute brings its gravestone down; an archer shoots from well back;
            // a necromancer raises the dead and hurls the void from behind them
            _ if self.look == Look::Bloater => &[(Move::Slam, 0.0, 1.9)],
            _ if self.look == Look::Ghoul => &[(Move::Slam, 0.0, 1.8), (Move::Leap, 3.5, 10.0)],
            _ if self.look == Look::GraveBrute => &[(Move::Slam, 0.0, 2.9)],
            _ if self.look == Look::BoneArcher => &[(Move::Slam, 0.0, 1.6), (Move::Shoot, 3.0, 30.0)],
            _ if self.look == Look::Necromancer => &[(Move::Slam, 0.0, 1.7), (Move::Summon, 0.0, 22.0), (Move::Spit, 3.5, 22.0)],
            Some(_) if monster => &[(Move::Slam, 0.0, 2.4)],
            // Ranges meeting, so there's nowhere to stand it won't do something
            Some(Build::Golem) => &[(Move::Slam, 0.0, 4.6), (Move::Quake, 3.0, 13.0), (Move::Throw, 4.6, 32.0)],
            Some(Build::Drake) => &[(Move::Slam, 0.0, 4.0), (Move::Breath, 0.0, 9.0), (Move::Storm, 10.0, 30.0), (Move::Spit, 4.0, 35.0)],
            Some(Build::Bear) => &[(Move::Slam, 0.0, 4.0), (Move::Maul, 2.5, 8.0), (Move::Charge, 4.0, 32.0)],
            Some(Build::Spider) => &[(Move::Slam, 0.0, 3.5), (Move::Summon, 0.0, 30.0), (Move::Web, 6.0, 20.0), (Move::Leap, 7.0, 22.0), (Move::Spit, 3.5, 25.0)],
            Some(Build::Treant) => &[(Move::Slam, 0.0, 5.0), (Move::RootWave, 7.0, 22.0), (Move::Roots, 4.0, 28.0), (Move::Volley, 8.0, GIVE_UP)],
            Some(Build::Wolf) => &[(Move::Slam, 0.0, 3.2), (Move::Summon, 0.0, 30.0), (Move::Maul, 2.5, 8.0), (Move::Leap, 7.0, 22.0)],
            Some(Build::Undead) => &[(Move::Slam, 0.0, 4.6), (Move::Summon, 0.0, 30.0), (Move::Quake, 3.0, 12.0), (Move::Spit, 4.6, 30.0)],
            // Only ever monsters, all seen to above
            Some(Build::Burrower | Build::Wraith | Build::Lurker | Build::Bat | Build::Boar) | None => &[],
        };
        moves.iter().find(|&&(m, near, far)| within(m, near, far)).map(|&(m, _, _)| m)
    }
}

/// Where a point of a boss's model, in meters with +x ahead, is in the world
fn on_body(a: &Animal, point: [f64; 3], vpm: f64) -> Vector3<f64> {
    let (c, s) = (a.heading.cos(), a.heading.sin());
    a.position + Vector3::new(c * point[0] - s * point[1], s * point[0] + c * point[1], point[2]) * vpm
}

/// Where fire, frost or venom comes out of it: its mouth, or the hand the dead cast with
fn mouth(a: &Animal, vpm: f64) -> Vector3<f64> {
    let point = match a.look.build() {
        Some(Build::Spider) => [1.35, 0.0, 1.1],
        Some(Build::Treant) => [0.6, 0.0, 3.0],
        Some(Build::Wolf) => [0.95, 0.0, 0.8],
        // The end of a crossbow raised to the shoulder
        _ if a.look == Look::Crossbowman => [0.82, -0.04, 1.44],
        // An arrow on a drawn bow, and a necromancer's orb thrust out on its staff
        _ if a.look == Look::BoneArcher => [0.8, 0.1, 1.44],
        _ if a.look == Look::Necromancer => [0.75, -0.2, 1.85],
        Some(Build::Undead) => [0.42, 0.26, 1.35],
        Some(Build::Burrower) => [0.95, 0.0, 0.32],
        // Between its claws thrust out together
        Some(Build::Wraith) => [0.9, 0.0, 1.5],
        Some(Build::Lurker) => [0.95, 0.0, 0.4],
        Some(Build::Bat) => [0.35, 0.0, 1.6],
        Some(Build::Boar) => [1.3, 0.0, 0.62],
        _ => [2.3, 0.0, 1.55],
    };
    on_body(a, point.map(|v| v * a.look.scale()), vpm)
}

/// The ground under `at`, in voxels, from a little over it to a few meters down, with room to
/// stand on it: None if there's none there
fn ground_under(world: &World, at: Vector3<f64>) -> Option<Vector3<f64>> {
    let vpm = world.voxels_per_meter as f64;
    let (x, y) = (at.x.floor() as i64, at.y.floor() as i64);
    if x < 2 || y < 2 || x >= world.size[0] as i64 - 2 || y >= world.size[1] as i64 - 2 {
        return None;
    }
    let top = at.z.floor() as i64 + 2 * vpm as i64;
    let room = |z: i64| (1..=(1.5 * vpm) as i64).all(|k| is_passable(world.material_at(x, y, z + k)));
    (top - 6 * vpm as i64..=top).rev().find(|&z| !is_passable(world.material_at(x, y, z)) && room(z)).map(|z| Vector3::new(at.x, at.y, z as f64 + 1.0))
}

/// How far from the player a monster that shoots likes to keep, in meters: it backs off from them
/// closer than the first, and comes on if they're further than the second
fn keep_off(look: Look) -> Option<(f64, f64)> {
    match look {
        Look::Crossbowman => Some((7.0, 18.0)),
        Look::BoneArcher => Some((8.0, 20.0)),
        Look::Necromancer => Some((7.0, 18.0)),
        Look::RimeWraith => Some((5.5, 16.0)),
        _ => None,
    }
}

/// Whether it hangs back from the player to shoot, rather than closing in to fight
pub fn hangs_back(look: Look) -> bool {
    keep_off(look).is_some()
}

/// Radians either side of straight at the player the members of a pack fan out as they come at
/// them, so they close in from round about them together
const FLANK: f64 = 0.6;
/// Meters from the middle of its pack a monster can get before it heads back towards the rest
const STRAY: f64 = 6.0;
/// How hard a monster's crowded off by another of its pack right on top of it, in meters a second
/// gained each second
const CROWDED: f64 = 5.0;

/// Which way monster `a`, chasing the player at `p` `distance` meters off, heads and how fast, to
/// come within `close` meters of them. In a pack it comes round to its own side of them, so the
/// pack closes in from round about together; it heads back towards the rest if it's strayed from
/// them, and holds back if it's got out ahead of them. One that shoots stands off and shoots,
/// sidestepping while it gets ready again; a shieldbearer walks in behind its shield; and a
/// gloomwing flits this way and that.
fn steer(a: &mut Animal, boss: &mut Boss, p: Vector3<f64>, distance: f64, close: f64, vpm: f64) {
    let look = a.look;
    if !look.is_monster() {
        return;
    }
    let flock = boss.flock;
    let straight = heading_to(a.position, p);
    if let Some((_, far)) = keep_off(look)
        && distance < far
    {
        // Keeping up behind the rest of its pack as they close in, shooting over them
        if flock.size > 1 && distance > (p - flock.front).xy().norm() / vpm + 8.0 {
            a.target_heading = straight;
            a.mood = Mood::Run;
            return;
        }
        let ranged = if matches!(look, Look::Crossbowman | Look::BoneArcher) { Move::Shoot } else { Move::Spit };
        if boss.cooldowns[ranged as usize] > 0.5 {
            let side = if ((boss.clock / 2.2) as i64 + a.id as i64) % 2 == 0 { 1.0 } else { -1.0 };
            a.target_heading = straight + side * FRAC_PI_2;
            a.mood = Mood::Walk;
        } else {
            a.target_heading = straight;
            a.mood = Mood::Idle;
        }
        return;
    }
    let mut heading = straight;
    if flock.size > 1 {
        let side = flock.slot as f64 - (flock.fighters.max(1) as f64 - 1.0) * 0.5;
        let fade = ((distance - close - 1.0) / 6.0).clamp(0.0, 1.0);
        heading += (side * FLANK).clamp(-1.3, 1.3) * fade;
        // How far it's got from the rest closing in with it, and how much nearer the player it is
        // than they are
        let stray = (a.position - flock.front).xy().norm() / vpm;
        let ahead = (p - flock.front).xy().norm() / vpm - distance;
        if stray > STRAY && distance > close + 3.0 {
            heading = heading_to(a.position, flock.front * 0.6 + p * 0.4);
        } else if ahead > 3.5 && distance > close + 2.5 && a.mood == Mood::Run {
            a.mood = Mood::Walk;
        }
    }
    match look {
        Look::Shieldbearer if distance < 5.0 && a.mood == Mood::Run => a.mood = Mood::Walk,
        Look::Gloomwing if distance > 1.5 => heading += 0.7 * (boss.clock * 2.3 + a.id as f64).sin(),
        _ => {}
    }
    a.target_heading = heading;
}

fn heading_to(from: Vector3<f64>, to: Vector3<f64>) -> f64 {
    let d = to - from;
    d.y.atan2(d.x)
}

/// What boss `a` does this frame: noticing the player at `player`, their feet in voxels, going after
/// them, and attacking. What its attacks do goes in its deeds.
pub fn think(a: &mut Animal, world: &World, player: Option<Vector3<f64>>, dt: f64) {
    let vpm = world.voxels_per_meter as f64;
    let look = a.look;
    let vitality = a.vitality();
    let Some(mut boss) = a.boss.take() else {
        return;
    };
    for c in &mut boss.cooldowns {
        *c -= dt;
    }
    boss.clock += dt;
    boss.retreat_wait -= dt;
    let struck = std::mem::take(&mut boss.struck);
    // Easing into whatever pose it took up last frame
    let pose = boss.target_pose();
    boss.posed = if pose == boss.posed.0 { (pose, boss.posed.1, boss.posed.2 + dt) } else { (pose, boss.posed.0, 0.0) };
    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
    let distance = player.map_or(f64::MAX, |p| (p - a.position).xy().norm() / vpm);
    let from_home = (a.position - boss.home).xy().norm() / vpm;

    // Turning on the player when it sees them near, not through the floor of a dungeon or the
    // rock of a cave, and giving up if they get away or it loses sight of them a while. Monsters
    // don't go as far.
    let monster = look.is_monster();
    let (notice, give_up, leash) = match look {
        Look::Wolf => (22.0, 34.0, 50.0),
        Look::Spiderling => (14.0, 24.0, 32.0),
        // Keeping watch with a crossbow, they see further
        Look::Crossbowman | Look::BoneArcher => (22.0, 32.0, 40.0),
        Look::Ghoul | Look::Necromancer => (18.0, 30.0, 38.0),
        Look::Tusker | Look::RimeWraith => (20.0, 32.0, 45.0),
        Look::DuneBurrower => (18.0, 30.0, 40.0),
        // Lying in wait, it doesn't stir till they're almost on top of it
        Look::MireLurker if boss.lurking => (7.0, 24.0, 30.0),
        Look::MireLurker => (16.0, 24.0, 30.0),
        Look::Gloomwing => (14.0, 26.0, 32.0),
        _ if monster => (MONSTER_NOTICE, MONSTER_GIVE_UP, MONSTER_LEASH),
        _ => (NOTICE, GIVE_UP, LEASH),
    };
    let level = player.is_some_and(|p| (p.z - a.position.z).abs() < 5.0 * vpm);
    boss.grace -= dt;
    let near = level && distance < notice && boss.grace <= 0.0;
    let seen = (near || boss.engaged) && player.is_some_and(|p| sees(a, p, world));
    if near && seen {
        boss.engaged = true;
    }
    // The rest of its pack is after the player: it comes along with them, whether or not it can
    // see them itself, so a pack fights together rather than one at a time
    let flock = boss.flock;
    let packed = monster && flock.size > 1;
    let alarmed = packed && flock.alarmed && level && boss.grace <= 0.0 && distance < give_up && (a.position - flock.middle).xy().norm() < 30.0 * vpm;
    if alarmed {
        boss.engaged = true;
    }
    boss.sighted = seen;
    boss.unseen = if seen || alarmed || !boss.engaged { 0.0 } else { boss.unseen + dt };
    // Stepping out of the way of the rest of its pack, so they keep close without standing in
    // each other, unless it's in the middle of a leap or a charge
    if packed && flock.crowd != Vector2::zeros() && !matches!(boss.phase, Phase::Act(Move::Leap | Move::Charge, _)) {
        a.status.push += Vector3::new(flock.crowd.x, flock.crowd.y, 0.0) * CROWDED * vpm * dt;
    }
    let lost = boss.unseen > if monster { MONSTER_LOST } else { LOST };
    if boss.engaged && (player.is_none() || distance > give_up || from_home > leash || lost) {
        boss.calm();
    }
    let busy = matches!(boss.phase, Phase::WindUp(..) | Phase::Act(..));
    if boss.engaged && boss.phase == Phase::Roam {
        // A monster gives a snarl or a rattle and comes straight on, a lurker bursting up out of
        // where it lay
        boss.phase = if monster { Phase::Chase } else { Phase::Act(Move::Roar, 0.0) };
        boss.lurking = false;
        boss.deeds.push(Deed::Roar { at: a.position, angry: false });
    } else if boss.engaged && look.is_raid() && !busy && (boss.stage as usize) < RAID_PHASES.len() && vitality < RAID_PHASES[boss.stage as usize] {
        // A raid boss going into its next phase: roaring, calling up help, ready to bring the sky
        // down, and in the last of them quicker about everything
        boss.stage += 1;
        boss.enraged = boss.stage as usize >= RAID_PHASES.len();
        boss.phase = Phase::Act(Move::Roar, 0.0);
        boss.deeds.push(Deed::Phase { at: a.position, stage: boss.stage });
        boss.cooldowns[Move::Cataclysm as usize] = 0.0;
        boss.cooldowns[Move::Summon as usize] = 0.0;
    } else if boss.engaged && !boss.enraged && !monster && !look.is_raid() && vitality < 0.5 && !busy {
        boss.enraged = true;
        boss.phase = Phase::Act(Move::Roar, 0.0);
        boss.deeds.push(Deed::Roar { at: a.position, angry: true });
    }
    // Angry, it's quicker about everything
    let quick: f64 = if boss.enraged { 0.75 } else { 1.0 };

    // The heavy ones shake the ground as they walk
    let heavy = matches!(look.build(), Some(Build::Golem | Build::Bear | Build::Treant)) || look.is_knight();
    if heavy && a.speed > 0.3 && (a.phase * 2.0).floor() != (boss.last_step * 2.0).floor() {
        boss.deeds.push(Deed::Stomp(a.position));
    }
    boss.last_step = a.phase;

    boss.phase = match boss.phase {
        Phase::Roam => {
            // Healing back up, wandering round home, keeping with the rest of its pack. A burrower
            // digs back in, and a lurker sinks back down to wait.
            a.health = (a.health + look.health() * 0.05 * dt).min(look.health());
            if look == Look::DuneBurrower && !boss.buried {
                boss.buried = true;
                boss.deeds.push(Deed::Burrow { at: a.position });
            }
            boss.lurking |= look == Look::MireLurker && from_home < 6.0;
            let strayed = packed && (flock.middle - a.position).xy().norm() > 3.5 * vpm && from_home < 12.0;
            if strayed {
                a.mood = Mood::Walk;
                a.target_heading = heading_to(a.position, flock.middle);
            } else if from_home > 6.0 {
                a.mood = Mood::Walk;
                a.target_heading = heading_to(a.position, boss.home);
            } else {
                a.mood = Mood::Idle;
            }
            Phase::Roam
        }
        Phase::Chase => {
            let p = player.unwrap_or(boss.home);
            a.target_heading = heading_to(a.position, p);
            // A crossbowman or a wraith the player's got too close to backs off, back towards the
            // rest of its pack, to shoot from further back; for a while, so it can't be run down
            // into a corner and left doing nothing
            if let Some((close_by, _)) = keep_off(look) {
                if boss.retreat > 0.0 {
                    boss.retreat -= dt;
                    if distance > close_by + 3.0 {
                        boss.retreat = 0.0;
                    }
                } else if distance < close_by && distance > 2.0 && boss.retreat_wait <= 0.0 {
                    boss.retreat = 2.5;
                    boss.retreat_wait = 5.5;
                }
            }
            // Only once it's facing them more or less, and a wraith struck vanishes from under
            // the blow if it can
            let facing = ((a.target_heading - a.heading + PI).rem_euclid(TAU) - PI).abs() < 0.6;
            let blink = (look == Look::RimeWraith && struck && boss.cooldowns[Move::Blink as usize] <= 0.0).then_some(Move::Blink);
            let backing = boss.retreat > 0.0 && distance > 1.8;
            match blink.or(boss.choose(distance).filter(|_| facing && !backing)) {
                Some(m) => {
                    a.mood = Mood::Idle;
                    boss.aim = p;
                    boss.deeds.push(Deed::WindUp(m, a.position));                    // A heavy blow's sometimes held back, wound up, a moment longer than the last,
                    // and sometimes followed by another, or two from a raid boss
                    let heavy_blow = !monster && matches!(m, Move::Slam | Move::Quake);
                    boss.hold = if heavy_blow && a.roll() < 0.35 { MOST_HOLD * (0.4 + 0.6 * a.roll()) } else { 0.0 };
                    boss.combo = match m {
                        Move::Slam if look.is_raid() => (a.roll() * 3.0) as u8,
                        // An angry knight nearly always follows up, and sometimes twice
                        Move::Slam if look.is_knight() && boss.enraged => (a.roll() < 0.75) as u8 + (a.roll() < 0.3) as u8,
                        Move::Slam if !monster && matches!(look.build(), Some(Build::Golem | Build::Undead | Build::Bear)) => (a.roll() < 0.45) as u8,
                        _ => 0,
                    };
                    Phase::WindUp(m, 0.0)
                }
                None => {
                    // Monsters come right up to the player
                    let close = closeness(look).0;
                    a.mood = if distance < close {
                        Mood::Idle
                    } else if matches!(look.build(), Some(Build::Bear | Build::Treant)) {
                        Mood::Walk
                    } else {
                        Mood::Run
                    };
                    if backing {
                        let away = (a.position - p).xy().try_normalize(1e-6).unwrap_or_else(|| -forward.xy());
                        let back = if packed { (flock.middle - a.position).xy() / vpm * 0.15 } else { Vector2::zeros() };
                        let way = away + back;
                        a.target_heading = way.y.atan2(way.x);
                        a.mood = Mood::Run;
                    } else {
                        steer(a, &mut boss, p, distance, close, vpm);
                    }
                    // Tunnelling along, it churns up the sand over it
                    if boss.buried && a.speed > 0.5 {
                        boss.trail -= dt;
                        if boss.trail <= 0.0 {
                            boss.trail = 0.14;
                            boss.deeds.push(Deed::Burrow { at: a.position });
                        }
                    }
                    Phase::Chase
                }
            }
        }
        Phase::WindUp(m, t) => {
            a.mood = Mood::Idle;
            // Following the player with its eyes as it gets ready, all but the last moment of
            // taking aim, so there's time to step aside. Where a burrower's coming up is fixed
            // from the first, and shows.
            let settled = matches!(m, Move::Shoot | Move::Lash | Move::Spit) && look.is_monster() && t > windup(look, m) - 0.25;
            if let Some(p) = player.filter(|_| m != Move::Erupt && !settled) {
                boss.aim = p;
                a.target_heading = heading_to(a.position, p);
            }
            // Showing how far its quake will reach
            if m == Move::Quake {
                boss.deeds.push(Deed::Mark { at: a.position, radius: quake_reach(look), closing: 0.0 });
            }
            // Where a knight's blow will land, showing on the ground, the ring closing in on it
            // as it comes: in front of it, all round it, or where it'll leap to
            if look.is_knight() {
                let closing = (1.0 - t / (windup(look, m) * quick.max(0.8) + boss.hold)).clamp(0.0, 1.0);
                let (sweep, spikes, _) = animals::knight_reach(look);
                let ring = match m {
                    Move::Slam => {
                        let (reach, radius, _, _) = slam(look);
                        Some((a.position + forward * reach * vpm, radius))
                    }
                    Move::Sweep => Some((a.position, sweep + RING_SPARE)),
                    Move::Spikes => Some((a.position, spikes + RING_SPARE)),
                    Move::Leap => Some((boss.aim, leap(look).2)),
                    _ => None,
                };
                if let Some((at, radius)) = ring {
                    boss.deeds.push(Deed::Mark { at, radius, closing });
                }
                // The ground stirring where the spikes will come up
                if m == Move::Spikes {
                    boss.trail -= dt;
                    if boss.trail <= 0.0 {
                        boss.trail = 0.12;
                        let round = a.roll() * TAU;
                        let r = (0.3 + 0.7 * a.roll()) * spikes * vpm;
                        boss.deeds.push(Deed::Burrow { at: a.position + Vector3::new(round.cos() * r, round.sin() * r, 0.0) });
                    }
                }
            }
            // The sand stirring where it's about to burst up
            if m == Move::Erupt {
                boss.deeds.push(Deed::Mark { at: boss.aim, radius: ERUPT_RADIUS, closing: 1.0 - t / windup(look, m) });
                boss.trail -= dt;
                if boss.trail <= 0.0 {
                    boss.trail = 0.1;
                    boss.deeds.push(Deed::Burrow { at: boss.aim });
                }
            }
            let t = t + dt;
            if t < windup(look, m) * quick.max(0.8) + boss.hold {
                Phase::WindUp(m, t)
            } else {
                // Now it does it
                boss.cooldowns[m as usize] = cooldown(look, m) * quick;
                match m {
                    Move::Slam => {
                        let (reach, radius, damage, crater) = slam(look);
                        boss.deeds.push(Deed::Slam { at: a.position + forward * reach * vpm, radius, damage, crater });
                        // A bloater's burst is the end of it, see Game::reap() for the cloud it
                        // leaves
                        if look == Look::Bloater {
                            a.health = animals::BURST;
                        }
                    }
                    // The dead rising, a pack answering a howl, spiderlings swarming out, to fight
                    // round it
                    Move::Summon => {
                        if let Some(brood) = look.brood() {
                            let count = if brood == Look::Spiderling || (look.is_knight() && boss.enraged) { 3 } else { 2 };
                            if boss.pack == 0 {
                                boss.pack = animals::new_pack();
                            }
                            boss.deeds.push(Deed::Summon { at: a.position, look: brood, count, pack: boss.pack });
                        }
                    }
                    Move::Shoot => boss.deeds.push(Deed::Bolt { from: mouth(a, vpm), to: boss.aim + Vector3::new(0.0, 0.0, 1.1 * vpm) }),
                    Move::Lash => boss.deeds.push(Deed::Lash { from: mouth(a, vpm), to: boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm) }),
                    Move::Burrow => {
                        boss.buried = true;
                        boss.deeds.push(Deed::Burrow { at: a.position });
                    }
                    // Up out of the sand where it showed, right by the player if they're still
                    // there, on the side it came from, so a pack comes up all round them
                    Move::Erupt => {
                        boss.buried = false;
                        let side = (a.position - boss.aim).xy().try_normalize(1e-6).unwrap_or_else(|| -forward.xy());
                        let near = boss.aim + Vector3::new(side.x, side.y, 0.0) * 1.3 * vpm;
                        if let Some(at) = ground_under(world, near).or_else(|| ground_under(world, boss.aim)) {
                            a.position = at;
                        }
                        let (_, _, damage, _) = slam(look);
                        boss.deeds.push(Deed::Erupt { at: a.position });
                        boss.deeds.push(Deed::Slam { at: boss.aim, radius: ERUPT_RADIUS, damage, crater: 0.0 });
                    }
                    // Gone in a swirl of frost, and back a few meters off to one side of the
                    // player, away from them, facing them
                    Move::Blink => {
                        let from = a.center(world.voxels_per_meter);
                        let away = (a.position - boss.aim).xy().try_normalize(1e-6).unwrap_or_else(|| -forward.xy());
                        let side: f64 = if a.roll() < 0.5 { 1.0 } else { -1.0 };
                        for turn in [side * 1.1, -side * 1.1, side * 0.3] {
                            let (c, s) = (turn.cos(), turn.sin());
                            let way = Vector3::new(away.x * c - away.y * s, away.x * s + away.y * c, 0.0);
                            if let Some(at) = ground_under(world, a.position + way * BLINK * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm)) {
                                a.position = at;
                                break;
                            }
                        }
                        a.heading = heading_to(a.position, boss.aim);
                        a.target_heading = a.heading;
                        boss.deeds.push(Deed::Blink { from, to: a.center(world.voxels_per_meter) });
                    }
                    Move::Throw => boss.deeds.push(Deed::Throw { from: on_body(a, [-0.55, -0.55, 4.05].map(|v| v * look.size()), vpm), to: boss.aim }),
                    // The void flung off a knight's blade as it sweeps it out, three of it fanned
                    // out across the player once it's angry
                    Move::Spit if look.is_knight() => {
                        let from = on_body(a, animals::knight_reach(look).2, vpm);
                        let to = boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm);
                        let across = Vector3::new(-(to - from).y, (to - from).x, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                        let fan: &[f64] = if boss.enraged { &[-1.0, 0.0, 1.0] } else { &[0.0] };
                        for &k in fan {
                            boss.deeds.push(Deed::Spit { from, to: to + across * k * SLASH_SPREAD * vpm });
                        }
                    }
                    Move::Spit => boss.deeds.push(Deed::Spit { from: mouth(a, vpm), to: boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm) }),
                    // Planting its feet to sweep its greatsword round, the sword hissing out low
                    Move::Sweep => {
                        boss.spun_from = a.heading;
                        boss.trail = 0.0;
                        boss.deeds.push(Deed::Stomp(a.position));
                        boss.deeds.push(Deed::Swipe(a.position, 0));
                    }
                    // Its greatsword driven into the ground, and bone spikes bursting up all round
                    // it, hurting whoever's still within them
                    Move::Spikes => {
                        let (_, spikes, _) = animals::knight_reach(look);
                        let damage = if look.is_raid() { SPIKES_DAMAGE * 1.3 } else { SPIKES_DAMAGE };
                        boss.deeds.push(Deed::Slam { at: a.position, radius: spikes, damage, crater: 0.0 });
                        boss.deeds.push(Deed::Stomp(a.position));
                        for k in 0..8 {
                            let round = k as f64 / 8.0 * TAU + a.roll() * 0.6;
                            let r = (0.4 + 0.55 * a.roll()) * spikes * vpm;
                            boss.deeds.push(Deed::Burrow { at: a.position + Vector3::new(round.cos() * r, round.sin() * r, 0.0) });
                        }
                    }
                    Move::Charge => {
                        boss.rammed = false;
                        a.target_heading = heading_to(a.position, boss.aim);
                        a.heading = a.target_heading;
                    }
                    Move::Leap => {
                        // Coming down just short of the player, not on top of them; a monster
                        // right in front of them
                        let off = boss.aim - a.position;
                        let short = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or(forward) * closeness(look).1 * vpm;
                        // On the ground there with room over it, not in the air or in a wall; where
                        // the player stands if there's nowhere like that
                        let to = boss.aim - short;
                        let (x, y) = (to.x.floor() as i64, to.y.floor() as i64);
                        let top = to.z.floor() as i64 + 2 * vpm as i64;
                        let room = |z: i64| (1..=(1.5 * vpm) as i64).all(|k| is_passable(world.material_at(x, y, z + k)));
                        let ground = (top - 6 * vpm as i64..=top).rev().find(|&z| !is_passable(world.material_at(x, y, z)) && room(z));
                        let to = ground.map_or(boss.aim, |z| Vector3::new(to.x, to.y, z as f64 + 1.0));
                        boss.leap = (a.position, to);
                        a.heading = heading_to(a.position, boss.aim);
                        a.target_heading = a.heading;
                        boss.deeds.push(Deed::Stomp(a.position));
                    }
                    Move::Roots => boss.deeds.push(Deed::Roots { at: boss.aim }),
                    Move::Volley => boss.deeds.push(Deed::Volley { from: mouth(a, vpm), to: boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm) }),
                    // The shockwave runs out from where it stands as it pounds the ground
                    Move::Quake => {
                        boss.leap.0 = a.position;
                        boss.deeds.push(Deed::Stomp(a.position));
                    }
                    Move::Maul => boss.deeds.push(Deed::Stomp(a.position)),
                    // Web, then a pounce on whoever's stuck in it
                    Move::Web => {
                        boss.deeds.push(Deed::Web { from: mouth(a, vpm), to: boss.aim });
                        boss.cooldowns[Move::Leap as usize] = boss.cooldowns[Move::Leap as usize].min(1.2);
                    }
                    // Angry, it sends three lines fanned out
                    Move::RootWave => boss.deeds.push(Deed::RootWave { from: a.position, to: boss.aim, lanes: if boss.enraged { 3 } else { 1 } }),
                    Move::Breath | Move::Roar | Move::Storm | Move::Cataclysm => {}
                }
                Phase::Act(m, 0.0)
            }
        }
        Phase::Act(m, t) => {
            let t = t + dt;
            a.mood = Mood::Idle;
            let mut over = t >= duration(look, m);
            match m {
                Move::Breath => {
                    // Sweeping slowly round after the player as it breathes, along the way it's
                    // facing, so they can get out of the way of it
                    if let Some(p) = player {
                        let turn = (heading_to(a.position, p) - a.heading + PI).rem_euclid(TAU) - PI;
                        a.target_heading = a.heading + turn.clamp(-BREATH_TURN * dt, BREATH_TURN * dt);
                    }
                    let from = mouth(a, vpm);
                    let to = player.map_or(from + forward * 8.0 * vpm, |p| p + Vector3::new(0.0, 0.0, 1.0 * vpm));
                    let ahead = (to - from).xy().norm().max(1.0);
                    let direction = Vector3::new(forward.x * ahead, forward.y * ahead, to.z - from.z).normalize();
                    boss.deeds.push(Deed::Breath { from, direction, dt });
                }
                Move::Charge => {
                    a.mood = Mood::Run;
                    if !boss.rammed && distance < 2.8 {
                        boss.rammed = true;
                        boss.deeds.push(Deed::Ram { damage: if look == Look::Tusker { 20.0 } else { 34.0 }, direction: forward });
                    }
                    // Run into a wall or a tree, it's left dazed: the player's chance
                    if t > 0.35 && a.speed < 1.0 {
                        boss.deeds.push(Deed::Stomp(a.position));
                        boss.deeds.push(Deed::Slam { at: a.position + forward * 2.0 * vpm, radius: 0.0, damage: 0.0, crater: 0.6 });
                        boss.phase = Phase::Stunned(2.0);
                        a.boss = Some(boss);
                        return;
                    }
                    // Past the player, it slows and turns round
                    over = over || (boss.rammed && distance > 6.0);
                }
                // Through the air in an arc, coming down hard where the player was
                Move::Leap => {
                    let (from, to) = boss.leap;
                    let (time, high, radius, damage) = leap(look);
                    let f = (t / time).min(1.0);
                    a.position = from.lerp(&to, f) + Vector3::new(0.0, 0.0, high * vpm * (PI * f).sin());
                    a.speed = 0.0;
                    boss.deeds.push(Deed::Mark { at: to, radius, closing: 1.0 - f });
                    if over {
                        a.position = to;
                        // A knight's blade cleaving down leaves no hole, so landing doesn't hold up
                        // the frame sending the ground's changes over
                        let crater = if monster || look.is_knight() { 0.0 } else { 0.6 };
                        boss.deeds.push(Deed::Slam { at: to, radius, damage, crater });
                        boss.deeds.push(Deed::Stomp(to));
                    }
                }
                // A knight's greatsword sweeping round low: out of the crouch across in front,
                // then on round with its whole body, all the way round once, or twice when it's
                // angry, hitting whoever's within its reach and not jumping as it passes them.
                // The ground's churned up under its point.
                Move::Sweep => {
                    let spins = boss.spins();
                    let swinging = SPIN * spins;
                    let arc = TAU * spins - (SWEPT - COILED);
                    let turned = |t: f64| arc * ((t - SWING) / (swinging - SWING)).clamp(0.0, 1.0);
                    let from = boss.spun_from;
                    let blade = |t: f64| from + turned(t) + COILED + (SWEPT - COILED) * (t / SWING).min(1.0);
                    a.heading = (from + turned(t)).rem_euclid(TAU);
                    a.target_heading = a.heading;
                    let (reach, _, _) = animals::knight_reach(look);
                    if let Some(p) = player {
                        let bearing = heading_to(a.position, p);
                        let passes = |t: f64| ((blade(t) - bearing) / TAU).floor();
                        if t - dt < swinging && passes(t) != passes((t - dt).max(0.0)) {
                            boss.deeds.push(Deed::Quake { at: a.position, from: 0.5, to: reach + RING_SPARE, dt });
                        }
                    }
                    boss.trail -= dt;
                    if t < swinging && boss.trail <= 0.0 {
                        boss.trail = 0.05;
                        let b = blade(t);
                        boss.deeds.push(Deed::Burrow { at: a.position + Vector3::new(b.cos(), b.sin(), 0.0) * reach * 0.95 * vpm });
                    }
                    // Into each further time round
                    if t > SWING && ((t - SWING) / SPIN).floor() != ((t - dt - SWING).max(0.0) / SPIN).floor() && t < swinging {
                        boss.deeds.push(Deed::Swipe(a.position, ((t - SWING) / SPIN) as usize));
                    }
                    over = t >= swinging + SPIN_SETTLE;
                }
                // The shockwave running out across the ground
                Move::Quake => {
                    let to = (t * QUAKE_PACE).min(quake_reach(look));
                    boss.deeds.push(Deed::Quake { at: boss.leap.0, from: (t - dt) * QUAKE_PACE, to, dt });
                }
                // Its element raining down all round the player, one after another
                Move::Cataclysm => {
                    let count = |t: f64| if t < CATACLYSM_START { None } else { Some(((t - CATACLYSM_START) / CATACLYSM_EVERY) as usize) };
                    if let Some(k) = count(t).filter(|&k| count(t - dt) != Some(k) && k < CATACLYSM_FALLS) {
                        boss.deeds.push(Deed::Cataclysm { to: player.unwrap_or(boss.aim), k });
                    }
                }
                // Breathing up into the sky, then its element raining down round the player
                Move::Storm => {
                    if t < 0.5 {
                        let from = mouth(a, vpm);
                        boss.deeds.push(Deed::Breath { from, direction: (forward * 0.3 + Vector3::z()).normalize(), dt });
                    }
                    let count = |t: f64| if t < STORM_START { None } else { Some(((t - STORM_START) / STORM_EVERY) as usize) };
                    if let Some(k) = count(t).filter(|&k| count(t - dt) != Some(k)) {
                        boss.deeds.push(Deed::Storm { to: player.unwrap_or(boss.aim), k });
                    }
                }
                // Lunging and swiping three times, turning only a little after the player between
                Move::Maul => {
                    let (k, local) = ((t / MAUL_SWIPE) as usize, t % MAUL_SWIPE);
                    if t - dt <= 0.0 || k != ((t - dt) / MAUL_SWIPE) as usize {
                        let turn = if look.element() == Some(Element::Frost) { 0.45 } else { 0.7 };
                        if let Some(p) = player {
                            let off = (heading_to(a.position, p) - a.heading + PI).rem_euclid(TAU) - PI;
                            a.heading += off.clamp(-turn, turn);
                        }
                        boss.deeds.push(Deed::Swipe(a.position, k));
                        boss.deeds.push(Deed::Stomp(a.position));
                    }
                    a.target_heading = a.heading;
                    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
                    let radius = if k >= 2 { 2.6 } else { 1.8 };
                    if local < MAUL_LUNGE {
                        // Up to the player, not through them
                        a.mood = if distance > look.reach() * 0.75 { Mood::Run } else { Mood::Idle };
                        let ahead = 2.2 + a.speed * (MAUL_LUNGE - local);
                        boss.deeds.push(Deed::Mark { at: a.position + forward * ahead * vpm, radius, closing: 1.0 - local / MAUL_LUNGE });
                    } else if local - dt < MAUL_LUNGE {
                        let last = if look.element() == Some(Element::Frost) { 20.0 } else { 26.0 };
                        let (damage, crater) = if k >= 2 { (last, 0.7) } else { (12.0, 0.0) };
                        boss.deeds.push(Deed::Slam { at: a.position + forward * 2.2 * vpm, radius, damage, crater });
                    }
                }
                _ => {}
            }
            match (over, m) {
                // Following one blow straight up with another, wound up only partway
                (true, Move::Slam) if boss.combo > 0 => {
                    boss.combo -= 1;
                    boss.hold = 0.0;
                    if let Some(p) = player {
                        a.target_heading = heading_to(a.position, p);
                    }
                    boss.deeds.push(Deed::WindUp(Move::Slam, a.position));
                    Phase::WindUp(Move::Slam, windup(look, Move::Slam) * 0.45)
                }
                (true, _) => {
                    boss.last = Some(m);
                    Phase::Recover(recovery(look, m) * quick)
                }
                _ => Phase::Act(m, t),
            }
        }
        Phase::Recover(t) => {
            a.mood = Mood::Idle;
            if t - dt > 0.0 { Phase::Recover(t - dt) } else { Phase::Chase }
        }
        Phase::Stunned(t) => {
            a.mood = Mood::Idle;
            a.speed = 0.0;
            if t - dt > 0.0 { Phase::Stunned(t - dt) } else { Phase::Chase }
        }
    };
    if !boss.engaged && boss.phase != Phase::Roam && !matches!(boss.phase, Phase::Act(..)) {
        boss.phase = Phase::Roam;
    }
    // Its whole body easing towards where its move puts it, as big as it is, a monster's less
    let (target, rate) = boss.sway_target();
    let amount = if monster { 0.6 } else { 1.0 };
    let reach = [amount, amount * look.body().1, amount * look.body().1];
    let ease = 1.0 - (-rate * dt).exp();
    for k in 0..3 {
        boss.sway[k] += (target[k] * reach[k] - boss.sway[k]) * ease;
    }
    let (sink, rate) = boss.sink_target();
    boss.sink += (sink - boss.sink) * (1.0 - (-rate * dt).exp());
    a.boss = Some(boss);
}
