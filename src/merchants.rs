//! Specialist merchants: a bowyer, a wandwright, a weaponsmith, a gunsmith, a jeweller and a
//! wingmaker, each keeping a stall of their own on the squares of the towns beside the general
//! merchant's, under a sign of their trade, with more of what they make and better of it. They're
//! talked to before they're traded with, see game\specialists.rs, and can be asked to come and work
//! at the player's base: for gold, once there's a fireplace built there, at most MOST_RESIDENTS of
//! them and one of each trade. There they set up a stand by the fire, restock every day, shelter
//! inside at night, and if a raid reaches the fire they're hurt and gone till midday tomorrow, never
//! for good.
//!
//! What's kept of them is the whole game's, see Household, in the save's section SECTION: who lives
//! at the base, where its fireplace is, the specialists the player's come across for the map, and
//! the stalls whose keepers moved away. Their base is always in the normal world: one invited in
//! another realm goes back there.

use serde::{Deserialize, Serialize};

use crate::animals::Look;
use crate::jewels::Category;
use crate::weapons::{Kind, Library};

use super::*;

/// What the save keeps of them under, see save::Save::section()
pub const SECTION: &str = "merchants";

/// The world the player's base is in, where those invited live: the normal one, in Realms too
pub const HOME_WORLD: &str = "normal";

/// The most merchants that live at the base
pub const MOST_RESIDENTS: usize = 4;

/// Seconds a merchant invited takes to get to the base
pub const TRAVEL_SECONDS: f64 = 45.0;

/// How near the base's fireplace raiders have to get, in meters, to hurt the merchants sheltering there
pub const RAID_REACH_M: f64 = 7.0;

/// A specialist's trade
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Trade {
    /// Bows, crossbows, arrows and bolts
    Bowyer,
    /// Wands, staves, sceptres, orbs and tomes, and jewels for them
    Wandwright,
    /// Blades, axes, spears and polearms
    Bladesmith,
    /// Guns, hand cannons, bombs and shot
    Gunsmith,
    /// Jewels for every kind of weapon
    Jeweller,
    /// Wings
    Wingmaker,
}

/// Which merchant sells each family of weapons, by its id in assets\weapons.json. A family that
/// isn't here goes by its kind, see trade_of(): a new family's a line here if it should go elsewhere.
pub const SPECIALITIES: &[(&str, Trade)] = &[
    ("bow", Trade::Bowyer),
    ("crossbow", Trade::Bowyer),
    ("arbalest", Trade::Bowyer),
    ("repeater", Trade::Bowyer),
    ("handcrossbow", Trade::Bowyer),
    ("twinprod", Trade::Bowyer),
    ("siegecrossbow", Trade::Bowyer),
    ("harpoon", Trade::Bowyer),
    ("boltthrower", Trade::Bowyer),
    ("staff", Trade::Wandwright),
    ("wand", Trade::Wandwright),
    ("scepter", Trade::Wandwright),
    ("orb", Trade::Wandwright),
    ("tome", Trade::Wandwright),
    ("totem", Trade::Wandwright),
    ("bell", Trade::Wandwright),
    ("battlestaff", Trade::Wandwright),
    ("rod", Trade::Wandwright),
    ("censer", Trade::Wandwright),
    ("sword", Trade::Bladesmith),
    ("axe", Trade::Bladesmith),
    ("hammer", Trade::Bladesmith),
    ("dagger", Trade::Bladesmith),
    ("spear", Trade::Bladesmith),
    ("greatsword", Trade::Bladesmith),
    ("katana", Trade::Bladesmith),
    ("scythe", Trade::Bladesmith),
    ("halberd", Trade::Bladesmith),
    ("whip", Trade::Bladesmith),
    ("flail", Trade::Bladesmith),
    ("maul", Trade::Bladesmith),
    ("warfan", Trade::Bladesmith),
    ("katar", Trade::Bladesmith),
    ("naginata", Trade::Bladesmith),
    // The third batch: a war pick's forged as a blade is, and throwing knives and a chakram are
    // blades for throwing; a prism and a runestone are a wandwright's
    ("warpick", Trade::Bladesmith),
    ("knives", Trade::Bladesmith),
    ("chakram", Trade::Bladesmith),
    ("prism", Trade::Wandwright),
    ("runestone", Trade::Wandwright),
    ("pistol", Trade::Gunsmith),
    ("musket", Trade::Gunsmith),
    ("blunderbuss", Trade::Gunsmith),
    ("handcannon", Trade::Gunsmith),
    ("bombs", Trade::Gunsmith),
];

impl Trade {
    pub const ALL: [Trade; 6] = [Trade::Bowyer, Trade::Wandwright, Trade::Bladesmith, Trade::Gunsmith, Trade::Jeweller, Trade::Wingmaker];

    /// How it's kept in the save
    pub fn id(self) -> &'static str {
        match self {
            Trade::Bowyer => "bowyer",
            Trade::Wandwright => "wandwright",
            Trade::Bladesmith => "bladesmith",
            Trade::Gunsmith => "gunsmith",
            Trade::Jeweller => "jeweller",
            Trade::Wingmaker => "wingmaker",
        }
    }

    pub fn by_id(id: &str) -> Option<Trade> {
        Trade::ALL.into_iter().find(|t| t.id() == id)
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// What they're called by their trade
    pub fn title(self) -> &'static str {
        match self {
            Trade::Bowyer => "Bowyer",
            Trade::Wandwright => "Wandwright",
            Trade::Bladesmith => "Weaponsmith",
            Trade::Gunsmith => "Gunsmith",
            Trade::Jeweller => "Jeweller",
            Trade::Wingmaker => "Wingmaker",
        }
    }

    /// What they sell, to finish "Sells ..."
    pub fn sells(self) -> &'static str {
        match self {
            Trade::Bowyer => "bows, crossbows, arrows and bolts",
            Trade::Wandwright => "wands, staves, sceptres, orbs, tomes and jewels for them",
            Trade::Bladesmith => "swords, axes, spears, polearms and jewels for them",
            Trade::Gunsmith => "guns, hand cannons, bombs and shot",
            Trade::Jeweller => "jewels for every kind of weapon",
            Trade::Wingmaker => "wings of many kinds",
        }
    }

    /// What they look like, see animals.rs
    pub fn look(self) -> Look {
        match self {
            Trade::Bowyer => Look::Bowyer,
            Trade::Wandwright => Look::Wandwright,
            Trade::Bladesmith => Look::Bladesmith,
            Trade::Gunsmith => Look::Gunsmith,
            Trade::Jeweller => Look::Jeweller,
            Trade::Wingmaker => Look::Wingmaker,
        }
    }

    pub fn of_look(look: Look) -> Option<Trade> {
        Trade::ALL.into_iter().find(|t| t.look() == look)
    }

    /// Gold asked to come and work at the base
    pub fn invite_cost(self) -> u32 {
        match self {
            Trade::Bowyer | Trade::Bladesmith => 250,
            Trade::Wandwright | Trade::Gunsmith => 300,
            Trade::Jeweller => 350,
            Trade::Wingmaker => 400,
        }
    }

    /// Their colour, for their name over their head and their mark on the map
    pub fn color(self) -> [u8; 3] {
        match self {
            Trade::Bowyer => [140, 215, 100],
            Trade::Wandwright => [185, 150, 255],
            Trade::Bladesmith => [240, 130, 105],
            Trade::Gunsmith => [225, 185, 120],
            Trade::Jeweller => [110, 225, 235],
            Trade::Wingmaker => [200, 228, 255],
        }
    }

    /// The site a stall of theirs is, as worldgen's towns put it down
    pub fn stall_site(self) -> &'static str {
        match self {
            Trade::Bowyer => "bowyer stall",
            Trade::Wandwright => "wandwright stall",
            Trade::Bladesmith => "bladesmith stall",
            Trade::Gunsmith => "gunsmith stall",
            Trade::Jeweller => "jeweller stall",
            Trade::Wingmaker => "wingmaker stall",
        }
    }

    pub fn of_site(kind: &str) -> Option<Trade> {
        Trade::ALL.into_iter().find(|t| t.stall_site() == kind)
    }

    /// How likely a town's specialist stall is to be theirs, next to the others: the bowyer and the
    /// wandwright the most, the jeweller and the wingmaker the least
    pub fn weight(self) -> u32 {
        match self {
            Trade::Bowyer | Trade::Wandwright => 3,
            Trade::Bladesmith | Trade::Gunsmith => 2,
            Trade::Jeweller | Trade::Wingmaker => 1,
        }
    }

    /// The special ammo they sell, as AmmoDef::fits has it
    pub fn ammo(self) -> &'static [&'static str] {
        match self {
            Trade::Bowyer => &["arrow", "bolt"],
            Trade::Gunsmith => &["shot"],
            _ => &[],
        }
    }

    /// The jewels they sell, and how rare at most, of jewels::TIERS
    pub fn jewels(self) -> (&'static [Category], usize) {
        match self {
            Trade::Wandwright => (&[Category::Magic], 2),
            Trade::Bladesmith => (&[Category::Melee], 2),
            Trade::Bowyer => (&[Category::Ranged], 1),
            Trade::Jeweller => (&Category::ALL, 3),
            _ => (&[], 0),
        }
    }
}

/// Which specialist sells weapons of `family`: as SPECIALITIES has it, or by its kind. None for a
/// tool.
pub fn trade_of(library: &Library, family: usize) -> Option<Trade> {
    let f = library.families.get(family)?;
    if f.tool.is_some() {
        return None;
    }
    let listed = SPECIALITIES.iter().find(|(id, _)| *id == f.id).map(|&(_, t)| t);
    Some(listed.unwrap_or(match f.kind {
        Kind::Melee => Trade::Bladesmith,
        Kind::Ranged => Trade::Bowyer,
        Kind::Magic => Trade::Wandwright,
    }))
}

/// The families of weapons `trade` sells, none for those who sell no weapons of their own
pub fn families(library: &Library, trade: Trade) -> Vec<usize> {
    (0..library.families.len()).filter(|&f| trade_of(library, f) == Some(trade)).collect()
}

/// The specialists at a town's stalls past its first, which the general merchant keeps: picked by
/// Trade::weight() from `seed`, no two the same
pub fn town_trades(stalls: usize, seed: u32) -> Vec<Option<Trade>> {
    let mut out = vec![None];
    let mut rng = hash(seed ^ 0x5bec_1a15);
    for _ in 1..stalls {
        let left: Vec<Trade> = Trade::ALL.into_iter().filter(|t| !out.contains(&Some(*t))).collect();
        let total: u32 = left.iter().map(|t| t.weight()).sum();
        rng = hash(rng);
        let mut pick = rng % total.max(1);
        let trade = left.iter().copied().find(|t| {
            let hit = pick < t.weight();
            pick = pick.saturating_sub(t.weight());
            hit
        });
        out.push(trade);
    }
    out.truncate(stalls);
    out
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

fn pick<'a>(list: &[&'a str], seed: u32) -> &'a str {
    list[hash(seed) as usize % list.len()]
}

// Names

const FIRST_HEADS: [&str; 24] = ["Al", "Bren", "Cor", "Da", "El", "Fen", "Gar", "Hal", "Is", "Jor", "Ke", "Lis", "Mor", "Ny", "Or", "Pel", "Quen", "Ro", "Sa", "Tor", "Ul", "Ves", "Wyn", "Yr"];
const FIRST_TAILS: [&str; 14] = ["a", "wen", "ric", "o", "is", "ard", "ette", "an", "ia", "ulf", "eth", "ora", "in", "mund"];

/// The first and second halves of each trade's family names, by Trade::index()
const FAMILY_HEADS: [[&str; 8]; 6] = [
    ["Yew", "Ash", "Fletch", "Grey", "String", "Elm", "Hawk", "Swift"],
    ["Star", "Moon", "Ember", "Rune", "Glim", "Sable", "Mist", "Hex"],
    ["Iron", "Steel", "Edge", "Anvil", "Grim", "Keen", "Brand", "Cold"],
    ["Powder", "Flint", "Brass", "Cinder", "Smoke", "Black", "Match", "Spark"],
    ["Gem", "Silver", "Opal", "Glint", "Garnet", "Pearl", "Facet", "Lustre"],
    ["Feather", "Gull", "Wren", "Kite", "Cloud", "Swallow", "Lark", "Breeze"],
];
const FAMILY_TAILS: [[&str; 6]; 6] = [
    ["wood", "feather", "hand", "shaft", "eye", "bough"],
    ["weave", "whisper", "glass", "ward", "bright", "thorn"],
    ["fold", "temper", "hammer", "forge", "hand", "brand"],
    ["barrel", "lock", "wick", "shot", "horn", "pan"],
    ["setter", "cut", "stone", "glass", "ring", "finch"],
    ["stitch", "loft", "wing", "sail", "quill", "drift"],
];

/// A specialist's name, rolled from `seed`: a first name of two syllables and a family name of
/// their trade, like Fenwen Yewfeather or Torulf Powderwick
pub fn name(trade: Trade, seed: u32) -> String {
    let h = |k: u32| hash(seed ^ k.wrapping_mul(0x9e37_79b9)) as usize;
    let t = trade.index();
    let first = format!("{}{}", FIRST_HEADS[h(1) % FIRST_HEADS.len()], FIRST_TAILS[h(2) % FIRST_TAILS.len()]);
    let family = format!("{}{}", FAMILY_HEADS[t][h(3) % 8], FAMILY_TAILS[t][h(4) % 6]);
    // The same word twice reads badly, as Swiftswift
    let family = if family.to_lowercase().starts_with(&FAMILY_TAILS[t][h(4) % 6].to_lowercase()) { format!("{}{}", FAMILY_HEADS[t][(h(3) + 1) % 8], FAMILY_TAILS[t][h(4) % 6]) } else { family };
    format!("{first} {family}")
}

// What they say

/// A greeting in their own voice, at their stall in town
pub fn greeting(trade: Trade, seed: u32) -> &'static str {
    pick(
        match trade {
            Trade::Bowyer => &[
                "\"A good bow is a promise kept at two hundred paces. Looking to make one?\"",
                "\"Yew for the long bows, ash for the crossbows, goose feather for the rest. Have a look.\"",
                "\"Mind the strings, they're waxed fresh. What can I string up for you?\"",
            ],
            Trade::Wandwright => &[
                "\"Careful where you point that hand. Every wand here's loaded with something.\"",
                "\"Staves, sceptres, an orb or two that only hums a little. Can I tempt you?\"",
                "\"Ah, a traveller with a spark in them. I can always tell.\"",
            ],
            Trade::Bladesmith => &[
                "\"Folded steel, true edges, no rust. You'll not find better this side of the hills.\"",
                "\"Every blade on this table I've beaten out myself. Pick one up, feel the balance.\"",
                "\"Need something that cuts? Or something that crushes? I do both.\"",
            ],
            Trade::Gunsmith => &[
                "\"Stand back from the powder, friend. Now, what needs a bang put through it?\"",
                "\"Muskets, pistols, a hand cannon if you've the shoulders for it.\"",
                "\"Brass, black powder and good iron shot. Everything a careful person needs.\"",
            ],
            Trade::Jeweller => &[
                "\"Every stone here's cut to sit in a socket. Your weapon's naked without one.\"",
                "\"Let the light catch them. Red for blades, green for bows, blue for the arcane.\"",
                "\"A jewel set right lifts a weapon higher than any whetstone. Have a look.\"",
            ],
            Trade::Wingmaker => &[
                "\"Ever wanted to step off a cliff and keep going? I can help with that.\"",
                "\"Every pair stitched by hand, every feather checked twice.\"",
                "\"Gull, falcon, something rarer if you've the gold. The sky's open to anyone with wings.\"",
            ],
        },
        seed,
    )
}

/// What they say at the base, greeting the player at their stand
pub fn home_greeting(trade: Trade, seed: u32) -> &'static str {
    pick(
        match trade {
            Trade::Bowyer => &["\"Back again? I've strung a few new ones since you left.\"", "\"Good light out here for checking a bow's bend. What do you need?\""],
            Trade::Wandwright => &["\"Your fire's put a lovely warmth in the crystals. Come, look.\"", "\"I've had a new batch charging all morning.\""],
            Trade::Bladesmith => &["\"I've been at the grindstone since dawn. Take your pick.\"", "\"Fresh steel, still warm. What'll it be?\""],
            Trade::Gunsmith => &["\"Powder's dry, barrels are clean. Ready when you are.\"", "\"I kept the powder well away from your fire, don't worry.\""],
            Trade::Jeweller => &["\"I've cut a few new stones by your firelight.\"", "\"Something sparkly for the master of the house?\""],
            Trade::Wingmaker => &["\"I've had a new pair stretched out to dry all day.\"", "\"Good wind up here today. Want to try a pair?\""],
        },
        seed,
    )
}

/// Their trade's own tip, a true one, when asked about the land
fn trade_tip(trade: Trade, seed: u32) -> &'static str {
    pick(
        match trade {
            Trade::Bowyer => &[
                "\"Hold your right click to aim a bow's power, and let go to loose it. Its mark shows where it lands.\"",
                "\"Special arrows don't grow on trees. Press T to pick which you're shooting.\"",
            ],
            Trade::Wandwright => &[
                "\"A jewel set into a staff lifts it a level, but only a jewel of its own kind and rarity fits.\"",
                "\"Magic needs mana, and mana comes back if you give it a breath.\"",
            ],
            Trade::Bladesmith => &[
                "\"A legendary blade's special lets go on R, once it's charged. Save it for something big.\"",
                "\"Right click swings heavy. Slower, but it hits like a cart.\"",
            ],
            Trade::Gunsmith => &["\"Q reloads. Count your shots, or the dark will count them for you.\"", "\"A hand cannon's shell bursts where it lands. Don't stand near where it lands.\""],
            Trade::Jeweller => &[
                "\"A jewel only fits a weapon of its own kind and rarity, and once it's set it stays set.\"",
                "\"The rarer the weapon, the more sockets it has. A mythic takes three.\"",
            ],
            Trade::Wingmaker => &["\"Jump, then hold Space in the air to glide. Some wings flap, some dash with Shift.\"", "\"The wing shrines up on the crags have wings in their nests, free for whoever climbs.\""],
        },
        seed,
    )
}

/// What's said of the land that's the same for them all
const LAND_LORE: [&str; 5] = [
    "\"Bandits keep camps out in the wilds. Clear one out and their hoard's yours.\"",
    "\"The old dungeons go deep. The deeper the hall, the worse what's in it, and the better what it guards.\"",
    "\"Nights bring raiders. Walls hold them well enough; it's the doors they go for.\"",
    "\"A fire burning keeps the beasts wary at night. Fewer of them dare come at once.\"",
    "\"Princesses keep armouries in the royal castles. Only the finest blades, at the finest prices.\"",
];

/// What they say when asked about the land, the `n`th time: a boss's lair if one's known, see
/// game\specialists.rs, and their trade's tips and what everyone knows by turns
pub fn land_hint(trade: Trade, seed: u32, n: usize, lair: Option<&str>) -> String {
    match (n % 3, lair) {
        (0, Some(lair)) => lair.to_owned(),
        (1, _) => trade_tip(trade, seed.wrapping_add(n as u32)).to_owned(),
        _ => pick(&LAND_LORE, seed.wrapping_add(n as u32 * 7)).to_owned(),
    }
}

/// A lair, as it's told: `who` lives `meters` off `way`
pub fn lair_hint(who: &str, meters: f64, way: &str) -> String {
    format!("\"Folk say the {who} has its lair about {:.0} meters {way} of here. Go well armed, or don't go.\"", (meters / 10.0).round() * 10.0)
}

/// Which way `to` is, as "to the north" or "to the south-east": x east, y north
pub fn way_to(to: [f64; 2]) -> &'static str {
    let a = to[1].atan2(to[0]).to_degrees().rem_euclid(360.0);
    ["to the east", "to the north-east", "to the north", "to the north-west", "to the west", "to the south-west", "to the south", "to the south-east"][((a + 22.5) / 45.0) as usize % 8]
}

/// What they say now and then at the base, when the player comes by
pub fn chatter(trade: Trade, seed: u32) -> &'static str {
    pick(
        match trade {
            Trade::Bowyer => &["\"Saw a hawk circling the hill this morning. Fine feathers on that one.\"", "\"Quiet out here. A bowyer likes a quiet place to work.\"", "\"Your fire's just right for drying glue.\""],
            Trade::Wandwright => &["\"The stars are sharper out here than in town.\"", "\"Something's humming in the ground under your house. Probably nothing.\"", "\"I sleep better by your fire than I ever did in the square.\""],
            Trade::Bladesmith => &["\"Your walls could take a few more hammer blows, if you ask me.\"", "\"Honest work, good fire, nobody haggling. I could get used to this.\"", "\"Bring me back something you've killed with my steel, eh?\""],
            Trade::Gunsmith => &["\"I'm keeping the powder downwind. You're welcome.\"", "\"Heard the raiders last night. Wished I'd had a loaded musket.\"", "\"Smoke and sulphur. Smells like home.\""],
            Trade::Jeweller => &["\"Your firelight's the best I've cut stones by.\"", "\"Don't tell the others, but the ruby's my favourite.\"", "\"Safer out here than in a town square with this much on the table.\""],
            Trade::Wingmaker => &["\"The wind off your hill is perfect for testing.\"", "\"I lost a pair to a gust this morning. They'll turn up.\"", "\"One day I'll make a pair that carries two.\""],
        },
        seed,
    )
}

/// What's in the way of asking one to come and work at the base, see can_invite()
#[derive(Clone, PartialEq, Debug)]
pub enum Refusal {
    /// One of their trade lives there already, by name
    Taken(String),
    /// There are MOST_RESIDENTS there already
    Full,
    /// No fireplace's built at the base
    NoHearth,
    /// This much more gold's wanted
    Gold(u32),
}

/// Whether a specialist of `trade` would come and work at the base, and for how much: one of each
/// trade, at most MOST_RESIDENTS, a fireplace built there, `home_known`, and the gold, of `gold`
pub fn can_invite(trade: Trade, gold: u32, home_known: bool, residents: &[Resident]) -> Result<u32, Refusal> {
    let cost = trade.invite_cost();
    if let Some(r) = residents.iter().find(|r| r.trade == trade.id()) {
        Err(Refusal::Taken(r.name.clone()))
    } else if residents.len() >= MOST_RESIDENTS {
        Err(Refusal::Full)
    } else if !home_known {
        Err(Refusal::NoHearth)
    } else if gold < cost {
        Err(Refusal::Gold(cost - gold))
    } else {
        Ok(cost)
    }
}

/// What they say to it
pub fn refusal_line(trade: Trade, refusal: &Refusal) -> String {
    match refusal {
        Refusal::Taken(name) => format!("\"You've {name} working at your fire already. One {} to a hearth, or we'd only fight over the customers.\"", trade.title().to_lowercase()),
        Refusal::Full => format!("\"{MOST_RESIDENTS} merchants at one fire already? There's no room for another stall.\""),
        Refusal::NoHearth => "\"Your place? Show me a fireplace first. Build one at your base (B, then the Fireplace) and ask again.\"".to_owned(),
        Refusal::Gold(short) => format!("\"Moving a whole stall isn't cheap. Bring {} gold; you're {short} short.\"", trade.invite_cost()),
    }
}

/// What they say on agreeing to come
pub fn accept_line(trade: Trade) -> &'static str {
    match trade {
        Trade::Bowyer => "\"Done! I'll bundle up the staves and be at your fireplace before long.\"",
        Trade::Wandwright => "\"How exciting! Let me pack the orbs carefully. I'll see you at your fireplace.\"",
        Trade::Bladesmith => "\"A forge of my own by your fire? I'll be there before the steel cools.\"",
        Trade::Gunsmith => "\"I'll pack the powder dry and be at your fireplace soon. Keep sparks away from the door.\"",
        Trade::Jeweller => "\"Somewhere safer than a town square? Gladly. I'll be at your fireplace shortly.\"",
        Trade::Wingmaker => "\"I'll fly over! Well, walk. I'll be at your fireplace soon.\"",
    }
}

// What's kept

/// The merchants living at the base, and what's known of the others, as the save keeps it
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Household {
    pub residents: Vec<Resident>,
    /// The base's fireplace in the home world: the middle of its foot, in meters of the whole
    /// landscape, and which way its opening faces, in radians from east. None till one's built.
    pub home: Option<[f64; 4]>,
    /// The specialists the player's come across, for the map
    pub known: Vec<Known>,
    /// The stalls whose keepers moved to the base, which stay empty
    pub left: Vec<[i64; 3]>,
}

/// A merchant living at the base
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Resident {
    /// Their trade, by Trade::id(): one this Innert doesn't know is kept but never comes out
    pub trade: String,
    pub name: String,
    /// Which look they have, by its folder in assets\models
    pub look: String,
    /// What their stock and lines are rolled from
    pub seed: u32,
    /// Seconds more before they get to the base, after they're asked
    pub arrive_in: f64,
    /// When they're back from being hurt in a raid, in seconds of game time, see game_seconds()
    #[serde(skip_serializing_if = "Option::is_none")]
    pub away_until: Option<f64>,
    /// The day their stock was last rolled, for it to be rolled again the next
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stocked_day: Option<u32>,
    /// The stall in town they came from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<[i64; 3]>,
}

impl Resident {
    pub fn trade(&self) -> Option<Trade> {
        Trade::by_id(&self.trade)
    }

    /// Whether they're out at their stand by the fire at `now`, see game_seconds(): there, by day,
    /// not hurt
    pub fn out(&self, night: bool, now: f64) -> bool {
        self.trade().is_some() && self.arrive_in <= 0.0 && !night && self.away_until.is_none_or(|back| now >= back)
    }
}

/// A specialist come across in a town: in which world, by their stall, where in meters, their
/// trade and name
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Known {
    pub world: String,
    pub key: [i64; 3],
    pub at: [f64; 2],
    pub trade: String,
    pub name: String,
}

/// How many specialists the map remembers, the oldest forgotten first
pub const KNOWN_MOST: usize = 200;

/// Game time in seconds since the first dawn: whole days and the time into this one
pub fn game_seconds(clock: &daynight::Clock) -> f64 {
    clock.day as f64 * daynight::CYCLE_SECONDS + clock.seconds
}

/// When those hurt in a raid on day `day`'s night are back: about midday the next day
pub fn back_from_hurt(day: u32) -> f64 {
    (day + 1) as f64 * daynight::CYCLE_SECONDS + daynight::seconds_at(12.0)
}

impl App {
    /// The merchants living at the base coming and going with the day, travelling there, taking
    /// hurt when a raid reaches the fire, and the specialists in towns noted for the map. Called each
    /// frame the world's going on.
    pub(super) fn update_merchants(&mut self, dt: f64) {
        self.game.tick_residents(dt);
        if !self.game.merchants_due(dt) {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let home_world = self.session.world_name() == HOME_WORLD;
        // The base's fireplace, the first built, seen in the home world and kept for the others
        if home_world && self.session.mode.is_some() {
            let hearth = self.bases.pieces().iter().filter(|p| p.kind == bases::Kind::Hearth).min_by_key(|p| p.id);
            let home = hearth.map(|p| {
                let m = p.middle();
                let f = bases::facing(p.turn);
                [m[0] / vpm, m[1] / vpm, (p.at[2] * bases::BLOCK) as f64 / vpm, (f[1] as f64).atan2(f[0] as f64)]
            });
            self.game.set_merchant_home(home);
        }
        // `--set home_merchants=bowyer,wandwright` has those living at the base already, for a look
        // at them once there's a fireplace, as `--script hearth` builds
        if let Some(list) = settings_from_args().into_iter().find(|(k, _)| k == "home_merchants").map(|(_, v)| v)
            && self.game.merchant_home().is_some()
            && self.game.household().residents.is_empty()
        {
            let trades: Vec<Trade> = list.split(',').filter_map(Trade::by_id).collect();
            self.game.stage_residents(&trades);
            println!("Living at the base: {}", self.game.household().residents.iter().map(|r| format!("{} the {}", r.name, r.trade)).collect::<Vec<_>>().join(", "));
        }
        let spot = self.game.merchant_home().filter(|_| home_world).map(|h| (Vector3::new(h[0] * vpm, h[1] * vpm, h[2] * vpm) - origin, h[3]));
        let night = !self.fixed_sun && self.cycle.monsters_out();
        let now = game_seconds(&self.cycle.clock);
        let water = &self.water;
        let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        self.game.place_residents(&self.world, &|x, y| water.surface_at(x, y), spot, night, now, self.cycle.clock.day, feet);
        // A raid that gets to the fire hurts those sheltering there
        if let Some((at, _)) = spot
            && self.raid.on
            && self.game.raider_near(at, RAID_REACH_M * vpm)
        {
            self.game.residents_hurt(back_from_hurt(self.cycle.clock.day), now);
        }
        let world = self.session.world_name().to_owned();
        self.game.note_specialists(&world, feet, origin, vpm);
    }

    /// The household as the save keeps it, see Household
    pub(super) fn merchants_save(&self) -> Option<serde_json::Value> {
        serde_json::to_value(self.game.household()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resident(trade: Trade) -> Resident {
        Resident { trade: trade.id().to_owned(), name: name(trade, 3), look: trade.id().to_owned(), seed: 3, ..Default::default() }
    }

    /// Every family of weapons but the tools has a specialist, the bows the bowyer's and the wands
    /// the wandwright's, and each of the trades that sell weapons sells several families
    #[test]
    fn every_family_has_a_specialist() {
        let library = Library::load();
        for (f, family) in library.families.iter().enumerate() {
            assert_eq!(trade_of(&library, f).is_none(), family.tool.is_some(), "{}", family.id);
        }
        assert_eq!(trade_of(&library, library.family("bow").unwrap()), Some(Trade::Bowyer));
        assert_eq!(trade_of(&library, library.family("crossbow").unwrap()), Some(Trade::Bowyer));
        assert_eq!(trade_of(&library, library.family("wand").unwrap()), Some(Trade::Wandwright));
        assert_eq!(trade_of(&library, library.family("musket").unwrap()), Some(Trade::Gunsmith));
        for trade in [Trade::Bowyer, Trade::Wandwright, Trade::Bladesmith, Trade::Gunsmith] {
            assert!(families(&library, trade).len() >= 4, "{trade:?}");
        }
        // Every family named in the table is one there is
        for (id, _) in SPECIALITIES {
            assert!(library.family(id).is_some(), "{id}");
        }
    }

    /// A town's first stall is the general merchant's, the rest specialists, never two of a trade,
    /// the bowyer and the wandwright the most often
    #[test]
    fn towns_have_specialists() {
        let mut counts = [0usize; 6];
        for seed in 0..600u32 {
            let trades = town_trades(3, seed);
            assert_eq!(trades.len(), 3);
            assert_eq!(trades[0], None);
            let (a, b) = (trades[1].unwrap(), trades[2].unwrap());
            assert_ne!(a, b);
            counts[a.index()] += 1;
            counts[b.index()] += 1;
        }
        assert!(counts.iter().all(|&n| n > 30), "{counts:?}");
        assert!(counts[Trade::Bowyer.index()] > counts[Trade::Jeweller.index()]);
        assert!(counts[Trade::Wandwright.index()] > counts[Trade::Wingmaker.index()]);
        assert_eq!(town_trades(1, 5), vec![None]);
    }

    /// Names are their trade's, not all the same, and the same every time from the same seed
    #[test]
    fn names_are_their_own() {
        let names: std::collections::HashSet<String> = (0..50).map(|s| name(Trade::Bowyer, s)).collect();
        assert!(names.len() > 35);
        assert_eq!(name(Trade::Wandwright, 9), name(Trade::Wandwright, 9));
        for trade in Trade::ALL {
            let n = name(trade, 12);
            assert!(n.split(' ').count() == 2 && n.chars().next().unwrap().is_uppercase(), "{n}");
            assert!(FAMILY_HEADS[trade.index()].iter().any(|h| n.split(' ').nth(1).unwrap().starts_with(h)), "{n}");
        }
    }

    /// What it takes to have one come and work at the base: a fireplace built, the gold, room, and
    /// not one of their trade there already; and they say which it is that's missing
    #[test]
    fn the_invitation_asks_for_a_fireplace_gold_and_room() {
        let cost = Trade::Bowyer.invite_cost();
        assert_eq!(can_invite(Trade::Bowyer, cost, true, &[]), Ok(cost));
        assert_eq!(can_invite(Trade::Bowyer, cost * 3, false, &[]), Err(Refusal::NoHearth));
        assert_eq!(can_invite(Trade::Bowyer, cost - 40, true, &[]), Err(Refusal::Gold(40)));
        assert!(refusal_line(Trade::Bowyer, &Refusal::Gold(40)).contains("40 short"));
        assert!(refusal_line(Trade::Bowyer, &Refusal::NoHearth).contains("fireplace"));
        // One of each trade
        let home = [resident(Trade::Bowyer)];
        assert!(matches!(can_invite(Trade::Bowyer, 9999, true, &home), Err(Refusal::Taken(_))));
        assert_eq!(can_invite(Trade::Wandwright, 9999, true, &home), Ok(Trade::Wandwright.invite_cost()));
        // And no more than MOST_RESIDENTS
        let full: Vec<Resident> = Trade::ALL[..MOST_RESIDENTS].iter().map(|&t| resident(t)).collect();
        for trade in &Trade::ALL[MOST_RESIDENTS..] {
            assert_eq!(can_invite(*trade, 9999, true, &full), Err(Refusal::Full));
        }
    }

    /// Those living at the base come out by day once they've got there, shelter at night, and when
    /// a raid's hurt them they're gone only till midday the next day
    #[test]
    fn residents_shelter_at_night_and_come_back_after_a_raid() {
        let mut r = resident(Trade::Gunsmith);
        r.arrive_in = 10.0;
        assert!(!r.out(false, 100.0), "still on the way");
        r.arrive_in = 0.0;
        assert!(r.out(false, 100.0));
        assert!(!r.out(true, 100.0), "in for the night");
        let day = 4;
        let night = day as f64 * daynight::CYCLE_SECONDS + daynight::seconds_at(22.0);
        r.away_until = Some(back_from_hurt(day));
        let dawn = (day + 1) as f64 * daynight::CYCLE_SECONDS + daynight::seconds_at(7.0);
        let afternoon = (day + 1) as f64 * daynight::CYCLE_SECONDS + daynight::seconds_at(13.0);
        assert!(night < dawn && dawn < afternoon);
        assert!(!r.out(false, dawn), "hurt, so not out the morning after");
        assert!(r.out(false, afternoon), "back by the afternoon");
        // An unknown trade, from a later Innert, is kept but never comes out
        let later = Resident { trade: "alchemist".into(), ..Default::default() };
        assert!(!later.out(false, 100.0));
    }

    /// The household goes into the save and comes back as it was; a save from before there were
    /// any, or one with things this Innert doesn't know, loads with what it can
    #[test]
    fn the_household_is_saved() {
        let mut home = Household { home: Some([10.5, 20.0, 3.0, 1.57]), ..Default::default() };
        let mut r = resident(Trade::Wandwright);
        r.away_until = Some(1234.5);
        r.stocked_day = Some(3);
        r.from = Some([1, 2, 3]);
        home.residents.push(r);
        home.residents.push(resident(Trade::Bowyer));
        home.known.push(Known { world: "normal".into(), key: [4, 5, 6], at: [1.0, 2.0], trade: "bowyer".into(), name: "Fen Yewwood".into() });
        home.left.push([1, 2, 3]);
        let mut save = save::Save::default();
        save.set_section(SECTION, &home);
        let text = serde_json::to_string(&save).unwrap();
        let back: save::Save = serde_json::from_str(&text).unwrap();
        assert_eq!(back.section::<Household>(SECTION), Some(home));
        // An old save has none
        let old: save::Save = serde_json::from_str(r#"{"version": 1, "mode": "realms"}"#).unwrap();
        assert_eq!(old.section::<Household>(SECTION).unwrap_or_default(), Household::default());
        // One with fields and trades a later Innert has
        let later: Household = serde_json::from_str(r#"{"residents": [{"trade": "alchemist", "name": "Ny Vial", "mood": "happy"}], "pets": 3}"#).unwrap();
        assert_eq!(later.residents.len(), 1);
        assert_eq!(later.residents[0].trade(), None);
    }

    #[test]
    fn ways_are_named() {
        assert_eq!(way_to([1.0, 0.0]), "to the east");
        assert_eq!(way_to([0.0, 1.0]), "to the north");
        assert_eq!(way_to([-1.0, -1.0]), "to the south-west");
    }
}
