//! Aiming at a spot: where a special attack or a power aimed at the ground comes down, and the
//! ring on the ground showing it, as bombs show where they'll land while they're aimed. The ring
//! shows while it's aimed, right click held with it charged, through its move till it's let go,
//! and on while what carries it is on its way, so where it's shown and where it lands are one.
//! Aimed at the sky, or further off than it reaches, it comes down as far off as it reaches that
//! way, along the ground: up slopes, down off ledges, and short of walls in the way.

use super::*;
use crate::weapons::Power;
use super::signatures::powers::power_timing;

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
        if power == Power::Vortex {
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
            if let Action::Power { t, length } = self.action
                && let Some(power) = profile.power
                && t <= power_timing(power).1 * length
                && let Some(spot) = self.power_spot(arena, weapon)
                && let Some(aim) = power_aim(power, profile)
            {
                self.marks.push((spot, aim.radius));
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
