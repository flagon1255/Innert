//! Aiming at a spot: where a special attack or a power aimed at the ground comes down, and the
//! ring on the ground showing it, as bombs show where they'll land while they're aimed. The ring
//! shows while it's aimed, right click held with it charged, through its move till it's let go,
//! and on while what carries it is on its way, so where it's shown and where it lands are one.
//! Aimed at the sky, or further off than it reaches, it comes down as far off as it reaches that
//! way, along the ground: up slopes, down off ledges, and short of walls in the way.

use super::*;
use crate::weapons::Power;
use super::signatures::powers::{
    ANVIL_REACH, BREATH_REACH, FISSURE_FROM, FISSURE_STEP, FISSURE_STEPS, FISTS_REACH, GALE_FROM, GALE_STEP, GALE_STEPS, HOOK_REACH, JAVELIN_REACH, LANCE_REACH, LASH_FROM, NOVA_REACH, RING_REACH, SEEKER_REACH, SHRAPNEL_REACH, STAKE_REACH, WHEEL_REACH, power_timing, wave_of,
};

/// How far a special attack aimed at a spot reaches, in meters out along the ground
pub(super) const SPOT_REACH: f64 = 30.0;
/// How far under what's aimed at the ground's looked for, in meters
const SPOT_DROP: f64 = 12.0;
/// How far the ground can rise each meter out for what's aimed along it to go on up it, and how
/// far apart it's looked at, in meters
const SPOT_CLIMB: f64 = 2.0;
const SPOT_STEP: f64 = 1.0;

/// How a special attack or power aimed at a spot is aimed: how far it reaches and how near it can
/// come, in meters out along the ground, and how big round its ring is
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SpotAim {
    pub(super) reach: f64,
    pub(super) least: f64,
    pub(super) radius: f64,
}

/// How `special` on a weapon with `profile` is aimed, if it's aimed at a spot. A bomb's thrown as
/// far as the bombs throw their bombs, so it lands in their ring.
pub(super) fn spot_aim(special: Special, profile: &Profile) -> Option<SpotAim> {
    let (reach, radius, least) = match special {
        Special::Meteors => (SPOT_REACH, 4.5, 0.0),
        Special::Rain => (SPOT_REACH, 4.5, 0.0),
        Special::BoltRain => (SPOT_REACH, BOLT_RAIN_REACH, 0.0),
        Special::Barrage => (SPOT_REACH, BARRAGE_RING, 0.0),
        Special::Comet => (SPOT_REACH, COMET_RING, 0.0),
        Special::Tempest => (SPOT_REACH, TEMPEST_ROUND, 0.0),
        Special::Blizzard => (SPOT_REACH, 5.0, 0.0),
        Special::Thunderstorm => (SPOT_REACH, 7.0, 0.0),
        Special::Sunburst => (SPOT_REACH, 9.0, 0.0),
        Special::Singularity => (SPOT_REACH, 4.5, 0.0),
        Special::Stasis => (SPOT_REACH, 6.0, 0.0),
        Special::RuneMines => (SPOT_REACH, 4.0, 0.0),
        Special::Plague => (SPOT_REACH, 5.5, 0.0),
        Special::Mire => (SPOT_REACH, 6.0, 0.0),
        Special::Eruption => (SPOT_REACH, ERUPTION_ROUND, 0.0),
        Special::Wildfire => (SPOT_REACH, WILDFIRE_ROUND, 0.0),
        Special::Stormchain => (SPOT_REACH, STORMCHAIN_ROUND, 0.0),
        Special::ClusterBomb => (SPOT_REACH, 4.5, 0.0),
        Special::PowderKeg => (SPOT_REACH, 5.0, 2.0),
        Special::Skyburst => (SPOT_REACH, SKYBURST_ROUND, 0.0),
        Special::Firewall => (FIREWALL_REACH, FIREWALL_RING, 3.0),
        Special::NetBolt => (SPOT_REACH, 4.5, 0.0),
        Special::Shackle => (CHAIN_FLING, 8.0, 0.0),
        Special::Scuttlers => (SPOT_REACH, 3.0, 2.0),
        Special::Firecrackers => (SPOT_REACH, 1.5, 5.0),
        Special::Stormstakes => (STAKES_AIM, 1.0, 6.0),
        Special::Lantern => (LANTERN_AIM, 2.0, 0.0),
        Special::Litany => (LITANY_AIM, 4.5, 0.0),
        Special::EmberKnell => (KNELL_AIM, 2.5, 0.0),
        Special::Bloodthread => (THREAD_AIM, 2.5, 0.0),
        Special::Bloodmoon => (SPOT_REACH, MOON_RING, 0.0),
        _ => return None,
    };
    let reach = if profile.style == Style::Lob { profile.stat("range").max(6.0) } else { reach };
    Some(SpotAim { reach, least, radius })
}

/// How a power on right click is aimed, if it's aimed at a spot: a hand cannon's mortar, a staff's
/// comet and bombs' vortex, its ring as big as the bombs' own
pub(super) fn power_aim(power: Power, profile: &Profile) -> Option<SpotAim> {
    match power {
        Power::Mortar => Some(SpotAim { reach: MORTAR_REACH, least: 0.0, radius: 3.5 }),
        Power::Comet => Some(SpotAim { reach: SPOT_REACH, least: 0.0, radius: 3.5 }),
        Power::Vortex => Some(SpotAim { reach: profile.stat("range").max(6.0), least: 0.0, radius: BOMB_BLAST * profile.stat("area").max(0.5) }),
        Power::Anvil => Some(SpotAim { reach: ANVIL_REACH, least: 2.5, radius: 3.5 }),
        Power::Stake => Some(SpotAim { reach: STAKE_REACH, least: 3.0, radius: 3.0 }),
        Power::Cloudburst => Some(SpotAim { reach: SPOT_REACH, least: 4.0, radius: 4.5 }),
        Power::Starshell => Some(SpotAim { reach: MORTAR_REACH, least: 4.0, radius: 6.0 }),
        Power::Beacon => Some(SpotAim { reach: SPOT_REACH, least: 0.0, radius: 5.0 }),
        Power::Shrapnel => Some(SpotAim { reach: profile.stat("range").max(6.0), least: 0.0, radius: SHRAPNEL_REACH }),
        _ => None,
    }
}

/// How far round where it comes down Bolt Rain, Tempest, Eruption, Wildfire, Stormchain and
/// Skyburst look for what to strike, in meters
pub(super) const BOLT_RAIN_REACH: f64 = 7.0;
pub(super) const TEMPEST_ROUND: f64 = 9.0;
pub(super) const ERUPTION_ROUND: f64 = 7.0;
pub(super) const WILDFIRE_ROUND: f64 = 5.0;
pub(super) const STORMCHAIN_ROUND: f64 = 5.0;
pub(super) const SKYBURST_ROUND: f64 = 8.0;
/// How big round each of the rings along a barrage's line and across a wall of fire are, and the
/// comet's, as it shows its own as it falls, in meters
const BARRAGE_RING: f64 = 2.4;
const FIREWALL_RING: f64 = 1.1;
const COMET_RING: f64 = 6.0;
/// How far a wall of fire, a mortar's shell and the signature special attacks shot at the ground
/// reach, in meters
const FIREWALL_REACH: f64 = 18.0;
const MORTAR_REACH: f64 = 40.0;
pub(super) const LANTERN_AIM: f64 = 30.0;
pub(super) const LITANY_AIM: f64 = 24.0;
pub(super) const KNELL_AIM: f64 = 18.0;
pub(super) const THREAD_AIM: f64 = 22.0;
/// How far apart a barrage's shells come down along its line, and how many, and a wall of fire's
/// stretches across, in meters
pub(super) const BARRAGE_SHELLS: usize = 7;
pub(super) const BARRAGE_GAP: f64 = 2.2;
pub(super) const FIREWALL_GAP: f64 = 2.0;
/// How many little charges a string of firecrackers is, and how far apart along it, in meters
pub(super) const CRACKERS: usize = 8;
pub(super) const CRACKER_GAP: f64 = 1.25;
/// Where storm stakes go into the ground round where they're aimed, in meters on along the way
/// from the player and across it to the right: the far one, then left, then right
pub(super) const STAKE_SPOTS: [(f64, f64); 3] = [(2.33, 0.0), (-1.17, -4.5), (-1.17, 4.5)];
/// How far storm stakes reach, in meters
const STAKES_AIM: f64 = 22.0;

/// The rings showing where `special` comes down at `spot`, sent out along `way`: one round it, or
/// for a barrage a line of them marching on along `way` and for a wall of fire a row across it
fn rings(special: Special, aim: SpotAim, spot: Vector3<f64>, way: Vector3<f64>, vpm: f64) -> Vec<(Vector3<f64>, f64)> {
    let across = way.cross(&Vector3::z());
    match special {
        Special::Barrage | Special::Firecrackers => {
            let half = if special == Special::Barrage { (BARRAGE_SHELLS - 1) as f64 * 0.5 * BARRAGE_GAP } else { (CRACKERS - 1) as f64 * 0.5 * CRACKER_GAP };
            [-half, 0.0, half].iter().map(|&d| (spot + way * d * vpm, aim.radius)).collect()
        }
        Special::Stormstakes => STAKE_SPOTS.iter().map(|&(on, side)| (spot + (way * on + way.cross(&Vector3::z()) * side) * vpm, aim.radius)).collect(),
        Special::Firewall => (0..5).map(|k| (spot + across * (k as f64 - 2.0) * FIREWALL_GAP * vpm, aim.radius)).collect(),
        _ => vec![(spot, aim.radius)],
    }
}

/// `v` along the ground
fn flat_way(v: Vector3<f64>) -> Vector3<f64> {
    Vector3::new(v.x, v.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x)
}

/// Which way along the ground `spot` is from the player, or the way they look if it's at their feet
pub(super) fn way_to(arena: &Arena, spot: Vector3<f64>) -> Vector3<f64> {
    let view = arena.view;
    let feet = arena.player.unwrap_or(view.eye - Vector3::z() * 1.6 * view.vpm);
    let off = (spot - feet).xy();
    if off.norm() > 0.5 * view.vpm { Vector3::new(off.x, off.y, 0.0).normalize() } else { flat_way(view.forward()) }
}

/// Whether there's anything solid at `p`
fn solid_at(world: &World, p: Vector3<f64>) -> bool {
    p.z < 0.0 || !is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
}

/// The top of the first solid ground under `at`, within `depth` voxels of it, or None if there's
/// none or `at` is in something solid itself
fn ground_below(world: &World, at: Vector3<f64>, depth: f64) -> Option<Vector3<f64>> {
    if solid_at(world, at) {
        return None;
    }
    let (x, y) = (at.x.floor() as i64, at.y.floor() as i64);
    let lowest = (at.z - depth).floor().max(-1.0) as i64;
    let mut z = at.z.floor() as i64;
    while z > lowest {
        z -= 1;
        if z < 0 || !is_passable(world.material_at(x, y, z)) {
            return (z >= 0).then(|| Vector3::new(at.x, at.y, (z + 1) as f64));
        }
    }
    None
}

/// The ground `meters` out from `feet` along `way`, followed along the ground step by step: up
/// slopes, down off ledges, stopping short of a wall or a drop too deep to see the bottom of, and
/// on the water where it's over the ground
fn along_ground(arena: &Arena, feet: Vector3<f64>, way: Vector3<f64>, meters: f64) -> Vector3<f64> {
    let world = arena.world;
    let vpm = arena.view.vpm;
    let up = Vector3::z();
    // Where it starts from: under the feet, or far under them for a player flying
    let mut at = ground_below(world, feet + up * 0.5 * vpm, 64.0 * vpm).unwrap_or(feet);
    let steps = (meters / SPOT_STEP).ceil().max(0.0) as usize;
    for k in 1..=steps {
        let out = (k as f64 * SPOT_STEP).min(meters);
        let probe = Vector3::new(feet.x + way.x * out * vpm, feet.y + way.y * out * vpm, at.z + SPOT_CLIMB * vpm);
        if probe.x < 0.0 || probe.y < 0.0 || probe.x >= world.size[0] as f64 || probe.y >= world.size[1] as f64 {
            break;
        }
        match ground_below(world, probe, (SPOT_CLIMB + SPOT_DROP) * vpm) {
            Some(ground) => at = ground,
            None => break,
        }
    }
    on_water(arena, at)
}

/// `at` brought up to the top of the water, if it's under it
fn on_water(arena: &Arena, at: Vector3<f64>) -> Vector3<f64> {
    let surface = (arena.water)(at.x, at.y);
    if surface.is_finite() && surface > at.z { Vector3::new(at.x, at.y, surface) } else { at }
}

impl Combat {
    /// Where something aimed with the crosshair on `crosshair`, see aim(), comes down, reaching
    /// `reach` meters out along the ground: on the ground there, or under what's aimed at, an
    /// animal or a wall, if that's in reach; else as far as it reaches that way, along the ground
    pub(super) fn spot_from(&self, arena: &Arena, crosshair: Vector3<f64>, reach: f64) -> Vector3<f64> {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let nothing = (crosshair - view.eye).norm() > 0.99 * AIM_RANGE * vpm;
        let off = (crosshair - feet).xy();
        let way = off.try_normalize(1e-6).map_or_else(|| flat_way(view.forward()), |w| Vector3::new(w.x, w.y, 0.0));
        if !nothing && off.norm() <= reach * vpm {
            // On the ground it's on, or under what it's on, backed off a wall towards the eye
            let back = crosshair - (crosshair - view.eye).try_normalize(1e-6).unwrap_or_else(|| view.forward()) * 0.3 * vpm;
            if let Some(ground) = ground_below(arena.world, crosshair + up * 0.25 * vpm, SPOT_DROP * vpm).or_else(|| ground_below(arena.world, back, SPOT_DROP * vpm)) {
                return on_water(arena, ground);
            }
        }
        let meters = if nothing { reach } else { (off.norm() / vpm).min(reach) };
        along_ground(arena, feet, way, meters)
    }

    /// Where something aimed with the crosshair as it is now comes down, reaching `reach` meters
    pub(super) fn spot(&self, arena: &Arena, reach: f64) -> Vector3<f64> {
        self.spot_from(arena, self.aim(arena), reach)
    }

    /// Where a spot aimed `aim` comes down, as spot() has it, but never nearer than it can come
    fn spot_aimed(&self, arena: &Arena, aim: SpotAim) -> Vector3<f64> {
        let spot = self.spot(arena, aim.reach);
        let view = arena.view;
        let vpm = view.vpm;
        let feet = arena.player.unwrap_or(view.eye - Vector3::z() * 1.6 * vpm);
        if (spot - feet).xy().norm() >= aim.least * vpm {
            return spot;
        }
        along_ground(arena, feet, flat_way(view.forward()), aim.least)
    }

    /// Where `special` on `weapon`, let go now, comes down, if it's aimed at a spot
    pub(super) fn special_spot(&self, arena: &Arena, weapon: &Weapon, special: Special) -> Option<Vector3<f64>> {
        spot_aim(special, &weapon.profile).map(|aim| self.spot_aimed(arena, aim))
    }

    /// Where `weapon`'s power, let go now, comes down, if it's aimed at a spot
    pub(super) fn power_spot(&self, arena: &Arena, weapon: &Weapon) -> Option<Vector3<f64>> {
        let power = weapon.profile.power?;
        if matches!(power, Power::Vortex | Power::Shrapnel) {
            return Some(self.lob_aim(arena, weapon).0);
        }
        power_aim(power, &weapon.profile).map(|aim| self.spot_aimed(arena, aim))
    }

    /// The rings for `special` coming down at `spot`, for `life` more seconds, after it's let go
    pub(super) fn keep_rings(&mut self, arena: &Arena, profile: &Profile, special: Special, spot: Vector3<f64>, life: f64) {
        if let Some(aim) = spot_aim(special, profile) {
            for (at, radius) in rings(special, aim, spot, way_to(arena, spot), arena.view.vpm) {
                self.spots.push((at, radius, life));
            }
        }
    }

    /// The ring for `weapon`'s power coming down at `spot`, for `life` more seconds
    pub(super) fn keep_power_ring(&mut self, weapon: &Weapon, spot: Vector3<f64>, life: f64) {
        if let Some(aim) = weapon.profile.power.and_then(|p| power_aim(p, &weapon.profile)) {
            self.spots.push((spot, aim.radius, life));
        }
    }

    /// What `special` let go now is aimed at: for those that throw, shoot or fling what they do at
    /// a spot themselves, the spot, its rings kept showing till it's there; for the rest the
    /// crosshair, see aim()
    pub(super) fn aimed_target(&mut self, arena: &Arena, weapon: &Weapon, special: Special) -> Vector3<f64> {
        let crosshair = self.aim(arena);
        let Some(spot) = self.special_spot(arena, weapon, special) else {
            return crosshair;
        };
        let view = arena.view;
        let far = (spot - view.eye).norm() / view.vpm;
        // About as long as what it sends gets there
        let life = match special {
            Special::ClusterBomb => VOLLEY_FIRST + 4.0 * VOLLEY_GAP + 1.1 + 0.025 * far,
            Special::Shackle => SHACKLE_RELEASE + 0.7,
            Special::Skyburst => 0.5 + far / 20.0,
            Special::NetBolt => 0.15 + far / 45.0,
            Special::Firecrackers => 2.0 + 0.025 * far,
            // Its move, then its shots flying there, see BloodMoon
            Special::Bloodmoon => 1.4 + far / BLOODSHOT_PACE,
            _ => return crosshair,
        };
        self.keep_rings(arena, &weapon.profile, special, spot, life);
        spot
    }

    /// The rings for `special`, carried there, kept showing a moment as what it calls down comes
    /// down at `spot`
    pub(super) fn arrived_rings(&mut self, arena: &Arena, profile: &Profile, special: Special, spot: Vector3<f64>) {
        let life = match special {
            Special::Meteors => 1.3,
            Special::Rain | Special::Barrage => 1.2,
            Special::BoltRain | Special::Eruption => 0.8,
            Special::Tempest => TEMPEST_GATHER + 0.8,
            _ => 0.25,
        };
        self.keep_rings(arena, profile, special, spot, life);
    }

    /// The rings on the ground this frame showing where what's aimed at a spot comes down, as the
    /// bombs' own ring does while they're aimed: `weapon`'s special attack while it's aimed with
    /// right click held, charged, and through its move till it's let go; its power through its own
    /// move; what's let go on its way there; and what's been let go of a while yet
    pub(super) fn mark_aims(&mut self, arena: &Arena, weapon: Option<&Weapon>, controls: Controls, dt: f64) {
        let vpm = arena.view.vpm;
        if let Some(weapon) = weapon {
            let profile = &weapon.profile;
            if let Some(special) = profile.special.as_ref().map(|s| s.0)
                && let Some(aim) = spot_aim(special, profile)
            {
                let moving = match self.action {
                    Action::Special { t, length, special: s } if s == special => release(special, profile) != Release::AtOnce && t <= lets_go(special, profile) * length,
                    _ => false,
                };
                // Aimed with right click, charged, but not bombs, which show where their own throw
                // comes down then, the same as it
                let aiming = controls.aiming && self.charge >= 1.0 && self.raise >= 1.0 && profile.style != Style::Lob && !matches!(self.action, Action::Special { .. } | Action::Power { .. });
                if moving || aiming {
                    let spot = self.spot_aimed(arena, aim);
                    self.marks.extend(rings(special, aim, spot, way_to(arena, spot), vpm));
                }
            }
            // Its spot while it's aimed, right click held, and through its move till it's let go
            let held = self.power.aiming && controls.aiming;
            if let Some(power) = profile.power
                && (held || matches!(self.action, Action::Power { t, length } if t <= power_timing(power).1 * length))
                && let Some(spot) = self.power_spot(arena, weapon)
                && let Some(aim) = power_aim(power, profile)
            {
                self.marks.push((spot, aim.radius));
            }
            // Every other power: while right click's held with it ready, a heavy blow carrying it
            // is coming down, or its own move's going on, till it's let go
            if let Some(power) = profile.power
                && power_way(power) != PowerWay::Spot
            {
                let ready = self.power.cooldown <= 0.0 && self.raise >= 1.0;
                let swinging = ready && matches!(self.action, Action::Swing { heavy: true, struck: false, .. });
                let moving = matches!(self.action, Action::Power { t, length } if t <= power_timing(power).1 * length);
                let aiming = ready && controls.aiming && !matches!(self.action, Action::Special { .. });
                if swinging || moving || aiming {
                    self.power_marks(arena, power);
                }
            }
        }
        // What carries a special attack on its way to where it comes down
        for p in &self.projectiles {
            if let Trick::Carry { special: Some(special), target, .. } = p.trick
                && let Some(aim) = spot_aim(special, &p.profile)
            {
                self.marks.extend(rings(special, aim, target, way_to(arena, target), vpm));
            }
        }
        for s in &mut self.spots {
            s.2 -= dt;
        }
        self.spots.retain(|s| s.2 > 0.0);
        self.marks.extend(self.spots.iter().map(|s| (s.0, s.1)));
    }
}

/// How a power on right click is aimed, and what shows where it'll go as it's aimed, see
/// Combat::power_marks()
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum PowerWay {
    /// Onto a spot on the ground, a ring round it, see power_aim()
    Spot,
    /// Along the crosshair to what it first meets within `reach` meters, or as far as that, or
    /// along each of `fan`'s turns either side of it, in radians: a ring `radius` round where each
    /// meets the ground or a wall, or ends, or a mark over the animal it meets
    Ray { reach: f64, fan: &'static [f64], radius: f64 },
    /// Along the ground the way it's aimed, from `from` meters out, `steps` steps `step` apart, a
    /// ring at each
    Line { from: f64, step: f64, steps: usize },
    /// At what's within SEEK_ROUND of what the crosshair's on within `reach`, marked over each,
    /// or else onto a ring there
    Seek { reach: f64 },
    /// All round the player, a ring `reach` meters round the feet
    Round { reach: f64 },
}

/// How far round what the crosshair's on a power's seekers look for what to go for, in meters
pub(super) const SEEK_ROUND: f64 = 7.0;
/// A blade fan's crescents and a breath's embers, turned either side of the crosshair, in radians
pub(super) const FAN: [f64; 3] = [-0.3, 0.0, 0.3];
pub(super) const BREATH_FAN: [f64; 3] = [-0.42, 0.0, 0.42];
/// How far apart the rings along a whip's wave are, in meters
const LASH_STEP: f64 = 2.0;

/// How `power` is aimed
pub(super) fn power_way(power: Power) -> PowerWay {
    let wave = |p: Power| wave_of(p).3;
    match power {
        Power::Mortar | Power::Comet | Power::Vortex | Power::Anvil | Power::Stake | Power::Cloudburst | Power::Starshell | Power::Beacon | Power::Shrapnel => PowerWay::Spot,
        // A gale runs on along the ground, rings showing its way; fists, a hook and a bolas go at
        // what the crosshair meets; a ring of blades goes out all round
        Power::Gale => PowerWay::Line { from: GALE_FROM, step: GALE_STEP, steps: GALE_STEPS },
        Power::Fists => PowerWay::Ray { reach: FISTS_REACH, fan: &[0.0], radius: 1.0 },
        Power::Hook => PowerWay::Ray { reach: HOOK_REACH, fan: &[0.0], radius: 1.0 },
        Power::Bolas => PowerWay::Ray { reach: AIM_RANGE, fan: &[0.0], radius: 1.5 },
        Power::BladeRing => PowerWay::Round { reach: RING_REACH },
        Power::Crescent | Power::Rending | Power::WindCut => PowerWay::Ray { reach: wave(power), fan: &[0.0], radius: 1.2 },
        Power::BladeFan => PowerWay::Ray { reach: wave(power), fan: &FAN, radius: 0.8 },
        Power::Wheel => PowerWay::Ray { reach: WHEEL_REACH, fan: &[0.0], radius: 1.0 },
        Power::Javelin => PowerWay::Ray { reach: JAVELIN_REACH, fan: &[0.0], radius: 1.8 },
        Power::CometArrow | Power::BurstBolt | Power::Ricochet => PowerWay::Ray { reach: AIM_RANGE, fan: &[0.0], radius: 1.5 },
        Power::LanceShot => PowerWay::Ray { reach: LANCE_REACH, fan: &[0.0], radius: 1.5 },
        Power::Breath => PowerWay::Ray { reach: BREATH_REACH, fan: &BREATH_FAN, radius: 1.2 },
        Power::Fissure => PowerWay::Line { from: FISSURE_FROM, step: FISSURE_STEP, steps: FISSURE_STEPS as usize },
        Power::LashWave => PowerWay::Line { from: LASH_FROM, step: LASH_STEP, steps: (wave(power) / LASH_STEP) as usize + 1 },
        Power::Seekers => PowerWay::Seek { reach: SEEKER_REACH },
        Power::Nova => PowerWay::Round { reach: NOVA_REACH },
    }
}

impl Combat {
    /// Where a line from `from` along `way`, `reach` meters long, first meets something: the
    /// ground, a wall, or an animal, which, or where it ends
    pub(super) fn ray_point(arena: &Arena, from: Vector3<f64>, way: Vector3<f64>, reach: f64) -> (Vector3<f64>, Option<usize>) {
        let vpm = arena.view.vpm;
        let far = from + way * reach * vpm;
        let mut t = solid_along(arena.world, from, far).unwrap_or(1.0);
        let mut who = None;
        for (i, animal) in arena.animals.iter().enumerate().filter(|(_, a)| a.health > 0.0 && !a.look.is_pet()) {
            if let Some(hit) = sphere_hit(from, far, animal.center(vpm as u32), animal.look.body().1 * vpm)
                && hit < t
            {
                t = hit;
                who = Some(i);
            }
        }
        (from + (far - from) * t, who)
    }

    /// Where a power aimed along the crosshair goes, each of its fan's: along the crosshair, or
    /// turned either side of it, to what it first meets within its reach, or as far as that
    pub(super) fn power_points(&self, arena: &Arena, power: Power) -> Vec<(Vector3<f64>, Option<usize>)> {
        let view = arena.view;
        let PowerWay::Ray { reach, fan, .. } = power_way(power) else {
            return Vec::new();
        };
        fan.iter().map(|&yaw| Combat::ray_point(arena, view.eye, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw) * view.forward(), reach)).collect()
    }

    /// The ground a power splits or runs along, the way it's aimed: a point every `step` meters
    /// from `from` meters out, `steps` of them, each on the ground there, up a slope or down off a
    /// ledge
    pub(super) fn power_line(arena: &Arena, from: f64, step: f64, steps: usize) -> Vec<Vector3<f64>> {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let way = flat_way(view.forward());
        let start = ground_under(arena.world, feet + way * from * vpm + up * vpm, vpm);
        (0..steps).map(|k| ground_under(arena.world, start + way * step * k as f64 * vpm + up * 1.5 * vpm, vpm)).collect()
    }

    /// What a power's seekers go for: what's within SEEK_ROUND of what the crosshair's on within
    /// `reach`, the nearest that first, by index, five at most; and where the crosshair's on
    pub(super) fn seek_targets(arena: &Arena, reach: f64) -> (Vec<usize>, Vector3<f64>) {
        let view = arena.view;
        let vpm = view.vpm;
        let (at, who) = Combat::ray_point(arena, view.eye, view.forward(), reach);
        let mut near: Vec<(f64, usize)> = (0..arena.animals.len())
            .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
            .map(|i| ((arena.animals[i].center(vpm as u32) - at).norm() / vpm - arena.animals[i].look.body().1, i))
            .filter(|&(d, i)| d < SEEK_ROUND || Some(i) == who)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        (near.into_iter().map(|(_, i)| i).take(5).collect(), at)
    }

    /// Over animal `i`, where a mark shows it's what a power goes for
    pub(super) fn over(arena: &Arena, i: usize) -> Vector3<f64> {
        let vpm = arena.view.vpm;
        let a = &arena.animals[i];
        a.center(vpm as u32) + Vector3::new(0.0, 0.0, (a.look.body().1 + 0.6) * vpm)
    }

    /// What shows where `power` goes this frame, as it's aimed: rings where it meets the ground or
    /// ends, along the ground it runs along, round the player, and marks over what it goes for
    pub(super) fn power_marks(&mut self, arena: &Arena, power: Power) {
        let vpm = arena.view.vpm;
        match power_way(power) {
            PowerWay::Spot => {}
            PowerWay::Ray { radius, .. } => {
                let eye = arena.view.eye;
                let PowerWay::Ray { reach, .. } = power_way(power) else { unreachable!() };
                for (at, who) in self.power_points(arena, power) {
                    match who {
                        Some(i) => self.aimed_at.push(Combat::over(arena, i)),
                        // Not out past all sight, where a shot that flies on and on never comes down
                        None if reach >= AIM_RANGE && (at - eye).norm() > 0.99 * reach * vpm => {}
                        // Ending in the air, as far as it goes: turned to face the eye, so it's seen
                        None if (at - eye).norm() > 0.99 * reach * vpm => self.faced.push((at, radius)),
                        None => self.marks.push((at, radius)),
                    }
                }
            }
            PowerWay::Line { from, step, steps } => {
                for at in Combat::power_line(arena, from, step, steps) {
                    self.marks.push((at, 0.8));
                }
            }
            PowerWay::Seek { reach } => {
                let (targets, at) = Combat::seek_targets(arena, reach);
                if targets.is_empty() {
                    self.marks.push((at, 2.0));
                }
                for i in targets {
                    self.aimed_at.push(Combat::over(arena, i));
                }
            }
            PowerWay::Round { reach } => {
                let view = arena.view;
                let feet = arena.player.unwrap_or(view.eye - Vector3::z() * 1.6 * vpm);
                self.marks.push((feet + Vector3::z() * 0.05 * vpm, reach));
            }
        }
    }

    /// The rings for `power` kept showing at each of `at` for `life` seconds after it's let go,
    /// as what it let go gets there
    pub(super) fn keep_power_marks(&mut self, power: Power, at: &[Vector3<f64>], life: f64) {
        let radius = match power_way(power) {
            PowerWay::Ray { radius, .. } => radius,
            PowerWay::Line { .. } => 0.8,
            PowerWay::Seek { .. } => 2.0,
            PowerWay::Round { reach } => reach,
            PowerWay::Spot => return,
        };
        for &p in at {
            self.spots.push((p, radius, life));
        }
    }
}
