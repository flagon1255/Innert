//! The Sky Isles' own bosses. Their guardians were the avians' two and two borrowed from the land
//! below; now two of their own float over their islands rather than walk them, see
//! src\rig\drifter.rs. The Aeolith, a spirit of the wind bound in rings of carved stone, flings its
//! great ring at the player like a discus, spins its rings up into a whirlwind that draws them in
//! and bursts, and rains its stone shards down on them. The Stormbell, a jelly of thundercloud,
//! rains lightning from its rim, lashes its tendrils out with lightning running on along the ground,
//! and charges up till it lets it all go in rings of lightning round it.
//!
//! What's here is what each is and its own moves, as Deeds for src\game.rs; src\bosses\roster.rs
//! asks here about them first.

use super::*;

/// Both of them, in the order they're added among the looks
pub const LOOKS: [Look; 2] = [Look::Aeolith, Look::Stormbell];

/// Whether `look` is one of them
pub fn has(look: Look) -> bool {
    LOOKS.contains(&look)
}

/// Whether `m` is one of their own moves, played out here
pub fn owns(m: Move) -> bool {
    matches!(m, Move::Ringcast | Move::Gyre | Move::Shardfall | Move::Staticfall | Move::Thunderlash | Move::Overcharge)
}

/// How much bigger its model is than its build's
pub fn scale(look: Look) -> f64 {
    if look == Look::Aeolith { 1.6 } else { 1.5 }
}

/// How much damage it takes: as the casters do, the one of stone a little more, the cloud less
pub fn health(look: Look) -> f64 {
    if look == Look::Aeolith { 2800.0 } else { 2450.0 }
}

/// How fast it drifts, and glides faster, in meters a second
pub fn speeds(look: Look) -> (f64, f64) {
    if look == Look::Aeolith { (1.3, 3.4) } else { (1.1, 2.8) }
}

/// Meters covered by one cycle of its drifting, at its build's size
pub fn stride(look: Look) -> f64 {
    if look == Look::Aeolith { 1.8 } else { 1.6 }
}

/// Radius of a sphere round the whole of it, in meters: the Aeolith's shards raised high over it,
/// the Stormbell's bell risen up to rain
pub fn reach(look: Look) -> f64 {
    look.scale() * if look == Look::Aeolith { 4.9 } else { 4.3 }
}

/// The body to aim at, the heart of the one and the bell of the other, floating: how high its
/// middle is, and how big it is, in meters
pub fn body(look: Look) -> (f64, f64) {
    let s = look.scale();
    let (high, round) = if look == Look::Aeolith { (1.75, 0.7) } else { (2.3, 0.95) };
    (high * s, round * s)
}

pub fn name(look: Look) -> &'static str {
    if look == Look::Aeolith { "Aeolith" } else { "Stormbell" }
}

/// What its attacks are made of: the wind's magic, and lightning
pub fn element(look: Look) -> Option<Element> {
    Some(if look == Look::Aeolith { Element::Arcane } else { Element::Storm })
}

/// What it's made of: its body, darker parts, things on it, what glows, and one more
pub(crate) fn skin(look: Look) -> Skin {
    let make = |body, dark, accent, glow, extra| Skin { body, dark, accent, glow, extra };
    if look == Look::Aeolith {
        // Pale carved stone, darker in its bands, teal inlay, runes glowing with the wind's magic,
        // gold bands
        make(LIMESTONE, TEMPLE_DARK, TEMPLE_TEAL, GLOW_ARCANE, GOLD)
    } else {
        // White cloud, storm grey under it, foam, lightning in its veins, a crystal crown
        make(FUR_WHITE, FUR_GRAY, FOAM, GLOW_STORM, CRYSTAL)
    }
}

/// The kinds of weapon it leaves most of the time: war fans, orbs, battlestaves and rods from the
/// one, rods, twin prods, bells and sceptres from the other
pub fn taste(look: Look) -> &'static [&'static str] {
    if look == Look::Aeolith { &["warfan", "orb", "battlestaff", "rod"] } else { &["rod", "twinprod", "bell", "scepter"] }
}

/// What its hoard's called, as it spills out of it
pub fn hoard(look: Look) -> &'static str {
    if look == Look::Aeolith { "windfall hoard" } else { "storm-cache" }
}

/// A blow at what's in front: the Aeolith's fists brought down together, the Stormbell's tendrils
/// stung down, how far ahead and round in meters, how hard, and no hole
pub fn slam(look: Look) -> (f64, f64, f64, f64) {
    if look == Look::Aeolith { (2.3, 2.6, 26.0, 0.0) } else { (3.0, 2.3, 22.0, 0.0) }
}

/// The moves each makes in a fight and from how far off, its own first
pub fn menu(look: Look) -> &'static [(Move, f64, f64)] {
    match look {
        // Its whirlwind for whoever crowds it, its shards and its ring further off, its fists close
        // to, and a bolt of the wind from afar
        Look::Aeolith => &[(Move::Gyre, 0.0, 5.0), (Move::Shardfall, 5.0, 24.0), (Move::Ringcast, 4.0, 18.0), (Move::Slam, 0.0, 3.6), (Move::Spit, 4.0, 28.0)],
        // Letting go its charge on whoever's under it, lashing at whoever's near, raining lightning
        // on whoever's further, stinging close to, and a ball of lightning from afar
        _ => &[(Move::Overcharge, 0.0, 5.5), (Move::Thunderlash, 3.0, 11.0), (Move::Staticfall, 6.0, 26.0), (Move::Slam, 0.0, 3.8), (Move::Spit, 5.0, 28.0)],
    }
}

/// Seconds the Aeolith's great ring is out for, flung and back, and how far through that it comes
/// down where it was aimed and how far it starts back, see rig\drifter.rs; and how far round its
/// burst there hurts, in meters
pub const RINGCAST_SECONDS: f64 = 1.7;
pub const RINGCAST_LANDS: f64 = 0.42;
pub const RINGCAST_BACK: f64 = 0.58;
const RINGCAST_RADIUS: f64 = 2.4;
/// Seconds the Aeolith's gyre whirls, how far out it draws the player in and how hard at the most,
/// in meters and meters a second, and how far round it bursts out at the last
const GYRE_SECONDS: f64 = 2.2;
const GYRE_REACH: f64 = 9.0;
const GYRE_PULL: f64 = 3.0;
const GYRE_BURST: f64 = 4.2;
/// How far round the player the Aeolith's other shards come down, and how far round each hurts
const SHARD_RING: f64 = 3.2;
const SHARD_RADIUS: f64 = 1.5;
/// Seconds the Stormbell rains lightning for, how far round the player it comes down, how far round
/// each strike hurts, and how far into its rain it strikes again where the player's gone
const STATIC_SECONDS: f64 = 1.3;
const STATIC_RING: f64 = 3.0;
const STATIC_RADIUS: f64 = 1.5;
const STATIC_AGAIN: f64 = 0.75;
/// How far apart the bursts along the Stormbell's lash are, in meters, and how many
const LASH_SPACING: f64 = 1.6;
const LASH_BURSTS: usize = 7;
/// Seconds the Stormbell's charge takes to run out; and for each ring of it, seconds into that it
/// bursts up, how far out, and how many bursts make it
const OVER_SECONDS: f64 = 0.9;
const OVER_RINGS: [(f64, f64, usize); 3] = [(0.0, 2.4, 7), (0.3, 4.6, 11), (0.6, 6.8, 15)];

/// Seconds each of their moves takes to get ready for, to make and to get over, and how long till
/// it can be made again
pub fn timing(_look: Look, m: Move) -> (f64, f64, f64, f64) {
    match m {
        Move::Ringcast => (1.0, RINGCAST_SECONDS, 0.8, 7.0),
        // Slowed and tipped after its whirlwind: the moment to hit it
        Move::Gyre => (1.1, GYRE_SECONDS, 1.5, 10.0),
        Move::Shardfall => (1.1, 0.5, 1.1, 8.0),
        Move::Staticfall => (1.2, STATIC_SECONDS, 0.9, 8.5),
        Move::Thunderlash => (0.85, 0.5, 0.8, 6.0),
        // Spent and sagging once it's let go
        Move::Overcharge => (1.5, OVER_SECONDS, 1.6, 11.0),
        _ => (0.8, 0.5, 0.8, 6.0),
    }
}

/// Where the Aeolith's shards come down: one on the player and the rest round them
fn shard_spots(boss: &Boss, vpm: f64) -> Vec<Vector3<f64>> {
    std::iter::once(boss.aim).chain(ring(boss.aim, 5, SHARD_RING, own_turn(boss) + 0.3, vpm)).collect()
}

/// Where the Aeolith's `k`th shard is flung from: raised high over it, see rig\drifter.rs
fn shard_from(a: &Animal, k: usize, vpm: f64) -> Vector3<f64> {
    let s = a.look.scale();
    let turn = k as f64 / 6.0 * TAU;
    on_body(a, [turn.cos() * 0.95 * s, turn.sin() * 0.95 * s, 3.6 * s], vpm)
}

/// Where the Stormbell's lightning comes down: on the player and round them, more of it once it's
/// angry
fn static_spots(boss: &Boss, vpm: f64) -> Vec<Vector3<f64>> {
    let n = if boss.enraged { 7 } else { 5 };
    std::iter::once(boss.aim).chain(ring(boss.aim, n, STATIC_RING, own_turn(boss) + 1.1, vpm)).collect()
}

/// Where the Stormbell's `k`th bolt leaves its rim, as it's risen to rain
fn rim(a: &Animal, k: usize, vpm: f64) -> Vector3<f64> {
    let s = a.look.scale();
    let turn = k as f64 * 2.4;
    on_body(a, [turn.cos() * 0.9 * s, turn.sin() * 0.9 * s, 2.75 * s], vpm)
}

/// Where the line of lightning the Stormbell's lash sends along the ground starts: under its
/// tendrils' tips, in front of it
fn lash_start(a: &Animal, vpm: f64) -> Vector3<f64> {
    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
    a.position + forward * 2.0 * a.look.scale() * vpm
}

/// The lines of lightning the Stormbell's lash sends running along the ground from in front of it
/// at the player at `to` and on past them, each burst a little after the one before; three fanned
/// out once it's angry
fn lash(a: &Animal, boss: &mut Boss, to: Vector3<f64>, vpm: f64) {
    let start = lash_start(a, vpm);
    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
    let ahead = (to - start).xy().try_normalize(1e-6).unwrap_or_else(|| forward.xy());
    let lanes: &[f64] = if boss.enraged { &[-0.33, 0.0, 0.33] } else { &[0.0] };
    for &turn in lanes {
        let (c, sn) = (turn.cos(), turn.sin());
        let way = Vector3::new(ahead.x * c - ahead.y * sn, ahead.x * sn + ahead.y * c, 0.0);
        for k in 0..LASH_BURSTS {
            let at = start + way * (LASH_SPACING * k as f64) * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm);
            boss.deeds.push(Deed::Flare { at, delay: 0.06 + 0.07 * k as f64, radius: 1.25, damage: 16.0 });
        }
    }
}

/// The `k`th ring of lightning the Stormbell lets go round itself, the first right under it too
fn discharge(a: &Animal, boss: &mut Boss, k: usize, vpm: f64) {
    let (_, r, n) = OVER_RINGS[k];
    let turn = own_turn(boss) + k as f64 * 0.37;
    for at in ring(a.position + Vector3::new(0.0, 0.0, 1.0 * vpm), n, r, turn, vpm) {
        boss.deeds.push(Deed::Flare { at, delay: 0.0, radius: 1.3, damage: 18.0 });
    }
    if k == 0 {
        boss.deeds.push(Deed::Flare { at: a.position + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.0, radius: 1.6, damage: 18.0 });
    }
    boss.deeds.push(Deed::Slam { at: a.position, radius: 0.0, damage: 0.0, crater: 0.0 });
}

/// Shown on the ground as boss `a` gets ready to make move `m`, `t` seconds into getting ready of
/// `whole`: where it'll come down, a ring closing in on each place
pub fn winding(m: Move, t: f64, whole: f64, a: &Animal, boss: &mut Boss, forward: Vector3<f64>, vpm: f64) {
    let closing = (1.0 - t / whole.max(0.1)).clamp(0.0, 1.0);
    match m {
        // Where the Aeolith's ring will come down; how far its whirlwind will draw the player in
        // from and where it'll burst; where its shards will fall
        Move::Ringcast => boss.deeds.push(Deed::Mark { at: boss.aim, radius: RINGCAST_RADIUS, closing }),
        Move::Gyre => {
            boss.deeds.push(Deed::Mark { at: a.position, radius: GYRE_BURST, closing });
            boss.deeds.push(Deed::Mark { at: a.position, radius: GYRE_REACH, closing: 0.0 });
        }
        Move::Shardfall => {
            for at in shard_spots(boss, vpm) {
                boss.deeds.push(Deed::Mark { at, radius: SHARD_RADIUS, closing });
            }
        }
        // Where the Stormbell's lightning will come down, the line its lash will send along the
        // ground, and the rings its charge will run out in
        Move::Staticfall => {
            for at in static_spots(boss, vpm) {
                boss.deeds.push(Deed::Mark { at, radius: STATIC_RADIUS, closing });
            }
        }
        Move::Thunderlash => {
            let start = lash_start(a, vpm);
            let ahead = (boss.aim - start).xy().try_normalize(1e-6).unwrap_or_else(|| forward.xy());
            for k in 0..4 {
                let at = start + Vector3::new(ahead.x, ahead.y, 0.0) * (LASH_SPACING * 1.5 * k as f64) * vpm;
                boss.deeds.push(Deed::Mark { at, radius: 1.25, closing });
            }
        }
        Move::Overcharge => {
            for (_, r, _) in OVER_RINGS {
                boss.deeds.push(Deed::Mark { at: a.position, radius: r + 1.0, closing });
            }
        }
        _ => {}
    }
}

/// Boss `a` making move `m`, now it's ready
pub fn begin(m: Move, a: &mut Animal, boss: &mut Boss, _player: Option<Vector3<f64>>, forward: Vector3<f64>, vpm: f64) {
    match m {
        // Its shards flung down from high over it where they showed, one after another
        Move::Shardfall => {
            for (k, to) in shard_spots(boss, vpm).into_iter().enumerate() {
                boss.deeds.push(Deed::Lob { from: shard_from(a, k, vpm), to, time: 0.5 + 0.06 * k as f64, damage: 16.0, radius: SHARD_RADIUS, rock: true });
            }
        }
        // Lightning out of its rim, fast, where it showed, one strike after another
        Move::Staticfall => {
            for (k, to) in static_spots(boss, vpm).into_iter().enumerate() {
                boss.deeds.push(Deed::Lob { from: rim(a, k, vpm), to, time: 0.3 + 0.1 * k as f64, damage: 16.0, radius: STATIC_RADIUS, rock: false });
            }
        }
        // Its tendrils lashed down in front of it, and lightning running on from there at the player
        Move::Thunderlash => {
            let (reach, radius, damage, _) = slam(a.look);
            boss.deeds.push(Deed::Slam { at: a.position + forward * reach * vpm, radius, damage, crater: 0.0 });
            boss.deeds.push(Deed::Swipe(a.position, 1));
            lash(a, boss, boss.aim, vpm);
        }
        // Letting go its charge: the first ring of lightning, right round it and under it
        Move::Overcharge => discharge(a, boss, 0, vpm),
        _ => {}
    }
}

/// Boss `a` `t` seconds into making move `m`, in a frame of `dt` seconds
pub fn during(m: Move, t: f64, dt: f64, a: &mut Animal, boss: &mut Boss, player: Option<Vector3<f64>>, vpm: f64) {
    let crossed = |at: f64| t >= at && t - dt < at;
    match m {
        // The Aeolith's great ring coming down where it showed, bursting, and sweeping back to it,
        // see rig\drifter.rs
        Move::Ringcast => {
            if crossed(RINGCAST_SECONDS * RINGCAST_LANDS) {
                boss.deeds.push(Deed::Slam { at: boss.aim, radius: RINGCAST_RADIUS, damage: 26.0, crater: 0.0 });
                boss.deeds.push(Deed::Flare { at: boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.0, radius: RINGCAST_RADIUS, damage: 0.0 });
                boss.deeds.push(Deed::Swipe(boss.aim, 2));
            }
        }
        // Its whirlwind drawing the player in, harder once it's angry, the ring it'll burst out to
        // closing in; then bursting, the wind and its magic flung out all round it
        Move::Gyre => {
            let pull = if boss.enraged { GYRE_PULL * 1.25 } else { GYRE_PULL };
            boss.deeds.push(Deed::Gust { at: a.position, reach: GYRE_REACH, pull, dt });
            boss.deeds.push(Deed::Mark { at: a.position, radius: GYRE_BURST, closing: (1.0 - t / GYRE_SECONDS).max(0.0) });
            if crossed(GYRE_SECONDS) {
                boss.deeds.push(Deed::Slam { at: a.position, radius: GYRE_BURST, damage: 28.0, crater: 0.0 });
                let turn = own_turn(boss);
                for (n, r) in [(6, 1.8), (10, 3.6)] {
                    for at in ring(a.position + Vector3::new(0.0, 0.0, 1.0 * vpm), n, r, turn, vpm) {
                        boss.deeds.push(Deed::Flare { at, delay: 0.0, radius: 1.2, damage: 0.0 });
                    }
                }
            }
        }
        // Lightning once more, where the player's gone, shown as it comes
        Move::Staticfall => {
            if let Some(p) = player
                && t < STATIC_AGAIN - 0.3
            {
                boss.aim = p;
            }
            if t < STATIC_AGAIN {
                boss.deeds.push(Deed::Mark { at: boss.aim, radius: STATIC_RADIUS, closing: 1.0 - t / STATIC_AGAIN });
            }
            if crossed(STATIC_AGAIN) {
                let to = boss.aim;
                boss.deeds.push(Deed::Lob { from: rim(a, 3, vpm), to, time: 0.25, damage: 16.0, radius: STATIC_RADIUS, rock: false });
            }
        }
        // The rest of its charge running out in rings, one further out than the last
        Move::Overcharge => {
            for k in 1..OVER_RINGS.len() {
                if crossed(OVER_RINGS[k].0) {
                    discharge(a, boss, k, vpm);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Sky Isles' guardians are their own two first, the avians' two after, and nothing
    /// borrowed from the land below is left among those that ward its raid bosses
    #[test]
    fn the_sky_isles_have_bosses_of_their_own() {
        let sky = crate::realms::by_id("sky").unwrap();
        let guarding: Vec<Look> = (0..crate::realms::guardian_count(sky)).map(|k| crate::realms::guardian_look(sky, k)).collect();
        for look in [Look::Aeolith, Look::Stormbell, Look::Stormtalon, Look::Harridan] {
            assert!(guarding.contains(&look), "{look:?} doesn't guard the Sky Isles: {guarding:?}");
        }
        assert!(!guarding.contains(&Look::Ironhide) && !guarding.contains(&Look::Treant), "{guarding:?}");
        // And they're what turns up there where the land below's would
        assert_eq!(Look::Ironhide.for_land(crate::worldgen::Land::Sky), Look::Aeolith);
        assert_eq!(Look::Treant.for_land(crate::worldgen::Land::Sky), Look::Stormbell);
    }

    /// Each is a boss drawn jointed, not a raid boss, with a name, an element and a hoard of its
    /// own, leaving weapons of families there are, and making three moves of its own no other boss
    /// makes, among at least five
    #[test]
    fn each_has_its_own() {
        let library = crate::weapons::Library::load();
        for look in LOOKS {
            assert!(look.is_boss() && crate::rig::rigged(look) && !look.is_raid(), "{look:?}");
            assert!(look.element().is_some() && crate::bosses::roster::hoard(look) == Some(hoard(look)));
            for family in taste(look) {
                assert!(library.families.iter().any(|f| f.id == *family), "{look:?} leaves {family}, which isn't a family");
            }
            let moves = repertoire(look);
            assert!(moves.len() >= 5 && moves.iter().filter(|&&m| owns(m)).count() == 3, "{look:?}: {moves:?}");
            for other in Look::ALL.into_iter().filter(|&l| l != look && l.build().is_some()) {
                assert!(repertoire(other).iter().all(|&m| !owns(m) || has(other)), "{other:?} makes one of {look:?}'s moves");
            }
        }
    }

    /// Plays boss `a` making move `m` from start to finish against a player standing still at
    /// `player`, a frame a time, and gives back everything it did
    fn play_out(a: &mut Animal, m: Move, player: Vector3<f64>, vpm: f64) -> Vec<Deed> {
        let mut boss = a.boss.take().unwrap();
        boss.aim = player;
        let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
        let (wind, act, _, _) = timing(a.look, m);
        let dt = 1.0 / 30.0;
        let mut t = 0.0;
        while t < wind {
            winding(m, t, wind, a, &mut boss, forward, vpm);
            t += dt;
        }
        begin(m, a, &mut boss, Some(player), forward, vpm);
        let mut t = 0.0;
        while t < act + dt {
            t += dt;
            during(m, t, dt, a, &mut boss, Some(player), vpm);
        }
        let deeds = std::mem::take(&mut boss.deeds);
        a.boss = Some(boss);
        deeds
    }

    /// Every move of theirs comes down where it showed it would as it got ready: each strike of
    /// lightning, each shard and the great ring on a ring shown on the ground first, and the lash's
    /// and the charge's lightning within what was shown
    #[test]
    fn they_strike_where_they_showed() {
        let vpm = 16.0;
        let player = Vector3::new(560.0, 520.0, 100.0);
        for (look, m) in [(Look::Aeolith, Move::Ringcast), (Look::Aeolith, Move::Shardfall), (Look::Aeolith, Move::Gyre), (Look::Stormbell, Move::Staticfall), (Look::Stormbell, Move::Thunderlash), (Look::Stormbell, Move::Overcharge)] {
            let mut a = Animal::boss(look, Vector3::new(500.0, 500.0, 100.0), [3, 4, 5], 7);
            let deeds = play_out(&mut a, m, player, vpm);
            let marks: Vec<(Vector3<f64>, f64)> = deeds.iter().filter_map(|d| if let Deed::Mark { at, radius, .. } = *d { Some((at, radius)) } else { None }).collect();
            let shown = |at: Vector3<f64>, spare: f64| marks.iter().any(|&(m, r)| (m - at).xy().norm() / vpm <= r + spare);
            let mut hurts = 0;
            for d in &deeds {
                match *d {
                    Deed::Lob { to, damage, .. } if damage > 0.0 => {
                        assert!(shown(to, 0.01), "{look:?}'s {m:?} lobs where nothing showed");
                        hurts += 1;
                    }
                    Deed::Flare { at, damage, .. } if damage > 0.0 => {
                        assert!(shown(at, 1.3), "{look:?}'s {m:?} bursts up where nothing showed");
                        hurts += 1;
                    }
                    Deed::Slam { at, damage, .. } if damage > 0.0 => {
                        assert!(shown(at, 0.01) || m == Move::Thunderlash, "{look:?}'s {m:?} lands where nothing showed");
                        hurts += 1;
                    }
                    _ => {}
                }
            }
            assert!(hurts > 0, "{look:?}'s {m:?} does nothing");
            // The whirlwind draws the player in all the while it whirls
            if m == Move::Gyre {
                assert!(deeds.iter().filter(|d| matches!(d, Deed::Gust { .. })).count() > 30);
            }
            // The great ring comes down once, on the player
            if m == Move::Ringcast {
                let slams: Vec<Vector3<f64>> = deeds.iter().filter_map(|d| if let Deed::Slam { at, damage, .. } = *d { (damage > 0.0).then_some(at) } else { None }).collect();
                assert_eq!(slams, vec![player]);
            }
        }
    }
}
