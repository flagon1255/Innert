//! The wyrms: great serpents that bore through the ground rather than slither over it, long and
//! legless, ringed with plates, eyeless, with a round maw of teeth. The Dune Wyrm under the
//! desert's sand and the Caldera Wyrm under the Cinder Crown's ash lie buried till someone comes
//! near, then come up under them, the ground rumbling and a ring showing where first, and break up
//! out of it, the ground blown open and rocks flying. Out in the open each arcs up out of the
//! ground through the air over the player to plunge back into it beyond them, sweeps its tail
//! round low across the ground to be jumped, and spews its element up out of its maw to come down
//! all round the player where it shows. Then it bores back down to come up under them again.
//!
//! Their long bodies are great serpents', see serpents::Coils and serpents::Crest::Maw; what's
//! here is their own moves and how they play out, as Deeds for src\game.rs.

use super::*;

/// Every one of them
pub const LOOKS: [Look; 2] = [Look::DuneWyrm, Look::CalderaWyrm];

/// Whether `look` is one of them
pub fn has(look: Look) -> bool {
    LOOKS.contains(&look)
}

/// Whether `m` is one of their own moves, played out here
pub fn owns(m: Move) -> bool {
    matches!(m, Move::Breach | Move::Thrash | Move::Spout)
}

/// Seconds a wyrm's arc through the air over the player takes, how high over the ground it goes
/// at the least, and how far beyond the player it plunges back in, in meters
const BREACH_SECONDS: f64 = 1.35;
const BREACH_HIGH: f64 = 4.5;
const BREACH_BEYOND: f64 = 7.0;
/// How far round where it plunges back in hurts, in meters, how hard, and how big a hole it blows
const BREACH_RADIUS: f64 = 3.0;
const BREACH_DAMAGE: f64 = 28.0;
const BREACH_CRATER: f64 = 1.3;
/// Seconds its tail takes sweeping round, and how far round it sweeps, in radians
const THRASH_SECONDS: f64 = 0.75;
const THRASH_ARC: f64 = 2.9;
/// How far the other way it draws its tail back first, in radians
const THRASH_COIL: f64 = 0.45;
/// How many globs of its element it spews, and more once it's angry, and how far round the
/// player the rest come down from the first, on them, in meters
const SPOUT_GLOBS: usize = 5;
const SPOUT_RING: f64 = 3.4;
/// How big the hole it breaks up out of the ground is, coming up under the player, in meters
pub const UPHEAVAL_CRATER: f64 = 1.5;

/// Seconds each of their moves takes to get ready for, to make and to get over, and how long till
/// it can be made again
pub fn timing(_look: Look, m: Move) -> (f64, f64, f64, f64) {
    match m {
        // Rearing high over the ground, swaying, before it goes: the time to get out from under
        // where it'll come down
        Move::Breach => (1.05, BREACH_SECONDS, 0.4, 9.0),
        // Slow to draw its tail back in: the moment to get at it
        Move::Thrash => (0.85, THRASH_SECONDS, 1.1, 6.5),
        Move::Spout => (0.9, 0.45, 0.9, 6.0),
        _ => (0.8, 0.5, 0.8, 6.0),
    }
}

/// What a wyrm does in a fight and from how far off its head, in meters: buried, coming up under
/// the player; out in the open, sweeping its tail round at whoever's close and biting, arcing
/// over the player from further off, spewing its element from further still, and boring back down
/// to come up under them once they've got away
pub fn moves(buried: bool) -> &'static [(Move, f64, f64)] {
    if buried {
        &[(Move::Erupt, 0.0, 7.5)]
    } else {
        &[(Move::Thrash, 0.0, 8.5), (Move::Slam, 0.0, 5.5), (Move::Breach, 8.0, 24.0), (Move::Spout, 6.0, 30.0), (Move::Burrow, 11.0, 40.0)]
    }
}

/// How high it rears its head in meters, how wide its maw gapes, 0 to 1, and how far under the
/// ground it's sunk its head, for its own moves, see Boss::great_pose(); `r` its body's thickness
pub fn pose(phase: Phase, r: f64) -> Option<(f64, f64, f64)> {
    Some(match phase {
        // Reared high, swaying, its maw opening; then arcing through the air maw first
        Phase::WindUp(Move::Breach, t) => (3.0 * r + 0.1 * (t * 24.0).sin(), 0.6, 0.0),
        Phase::Act(Move::Breach, _) => (0.4 * r, 1.0, 0.0),
        // Low to the ground as it sweeps its tail
        Phase::WindUp(Move::Thrash, _) | Phase::Act(Move::Thrash, _) => (0.5 * r, 0.45, 0.0),
        // Rearing up with its maw wide, its throat glowing, and spewing
        Phase::WindUp(Move::Spout, _) => (2.6 * r, 0.75, 0.0),
        Phase::Act(Move::Spout, _) => (3.1 * r, 1.0, 0.0),
        _ => return None,
    })
}

/// `n` points round `middle`, in voxels, `r` meters out, starting `turn` radians round
fn ring(middle: Vector3<f64>, n: usize, r: f64, turn: f64, vpm: f64) -> impl Iterator<Item = Vector3<f64>> {
    (0..n).map(move |k| {
        let a = turn + k as f64 / n as f64 * TAU;
        middle + Vector3::new(a.cos(), a.sin(), 0.0) * r * vpm
    })
}

/// Which way round each wyrm turns things, its own
fn own_turn(boss: &Boss) -> f64 {
    ((boss.key[0] ^ boss.key[1].wrapping_mul(11)) as f64 * 0.43).rem_euclid(TAU)
}

/// Where its spew comes down: on the player and round them, more of it once it's angry
fn spout_spots(boss: &Boss, vpm: f64) -> Vec<Vector3<f64>> {
    let n = if boss.enraged { SPOUT_GLOBS + 2 } else { SPOUT_GLOBS };
    std::iter::once(boss.aim).chain(ring(boss.aim, n - 1, SPOUT_RING, own_turn(boss), vpm)).collect()
}

/// Where it'll plunge back into the ground arcing over the player at `at` from `from`: beyond
/// them, as far again as BREACH_BEYOND, on the ground there if there's any
fn breach_end(world: &World, from: Vector3<f64>, at: Vector3<f64>, forward: Vector3<f64>, vpm: f64) -> Vector3<f64> {
    let way = (at - from).xy().try_normalize(1e-6).unwrap_or_else(|| forward.xy());
    let beyond = at + Vector3::new(way.x, way.y, 0.0) * BREACH_BEYOND * vpm;
    ground_under(world, beyond + Vector3::new(0.0, 0.0, 2.0 * vpm)).or_else(|| ground_under(world, at)).unwrap_or(at)
}

/// The tip of its tail's bearing from where it sweeps round, radians round from east, and how far
/// out it is, in meters
fn tail_bearing(coils: &serpents::Coils, vpm: f64) -> (f64, f64) {
    let (pivot, reach) = coils.sweep_reach(vpm);
    let tip = coils.points[coils.points.len() - 1] - pivot;
    (tip.y.atan2(tip.x), reach / vpm)
}

/// Shown on the ground as wyrm `a` gets ready to make move `m`, `t` seconds into getting ready of
/// `whole`: where it'll come down, a ring closing in on each place
pub fn winding(m: Move, t: f64, whole: f64, a: &Animal, boss: &mut Boss, world: &World, forward: Vector3<f64>, vpm: f64) {
    let closing = (1.0 - t / whole.max(0.1)).clamp(0.0, 1.0);
    let f = 1.0 - closing;
    match m {
        // Where it'll plunge back in, beyond the player
        Move::Breach => {
            let to = breach_end(world, a.position, boss.aim, forward, vpm);
            boss.deeds.push(Deed::Mark { at: to, radius: BREACH_RADIUS, closing });
        }
        // Its tail drawn back the other way, the ring it'll sweep round shown round it
        Move::Thrash => {
            let sign = if boss.ring.2 >= 0.0 { 1.0 } else { -1.0 };
            if let Some(coils) = &mut boss.coils {
                if t <= 0.0 {
                    let (pivot, _) = coils.sweep_reach(vpm);
                    let off = (boss.aim - pivot).xy();
                    let (bearing, _) = tail_bearing(coils, vpm);
                    // Round towards the side the player's on
                    let way = ((off.y.atan2(off.x) - bearing + PI).rem_euclid(TAU) - PI).signum();
                    boss.ring = (pivot, bearing, if way == 0.0 { 1.0 } else { way });
                }
                coils.sweep = Some(-sign * THRASH_COIL * rig_ease(f));
                let (pivot, reach) = coils.sweep_reach(vpm);
                boss.deeds.push(Deed::Mark { at: pivot, radius: reach / vpm + 0.6, closing });
            }
        }
        Move::Spout => {
            for at in spout_spots(boss, vpm) {
                boss.deeds.push(Deed::Mark { at, radius: 1.7, closing });
            }
        }
        _ => {}
    }
}

/// From 0 to 1 as `f` does, easing in and out
fn rig_ease(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    f * f * (3.0 - 2.0 * f)
}

/// Wyrm `a` making move `m`, now it's ready
pub fn begin(m: Move, a: &mut Animal, boss: &mut Boss, world: &World, forward: Vector3<f64>, vpm: f64) {
    match m {
        // Up off the ground arcing through the air, its body following, over where the player stood
        // to where it showed
        Move::Breach => {
            let to = breach_end(world, a.position, boss.aim, forward, vpm);
            boss.leap = (a.position, to);
            a.heading = heading_to(a.position, to);
            a.target_heading = a.heading;
            if let Some(coils) = &mut boss.coils {
                coils.soar = true;
            }
            boss.deeds.push(Deed::Erupt { at: a.position });
            boss.deeds.push(Deed::Stomp(a.position));
        }
        Move::Thrash => {
            boss.trail = 0.0;
            boss.deeds.push(Deed::Swipe(a.position, 2));
            boss.deeds.push(Deed::Stomp(a.position));
        }
        // Its element spewed up out of its maw to come down where it showed, the first on the
        // player and the rest one after another round them, bursting up where it lands
        Move::Spout => {
            let from = spout(a, boss, vpm);
            for (k, to) in spout_spots(boss, vpm).into_iter().enumerate() {
                let time = 0.8 + 0.06 * k as f64;
                boss.deeds.push(Deed::Lob { from, to: to + Vector3::new(0.0, 0.0, 0.3 * vpm), time, damage: 16.0, radius: 1.7, rock: false });
                boss.deeds.push(Deed::Flare { at: to + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: time + 0.05, radius: 1.5, damage: 0.0 });
            }
        }
        _ => {}
    }
}

/// Wyrm `a` `t` seconds into making move `m`, in a frame of `dt` seconds
pub fn during(m: Move, t: f64, dt: f64, a: &mut Animal, boss: &mut Boss, player: Option<Vector3<f64>>, vpm: f64) {
    let crossed = |at: f64| t >= at && t - dt < at;
    match m {
        // Through the air in an arc, its maw first, plunging back into the ground where it showed,
        // breaking it open; under it after, to come up under the player again
        Move::Breach => {
            let (from, to) = boss.leap;
            let f = (t / BREACH_SECONDS).min(1.0);
            let high = (BREACH_HIGH + 0.12 * (to - from).xy().norm() / vpm) * vpm;
            a.position = from.lerp(&to, f) + Vector3::new(0.0, 0.0, high * (PI * f).sin());
            a.speed = 0.0;
            boss.deeds.push(Deed::Mark { at: to, radius: BREACH_RADIUS, closing: 1.0 - f });
            if t >= BREACH_SECONDS {
                a.position = to;
                boss.deeds.push(Deed::Slam { at: to, radius: BREACH_RADIUS, damage: BREACH_DAMAGE, crater: BREACH_CRATER });
                boss.deeds.push(Deed::Erupt { at: to });
                boss.deeds.push(Deed::Stomp(to));
                boss.buried = true;
                if let Some(coils) = &mut boss.coils {
                    coils.soar = false;
                }
            }
        }
        // Its tail sweeping round across the ground, hitting whoever's within its reach and not
        // jumping as it passes them, the ground churned up under its tip
        Move::Thrash => {
            let (middle, from, sign) = boss.ring;
            let swing = |t: f64| sign * (-THRASH_COIL + (THRASH_ARC + THRASH_COIL) * rig_ease(t / THRASH_SECONDS));
            let Some(coils) = &mut boss.coils else {
                return;
            };
            coils.sweep = Some(swing(t));
            let (_, reach) = coils.sweep_reach(vpm);
            let tip = coils.body().last().unwrap_or(middle);
            if let Some(p) = player {
                let bearing = heading_to(middle, p);
                let passes = |t: f64| ((from + swing(t) - bearing) / TAU).floor();
                if t - dt < THRASH_SECONDS && passes(t) != passes((t - dt).max(0.0)) {
                    boss.deeds.push(Deed::Quake { at: middle, from: 0.6, to: reach / vpm + 1.0, dt });
                }
            }
            boss.trail -= dt;
            if boss.trail <= 0.0 {
                boss.trail = 0.06;
                boss.deeds.push(Deed::Burrow { at: tip });
            }
            if crossed(THRASH_SECONDS * 0.5) {
                boss.deeds.push(Deed::Swipe(tip, 1));
            }
        }
        _ => {}
    }
}

/// A wyrm come up under the player at `at`: the ground blown open, see UPHEAVAL_CRATER, its element
/// bursting up there and rocks thrown out all round, to come down where they show
pub fn erupted(a: &Animal, boss: &mut Boss, at: Vector3<f64>, vpm: f64) {
    let from = a.position + Vector3::new(0.0, 0.0, 1.5 * vpm);
    boss.deeds.push(Deed::Flare { at: at + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.0, radius: 2.2, damage: 0.0 });
    let n = if boss.enraged { 7 } else { 5 };
    for (k, to) in ring(at, n, 5.0, own_turn(boss) + a.heading, vpm).enumerate() {
        boss.deeds.push(Deed::Lob { from, to: to + Vector3::new(0.0, 0.0, 0.5 * vpm), time: 0.75 + 0.05 * k as f64, damage: 12.0, radius: 1.3, rock: true });
    }
}

/// A wyrm's tail eased back once it's swept it, and let go of after, and nothing left of an arc
/// through the air it's not making: at the end of each time it thinks
pub fn settle(phase: Phase, boss: &mut Boss, dt: f64) {
    let Some(coils) = &mut boss.coils else {
        return;
    };
    let thrashing = matches!(phase, Phase::WindUp(Move::Thrash, _) | Phase::Act(Move::Thrash, _));
    let breaching = matches!(phase, Phase::Act(Move::Breach, _));
    if !breaching {
        coils.soar = false;
    }
    if !thrashing && let Some(angle) = coils.sweep {
        let eased = angle * (-4.0 * dt).exp();
        coils.sweep = (eased.abs() > 0.02).then_some(eased);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat field of dirt 192 m across at 8 voxels a meter, its ground 4 m up
    fn field() -> World {
        let size = [1536u32, 1536, 128];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::SAND);
        world.voxels_per_meter = 8;
        world
    }

    /// Lying in wait it's buried, and coming up under the player it breaks the ground open where it
    /// showed the ring, rocks flying out round it, and its body comes up out of the hole after its
    /// head
    #[test]
    fn it_comes_up_under_the_player_breaking_the_ground() {
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        for look in LOOKS {
            let home = Vector3::new(96.0, 96.0, 4.0) * vpm;
            let mut wyrm = Animal::boss(look, home, [5, 9, -1], 3);
            assert!(wyrm.boss.as_ref().unwrap().buried(), "{look:?} lies in wait buried");
            let player = Some(home + Vector3::new(9.0, 3.0, 0.0) * vpm);
            let (mut ring, mut crater, mut rocks) = (None, None, 0);
            for _ in 0..360 {
                wyrm.update(&world, &water, player, 1.0 / 30.0);
                for d in wyrm.boss.as_mut().unwrap().deeds.drain(..) {
                    match d {
                        Deed::Mark { at, .. } if crater.is_none() => ring = Some(at),
                        Deed::Slam { at, crater: c, .. } if c >= UPHEAVAL_CRATER => crater = crater.or(Some(at)),
                        Deed::Lob { rock: true, .. } => rocks += 1,
                        _ => {}
                    }
                }
                if crater.is_some() && rocks > 0 {
                    break;
                }
            }
            let (ring, crater) = (ring.expect("a ring shows first"), crater.expect("it breaks the ground"));
            assert!((ring - crater).xy().norm() < 0.5 * vpm, "{look:?} came up {:.1} m from its ring", (ring - crater).xy().norm() / vpm);
            assert!((crater - player.unwrap()).xy().norm() < 2.0 * vpm, "{look:?} came up away from the player");
            assert!(rocks >= 5, "{look:?} threw {rocks} rocks");
        }
    }

    /// Arcing over the player its head goes high over them, its body follows it up through the air
    /// rather than lying along the ground, and it plunges back in beyond them, buried again
    #[test]
    fn it_arcs_over_the_player_and_plunges_back_in() {
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let home = Vector3::new(96.0, 96.0, 4.0) * vpm;
        let mut wyrm = Animal::boss(Look::DuneWyrm, home, [5, 9, -1], 3);
        let boss = wyrm.boss.as_mut().unwrap();
        boss.buried = false;
        boss.provoke();
        boss.forced = Some(Move::Breach);
        let player = Some(home + Vector3::new(14.0, 0.0, 0.0) * vpm);
        let (mut highest, mut body_high, mut landed) = (0.0f64, 0.0f64, None);
        for _ in 0..400 {
            wyrm.update(&world, &water, player, 1.0 / 30.0);
            let coils = wyrm.coils().unwrap();
            highest = highest.max(coils.head.z / vpm - 4.0);
            body_high = body_high.max(coils.points[3].z / vpm - 4.0);
            for d in wyrm.boss.as_mut().unwrap().deeds.drain(..) {
                if let Deed::Slam { at, crater, .. } = d
                    && crater >= BREACH_CRATER
                {
                    landed = landed.or(Some(at));
                }
            }
            if landed.is_some() {
                break;
            }
        }
        let landed = landed.expect("it came down");
        assert!(highest > BREACH_HIGH, "its head went only {highest:.1} m up");
        assert!(body_high > 2.5, "its body stayed down, {body_high:.1} m up");
        assert!(landed.x > player.unwrap().x + 4.0 * vpm, "it came down short of beyond the player");
        assert!(wyrm.boss.as_ref().unwrap().buried(), "it's back under the ground");
    }

    /// Sweeping its tail round, the player standing in its way is struck by it once as it passes,
    /// unless they jump it
    #[test]
    fn its_tail_sweeps_round_through_the_player() {
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let home = Vector3::new(96.0, 96.0, 4.0) * vpm;
        let mut wyrm = Animal::boss(Look::CalderaWyrm, home, [5, 9, -1], 3);
        let boss = wyrm.boss.as_mut().unwrap();
        boss.buried = false;
        boss.provoke();
        boss.forced = Some(Move::Thrash);
        // Laid out straight first, then the player a little way off to its side
        for _ in 0..30 {
            wyrm.update(&world, &water, None, 1.0 / 30.0);
            wyrm.boss.as_mut().unwrap().deeds.clear();
        }
        let player = Some(home + Vector3::new(-3.0, 6.0, 0.0) * vpm);
        let (mut quakes, mut swept) = (0, 0.0f64);
        for _ in 0..240 {
            wyrm.update(&world, &water, player, 1.0 / 30.0);
            swept = swept.max(wyrm.coils().unwrap().sweep.map_or(0.0, f64::abs));
            quakes += wyrm.boss.as_mut().unwrap().deeds.drain(..).filter(|d| matches!(d, Deed::Quake { .. })).count();
        }
        assert!(swept > 2.0, "its tail swept only {swept:.2} rad round");
        assert!(quakes >= 1, "its tail never passed the player");
    }
}
