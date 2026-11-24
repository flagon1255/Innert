//! The specialist merchants as the game has them, see src\merchants.rs: keeping their stalls in the
//! towns, talked to before they're traded with, what they stock, and those living at the base,
//! coming out to their stands by the fire by day and in at night.

use super::*;
use crate::merchants::{self, Household, Resident, Trade};

/// A specialist keeping a stall: in a town, or at the base
#[derive(Clone)]
pub(super) struct Keeper {
    trade: Trade,
    name: String,
    /// What their stock's rolled from
    seed: u32,
    /// Living at the base
    home: bool,
}

/// Talking to a specialist: who, what they said last, how many times they've been asked about the
/// land, and whether they've just agreed to come to the base, to go once they're left
struct Talk {
    key: [i64; 3],
    line: String,
    asked: usize,
    invited: bool,
}

/// Seconds between looks at who should be out at the base
const DUE_SECONDS: f64 = 0.5;
/// Seconds at the least between one living at the base saying something to the player passing by,
/// and how near they have to come, in meters
const CHATTER_SECONDS: f64 = 50.0;
const CHATTER_NEAR: f64 = 4.5;
/// How near the player has to be to the base, in meters, to be told the merchants are going in
const HOME_NEAR: f64 = 60.0;
/// How near a specialist the player has to come, in meters, for the map to mark them
const KNOWN_NEAR: f64 = 60.0;

/// The specialists: who keeps which stall in this piece of the world, by its key, who's being
/// talked to, those living at the base and their stands there
#[derive(Default)]
pub(super) struct Trades {
    keepers: HashMap<[i64; 3], Keeper>,
    talk: Option<Talk>,
    household: Household,
    /// The stands at the base, where in voxels and which way they face, and whose
    stands: Vec<(Vector3<f64>, f64)>,
    stand_of: Vec<Trade>,
    due: f64,
    chatter: f64,
    rng: u32,
    vpm: f64,
}

impl Trades {
    /// Lets go of those in the world being left, as Game::leave_world() does the rest
    pub(super) fn leave_world(&mut self) {
        self.keepers.clear();
        self.talk = None;
        self.stands.clear();
        self.stand_of.clear();
    }

    pub(super) fn stands(&self) -> &[(Vector3<f64>, f64)] {
        &self.stands
    }
}

/// Where the stock of the merchant of `trade` living at the base is kept, among Game::stocks
fn home_key(trade: Trade) -> [i64; 3] {
    [-(1i64 << 40) - trade.index() as i64, 0, -91]
}

/// The floor to stand on at `at`, in voxels, about as high: the first solid ground with room over it
/// looking down from a little over there, so on the floor of a house and not up on its roof
fn floor_at(world: &World, at: Vector3<f64>) -> Option<f64> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    if at.x < 2.0 || at.y < 2.0 || at.x >= sx - 2.0 || at.y >= sy - 2.0 {
        return None;
    }
    let (x, y) = (at.x as i64, at.y as i64);
    let room = |k: i64| (1..=(1.8 * vpm) as i64).all(|j| is_passable(world.material_at(x, y, k + j)));
    let from = (at.z + 1.2 * vpm) as i64;
    (from - (3.0 * vpm) as i64..=from).rev().find(|&k| !is_passable(world.material_at(x, y, k)) && room(k)).map(|k| k as f64 + 1.0)
}

/// The lucks the weapons a specialist has are rolled with, as for Library::odds(), before a
/// realm's scaling: more of them than the travelling merchant's MERCHANT_LUCKS, and luckier
const SPECIALIST_LUCKS: [f64; 7] = [0.8, 1.0, 1.2, 1.4, 1.7, 2.0, 2.4];
/// The share of their weapons that aren't of their own trade, one in this many
const OTHER_TRADE: u32 = 8;

impl Game {
    /// The specialist talked to, by their stall's key
    pub(super) fn talking_to(&self) -> Option<[i64; 3]> {
        self.trades.talk.as_ref().map(|t| t.key)
    }

    /// Stops talking: one who's agreed to come to the base packs up and goes
    pub(super) fn stop_talking(&mut self) {
        if let Some(talk) = self.trades.talk.take()
            && talk.invited
        {
            self.depart(talk.key);
        }
    }

    pub(super) fn is_specialist(&self, key: [i64; 3]) -> bool {
        self.trades.keepers.contains_key(&key)
    }

    /// The trade of the specialist keeping stall `key`, and what their stock's rolled from
    pub(super) fn specialist_of(&self, key: [i64; 3]) -> Option<(Trade, u32)> {
        self.trades.keepers.get(&key).map(|k| (k.trade, k.seed))
    }

    /// What their stall's window is called: who they are and their trade
    pub(super) fn shop_title(&self, key: [i64; 3]) -> Option<String> {
        self.trades.keepers.get(&key).map(|k| format!("{}  ·  {}", k.name, k.trade.title()))
    }

    /// A specialist of `trade` at their stall in town, `key`, standing at `at`, in voxels, facing
    /// `heading`: unless they've moved to the base
    #[allow(clippy::too_many_arguments)]
    pub(super) fn keep_stall(&mut self, world: &World, water: WaterLevel, key: [i64; 3], at: Vector3<f64>, trade: Trade, heading: f64, seed: u32) {
        if self.trades.household.left.contains(&key) {
            return;
        }
        let Some(found) = animals::spawn_keeper(world, water, trade.look(), at, 0.0, seed) else {
            return;
        };
        let keeper = animals::keeper_at(trade.look(), found.position, heading, seed);
        let stock = hash(self.seed ^ hash(key[0] as u32 ^ hash(key[1] as u32 ^ hash(key[2] as u32 ^ 0x5bec))));
        self.trades.keepers.insert(key, Keeper { trade, name: merchants::name(trade, seed), seed: stock, home: false });
        self.merchants.push((keeper.id, key));
        self.animals.push(keeper);
    }

    /// One who's agreed to come to the base leaves their stall in town
    fn depart(&mut self, key: [i64; 3]) {
        if let Some(at) = self.merchants.iter().position(|(_, k)| *k == key) {
            let (id, _) = self.merchants.remove(at);
            self.animals.retain(|a| a.id != id);
        }
        self.trades.keepers.remove(&key);
        self.stocks.remove(&key);
        self.target = None;
    }

    /// Starts talking to the specialist at stall `key`, standing at `at`
    pub(super) fn talk_to(&mut self, key: [i64; 3], at: Vector3<f64>) {
        let Some(keeper) = self.trades.keepers.get(&key) else {
            return;
        };
        self.trades.rng = hash(self.trades.rng ^ keeper.seed ^ at.x as u32);
        let line = if keeper.home { merchants::home_greeting(keeper.trade, self.trades.rng) } else { merchants::greeting(keeper.trade, self.trades.rng) };
        self.trades.talk = Some(Talk { key, line: line.to_owned(), asked: 0, invited: false });
        self.cues.push(Cue::here(Sound::Chest, 0.35).pitched(1.6));
    }

    /// The nearest lair of a boss not yet beaten to `at`, in voxels, as it's told
    fn lair_told(&self, at: Vector3<f64>) -> Option<String> {
        let vpm = self.world_vpm();
        let lair = self.lairs.iter().filter(|l| !self.defeated.contains(&l.key) && l.look.is_boss()).min_by(|a, b| (a.position - at).norm().total_cmp(&(b.position - at).norm()))?;
        let to = (lair.position - at) / vpm;
        Some(merchants::lair_hint(lair.look.name(), to.xy().norm(), merchants::way_to([to.x, to.y])))
    }

    /// Voxels a meter of the world, as it was when those at the base were last placed
    fn world_vpm(&self) -> f64 {
        if self.trades.vpm > 0.0 { self.trades.vpm } else { animals::MODEL_RES }
    }

    /// Asks the specialist talked to to come and work at the base: they say what's missing, or
    /// take the gold and agree, setting out once they're left
    fn invite(&mut self) {
        let Some(key) = self.talking_to() else {
            return;
        };
        let Some(keeper) = self.trades.keepers.get(&key).cloned() else {
            return;
        };
        let home_known = self.trades.household.home.is_some();
        let line = match merchants::can_invite(keeper.trade, self.gold, home_known, &self.trades.household.residents) {
            Err(refusal) => {
                self.cues.push(Cue::here(Sound::Hit, 0.2).pitched(0.6));
                merchants::refusal_line(keeper.trade, &refusal)
            }
            Ok(cost) => {
                self.gold -= cost;
                self.trades.household.residents.push(Resident {
                    trade: keeper.trade.id().to_owned(),
                    name: keeper.name.clone(),
                    look: crate::assets::look_folder(keeper.trade.look()),
                    seed: keeper.seed,
                    arrive_in: merchants::TRAVEL_SECONDS,
                    from: Some(key),
                    ..Default::default()
                });
                self.trades.household.left.push(key);
                self.cues.push(Cue::here(Sound::Coins, 0.8));
                self.cues.push(Cue::here(Sound::Fanfare, 0.5));
                self.milestone = true;
                self.say(format!("{} the {} is moving to your base, for {cost} gold.", keeper.name, keeper.trade.title().to_lowercase()));
                if let Some(talk) = &mut self.trades.talk {
                    talk.invited = true;
                }
                merchants::accept_line(keeper.trade).to_owned()
            }
        };
        if let Some(talk) = &mut self.trades.talk {
            talk.line = line;
        }
    }

    /// The window of talking to a specialist: what they said, and what to say back. False once
    /// it's closed.
    pub(super) fn talk_window(&mut self, ctx: &egui::Context) -> bool {
        let Some(talk) = &self.trades.talk else {
            return false;
        };
        let key = talk.key;
        let (line, invited) = (talk.line.clone(), talk.invited);
        let Some(keeper) = self.trades.keepers.get(&key).cloned() else {
            self.trades.talk = None;
            return false;
        };
        #[derive(Clone, Copy)]
        enum Say {
            Trade,
            Land,
            Invite,
            Chat,
            Bye,
        }
        let mut open = true;
        let mut said = None;
        let [r, g, b] = keeper.trade.color();
        let color = Color32::from_rgb(r, g, b);
        let home_known = self.trades.household.home.is_some();
        let living = self.trades.household.residents.len();
        let cost = keeper.trade.invite_cost();
        let taken = self.trades.household.residents.iter().any(|r| r.trade == keeper.trade.id());
        egui::Window::new(format!("{}  ·  {}", keeper.name, keeper.trade.title()))
            .id(egui::Id::new("specialist talk"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 110.0])
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(RichText::new(format!("Sells {}", keeper.trade.sells())).color(color).small());
                if keeper.home {
                    ui.label(RichText::new("Lives and trades at your fireplace").color(Color32::from_rgb(140, 225, 120)).small());
                }
                ui.add_space(6.0);
                ui.label(RichText::new(&line).italics().size(15.5));
                ui.add_space(10.0);
                let mut button = |ui: &mut egui::Ui, text: RichText, what: Say| {
                    if ui.add_sized([440.0, 30.0], egui::Button::new(text)).clicked() {
                        said = Some(what);
                    }
                };
                button(ui, RichText::new("Trade").strong().size(15.0), Say::Trade);
                button(ui, RichText::new("Ask about the land").size(15.0), Say::Land);
                if keeper.home {
                    button(ui, RichText::new("How are you finding it here?").size(15.0), Say::Chat);
                } else if !invited {
                    button(ui, RichText::new(format!("Come and work at my place  ·  {cost} gold")).color(GOLD).size(15.0), Say::Invite);
                    // What it takes, each part green once it's there
                    let ok = Color32::from_rgb(140, 225, 120);
                    let short = Color32::from_rgb(255, 140, 110);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(RichText::new("Needs:").weak().small());
                        ui.label(RichText::new(if home_known { "a fireplace at your base (built)," } else { "a fireplace at your base (none yet)," }).color(if home_known { ok } else { short }).small());
                        ui.label(RichText::new(format!("{cost} gold (you have {}),", self.gold)).color(if self.gold >= cost { ok } else { short }).small());
                        let room = living < merchants::MOST_RESIDENTS && !taken;
                        ui.label(RichText::new(format!("room ({living} of {} living there{})", merchants::MOST_RESIDENTS, if taken { ", one of their trade already" } else { "" })).color(if room { ok } else { short }).small());
                    });
                }
                ui.add_space(4.0);
                button(ui, RichText::new("Goodbye").weak().size(14.0), Say::Bye);
            });
        let at = self.merchants.iter().find(|(_, k)| *k == key).and_then(|(id, _)| self.animals.iter().find(|a| a.id == *id)).map(|a| a.position);
        match said {
            Some(Say::Trade) => {
                self.stop_talking();
                if self.trades.keepers.contains_key(&key) {
                    self.stock(key);
                    self.shop = Some(key);
                    self.cues.push(Cue::here(Sound::Chest, 0.5).pitched(1.3));
                }
            }
            Some(Say::Land) => {
                let lair = at.and_then(|at| self.lair_told(at));
                if let Some(talk) = &mut self.trades.talk {
                    talk.line = merchants::land_hint(keeper.trade, keeper.seed, talk.asked, lair.as_deref());
                    talk.asked += 1;
                }
            }
            Some(Say::Chat) => {
                self.trades.rng = hash(self.trades.rng.wrapping_add(1));
                let line = merchants::chatter(keeper.trade, self.trades.rng).to_owned();
                if let Some(talk) = &mut self.trades.talk {
                    talk.line = line;
                }
            }
            Some(Say::Invite) => self.invite(),
            Some(Say::Bye) => self.stop_talking(),
            None => {}
        }
        if !open {
            self.stop_talking();
        }
        self.shopping()
    }

    /// What the specialist of `trade` at stall `key` has to sell, rolled from `seed`: weapons, most
    /// of their own trade and luckier than the travelling merchant's, and the ammo, jewels or wings
    /// of their trade, see Trade
    pub(super) fn specialist_stock(&mut self, key: [i64; 3], trade: Trade, seed: u32) {
        let families = merchants::families(&self.library, trade);
        let mut seed = hash(seed ^ 0x5bec_7a11);
        let mut wares = Vec::new();
        // The jeweller has a couple of weapons to set their stones in, the wingmaker none
        let count = match trade {
            Trade::Wingmaker => 0,
            Trade::Jeweller => 2,
            _ => SPECIALIST_LUCKS.len(),
        };
        for &luck in SPECIALIST_LUCKS.iter().rev().take(count) {
            seed = hash(seed);
            let family = (!families.is_empty() && hash(seed ^ 0x0ff) % OTHER_TRADE != 0).then(|| families[hash(seed ^ 0xfa) as usize % families.len()]);
            // Of about the player's own level, as the travelling merchant's are
            let weapon = self.library.generate(seed, family, luck * loot_scaling(self.scaling));
            let weapon = self.levelled_by(weapon, self.level, Source::Merchant, seed);
            self.weapons.push(weapon);
            wares.push(Ware::Weapon(self.weapons.len() - 1));
        }
        self.models_version += 1;
        wares.sort_by_key(|w| match *w {
            Ware::Weapon(i) => self.weapon_price(i),
            Ware::Food(..) => 0,
        });
        // Wings, the plainest the player hasn't and rarer ones besides
        if trade == Trade::Wingmaker {
            let mut kinds: Vec<u8> = Vec::new();
            let has = |k: u8, kinds: &[u8]| self.carries(Item::Wings(k)) || kinds.contains(&k);
            if let Some(k) = wings::plainest(|k| has(k, &kinds)) {
                kinds.push(k);
            }
            for _ in 0..12 {
                if kinds.len() >= 4 {
                    break;
                }
                seed = hash(seed);
                if let Some(k) = wings::pick((seed >> 8) as f64 / 16_777_216.0, 3.0 * loot_scaling(self.scaling), |k| has(k, &kinds)) {
                    kinds.push(k);
                }
            }
            kinds.sort_by_key(|&k| wings::KINDS[k as usize].price);
            wares.extend(kinds.into_iter().map(|k| Ware::Food(Item::Wings(k), 1)));
        }
        // Their ammo, a few bundles of each kind
        let fits = trade.ammo();
        let mut ammo: Vec<usize> = (0..self.library.ammo.len()).filter(|&k| fits.contains(&self.library.ammo[k].fits.as_str())).collect();
        for i in (1..ammo.len()).rev() {
            seed = hash(seed);
            ammo.swap(i, seed as usize % (i + 1));
        }
        for k in ammo.into_iter().take(6) {
            wares.push(Ware::Food(Item::Ammo(k, self.library.ammo[k].bundle), 4));
        }
        // Their jewels
        let (categories, rarest) = trade.jewels();
        let mut jewels = Vec::new();
        for &category in categories {
            for tier in 0..=rarest {
                jewels.push(crate::jewels::jewel(category, tier));
            }
        }
        let most = if trade == Trade::Jeweller { 9 } else { 3 };
        for i in (1..jewels.len()).rev() {
            seed = hash(seed);
            jewels.swap(i, seed as usize % (i + 1));
        }
        let mut jewels: Vec<u8> = jewels.into_iter().take(most).collect();
        jewels.sort_unstable();
        for j in jewels {
            seed = hash(seed);
            wares.push(Ware::Food(Item::Jewel(j), 2 + seed % 3));
        }
        // And potions, for those who sell what's fought up close or with magic
        if matches!(trade, Trade::Wandwright | Trade::Bladesmith) {
            wares.push(Ware::Food(Item::Potion, 4));
        }
        self.stocks.insert(key, wares);
    }

    // At the base

    /// Counts down to the next look at who should be out at the base: true when it's time
    pub fn merchants_due(&mut self, dt: f64) -> bool {
        self.trades.due -= dt;
        if self.trades.due > 0.0 {
            return false;
        }
        self.trades.due = DUE_SECONDS;
        true
    }

    /// Where the base's fireplace is, as merchants::Household::home has it
    pub fn set_merchant_home(&mut self, home: Option<[f64; 4]>) {
        if self.trades.household.home != home {
            self.trades.household.home = home;
        }
    }

    pub fn merchant_home(&self) -> Option<[f64; 4]> {
        self.trades.household.home
    }

    /// Those living at the base, and the rest of what's kept of the specialists
    pub fn household(&self) -> &Household {
        &self.trades.household
    }

    /// Takes up what the save has of them, in place of what there was
    pub fn load_household(&mut self, household: Household) {
        self.trades.household = household;
        self.trades.stands.clear();
        self.trades.stand_of.clear();
        self.trades.talk = None;
    }

    /// Those on their way to the base getting there, and those living there saying something now
    /// and then as the player passes by
    pub fn tick_residents(&mut self, dt: f64) {
        let mut arrived = Vec::new();
        for r in &mut self.trades.household.residents {
            if r.arrive_in > 0.0 {
                r.arrive_in -= dt;
                if r.arrive_in <= 0.0 {
                    r.arrive_in = 0.0;
                    arrived.push(format!("{} has reached your base and set up a stand by the fireplace.", r.name));
                }
            }
        }
        if !arrived.is_empty() {
            self.milestone = true;
        }
        for message in arrived {
            self.say(message);
        }
        self.trades.chatter -= dt;
        if self.trades.chatter > 0.0 || self.shopping() {
            return;
        }
        let Some(feet) = self.feet else {
            return;
        };
        let vpm = self.world_vpm();
        let near = self.merchants.iter().filter_map(|&(id, key)| Some((self.trades.keepers.get(&key).filter(|k| k.home)?, self.animals.iter().find(|a| a.id == id)?))).find(|(_, a)| (a.position - feet).norm() < CHATTER_NEAR * vpm);
        if let Some((keeper, _)) = near {
            self.trades.rng = hash(self.trades.rng.wrapping_add(0x7a1c));
            let message = format!("{}: {}", keeper.name, merchants::chatter(keeper.trade, self.trades.rng));
            self.trades.chatter = CHATTER_SECONDS + (self.trades.rng % 30) as f64;
            self.say(message);
        }
    }

    /// Those living at the base out at their stands by the fireplace, `home` where it is in voxels
    /// and which way it faces, by day, and gone in at night, as merchants::Resident::out() has it,
    /// `now` in seconds of game time on day `day`; their stock rolled afresh each day. `feet` is
    /// where the player is.
    #[allow(clippy::too_many_arguments)]
    pub fn place_residents(&mut self, world: &World, _water: WaterLevel, home: Option<(Vector3<f64>, f64)>, night: bool, now: f64, day: u32, feet: Vector3<f64>) {
        let vpm = world.voxels_per_meter as f64;
        self.trades.vpm = vpm;
        let residents = self.trades.household.residents.clone();
        let mut went_in = 0;
        let mut back = Vec::new();
        for (slot, r) in residents.iter().enumerate() {
            let Some(trade) = r.trade() else {
                continue;
            };
            let key = home_key(trade);
            let present = self.merchants.iter().any(|(_, k)| *k == key);
            let out = home.is_some() && r.out(night, now);
            if out && !present {
                let (hearth, facing) = home.unwrap();
                let Some((at, stand)) = self.stand_spot(world, hearth, facing, slot) else {
                    continue;
                };
                let keeper = animals::keeper_at(trade.look(), at, facing, r.seed);
                self.trades.keepers.insert(key, Keeper { trade, name: r.name.clone(), seed: hash(r.seed ^ day.wrapping_mul(0x9e37_79b9)), home: true });
                self.merchants.push((keeper.id, key));
                self.animals.push(keeper);
                self.trades.stands.push((stand, facing));
                self.trades.stand_of.push(trade);
                if r.away_until.is_some() {
                    back.push(r.name.clone());
                }
            } else if !out && present {
                if self.shop == Some(key) {
                    self.shop = None;
                }
                if self.talking_to() == Some(key) {
                    self.trades.talk = None;
                }
                self.depart(key);
                if let Some(k) = self.trades.stand_of.iter().position(|&t| t == trade) {
                    self.trades.stand_of.remove(k);
                    self.trades.stands.remove(k);
                }
                went_in += night as usize;
            }
            // Their stock rolled afresh each day, though not while it's being looked through
            if out && r.stocked_day != Some(day) && self.shop != Some(key) {
                self.stocks.remove(&key);
                if let Some(keeper) = self.trades.keepers.get_mut(&key) {
                    keeper.seed = hash(r.seed ^ day.wrapping_mul(0x9e37_79b9));
                }
                if let Some(saved) = self.trades.household.residents.get_mut(slot) {
                    saved.stocked_day = Some(day);
                }
            }
            if out && let Some(saved) = self.trades.household.residents.get_mut(slot) {
                saved.away_until = None;
            }
        }
        let near = home.is_some_and(|(at, _)| (at - feet).norm() < HOME_NEAR * vpm);
        if went_in > 0 && near {
            self.say("Night's falling: your merchants pack up their stands and shelter inside by the fire.");
        }
        if !back.is_empty() {
            self.say(format!("{} {} back at the base, bandaged up but trading again.", back.join(" and "), if back.len() == 1 { "is" } else { "are" }));
        }
    }

    /// Where the `slot`th of those living at the base stands, and their stand before them: out in
    /// front of the fireplace at `hearth`, in voxels, facing `facing`, side by side, or wherever
    /// round it there's floor if that's in a wall, keeping clear of the others
    fn stand_spot(&self, world: &World, hearth: Vector3<f64>, facing: f64, slot: usize) -> Option<(Vector3<f64>, Vector3<f64>)> {
        let vpm = world.voxels_per_meter as f64;
        let ahead = Vector3::new(facing.cos(), facing.sin(), 0.0);
        let side = Vector3::new(-ahead.y, ahead.x, 0.0);
        let across = [-0.8, 0.8, -2.4, 2.4, 0.0, -4.0, 4.0][slot % 7];
        let taken: Vec<Vector3<f64>> = self.trades.stands.clone().into_iter().map(|s| s.0).collect();
        let tries = std::iter::once((2.6, across)).chain((0..36).map(|k| (2.0 + (k / 12) as f64 * 1.5, ((k % 12) as f64 - 5.5) * 0.8)));
        for (out, by) in tries {
            let at = hearth + (ahead * out + side * by) * vpm;
            let stand = at + ahead * 0.95 * vpm;
            let (Some(z), Some(sz)) = (floor_at(world, at), floor_at(world, stand)) else {
                continue;
            };
            if (z - hearth.z).abs() > 1.2 * vpm || (sz - z).abs() > 0.3 * vpm {
                continue;
            }
            let (at, stand) = (Vector3::new(at.x, at.y, z), Vector3::new(stand.x, stand.y, sz));
            if taken.iter().all(|t| (t - stand).xy().norm() > 1.5 * vpm) {
                return Some((at, stand));
            }
        }
        None
    }

    /// Whether any of the night's raiders still standing is within `reach`, in voxels, of `at`
    pub fn raider_near(&self, at: Vector3<f64>, reach: f64) -> bool {
        self.animals.iter().any(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.raider) && (a.position - at).norm() < reach)
    }

    /// A raid's reached the fire: those sheltering by it are hurt, and go off to heal till `back`,
    /// in seconds of game time, `now` being now. Never for good.
    pub fn residents_hurt(&mut self, back: f64, now: f64) {
        let mut hurt = Vec::new();
        for r in &mut self.trades.household.residents {
            if r.trade().is_some() && r.arrive_in <= 0.0 && r.away_until.is_none_or(|b| now >= b) {
                r.away_until = Some(back);
                hurt.push(r.name.clone());
            }
        }
        if hurt.is_empty() {
            return;
        }
        self.milestone = true;
        self.cues.push(Cue::here(Sound::Hurt, 0.6).pitched(1.2));
        self.say(format!("The raiders broke through to your fireplace! {} got hurt and fled to the healer; back about midday tomorrow.", hurt.join(", ")));
    }

    /// The world moved on `shift` voxels: the stands at the base move with it, and those whose
    /// keepers are now outside it go with them
    pub(super) fn recenter_trades(&mut self, shift: Vector3<f64>) {
        let merchants = &self.merchants;
        let trades = &mut self.trades;
        trades.keepers.retain(|key, _| merchants.iter().any(|(_, k)| k == key));
        if trades.talk.as_ref().is_some_and(|t| !trades.keepers.contains_key(&t.key)) {
            trades.talk = None;
        }
        let mut k = 0;
        while k < trades.stands.len() {
            if trades.keepers.contains_key(&home_key(trades.stand_of[k])) {
                trades.stands[k].0 -= shift;
                k += 1;
            } else {
                trades.stands.remove(k);
                trades.stand_of.remove(k);
            }
        }
    }

    /// Notes the specialists the player's come near in world `world`, for the map, `feet` where
    /// they are in voxels of the world whose corner is at `origin` in the whole landscape
    pub fn note_specialists(&mut self, world: &str, feet: Vector3<f64>, origin: Vector3<f64>, vpm: f64) {
        for &(id, key) in &self.merchants {
            let Some(keeper) = self.trades.keepers.get(&key).filter(|k| !k.home) else {
                continue;
            };
            let Some(animal) = self.animals.iter().find(|a| a.id == id) else {
                continue;
            };
            if (animal.position - feet).norm() > KNOWN_NEAR * vpm || self.trades.household.known.iter().any(|k| k.key == key && k.world == world) {
                continue;
            }
            let at = (animal.position + origin) / vpm;
            self.trades.household.known.push(merchants::Known { world: world.to_owned(), key, at: [at.x, at.y], trade: keeper.trade.id().to_owned(), name: keeper.name.clone() });
        }
        let known = &mut self.trades.household.known;
        if known.len() > merchants::KNOWN_MOST {
            let extra = known.len() - merchants::KNOWN_MOST;
            known.drain(..extra);
        }
    }

    /// The specialists to mark on the map of world `world`, where in meters, what's written by them
    /// and their colour: those come across, and those living at the base
    pub fn merchant_marks(&self, world: &str) -> Vec<([f64; 2], String, [u8; 3])> {
        let household = &self.trades.household;
        let mut marks: Vec<([f64; 2], String, [u8; 3])> = household
            .known
            .iter()
            .filter(|k| k.world == world && !household.left.contains(&k.key))
            .filter_map(|k| {
                let trade = Trade::by_id(&k.trade)?;
                Some((k.at, trade.title().to_owned(), trade.color()))
            })
            .collect();
        if world == merchants::HOME_WORLD
            && let Some(home) = household.home
            && !household.residents.is_empty()
        {
            let names: Vec<&str> = household.residents.iter().filter_map(|r| r.trade()).map(|t| t.title()).collect();
            marks.push(([home[0], home[1]], format!("Home: {}", names.join(", ")), [140, 225, 120]));
        }
        marks
    }

    // For trying it out

    /// A specialist of `trade` `distance` meters along `ahead` from `near`, in voxels, facing back
    pub fn summon_specialist(&mut self, world: &World, water: WaterLevel, near: Vector3<f64>, ahead: Vector3<f64>, distance: f64, trade: Trade) {
        let vpm = world.voxels_per_meter as f64;
        let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let at = near + flat * distance * vpm;
        let key = [at.x as i64, at.y as i64, -4 - trade.index() as i64];
        let heading = (-flat.y).atan2(-flat.x);
        self.keep_stall(world, water, key, at, trade, heading, hash(at.x as u32 ^ trade.index() as u32));
    }

    /// Talks to the specialist nearest `near`, then says `what`: "invite", "land", "chat" or
    /// "trade", with `gold` to pay and with `home`, a fireplace at the base to come to
    pub fn stage_talk(&mut self, near: Vector3<f64>, what: Option<&str>, gold: Option<u32>, home: bool) {
        let nearest = self.merchants.iter().filter(|(_, k)| self.trades.keepers.contains_key(k)).filter_map(|&(id, key)| Some((self.animals.iter().find(|a| a.id == id)?.position, key))).min_by(|a, b| (a.0 - near).norm().total_cmp(&(b.0 - near).norm()));
        let Some((at, key)) = nearest else {
            println!("No specialist about to talk to");
            return;
        };
        if let Some(gold) = gold {
            self.gold = gold;
        }
        if home && self.trades.household.home.is_none() {
            self.trades.household.home = Some([0.0, 0.0, 0.0, 0.0]);
        }
        self.talk_to(key, at);
        match what {
            Some("invite") => self.invite(),
            Some("land") => {
                let lair = self.lair_told(at);
                if let (Some(talk), Some((trade, seed))) = (&mut self.trades.talk, self.trades.keepers.get(&key).map(|k| (k.trade, k.seed))) {
                    talk.line = merchants::land_hint(trade, seed, talk.asked, lair.as_deref());
                    talk.asked += 1;
                }
            }
            Some("trade") => {
                self.trades.talk = None;
                self.stock(key);
                self.shop = Some(key);
            }
            _ => {}
        }
        println!("Talking to {}: {}", self.shop_title(key).unwrap_or_default(), self.trades.talk.as_ref().map_or("(trading)", |t| t.line.as_str()));
    }

    /// Has those of `trades` live at the base already, there now, for a look at them
    pub fn stage_residents(&mut self, trades: &[Trade]) {
        for (k, &trade) in trades.iter().enumerate() {
            if self.trades.household.residents.iter().any(|r| r.trade == trade.id()) {
                continue;
            }
            let seed = hash(0x5e77 + k as u32 * 31 + trade.index() as u32);
            self.trades.household.residents.push(Resident { trade: trade.id().to_owned(), name: merchants::name(trade, seed), look: crate::assets::look_folder(trade.look()), seed, ..Default::default() });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daynight;

    fn game() -> Game {
        let world = World::from_packed_occupancy([8; 3], &[0; 4], AIR);
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        Game::new(&world, &dry, 5, Vector3::new(4.0, 4.0, 4.0))
    }

    /// A specialist's stall has more weapons than the travelling merchant's, most of them of their
    /// own trade, and rarer, and the ammo, jewels or wings of their trade
    #[test]
    fn specialists_stock_mostly_their_own_trade() {
        let mut game = game();
        let library = Library::load();
        let mut general_grades = Vec::new();
        for n in 0..30 {
            let key = [n, 7 * n, 7];
            game.stock(key);
            general_grades.extend(game.stocks[&key].iter().filter_map(|w| if let Ware::Weapon(i) = *w { Some(game.weapons[i].grade) } else { None }));
        }
        let general = general_grades.iter().sum::<usize>() as f64 / general_grades.len() as f64;
        for trade in Trade::ALL {
            let (mut theirs, mut all, mut grades) = (0, 0, 0);
            for n in 0..30i64 {
                let key = [n, 3 * n, -20 - trade.index() as i64];
                game.trades.keepers.insert(key, Keeper { trade, name: merchants::name(trade, n as u32), seed: hash(n as u32 ^ 0x77), home: false });
                game.stock(key);
                let wares = &game.stocks[&key];
                for ware in wares {
                    match *ware {
                        Ware::Weapon(i) => {
                            all += 1;
                            grades += game.weapons[i].grade;
                            theirs += (merchants::trade_of(&library, game.weapons[i].family) == Some(trade)) as usize;
                        }
                        Ware::Food(Item::Ammo(k, _), _) => assert!(trade.ammo().contains(&library.ammo[k].fits.as_str()), "{trade:?} sells {}", library.ammo[k].id),
                        Ware::Food(Item::Wings(_), _) => assert_eq!(trade, Trade::Wingmaker),
                        Ware::Food(Item::Jewel(j), _) => assert!(trade.jewels().0.contains(&crate::jewels::parts(j).0), "{trade:?}"),
                        _ => {}
                    }
                }
                // Each sells what's theirs
                match trade {
                    Trade::Bowyer | Trade::Gunsmith => assert!(wares.iter().any(|w| matches!(w, Ware::Food(Item::Ammo(..), _)))),
                    Trade::Jeweller => assert!(wares.iter().filter(|w| matches!(w, Ware::Food(Item::Jewel(_), _))).count() >= 6),
                    Trade::Wingmaker => assert!(wares.iter().any(|w| matches!(w, Ware::Food(Item::Wings(_), _)))),
                    _ => {}
                }
                // And the travelling merchant's wings and hook aren't put on their stalls
                if trade != Trade::Wingmaker {
                    assert!(!wares.iter().any(|w| matches!(w, Ware::Food(Item::Wings(_) | Item::Hook, _))), "{trade:?}");
                }
            }
            if matches!(trade, Trade::Bowyer | Trade::Wandwright | Trade::Bladesmith | Trade::Gunsmith) {
                assert_eq!(all, 30 * SPECIALIST_LUCKS.len());
                assert!(theirs as f64 >= 0.75 * all as f64, "{trade:?}: {theirs} of {all} their own");
                let average = grades as f64 / all as f64;
                assert!(average > general, "{trade:?}: {average:.2} against the travelling merchant's {general:.2}");
            }
        }
    }

    /// Talking to a specialist and asking them to the base: without a fireplace they say so and stay;
    /// with one and the gold they take it, and go from their stall once they're left; one of each
    /// trade, so another bowyer won't come
    #[test]
    fn asking_a_specialist_to_the_base() {
        let mut game = game();
        let key = [10, 20, 7];
        let other = [30, 40, 7];
        for (k, trade) in [(key, Trade::Bowyer), (other, Trade::Bowyer)] {
            game.trades.keepers.insert(k, Keeper { trade, name: merchants::name(trade, k[0] as u32), seed: 9, home: false });
        }
        game.set_gold(1000);
        game.talk_to(key, Vector3::zeros());
        game.invite();
        assert!(game.household().residents.is_empty(), "no fireplace yet");
        assert!(game.trades.talk.as_ref().unwrap().line.contains("fireplace"));
        assert_eq!(game.gold, 1000);
        game.set_merchant_home(Some([1.0, 2.0, 3.0, 0.0]));
        game.invite();
        assert_eq!(game.household().residents.len(), 1);
        assert_eq!(game.gold, 1000 - Trade::Bowyer.invite_cost());
        assert_eq!(game.household().residents[0].arrive_in, merchants::TRAVEL_SECONDS);
        // Once left, they've gone from their stall, and don't come back to it
        game.close_shop();
        assert!(!game.is_specialist(key));
        assert!(game.household().left.contains(&key));
        // Another bowyer won't come
        game.talk_to(other, Vector3::zeros());
        game.invite();
        assert_eq!(game.household().residents.len(), 1);
        assert!(game.trades.talk.as_ref().unwrap().line.contains("working at your fire"));
        // They get there in time, and it's said
        game.tick_residents(merchants::TRAVEL_SECONDS + 1.0);
        assert_eq!(game.household().residents[0].arrive_in, 0.0);
        assert!(game.messages.iter().any(|(m, _)| m.contains("reached your base")));
    }

    /// No more than MOST_RESIDENTS live at the base, one of each trade
    #[test]
    fn at_most_four_live_at_the_base_one_of_each() {
        let mut game = game();
        game.set_gold(100_000);
        game.set_merchant_home(Some([1.0, 2.0, 3.0, 0.0]));
        for (n, trade) in Trade::ALL.into_iter().chain(Trade::ALL).enumerate() {
            let key = [n as i64, 5, 7];
            game.trades.keepers.insert(key, Keeper { trade, name: merchants::name(trade, n as u32), seed: n as u32, home: false });
            game.talk_to(key, Vector3::zeros());
            game.invite();
            game.close_shop();
        }
        let residents = &game.household().residents;
        assert_eq!(residents.len(), merchants::MOST_RESIDENTS);
        let mut trades: Vec<&str> = residents.iter().map(|r| r.trade.as_str()).collect();
        trades.dedup();
        assert_eq!(trades.len(), merchants::MOST_RESIDENTS);
    }

    /// A raid reaching the fire hurts those living there, who are gone till midday the next day and
    /// then come back: none is ever lost
    #[test]
    fn merchants_at_the_base_survive_a_raid() {
        let mut game = game();
        game.stage_residents(&[Trade::Bowyer, Trade::Wandwright]);
        let day = 3;
        let night = day as f64 * daynight::CYCLE_SECONDS + daynight::seconds_at(23.0);
        let back = merchants::back_from_hurt(day);
        game.residents_hurt(back, night);
        assert!(game.household().residents.iter().all(|r| r.away_until == Some(back)));
        assert!(game.messages.iter().any(|(m, _)| m.contains("back about midday tomorrow")));
        // Hurt again the same night changes nothing
        game.residents_hurt(back + 50.0, night + 10.0);
        assert!(game.household().residents.iter().all(|r| r.away_until == Some(back)));
        let morning = (day + 1) as f64 * daynight::CYCLE_SECONDS + daynight::seconds_at(8.0);
        assert!(game.household().residents.iter().all(|r| !r.out(false, morning)));
        assert!(game.household().residents.iter().all(|r| r.out(false, back + 1.0)));
        assert_eq!(game.household().residents.len(), 2);
        // And they're kept through a save
        let mut save = save::Save::default();
        save.set_section(merchants::SECTION, game.household());
        let mut other = self::game();
        other.load_household(save.section(merchants::SECTION).unwrap_or_default());
        assert_eq!(other.household(), game.household());
    }
}
