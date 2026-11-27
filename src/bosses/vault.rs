//! Getting out of where it's stuck. A boss, a demi-boss or a monster after the player that's got
//! nowhere for a second or so, its way blocked by the side of a pit, a hole the player dug, a ledge
//! or a wall, or with the player stood up out of its reach above it, bounds out: it crouches, so
//! it's seen coming, and leaps in an arc onto the nearest ground it can stand on towards them,
//! landing heavily. What flies rises up out of it instead, and the biggest bound the furthest.
//! Raiders being led round walls to a door never vault them, see Boss::lure, nor do those that
//! swim, burrow or think for themselves. And what's left standing over nothing, the ground dug or
//! blown out from under it, falls to what's below, see falls().

use super::*;

/// Seconds getting nowhere before it bounds out, a monster sooner
const MIRED: f64 = 1.2;
const MONSTER_MIRED: f64 = 0.9;
/// Meters nearer the player it has to get for that to count as getting somewhere
const PROGRESS: f64 = 0.7;
/// Seconds it lands heavily for, and till it can bound again
const LANDING: f64 = 0.35;
const AGAIN: f64 = 1.6;

/// Seconds getting nowhere before `look` bounds out
fn wait(look: Look) -> f64 {
    if look.is_monster() { MONSTER_MIRED } else { MIRED }
}

/// How high and how far `a` can bound, in meters: a monster a little way, a demi-boss and a boss
/// further, the biggest furthest of all
pub fn reach(a: &Animal) -> (f64, f64) {
    let look = a.look;
    if look.is_monster() && a.demi.is_none() {
        let s = look.scale().max(1.0);
        return ((3.0 * s).min(4.0), (4.5 * s).min(7.0));
    }
    let s = look.size().max(look.scale()).max(a.grown()).max(1.0);
    ((3.6 + 1.6 * s).min(10.0), (6.0 + 2.5 * s).min(16.0))
}

/// Seconds `look` crouches before it bounds: long enough to see coming, but not for what flies,
/// which only rises
pub fn crouch(look: Look) -> f64 {
    if look.flies() {
        0.15
    } else if look.is_monster() {
        0.3
    } else {
        0.42 + 0.06 * look.size().max(1.0).min(4.0)
    }
}

/// Seconds in the air bounding `across` meters and `up` meters up
fn airborne(across: f64, up: f64) -> f64 {
    (0.5 + 0.06 * across + 0.07 * up.max(0.0)).clamp(0.55, 1.35)
}

/// Whether `look` ever bounds out of where it's stuck: not what swims, digs about under the ground
/// as it goes, or thinks for itself
pub fn bounds(look: Look) -> bool {
    !(look.swims() || look.is_kraken() || look.is_great_serpent() || look.is_dragon() || look.is_winged() || look.burrows())
}

/// Ground with room for `look` on it at `x`, `y`, in voxels, between `low` and `high`: the top of
/// the highest solid voxel there with its height clear over it. None if there's nowhere.
fn standing(world: &World, look: Look, x: f64, y: f64, low: f64, high: f64) -> Option<f64> {
    let vpm = world.voxels_per_meter as f64;
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    if xi < 2 || yi < 2 || xi >= world.size[0] as i64 - 2 || yi >= world.size[1] as i64 - 2 {
        return None;
    }
    let room = (look.body().0 * 1.4).clamp(1.2, 5.0) * vpm;
    let step = ((0.25 * vpm) as i64).max(1);
    let clear = |z: i64| (1..=room as i64).step_by(step as usize).all(|k| is_passable(world.material_at(xi, yi, z + k)));
    let top = high.floor() as i64;
    let mut z = top;
    while z >= low.floor() as i64 {
        if !is_passable(world.material_at(xi, yi, z)) {
            return clear(z).then_some(z as f64 + 1.0);
        }
        z -= 1;
    }
    None
}

/// Where `a` could bound to, out of where it's stuck, towards `target`, in voxels: the nearest
/// ground it can stand on, a good deal broad enough, that's up out of the hole it's in or a good
/// way nearer them across what's in its way, with nothing in the way of the arc there
pub fn landing(a: &Animal, world: &World, target: Vector3<f64>) -> Option<Vector3<f64>> {
    let vpm = world.voxels_per_meter as f64;
    let look = a.look;
    let (up, out) = reach(a);
    let from = a.position;
    let now = (target - from).xy().norm() / vpm;
    let toward = heading_to(from, target);
    let wide = (look.body().1 * 0.7).clamp(0.25, 2.0) * vpm;
    let mut best: Option<(f64, Vector3<f64>)> = None;
    let mut r = 1.25f64;
    while r <= out {
        for k in 0..24 {
            let turn = toward + (k as f64 / 24.0) * TAU;
            let (c, s) = (turn.cos(), turn.sin());
            let (x, y) = (from.x + c * r * vpm, from.y + s * r * vpm);
            let Some(z) = standing(world, look, x, y, from.z - 1.5 * vpm, from.z + up * vpm) else {
                continue;
            };
            let rise = (z - from.z) / vpm;
            let spot = Vector3::new(x, y, z);
            let left = (target - spot).xy().norm() / vpm;
            // Up out of the hole, or well across what's in the way towards them, and not further
            // from them than it is, unless it's straight up out of a pit
            let out_of_it = rise > 0.7 && left < now + 0.5 * r;
            let across = ((target.z - z) / vpm).abs() < 1.5 && rise > -1.0 && left < now - 2.0 && left > 0.8;
            if !(out_of_it || across) {
                continue;
            }
            // Broad enough to stand on, not the top of a post
            let broad = [(wide, 0.0), (-wide, 0.0), (0.0, wide), (0.0, -wide)]
                .iter()
                .all(|&(dx, dy)| standing(world, look, x + dx, y + dy, z - 1.0 * vpm, z + 0.6 * vpm).is_some());
            if !broad {
                continue;
            }
            // Nothing over the arc it'd go along
            let middle = look.body().0.clamp(0.5, 2.5) * vpm;
            let blocked = (1..16).any(|i| {
                let p = along(from, spot, i as f64 / 16.0, vpm);
                [0.3 * vpm, middle].iter().any(|&h| !is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, (p.z + h).floor() as i64)))
            });
            if blocked {
                continue;
            }
            // Nearest to them, and up near the height they're at, not too far a bound
            let score = left + 0.35 * r + 0.8 * ((target.z - z) / vpm).abs();
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, spot));
            }
        }
        // The nearest ring with anywhere on it will do, once a little further's been looked at
        if best.is_some() && r > 3.0 {
            break;
        }
        r += 1.0;
    }
    best.map(|(_, spot)| spot)
}

/// How far over the straight line from `from` to `to` the middle of the arc is, in voxels, for a
/// bound clearing the higher end by a meter or so, and seconds in the air
fn arc(from: Vector3<f64>, to: Vector3<f64>, vpm: f64) -> (f64, f64) {
    let across = (to - from).xy().norm() / vpm;
    let up = (to.z - from.z) / vpm;
    let over = from.z.max(to.z) + (0.9 + 0.12 * across) * vpm;
    (airborne(across, up), over - (from.z + to.z) * 0.5)
}

/// Where a bound from `from` to `to` has it `f` of the way along, 0 to 1: bounding up onto
/// something, it's mostly up first and then across onto it, and down off something across first
pub fn along(from: Vector3<f64>, to: Vector3<f64>, f: f64, vpm: f64) -> Vector3<f64> {
    let (_, peak) = arc(from, to, vpm);
    let f = f.clamp(0.0, 1.0);
    let rise = (to.z - from.z) / vpm;
    let g = if rise > 0.5 {
        f.powf(1.7)
    } else if rise < -0.5 {
        1.0 - (1.0 - f).powf(1.7)
    } else {
        f
    };
    let flat = from + (to - from) * g;
    Vector3::new(flat.x, flat.y, from.z + (to.z - from.z) * f + peak * 4.0 * f * (1.0 - f))
}

impl Boss {
    /// Keeping track of whether `a`, after the player at `p` `distance` meters off, is getting
    /// anywhere, and once it's got nowhere a while, where it'll bound to: Some if it should start
    /// crouching to bound there now
    pub(super) fn mired(&mut self, a: &Animal, world: &World, p: Vector3<f64>, distance: f64, dt: f64) -> Option<Vector3<f64>> {
        let look = a.look;
        if !self.note_mired(a, p, distance, true, world.voxels_per_meter as f64, dt) {
            return None;
        }
        if self.mire < wait(look) {
            return None;
        }
        self.mire = 0.0;
        // Looked for once a while, as looking's not cheap
        let spot = landing(a, world, p);
        if spot.is_none() {
            self.cooldowns[Move::Vault as usize] = 0.8;
        }
        spot
    }

    /// How long `a`'s been getting nowhere after the player at `p`, `distance` meters off, counted
    /// on whatever it's doing: with them up out of its reach over it, or, `chasing`, trying to go
    /// and not moving. False if it never bounds just now.
    pub(super) fn note_mired(&mut self, a: &Animal, p: Vector3<f64>, distance: f64, chasing: bool, vpm: f64, dt: f64) -> bool {
        let look = a.look;
        if !bounds(look) || self.buried || self.raider && self.lure.is_some() {
            self.mire = 0.0;
            return false;
        }
        let up = (p.z - a.position.z) / vpm;
        // Higher than it can step up, and not so far off it's no matter
        let above = up > 1.5 + 0.25 * look.size().max(1.0) && distance < 14.0 * look.size().max(1.0);
        let blocked = chasing && a.mood != Mood::Idle && a.speed < 0.3 * look.walk_speed() && distance > closeness(look).0 + 0.5;
        // Getting a good way nearer them counts as getting somewhere
        if self.mire <= 0.0 || distance < self.closest - PROGRESS {
            self.closest = distance;
            self.mire = 0.0;
        }
        if above || blocked {
            self.mire += dt;
        } else if chasing {
            self.mire = (self.mire - dt).max(0.0);
        }
        self.cooldowns[Move::Vault as usize] <= 0.0
    }

    /// Whether it's been getting nowhere long enough it's about to bound out, so it doesn't start
    /// anything else first
    pub(super) fn gathering(&self, look: Look) -> bool {
        self.mire > 0.5 * wait(look)
    }

    /// Seconds getting nowhere before it bounds out
    pub(super) fn wait_for(&self, look: Look) -> f64 {
        wait(look)
    }

    /// Seconds in the air of the bound it's making, from where it set off to where it lands
    pub(super) fn vault_seconds(&self) -> f64 {
        self.vault_len
    }

    /// Set to bound from `from` to `to`, in voxels
    pub(super) fn set_vault(&mut self, from: Vector3<f64>, to: Vector3<f64>, vpm: f64) {
        self.leap = (from, to);
        self.vault_len = arc(from, to, vpm).0;
    }
}

/// Seconds it lands heavily for after bounding, and till it can again
pub fn timing() -> (f64, f64) {
    (LANDING, AGAIN)
}

/// Whether `a` is stood on nothing, the ground dug or blown away from under it, and should fall
/// to what's below: not in the middle of a leap or a bound, nor what flies, swims, burrows or keeps
/// its own body
pub fn falls(a: &Animal, world: &World) -> bool {
    let Some(boss) = &a.boss else {
        return false;
    };
    if !bounds(a.look) || a.look.flies() || boss.buried || a.health <= 0.0 && boss.rig.is_none() {
        return false;
    }
    if matches!(boss.phase, Phase::Act(Move::Leap | Move::Pounce | Move::Vault, _)) {
        return false;
    }
    let below = (a.position.z - 1.0).floor() as i64;
    is_passable(world.material_at(a.position.x.floor() as i64, a.position.y.floor() as i64, below))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::material::DIRT;

    /// Flat ground 6 m up over a 32 m square, at 4 voxels a meter, with a pit 8 m across and 4 m
    /// deep in the middle of it, or none
    fn pitted(pit: bool) -> World {
        let size = [128u32, 128, 64];
        let (tx, ty) = (32, 32);
        let mut occupancy = vec![0u128; tx * ty * 8];
        for z in 0..3 {
            for y in 0..ty {
                for x in 0..tx {
                    let inside = (12..20).contains(&x) && (12..20).contains(&y);
                    if z == 0 || !(pit && inside) {
                        occupancy[x + (y + z * ty) * tx] = u128::MAX;
                    }
                }
            }
        }
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 4;
        world
    }

    /// A boss, a jointed one of each build, a demi-boss's kind and a monster, put down at the bottom
    /// of a pit with the player up on its rim gets out of it within a few seconds: crouching first,
    /// so it's seen coming, then bounding up out of it onto the ground they're on, never into the
    /// side of it
    #[test]
    fn bosses_bound_up_out_of_a_pit() {
        let world = pitted(true);
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let vpm = 4.0;
        let floor = 8.0;
        let rim = 24.0;
        let player = Vector3::new(64.0 + 11.0 * vpm, 64.0, rim);
        for look in [Look::Golem, Look::Ironhide, Look::GraveKnight, Look::Spider, Look::Ogre, Look::SandPharaoh, Look::Ghoul, Look::Wolf, Look::GraveBrute] {
            let mut a = Animal::boss(look, Vector3::new(64.0, 64.0, floor), [7, 8, 9], 3);
            // A brute grown big down in a ravine, as a demi-boss, bounds as a boss does
            if look == Look::GraveBrute {
                a.demi = Some(0);
            }
            if let Some(boss) = &mut a.boss {
                boss.engaged = true;
                boss.phase = Phase::Chase;
            }
            let dt = 1.0 / 30.0;
            let (mut crouched, mut out) = (false, None);
            for f in 0..(9.0 / dt) as usize {
                a.update(&world, &dry, Some(player), dt);
                let boss = a.boss.as_ref().unwrap();
                crouched |= matches!(boss.phase, Phase::WindUp(Move::Vault | Move::Leap, _));
                // Never through the ground on its way out
                let at = a.position;
                assert!(!matches!(boss.phase, Phase::Act(Move::Vault, _)) || is_passable(world.material_at(at.x.floor() as i64, at.y.floor() as i64, (at.z + 0.5 * vpm).floor() as i64)), "{look:?} went into the ground at {at:?}");
                if a.position.z > rim - 1.0 && !matches!(boss.phase, Phase::Act(Move::Vault, _)) {
                    out = Some(f as f64 * dt);
                    break;
                }
            }
            let out = out.unwrap_or_else(|| panic!("{look:?} is still down in the pit at {:?}, {}", a.position, a.boss.as_ref().unwrap().describe()));
            assert!(crouched, "{look:?} got out without crouching to bound or leap");
            assert!(out < 7.0, "{look:?} took {out:.1} s");
            println!("{look:?} out in {out:.2} s");
        }
    }

    /// On level ground, chasing the player round it, it never bounds; and the ground dug out from
    /// under one standing there, it falls to what's below
    #[test]
    fn only_bounds_when_stuck_and_falls_where_the_ground_goes() {
        let mut world = pitted(false);
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let vpm = 4.0;
        let mut a = Animal::boss(Look::Golem, Vector3::new(40.0, 64.0, 24.0), [1, 2, 3], 5);
        a.boss.as_mut().unwrap().engaged = true;
        let dt = 1.0 / 30.0;
        for f in 0..240 {
            let t = f as f64 * dt;
            let player = Vector3::new(64.0 + 30.0 * t.cos(), 64.0 + 30.0 * t.sin(), 24.0);
            a.update(&world, &dry, Some(player), dt);
            assert!(!matches!(a.boss.as_ref().unwrap().phase, Phase::WindUp(Move::Vault, _) | Phase::Act(Move::Vault, _)), "bounded on level ground");
        }
        let at = a.position;
        world.carve([at.x, at.y, at.z - 1.5 * vpm], 2.0 * vpm);
        let mut b = Animal::boss(Look::Golem, at, [1, 2, 4], 5);
        b.boss.as_mut().unwrap().leave_be();
        for _ in 0..60 {
            b.update(&world, &dry, None, dt);
        }
        assert!(b.position.z < at.z - 1.5 * vpm, "still standing on nothing at {:?}", b.position);
    }
}
