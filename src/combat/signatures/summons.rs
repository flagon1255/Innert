//! A necromancer's summons, see weapons::Power::summons(): the Bone Phalanx, armored skeletons
//! standing their ground in a line; Bone Archers, skeletons keeping back and shooting; the Grave
//! Colossus, a grave brute, slow and huge, striking all round where its club lands; Wraith Flight,
//! wraiths flying at what's near and swooping through it again and again; and Corpse Rising, the
//! slain lying about raised as what they were.
//!
//! Each is a shade sent to the ring, seen flying there, and what it raises climbs up out of the
//! ground there, fights with blows or arrows of its own, and crumbles back into the ground when its
//! time's out. They're familiars, as Raise Dead's skeletons are, see faith.rs, drawn with the
//! creatures' own models, so they cost no memory of their own. No more than MINIONS of all of
//! them can be up at once, the oldest crumbling for the new; and right click while the power comes
//! round sends every one of them at what's aimed at.

use super::faith::{CRUMBLE_SECONDS, Minion, RISE_SECONDS, RISEN_DRAIN, Risen, hostile};
use super::powers::Shape;
use super::*;
use crate::weapons::Power;

/// How far off a shade can be sent to raise the dead, in meters
pub(in crate::combat) const CALL_REACH: f64 = 10.0;
/// How long, in seconds, the dead sent at something go after it
const ORDER_SECONDS: f64 = 8.0;
/// How far off the dead look for what to fight, in meters
const MINION_HUNT: f64 = 24.0;
/// The Bone Phalanx: how many, how far apart they stand, how far from where they stand they go
/// after what comes near, in meters, how long they last, and how hard their blows are next to
/// the spell's
const PHALANX: usize = 4;
const PHALANX_GAP: f64 = 1.3;
const PHALANX_GUARD: f64 = 7.0;
const PHALANX_SECONDS: f64 = 20.0;
const PHALANX_POWER: f64 = 0.5;
/// Bone Archers: how many, how far they shoot, in meters, how often, how fast their arrows fly,
/// how near they let things come before they step back, how long they last, and how hard
const ARCHERS: usize = 3;
const ARCHER_RANGE: f64 = 26.0;
const ARCHER_PACE: f64 = 1.3;
const ARROW_SPEED: f64 = 36.0;
const ARCHER_BACK: f64 = 1.0;
/// How long an archer takes to draw and loose
const ARCHER_DRAW: f64 = 0.5;
const ARCHER_SECONDS: f64 = 18.0;
const ARCHER_POWER: f64 = 0.45;
/// The Grave Colossus: how long it lasts, how fast it walks, how long its blow takes, how far
/// round where it lands it strikes, and how hard
const COLOSSUS_SECONDS: f64 = 22.0;
const COLOSSUS_SPEED: f64 = 3.2;
const COLOSSUS_BLOW: f64 = 1.1;
const COLOSSUS_ROUND: f64 = 2.6;
const COLOSSUS_POWER: f64 = 1.4;
/// Wraith Flight: how many, how long they last, how high they hover, how fast they fly and dive,
/// in meters a second, how long they rest between swoops, and how hard each is
const WRAITHS: usize = 3;
const WRAITH_SECONDS: f64 = 12.0;
const WRAITH_HOVER: f64 = 1.6;
const WRAITH_FLY: f64 = 7.0;
const WRAITH_DIVE: f64 = 12.0;
const WRAITH_REST: f64 = 0.9;
const WRAITH_POWER: f64 = 0.6;
/// Corpse Rising: how far round the ring the slain are raised from, in meters, how many at most,
/// how many of the long dead rise if too few lie there, how long they last, and how hard
const CORPSE_REACH: f64 = 12.0;
const CORPSES: usize = 5;
const CORPSES_LEAST: usize = 2;
const CORPSE_SECONDS: f64 = 16.0;
const CORPSE_POWER: f64 = 0.55;
/// How fast skeletons walk, how far they reach, and how long a skeleton's blow takes
const BONES_SPEED: f64 = 3.8;
const BONES_REACH: f64 = 1.3;
const BONES_BLOW: f64 = 0.6;
/// When in a blow or a draw it lands or looses
const LANDS: f64 = 0.55;

/// What the dead go for, all of them together, see Combat::choose_prey(): first what they were
/// sent at with right click; then what the player last struck or aimed a power at, for
/// FOCUS_SECONDS after, within FOCUS_LEASH meters of them; then what's struck the player in the
/// last ATTACKER_SECONDS, within ATTACKER_LEASH; then what's fighting them and come within
/// CLOSING; then the nearest thing that fights within LEASH. Once on something they keep on it
/// till it dies or goes LEASH_SLACK times as far off, or something comes up that's more to be gone
/// for.
const SENT: u8 = 0;
const FOCUS: u8 = 1;
const ATTACKER: u8 = 2;
const CLOSING_IN: u8 = 3;
const NEAREST: u8 = 4;
const FOCUS_SECONDS: f64 = 5.0;
const ATTACKER_SECONDS: f64 = 5.0;
const FOCUS_LEASH: f64 = 40.0;
const ATTACKER_LEASH: f64 = 30.0;
const CLOSING: f64 = 8.0;
const LEASH: f64 = 24.0;
const LEASH_SLACK: f64 = 1.5;
/// The least time, in seconds, what the player struck stays what they're after before something
/// else the player strikes takes its place, so blows landing on two at once never have the dead
/// turning back and forth between them
const HOLD: f64 = 1.0;
/// How far behind the player one of the dead can be left, in meters, before it climbs up out of
/// the ground beside them instead, as after a teleport or a gate; how far from the player it
/// starts to hurry after them and how far it's hurrying all it can, and how fast that is, in
/// meters a second, which is faster than the player sprints
const REJOIN: f64 = 32.0;
const HURRY_FROM: f64 = 4.0;
const HURRY_TO: f64 = 10.0;
const HURRY: f64 = 10.5;
/// How far the player can go from where the phalanx stands its ground, in meters, before it comes
/// after them to stand it again wherever they stop
const PHALANX_LEASH: f64 = 18.0;
/// Where the dead keep round the player with nothing to fight, behind them the way they've been
/// going: how far round from straight behind, in radians, and out from them, in meters, for each
/// of the first eight; the rest keep a ring further out
const GUARD: [(f64, f64); 8] = [(0.45, 2.6), (-0.45, 2.6), (1.15, 2.8), (-1.15, 2.8), (0.0, 3.6), (1.8, 3.0), (-1.8, 3.0), (0.0, 1.9)];
/// How far off its place by the player one of the dead lets itself be before it walks back to it,
/// in meters, so it doesn't shuffle about with every step the player takes
const GUARD_SLACK: f64 = 1.6;

/// The dead raised, taken along as the player goes through a gate to another world, see
/// Combat::take_raised()
pub struct Raised(Vec<Familiar>);

/// What the dead are after and why, see Combat::choose_prey()
#[derive(Default)]
pub(in crate::combat) struct Pack {
    /// What the player struck since the dead last looked, by id, and what they aimed a power at
    struck: Vec<u32>,
    /// What the player struck or aimed at last, by id, when they first did and when they last did
    focus: Option<(u32, f64, f64)>,
    /// What's struck the player lately, by id, and when it last did
    attackers: Vec<(u32, f64)>,
    /// What they're after, by id, and how much it's to be gone for, SENT to NEAREST
    on: Option<(u32, u8)>,
    /// And by index, this frame, for each of them to go after
    prey: Option<(usize, u8)>,
    /// Whether what's striking now is one of the dead or a familiar, not the player
    pub(in crate::combat) by_minion: bool,
    /// The boss or monster whose doing game.rs is carrying out, by id, for whatever it throws to
    /// be known as its, see Combat::hurl()
    pub(in crate::combat) doer: Option<u32>,
    /// Which way the player's been going, in radians, for where the dead keep round them, and
    /// where their feet were last frame
    heading: Option<f64>,
    last_feet: Option<Vector3<f64>>,
    /// Over the head of what they're after, for the mark showing it, see mark_instances()
    mark: Option<Vector3<f64>>,
}

impl Pack {
    pub(in crate::combat) fn recenter(&mut self, shift: Vector3<f64>) {
        for at in [&mut self.last_feet, &mut self.mark].into_iter().flatten() {
            *at -= shift;
        }
    }
}

/// Which of them each kind of shade raises
fn kind_of(power: Power) -> u8 {
    match power {
        Power::Phalanx => 0,
        Power::Archers => 1,
        Power::Colossus => 2,
        Power::Wraiths => 3,
        _ => 4,
    }
}

/// How long their moves take, and when the shade leaves the weapon
pub(in crate::combat) fn summon_timing(power: Power) -> Option<(f64, f64)> {
    power.summons().then_some((1.0, 0.5))
}

/// Their moves: a caster raised high and held as the dead are called, then thrust down at the
/// ground ahead; a scythe or dagger raised in both hands and driven down into it
pub(in crate::combat) fn summon_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = summon_timing(power)?;
    let magic = profile.kind == Kind::Magic && profile.style != Style::Glaive;
    if magic {
        let at = spell_at(profile.style);
        return Some(vec![
            (0.15, at([0.0, -0.05, 0.12], [0.0, -0.25, 0.97]), false),
            (0.36, at([0.01, -0.07, 0.17], [0.02, -0.35, 0.94]), false),
            (release - 0.05, at([0.0, -0.06, 0.18], [0.0, -0.3, 0.95]), false),
            (release, at([-0.03, 0.22, -0.1], [-0.04, 0.7, -0.71]), true),
            (release + 0.14, at([-0.03, 0.23, -0.13], [-0.04, 0.64, -0.77]), false),
            (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
        ]);
    }
    Some(vec![
        (0.2, pose([0.18, 0.3, 0.04], [0.1, 0.05, 1.0], [0.0, 1.0, -0.05]), false),
        (release - 0.06, pose([0.18, 0.27, 0.1], [0.1, -0.12, 0.99], [0.0, 0.99, 0.12]), false),
        (release, pose([0.04, 0.6, -0.38], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]), true),
        (release + 0.12, pose([0.04, 0.62, -0.44], [0.0, 0.52, -0.85], [0.0, -0.85, -0.52]), false),
        (0.9, pose([0.2, 0.48, -0.3], [0.0, 0.55, 0.83], [0.0, 1.0, -0.1]), false),
    ])
}

/// How fast one of the dead that walks at `speed` meters a second goes after the player, `d`
/// meters off: hurrying, faster than the player can run, once they've got ahead
pub(in crate::combat) fn hurried(speed: f64, d: f64) -> f64 {
    let t = ((d - HURRY_FROM) / (HURRY_TO - HURRY_FROM)).clamp(0.0, 1.0);
    speed + (HURRY.max(speed) - speed) * t
}

/// Whether what fights is up and about to: not a boss lying in its lair yet to come for the
/// player, unless it's the sort that would anyway
fn roused(a: &Animal) -> bool {
    a.boss.as_ref().is_none_or(|b| b.engaged) || a.look.is_monster()
}

/// Whether one of the slain can rise as what it was: what walks on two legs or four and is no
/// bigger than a man much, the rest rising as the long dead
fn risen_as(look: Look) -> Look {
    if look.build().is_some() && look.reach() < 3.0 && !look.is_pet() { look } else { Look::Zombie }
}

impl Combat {
    /// One of them let go: a shade sent to the ring, and what's raised already sent at what's aimed
    /// at. True if it was one of them.
    pub(in crate::combat) fn unleash_summons(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, base: f64) -> bool {
        if !power.summons() {
            return false;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let from = self.held_point(weapon, view, false);
        let target = self.aim(arena);
        let spot = self.power_spot(arena, weapon).unwrap_or(target);
        self.keep_power_ring(weapon, spot, 1.5 + RISE_SECONDS);
        let shadow = Rc::new(Profile { element: Some(Element::Void), ..(*weapon.profile).clone() });
        // Lobbed, so it clears a ledge it's sent down off
        self.launch(&shadow, None, Form::Bolt, from, arced(from, spot, 2.0, vpm), base, 2.0);
        self.signed(Sig::Power(Shape::Calling { kind: kind_of(power) }));
        self.command_minions(arena);
        self.cues.push(Cue::here(Sound::Wail, 0.6).pitched(0.7));
        self.cues.push(Cue::here(Sound::Hex, 0.6).pitched(0.6));
        true
    }

    /// The weapon through one of their moves, `f` of the way through it: death drawn up out of
    /// the ground into it
    pub(in crate::combat) fn summon_gathers(&mut self, arena: &Arena, weapon: &Weapon, power: Power, f: f64, dt: f64) {
        let Some((_, release)) = summon_timing(power) else {
            return;
        };
        if f < 0.05 || f > release {
            return;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let tip = self.held_point(weapon, view, false);
        let feet = arena.player.unwrap_or(view.eye - Vector3::z() * 1.6 * vpm);
        if random(&mut self.rng) < dt * 35.0 {
            let a = random(&mut self.rng) * TAU;
            let at = feet + Vector3::new(a.cos(), a.sin(), 0.1) * (0.8 + random(&mut self.rng)) * vpm;
            let look = Element::look(Some(if random(&mut self.rng) < 0.3 { Element::Blood } else { Element::Void }));
            self.fine(at, (tip - at) * 2.0, look, 0.45, 0.0);
        }
    }

    /// Every one of the dead raised sent at what the crosshair's on, or what's nearest it, for a
    /// while: true if there were any up and something to send them at
    pub(in crate::combat) fn command_minions(&mut self, arena: &Arena) -> bool {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let raised: Vec<Vector3<f64>> = self.familiars.iter().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS).map(|f| f.position).collect();
        if raised.is_empty() {
            return false;
        }
        let (targets, _) = Combat::seek_targets(arena, MINION_HUNT + 6.0);
        let Some(&i) = targets.first() else {
            return false;
        };
        let a = &arena.animals[i];
        self.ordered = Some((a.id, ORDER_SECONDS));
        // On it at once, as all of them are shown being sent
        self.summons.on = Some((a.id, SENT));
        self.summons.prey = Some((i, SENT));
        let over = Combat::over(arena, i);
        self.spots.push((a.position + up * 0.05 * vpm, a.body().1 + 0.6, 0.8));
        let void = Element::look(Some(Element::Void));
        for at in raised {
            self.lightning(at + up * 1.3 * vpm, over, void, 0.02, 0, 1, 0.0, 0.0);
        }
        self.cues.push(Cue::here(Sound::Wail, 0.5).pitched(1.3));
        true
    }

    /// The dead raised that are up, taken to go along with the player to another world: put back
    /// with bring_raised() once it's there, they climb up out of the ground beside the player as
    /// soon as they're found to be far from them, see rejoin()
    pub fn take_raised(&mut self) -> Raised {
        let (raised, rest) = std::mem::take(&mut self.familiars).into_iter().partition(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS);
        self.familiars = rest;
        Raised(raised)
    }

    pub fn bring_raised(&mut self, raised: Raised) {
        self.familiars.extend(raised.0);
    }

    /// The player struck animal `i` with a blow or a shot of their own, or aimed a power at it,
    /// for the dead to go after, see choose_prey()
    pub(in crate::combat) fn struck_by_player(&mut self, id: u32) {
        if !self.summons.by_minion && !self.summons.struck.contains(&id) && self.summons.struck.len() < 16 {
            self.summons.struck.push(id);
        }
    }

    /// Something struck the player, by id, for the dead to turn on it, see game.rs
    pub fn blame(&mut self, id: u32) {
        let now = self.time;
        match self.summons.attackers.iter_mut().find(|(a, _)| *a == id) {
            Some(a) => a.1 = now,
            None => self.summons.attackers.push((id, now)),
        }
    }

    /// The boss or monster whose doing game.rs is carrying out, so what it throws and how it
    /// strikes the player is known as its, see blame()
    pub fn set_doer(&mut self, id: Option<u32>) {
        self.summons.doer = id;
    }

    pub fn doer(&self) -> Option<u32> {
        self.summons.doer
    }

    /// What the dead are after, by id, and why, for a screenshot run to say
    pub fn summons_on(&self) -> Option<(u32, &'static str)> {
        self.summons.on.map(|(id, tier)| (id, ["sent", "struck by the player", "struck the player", "closing in", "nearest"][tier as usize]))
    }

    /// How many of the dead are up, not crumbling
    pub fn raised_count(&self) -> usize {
        self.familiars.iter().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS).count()
    }

    /// The power of the weapon in hand ready to use, with the mana for it, for a screenshot run
    pub fn ready_power(&mut self) {
        self.power.cooldown = 0.0;
        self.mana = MAX_MANA;
    }

    /// What's been blamed for striking the player lately, by id, for tests to see
    #[cfg(test)]
    pub fn blamed(&self) -> Vec<u32> {
        self.summons.attackers.iter().map(|&(id, _)| id).collect()
    }

    /// Whether one of the dead may strike `a`: anything that fights, or whatever they're after,
    /// which the player struck or sent them at; never townsfolk, merchants or pets, nor the
    /// beasts of the wild the player's let be
    pub(in crate::combat) fn fair_game(&self, a: &Animal) -> bool {
        hostile(a) || (a.health > 0.0 && !a.look.is_pet() && self.summons.on.is_some_and(|(id, _)| id == a.id))
    }

    /// How much animal `i` is to be gone for by the dead, if at all: SENT, FOCUS, ATTACKER,
    /// CLOSING_IN or NEAREST, the player's feet at `feet`, `held` if it's what they're already after, which they
    /// let go of only further off
    fn tier_of(&self, arena: &Arena, i: usize, feet: Vector3<f64>, held: bool) -> Option<u8> {
        let a = &arena.animals[i];
        if a.health <= 0.0 || a.look.is_pet() {
            return None;
        }
        if self.ordered.is_some_and(|(id, left)| id == a.id && left > 0.0) {
            return Some(SENT);
        }
        let d = (a.position - feet).norm() / arena.view.vpm;
        let slack = if held { LEASH_SLACK } else { 1.0 };
        if self.summons.focus.is_some_and(|(id, _, last)| id == a.id && self.time - last < FOCUS_SECONDS) && d < FOCUS_LEASH * slack {
            return Some(FOCUS);
        }
        if !hostile(a) {
            return None;
        }
        let attacking = self.summons.attackers.iter().any(|&(id, at)| id == a.id && self.time - at < ATTACKER_SECONDS);
        if attacking && d < ATTACKER_LEASH * slack {
            return Some(ATTACKER);
        }
        if a.boss.as_ref().is_some_and(|b| b.engaged) && d < CLOSING * slack {
            return Some(CLOSING_IN);
        }
        if roused(a) && d < LEASH * slack {
            return Some(NEAREST);
        }
        None
    }

    /// What the dead are to go after this frame, all of them together: what's most to be gone
    /// for, see SENT, kept on till it dies, gets away or something more to be gone for comes up
    pub(in crate::combat) fn choose_prey(&mut self, arena: &Arena, dt: f64) {
        let vpm = arena.view.vpm;
        let feet = arena.player.unwrap_or(arena.view.eye - Vector3::z() * 1.6 * vpm);
        // Which way the player's going, for where the dead keep behind them, eased round so they
        // aren't swinging about at every turn
        let moved = self.summons.last_feet.map_or(Vector3::zeros(), |last| feet - last).xy();
        self.summons.last_feet = Some(feet);
        let facing = || {
            let f = arena.view.forward();
            f.y.atan2(f.x)
        };
        let heading = self.summons.heading.get_or_insert_with(facing);
        if moved.norm() > 1.0 * vpm * dt && moved.norm() < 30.0 * vpm * dt.max(1.0 / 30.0) {
            let to = moved.y.atan2(moved.x);
            let turn = (to - *heading + PI).rem_euclid(TAU) - PI;
            *heading += turn * (3.0 * dt).min(1.0);
        }
        // What the player struck since: what they're after if it's among it, as a blow sweeping
        // through a crowd strikes it too, otherwise whichever of it was nearest the crosshair;
        // and not off what they struck last till it's been HOLD seconds
        let struck = std::mem::take(&mut self.summons.struck);
        let on = self.summons.on.map(|(id, _)| id);
        if !struck.is_empty() {
            let forward = arena.view.forward();
            let pick = if on.is_some_and(|id| struck.contains(&id)) {
                on
            } else {
                struck
                    .iter()
                    .filter_map(|&id| arena.animals.iter().find(|a| a.id == id))
                    .map(|a| ((a.center(vpm as u32) - arena.view.eye).try_normalize(1e-6).map_or(-1.0, |w| w.dot(&forward)), a.id))
                    .max_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id)
            };
            if let Some(id) = pick {
                let now = self.time;
                self.summons.focus = match self.summons.focus {
                    Some((was, since, _)) if was == id => Some((id, since, now)),
                    Some((was, since, last)) if now - since < HOLD && arena.animals.iter().any(|a| a.id == was && a.health > 0.0) => Some((was, since, last)),
                    _ => Some((id, now, now)),
                };
            }
        }
        let now = self.time;
        self.summons.attackers.retain(|&(_, at)| now - at < ATTACKER_SECONDS);
        if self.summons.focus.is_some_and(|(_, _, last)| now - last >= FOCUS_SECONDS) {
            self.summons.focus = None;
        }
        if !self.familiars.iter().any(|f| f.risen.is_some()) {
            self.summons.prey = None;
            self.summons.on = None;
            self.summons.mark = None;
            return;
        }
        // The most to be gone for there is: of those that struck the player, the one that did
        // last; otherwise the nearest
        let mut best: Option<(u8, f64, usize)> = None;
        for i in 0..arena.animals.len() {
            let Some(tier) = self.tier_of(arena, i, feet, false) else { continue };
            let a = &arena.animals[i];
            let d = (a.position - feet).norm() / vpm;
            let score = match tier {
                ATTACKER => self.summons.attackers.iter().find(|&&(id, _)| id == a.id).map_or(d, |&(_, at)| now - at),
                _ => d,
            };
            if best.is_none_or(|(t, s, _)| tier < t || (tier == t && score < s)) {
                best = Some((tier, score, i));
            }
        }
        let current = on.and_then(|id| arena.animals.iter().position(|a| a.id == id)).and_then(|i| self.tier_of(arena, i, feet, true).map(|t| (i, t)));
        let chosen = match (current, best) {
            (Some((i, t)), Some((bt, _, b))) if bt < t && b != i => Some((b, bt)),
            (Some(c), _) => Some(c),
            (None, b) => b.map(|(t, _, i)| (i, t)),
        };
        self.summons.prey = chosen;
        self.summons.on = chosen.map(|(i, t)| (arena.animals[i].id, t));
        self.summons.mark = chosen.map(|(i, _)| Combat::over(arena, i));
    }

    /// The mark over what the dead are after: three motes of the dark that raised them going
    /// round over its head, faint, so it's seen what they'll go for without it taking the eye
    pub(in crate::combat) fn mark_instances(&self, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let Some(at) = self.summons.mark else {
            return Vec::new();
        };
        let vpm = view.vpm;
        let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Void)));
        // A little bigger further off, so it reads, but never much
        let far = ((at - view.eye).norm() / vpm / 12.0).clamp(1.0, 2.5).sqrt();
        (0..3)
            .map(|k| {
                let a = self.time * 2.4 + k as f64 * TAU / 3.0;
                let p = at + Vector3::new(a.cos(), a.sin(), 0.0) * 0.28 * far * vpm;
                (p, place(model, ball, MODEL_RES, Vector3::zeros(), p, UnitQuaternion::identity(), vpm, 0.045 * far, MOVING))
            })
            .collect()
    }

    /// What one of the dead goes after this frame, by index, and how much it's to be gone for
    pub(in crate::combat) fn prey(&self) -> Option<(usize, u8)> {
        self.summons.prey
    }

    /// Where the `slot`th of the dead keeps by the player with nothing to fight, on the ground:
    /// round behind them the way they've been going, further out for the colossus
    pub(in crate::combat) fn guard_spot(&self, arena: &Arena, slot: u8, big: bool) -> Vector3<f64> {
        let vpm = arena.view.vpm;
        let feet = arena.player.unwrap_or(arena.view.eye - Vector3::z() * 1.6 * vpm);
        let (turn, out) = GUARD[slot as usize % GUARD.len()];
        let out = out + (slot as usize / GUARD.len()) as f64 * 1.4 + if big { 1.6 } else { 0.0 };
        let a = self.summons.heading.unwrap_or(0.0) + PI + turn;
        Combat::ground_below(arena, feet + Vector3::new(a.cos(), a.sin(), 0.0) * out * vpm)
    }

    /// The ground under `at`, however far down, or the top of the water over it; `at` itself if
    /// there's none
    pub(in crate::combat) fn ground_below(arena: &Arena, at: Vector3<f64>) -> Vector3<f64> {
        let vpm = arena.view.vpm;
        let from = at + Vector3::z() * 1.0 * vpm;
        let to = Vector3::new(at.x, at.y, 0.0);
        let ground = solid_along(arena.world, from, to).map_or(at, |t| from + (to - from) * t);
        Combat::floor_at(arena, ground)
    }

    /// Which of the places round the player the next of the dead raised keeps, the first one none
    /// of those up keeps
    pub(in crate::combat) fn free_slot(&self) -> u8 {
        (0..u8::MAX).find(|&k| !self.familiars.iter().any(|f| f.risen.is_some_and(|r| r.slot == k))).unwrap_or(0)
    }

    /// One of the dead, `f`, left too far behind the player, after a teleport, through a gate, or
    /// with the player gone on too fast: climbing up out of the ground beside them again, its
    /// place by them, true if it did. Not while it's still climbing out or crumbling.
    pub(in crate::combat) fn rejoin(&mut self, arena: &Arena, f: &mut Familiar, r: &mut Risen) -> bool {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        if r.age < RISE_SECONDS || f.life <= CRUMBLE_SECONDS {
            return false;
        }
        let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
        let off = f.position - feet;
        // Flying, the player's far above what follows them on the ground, and gone on too fast
        // for it to keep up: let further behind, so it isn't climbing up out of the ground under
        // them over and over, and up beside them once they land
        let (far, reach) = if arena.player.is_some() { (off.norm(), REJOIN) } else { (off.xy().norm(), REJOIN * 2.0) };
        let [sx, sy, _] = arena.world.size.map(f64::from);
        let outside = f.position.x < 0.0 || f.position.y < 0.0 || f.position.x >= sx || f.position.y >= sy;
        if far < reach * vpm && !outside {
            return false;
        }
        let void = Element::look(Some(Element::Void));
        if !outside && (f.position - arena.view.eye).norm() < 90.0 * vpm {
            self.sparks(f.position + up * 0.6 * vpm, 8, 1.5, void, 0.4, 0.5);
        }
        // Its place by the player, or where they stand if that's off a ledge or up a wall
        let spot = self.guard_spot(arena, r.slot, r.look == Look::GraveBrute);
        let at = if arena.player.is_some() && (spot.z - feet.z).abs() > 2.5 * vpm { feet } else { spot };
        f.position = if r.kind == Minion::Wraith { at + up * WRAITH_HOVER * vpm } else { at };
        r.age = 0.0;
        r.swing = None;
        r.moving = false;
        r.post = if r.kind == Minion::Wraith { Vector3::zeros() } else { at };
        f.wait = f.wait.max(RISE_SECONDS);
        self.sparks(at + up * 0.2 * vpm, 10, 2.4, void, 0.6, 0.6);
        for _ in 0..4 {
            let v = (self.any_direction() + up * 1.4) * 2.5 * vpm;
            self.drop(at + up * 0.1 * vpm, v, DROP_MUD, 1.0);
        }
        true
    }

    /// One of the dead with nothing to fight going back to its place by the player, `slot`,
    /// walking at `speed` meters a second, hurrying when the player's got ahead; standing there
    /// till it's GUARD_SLACK off it
    pub(in crate::combat) fn keep_by(&mut self, arena: &Arena, f: &mut Familiar, r: &mut Risen, speed: f64, dt: f64) {
        let vpm = arena.view.vpm;
        let spot = self.guard_spot(arena, r.slot, r.look == Look::GraveBrute);
        let d = (spot - f.position).xy().norm();
        if !r.moving && d < GUARD_SLACK * vpm {
            f.position = Combat::floor_at(arena, f.position);
            return;
        }
        let feet = arena.player.unwrap_or(arena.view.eye - Vector3::z() * 1.6 * vpm);
        let speed = hurried(speed, (f.position - feet).xy().norm() / vpm);
        let heading = r.heading;
        if self.step_towards(arena, f, r, spot, 0.25 * vpm, speed, dt) {
            // Facing the way it was going as it stops, rather than at the spot under it
            r.heading = heading;
        }
    }

    /// Room made for `n` more of the dead: the oldest of those up crumbling away, so no more than
    /// MINIONS are ever up at once
    pub(in crate::combat) fn make_room(&mut self, n: usize) {
        let up = self.familiars.iter().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS).count();
        let mut over = (up + n).saturating_sub(MINIONS);
        for f in self.familiars.iter_mut().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS) {
            if over == 0 {
                break;
            }
            f.life = CRUMBLE_SECONDS;
            over -= 1;
        }
    }

    /// The shade come down at `at`: what it was sent to raise climbing up out of the ground there
    pub(in crate::combat) fn summons_land(&mut self, arena: &mut Arena, p: &Projectile, kind: u8, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let ground = Combat::floor_at(arena, at);
        let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
        let toward = flat_of(ground - feet);
        let across = toward.cross(&up);
        let base = p.damage;
        let ring = |k: usize, n: usize, r: f64| {
            let a = k as f64 / n as f64 * TAU + 0.4;
            ground + Vector3::new(a.cos(), a.sin(), 0.0) * r * vpm
        };
        // Where each rises, as what, fighting how, for how long, how hard
        let mut raised: Vec<(Vector3<f64>, Look, Minion, f64, f64)> = match kind {
            0 => (0..PHALANX).map(|k| (ground + across * (k as f64 - (PHALANX - 1) as f64 * 0.5) * PHALANX_GAP * vpm, Look::ArmoredSkeleton, Minion::Phalanx, PHALANX_SECONDS, base * PHALANX_POWER)).collect(),
            1 => (0..ARCHERS).map(|k| (ring(k, ARCHERS, 1.3), Look::BoneArcher, Minion::Archer, ARCHER_SECONDS, base * ARCHER_POWER)).collect(),
            2 => vec![(ground, Look::GraveBrute, Minion::Colossus, COLOSSUS_SECONDS, base * COLOSSUS_POWER)],
            3 => (0..WRAITHS).map(|k| (ring(k, WRAITHS, 1.2), Look::RimeWraith, Minion::Wraith, WRAITH_SECONDS, base * WRAITH_POWER)).collect(),
            _ => self.graves.take_near(ground, CORPSE_REACH * vpm, CORPSES).into_iter().map(|(at, look)| (at, risen_as(look), Minion::Corpse, CORPSE_SECONDS, base * CORPSE_POWER)).collect(),
        };
        // Too few of the slain lying there: the long dead rise out of the ground instead
        while kind == 4 && raised.len() < CORPSES_LEAST {
            let k = raised.len();
            raised.push((ring(k, CORPSES_LEAST, 1.0), if k % 2 == 0 { Look::Zombie } else { Look::ArmoredZombie }, Minion::Corpse, CORPSE_SECONDS, base * CORPSE_POWER));
        }
        self.make_room(raised.len());
        let profile = p.profile.clone();
        for (k, (spot, look, minion, life, damage)) in raised.into_iter().enumerate() {
            let at = Combat::floor_at(arena, spot);
            let heading = toward.y.atan2(toward.x) + (random(&mut self.rng) - 0.5) * 0.5;
            let slot = self.free_slot();
            self.familiars.push(Familiar {
                position: at,
                life: life + k as f64 * 0.2,
                wait: RISE_SECONDS + 0.15 * k as f64,
                profile: profile.clone(),
                damage,
                phantom: false,
                risen: Some(Risen { look, heading, phase: 0.0, moving: false, age: -0.15 * k as f64, swing: None, kind: minion, post: if minion == Minion::Wraith { Vector3::zeros() } else { at }, slot }),
            });
            self.sparks(at + up * 0.2 * vpm, 10, 2.4, Element::look(Some(Element::Void)), 0.6, 0.6);
            for _ in 0..4 {
                let v = (self.any_direction() + up * 1.4) * 2.5 * vpm;
                self.drop(at + up * 0.1 * vpm, v, DROP_MUD, 1.0);
            }
        }
        let big = kind == 2;
        self.blasts.push(Blast { position: ground + up * 0.2 * vpm, look: Element::look(Some(Element::Void)), radius: if big { 2.0 } else { 1.4 }, life: 0.6, lifetime: 0.6 });
        self.flashes.push(Flash { position: ground + up * vpm, color: [0.6, 0.12, 0.5], reach: 9.0, life: 0.5, lifetime: 0.5 });
        self.cues.push(Cue::at(Sound::Wail, ground, 0.9).pitched(if big { 0.45 } else { 0.65 }));
        self.cues.push(Cue::at(Sound::Quake, ground, if big { 0.9 } else { 0.5 }).pitched(1.2));
        self.shake = self.shake.max(if big { 0.4 } else { 0.2 });
    }

    /// How a shade sent to raise the dead is drawn: a dark orb, blood in its heart, a ring of
    /// shadow turning round it
    pub(in crate::combat) fn calling_instances(&self, p: &Projectile, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        // Smaller right by the eye as it leaves the hand, so it never fills the view
        let near = ((p.position - eye).norm() / vpm / 4.0).clamp(0.3, 1.0);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt() * near;
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 6.0);
        let void = Element::look(Some(Element::Void));
        let mut out = Vec::new();
        for (look, size) in [(void, 0.18), (Element::look(Some(Element::Blood)), 0.08)] {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            out.push((eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size * far, MOVING)));
        }
        out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.35 * far, void, 0.6 * far, self.time * 8.0));
        out
    }

    /// One of a necromancer's summons going on, after it's climbed out of the ground and before it
    /// crumbles: each fighting its own way
    pub(in crate::combat) fn walk_minion(&mut self, arena: &mut Arena, f: &mut Familiar, dt: f64) {
        let Some(r) = f.risen else { return };
        match r.kind {
            Minion::Wraith => self.fly_wraith(arena, f, r, dt),
            Minion::Archer => self.walk_archer(arena, f, r, dt),
            Minion::Colossus => self.walk_bones(arena, f, r, dt, COLOSSUS_SPEED, 2.0, COLOSSUS_BLOW),
            _ => self.walk_bones(arena, f, r, dt, BONES_SPEED, BONES_REACH, BONES_BLOW),
        }
    }

    /// What one of the dead, `f`, goes after: what they're all after, see choose_prey(); but a
    /// phalanx's skeleton stands its ground, going only after what they were sent at, what the
    /// player struck or what's attacking them, and otherwise the nearest thing that fights within
    /// PHALANX_GUARD of where it stands
    fn minion_prey(&self, arena: &Arena, f: &Familiar, r: &Risen) -> Option<usize> {
        let vpm = arena.view.vpm;
        let shared = self.prey();
        if r.kind != Minion::Phalanx {
            return shared.map(|(i, _)| i);
        }
        if let Some((i, tier)) = shared
            && tier <= CLOSING_IN
        {
            return Some(i);
        }
        (0..arena.animals.len())
            .filter(|&i| hostile(&arena.animals[i]) && roused(&arena.animals[i]) && (arena.animals[i].position - r.post).xy().norm() < PHALANX_GUARD * vpm)
            .map(|i| ((arena.animals[i].position - f.position).xy().norm(), i))
            .filter(|&(d, _)| d < MINION_HUNT * vpm)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, i)| i)
    }

    /// Walking `f` towards `goal` on the ground at `speed` meters a second till it's within `stop`
    /// voxels of it, facing it: true if it's there
    fn step_towards(&mut self, arena: &Arena, f: &mut Familiar, r: &mut Risen, goal: Vector3<f64>, stop: f64, speed: f64, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let to = (goal - f.position).xy();
        let d = to.norm();
        if d > 1e-6 {
            r.heading = to.y.atan2(to.x);
        }
        if d <= stop {
            r.moving = false;
            return true;
        }
        let way = to / d;
        let step = (speed * vpm * dt).min(d - stop);
        f.position = Combat::floor_at(arena, f.position + Vector3::new(way.x, way.y, 0.0) * step);
        r.phase = (r.phase + step / (1.3 * vpm * (speed / BONES_SPEED).max(0.6))).fract();
        r.moving = true;
        false
    }

    /// A skeleton of the phalanx, or the colossus, going on: walking at what it's after and
    /// striking it, a phalanx's going back to stand its ground where it rose when there's nothing
    /// near it, or after the player to stand it again where they stop once they've left it well
    /// behind, the colossus keeping by the player; the colossus's blow striking all round where it
    /// lands and shaking the ground
    #[allow(clippy::too_many_arguments)]
    fn walk_bones(&mut self, arena: &mut Arena, f: &mut Familiar, mut r: Risen, dt: f64, speed: f64, reach: f64, blow: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let colossus = r.kind == Minion::Colossus;
        // The ground round the colossus heaving as it stands up out of it, throwing what's there
        if colossus && r.age - dt < RISE_SECONDS {
            let profile = f.profile.clone();
            let near: Vec<usize> = Combat::foes_round(arena, f.position + up * 0.5 * vpm, COLOSSUS_ROUND).into_iter().filter(|&i| self.fair_game(&arena.animals[i])).collect();
            for i in near {
                let at = arena.animals[i].center(vpm as u32);
                self.strike(arena, &profile, i, Blow { damage: f.damage * 0.6, crit: false, heavy: true, from: f.position, depth: 1, element: profile.element.or(Some(Element::Void)) });
                self.sparks(at, 6, 3.0, Element::look(None), 1.0, 0.4);
                let a = &mut arena.animals[i];
                if a.health > 0.0 {
                    a.status.staggered = a.status.staggered.max(0.6);
                    if a.boss.is_some() {
                        a.status.limit_for_boss();
                    }
                }
            }
            for _ in 0..8 {
                let v = (self.any_direction() * 2.5 + up * 3.0) * vpm;
                self.drop(f.position + up * 0.3 * vpm, v, DROP_MUD, 1.1);
            }
            if (f.position - arena.view.eye).norm() > 3.0 * vpm {
                self.swells.push(Swell { model: WAVE, position: f.position, radius: COLOSSUS_ROUND * WAVE_SWELL, life: 0.5, lifetime: 0.5 });
            }
            self.cues.push(Cue::at(Sound::Quake, f.position, 0.9).pitched(0.7));
            self.shake = self.shake.max(0.3);
        }
        if let Some(s) = r.swing {
            let next = s + dt / blow;
            if s < LANDS && next >= LANDS {
                let ahead = Vector3::new(r.heading.cos(), r.heading.sin(), 0.0);
                let lands = f.position + ahead * reach * 0.8 * vpm + up * 0.9 * vpm;
                let round = if colossus { COLOSSUS_ROUND } else { 0.9 };
                let struck: Vec<usize> = Combat::foes_round(arena, lands, round).into_iter().filter(|&i| self.fair_game(&arena.animals[i])).take(if colossus { 6 } else { 1 }).collect();
                let profile = f.profile.clone();
                for i in struck {
                    let at = arena.animals[i].center(vpm as u32);
                    self.strike(arena, &profile, i, Blow { damage: f.damage, crit: false, heavy: colossus, from: f.position + up * vpm, depth: 1, element: profile.element.or(Some(Element::Void)) });
                    self.heal += f.damage * RISEN_DRAIN * if colossus { 0.5 } else { 1.0 };
                    self.sparks(at, 8, 3.0, Element::look(Some(Element::Blood)), 0.6, 0.35);
                    let a = &mut arena.animals[i];
                    if a.health > 0.0 {
                        a.status.staggered = a.status.staggered.max(if colossus { 0.8 } else { 0.4 });
                        if a.boss.is_some() {
                            a.status.limit_for_boss();
                        } else if colossus {
                            let away = (a.position - lands).xy().try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.5));
                            a.status.push = away * 6.0 * vpm;
                        }
                    }
                    self.cues.push(Cue::at(Sound::Hit, at, 0.6).pitched(if colossus { 0.6 } else { 0.8 }));
                }
                if colossus {
                    let ground = Combat::floor_at(arena, lands);
                    for _ in 0..5 {
                        let v = (self.any_direction() * 2.0 + up * 2.5) * vpm;
                        self.drop(ground + up * 0.2 * vpm, v, DROP_MUD, 1.0);
                    }
                    if (ground - arena.view.eye).norm() > 3.0 * vpm {
                        self.swells.push(Swell { model: WAVE, position: ground, radius: COLOSSUS_ROUND * WAVE_SWELL, life: 0.5, lifetime: 0.5 });
                    }
                    self.sparks(ground, 14, 4.0, Element::look(None), 1.2, 0.5);
                    self.cues.push(Cue::at(Sound::Slam, ground, 0.9).pitched(0.6));
                    let near = (1.0 - (ground - arena.view.eye).norm() / vpm / 20.0).max(0.0);
                    self.shake = self.shake.max(0.3 * near);
                } else {
                    self.cues.push(Cue::at(Sound::Swing, f.position, 0.4).pitched(0.8));
                }
            }
            r.swing = (next < 1.0).then_some(next);
            r.moving = false;
            f.risen = Some(r);
            return;
        }
        let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
        match self.minion_prey(arena, f, &r) {
            Some(i) => {
                let a = &arena.animals[i];
                let goal = a.position;
                let stop = (reach + a.body().1 * 0.7) * vpm;
                let speed = hurried(speed, (f.position - feet).xy().norm() / vpm);
                if self.step_towards(arena, f, &mut r, goal, stop, speed, dt) && f.wait <= 0.0 {
                    r.swing = Some(0.0);
                    f.wait = blow + if colossus { 0.4 } else { 0.35 };
                }
            }
            // After the player once they've left where it stands its ground well behind, to stand
            // it again where it stops by them
            None if !colossus && (r.post - feet).xy().norm() > PHALANX_LEASH * vpm => {
                self.keep_by(arena, f, &mut r, speed, dt);
                if !r.moving {
                    r.post = f.position;
                }
            }
            // Back to where it stands its ground
            None if !colossus => {
                let post = r.post;
                if self.step_towards(arena, f, &mut r, post, 0.3 * vpm, speed, dt) {
                    f.position = Combat::floor_at(arena, f.position);
                }
            }
            None => self.keep_by(arena, f, &mut r, speed, dt),
        }
        f.risen = Some(r);
    }

    /// A skeleton archer going on: keeping back from what's after it, drawing and loosing an arrow
    /// at it every ARCHER_PACE, walking up till it's in reach, keeping by the player when there's
    /// nothing
    fn walk_archer(&mut self, arena: &mut Arena, f: &mut Familiar, mut r: Risen, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let prey = self.minion_prey(arena, f, &r);
        if let Some(s) = r.swing {
            let next = s + dt / ARCHER_DRAW;
            if s < LANDS && next >= LANDS
                && let Some(i) = prey
            {
                let from = f.position + Vector3::new(r.heading.cos(), r.heading.sin(), 0.0) * 0.4 * vpm + up * 1.45 * vpm;
                let to = arena.animals[i].center(vpm as u32);
                let way = lead(from, to, ARROW_SPEED, ARROW_GRAVITY, vpm);
                self.launch(&f.profile, Some(Missile::arrow(0, true)), Form::Bolt, from, way * ARROW_SPEED * vpm, f.damage, ARROW_GRAVITY);
                // Its own, not the player's, so what it strikes isn't taken for what the player's
                // going for
                if let Some(p) = self.projectiles.last_mut() {
                    p.minion = true;
                }
                self.cues.push(Cue::at(Sound::Twang, from, 0.5).pitched(1.1));
            }
            r.swing = (next < 1.0).then_some(next);
            r.moving = false;
            f.risen = Some(r);
            return;
        }
        match prey {
            Some(i) => {
                let a = &arena.animals[i];
                let off = (a.position - f.position).xy();
                let d = off.norm() / vpm;
                if d > 1e-6 {
                    r.heading = off.y.atan2(off.x);
                }
                // Stepping back from what's come too near, and walking up to what's out of reach
                if d < ARCHER_BACK + a.body().1 {
                    let away = f.position - Vector3::new(off.x, off.y, 0.0) / d;
                    let heading = r.heading;
                    self.step_towards(arena, f, &mut r, away, 0.0, BONES_SPEED * 0.8, dt);
                    r.heading = heading;
                } else if d > ARCHER_RANGE {
                    let goal = a.position;
                    let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
                    let speed = hurried(BONES_SPEED, (f.position - feet).xy().norm() / vpm);
                    self.step_towards(arena, f, &mut r, goal, ARCHER_RANGE * 0.8 * vpm, speed, dt);
                } else {
                    r.moving = false;
                    if f.wait <= 0.0 {
                        r.swing = Some(0.0);
                        f.wait = ARCHER_PACE;
                    }
                }
            }
            None => self.keep_by(arena, f, &mut r, BONES_SPEED, dt),
        }
        f.risen = Some(r);
    }

    /// A wraith going on: hovering, circling what it's after, then diving through it, striking and
    /// chilling it as it passes and flying on and up, resting a moment before the next; circling
    /// the player when there's nothing
    fn fly_wraith(&mut self, arena: &mut Arena, f: &mut Familiar, mut r: Risen, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let mut v = r.post;
        let prey = self.minion_prey(arena, f, &r);
        let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
        let fly = hurried(WRAITH_FLY, (f.position - feet).xy().norm() / vpm);
        match prey {
            Some(i) if f.wait <= 0.0 => {
                let center = arena.animals[i].center(vpm as u32);
                let to = center - f.position;
                let dive = to.try_normalize(1e-6).unwrap_or_else(Vector3::x) * WRAITH_DIVE * vpm;
                v = v.lerp(&dive, (8.0 * dt).min(1.0));
                if to.norm() < (0.7 + arena.animals[i].body().1) * vpm {
                    let profile = f.profile.clone();
                    self.strike(arena, &profile, i, Blow { damage: f.damage, crit: false, heavy: false, from: f.position, depth: 1, element: Some(Element::Void) });
                    let s = &mut arena.animals[i].status;
                    s.chill = (s.chill + 0.35).min(1.0);
                    s.slowed = s.slowed.max(1.2);
                    s.slow = s.slow.max(0.3);
                    self.sparks(center, 10, 3.0, Element::look(Some(Element::Frost)), 0.2, 0.4);
                    self.cues.push(Cue::at(Sound::Wail, center, 0.4).pitched(1.5));
                    f.wait = WRAITH_REST;
                    v = (to.normalize() * 0.6 + up * 0.6).normalize() * WRAITH_DIVE * 0.8 * vpm;
                }
            }
            _ => {
                let anchor = match prey {
                    Some(i) => arena.animals[i].position,
                    None => feet,
                };
                // Each round at its own place
                let a = self.time * 1.3 + r.slot as f64 * TAU / 3.0 + r.slot as f64 * 0.37;
                let ground = Combat::floor_at(arena, anchor);
                let goal = Vector3::new(anchor.x, anchor.y, ground.z) + Vector3::new(a.cos(), a.sin(), 0.0) * 3.5 * vpm + up * (WRAITH_HOVER + 0.6) * vpm;
                let want = (goal - f.position) * 2.0;
                let want = if want.norm() > fly * vpm { want.normalize() * fly * vpm } else { want };
                v = v.lerp(&want, (3.0 * dt).min(1.0));
            }
        }
        f.position += v * dt;
        let ground = Combat::floor_at(arena, f.position);
        f.position.z = f.position.z.max(ground.z + 0.4 * vpm);
        r.post = v;
        if v.xy().norm() > 0.1 * vpm {
            r.heading = v.y.atan2(v.x);
        }
        r.moving = true;
        r.phase = (r.phase + dt * 1.5).fract();
        if random(&mut self.rng) < dt * 20.0 {
            let at = f.position + up * (0.5 + random(&mut self.rng)) * vpm;
            self.fine(at, -v * 0.2, Element::look(Some(Element::Void)), 0.4, 0.0);
        }
        f.risen = Some(r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::Library;

    /// Open ground, 48 m across, four meters of earth under four of sky
    fn field() -> World {
        let size = [384u32, 384, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 8;
        world
    }

    const VPM: f64 = 8.0;

    fn feet() -> Vector3<f64> {
        Vector3::new(24.0, 10.0, 4.0) * VPM
    }

    /// Using `part`'s power as a player would at a pack of `pack` ahead, watched `seconds`: how
    /// many of the pack were hurt, the most of the dead up at once, what they were, and the
    /// combat after
    fn raised(part: &str, pack: &[Look], seconds: f64, again: Option<f64>) -> (usize, usize, Vec<(Look, Minion)>, Combat) {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part(part, None).unwrap_or_else(|| panic!("no weapon with {part}"));
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.15), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 };
        let mut animals = crate::animals::spawn_pack(&world, &dry, pack, feet() + Vector3::new(0.0, 9.0 * VPM, 0.0), crate::animals::PACK_SPREAD, 5);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 60.0;
        let (mut most, mut kinds) = (0, Vec::new());
        for frame in 0..(seconds / dt) as usize {
            let t = frame as f64 * dt - 0.4;
            let mut controls = combat.power_controls(&weapon, t, dt);
            if let Some(at) = again
                && t >= at
                && t < at + dt * 1.5
            {
                controls.heavy = true;
                controls.aiming = true;
            }
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
            let up: Vec<(Look, Minion)> = combat.familiars.iter().filter_map(|f| f.risen.map(|r| (r.look, r.kind))).collect();
            most = most.max(up.len());
            for k in up {
                if !kinds.contains(&k) {
                    kinds.push(k);
                }
            }
        }
        (animals.iter().filter(|a| a.health < a.max_health).count(), most, kinds, combat)
    }

    /// Each summoning power raises what it should, as many as it should, and they fight: the
    /// phalanx's skeletons, the archers' arrows, the colossus's blows and the wraiths' swoops all
    /// hurt the pack
    #[test]
    fn each_raises_its_own_and_they_fight() {
        for (part, look, kind, count) in [
            ("phalanx_binding", Look::ArmoredSkeleton, Minion::Phalanx, PHALANX),
            ("archers_crystal", Look::BoneArcher, Minion::Archer, ARCHERS),
            ("colossus_focus", Look::GraveBrute, Minion::Colossus, 1),
            ("wraith_shaft", Look::RimeWraith, Minion::Wraith, WRAITHS),
            ("corpse_crystal", Look::Zombie, Minion::Corpse, CORPSES_LEAST),
        ] {
            let (hurt, most, kinds, _) = raised(part, &[Look::Bandit; 3], 8.0, None);
            assert!(kinds.contains(&(look, kind)), "{part} raised {kinds:?}");
            assert_eq!(most, count, "{part} raised {most}");
            assert!(hurt >= 1, "{part}'s dead hurt none of the pack");
        }
    }

    /// The archers shoot arrows to be seen flying, rather than striking from afar unseen
    #[test]
    fn archers_shoot_arrows() {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part("archers_crystal", None).unwrap();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.15), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 };
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 2], feet() + Vector3::new(0.0, 20.0 * VPM, 0.0), crate::animals::PACK_SPREAD, 5);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 60.0;
        let mut arrows = 0;
        for frame in 0..(6.0 / dt) as usize {
            let t = frame as f64 * dt - 0.4;
            let controls = combat.power_controls(&weapon, t, dt);
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
            arrows = arrows.max(combat.projectiles.iter().filter(|p| p.arrow && p.shot == Shot::Arrow).count());
        }
        assert!(arrows >= 1, "no arrows were seen flying");
    }

    /// The slain rise as what they were, and no more than MINIONS of the dead are ever up at once
    #[test]
    fn the_slain_rise_as_themselves_and_no_more_than_eight() {
        let library = Library::load();
        let world = field();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.15), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 };
        // Three wolves lying slain where the shade comes down
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Wolf; 3], feet() + Vector3::new(0.0, 9.0 * VPM, 0.0), 1.5, 5);
        for a in &mut animals {
            a.health = 0.0;
        }
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let corpses = library.with_part("corpse_crystal", None).unwrap();
        let phalanx = library.with_part("phalanx_binding", None).unwrap();
        let dt = 1.0 / 60.0;
        let mut most = 0;
        for (weapon, from) in [(&corpses, 0.0), (&phalanx, 2.5), (&corpses, 5.0)] {
            combat.power.cooldown = 0.0;
            for frame in 0..(2.8 / dt) as usize {
                let t = frame as f64 * dt + from;
                let controls = combat.power_controls(weapon, t - from - 0.5, dt);
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
                combat.update(&mut arena, Some((0, weapon)), None, controls, dt);
                let up = combat.familiars.iter().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS).count();
                most = most.max(up);
            }
            if from == 0.0 {
                let wolves = combat.familiars.iter().filter(|f| f.risen.is_some_and(|r| r.look == Look::Wolf && r.kind == Minion::Corpse)).count();
                assert_eq!(wolves, 3, "the three slain wolves rose as wolves");
            }
        }
        assert!(most <= MINIONS, "{most} of the dead were up at once");
        assert!(most >= 7, "only {most} were ever up");
    }

    /// Right click while it comes round sends every one of them at what's aimed at
    #[test]
    fn right_click_sends_them_at_what_is_aimed_at() {
        let (_, _, _, combat) = raised("phalanx_binding", &[Look::Bandit; 2], 4.0, Some(3.0));
        assert!(combat.ordered.is_some_and(|(_, left)| left > 0.0), "they weren't sent at anything");
        assert!(combat.power.cooldown > 0.0);
    }

    /// A fight on open ground `meters` across, the player standing at `feet` looking along
    /// `facing` radians round from +y, with the dead raised by the player and what's about
    struct Fight {
        world: World,
        animals: Vec<Animal>,
        combat: Combat,
        feet: Vector3<f64>,
        facing: f64,
        profile: Rc<Profile>,
    }

    impl Fight {
        fn new(meters: u32) -> Fight {
            let size = [meters * 8, meters * 8, 64];
            let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
            let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
            occupancy[..tx * ty * 4].fill(u128::MAX);
            let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
            world.voxels_per_meter = 8;
            let profile = Library::load().with_part("phalanx_binding", None).unwrap().profile.clone();
            let mut combat = Combat::new(1, 0);
            combat.mana = 1000.0;
            let feet = Vector3::new(meters as f64 * 0.5, meters as f64 * 0.5, 4.0) * VPM;
            Fight { world, animals: Vec::new(), combat, feet, facing: 0.0, profile }
        }

        fn view(&self) -> View {
            let rotation = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.facing) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.1);
            View { eye: self.feet + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation, tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 }
        }

        /// `look` standing `at` meters from the player, along x and y, by index
        fn put(&mut self, look: Look, at: [f64; 2]) -> usize {
            let dry = |_: f64, _: f64| f64::NEG_INFINITY;
            let near = self.feet + Vector3::new(at[0], at[1], 0.0) * VPM;
            let mut one = crate::animals::spawn_pack(&self.world, &dry, &[look], near, 0.1, 7 + self.animals.len() as u32);
            assert_eq!(one.len(), 1, "no {look:?} put down");
            one[0].position = Vector3::new(near.x, near.y, 4.0 * VPM);
            self.animals.append(&mut one);
            self.animals.len() - 1
        }

        /// One of the dead, `kind`, risen already as `look`, `at` meters from the player
        fn raise(&mut self, look: Look, kind: Minion, at: [f64; 2]) {
            let position = self.feet + Vector3::new(at[0], at[1], if kind == Minion::Wraith { WRAITH_HOVER } else { 0.0 }) * VPM;
            let slot = self.combat.free_slot();
            let post = if kind == Minion::Wraith { Vector3::zeros() } else { position };
            let risen = Risen { look, heading: 0.0, phase: 0.0, moving: false, age: RISE_SECONDS, swing: None, kind, post, slot };
            self.combat.familiars.push(Familiar { position, life: 60.0, wait: 0.0, profile: self.profile.clone(), damage: 12.0, phantom: false, risen: Some(risen) });
        }

        /// One of each, eight in all, round the player
        fn raise_all(&mut self) {
            for (k, (look, kind)) in [(Look::ArmoredSkeleton, Minion::Phalanx), (Look::ArmoredSkeleton, Minion::Phalanx), (Look::BoneArcher, Minion::Archer), (Look::BoneArcher, Minion::Archer), (Look::GraveBrute, Minion::Colossus), (Look::RimeWraith, Minion::Wraith), (Look::Skeleton, Minion::Raised), (Look::Zombie, Minion::Corpse)].into_iter().enumerate() {
                let a = k as f64 * 0.8;
                self.raise(look, kind, [a.cos() * 2.5, a.sin() * 2.5]);
            }
        }

        fn step(&mut self, dt: f64) {
            let view = self.view();
            let dry = |_: f64, _: f64| f64::NEG_INFINITY;
            let mut arena = Arena { world: &self.world, animals: &mut self.animals, view: &view, player: Some(self.feet), water: &dry };
            self.combat.update(&mut arena, None, None, Controls::default(), dt);
        }

        /// The player striking animal `i` themselves
        fn strike(&mut self, i: usize) {
            let view = self.view();
            let dry = |_: f64, _: f64| f64::NEG_INFINITY;
            let profile = self.profile.clone();
            let feet = self.feet;
            let mut arena = Arena { world: &self.world, animals: &mut self.animals, view: &view, player: Some(feet), water: &dry };
            self.combat.strike(&mut arena, &profile, i, Blow { damage: 1.0, crit: false, heavy: false, from: feet, depth: 0, element: None });
        }

        /// What they're after, by index, and how much
        fn on(&self) -> Option<(usize, u8)> {
            self.combat.summons.on.and_then(|(id, tier)| self.animals.iter().position(|a| a.id == id).map(|i| (i, tier)))
        }

        fn raised(&self) -> Vec<(&Familiar, Risen)> {
            self.combat.familiars.iter().filter_map(|f| f.risen.map(|r| (f, r))).collect()
        }

        /// How far the furthest of them is from the player, in meters
        fn furthest(&self) -> f64 {
            self.raised().iter().map(|(f, _)| (f.position - self.feet).xy().norm() / VPM).fold(0.0, f64::max)
        }
    }

    const DT: f64 = 1.0 / 60.0;

    /// They keep up with the player running off, the phalanx too once it's left behind, never
    /// left so far behind they're lost, and never crumbling before their time
    #[test]
    fn they_keep_up_with_the_player() {
        let mut fight = Fight::new(160);
        fight.raise_all();
        // Sprinting off along x for six seconds, 54 m: the phalanx left standing its ground till
        // the player's well off, then coming after them
        let (mut most, mut phalanx): (f64, f64) = (0.0, 0.0);
        for _ in 0..(6.0 / DT) as usize {
            fight.feet.x += 9.0 * VPM * DT;
            fight.facing = -FRAC_PI_2;
            fight.step(DT);
            for (f, r) in fight.raised() {
                let d = (f.position - fight.feet).xy().norm() / VPM;
                if r.kind == Minion::Phalanx {
                    phalanx = phalanx.max(d);
                } else {
                    most = most.max(d);
                }
            }
            assert!(fight.raised().iter().all(|(f, r)| f.life > CRUMBLE_SECONDS && r.age >= RISE_SECONDS), "one of them crumbled, or climbed out again, as the player ran");
        }
        assert!(most < 12.0, "left {most:.1} m behind a sprinting player");
        assert!(phalanx < PHALANX_LEASH + 4.0, "the phalanx left {phalanx:.1} m behind");
        // And all round them again a few seconds after they stop
        for _ in 0..(4.0 / DT) as usize {
            fight.step(DT);
        }
        let furthest = fight.furthest();
        assert!(furthest < 6.0, "still {furthest:.1} m off once the player stopped");
        assert_eq!(fight.raised().len(), 8);
    }

    /// Left far behind, as by a teleport, or out of the world once it's moved on, they climb up
    /// out of the ground beside the player, never just gone; and they go along through a gate
    #[test]
    fn they_rise_again_beside_the_player_after_a_teleport() {
        let mut fight = Fight::new(160);
        fight.raise_all();
        fight.step(DT);
        // Teleported 60 m off
        fight.feet.y += 60.0 * VPM;
        let mut climbed = 0;
        for _ in 0..(3.0 / DT) as usize {
            fight.step(DT);
            climbed = climbed.max(fight.raised().iter().filter(|(_, r)| r.age < RISE_SECONDS).count());
        }
        assert_eq!(climbed, 8, "each climbed up out of the ground again by the player");
        let furthest = fight.furthest();
        assert!(furthest < 6.0, "{furthest:.1} m off the player after the teleport");
        // The world moved on so far they're out of it
        fight.combat.recenter(Vector3::new(130.0 * VPM, 0.0, 0.0));
        fight.feet.x -= 130.0 * VPM;
        fight.feet.x = fight.feet.x.max(10.0 * VPM);
        for _ in 0..(2.0 / DT) as usize {
            fight.step(DT);
        }
        let furthest = fight.furthest();
        assert!(furthest < 6.0 && fight.raised().len() == 8, "{furthest:.1} m off after the world moved on");
        // Through a gate: taken along, the world's let go of, and back beside the player there
        let raised = fight.combat.take_raised();
        fight.combat.clear();
        fight.combat.bring_raised(raised);
        fight.feet = Vector3::new(140.0, 30.0, 4.0) * VPM;
        for _ in 0..(2.0 / DT) as usize {
            fight.step(DT);
        }
        let furthest = fight.furthest();
        assert!(furthest < 6.0 && fight.raised().len() == 8, "{furthest:.1} m off through a gate, {} came", fight.raised().len());
    }

    /// In a busy fight, sparks and drops and shots flying all about, they're still drawn: before
    /// what's flying about them, as they were left out of what's drawn and seemed to vanish when
    /// they were kept only as near as they were
    #[test]
    fn they_are_drawn_however_busy_the_fight() {
        let mut fight = Fight::new(64);
        fight.raise_all();
        for (k, f) in fight.combat.familiars.iter_mut().enumerate() {
            let a = k as f64 * 0.8;
            f.position = fight.feet + Vector3::new(a.cos(), 1.0 + a.sin() * 0.3, 0.0) * 9.0 * VPM;
        }
        let view = fight.view();
        let up = Vector3::z();
        // A hundred and forty drops and sparks thrown up round the player and shots flying out
        for k in 0..140 {
            let at = fight.feet + Vector3::new((k % 7) as f64 - 3.0, 1.0 + (k / 7) as f64 * 0.2, 1.0) * VPM;
            fight.combat.drop(at, up * VPM, DROP_MUD, 1.0);
        }
        let profile = fight.profile.clone();
        for k in 0..30 {
            let from = view.eye + Vector3::new(k as f64 * 0.3 - 4.5, 4.0, 0.0) * VPM;
            fight.combat.launch(&profile, Some(Missile::arrow(0, true)), Form::Bolt, from, Vector3::y() * 30.0 * VPM, 1.0, ARROW_GRAVITY);
        }
        let mut drawn = fight.combat.flying_instances(&view);
        // Their bodies are drawn among the animals, ranked as if a hundredth as far off, see
        // Game::instances() and combat\raised.rs: one each. What's drawn of them among what's
        // flying about is the glow in each one's chest.
        let raised = fight.raised().len();
        assert_eq!(fight.combat.raised_bodies(&view).iter().filter(|b| b.distance < 40.0 * VPM).count(), raised, "a body each");
        let mut bodies = Vec::new();
        for (f, r) in fight.raised() {
            fight.combat.risen_glow(f, &r, &view, &mut bodies);
        }
        let near = |a: &AnimalInstance, b: &AnimalInstance| (a.sphere().0 - b.sphere().0).norm() < 1e-3;
        // As many as there's room for in a fight, see game::MAX_INSTANCES, with seventy creatures
        // about and the leaves' third taken
        let room = 40;
        drawn.sort_by(|a, b| a.0.total_cmp(&b.0));
        let kept: Vec<AnimalInstance> = drawn.iter().take(room).map(|(_, i)| *i).collect();
        let shown = bodies.iter().filter(|(_, b)| kept.iter().any(|k| near(k, b))).count();
        assert_eq!(shown, bodies.len(), "only {shown} of {} of what's drawn of them is", bodies.len());
        // As it was: kept only as near as they are, none of them would be
        let mut plain: Vec<(f64, AnimalInstance)> = drawn.iter().filter(|(_, i)| !bodies.iter().any(|(_, b)| near(i, b))).copied().collect();
        plain.extend(bodies.iter().copied());
        plain.sort_by(|a, b| a.0.total_cmp(&b.0));
        let was = bodies.iter().filter(|(_, b)| plain.iter().take(room).any(|(_, k)| near(k, b))).count();
        assert!(was < bodies.len() / 2, "{was} of them were drawn before anyway");
    }

    /// Raise Dead with as many up as there can be: the oldest crumble away for its three, rather
    /// than being gone at once
    #[test]
    fn raise_dead_makes_room_by_crumbling() {
        let library = Library::load();
        let focus = library.generate(11, library.family("skullfocus"), 0.0);
        assert_eq!(focus.profile.power, Some(Power::RaiseDead));
        let mut fight = Fight::new(64);
        fight.raise_all();
        let mut most = 0;
        let mut rose = None;
        for frame in 0..(3.0 / DT) as usize {
            let t = frame as f64 * DT - 0.3;
            let controls = fight.combat.power_controls(&focus, t, DT);
            let view = fight.view();
            let dry = |_: f64, _: f64| f64::NEG_INFINITY;
            let mut arena = Arena { world: &fight.world, animals: &mut fight.animals, view: &view, player: Some(fight.feet), water: &dry };
            fight.combat.update(&mut arena, Some((0, &focus)), None, controls, DT);
            let up = fight.raised();
            most = most.max(up.iter().filter(|(f, _)| f.life > CRUMBLE_SECONDS).count());
            // As they rise: how many there are, and how many crumbling
            if rose.is_none() && up.iter().any(|(f, r)| r.kind == Minion::Raised && f.life < 30.0) {
                rose = Some((up.len(), up.iter().filter(|(f, _)| f.life <= CRUMBLE_SECONDS).count()));
            }
        }
        assert_eq!(rose, Some((11, 3)), "as Raise Dead's rose, (all there were, crumbling)");
        assert!(most <= MINIONS, "{most} up at once");
        assert_eq!(fight.raised().iter().filter(|(f, r)| r.kind == Minion::Raised && f.life > CRUMBLE_SECONDS && f.life < 30.0).count(), 3, "Raise Dead's three rose");
    }

    /// They go for what's nearest at first, then all of them for what the player strikes, the
    /// archers shooting at it; not off it at once for something else the player strikes; and
    /// back to what's near once the player's let it be a while
    #[test]
    fn they_go_for_what_the_player_strikes() {
        let mut fight = Fight::new(64);
        let ahead = fight.put(Look::Bandit, [0.0, 8.0]);
        fight.put(Look::Bandit, [1.5, 9.0]);
        let aside = fight.put(Look::Bandit, [-11.0, 0.0]);
        let other = fight.put(Look::Bandit, [11.0, 1.0]);
        fight.raise_all();
        for _ in 0..20 {
            fight.step(DT);
        }
        // Struck by them, it's coming for the player now
        assert!(matches!(fight.on(), Some((i, NEAREST | CLOSING_IN)) if i == ahead), "the nearest first, not {:?}", fight.on());
        fight.strike(aside);
        fight.step(DT);
        assert_eq!(fight.on(), Some((aside, FOCUS)), "what the player struck");
        // Struck right after by the player too: still on the first
        fight.strike(other);
        fight.step(DT);
        assert_eq!(fight.on(), Some((aside, FOCUS)), "off what the player struck at once");
        let health = fight.animals[aside].health;
        let mut toward = 0;
        let mut shots = 0;
        let had: usize = fight.combat.projectiles.len();
        let mut seen = had;
        for _ in 0..(4.0 / DT) as usize {
            fight.step(DT);
            let n = fight.combat.projectiles.len();
            if n > seen {
                for p in &fight.combat.projectiles[seen..] {
                    if p.minion {
                        shots += 1;
                        let to = (fight.animals[aside].position - p.position).xy().normalize();
                        toward += (p.velocity.xy().normalize().dot(&to) > 0.9) as usize;
                    }
                }
            }
            seen = fight.combat.projectiles.len();
        }
        assert!(shots >= 2 && toward == shots, "the archers shot {toward} of {shots} at what the player struck");
        assert!(fight.animals[aside].health < health, "they didn't hurt what the player struck");
        assert_eq!(fight.on().map(|o| o.0), Some(aside));
        // Struck again, the other this time, once it's been a while: on that
        fight.strike(other);
        fight.step(DT);
        assert_eq!(fight.on(), Some((other, FOCUS)));
        // Let be a while, it's only what's near, or coming for the player, like the rest
        for _ in 0..((FOCUS_SECONDS + 0.5) / DT) as usize {
            fight.step(DT);
        }
        assert!(fight.on().is_some_and(|(_, tier)| tier >= CLOSING_IN), "{:?} after the player let it be", fight.on());
    }

    /// What strikes the player from behind is turned on, before what's nearer in front; what the
    /// player strikes before that; and what's sent at with right click before anything
    #[test]
    fn they_turn_on_what_strikes_the_player() {
        let mut fight = Fight::new(64);
        let ahead = fight.put(Look::Bandit, [0.0, 7.0]);
        let behind = fight.put(Look::Bandit, [0.5, -12.0]);
        fight.raise_all();
        for _ in 0..30 {
            fight.step(DT);
        }
        assert!(matches!(fight.on(), Some((i, NEAREST | CLOSING_IN)) if i == ahead), "{:?}", fight.on());
        let id = fight.animals[behind].id;
        fight.combat.blame(id);
        fight.step(DT);
        assert_eq!(fight.on(), Some((behind, ATTACKER)), "they didn't turn on what struck the player");
        let health = fight.animals[behind].health;
        for _ in 0..(4.0 / DT) as usize {
            fight.step(DT);
        }
        assert!(fight.animals[behind].health < health, "they didn't hurt what struck the player");
        // Coming on at the player, not yet having struck, is turned on too
        let mut closing = Fight::new(64);
        let ahead = closing.put(Look::Bandit, [0.0, 6.0]);
        let coming = closing.put(Look::Bandit, [0.0, -7.5]);
        closing.raise_all();
        closing.step(DT);
        assert_eq!(closing.on(), Some((ahead, NEAREST)), "{:?}", closing.on());
        closing.animals[coming].boss.as_mut().unwrap().provoke();
        closing.step(DT);
        assert_eq!(closing.on(), Some((coming, CLOSING_IN)));
        // What the player strikes before what struck them
        fight.strike(ahead);
        fight.step(DT);
        assert_eq!(fight.on(), Some((ahead, FOCUS)));
        // And what they're sent at before anything, for as long as they're sent
        fight.combat.ordered = Some((fight.animals[behind].id, ORDER_SECONDS));
        fight.step(DT);
        assert_eq!(fight.on(), Some((behind, SENT)));
        fight.strike(ahead);
        fight.step(DT);
        assert_eq!(fight.on(), Some((behind, SENT)), "taken off what they were sent at");
    }

    /// Two as near as each other, the nearer of them changing every frame, and the player striking
    /// both in turn, quickly: they don't turn back and forth between them
    #[test]
    fn they_dont_flip_flop() {
        let mut fight = Fight::new(64);
        let left = fight.put(Look::Bandit, [-7.0, 0.0]);
        let right = fight.put(Look::Bandit, [7.0, 0.0]);
        fight.raise_all();
        let mut changes = 0;
        let mut last = None;
        for frame in 0..(3.0 / DT) as usize {
            let nudge = if frame % 2 == 0 { 0.1 } else { -0.1 } * VPM;
            fight.animals[left].position.x += nudge;
            fight.animals[right].position.x += nudge;
            fight.step(DT);
            let on = fight.on().map(|o| o.0);
            changes += (last.is_some() && on != last) as usize;
            last = on;
        }
        assert_eq!(changes, 0, "they turned {changes} times between two as near");
        // Striking each in turn five times a second
        let mut when = Vec::new();
        for frame in 0..(4.0 / DT) as usize {
            if frame % 12 == 0 {
                fight.strike(if frame % 24 == 0 { left } else { right });
            }
            fight.step(DT);
            let on = fight.on().map(|o| o.0);
            if on != last {
                when.push(frame as f64 * DT);
            }
            last = on;
        }
        assert!(when.windows(2).all(|w| w[1] - w[0] > HOLD - 0.1), "they turned at {when:?}");
        assert!(when.len() <= 4, "they turned {} times in 4 s", when.len());
    }

    /// Townsfolk, merchants, pets and the beasts of the wild are left be, whatever's near; but a
    /// beast the player strikes is gone after
    #[test]
    fn they_leave_be_what_doesnt_fight() {
        let mut fight = Fight::new(64);
        let calm: Vec<usize> = [(Look::Villager, [2.0, 4.0]), (Look::Merchant, [-3.0, 3.0]), (Look::Dog, [3.0, -2.0]), (Look::Squirrel, [-2.0, -3.0]), (Look::Bear, [5.0, 5.0])].into_iter().map(|(look, at)| fight.put(look, at)).collect();
        fight.raise_all();
        for _ in 0..(6.0 / DT) as usize {
            fight.step(DT);
        }
        assert_eq!(fight.on(), None);
        for &i in &calm {
            let a = &fight.animals[i];
            assert!(a.health >= a.max_health, "they struck the {:?}", a.look);
        }
        let bear = calm[4];
        fight.strike(bear);
        let health = fight.animals[bear].health;
        for _ in 0..(4.0 / DT) as usize {
            fight.step(DT);
        }
        assert!(fight.animals[bear].health < health, "they let be the bear the player struck");
        for &i in &calm[..4] {
            let a = &fight.animals[i];
            assert!(a.health >= a.max_health, "they struck the {:?}", a.look);
        }
    }
}
