//! Four more bosses, each fighting a way none before them did, for where there were fewest. The
//! Winterglass Duelist, a swordsman of the snowbound castles in armour rimed white, fast on its feet:
//! it sidesteps a blow and cuts back, lunges the length of a hall, and sends crescents of frost off
//! its blade. The Hive Matriarch, an insect queen of the jungle's caverns and colonnades, its
//! abdomen swollen with brood: it calls them up, looses a swarm of venomous motes that spirals in
//! on the player, and rains its stingers down. The Gloamdelver, a great blind digger of the caves,
//! crystals grown out of its back: it burrows to come up under the player, flings a spray of
//! crystal shards out of the ground and sends a line of them bursting up at them. And the Fenhulk,
//! huge and slow, a hulk of peat and reeds out of the flooded places, the ground shaking under every
//! step: it stamps so the bog heaves in waves round it, and spews mud and venom over the player.
//!
//! Each is drawn jointed, see src\rig\wardens.rs. What's here is what each is and its own moves, as
//! Deeds for src\game.rs; src\bosses\roster.rs asks here about them first.

use super::*;

/// Every one of them, in the order they're added among the looks
pub const LOOKS: [Look; 4] = [Look::WinterglassDuelist, Look::HiveMatriarch, Look::Gloamdelver, Look::Fenhulk];

/// Whether `look` is one of them
pub fn has(look: Look) -> bool {
    LOOKS.contains(&look)
}

/// Whether `m` is one of their own moves, played out here
pub fn owns(m: Move) -> bool {
    matches!(m, Move::Lunge | Move::Riposte | Move::Crescent | Move::Swarm | Move::Stingrain | Move::Rockspray | Move::Undermine | Move::Mirestomp | Move::Bogspew)
}

/// How much bigger its model is than its build's
pub fn scale(look: Look) -> f64 {
    match look {
        Look::WinterglassDuelist => 1.25,
        Look::HiveMatriarch => 1.6,
        Look::Gloamdelver => 1.55,
        _ => 2.3,
    }
}

/// How much damage it takes: the duelist as the casters, quick as it is; the queen, the digger and
/// the hulk as the heavy ones, see levels::BOSS_HEALTH
pub fn health(look: Look) -> f64 {
    match look {
        Look::WinterglassDuelist => 2500.0,
        _ => 3000.0,
    }
}

/// How fast it walks and runs, in meters a second
pub fn speeds(look: Look) -> (f64, f64) {
    match look {
        // Quicker on its feet than any of them
        Look::WinterglassDuelist => (2.4, 7.2),
        Look::HiveMatriarch => (1.3, 3.4),
        Look::Gloamdelver => (1.5, 4.2),
        // Slow as the bog it came out of
        _ => (0.9, 1.9),
    }
}

/// Meters covered by one cycle of its walk, at its build's size
pub fn stride(look: Look) -> f64 {
    match look {
        Look::WinterglassDuelist => 1.7,
        Look::HiveMatriarch => 1.3,
        Look::Gloamdelver => 1.3,
        _ => 1.6,
    }
}

/// Radius of a sphere round the whole of it, however it's posed, in meters
pub fn reach(look: Look) -> f64 {
    look.scale()
        * match look {
            Look::WinterglassDuelist => 3.1,
            Look::HiveMatriarch => 2.9,
            Look::Gloamdelver => 2.6,
            _ => 3.2,
        }
}

/// The body to aim at: how high its middle is, and how big it is, in meters
pub fn body(look: Look) -> (f64, f64) {
    let s = look.scale();
    let (high, round) = match look {
        Look::WinterglassDuelist => (1.15, 0.36),
        Look::HiveMatriarch => (1.15, 0.6),
        Look::Gloamdelver => (1.0, 0.62),
        _ => (1.45, 0.75),
    };
    (high * s, round * s)
}

pub fn name(look: Look) -> &'static str {
    match look {
        Look::WinterglassDuelist => "Winterglass Duelist",
        Look::HiveMatriarch => "Hive Matriarch",
        Look::Gloamdelver => "Gloamdelver",
        _ => "Fenhulk",
    }
}

/// What its attacks are made of: frost off the duelist's blade, the queen's venom and the bog's,
/// and the glow of the digger's crystals
pub fn element(look: Look) -> Option<Element> {
    Some(match look {
        Look::WinterglassDuelist => Element::Frost,
        Look::Gloamdelver => Element::Arcane,
        _ => Element::Poison,
    })
}

/// What it's made of: its body, darker parts, things on it, what glows, and one more
pub(crate) fn skin(look: Look) -> Skin {
    let make = |body, dark, accent, glow, extra| Skin { body, dark, accent, glow, extra };
    match look {
        // Steel rimed white, darker steel, a cloak of white fur, frost in its visor, a blade of ice
        Look::WinterglassDuelist => make(STEEL, DARK_STEEL, FUR_WHITE, GLOW_FROST, ICE),
        // Black chitin, darker between its plates, gold bands, venom glowing in its brood, crystal
        // wings
        Look::HiveMatriarch => make(OBSIDIAN, BARK_DARK, GOLD, GLOW_POISON, CRYSTAL),
        // Grey fur, rock in its hide, crystals out of its back glowing, the pale bone of its claws
        Look::Gloamdelver => make(FUR_GRAY, ROCK, CRYSTAL, GLOW_ARCANE, BONE),
        // Peat, darker mud, moss over it, the bog's venom glowing through, reeds growing out of it
        _ => make(DIRT, CLAY, MOSS, GLOW_POISON, REED),
    }
}

/// The kinds of weapon it leaves most of the time, as weapon families' ids, see bosses::taste()
pub fn taste(look: Look) -> &'static [&'static str] {
    match look {
        Look::WinterglassDuelist => &["sword", "katana", "dagger", "warfan"],
        Look::HiveMatriarch => &["katar", "whip", "totem", "wand"],
        Look::Gloamdelver => &["hammer", "orb", "katar", "maul"],
        _ => &["maul", "totem", "staff", "flail"],
    }
}

/// What its hoard's called, as it spills out of it
pub fn hoard(look: Look) -> &'static str {
    match look {
        Look::WinterglassDuelist => "winterglass cache",
        Look::HiveMatriarch => "royal comb",
        Look::Gloamdelver => "geode-hoard",
        _ => "bog-hoard",
    }
}

/// A blow at what's in front: a cut of the duelist's blade, the queen's scythed forelimbs, the
/// digger's claws, the hulk's fists; how far ahead and round in meters, how hard, how big a hole
pub fn slam(look: Look) -> (f64, f64, f64, f64) {
    match look {
        Look::WinterglassDuelist => (2.0, 1.9, 22.0, 0.0),
        Look::HiveMatriarch => (2.1, 2.3, 24.0, 0.0),
        Look::Gloamdelver => (2.0, 2.4, 26.0, 0.8),
        _ => (3.0, 3.4, 34.0, 1.0),
    }
}

/// The moves each makes in a fight and from how far off, its own first; the digger, dug in, only
/// comes up under the player
pub fn menu(look: Look, buried: bool) -> &'static [(Move, f64, f64)] {
    match look {
        // Lunging in from a few strides off, crescents from further, cutting close to; its
        // riposte's as it's struck, see dodges()
        Look::WinterglassDuelist => &[(Move::Lunge, 4.0, 12.0), (Move::Crescent, 5.0, 24.0), (Move::Slam, 0.0, 3.0), (Move::Spit, 3.0, 26.0)],
        // Its brood, its swarm from a little way off, its stingers from further, its scythes close
        Look::HiveMatriarch => &[(Move::Summon, 0.0, 24.0), (Move::Swarm, 3.0, 18.0), (Move::Stingrain, 5.0, 26.0), (Move::Slam, 0.0, 3.4)],
        Look::Gloamdelver if buried => &[(Move::Erupt, 0.0, 3.0)],
        // Its crystals bursting up in a line at whoever's a little way off, shards flung at whoever's
        // further, digging in to come up under whoever's further still, its claws close to
        Look::Gloamdelver => &[(Move::Undermine, 3.5, 14.0), (Move::Rockspray, 5.0, 22.0), (Move::Burrow, 9.0, 40.0), (Move::Slam, 0.0, 3.4)],
        // The bog heaving round it for whoever's near, mud spewed over whoever's further, its fists
        // close to and a clod of peat hurled from afar
        _ => &[(Move::Mirestomp, 0.0, 7.5), (Move::Bogspew, 4.0, 20.0), (Move::Slam, 0.0, 3.6), (Move::Throw, 6.0, 30.0)],
    }
}

/// The move it makes as it's struck, if it's one that sidesteps the blow and cuts back
pub fn dodges(look: Look) -> Option<Move> {
    (look == Look::WinterglassDuelist).then_some(Move::Riposte)
}

/// Seconds the duelist's lunge takes, how far it goes at the most, in meters, and how near the
/// player it has to pass to run them through
const LUNGE_SECONDS: f64 = 0.34;
const LUNGE_REACH: f64 = 10.0;
const LUNGE_HITS: f64 = 1.5;
/// Seconds into its riposte it's sidestepped, how far, and when it cuts back
const RIPOSTE_STEP: f64 = 0.2;
const RIPOSTE_SIDE: f64 = 3.0;
const RIPOSTE_CUT: f64 = 0.3;
/// Seconds into its crescent it sends each off its blade
const CRESCENTS: [f64; 3] = [0.0, 0.2, 0.4];
/// Seconds the Matriarch's swarm pours out for, between each mote, and how far round the player
/// it starts spiralling in from and ends, in meters
const SWARM_SECONDS: f64 = 1.8;
const SWARM_EVERY: f64 = 0.07;
const SWARM_FROM: f64 = 5.0;
const SWARM_TO: f64 = 0.6;
/// How far round the player the Matriarch's stingers come down, and how far round each hurts
const STING_RING: f64 = 3.0;
const STINGER_RADIUS: f64 = 1.4;
/// The Gloamdelver's shards: how many, more once it's angry, and how far apart across the player
const SHARDS: [usize; 2] = [5, 7];
const SHARD_SPREAD: f64 = 2.4;
/// Its line of crystals: seconds between each bursting up, how far apart, and how many
const UNDERMINE_EVERY: f64 = 0.09;
const UNDERMINE_SPACING: f64 = 1.5;
const UNDERMINE_SPIKES: usize = 9;
/// How far the Fenhulk's stamp heaves the bog, in meters, and how far out its rings of mud burst
const MIRE_REACH: f64 = 9.0;
const MIRE_RINGS: [f64; 3] = [3.0, 5.5, 8.0];
/// How far round the player its spew comes down
const SPEW_RING: f64 = 2.8;

/// Seconds each of their moves takes to get ready for, to make and to get over, and how long till
/// it can be made again
pub fn timing(_look: Look, m: Move) -> (f64, f64, f64, f64) {
    match m {
        // Drawn back and crouched, long enough to see where it'll go
        Move::Lunge => (0.7, LUNGE_SECONDS, 0.7, 5.5),
        // Hardly any warning, but open a moment after it's cut
        Move::Riposte => (0.1, RIPOSTE_CUT + 0.25, 0.6, 4.5),
        Move::Crescent => (0.6, CRESCENTS[2] + 0.2, 0.6, 5.0),
        Move::Swarm => (0.9, SWARM_SECONDS, 0.9, 9.0),
        Move::Stingrain => (1.0, 0.45, 0.8, 7.0),
        Move::Rockspray => (0.8, 0.45, 0.8, 5.5),
        // Its claws stuck in the ground after: the moment to get in close
        Move::Undermine => (1.0, UNDERMINE_EVERY * UNDERMINE_SPIKES as f64 + 0.1, 1.3, 8.0),
        // Slow to set its foot back down and get its balance
        Move::Mirestomp => (1.3, MIRE_REACH / QUAKE_PACE, 1.4, 9.0),
        Move::Bogspew => (0.95, 0.7, 0.9, 7.0),
        _ => (0.8, 0.5, 0.8, 6.0),
    }
}

/// Where the Matriarch's stingers come down: on the player and in a ring round them, more of them
/// once it's angry
fn sting_spots(boss: &Boss, vpm: f64) -> Vec<Vector3<f64>> {
    let n = if boss.enraged { 8 } else { 6 };
    std::iter::once(boss.aim).chain(ring(boss.aim, n, STING_RING, own_turn(boss), vpm)).collect()
}

/// Where the Fenhulk's spew comes down
fn spew_spots(boss: &Boss, vpm: f64) -> Vec<Vector3<f64>> {
    let n = if boss.enraged { 7 } else { 5 };
    std::iter::once(boss.aim).chain(ring(boss.aim, n, SPEW_RING, own_turn(boss) + 0.7, vpm)).collect()
}

/// The way the duelist's lunge goes, from where it stands at the player at `to`, and how far, in
/// meters: just past them, no further than it reaches
fn lunge_line(a: &Animal, to: Vector3<f64>, vpm: f64) -> (Vector3<f64>, f64) {
    let off = (to - a.position).xy();
    let way = off.try_normalize(1e-6).unwrap_or_else(|| Vector2::new(a.heading.cos(), a.heading.sin()));
    (Vector3::new(way.x, way.y, 0.0), (off.norm() / vpm + 1.5).min(LUNGE_REACH))
}

/// Where the Gloamdelver's `k`th crystal of its line bursts up, along the way to the player
fn undermine_spot(a: &Animal, boss: &Boss, k: usize, vpm: f64) -> Vector3<f64> {
    let ahead = (boss.aim - a.position).xy().try_normalize(1e-6).unwrap_or_else(|| Vector2::new(a.heading.cos(), a.heading.sin()));
    let d = 1.6 * a.look.scale() + UNDERMINE_SPACING * k as f64;
    a.position + Vector3::new(ahead.x, ahead.y, 0.0) * d * vpm
}

/// Shown on the ground as boss `a` gets ready to make move `m`, `t` seconds into getting ready of
/// `whole`: where it'll come down, a ring closing in on each place
pub fn winding(m: Move, t: f64, whole: f64, a: &Animal, boss: &mut Boss, _forward: Vector3<f64>, vpm: f64) {
    let closing = (1.0 - t / whole.max(0.1)).clamp(0.0, 1.0);
    match m {
        // The line it'll lunge along
        Move::Lunge => {
            let (way, long) = lunge_line(a, boss.aim, vpm);
            for k in 0..4 {
                let at = a.position + way * (1.5 + (long - 1.5) * k as f64 / 3.0) * vpm;
                boss.deeds.push(Deed::Mark { at, radius: LUNGE_HITS, closing });
            }
        }
        Move::Stingrain => {
            for at in sting_spots(boss, vpm) {
                boss.deeds.push(Deed::Mark { at, radius: STINGER_RADIUS, closing });
            }
        }
        Move::Swarm => boss.deeds.push(Deed::Mark { at: boss.aim, radius: SWARM_FROM, closing: 0.0 }),
        Move::Undermine => {
            for k in (0..UNDERMINE_SPIKES).step_by(2) {
                boss.deeds.push(Deed::Mark { at: undermine_spot(a, boss, k, vpm), radius: 1.2, closing });
            }
        }
        Move::Mirestomp => {
            boss.deeds.push(Deed::Mark { at: a.position, radius: MIRE_REACH, closing });
            // The bog stirring round its foot as it's raised
            boss.trail -= 1.0 / 60.0;
            if boss.trail <= 0.0 {
                boss.trail = 0.15;
                let turn = (boss.clock * 7.3).sin() * PI + boss.clock * 2.1;
                let r = (1.5 + 3.0 * (0.5 + 0.5 * (boss.clock * 5.1).cos())) * vpm;
                boss.deeds.push(Deed::Burrow { at: a.position + Vector3::new(turn.cos() * r, turn.sin() * r, 0.0) });
            }
        }
        Move::Bogspew => {
            for at in spew_spots(boss, vpm) {
                boss.deeds.push(Deed::Mark { at, radius: 1.6, closing });
            }
        }
        _ => {}
    }
}

/// Boss `a` making move `m`, now it's ready
pub fn begin(m: Move, a: &mut Animal, boss: &mut Boss, player: Option<Vector3<f64>>, forward: Vector3<f64>, vpm: f64) {
    let look = a.look;
    let s = look.scale();
    match m {
        // Pushing off, the line it'll go along fixed
        Move::Lunge => {
            let (way, long) = lunge_line(a, boss.aim, vpm);
            a.heading = way.y.atan2(way.x);
            a.target_heading = a.heading;
            boss.leap = (a.position, a.position + way * long * vpm);
            boss.rammed = false;
            boss.deeds.push(Deed::Stomp(a.position));
            boss.deeds.push(Deed::Swipe(a.position, 0));
        }
        // Stepping aside from the blow, to the side it chose, away from where it was struck from
        Move::Riposte => {
            let toward = player.map_or(forward, |p| (p - a.position).try_normalize(1e-6).unwrap_or(forward));
            let side = if a.roll() < 0.5 { 1.0 } else { -1.0 };
            let across = Vector3::new(-toward.y * side, toward.x * side, 0.0);
            boss.leap = (a.position, a.position + across * RIPOSTE_SIDE * vpm);
            boss.deeds.push(Deed::Swipe(a.position, 0));
        }
        Move::Stingrain => {
            let from = emit(a, boss, vpm);
            for (k, to) in sting_spots(boss, vpm).into_iter().enumerate() {
                boss.deeds.push(Deed::Lob { from, to, time: 0.9 + 0.06 * k as f64, damage: 16.0, radius: STINGER_RADIUS, rock: true });
            }
            boss.deeds.push(Deed::Swipe(a.position, 1));
        }
        Move::Swarm => {
            boss.trail = 0.0;
            boss.deeds.push(Deed::Roar { at: a.position, angry: boss.enraged });
        }
        // Its claws torn up out of the ground, crystal shards flung out of it fanned across the player
        Move::Rockspray => {
            let from = a.position + forward * 1.4 * s * vpm + Vector3::new(0.0, 0.0, 0.6 * vpm);
            let across = Vector3::new(-forward.y, forward.x, 0.0);
            let n = SHARDS[boss.enraged as usize];
            for k in 0..n {
                let off = k as f64 - (n - 1) as f64 * 0.5;
                let to = boss.aim + across * off * SHARD_SPREAD * vpm;
                boss.deeds.push(Deed::Lob { from: from + across * off * 0.3 * vpm, to, time: 0.65 + 0.04 * off.abs(), damage: 14.0, radius: 1.3, rock: true });
            }
            boss.deeds.push(Deed::Burrow { at: from });
            boss.deeds.push(Deed::Slam { at: from, radius: 0.0, damage: 0.0, crater: 0.0 });
        }
        Move::Undermine => {
            boss.trail = 0.0;
            boss.deeds.push(Deed::Slam { at: a.position + forward * 1.4 * s * vpm, radius: 1.6, damage: 18.0, crater: 0.0 });
            boss.deeds.push(Deed::Stomp(a.position));
        }
        // Its foot brought down, the bog heaving out from under it
        Move::Mirestomp => {
            boss.leap.0 = a.position;
            boss.deeds.push(Deed::Stomp(a.position));
            boss.deeds.push(Deed::Slam { at: a.position + forward * 1.2 * s * vpm, radius: 2.4, damage: 22.0, crater: 0.0 });
            for (k, r) in MIRE_RINGS.into_iter().enumerate() {
                let n = 6 + 3 * k;
                for at in ring(a.position + Vector3::new(0.0, 0.0, 1.0 * vpm), n, r, own_turn(boss) + k as f64 * 0.5, vpm) {
                    boss.deeds.push(Deed::Flare { at, delay: r / QUAKE_PACE + 0.05, radius: 1.2, damage: 14.0 });
                }
            }
        }
        Move::Bogspew => {
            let from = emit(a, boss, vpm);
            for (k, to) in spew_spots(boss, vpm).into_iter().enumerate() {
                boss.deeds.push(Deed::Lob { from, to, time: 0.95 + 0.08 * k as f64, damage: 15.0, radius: 1.6, rock: false });
            }
        }
        _ => {}
    }
}

/// Boss `a` `t` seconds into making move `m`, in a frame of `dt` seconds
#[allow(clippy::too_many_arguments)]
pub fn during(m: Move, t: f64, dt: f64, a: &mut Animal, boss: &mut Boss, player: Option<Vector3<f64>>, world: &World, vpm: f64) {
    let look = a.look;
    let crossed = |at: f64| t >= at && t - dt < at;
    // Going along the ground from where it was towards `to`, `f` of the way, so long as there's
    // ground to go on; stopping where there isn't
    let slide = |a: &mut Animal, from: Vector3<f64>, to: Vector3<f64>, f: f64| {
        let want = from.lerp(&to, f.clamp(0.0, 1.0)) + Vector3::new(0.0, 0.0, 1.0 * vpm);
        match ground_under(world, want) {
            Some(at) if (at.z - a.position.z).abs() < 1.2 * vpm => a.position = at,
            _ => {}
        }
    };
    match m {
        // Across the ground in a blur, frost left in its wake, running through whoever it passes
        Move::Lunge => {
            let (from, to) = boss.leap;
            let f = t / LUNGE_SECONDS;
            slide(a, from, to, f * (2.0 - f));
            a.speed = 0.0;
            let trail = (t / 0.06) as usize;
            if trail != ((t - dt).max(0.0) / 0.06) as usize {
                boss.deeds.push(Deed::Flare { at: a.position + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.15, radius: 0.8, damage: 0.0 });
            }
            if let Some(p) = player
                && !boss.rammed
                && (p - a.position).xy().norm() < LUNGE_HITS * vpm
                && (p.z - a.position.z).abs() < 2.5 * vpm
            {
                boss.rammed = true;
                let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
                boss.deeds.push(Deed::Ram { damage: 26.0, direction: forward });
            }
        }
        // Out of the way, a frost of it left where it stood, then cutting back at the player
        Move::Riposte => {
            let (from, to) = boss.leap;
            slide(a, from, to, (t / RIPOSTE_STEP).min(1.0));
            a.speed = 0.0;
            if crossed(RIPOSTE_STEP) {
                boss.deeds.push(Deed::Blink { from: from + Vector3::new(0.0, 0.0, 1.2 * vpm), to: a.position + Vector3::new(0.0, 0.0, 1.2 * vpm) });
            }
            if let Some(p) = player {
                a.target_heading = heading_to(a.position, p);
                a.heading = a.target_heading;
            }
            if crossed(RIPOSTE_CUT) {
                let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
                let (reach, radius, _, _) = slam(look);
                boss.deeds.push(Deed::Slam { at: a.position + forward * reach * vpm, radius, damage: 20.0, crater: 0.0 });
                boss.deeds.push(Deed::Swipe(a.position, 2));
            }
        }
        // Crescents of frost off its blade, one each time it's swept across, fanned out once it's
        // angry
        Move::Crescent => {
            for (k, &at) in CRESCENTS.iter().enumerate() {
                if crossed(at) || at == 0.0 && t - dt <= 0.0 {
                    let from = emit(a, boss, vpm);
                    let to = player.unwrap_or(boss.aim) + Vector3::new(0.0, 0.0, 1.0 * vpm);
                    let across = Vector3::new(-(to - from).y, (to - from).x, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                    let fan: &[f64] = if boss.enraged { &[-1.0, 0.0, 1.0] } else { &[0.0] };
                    for &j in fan {
                        boss.deeds.push(Deed::Spit { from, to: to + across * (j * 2.2 + (k as f64 - 1.0) * 0.6) * vpm });
                    }
                    boss.deeds.push(Deed::Swipe(a.position, k));
                }
            }
        }
        // Mote after mote of venom pouring out of its abdomen, spiralling in on where the player is
        Move::Swarm => {
            let due = ((t / SWARM_EVERY) as usize).min((SWARM_SECONDS / SWARM_EVERY) as usize);
            let done = boss.trail as usize;
            let p = player.unwrap_or(boss.aim);
            for k in done..due {
                let f = k as f64 * SWARM_EVERY / SWARM_SECONDS;
                let r = SWARM_FROM + (SWARM_TO - SWARM_FROM) * f;
                let turn = own_turn(boss) + k as f64 * 0.9;
                let to = p + Vector3::new(turn.cos() * r, turn.sin() * r, 0.4) * vpm;
                let from = emit(a, boss, vpm);
                boss.deeds.push(Deed::Lob { from, to, time: 0.55 + 0.15 * (k % 3) as f64, damage: 5.0, radius: 0.9, rock: false });
            }
            boss.trail = due as f64;
        }
        // Its crystals bursting up one after another along the way to the player
        Move::Undermine => {
            for k in 0..UNDERMINE_SPIKES {
                if crossed(k as f64 * UNDERMINE_EVERY) || k == 0 && t - dt <= 0.0 {
                    let at = undermine_spot(a, boss, k, vpm);
                    let lean = (at - a.position).xy().try_normalize(1e-6).map_or(Vector3::z(), |w| Vector3::new(w.x, w.y, 0.0));
                    boss.deeds.push(Deed::Spire { at, height: 1.6 + 0.12 * k as f64, lean });
                    boss.deeds.push(Deed::Flare { at: at + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.0, radius: 1.2, damage: 18.0 });
                }
            }
        }
        // The bog heaving out across the ground from under its foot
        Move::Mirestomp => {
            let to = (t * QUAKE_PACE).min(MIRE_REACH);
            boss.deeds.push(Deed::Quake { at: boss.leap.0, from: (t - dt).max(0.0) * QUAKE_PACE, to, dt });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each is a boss drawn jointed, with a name, an element and a hoard of its own, leaving weapons
    /// of families there are, making three moves or more of which at least two are its own, and
    /// turning up somewhere: in the land in place of another boss, at a kind of ruin, or among a
    /// realm's guardians
    #[test]
    fn each_fights_its_own_way_somewhere() {
        use crate::worldgen::Land;
        let library = crate::weapons::Library::load();
        let mut names = Vec::new();
        let bases = [Look::Golem, Look::Drake, Look::Ironhide, Look::Treant, Look::Direwolf, Look::Spider, Look::Ogre, Look::Colossus, Look::Elderhorn, Look::GraveKnight];
        let grounds = [GRASS, SNOW, SAND, JUNGLE_GRASS, STONE];
        let lands = [Land::Normal, Land::Dream, Land::Ice, Land::Sky];
        for look in LOOKS {
            assert!(look.is_boss() && !look.is_raid() && crate::rig::rigged(look), "{look:?}");
            assert!(look.element().is_some() && crate::bosses::roster::hoard(look).is_some());
            assert!(!names.contains(&look.name()));
            names.push(look.name());
            for family in taste(look) {
                assert!(library.families.iter().any(|f| f.id == *family), "{look:?} leaves {family}, which isn't a family");
            }
            let moves = repertoire(look);
            let own: Vec<Move> = moves.iter().copied().filter(|&m| owns(m)).collect();
            assert!(moves.len() >= 4 && own.len() >= 2, "{look:?}: {moves:?}");
            let in_land = bases.iter().any(|b| lands.iter().any(|&l| grounds.iter().any(|&g| b.for_land(l).for_ground(g) == look)));
            let in_realm = crate::realms::REALMS.iter().any(|r| r.guardians.contains(&look));
            let at_sites = matches!(look, Look::Gloamdelver | Look::Fenhulk);
            assert!(in_land || in_realm || at_sites, "{look:?} is nowhere");
        }
        assert_eq!(dodges(Look::WinterglassDuelist), Some(Move::Riposte));
    }

    /// What's shown on the ground as each gets ready is where it then comes down: the Matriarch's
    /// stingers and the Fenhulk's spew where their rings were; and the duelist's lunge goes no
    /// further than it reaches
    #[test]
    fn what_shows_is_where_it_lands() {
        let vpm = 16.0;
        for (look, m) in [(Look::HiveMatriarch, Move::Stingrain), (Look::Fenhulk, Move::Bogspew)] {
            let mut a = Animal::boss(look, Vector3::new(500.0, 500.0, 100.0), [3, 4, 5], 7);
            let mut boss = a.boss.take().unwrap();
            boss.aim = Vector3::new(700.0, 520.0, 100.0);
            winding(m, 0.5, 1.0, &a, &mut boss, Vector3::x(), vpm);
            let marks: Vec<Vector3<f64>> = boss.deeds.drain(..).filter_map(|d| if let Deed::Mark { at, .. } = d { Some(at) } else { None }).collect();
            let aim = boss.aim;
            begin(m, &mut a, &mut boss, Some(aim), Vector3::x(), vpm);
            let lobs: Vec<Vector3<f64>> = boss.deeds.drain(..).filter_map(|d| if let Deed::Lob { to, .. } = d { Some(to) } else { None }).collect();
            assert!(!marks.is_empty() && marks == lobs, "{look:?}");
        }
        let a = Animal::boss(Look::WinterglassDuelist, Vector3::new(500.0, 500.0, 100.0), [3, 4, 5], 7);
        let (_, long) = lunge_line(&a, Vector3::new(900.0, 500.0, 100.0), vpm);
        assert!((long - LUNGE_REACH).abs() < 1e-9);
    }
}
