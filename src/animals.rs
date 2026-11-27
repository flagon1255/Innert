//! Animals wandering the landscape: bears, dogs, cats and squirrels. Each is a little model made
//! of voxels, built here from a few rounded shapes, with a few frames of walking. They're drawn by
//! shaders\traverse.comp, which traces rays through their voxels, so they cast shadows and show up
//! in reflections like everything else.

use std::f64::consts::{PI, TAU};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use nalgebra::{UnitQuaternion, Vector2, Vector3};
use vulkano::buffer::BufferContents;

use crate::bosses::{self, Boss};
use crate::combat::Status;
use crate::serpents;

/// Fish and sharks: how they look, swim in schools and hunt, see Look::Silverfin
pub mod sealife;
use crate::weapons::Element;
use crate::player::WaterLevel;
use crate::worldgen::{Land, PropKind, World, is_passable, material::*};

/// What an animal, or something lying around to pick up, looks like. Each has its own model, in
/// several frames.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Bear,
    Dog,
    GrayCat,
    GingerCat,
    Squirrel,
    Apple,
    Meat,
    Candle,
    Skull,
    Chest,
    OpenChest,
    /// A travelling merchant, standing by its wares
    Merchant,
    /// Gold dropped, a healing potion and a loaf, lying on the ground
    Coins,
    Potion,
    Bread,
    // Bosses, see src\bosses.rs. Golems, drakes and the great bear come in kinds for the cold, the
    // desert or the jungle.
    Golem,
    Drake,
    Ironhide,
    FrostGolem,
    MagmaGolem,
    FrostWyrm,
    VenomDrake,
    Frostfang,
    Spider,
    Treant,
    Direwolf,
    WinterAlpha,
    GraveKnight,
    StormDrake,
    Rimeweaver,
    // Raid bosses: huge, in three phases, calling up help and bringing the sky down
    Colossus,
    ElderWyrm,
    CryptTitan,
    // Bosses of builds of their own, drawn jointed, see src\rig.rs: a cragmaw ogre with its club in
    // the hills and the old castles, a rime jotun with its frozen maul in the cold, a thunderborn
    // giant on the high stone, a sunken pharaoh in the desert ruins, a mire crone in the jungle
    // shrines, an elderhorn stag in the old woods; and the Dreaming Red's own raid bosses, the
    // Hollow Mother in its soft hollows and the Bleeding Hart in its dreaming wood
    Ogre,
    RimeJotun,
    StormGiant,
    SandPharaoh,
    BogHag,
    Elderhorn,
    HollowMother,
    BleedingHart,
    // Monsters: smaller, and many of them, going for the player on sight. Wolves roam the land,
    // the rest haunt dungeons.
    Wolf,
    Skeleton,
    ArmoredSkeleton,
    Zombie,
    ArmoredZombie,
    Spiderling,
    // Bandits: people gone bad, roaming in gangs and keeping camps, with a warlord over the worst
    // of them
    Bandit,
    BanditBrute,
    Warlord,
    /// Folk living in the towns, strolling round their squares: a man in a tunic and straw hat,
    /// and a woman in a long dress with a basket on her arm
    Villager,
    Townswoman,
    /// The princess of a royal castle, keeping the royal armoury in its throne room, and the
    /// guards standing at its gate and by her throne, see src\worldgen\castles.rs
    Princess,
    RoyalGuard,
    // More monsters, each to its own kind of country: bandits who hang back with crossbows or
    // hold the line behind a shield, burrowers under the desert sand, wraiths drifting over the
    // snow, lurkers lying in wait in the jungle's shallows, gloomwings swarming in caves and
    // ravines, and tuskers charging through the woods
    Crossbowman,
    Shieldbearer,
    DuneBurrower,
    RimeWraith,
    MireLurker,
    Gloomwing,
    Tusker,
    // The dead of the fallen cities under the land, and of fallen castles: a bloated corpse that
    // bursts into a cloud of venom, a lean ghoul that leaps, a hulking brute swinging a broken
    // gravestone, skeletons with bows, and the necromancer who raises them all
    Bloater,
    Ghoul,
    GraveBrute,
    BoneArcher,
    Necromancer,
    // The way through the realms, see src\realms.rs: the Innert Gate standing near where the
    // journey starts, with sockets for the blocks the realms' raid bosses leave, the portal back
    // to it in each realm, the glowing veil across the opening of either once it's open, and an
    // Innert block itself
    Teleporter,
    RealmPortal,
    PortalVeil,
    InnertBlock,
    // Getting about the sky isles, see src\rope.rs: a grappling hook's head, and wings spread to
    // glide
    GrappleHook,
    Wings,
    // Dragons, see src\dragons.rs: drakelings of four elements that fly, swoop, breathe and come
    // down to fight on foot; the three great dragons of the Dragon realm's lairs; and the
    // Worldpyre Ancient, its finale
    Ashwing,
    Sleetwing,
    Squallwing,
    Blightwing,
    PyreMatriarch,
    CindermawTyrant,
    GildscaleHoardwarden,
    WorldpyreAncient,
    // Serpents, see src\serpents.rs: a viper hidden in the reeds, a coiler swimming the rivers and
    // lakes, a serpent gliding over the sky isles, an adder of the dreaming realm and one swimming
    // through the snow of the frozen one
    ReedViper,
    SiltCoiler,
    GaleSerpent,
    BloodAdder,
    DriftSerpent,
    // Great serpents, too long for one model, drawn a piece at a time, see serpents::Coils: each
    // realm's own lesser boss, the Water realm's three raid bosses, and the ultra boss
    Meremaw,
    Galecoil,
    Veinbinder,
    Shatterfang,
    Hollowtide,
    Reefcoil,
    Gyremaw,
    Ysveth,
    // Omens on the way to the Coil Unending's lair: a serpent's skin shed and left lying, and the
    // bones of a great serpent it ate
    ShedSkin,
    SerpentBones,
    // Avians and gargoyles, drawn out of jointed pieces and flown by src\avians.rs, see also
    // src\gargoyles.rs: harpies and skyhunters, the sky isles' guardians and their three raid
    // bosses; gargoyles of each land's stone, the castles' grotesque and the dreaming realm's
    // bloodwake
    Harpy,
    Skyhunter,
    Stormtalon,
    Harridan,
    Roc,
    Thunderbird,
    HarpyQueen,
    Gargoyle,
    BloodGargoyle,
    RimeGargoyle,
    CinderGargoyle,
    MossGargoyle,
    Grotesque,
    BloodwakeGargoyle,
    // An ordinary serpent's forked tongue, drawn apart from it as it flicks in and out, see
    // serpents::flick()
    SerpentTongue,
    // Bosses for where there were fewest, drawn jointed, see src\bosses\roster.rs: the Slagwright,
    // a smith of black iron with slag in its crucible, and the Cinderback Tortoise, vents smoking
    // on its shell, for the Cinder Crown and the desert; the Lantern Widow and the Dreadmare for
    // the Dreaming Red, the widow in the flooded churches too; the Sunstinger, a scorpion of the
    // desert ruins; the Grovemaul, a great ape of the jungle; and two raid bosses, the Hoarback
    // Mammoth of the Rimelands and the snowbound castles, and the Gilded Sphinx of the desert ruins
    Slagwright,
    Cinderback,
    LanternWidow,
    Dreadmare,
    Sunstinger,
    Grovemaul,
    Hoarback,
    GildedSphinx,
    // Specialist merchants, each keeping a stall of their own trade in the towns, and who can be
    // asked to come and work at the player's base, see src\merchants.rs: a bowyer, a wandwright, a
    // weaponsmith, a gunsmith, a jeweller and a wingmaker; and the stand they set up there
    Bowyer,
    Wandwright,
    Bladesmith,
    Gunsmith,
    Jeweller,
    Wingmaker,
    TradeStall,
    // Life in the water, see src\animals\sealife.rs: fish swimming in schools, silverfins in the
    // open sea, reeflings in its warm shallows, brookbacks in the rivers and lakes, pale gloamfins
    // in the dim water under the rock and frostfins in the cold; the saltjaw shark of the open sea
    // and the great pale ghostmaw hunting it by night; and a cooked fish and a shark's tooth lying
    // about, raw and cooked
    Silverfin,
    Reefling,
    Brookback,
    Gloamfin,
    Frostfin,
    Saltjaw,
    Ghostmaw,
    RawFish,
    CookedFish,
    SharkTooth,
    // Bosses of the wild places, see src\bosses\wilds.rs: the Tidewrack Kraken of the deep water
    // and the Grotto Squid of the flooded caves and the smaller lakes, drawn a piece at a time, see
    // src\bosses\kraken.rs; the Dune Wyrm of the desert and the Caldera Wyrm of the Cinder Crown,
    // great serpents that bore through the ground, see src\bosses\wyrms.rs; and the Skymane, a
    // maned lion on wings of feathers, flown as the dragons are, see src\dragons\skymane.rs
    Kraken,
    GrottoSquid,
    DuneWyrm,
    CalderaWyrm,
    Skymane,
    // Bosses of the Sky Isles' own, drifting over their floating islands, drawn jointed, see
    // src\bosses\roster.rs and src\rig\drifter.rs: the Aeolith, a spirit of the wind bound in rings of
    // carved stone, and the Stormbell, a jelly of thundercloud trailing crackling tendrils
    Aeolith,
    Stormbell,
}

/// The build of a boss or monster, which decides how it moves and fights
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Build {
    Golem,
    Bear,
    Spider,
    Treant,
    Wolf,
    /// Walking dead, bones or rotting flesh, maybe in armor
    Undead,
    /// Low and many-legged, with pincers and a stinging tail, digging through sand
    Burrower,
    /// A shroud drifting over the ground with nothing under it
    Wraith,
    /// Squat and broad, crawling, with a mouth as wide as its head
    Lurker,
    /// Flying on leathery wings
    Bat,
    /// A great heavy beast on four legs, with tusks
    Boar,
    /// A dragon: drawn and flown by src\dragons.rs, out of jointed pieces rather than frames
    Dragon,
    /// A serpent: a long body slithering in S-curves, rearing up coiled to strike, see
    /// src\serpents.rs
    Serpent,
    /// An avian or a gargoyle: drawn and flown by src\avians.rs, out of jointed pieces
    Winged,
    /// A shark: a long body sweeping its tail from side to side, never leaving the water, steered
    /// by src\animals\sealife.rs
    Fish,
    /// A kraken or a squid: a mantle, a head and arms, swimming in deep water, see
    /// src\bosses\kraken.rs
    Kraken,
}

/// What a walking dead or a bandit has in its hands
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arms {
    Blade,
    Crossbow,
    /// A round shield on one arm and a mace in the other hand
    Shield,
    /// A bow, drawn back to the cheek to loose an arrow
    Bow,
    /// A broken gravestone swung in both hands
    Slab,
}

/// The materials a boss is made of: its body, darker parts, things on it, what glows, and one
/// more, like horns or crystals
#[derive(Clone, Copy)]
pub(crate) struct Skin {
    pub(crate) body: u8,
    pub(crate) dark: u8,
    pub(crate) accent: u8,
    pub(crate) glow: u8,
    pub(crate) extra: u8,
}

impl Look {
    pub const ALL: [Look; 142] = [
        Look::Bear,
        Look::Dog,
        Look::GrayCat,
        Look::GingerCat,
        Look::Squirrel,
        Look::Apple,
        Look::Meat,
        Look::Candle,
        Look::Skull,
        Look::Chest,
        Look::OpenChest,
        Look::Merchant,
        Look::Coins,
        Look::Potion,
        Look::Bread,
        Look::Golem,
        Look::Drake,
        Look::Ironhide,
        Look::FrostGolem,
        Look::MagmaGolem,
        Look::FrostWyrm,
        Look::VenomDrake,
        Look::Frostfang,
        Look::Spider,
        Look::Treant,
        Look::Direwolf,
        Look::WinterAlpha,
        Look::GraveKnight,
        Look::StormDrake,
        Look::Rimeweaver,
        Look::Colossus,
        Look::ElderWyrm,
        Look::CryptTitan,
        Look::Ogre,
        Look::RimeJotun,
        Look::StormGiant,
        Look::SandPharaoh,
        Look::BogHag,
        Look::Elderhorn,
        Look::HollowMother,
        Look::BleedingHart,
        Look::Wolf,
        Look::Skeleton,
        Look::ArmoredSkeleton,
        Look::Zombie,
        Look::ArmoredZombie,
        Look::Spiderling,
        Look::Bandit,
        Look::BanditBrute,
        Look::Warlord,
        Look::Villager,
        Look::Townswoman,
        Look::Princess,
        Look::RoyalGuard,
        Look::Crossbowman,
        Look::Shieldbearer,
        Look::DuneBurrower,
        Look::RimeWraith,
        Look::MireLurker,
        Look::Gloomwing,
        Look::Tusker,
        Look::Bloater,
        Look::Ghoul,
        Look::GraveBrute,
        Look::BoneArcher,
        Look::Necromancer,
        Look::Teleporter,
        Look::RealmPortal,
        Look::PortalVeil,
        Look::InnertBlock,
        Look::GrappleHook,
        Look::Wings,
        Look::Ashwing,
        Look::Sleetwing,
        Look::Squallwing,
        Look::Blightwing,
        Look::PyreMatriarch,
        Look::CindermawTyrant,
        Look::GildscaleHoardwarden,
        Look::WorldpyreAncient,
        Look::ReedViper,
        Look::SiltCoiler,
        Look::GaleSerpent,
        Look::BloodAdder,
        Look::DriftSerpent,
        Look::Meremaw,
        Look::Galecoil,
        Look::Veinbinder,
        Look::Shatterfang,
        Look::Hollowtide,
        Look::Reefcoil,
        Look::Gyremaw,
        Look::Ysveth,
        Look::ShedSkin,
        Look::SerpentBones,
        Look::Harpy,
        Look::Skyhunter,
        Look::Stormtalon,
        Look::Harridan,
        Look::Roc,
        Look::Thunderbird,
        Look::HarpyQueen,
        Look::Gargoyle,
        Look::BloodGargoyle,
        Look::RimeGargoyle,
        Look::CinderGargoyle,
        Look::MossGargoyle,
        Look::Grotesque,
        Look::BloodwakeGargoyle,
        Look::SerpentTongue,
        Look::Slagwright,
        Look::Cinderback,
        Look::LanternWidow,
        Look::Dreadmare,
        Look::Sunstinger,
        Look::Grovemaul,
        Look::Hoarback,
        Look::GildedSphinx,
        Look::Bowyer,
        Look::Wandwright,
        Look::Bladesmith,
        Look::Gunsmith,
        Look::Jeweller,
        Look::Wingmaker,
        Look::TradeStall,
        Look::Silverfin,
        Look::Reefling,
        Look::Brookback,
        Look::Gloamfin,
        Look::Frostfin,
        Look::Saltjaw,
        Look::Ghostmaw,
        Look::RawFish,
        Look::CookedFish,
        Look::SharkTooth,
        Look::Kraken,
        Look::GrottoSquid,
        Look::DuneWyrm,
        Look::CalderaWyrm,
        Look::Skymane,
        Look::Aeolith,
        Look::Stormbell,
    ];

    /// One of the dragons, see src\dragons.rs
    pub fn is_dragon(self) -> bool {
        crate::dragons::KINDS.contains(&self)
    }

    /// An avian or a gargoyle, see src\avians.rs
    pub fn is_winged(self) -> bool {
        crate::avians::is_winged(self)
    }

    /// A boss, guarding a lair, rather than a monster or an animal
    pub fn is_boss(self) -> bool {
        self.build().is_some() && !self.is_monster()
    }

    /// One of a bandit gang, warlords aside
    pub fn is_bandit(self) -> bool {
        matches!(self, Look::Bandit | Look::BanditBrute | Look::Crossbowman | Look::Shieldbearer)
    }

    /// A monster roaming its own kind of country in packs, like wolves do, rather than keeping to
    /// a den: burrowers in the desert, wraiths on the snow, lurkers in the jungle and tuskers in
    /// the woods
    pub fn is_wild(self) -> bool {
        matches!(self, Look::DuneBurrower | Look::RimeWraith | Look::MireLurker | Look::Tusker) || crate::dragons::is_drakeling(self) || self.is_serpent()
    }

    /// A beast rather than a person or the dead, with nothing on it worth taking
    pub fn is_beast(self) -> bool {
        matches!(self, Look::Wolf | Look::Spiderling | Look::DuneBurrower | Look::MireLurker | Look::Gloomwing | Look::Tusker | Look::Skyhunter) || crate::dragons::is_drakeling(self) || self.is_serpent() || sealife::is_shark(self)
    }

    /// A fish or a shark, living in the water and never leaving it, see src\animals\sealife.rs
    pub fn is_sea(self) -> bool {
        sealife::is_sea(self)
    }

    /// One of the ordinary serpents, see src\serpents.rs
    pub fn is_serpent(self) -> bool {
        matches!(self, Look::ReedViper | Look::SiltCoiler | Look::GaleSerpent | Look::BloodAdder | Look::DriftSerpent)
    }

    /// One of the great serpents, a boss drawn a piece at a time, see serpents::Coils
    pub fn is_great_serpent(self) -> bool {
        matches!(self, Look::Meremaw | Look::Galecoil | Look::Veinbinder | Look::Shatterfang | Look::Hollowtide | Look::Reefcoil | Look::Gyremaw | Look::Ysveth) || crate::bosses::wyrms::has(self)
    }

    /// A kraken or a squid, kept to deep water, see src\bosses\kraken.rs
    pub fn is_kraken(self) -> bool {
        crate::bosses::kraken::has(self)
    }

    /// The ultra boss, far bigger and harder than any raid boss: the Coil Unending. The Worldpyre
    /// Ancient's the other, see src\dragons.rs, which goes its own way about all this asks after.
    pub fn is_ultra(self) -> bool {
        self == Look::Ysveth
    }

    /// How deep the water it'll wade through can be, in meters: any for what flies over it or
    /// swims in it
    pub fn wade(self) -> f64 {
        match self {
            Look::MireLurker => 1.2,
            // Into the reeds along the water's edge
            Look::ReedViper => 0.45,
            _ if self.flies() || self.swims() => 1000.0,
            _ => 0.2,
        }
    }

    /// Flying, over water as well as land
    pub fn flies(self) -> bool {
        matches!(self, Look::Gloomwing | Look::GaleSerpent | Look::Galecoil) || self.is_dragon() || self.is_winged()
    }

    /// Swimming, along the top of deep water, with only its back above it
    pub fn swims(self) -> bool {
        self == Look::SiltCoiler || serpents::great_of(self).is_some_and(|g| g.medium == serpents::Medium::Water) || self.is_kraken()
    }

    /// Digging along under the sand, the snow or the ice, to come up under the player
    pub fn burrows(self) -> bool {
        matches!(self, Look::DuneBurrower | Look::DriftSerpent | Look::Shatterfang | Look::Hollowtide) || crate::bosses::wyrms::has(self)
    }

    /// Lying low in wait till something comes near: in the shallows, the reeds or deep water
    pub fn lurks(self) -> bool {
        matches!(self, Look::MireLurker | Look::ReedViper | Look::SiltCoiler)
    }

    /// How many of it go about together, when it goes about in a pack of its own kind
    pub fn pack_size(self) -> usize {
        match self {
            Look::Gloomwing => 5,
            _ if crate::dragons::is_drakeling(self) => 2,
            Look::Spiderling => 4,
            Look::MireLurker | Look::Tusker => 2,
            _ if self.is_serpent() => 2,
            Look::Skyhunter => 2,
            _ if crate::gargoyles::is_lesser(self) => 2,
            _ => 3,
        }
    }

    /// A raid boss: far bigger and tougher than the rest, fought in three phases. The Water
    /// realm's three great serpents are, see serpents::GREATS.
    pub fn is_raid(self) -> bool {
        matches!(self, Look::Colossus | Look::ElderWyrm | Look::CryptTitan | Look::HollowMother | Look::BleedingHart | Look::Hollowtide | Look::Reefcoil | Look::Gyremaw)
            || matches!(self, Look::Hoarback | Look::GildedSphinx)
            || crate::dragons::is_great(self)
            || crate::avians::is_raid(self)
    }

    /// A monster: it fights like a boss, but it's smaller and there are many of them about
    pub fn is_monster(self) -> bool {
        matches!(self, Look::Wolf | Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie | Look::Spiderling | Look::Bandit | Look::BanditBrute)
            || matches!(self, Look::Crossbowman | Look::Shieldbearer | Look::DuneBurrower | Look::RimeWraith | Look::MireLurker | Look::Gloomwing | Look::Tusker)
            || self.is_risen()
            || crate::dragons::is_drakeling(self)
            || self.is_serpent()
            || crate::avians::is_monster(self)
            || sealife::is_shark(self)
    }

    /// One of the dead added for the fallen cities, see Look::Bloater
    pub fn is_risen(self) -> bool {
        matches!(self, Look::Bloater | Look::Ghoul | Look::GraveBrute | Look::BoneArcher | Look::Necromancer)
    }

    /// A living person, for bandits: upright, a face under a hood or a helm
    fn living(self) -> bool {
        matches!(self, Look::Bandit | Look::BanditBrute | Look::Warlord | Look::Crossbowman | Look::Shieldbearer)
    }

    /// What it has in its hands, for the walking dead and bandits
    fn arms(self) -> Arms {
        match self {
            Look::Crossbowman => Arms::Crossbow,
            Look::Shieldbearer => Arms::Shield,
            Look::BoneArcher => Arms::Bow,
            Look::GraveBrute => Arms::Slab,
            _ => Arms::Blade,
        }
    }

    /// How a boss or monster moves and fights, None for an animal or a thing
    pub fn build(self) -> Option<Build> {
        Some(match self {
            Look::Golem | Look::FrostGolem | Look::MagmaGolem | Look::Colossus => Build::Golem,
            // Giants are as heavy as golems; the pharaoh, the crone and the Hollow Mother cast as the
            // dead do; the stags charge as the great bears do
            Look::Ogre | Look::RimeJotun | Look::StormGiant => Build::Golem,
            Look::SandPharaoh | Look::BogHag | Look::HollowMother => Build::Undead,
            Look::Elderhorn | Look::BleedingHart => Build::Bear,
            // The smith, the ape and the tortoise are as heavy as golems; the widow casts as the
            // dead do; the mare runs as the alpha wolves do; the scorpion scuttles as the
            // Broodmother does; the mammoth and the sphinx go as the great bears do
            Look::Slagwright | Look::Grovemaul | Look::Cinderback => Build::Golem,
            Look::LanternWidow => Build::Undead,
            Look::Dreadmare => Build::Wolf,
            Look::Sunstinger => Build::Spider,
            Look::Hoarback | Look::GildedSphinx => Build::Bear,
            // The Sky Isles' own drift light as the casting dead walk, never shaking the ground
            Look::Aeolith | Look::Stormbell => Build::Undead,
            Look::Ironhide | Look::Frostfang => Build::Bear,
            Look::Spider | Look::Rimeweaver | Look::Spiderling => Build::Spider,
            Look::Treant => Build::Treant,
            Look::Direwolf | Look::WinterAlpha | Look::Wolf => Build::Wolf,
            Look::GraveKnight | Look::CryptTitan | Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie => Build::Undead,
            Look::Bloater | Look::Ghoul | Look::GraveBrute | Look::BoneArcher | Look::Necromancer => Build::Undead,
            // Bandits move and fight as the dead in armor do, but alive
            Look::Bandit | Look::BanditBrute | Look::Warlord | Look::Crossbowman | Look::Shieldbearer => Build::Undead,
            Look::DuneBurrower => Build::Burrower,
            Look::RimeWraith => Build::Wraith,
            Look::MireLurker => Build::Lurker,
            Look::Gloomwing => Build::Bat,
            Look::Tusker => Build::Boar,
            _ if self.is_dragon() => Build::Dragon,
            _ if self.is_serpent() || self.is_great_serpent() => Build::Serpent,
            _ if self.is_winged() => Build::Winged,
            _ if sealife::is_shark(self) => Build::Fish,
            _ if self.is_kraken() => Build::Kraken,
            _ => return None,
        })
    }

    /// How much bigger its model is than its build's, which is made at the size of the monster
    /// where there is one
    pub fn scale(self) -> f64 {
        match self {
            Look::Direwolf | Look::WinterAlpha => 2.1,
            Look::GraveKnight => 2.2,
            Look::Warlord => 1.7,
            Look::BanditBrute => 1.12,
            // Hulking, a head taller than the rest of the dead, and swollen up bigger than a man
            Look::GraveBrute => 1.32,
            Look::Bloater => 1.15,
            Look::Shieldbearer => 1.06,
            Look::DuneBurrower => 1.15,
            Look::Spiderling => 0.5,
            // Raid bosses tower over the rest of their kind; the Crypt Titan fits under the roof
            // of a dungeon's deepest hall, just
            Look::Colossus => 1.7,
            Look::ElderWyrm => 1.8,
            Look::CryptTitan => 2.75,
            Look::Ogre => 1.5,
            Look::RimeJotun => 1.7,
            Look::StormGiant => 1.65,
            Look::SandPharaoh => 1.45,
            Look::BogHag => 1.4,
            Look::Elderhorn => 1.45,
            Look::HollowMother => 2.3,
            Look::BleedingHart => 2.0,
            _ if crate::bosses::roster::has(self) => crate::bosses::roster::scale(self),
            _ => 1.0,
        }
    }

    /// How much bigger it is than the boss its build is made at the size of, which the build's
    /// numbers, like how far it reaches, are for: 1 for a boss, more for a raid boss
    pub fn size(self) -> f64 {
        match self {
            Look::Colossus | Look::ElderWyrm | Look::HollowMother | Look::BleedingHart | Look::Hoarback | Look::GildedSphinx => self.scale(),
            Look::CryptTitan => self.scale() / Look::GraveKnight.scale(),
            _ if self.is_dragon() => crate::dragons::size(self),
            _ if self.is_winged() => crate::avians::size(self),
            _ => 1.0,
        }
    }

    /// The monster a boss calls up to help it, if it does
    pub fn brood(self) -> Option<Look> {
        match self {
            _ if self.is_dragon() => crate::dragons::brood(self),
            _ if self.is_winged() => crate::avians::brood(self),
            Look::Direwolf | Look::WinterAlpha => Some(Look::Wolf),
            Look::GraveKnight => Some(Look::Skeleton),
            Look::Necromancer => Some(Look::Zombie),
            Look::CryptTitan => Some(Look::ArmoredSkeleton),
            Look::Spider | Look::Rimeweaver => Some(Look::Spiderling),
            Look::Warlord => Some(Look::Bandit),
            // The jotun howls up wolves, the pharaoh raises its dead, the crone calls up what lurks
            // in the mire, and the Hollow Mother calls her children
            Look::RimeJotun => Some(Look::Wolf),
            Look::SandPharaoh => Some(Look::Skeleton),
            Look::BogHag => Some(Look::MireLurker),
            Look::HollowMother => Some(Look::Ghoul),
            // The mammoth trumpets for the wolves that follow its herd, and the sphinx calls the
            // burrowers up out of the sand
            Look::Hoarback => Some(Look::Wolf),
            Look::GildedSphinx => Some(Look::DuneBurrower),
            // The Water realm's raid serpents call up serpents of their own
            Look::Hollowtide => Some(Look::SiltCoiler),
            Look::Reefcoil => Some(Look::ReedViper),
            Look::Gyremaw => Some(Look::GaleSerpent),
            _ => None,
        }
    }

    /// The fallen king of the dead and the titan made after it: a tall, gaunt skeleton in a
    /// broken crown and a tattered cape, with a greatsword, see knight()
    pub fn is_knight(self) -> bool {
        matches!(self, Look::GraveKnight | Look::CryptTitan)
    }

    /// Whether it ever walks, so has frames of walking: things lying about don't, nor do the
    /// merchant and the royals, who stand where they are
    pub(crate) fn walks(self) -> bool {
        !matches!(
            self,
            Look::Apple | Look::Meat | Look::Candle | Look::Skull | Look::Chest | Look::OpenChest | Look::Coins | Look::Potion | Look::Bread | Look::Merchant | Look::Princess | Look::RoyalGuard | Look::ShedSkin | Look::SerpentBones | Look::SerpentTongue
        ) && !self.is_shrine()
            && !matches!(self, Look::RawFish | Look::CookedFish | Look::SharkTooth)
            && !self.is_merchant()
            && self != Look::TradeStall
    }

    /// Part of the way through the realms, which stands where it is, see src\realms.rs
    pub fn is_shrine(self) -> bool {
        matches!(self, Look::Teleporter | Look::RealmPortal | Look::PortalVeil | Look::InnertBlock | Look::GrappleHook | Look::Wings)
    }

    /// Bones rather than rotting flesh, for the walking dead
    fn bony(self) -> bool {
        matches!(self, Look::Skeleton | Look::ArmoredSkeleton | Look::GraveKnight | Look::CryptTitan | Look::BoneArcher | Look::Necromancer)
    }

    /// Wearing armor, for the walking dead
    fn armored(self) -> bool {
        matches!(self, Look::ArmoredSkeleton | Look::ArmoredZombie | Look::GraveKnight | Look::CryptTitan | Look::BanditBrute | Look::Warlord | Look::Shieldbearer)
    }

    /// What its attacks are made of: what a drake breathes, what a golem's blows leave behind
    pub fn element(self) -> Option<Element> {
        match self {
            _ if self.is_dragon() => Some(crate::dragons::element(self)),
            _ if self.is_winged() => crate::avians::element(self),
            Look::FrostGolem | Look::FrostWyrm | Look::Frostfang | Look::WinterAlpha | Look::Rimeweaver => Some(Element::Frost),
            Look::MagmaGolem | Look::Drake | Look::ElderWyrm => Some(Element::Fire),
            Look::CryptTitan => Some(Element::Void),
            Look::RimeJotun => Some(Element::Frost),
            Look::StormGiant => Some(Element::Storm),
            Look::SandPharaoh => Some(Element::Fire),
            Look::BogHag | Look::Elderhorn => Some(Element::Poison),
            Look::HollowMother | Look::BleedingHart => Some(Element::Blood),
            _ if crate::bosses::roster::has(self) => crate::bosses::roster::element(self),
            _ if crate::bosses::wilds::has(self) => crate::bosses::wilds::element(self),
            Look::VenomDrake | Look::Spider | Look::Treant | Look::Spiderling | Look::Zombie | Look::ArmoredZombie | Look::DuneBurrower | Look::Bloater => Some(Element::Poison),
            Look::Necromancer => Some(Element::Void),
            Look::RimeWraith => Some(Element::Frost),
            Look::GraveKnight => Some(Element::Void),
            Look::StormDrake => Some(Element::Storm),
            // A viper's venom, a gale serpent's lightning, an adder's blood and a drift serpent's
            // frost; a coiler only bites
            Look::ReedViper | Look::Meremaw | Look::Reefcoil => Some(Element::Poison),
            Look::GaleSerpent | Look::Galecoil | Look::Gyremaw => Some(Element::Storm),
            Look::BloodAdder | Look::Veinbinder => Some(Element::Blood),
            Look::DriftSerpent | Look::Shatterfang => Some(Element::Frost),
            Look::Hollowtide => Some(Element::Void),
            // The Coil Unending's changes as it's hurt, see serpents::ULTRA_ELEMENTS
            Look::Ysveth => Some(Element::Poison),
            _ => None,
        }
    }

    /// The kind of `self`, a boss, that lives where the ground's `ground`: the cold, the desert or
    /// the jungle have their own
    pub fn for_ground(self, ground: u8) -> Look {
        let cold = matches!(ground, SNOW | ICE);
        let desert = matches!(ground, SAND | SANDSTONE | SANDSTONE_DARK | DRY_GRASS);
        let jungle = matches!(ground, JUNGLE_GRASS | VINE);
        let stony = matches!(ground, STONE | ROCK | GRAVEL);
        match self {
            // A mammoth in the snowbound castles rather than the Colossus; a smith in the desert's
            // caverns rather than an ogre, and a scorpion rather than a bear; a great ape in the
            // jungle's shrines and colonnades, see src\bosses\roster.rs
            Look::Colossus if cold => Look::Hoarback,
            Look::Ogre if desert => Look::Slagwright,
            Look::Ironhide if desert => Look::Sunstinger,
            Look::Treant | Look::Direwolf if jungle => Look::Grovemaul,
            // A wyrm boring through the sand out in front of the desert's colonnades and towers,
            // and the Skymane keeping the towers and colonnades up on the bare rock of the
            // mountains, see src\bosses\wilds.rs
            Look::Direwolf if desert => Look::DuneWyrm,
            Look::Direwolf if stony => Look::Skymane,
            Look::Golem if cold => Look::FrostGolem,
            Look::Golem if desert => Look::MagmaGolem,
            Look::Drake if cold => Look::FrostWyrm,
            Look::Drake if jungle => Look::VenomDrake,
            Look::Drake if stony => Look::StormDrake,
            Look::Ironhide if cold => Look::Frostfang,
            Look::Ogre if cold => Look::RimeJotun,
            Look::Ogre if stony => Look::StormGiant,
            Look::Treant | Look::Elderhorn | Look::Golem if jungle => Look::BogHag,
            Look::Direwolf if cold => Look::WinterAlpha,
            Look::Spider if cold => Look::Rimeweaver,
            _ => self,
        }
    }

    /// The kind of `self`, a boss, that lives in land `land`: the Dreaming Red has its own, the
    /// Lantern Widow where a drake would be, in its flooded churches and towers, and the Dreadmare
    /// where an alpha wolf would roam; and the Sky Isles theirs, the Aeolith where a great bear would
    /// be and the Stormbell where a treant would, see src\bosses\roster.rs
    pub fn for_land(self, land: Land) -> Look {
        match (self, land) {
            (Look::Drake, Land::Dream) => Look::LanternWidow,
            (Look::Direwolf, Land::Dream) => Look::Dreadmare,
            // The Caldera Wyrm boring through the ash of the Cinder Crown where a wolf or a spider
            // would be, see src\bosses\wilds.rs
            (Look::Direwolf | Look::Spider, Land::Dragon) => Look::CalderaWyrm,
            (Look::Ironhide, Land::Sky) => Look::Aeolith,
            (Look::Treant, Land::Sky) => Look::Stormbell,
            _ => self,
        }
    }

    pub(crate) fn skin(self) -> Skin {
        let make = |body, dark, accent, glow, extra| Skin { body, dark, accent, glow, extra };
        match self {
            Look::FrostGolem => make(ICE, STONE, SNOW, GLOW_FROST, CRYSTAL),
            Look::MagmaGolem => make(OBSIDIAN, BARK_DARK, GLOW_FIRE, GLOW_FIRE, GLOW_FIRE),
            Look::Ironhide => make(FUR_DARK, FUR_BROWN, DARK_STEEL, GLOW_BLOOD, BONE),
            Look::Frostfang => make(FUR_WHITE, FUR_GRAY, STEEL, GLOW_FROST, ICE),
            Look::Spider => make(FUR_DARK, BARK_DARK, GLOW_POISON, GLOW_BLOOD, BONE),
            // Dark enough to stand out against the snow it lives on
            Look::Rimeweaver => make(STEEL, OBSIDIAN, GLOW_FROST, GLOW_FROST, ICE),
            Look::Spiderling => make(FUR_BROWN, BARK_DARK, GLOW_POISON, GLOW_BLOOD, BONE),
            Look::Treant => make(BARK_DARK, WOOD, MOSS, GLOW_POISON, LEAVES),
            Look::Direwolf => make(FUR_DARK, FUR_GRAY, FUR_GRAY, GLOW_BLOOD, BONE),
            Look::WinterAlpha => make(FUR_GRAY, FUR_DARK, FUR_WHITE, GLOW_FROST, ICE),
            Look::Wolf => make(FUR_GRAY, FUR_DARK, FUR_CREAM, GLOW_FIRE, BONE),
            // The walking dead: their bones or skin, rags, armor, the light in their eyes, and
            // their blade. A fallen king's bones in black armor, void burning in its eyes and its
            // chest, its crown and trim gone to tarnished bronze; see knight().
            Look::GraveKnight => make(BONE, BARK_DARK, DARK_STEEL, GLOW_VOID, BRONZE),
            // Raid bosses: a colossus of dark stone veined with fire, and a titan of the dead in
            // gold-chased black armor
            Look::Colossus => make(STONE, OBSIDIAN, GLOW_FIRE, GLOW_FIRE, CRYSTAL),
            Look::CryptTitan => make(BONE, BARK_DARK, OBSIDIAN, GLOW_VOID, GOLD),
            // An ogre's tan hide, leather, a fur kilt, eyes like embers and tusks; a jotun's skin
            // blue as ice, steel, white furs, frost in its eyes and icicles; a thunderborn giant grey
            // as the mountain, bound in bronze, lightning in its eyes and gold; a pharaoh's linen
            // wrappings, dark sandstone, gold, the sun in its eyes and a teal-striped headdress; a
            // crone's mossy skin, rags, leather, venom glowing and dead wood; an elderhorn's brown
            // coat, darker legs, moss on its antlers, eyes of gold and antlers of bone; the Hollow
            // Mother pale as cream, black hair, a red veil, blood in her heart and a crown of bone;
            // the Bleeding Hart dark as old blood, crimson brambles for antlers, blood in its eyes
            Look::Ogre => make(FUR_TAN, LEATHER, FUR_BROWN, GLOW_FIRE, BONE),
            Look::RimeJotun => make(ICE, STEEL, FUR_WHITE, GLOW_FROST, CRYSTAL),
            Look::StormGiant => make(STONE, DARK_STEEL, BRONZE, GLOW_STORM, GOLD),
            Look::SandPharaoh => make(LINEN, SANDSTONE_DARK, GOLD, GLOW_FIRE, TEMPLE_TEAL),
            Look::BogHag => make(MOSS, BARK_DARK, LEATHER, GLOW_POISON, DEAD_WOOD),
            Look::Elderhorn => make(FUR_BROWN, FUR_DARK, MOSS, GOLD, BONE),
            Look::HollowMother => make(FUR_CREAM, OBSIDIAN, TEMPLE_RED, GLOW_BLOOD, BONE),
            Look::BleedingHart => make(FUR_DARK, OBSIDIAN, LEAVES_CRIMSON, GLOW_BLOOD, BONE),
            _ if crate::bosses::roster::has(self) => crate::bosses::roster::skin(self),
            Look::Skeleton => make(BONE, BARK_DARK, CLOTH, GLOW_FROST, STEEL),
            Look::ArmoredSkeleton => make(BONE, BARK_DARK, STEEL, GLOW_FROST, STEEL),
            Look::Zombie => make(MOSS, PINE, LINEN, GLOW_POISON, BONE),
            Look::ArmoredZombie => make(MOSS, PINE, DARK_STEEL, GLOW_POISON, BONE),
            // A bloater pale and swollen, veined and blotched green, venom glowing through its split
            // skin; a ghoul grey as ash with red eyes and bone claws; a brute's grey-green hide,
            // rags, a gravestone of pale stone; an archer's bones in a dark hood, its bow of dark
            // wood; a necromancer's bones in a black robe trimmed with bronze, void in its eyes
            Look::Bloater => make(FUR_CREAM, MOSS, BARK_DARK, GLOW_POISON, CACTUS),
            Look::Ghoul => make(FUR_GRAY, OBSIDIAN, BARK_DARK, GLOW_BLOOD, BONE),
            Look::GraveBrute => make(MOSS, BARK_DARK, LEATHER, GLOW_POISON, LIMESTONE),
            Look::BoneArcher => make(BONE, BARK_DARK, TEMPLE_DARK, GLOW_FROST, BARK_DARK),
            Look::Necromancer => make(BONE, OBSIDIAN, BRONZE, GLOW_VOID, DEAD_WOOD),
            // Bandits: their skin, their hood or cloak and breeches, their shirt or armor, their
            // eyes, and their blade. The warlord in bronze with a red cloak.
            Look::Bandit => make(FUR_TAN, LEATHER, CLOTH, EYE, STEEL),
            Look::BanditBrute => make(FUR_TAN, LEATHER, DARK_STEEL, EYE, STEEL),
            Look::Warlord => make(FUR_TAN, TEMPLE_RED, BRONZE, EYE, DARK_STEEL),
            // A crossbowman in a green jerkin and a dark hat, and a shieldbearer in blackened iron
            // over leather, a mace in hand
            Look::Crossbowman => make(FUR_TAN, BARK_DARK, MOSS, EYE, WOOD),
            Look::Shieldbearer => make(FUR_TAN, LEATHER, DARK_STEEL, EYE, STEEL),
            // Plates the color of the sand, banded darker, its stinger glowing with venom and its
            // pincers tipped black
            Look::DuneBurrower => make(SANDSTONE, SANDSTONE_DARK, BRONZE, GLOW_POISON, OBSIDIAN),
            // Dark rags, so it shows against the snow, with frost-blue light in its eyes and ice
            // for claws and a crown
            Look::RimeWraith => make(DARK_STEEL, OBSIDIAN, TEMPLE_TEAL, GLOW_FROST, ICE),
            // Mossy green hide, a pale belly, gold eyes and a red tongue
            Look::MireLurker => make(MOSS, PINE, FUR_CREAM, GOLD, FUR_RED),
            // Dark fur, leathery wings on dark finger bones, red eyes and a pale bony snout
            Look::Gloomwing => make(FUR_DARK, LEATHER, BARK_DARK, GLOW_BLOOD, BONE),
            // Brown hide, black bristles and hooves, a tan snout, red eyes and bone tusks
            Look::Tusker => make(FUR_BROWN, FUR_DARK, FUR_TAN, GLOW_FIRE, BONE),
            _ => make(ROCK, GRAVEL, MOSS, GLOW_FROST, CRYSTAL),
        }
    }

    pub fn of_prop(kind: PropKind) -> Look {
        match kind {
            PropKind::Candle => Look::Candle,
            PropKind::Skull => Look::Skull,
            PropKind::Chest => Look::Chest,
        }
    }

    /// Voxels per meter of its model: the same size as the world's, like everything else but what's
    /// held right in front of the eye, a grappling hook or wings, finer like a weapon in hand
    pub(crate) fn resolution(self) -> f64 {
        match self {
            Look::GrappleHook => 3.0 * MODEL_RES,
            Look::Wings => 4.0 * MODEL_RES,
            // Fish are small, and finer, so a tail has room to sway
            _ if sealife::is_small(self) => sealife::FINE * MODEL_RES,
            // Wolves made finer, to see what that costs and how it looks: their frames baked
            // again from their shapes at the finer grid, see src\baked.rs, smoother, with a little
            // more on them that only shows then
            Look::Wolf => FINER * MODEL_RES,
            _ => MODEL_RES,
        }
    }

    /// Whether its model's made finer than the world's for what only shows then, see FINER
    pub(crate) fn finer(self) -> bool {
        self.resolution() >= FINER * MODEL_RES && !matches!(self, Look::GrappleHook | Look::Wings) && !sealife::is_small(self)
    }

    /// Radius of a sphere around the whole model wherever it's turned, in meters
    pub fn reach(self) -> f64 {
        match self {
            Look::Chest | Look::OpenChest => 0.95,
            // From their feet up to the top of the pack on its back
            Look::Merchant => 2.2,
            Look::Villager | Look::Townswoman | Look::Princess => 1.95,
            // Up to the tip of the spear
            Look::RoyalGuard => 2.55,
            Look::Coins | Look::Potion | Look::Bread => 0.25,
            Look::Bear => 1.6,
            Look::Dog => 0.8,
            Look::GrayCat | Look::GingerCat => 0.45,
            Look::Squirrel => 0.3,
            Look::Apple => 0.14,
            Look::Meat => 0.22,
            Look::Candle => 0.2,
            Look::Skull => 0.2,
            Look::Golem | Look::FrostGolem | Look::MagmaGolem | Look::Colossus => 4.4 * self.size(),
            // The drakes are dragons, drawn in jointed pieces, see src\dragons.rs
            Look::Drake | Look::FrostWyrm | Look::VenomDrake | Look::StormDrake | Look::ElderWyrm => crate::dragons::reach(self),
            Look::Ironhide | Look::Frostfang => 4.0,
            Look::Spider | Look::Rimeweaver | Look::Spiderling => 3.1 * self.scale(),
            Look::Treant => 5.5,
            // Their arms or clubs raised high overhead, or antlers reared up
            Look::Ogre | Look::RimeJotun | Look::StormGiant => 4.6 * self.scale(),
            Look::SandPharaoh | Look::BogHag => 3.0 * self.scale(),
            Look::HollowMother => 3.6 * self.scale(),
            Look::Elderhorn | Look::BleedingHart => 4.0 * self.scale(),
            Look::Wolf => 1.2 * self.scale(),
            Look::Direwolf | Look::WinterAlpha => 1.5 * self.scale(),
            // A greatsword raised high overhead, or swept out to its side
            Look::GraveKnight | Look::CryptTitan => 3.45 * self.scale(),
            // A blade raised overhead
            Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie => 2.5 * self.scale(),
            Look::Bloater => 1.9 * self.scale(),
            // Its claws thrown high as it leaps
            Look::Ghoul => 2.3,
            // The gravestone raised high overhead
            Look::GraveBrute => 3.1 * self.scale(),
            Look::BoneArcher => 2.3,
            // Its staff raised high
            Look::Necromancer => 2.9,
            Look::Bandit | Look::BanditBrute | Look::Warlord | Look::Crossbowman => 2.5 * self.scale(),
            // A mace raised high over its shield
            Look::Shieldbearer => 2.6 * self.scale(),
            // Its stinger whipped over its head, or laid out behind as it digs
            Look::DuneBurrower => 1.9 * self.scale(),
            // Up to its claws raised high, drifting over the ground
            Look::RimeWraith => 2.7,
            Look::MireLurker => 1.5,
            // Flying, its wings up
            Look::Gloomwing => 2.4,
            Look::Tusker => 1.95,
            // Up to the stone on top of its lintel, and out to the corners of its dais
            Look::Teleporter => 8.3,
            Look::RealmPortal => 7.9,
            Look::PortalVeil => 6.6,
            Look::InnertBlock => 0.28,
            Look::GrappleHook => 0.5,
            Look::Wings => 1.5,
            Look::Ashwing | Look::Sleetwing | Look::Squallwing | Look::Blightwing | Look::PyreMatriarch | Look::CindermawTyrant | Look::GildscaleHoardwarden | Look::WorldpyreAncient => crate::dragons::reach(self),
            // Its head lunged out ahead of it, see serpents::spine()
            Look::ReedViper | Look::SiltCoiler | Look::GaleSerpent | Look::BloodAdder | Look::DriftSerpent => serpents::kind(self).map_or(2.5, serpents::reach),
            // Only its head, as the rest of it's drawn a piece at a time, see serpents::Coils
            Look::Meremaw | Look::Galecoil | Look::Veinbinder | Look::Shatterfang | Look::Hollowtide | Look::Reefcoil | Look::Gyremaw | Look::Ysveth | Look::DuneWyrm | Look::CalderaWyrm => {
                serpents::great_of(self).map_or(4.0, |g| g.head + 2.0 * g.girth)
            }
            // Its arms spread out round it, see src\bosses\kraken.rs; its wings spread, see
            // src\dragons\skymane.rs
            Look::Kraken | Look::GrottoSquid => crate::bosses::kraken::reach(self),
            Look::Skymane => crate::dragons::reach(self),
            Look::ShedSkin => 2.0,
            Look::SerpentBones => 3.0,
            Look::SerpentTongue => 0.3,
            // Its wings spread, see src\avians.rs
            Look::Harpy | Look::Skyhunter | Look::Stormtalon | Look::Harridan | Look::Roc | Look::Thunderbird | Look::HarpyQueen => crate::avians::reach(self),
            Look::Gargoyle | Look::BloodGargoyle | Look::RimeGargoyle | Look::CinderGargoyle | Look::MossGargoyle | Look::Grotesque | Look::BloodwakeGargoyle => crate::avians::reach(self),
            Look::Slagwright | Look::Cinderback | Look::LanternWidow | Look::Dreadmare | Look::Sunstinger | Look::Grovemaul | Look::Hoarback | Look::GildedSphinx => crate::bosses::roster::reach(self),
            Look::Aeolith | Look::Stormbell => crate::bosses::roster::sky::reach(self),
            // Up to the top of a hat, a staff or a bow held upright, or wings on the back
            Look::Bowyer | Look::Wandwright | Look::Bladesmith | Look::Gunsmith | Look::Jeweller | Look::Wingmaker => 2.2,
            Look::TradeStall => 2.0,
            Look::Silverfin | Look::Reefling | Look::Brookback | Look::Gloamfin | Look::Frostfin | Look::Saltjaw | Look::Ghostmaw | Look::RawFish | Look::CookedFish | Look::SharkTooth => sealife::reach(self),
        }
    }

    /// The body to aim at: how high its middle is, and how big it is, in meters
    pub fn body(self) -> (f64, f64) {
        match self {
            Look::Bear => (0.85, 0.75),
            Look::Dog => (0.5, 0.35),
            Look::GrayCat | Look::GingerCat => (0.28, 0.22),
            Look::Squirrel => (0.14, 0.16),
            Look::Apple => (0.06, 0.2),
            Look::Meat => (0.05, 0.25),
            Look::Candle => (0.1, 0.1),
            Look::Skull => (0.1, 0.12),
            Look::Chest | Look::OpenChest => (0.22, 0.4),
            Look::Merchant => (0.95, 0.45),
            Look::Villager | Look::Townswoman | Look::Princess => (0.95, 0.4),
            Look::RoyalGuard => (1.0, 0.42),
            Look::Coins | Look::Potion | Look::Bread => (0.08, 0.22),
            Look::Golem | Look::FrostGolem | Look::MagmaGolem | Look::Colossus => (2.0 * self.size(), 1.1 * self.size()),
            Look::Drake | Look::FrostWyrm | Look::VenomDrake | Look::StormDrake | Look::ElderWyrm => crate::dragons::body(self),
            Look::Ironhide | Look::Frostfang => (1.6, 1.05),
            Look::Spider | Look::Rimeweaver | Look::Spiderling => (0.9 * self.scale(), 0.95 * self.scale()),
            Look::Treant => (2.4, 1.2),
            Look::Ogre | Look::RimeJotun | Look::StormGiant => (1.5 * self.scale(), 0.55 * self.scale()),
            Look::SandPharaoh => (1.0 * self.scale(), 0.36 * self.scale()),
            Look::BogHag => (0.8 * self.scale(), 0.4 * self.scale()),
            // Floating over the ground
            Look::HollowMother => (1.5 * self.scale(), 0.4 * self.scale()),
            Look::Elderhorn | Look::BleedingHart => (1.2 * self.scale(), 0.55 * self.scale()),
            Look::Wolf | Look::Direwolf | Look::WinterAlpha => (0.55 * self.scale(), 0.42 * self.scale()),
            Look::Skeleton | Look::ArmoredSkeleton | Look::Zombie | Look::ArmoredZombie | Look::GraveKnight | Look::CryptTitan => (1.0 * self.scale(), 0.36 * self.scale()),
            Look::Bandit | Look::BanditBrute | Look::Warlord | Look::Crossbowman | Look::Shieldbearer => (1.0 * self.scale(), 0.36 * self.scale()),
            Look::Bloater => (0.95 * self.scale(), 0.55 * self.scale()),
            // Crouched low
            Look::Ghoul => (0.8, 0.36),
            Look::GraveBrute => (1.0 * self.scale(), 0.42 * self.scale()),
            Look::BoneArcher | Look::Necromancer => (1.0, 0.36),
            Look::DuneBurrower => (0.4 * self.scale(), 0.6 * self.scale()),
            Look::RimeWraith => (1.4, 0.42),
            Look::MireLurker => (0.42, 0.62),
            // Up where it flies
            Look::Gloomwing => (1.65, 0.34),
            Look::Tusker => (0.85, 0.74),
            Look::Teleporter => (2.6, 2.3),
            Look::RealmPortal => (2.6, 2.1),
            Look::PortalVeil => (2.7, 1.2),
            Look::InnertBlock => (0.12, 0.3),
            Look::GrappleHook => (0.1, 0.3),
            Look::Wings => (0.3, 0.6),
            Look::Ashwing | Look::Sleetwing | Look::Squallwing | Look::Blightwing | Look::PyreMatriarch | Look::CindermawTyrant | Look::GildscaleHoardwarden | Look::WorldpyreAncient => crate::dragons::body(self),
            // Low along the ground, and only as thick as its body, as it's hit wherever along it
            // the blow's aimed, see serpents::focus()
            Look::ReedViper | Look::SiltCoiler | Look::GaleSerpent | Look::BloodAdder | Look::DriftSerpent => (0.3, 0.42),
            // Its head and neck, or wherever along its body the blow's aimed, see serpents::focus()
            Look::Meremaw | Look::Galecoil | Look::Veinbinder | Look::Shatterfang | Look::Hollowtide | Look::Reefcoil | Look::Gyremaw | Look::Ysveth | Look::DuneWyrm | Look::CalderaWyrm => {
                serpents::great_of(self).map_or((1.0, 1.0), |g| (g.girth + g.hover, g.girth * 1.2))
            }
            // Its head and mantle, over the water, see src\bosses\kraken.rs
            Look::Kraken | Look::GrottoSquid => crate::bosses::kraken::body(self),
            Look::Skymane => crate::dragons::body(self),
            Look::ShedSkin => (0.05, 0.3),
            Look::SerpentBones => (0.3, 0.6),
            Look::SerpentTongue => (0.0, 0.05),
            // Wherever on it the blow's aimed, see avians::Flier::aim()
            Look::Harpy | Look::Skyhunter | Look::Stormtalon | Look::Harridan | Look::Roc | Look::Thunderbird | Look::HarpyQueen => crate::avians::body(self),
            Look::Gargoyle | Look::BloodGargoyle | Look::RimeGargoyle | Look::CinderGargoyle | Look::MossGargoyle | Look::Grotesque | Look::BloodwakeGargoyle => crate::avians::body(self),
            Look::Slagwright | Look::Cinderback | Look::LanternWidow | Look::Dreadmare | Look::Sunstinger | Look::Grovemaul | Look::Hoarback | Look::GildedSphinx => crate::bosses::roster::body(self),
            Look::Aeolith | Look::Stormbell => crate::bosses::roster::sky::body(self),
            Look::Bowyer | Look::Wandwright | Look::Bladesmith | Look::Gunsmith | Look::Jeweller | Look::Wingmaker => (0.95, 0.42),
            Look::TradeStall => (0.5, 0.6),
            Look::Silverfin | Look::Reefling | Look::Brookback | Look::Gloamfin | Look::Frostfin | Look::Saltjaw | Look::Ghostmaw | Look::RawFish | Look::CookedFish | Look::SharkTooth => sealife::body(self),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Look::Bear => "bear",
            Look::Dog => "dog",
            Look::GrayCat | Look::GingerCat => "cat",
            Look::Squirrel => "squirrel",
            Look::Apple => "apple",
            Look::Meat => "meat",
            Look::Candle => "candle",
            Look::Skull => "skull",
            Look::Chest | Look::OpenChest => "chest",
            Look::Merchant => "merchant",
            Look::Villager | Look::Townswoman => "villager",
            Look::Princess => "princess",
            Look::RoyalGuard => "royal guard",
            Look::Coins => "gold",
            Look::Potion => "healing potion",
            Look::Bread => "loaf of bread",
            Look::Golem => "Stone Warden",
            Look::Drake => "Ember Drake",
            Look::Ironhide => "Ironhide",
            Look::FrostGolem => "Frost Colossus",
            Look::MagmaGolem => "Magma Golem",
            Look::FrostWyrm => "Frost Wyrm",
            Look::VenomDrake => "Venom Drake",
            Look::Frostfang => "Frostfang",
            Look::Spider => "Broodmother",
            Look::Treant => "Ancient Treant",
            Look::Direwolf => "Alpha Direwolf",
            Look::WinterAlpha => "Winter Alpha",
            Look::GraveKnight => "Grave Knight",
            Look::StormDrake => "Thunder Drake",
            Look::Rimeweaver => "Rimeweaver",
            Look::Colossus => "Earthbreaker Colossus",
            Look::ElderWyrm => "Elder Cinder Wyrm",
            Look::CryptTitan => "Crypt Titan",
            Look::Ogre => "Cragmaw Ogre",
            Look::RimeJotun => "Rime Jotun",
            Look::StormGiant => "Thunderborn Giant",
            Look::SandPharaoh => "Sunken Pharaoh",
            Look::BogHag => "Mire Crone",
            Look::Elderhorn => "Elderhorn",
            Look::HollowMother => "Hollow Mother",
            Look::BleedingHart => "Bleeding Hart",
            Look::Wolf => "wolf",
            Look::Skeleton => "skeleton",
            Look::ArmoredSkeleton => "armoured skeleton",
            Look::Zombie => "zombie",
            Look::ArmoredZombie => "armoured zombie",
            Look::Spiderling => "cave spider",
            Look::Bandit => "bandit",
            Look::BanditBrute => "bandit brute",
            Look::Warlord => "Bandit Warlord",
            Look::Crossbowman => "bandit crossbowman",
            Look::Shieldbearer => "bandit shieldbearer",
            Look::DuneBurrower => "dune burrower",
            Look::RimeWraith => "rime wraith",
            Look::MireLurker => "mire lurker",
            Look::Gloomwing => "gloomwing",
            Look::Tusker => "tusker",
            Look::Bloater => "bloater",
            Look::Ghoul => "ghoul",
            Look::GraveBrute => "grave brute",
            Look::BoneArcher => "skeleton archer",
            Look::Necromancer => "necromancer",
            Look::Teleporter => "Innert Gate",
            Look::RealmPortal => "portal home",
            Look::PortalVeil => "portal",
            Look::InnertBlock => "Innert block",
            Look::GrappleHook => "grappling hook",
            Look::Wings => "wings",
            Look::Ashwing | Look::Sleetwing | Look::Squallwing | Look::Blightwing | Look::PyreMatriarch | Look::CindermawTyrant | Look::GildscaleHoardwarden | Look::WorldpyreAncient => crate::dragons::name(self),
            Look::ReedViper => "reed viper",
            Look::SiltCoiler => "silt coiler",
            Look::GaleSerpent => "gale serpent",
            Look::BloodAdder => "bloodvein adder",
            Look::DriftSerpent => "drift serpent",
            Look::Meremaw => "Meremaw",
            Look::Galecoil => "Galecoil",
            Look::Veinbinder => "Veinbinder",
            Look::Shatterfang => "Shatterfang",
            Look::Hollowtide => "Hollowtide",
            Look::Reefcoil => "Reefcoil",
            Look::Gyremaw => "Gyremaw",
            Look::Ysveth => "Coil Unending",
            Look::ShedSkin => "shed serpent skin",
            Look::SerpentBones => "serpent bones",
            Look::SerpentTongue => "serpent's tongue",
            Look::Harpy | Look::Skyhunter | Look::Stormtalon | Look::Harridan | Look::Roc | Look::Thunderbird | Look::HarpyQueen => crate::avians::name(self),
            Look::Gargoyle | Look::BloodGargoyle | Look::RimeGargoyle | Look::CinderGargoyle | Look::MossGargoyle | Look::Grotesque | Look::BloodwakeGargoyle => crate::avians::name(self),
            Look::Slagwright | Look::Cinderback | Look::LanternWidow | Look::Dreadmare | Look::Sunstinger | Look::Grovemaul | Look::Hoarback | Look::GildedSphinx => crate::bosses::roster::name(self),
            Look::Aeolith | Look::Stormbell => crate::bosses::roster::sky::name(self),
            Look::Bowyer => "bowyer",
            Look::Wandwright => "wandwright",
            Look::Bladesmith => "weaponsmith",
            Look::Gunsmith => "gunsmith",
            Look::Jeweller => "jeweller",
            Look::Wingmaker => "wingmaker",
            Look::TradeStall => "merchant's stand",
            Look::Silverfin | Look::Reefling | Look::Brookback | Look::Gloamfin | Look::Frostfin | Look::Saltjaw | Look::Ghostmaw | Look::RawFish | Look::CookedFish | Look::SharkTooth => sealife::name(self),
            Look::Kraken | Look::GrottoSquid | Look::DuneWyrm | Look::CalderaWyrm | Look::Skymane => crate::bosses::wilds::name(self),
        }
    }

    /// Pets get petted, the rest can be hunted for meat
    pub fn is_pet(self) -> bool {
        // Merchants can't be hurt either, only traded with, nor can villagers, only talked to
        matches!(self, Look::Dog | Look::GrayCat | Look::GingerCat) || self.is_merchant() || self.is_villager() || self.is_royal()
    }

    /// A merchant, the travelling one or a specialist of a trade, see src\merchants.rs, standing by
    /// their wares
    pub fn is_merchant(self) -> bool {
        matches!(self, Look::Merchant | Look::Bowyer | Look::Wandwright | Look::Bladesmith | Look::Gunsmith | Look::Jeweller | Look::Wingmaker)
    }

    /// The princess of a royal castle or one of her guards, who stay where they are
    pub fn is_royal(self) -> bool {
        matches!(self, Look::Princess | Look::RoyalGuard)
    }

    /// One of the folk living in a town
    pub fn is_villager(self) -> bool {
        matches!(self, Look::Villager | Look::Townswoman)
    }

    /// How much damage it takes to hunt. A bare-handed hit does PUNCH. A monster's or a boss's is
    /// its own before its level's toughness, see levels::toughness(), and as much as any other of
    /// the same sort's for what it is, so a level's as tough to beat whatever realm it's in, see
    /// levels::MONSTER_HEALTH: of monsters, swarmers 120, the quick and those that shoot 150,
    /// most 175, the armored and the stone 210, brutes 245; of lesser bosses, spiders and those
    /// that fly 2350, most 2600, golems, giants and the like 3000; and raid bosses 13500 to 15500.
    pub fn health(self) -> f64 {
        match self {
            // As tough as the bosses of their tier, see src\bosses\roster.rs
            _ if crate::bosses::roster::has(self) => crate::bosses::roster::health(self),
            // A fish dies to any blow; a shark's as tough as the armored or the brutes
            _ if sealife::is_sea(self) => sealife::health(self),
            _ if crate::bosses::wilds::has(self) => crate::bosses::wilds::health(self),
            Look::Bear => 140.0,
            Look::Dog => 60.0,
            Look::GrayCat | Look::GingerCat => 30.0,
            Look::Golem => 3000.0,
            Look::FrostGolem => 3000.0,
            Look::MagmaGolem => 3000.0,
            Look::Drake | Look::VenomDrake => 2600.0,
            Look::FrostWyrm => 2600.0,
            Look::Ironhide => 2600.0,
            Look::Frostfang => 2600.0,
            Look::Spider => 2350.0,
            Look::Treant => 3000.0,
            Look::Direwolf => 2600.0,
            Look::WinterAlpha => 2600.0,
            Look::GraveKnight => 3000.0,
            Look::StormDrake => 2600.0,
            Look::Rimeweaver => 2350.0,
            Look::Colossus => 15500.0,
            Look::ElderWyrm => 13500.0,
            Look::CryptTitan => 15500.0,
            Look::Ogre => 3000.0,
            Look::RimeJotun => 3000.0,
            Look::StormGiant => 3000.0,
            Look::SandPharaoh => 2600.0,
            Look::BogHag => 2600.0,
            Look::Elderhorn => 2600.0,
            Look::HollowMother => 14500.0,
            Look::BleedingHart => 14500.0,
            Look::Wolf => 150.0,
            Look::Skeleton => 175.0,
            Look::ArmoredSkeleton => 210.0,
            Look::Zombie => 175.0,
            Look::ArmoredZombie => 210.0,
            Look::Spiderling => 120.0,
            Look::Bandit => 175.0,
            Look::BanditBrute => 245.0,
            Look::Warlord => 3000.0,
            Look::Crossbowman => 150.0,
            Look::Shieldbearer => 210.0,
            Look::DuneBurrower => 175.0,
            Look::RimeWraith => 175.0,
            Look::MireLurker => 175.0,
            Look::Gloomwing => 120.0,
            Look::Tusker => 245.0,
            Look::Bloater => 175.0,
            Look::Ghoul => 150.0,
            Look::GraveBrute => 245.0,
            Look::BoneArcher => 150.0,
            Look::Necromancer => 175.0,
            _ if self.is_dragon() => crate::dragons::health(self),
            _ if self.is_winged() => crate::avians::health(self),
            Look::ReedViper => 175.0,
            Look::SiltCoiler => 210.0,
            Look::GaleSerpent => 175.0,
            Look::BloodAdder => 175.0,
            Look::DriftSerpent => 175.0,
            Look::Meremaw => 2600.0,
            Look::Galecoil => 2350.0,
            Look::Veinbinder => 3000.0,
            Look::Shatterfang => 2600.0,
            Look::Hollowtide => 14500.0,
            Look::Reefcoil => 14500.0,
            Look::Gyremaw => 13500.0,
            // Two and a half raid serpents' worth, and the same again for each realm's toughness
            // past the normal world's, see serpents::ULTRA_STAGES
            Look::Ysveth => 32000.0,
            _ => 20.0,
        }
    }

    /// How much meat it gives

    pub fn meat(self) -> u32 {
        match self {
            Look::Bear => 3,
            Look::Tusker => 2,
            Look::Squirrel | Look::Wolf | Look::MireLurker => 1,
            Look::Ironhide | Look::Frostfang | Look::Direwolf | Look::WinterAlpha => 6,
            Look::Hoarback => 8,
            _ if crate::dragons::is_drakeling(self) => 1,
            _ => 0,
        }
    }

    // Speeds in meters per second
    pub fn walk_speed(self) -> f64 {
        match self {
            _ if self.is_dragon() => return crate::dragons::speeds(self).0,
            _ if crate::bosses::roster::has(self) => return crate::bosses::roster::speeds(self).0,
            Look::Colossus => return 1.8,
            Look::ElderWyrm => return 2.2,
            Look::CryptTitan => return 1.6,
            Look::Ogre | Look::RimeJotun | Look::StormGiant => return 1.5,
            Look::SandPharaoh => return 1.2,
            Look::BogHag => return 1.0,
            Look::HollowMother => return 1.4,
            Look::Elderhorn => return 1.8,
            Look::BleedingHart => return 2.0,
            Look::Wolf => return 1.8,
            Look::Direwolf | Look::WinterAlpha => return 2.2,
            Look::Zombie | Look::ArmoredZombie => return 0.8,
            Look::Skeleton | Look::ArmoredSkeleton => return 1.4,
            Look::Bloater => return 0.6,
            Look::Ghoul => return 1.6,
            Look::GraveBrute => return 0.75,
            Look::BoneArcher => return 1.3,
            Look::Necromancer => return 1.1,
            Look::Spiderling => return 2.6,
            Look::Bandit | Look::Crossbowman => return 1.5,
            Look::BanditBrute => return 1.2,
            // Edging forward behind its shield
            Look::Shieldbearer => return 1.3,
            Look::Warlord => return 1.4,
            Look::Ysveth => return 2.6,
            _ if self.is_great_serpent() => return 2.2,
            _ => {}
        }
        match self.build() {
            Some(Build::Golem) => 1.3,
            Some(Build::Bear) => 1.6,
            Some(Build::Spider) => 2.4,
            Some(Build::Treant) => 0.9,
            Some(Build::Wolf) => 1.8,
            Some(Build::Undead) => 1.2,
            Some(Build::Burrower) => 1.4,
            Some(Build::Wraith) => 1.3,
            Some(Build::Lurker) => 1.0,
            Some(Build::Bat) => 2.5,
            Some(Build::Boar) => 1.3,
            Some(Build::Dragon) => crate::dragons::speeds(self).0,
            Some(Build::Serpent) => 1.3,
            Some(Build::Winged) => crate::avians::speeds(self).0,
            Some(Build::Fish) => sealife::speeds(self).0,
            Some(Build::Kraken) => crate::bosses::kraken::speeds(self).0,
            None => match self {
                Look::Bear => 1.1,
                Look::Dog => 1.6,
                Look::Squirrel => 1.4,
                // Strolling, in no hurry
                Look::Villager | Look::Townswoman => 0.8,
                _ => 0.9,
            },
        }
    }

    pub fn run_speed(self) -> f64 {
        match self {
            _ if self.is_dragon() => return crate::dragons::speeds(self).1,
            _ if crate::bosses::roster::has(self) => return crate::bosses::roster::speeds(self).1,
            Look::Colossus => return 3.2,
            Look::ElderWyrm => return 5.5,
            Look::CryptTitan => return 3.4,
            Look::Ogre | Look::RimeJotun | Look::StormGiant => return 3.2,
            Look::SandPharaoh => return 2.4,
            Look::BogHag => return 2.2,
            Look::HollowMother => return 3.0,
            // Charging, see bosses::Boss::haste()
            Look::Elderhorn => return 7.5,
            Look::BleedingHart => return 8.0,
            Look::Wolf => return 6.5,
            Look::Direwolf | Look::WinterAlpha => return 8.0,
            Look::Skeleton => return 3.2,
            Look::ArmoredSkeleton | Look::GraveKnight => return 2.8,
            // Shambling, but quicker than they look
            Look::Zombie => return 2.6,
            Look::ArmoredZombie => return 2.2,
            // Waddling, but it gets there; a ghoul's on the player in a moment
            Look::Bloater => return 1.7,
            Look::Ghoul => return 4.8,
            Look::GraveBrute => return 1.9,
            Look::BoneArcher => return 3.0,
            Look::Necromancer => return 2.4,
            Look::Spiderling => return 7.0,
            // Quicker than the dead, and they come running
            Look::Bandit => return 4.2,
            Look::Crossbowman => return 4.0,
            Look::BanditBrute => return 3.0,
            Look::Shieldbearer => return 2.6,
            Look::Warlord => return 3.4,
            // Quick, but not so quick it can't be got away from; a coiler's quickest in the water,
            // and a gale serpent gliding
            Look::ReedViper => return 4.4,
            Look::SiltCoiler => return 4.0,
            Look::GaleSerpent => return 6.2,
            Look::BloodAdder => return 3.8,
            Look::DriftSerpent => return 4.2,
            // A great serpent's head comes on quickly, its body pouring after it
            Look::Galecoil => return 7.0,
            Look::Ysveth => return 6.5,
            _ if self.is_great_serpent() => return 5.5,
            _ => {}
        }
        match self.build() {
            Some(Build::Golem) => 2.4,
            // Charging
            Some(Build::Bear) => 10.0,
            Some(Build::Spider) => 6.0,
            Some(Build::Treant) => 1.6,
            Some(Build::Wolf) => 6.5,
            Some(Build::Undead) => 3.0,
            Some(Build::Burrower) => 3.8,
            Some(Build::Wraith) => 3.4,
            // Lunging along in bursts
            Some(Build::Lurker) => 4.4,
            Some(Build::Bat) => 7.5,
            Some(Build::Boar) => 5.2,
            Some(Build::Dragon) => crate::dragons::speeds(self).1,
            Some(Build::Serpent) => 4.2,
            Some(Build::Winged) => crate::avians::speeds(self).1,
            Some(Build::Fish) => sealife::speeds(self).1,
            Some(Build::Kraken) => crate::bosses::kraken::speeds(self).1,
            None => match self {
                Look::Bear => 3.2,
                Look::Dog => 4.5,
                Look::Squirrel => 4.0,
                _ => 3.0,
            },
        }
    }

    /// Meters covered by one cycle of its walk
    pub fn stride(self) -> f64 {
        match self {
            Look::Ogre | Look::RimeJotun | Look::StormGiant => return 1.7 * self.scale(),
            Look::SandPharaoh | Look::BogHag => return 1.2 * self.scale(),
            Look::HollowMother => return 1.6 * self.scale(),
            Look::Elderhorn | Look::BleedingHart => return 2.4 * self.scale(),
            _ if crate::bosses::roster::has(self) => return crate::bosses::roster::stride(self) * self.scale(),
            _ => {}
        }
        match self.build() {
            Some(Build::Dragon) => crate::dragons::stride(self),
            Some(Build::Golem) => 2.4 * self.size(),
            Some(Build::Bear) => 2.6 * self.size(),
            Some(Build::Spider) => 1.8 * self.scale(),
            Some(Build::Treant) => 2.8,
            Some(Build::Wolf) => 1.4 * self.scale(),
            Some(Build::Undead) => 1.3 * self.scale(),
            Some(Build::Burrower) => 0.9 * self.scale(),
            // A slow drift, rising and falling
            Some(Build::Wraith) => 2.4,
            Some(Build::Lurker) => 1.2,
            Some(Build::Bat) => 1.6,
            Some(Build::Boar) => 1.9,
            // As far as its S-curve's waves are apart, see serpents::stride()
            Some(Build::Serpent) => serpents::kind(self).map_or(1.7, serpents::stride),
            Some(Build::Winged) => crate::avians::stride(self),
            // A sweep of its tail, see sealife::act(), which keeps its own time
            Some(Build::Fish) => 2.0,
            // A pull of its arms through the water
            Some(Build::Kraken) => 3.0,
            None => match self {
                Look::Bear => 1.5,
                Look::Dog => 0.9,
                Look::Squirrel => 0.35,
                Look::Villager | Look::Townswoman => 1.3,
                _ => 0.55,
            },
        }
    }

    /// Highest ledge it can step up, in meters
    fn climb(self) -> f64 {
        match self {
            Look::Dog => 0.4,
            _ if self.is_boss() => 1.0,
            // Flying, or drifting over it
            Look::Gloomwing | Look::RimeWraith | Look::GaleSerpent => 1.0,
            Look::Tusker | Look::DuneBurrower => 0.6,
            // Pouring itself up over a ledge
            _ if self.is_serpent() => 0.7,
            _ => 0.5,
        }
    }
}

/// Frame 0 stands still, 1 to WALK_FRAMES are a walk cycle, then come a boss's poses as it
/// attacks, see POSE_WINDUP and the rest, and frames partway between them to ease from one to the
/// next, see tween(). Only bosses and monsters have the poses, and only knights the last four.
/// Every look has every frame, those it never shows left empty, so a frame's model is found the
/// same way for all of them, see model_index().
const WALK_FRAMES: usize = 20;
/// Getting ready to attack
pub const POSE_WINDUP: usize = WALK_FRAMES + 1;
/// Bringing it down
pub const POSE_STRIKE: usize = WALK_FRAMES + 2;
/// Something each boss does its own way: throwing, spreading its wings or lowering its head to charge
pub const POSE_SPECIAL: usize = WALK_FRAMES + 3;
/// A knight crouched low, its greatsword drawn back behind it to sweep it round, see knight(); and
/// a serpent lying coiled up at rest, see serpents::spine()
pub const POSE_COIL: usize = WALK_FRAMES + 4;
/// The greatsword swept out flat to its side, low over the ground
pub const POSE_SWEEP: usize = WALK_FRAMES + 5;
/// The greatsword raised high in both hands, point down
pub const POSE_RAISE: usize = WALK_FRAMES + 6;
/// The greatsword driven down into the ground, bone spikes bursting up all round
pub const POSE_SPIKES: usize = WALK_FRAMES + 7;

/// Two poses with frames between them to ease from one to the other, 0 being standing: how many
/// frames, and seconds it takes to go from the first to the second and back. Getting ready and
/// settling after are slow, so they're seen coming; blows are quick, so they land hard.
#[derive(Clone, Copy)]
struct Tween {
    from: usize,
    to: usize,
    steps: usize,
    there: f64,
    back: f64,
    /// Only knights have these
    knights: bool,
}

const fn tween_of(from: usize, to: usize, steps: usize, there: f64, back: f64, knights: bool) -> Tween {
    Tween { from, to, steps, there, back, knights }
}

/// Every pair of poses a boss goes between, so it never snaps from one to the next
const TWEENS: [Tween; 12] = [
    tween_of(0, POSE_WINDUP, 3, 0.28, 0.3, false),
    tween_of(POSE_WINDUP, POSE_STRIKE, 1, 0.08, 0.2, false),
    tween_of(POSE_STRIKE, 0, 3, 0.4, 0.15, false),
    tween_of(0, POSE_SPECIAL, 2, 0.3, 0.35, false),
    tween_of(POSE_SPECIAL, POSE_STRIKE, 1, 0.08, 0.2, false),
    tween_of(POSE_WINDUP, POSE_SPECIAL, 2, 0.2, 0.2, false),
    tween_of(0, POSE_COIL, 2, 0.3, 0.3, true),
    tween_of(POSE_COIL, POSE_SWEEP, 1, 0.07, 0.2, true),
    tween_of(POSE_SWEEP, 0, 3, 0.4, 0.2, true),
    tween_of(0, POSE_RAISE, 2, 0.32, 0.3, true),
    tween_of(POSE_RAISE, POSE_SPIKES, 1, 0.09, 0.2, true),
    tween_of(POSE_SPIKES, 0, 2, 0.45, 0.2, true),
];

const fn tween_frames() -> usize {
    let mut n = 0;
    let mut k = 0;
    while k < TWEENS.len() {
        n += TWEENS[k].steps;
        k += 1;
    }
    n
}

/// The first of the frames between poses
const FIRST_TWEEN: usize = POSE_SPIKES + 1;
pub(crate) const FRAMES: usize = FIRST_TWEEN + tween_frames();

/// Which frame to draw `look` in `since` seconds after it went from pose `a` to pose `b`, 0 being
/// standing, easing in and out: None once it's got there, or if it has no frames between them
pub fn tween(look: Look, a: usize, b: usize, since: f64) -> Option<usize> {
    let mut first = FIRST_TWEEN;
    for t in TWEENS {
        let forward = (t.from, t.to) == (a, b);
        if forward || (t.to, t.from) == (a, b) {
            if !has_tween(look, t) {
                return None;
            }
            // A serpent settles slowly into its coils and comes out of them quickly
            let slow = if look.is_serpent() && t.knights { if forward == (t.to == POSE_COIL) { 3.3 } else { 1.2 } } else { 1.0 };
            let f = since / (slow * if forward { t.there } else { t.back });
            if f >= 1.0 {
                return None;
            }
            let eased = f * f * (3.0 - 2.0 * f);
            // How far along from `t.from` to `t.to`
            let along = if forward { eased } else { 1.0 - eased };
            let k = (along * (t.steps + 1) as f64).round() as usize;
            return Some(match k {
                0 => t.from,
                k if k > t.steps => t.to,
                k => first + k - 1,
            });
        }
        first += t.steps;
    }
    None
}

/// Seconds something posed live takes to go from one pose to another it has no frames between,
/// eased, rather than snapping, see tween_path()
const CROSSFADE: f64 = 0.16;

/// What `look` goes through `since` seconds after it went from pose `a` to pose `b`, 0 being
/// standing or walking, to pose it live, see src\baked.rs: the frames from the one to the other,
/// the first `a` and the last `b`, and how far along them it is, 0 to 1, eased in and out as
/// tween() eases them. Two poses with no frames between are gone between in CROSSFADE seconds.
/// None once it's got there.
pub fn tween_path(look: Look, a: usize, b: usize, since: f64) -> Option<(Vec<usize>, f64)> {
    if a == b {
        return None;
    }
    let mut first = FIRST_TWEEN;
    for t in TWEENS {
        let forward = (t.from, t.to) == (a, b);
        if (forward || (t.to, t.from) == (a, b)) && has_tween(look, t) {
            let slow = if look.is_serpent() && t.knights { if forward == (t.to == POSE_COIL) { 3.3 } else { 1.2 } } else { 1.0 };
            let f = since / (slow * if forward { t.there } else { t.back });
            if f >= 1.0 {
                return None;
            }
            let mut path: Vec<usize> = std::iter::once(t.from).chain(first..first + t.steps).chain(std::iter::once(t.to)).collect();
            if !forward {
                path.reverse();
            }
            return Some((path, f * f * (3.0 - 2.0 * f)));
        }
        first += t.steps;
    }
    let f = since / CROSSFADE;
    (f < 1.0).then(|| (vec![a, b], f * f * (3.0 - 2.0 * f)))
}

/// Whether `look` goes between the poses of `t`: only knights have theirs, but for a serpent
/// settling into its coils, which is a knight's crouch's frame, see POSE_COIL
fn has_tween(look: Look, t: Tween) -> bool {
    !t.knights || look.is_knight() || look.is_serpent() && (t.from, t.to) == (0, POSE_COIL)
}

/// Whether `look` is ever drawn in `frame`, so its model's worth making
fn shown(look: Look, frame: usize) -> bool {
    match frame {
        // A great serpent's drawn a piece at a time, with models of its own, see serpents::Coils
        _ if look.is_great_serpent() => false,
        // And the bosses drawn out of jointed pieces, see src\rig.rs, and a kraken's, a piece at
        // a time, see src\bosses\kraken.rs
        _ if crate::rig::rigged(look) || look.is_kraken() => false,
        0 => true,
        // Dragons are drawn out of jointed pieces of their own, see src\dragons.rs, and so are
        // avians and gargoyles, see src\avians.rs
        _ if look.is_dragon() || look.is_winged() => false,
        _ if frame <= WALK_FRAMES => look.walks(),
        _ if look.build().is_none() => false,
        _ if frame < POSE_COIL => true,
        POSE_COIL if look.is_serpent() => true,
        _ if frame < FIRST_TWEEN => look.is_knight(),
        _ => has_tween(look, tween_at(frame).0),
    }
}

/// The pair of poses frame `frame` is between, and how far from the first to the second
fn tween_at(frame: usize) -> (Tween, f64) {
    let mut k = frame - FIRST_TWEEN;
    for t in TWEENS {
        if k < t.steps {
            return (t, (k + 1) as f64 / (t.steps + 1) as f64);
        }
        k -= t.steps;
    }
    unreachable!("frame {frame} is past the last")
}

/// How far a boss is into each of its poses, 0 to 1, so it can be drawn partway between them
#[derive(Clone, Copy)]
struct Pose {
    windup: f64,
    strike: f64,
    special: f64,
    // Only knights strike these, see knight()
    coil: f64,
    sweep: f64,
    raise: f64,
    spikes: f64,
}

impl Pose {
    const REST: Pose = Pose { windup: 0.0, strike: 0.0, special: 0.0, coil: 0.0, sweep: 0.0, raise: 0.0, spikes: 0.0 };

    /// The pose drawn in `frame`, see POSE_WINDUP and tween()
    fn of(frame: usize) -> Pose {
        match frame {
            POSE_WINDUP => Pose { windup: 1.0, ..Pose::REST },
            POSE_STRIKE => Pose { strike: 1.0, ..Pose::REST },
            POSE_SPECIAL => Pose { special: 1.0, ..Pose::REST },
            POSE_COIL => Pose { coil: 1.0, ..Pose::REST },
            POSE_SWEEP => Pose { sweep: 1.0, ..Pose::REST },
            POSE_RAISE => Pose { raise: 1.0, ..Pose::REST },
            POSE_SPIKES => Pose { spikes: 1.0, ..Pose::REST },
            _ if frame >= FIRST_TWEEN => {
                // Partway from one pose of a pair to the other
                let (pair, t) = tween_at(frame);
                let (a, b) = (Pose::of(pair.from), Pose::of(pair.to));
                let mix = |x: f64, y: f64| x + (y - x) * t;
                Pose {
                    windup: mix(a.windup, b.windup),
                    strike: mix(a.strike, b.strike),
                    special: mix(a.special, b.special),
                    coil: mix(a.coil, b.coil),
                    sweep: mix(a.sweep, b.sweep),
                    raise: mix(a.raise, b.raise),
                    spikes: mix(a.spikes, b.spikes),
                }
            }
            _ => Pose::REST,
        }
    }

    /// What's `rest` standing, `windup`, `strike` and `special` in each pose, in this one
    fn mix(self, rest: f64, windup: f64, strike: f64, special: f64) -> f64 {
        rest + self.windup * (windup - rest) + self.strike * (strike - rest) + self.special * (special - rest)
    }

    /// Where something that's at `rest` standing and at the others in each pose is in this one
    fn at(self, rest: Vec3, windup: Vec3, strike: Vec3, special: Vec3) -> Vec3 {
        [0, 1, 2].map(|i| self.mix(rest[i], windup[i], strike[i], special[i]))
    }
}

/// How far through its walk frame `frame` is, None for none, and how much of the first three
/// poses it's in, getting ready, striking and its own, and of lying coiled up at rest, a serpent's
/// POSE_COIL: what a serpent's body is shaped by, see serpents::spine()
pub(crate) fn frame_shape(frame: usize) -> (Option<f64>, [f64; 4]) {
    let phase = (1..=WALK_FRAMES).contains(&frame).then(|| (frame - 1) as f64 / WALK_FRAMES as f64);
    let pose = Pose::of(frame);
    (phase, [pose.windup, pose.strike, pose.special, pose.coil])
}

/// Voxels per meter of everything drawn like an animal, weapons and leaves too: the same as the
/// world's, so every voxel is the same size
pub const MODEL_RES: f64 = 16.0;

/// How many times finer than the world's the creatures made finer are, see Look::resolution(): their
/// voxels a meter, as the world's stay MODEL_RES
pub const FINER: f64 = 2.0;

type Vec3 = [f64; 3];

/// A capsule whose radius goes from `ra` at `a` to `rb` at `b`, stretched by `stretch` around its
/// middle line so it can be an ellipsoid too. Or with `boxed`, the box with corners `a` and `b`.
/// Earlier shapes win where they overlap.
#[derive(Clone, Copy)]
pub struct Part {
    a: Vec3,
    b: Vec3,
    ra: f64,
    rb: f64,
    stretch: Vec3,
    boxed: bool,
    material: u8,
    /// Darker bands across the body, every few voxels along x
    stripes: u8,
    /// What it's part of, for telling a weapon's parts apart in its model, see voxelize_tagged()
    tag: u8,
    /// Which bone of a creature baked from jointed pieces it's part of, see src\baked.rs, 0 for
    /// none; and the bone its far half is part of instead, past its middle, as a leg's shin, 0 for
    /// none
    bone: u8,
    split: u8,
}

pub fn ball(c: Vec3, r: f64, material: u8) -> Part {
    Part { a: c, b: c, ra: r, rb: r, stretch: [1.0; 3], boxed: false, material, stripes: AIR, tag: 0, bone: 0, split: 0 }
}

pub fn ellipsoid(c: Vec3, radii: Vec3, material: u8) -> Part {
    let r = radii[0].min(radii[1]).min(radii[2]);
    Part { a: c, b: c, ra: r, rb: r, stretch: radii.map(|v| v / r), boxed: false, material, stripes: AIR, tag: 0, bone: 0, split: 0 }
}

pub fn limb(a: Vec3, b: Vec3, ra: f64, rb: f64, material: u8) -> Part {
    Part { a, b, ra, rb, stretch: [1.0; 3], boxed: false, material, stripes: AIR, tag: 0, bone: 0, split: 0 }
}

pub fn cuboid(lo: Vec3, hi: Vec3, material: u8) -> Part {
    Part { a: lo, b: hi, ra: 0.0, rb: 0.0, stretch: [1.0; 3], boxed: true, material, stripes: AIR, tag: 0, bone: 0, split: 0 }
}

/// The shapes from `from` on that aren't part of a bone yet made part of bone `bone`, see
/// src\baked.rs
fn tag(p: &mut [Part], from: usize, bone: u8) {
    for part in &mut p[from..] {
        if part.bone == 0 {
            part.bone = bone;
        }
    }
}

impl Part {
    /// Tagged `tag`, see voxelize_tagged()
    pub fn tagged(mut self, tag: u8) -> Self {
        self.tag = tag;
        self
    }

    /// Part of bone `bone` of a creature baked from jointed pieces, see src\baked.rs
    pub(crate) fn on(mut self, bone: u8) -> Self {
        self.bone = bone;
        self
    }

    /// Its far half, past its middle, part of bone `far` instead, as a leg's shin is
    pub(crate) fn halved(mut self, far: u8) -> Self {
        self.split = far;
        self
    }

    /// The bone it's part of and the bone its far half is, 0 for none, see on() and halved()
    pub(crate) fn bones(&self) -> (u8, u8) {
        (self.bone, self.split)
    }

    /// Its ends, the same for a ball, and whether it's a box with those corners
    pub(crate) fn ends(&self) -> (Vec3, Vec3, bool) {
        (self.a, self.b, self.boxed)
    }

    /// Its radius at either end, before it's stretched
    pub(crate) fn radii(&self) -> (f64, f64) {
        (self.ra, self.rb)
    }

    /// Its radius, the biggest it is round its middle line, stretched
    pub(crate) fn girth(&self) -> f64 {
        self.ra.max(self.rb) * self.stretch.iter().copied().fold(0.0, f64::max)
    }

    /// How far along from its first end to its second `p` is, 0 to 1, for a capsule
    pub(crate) fn along(&self, p: Vec3) -> f64 {
        let ab = [0, 1, 2].map(|i| self.b[i] - self.a[i]);
        let length2: f64 = ab.iter().map(|v| v * v).sum();
        if length2 <= 0.0 {
            return 0.0;
        }
        ((0..3).map(|i| (p[i] - self.a[i]) * ab[i]).sum::<f64>() / length2).clamp(0.0, 1.0)
    }

    pub(crate) fn striped(mut self, stripes: u8) -> Self {
        self.stripes = stripes;
        self
    }

    /// What it's made of, and the darker bands across it, if any
    pub(crate) fn materials(&self) -> (u8, u8) {
        (self.material, self.stripes)
    }

    /// `s` times as big, and as far from the origin
    pub fn scaled(&self, s: f64) -> Part {
        Part { a: self.a.map(|v| v * s), b: self.b.map(|v| v * s), ra: self.ra * s, rb: self.rb * s, ..*self }
    }

    /// Moved `by`, as it is
    pub fn moved(&self, by: Vec3) -> Part {
        Part { a: [0, 1, 2].map(|i| self.a[i] + by[i]), b: [0, 1, 2].map(|i| self.b[i] + by[i]), ..*self }
    }

    /// Squashed or stretched around its middle, `stretch` times as big along each axis
    pub fn stretched(mut self, stretch: Vec3) -> Self {
        self.stretch = [0, 1, 2].map(|i| self.stretch[i] * stretch[i]);
        self
    }

    /// At least a voxel thick at `res` voxels per meter whichever way it's turned, so nothing
    /// thin falls between the voxels
    pub(crate) fn thickened(&self, res: f64) -> Part {
        let (a, b) = (self.a, self.b);
        if self.boxed {
            let half = 0.51 / res;
            let mut lo = a;
            let mut hi = b;
            for i in 0..3 {
                if hi[i] - lo[i] < 2.0 * half {
                    let mid = (lo[i] + hi[i]) * 0.5;
                    lo[i] = mid - half;
                    hi[i] = mid + half;
                }
            }
            return Part { a: lo, b: hi, ..*self };
        }
        let least = 0.72 / res;
        let thinnest = self.ra.min(self.rb).max(1e-6);
        let (ra, rb) = (self.ra.max(least), self.rb.max(least));
        let k = ra.min(rb);
        let stretch = self.stretch.map(|s| (s * thinnest / k).max(least / k));
        Part { a, b, ra, rb, stretch, ..*self }
    }

    pub(crate) fn contains(&self, p: Vec3) -> bool {
        if self.boxed {
            return (0..3).all(|i| p[i] >= self.a[i] && p[i] <= self.b[i]);
        }
        // Squash the point towards the middle so stretched shapes become plain capsules
        let mid = [0, 1, 2].map(|i| (self.a[i] + self.b[i]) * 0.5);
        let q = [0, 1, 2].map(|i| mid[i] + (p[i] - mid[i]) / self.stretch[i]);
        let ab = [0, 1, 2].map(|i| self.b[i] - self.a[i]);
        let aq = [0, 1, 2].map(|i| q[i] - self.a[i]);
        let length2: f64 = ab.iter().map(|v| v * v).sum();
        let t = if length2 > 0.0 { (aq.iter().zip(&ab).map(|(a, b)| a * b).sum::<f64>() / length2).clamp(0.0, 1.0) } else { 0.0 };
        let r = self.ra + (self.rb - self.ra) * t;
        let d2: f64 = (0..3).map(|i| (aq[i] - ab[i] * t).powi(2)).sum();
        d2 <= r * r
    }

    pub(crate) fn reach(&self) -> (Vec3, Vec3) {
        if self.boxed {
            return (self.a, self.b);
        }
        let r = self.ra.max(self.rb);
        let s = self.stretch;
        let mid = [0, 1, 2].map(|i| (self.a[i] + self.b[i]) * 0.5);
        let lo = [0, 1, 2].map(|i| mid[i] + (self.a[i].min(self.b[i]) - r - mid[i]) * s[i]);
        let hi = [0, 1, 2].map(|i| mid[i] + (self.a[i].max(self.b[i]) + r - mid[i]) * s[i]);
        (lo, hi)
    }
}

/// A leg hanging from `hip`, swung forwards by `swing` radians, ending in a paw: its upper half
/// and its paw parts of `bones`, its thigh and its shin, for a creature baked from jointed pieces
#[allow(clippy::too_many_arguments)]
fn leg(parts: &mut Vec<Part>, hip: Vec3, length: f64, radius: f64, swing: f64, fur: u8, paw: u8, bones: [u8; 2]) {
    let foot = [hip[0] + swing.sin() * length, hip[1], hip[2] - swing.cos() * length + radius * 0.6];
    parts.push(ball(foot, radius * 1.1, paw).on(bones[1]));
    parts.push(limb(hip, foot, radius * 1.15, radius, fur).on(bones[0]).halved(bones[1]));
}

/// `v` turned up by `angle` radians about `pivot`, round the y axis, so what's in front rises
fn rear(v: Vec3, pivot: Vec3, angle: f64) -> Vec3 {
    let (dx, dz) = (v[0] - pivot[0], v[2] - pivot[2]);
    let (c, s) = (angle.cos(), angle.sin());
    [pivot[0] + dx * c - dz * s, v[1], pivot[2] + dx * s + dz * c]
}

/// The Broodmother: a giant spider, dark and hairy, venom glowing in the markings on its back and
/// a cluster of red eyes. It rears up to bite, and crouches to leap.
fn spider(p: &mut Vec<Part>, angle: f64, moving: f64, pose: Pose, skin: Skin) {
    use crate::baked::spider::*;
    let start = p.len();
    // Reared up on its back legs, lunging low, or crouched to spring
    let raise = pose.mix(0.0, 0.5, -0.15, 0.0);
    let drop = pose.mix(0.0, 0.0, 0.25, 0.45);
    let lift = |v: Vec3| {
        let up = raise * (v[0] + 0.6).max(0.0) * 0.8 - drop;
        [v[0], v[1], v[2] + up]
    };
    // Eight eyes, and fangs
    for (y, z, r) in [(0.1, 1.42, 0.06), (-0.1, 1.42, 0.06), (0.2, 1.36, 0.045), (-0.2, 1.36, 0.045), (0.06, 1.52, 0.04), (-0.06, 1.52, 0.04), (0.16, 1.5, 0.03), (-0.16, 1.5, 0.03)] {
        p.push(ball(lift([1.18, y, z]), r, skin.glow));
    }
    for y in [-0.09, 0.09] {
        p.push(limb(lift([1.22, y, 1.2]), lift([1.38, y * 0.6, 0.92]), 0.05, 0.012, skin.extra));
    }
    p.push(ellipsoid(lift([0.85, 0.0, 1.32]), [0.4, 0.38, 0.32], skin.body));
    tag(p, start, BODY);
    // The abdomen, with glowing markings down its back
    for k in 0..3 {
        let x = -0.55 - k as f64 * 0.38;
        p.push(ellipsoid(lift([x, 0.0, 1.9 - k as f64 * 0.05]), [0.14, 0.32, 0.06], skin.accent).on(ABDOMEN));
    }
    p.push(ellipsoid(lift([-0.85, 0.0, 1.45]), [0.9, 0.75, 0.62], skin.body).on(ABDOMEN));
    // Eight legs, in two sets of four lifting by turns, each bent up at the knee
    for side in [-1.0, 1.0] {
        for k in 0..4 {
            let along = 0.95 - k as f64 * 0.45;
            let step = (angle + k as f64 * PI * 0.5 + if side > 0.0 { PI } else { 0.0 }).sin() * moving;
            let hip = lift([along * 0.6 + 0.3, side * 0.3, 1.3]);
            let reach = 1.6 + 0.2 * (1.0 - (k as f64 - 1.5).abs() / 1.5);
            // Its front legs raised as it rears
            let front = if k == 0 { pose.windup } else { 0.0 };
            let foot_x = along * 1.4 + 0.15 * step + 0.6 * front;
            let foot_z = 0.15 * step.max(0.0) + 1.4 * front;
            let foot = [foot_x, side * reach, foot_z];
            let knee = [(hip[0] + foot[0]) * 0.5, side * (reach * 0.65), hip[2] + 0.55 - drop * 0.5];
            let leg = (side > 0.0) as usize * 4 + k as usize;
            p.push(limb(hip, knee, 0.1, 0.08, skin.dark).on(FEMUR[leg]));
            p.push(limb(knee, foot, 0.08, 0.03, skin.dark).on(TIBIA[leg]));
        }
    }
}

/// A wolf, lean and grey, eyes glowing. It crouches to spring, lunges with its jaws open, and
/// throws its head back to howl.
fn wolf(p: &mut Vec<Part>, angle: f64, swing: f64, moving: f64, pose: Pose, skin: Skin, fine: bool) {
    let (fur, dark) = (skin.body, skin.dark);
    let crouch = pose.mix(0.0, 0.14, 0.0, 0.04);
    let lunge = pose.mix(0.0, -0.08, 0.22, 0.0);
    let howl = pose.special;
    let open = pose.strike + pose.special > 0.4;
    let head = [0.74 + lunge - 0.12 * howl, 0.0, 0.78 - crouch + 0.3 * howl];
    // Its muzzle points on ahead, or up at the sky
    let snout = [head[0] + 0.2 - 0.12 * howl, 0.0, head[2] - 0.04 + 0.2 * howl];
    use crate::baked::wolf::*;
    // Made finer, see FINER, what only shows then, before what it's on: fangs showing at the front
    // of its muzzle, a darker saddle of fur down its back and a ruff at its neck
    if fine {
        for s in [-1.0, 1.0] {
            p.push(limb([snout[0] - 0.03, s * 0.028, snout[2] - 0.035], [snout[0] - 0.025, s * 0.026, snout[2] - 0.075], 0.011, 0.004, BONE).on(MUZZLE));
        }
        for k in 0..5 {
            // Just under the top of its back there, so it darkens it without standing out of it
            let u = -0.3 + 0.13 * k as f64;
            let top = 0.6 - crouch + 0.19 * (1.0 - (u / 0.5).powi(2)).max(0.0).sqrt();
            p.push(ellipsoid([u + lunge * 0.3, 0.0, top - 0.02], [0.075, 0.07, 0.03], dark).on(BODY));
        }
        for s in [-1.0, 1.0] {
            p.push(ellipsoid([0.5 + lunge * 0.5, s * 0.1, 0.7 - crouch], [0.06, 0.04, 0.09], skin.accent).on(NECK));
        }
    }
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.09, s * 0.07, head[2] + 0.05], 0.025, skin.glow).on(HEAD));
        p.push(limb([head[0] - 0.03, s * 0.07, head[2] + 0.1], [head[0] - 0.05, s * 0.09, head[2] + 0.24], 0.04, 0.01, dark).on(HEAD));
    }
    p.push(ball(snout, 0.03, EYE).on(MUZZLE));
    if open {
        p.push(limb([head[0] + 0.05, 0.0, head[2] - 0.08], [snout[0], 0.0, snout[2] - 0.12], 0.045, 0.025, dark).on(JAW));
    }
    p.push(limb(head, snout, 0.075, 0.05, fur).on(MUZZLE));
    p.push(ball(head, 0.13, fur).on(HEAD));
    p.push(limb([0.42 + lunge * 0.5, 0.0, 0.62 - crouch], head, 0.12, 0.1, fur).on(NECK));
    p.push(ellipsoid([0.28 + lunge * 0.5, 0.0, 0.55 - crouch], [0.22, 0.17, 0.18], skin.accent).on(CHEST));
    p.push(ellipsoid([0.0 + lunge * 0.3, 0.0, 0.6 - crouch], [0.5, 0.18, 0.19], fur).on(BODY));
    // Its tail, low as it creeps, up as it howls
    let sway = 0.08 * (angle + 1.0).sin() * moving;
    p.push(limb([-0.45, 0.0, 0.62 - crouch], [-0.82, sway, 0.45 - crouch + 0.35 * howl], 0.06, 0.035, dark).on(TAIL));
    for (x, y, s) in [(0.3, 0.12, 1.0), (0.3, -0.12, -1.0), (-0.32, 0.12, -1.0), (-0.32, -0.12, 1.0)] {
        // Its hind legs stretched out behind as it lunges
        let reach = if x < 0.0 { -0.4 * pose.strike } else { 0.5 * pose.strike };
        let k = crate::baked::leg_of(x, y);
        leg(p, [x + lunge * 0.4, y, 0.52 - crouch], 0.52 - crouch, 0.055, 0.5 * swing * s + reach, fur, dark, [UPPER[k], LOWER[k]]);
    }
}

/// The walking dead, about 1.8 meters tall: bones or rotting flesh in rags, maybe in armor, eyes
/// burning, a blade in the right hand. It raises the blade overhead and brings it down; zombies
/// shamble with their arms out. `living`, it's a bandit: standing straight, a face under a hood or
/// a helm, a scarf over its mouth, a shirt or armor, and a cloak if it's armored. `arms` is what it
/// holds: a blade, a crossbow it raises to its shoulder to aim and points at the ground to span
/// again, or a round shield it holds up in front and a mace it brings down over the top of it.
#[allow(clippy::too_many_arguments)]
fn undead(p: &mut Vec<Part>, swing: f64, pose: Pose, skin: Skin, bony: bool, armored: bool, living: bool, arms: Arms) {
    use crate::baked::undead::*;
    let (flesh, dark, glow) = (skin.body, skin.dark, skin.glow);
    let start = p.len();
    // Leaning into a blow, and hunched as zombies are
    let hunch = if bony || living { 0.0 } else { 0.08 };
    let lean = if arms == Arms::Crossbow { pose.mix(0.0, 0.03, -0.02, 0.1) } else { pose.mix(hunch, hunch - 0.05, hunch + 0.22, hunch + 0.05) };
    let up = |v: Vec3| [v[0] + lean * (v[2] - 0.9).max(0.0), v[1], v[2]];
    let limb_r = if bony { 0.045 } else if arms == Arms::Slab { 0.09 } else { 0.07 };
    // Its head: a skull with burning sockets, or a rotting face, or a living one
    let head = up([0.02, 0.0, 1.62]);
    if living {
        for s in [-1.0, 1.0] {
            p.push(ball([head[0] + 0.115, s * 0.045, head[2] + 0.03], 0.02, glow));
        }
        p.push(ellipsoid([head[0] + 0.07, 0.0, head[2] - 0.06], [0.07, 0.1, 0.05], skin.accent));
        p.push(ball([head[0] + 0.02, 0.0, head[2]], 0.12, flesh));
        if arms == Arms::Crossbow {
            // A broad-brimmed hat pulled low, a band round it
            p.push(ellipsoid([head[0] - 0.01, 0.0, head[2] + 0.115], [0.12, 0.12, 0.03], BRONZE));
            p.push(ellipsoid([head[0] - 0.01, 0.0, head[2] + 0.15], [0.11, 0.11, 0.07], dark));
            p.push(ellipsoid([head[0] + 0.0, 0.0, head[2] + 0.09], [0.24, 0.23, 0.022], dark));
            p.push(ellipsoid([head[0] - 0.05, 0.0, head[2] + 0.0], [0.11, 0.135, 0.11], dark));
        } else if armored {
            // A helm, open at the face, with a crest
            p.push(ellipsoid([head[0] - 0.04, 0.0, head[2] + 0.05], [0.15, 0.15, 0.15], skin.accent));
            p.push(limb([head[0] - 0.12, 0.0, head[2] + 0.12], [head[0] + 0.05, 0.0, head[2] + 0.2], 0.03, 0.02, dark));
        } else {
            // A hood up over its head, open in front
            p.push(ellipsoid([head[0] - 0.04, 0.0, head[2] + 0.03], [0.14, 0.15, 0.16], dark));
        }
    } else {
        for s in [-1.0, 1.0] {
            p.push(ball([head[0] + 0.1, s * 0.045, head[2] + 0.02], 0.028, glow));
        }
        if armored {
            // A helm with a slit
            p.push(ellipsoid([head[0] - 0.01, 0.0, head[2] + 0.04], [0.15, 0.15, 0.15], skin.accent));
        }
        p.push(ellipsoid([head[0] + 0.05, 0.0, head[2] - 0.09], [0.08, 0.08, 0.05], if bony { flesh } else { dark }));
        p.push(ball(head, 0.13, flesh));
    }
    p.push(limb(up([0.0, 0.0, 1.3]), head, 0.05, 0.05, flesh));
    tag(p, start, HEAD);
    let start = p.len();
    // Its chest: ribs, or rags over flesh, a shirt, maybe a breastplate
    let chest = up([0.0, 0.0, 1.25]);
    if armored {
        p.push(ellipsoid(chest, [0.15, 0.22, 0.22], skin.accent));
        for s in [-1.0, 1.0] {
            p.push(ellipsoid(up([0.0, s * 0.24, 1.44]), [0.12, 0.12, 0.08], skin.accent));
        }
        if living {
            // A cloak down its back
            p.push(ellipsoid(up([-0.17, 0.0, 1.0]), [0.05, 0.24, 0.45], dark));
        }
    }
    if bony {
        p.push(ellipsoid(chest, [0.12, 0.17, 0.2], flesh).striped(dark));
        p.push(limb(up([0.0, 0.0, 0.92]), chest, 0.04, 0.04, flesh));
    } else {
        p.push(ellipsoid(up([0.0, 0.0, 1.12]), [0.15, 0.2, 0.28], skin.accent));
        p.push(ellipsoid(chest, [0.13, 0.2, 0.22], if living { skin.accent } else { flesh }));
    }
    tag(p, start, TORSO);
    p.push(ellipsoid([0.0, 0.0, 0.92], [0.1, 0.15, 0.07], if armored { skin.accent } else { dark }).on(PELVIS));
    // Sleeves on the living, armor on the armored
    let upper = if armored || living { skin.accent } else { flesh };
    // An arm from `shoulder` to `hand`, its elbow out to the side and down, on the right or the
    // left, 0 or 1
    let arm = |p: &mut Vec<Part>, shoulder: Vec3, hand: Vec3, out: f64, side: usize| {
        let elbow = [(shoulder[0] + hand[0]) * 0.5 - 0.06, (shoulder[1] + hand[1]) * 0.5 + out, (shoulder[2] + hand[2]) * 0.5 - 0.12];
        p.push(ball(hand, limb_r * 1.1, flesh).on(FORE[side]));
        p.push(limb(elbow, hand, limb_r * 0.9, limb_r * 0.8, flesh).on(FORE[side]));
        p.push(limb(shoulder, elbow, limb_r, limb_r * 0.9, upper).on(ARM[side]));
    };
    if arms == Arms::Bow {
        bow(p, up, &arm, pose, skin);
    } else if arms == Arms::Slab {
        // A brute's shoulders, hunched and heavy, and rags hanging off them
        for s in [-1.0, 1.0] {
            p.push(ball(up([-0.02, s * 0.25, 1.4]), 0.13, flesh).on(TORSO));
        }
        p.push(ellipsoid(up([-0.05, 0.0, 1.32]), [0.17, 0.27, 0.2], dark).on(TORSO));
        let start = p.len();
        // The gravestone: carried low in front, raised high behind its head, brought down in
        // front, and lifted as it roars, both hands on its broken end
        let hand = pose.at([0.32, 0.0, 0.92], [-0.12, 0.0, 1.95], [0.62, 0.0, 1.05], [0.15, 0.0, 2.05]);
        let dir = pose.at([0.45, 0.0, -0.75], [-0.42, 0.0, 0.8], [0.92, 0.0, -0.35], [0.1, 0.0, 1.0]);
        let n = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt().max(0.1);
        let along = |k: f64| [hand[0] + dir[0] / n * k, hand[1] + dir[1] / n * k, hand[2] + dir[2] / n * k];
        // Broad and flat across, so it's seen broad from in front whichever way it's swung: rows
        // side by side, its top rounded, a chip out of one corner, moss on it
        let row = |k: f64, y: f64| [along(k)[0], along(k)[1] + y, along(k)[2]];
        p.push(ball(row(0.78, 0.15), 0.08, AIR));
        p.push(ball(row(0.5, -0.05), 0.07, MOSS));
        p.push(ball(row(0.62, 0.0), 0.15, skin.extra));
        for y in [-0.15, 0.0, 0.15] {
            p.push(limb(row(0.05, y), row(0.7 - f64::abs(y) * 0.6, y), 0.13, 0.13, skin.extra));
        }
        tag(p, start, HELD);
        for s in [-1.0, 1.0] {
            let grip = [hand[0], hand[1] + s * 0.07, hand[2]];
            arm(p, up([0.0, s * 0.24, 1.42]), grip, s * 0.12, (s > 0.0) as usize);
        }
    } else if arms == Arms::Crossbow {
        let start = p.len();
        // Held low across it, raised to the shoulder to aim, kicking up as it looses, and pointed
        // at the ground as it's spanned again
        let stock = up(pose.at([0.1, -0.12, 1.0], [0.04, -0.13, 1.42], [0.04, -0.13, 1.44], [0.16, -0.08, 1.02]));
        let tip = up(pose.at([0.6, -0.02, 1.12], [0.76, -0.04, 1.44], [0.72, -0.04, 1.6], [0.38, -0.06, 0.48]));
        let along = Vector3::from(tip) - Vector3::from(stock);
        let dir = along.normalize();
        let side = Vector3::new(-dir.y, dir.x, 0.0).try_normalize(1e-6).unwrap_or(Vector3::y());
        let at = |f: f64, s: f64, u: f64| -> Vec3 { (Vector3::from(stock) + dir * f * along.norm() + side * s + Vector3::z() * u).into() };
        // The prod across its end, the string drawn back to the latch, and a bolt on it unless
        // it's just been loosed
        let ends = [at(0.93, 0.27, 0.0), at(0.93, -0.27, 0.0)];
        let latch = if pose.strike > 0.5 { at(0.9, 0.0, 0.01) } else { at(0.5, 0.0, 0.02) };
        for end in ends {
            p.push(limb(at(1.0, 0.0, 0.0), end, 0.026, 0.016, DARK_STEEL));
            p.push(limb(end, latch, 0.008, 0.008, BOWSTRING));
        }
        if pose.strike < 0.5 {
            p.push(limb(at(0.5, 0.0, 0.035), at(1.06, 0.0, 0.035), 0.014, 0.012, STEEL).on(BOLT));
        }
        p.push(limb(stock, tip, 0.04, 0.03, skin.extra));
        tag(p, start, HELD);
        // A quiver of bolts on its back
        for k in 0..3 {
            p.push(ball(up([-0.2, 0.1 + 0.035 * k as f64, 1.48]), 0.025, LINEN).on(TORSO));
        }
        p.push(limb(up([-0.17, 0.1, 0.92]), up([-0.2, 0.13, 1.44]), 0.05, 0.055, LEATHER).on(TORSO));
        // One hand at the trigger, the other holding it up under its middle
        arm(p, up([0.0, -0.22, 1.42]), at(0.12, -0.02, -0.03), -0.06, 0);
        arm(p, up([0.0, 0.22, 1.42]), at(0.5, 0.0, -0.05), 0.08, 1);
    } else {
        if arms == Arms::Shield {
            // A round shield on its left arm, held up in front, drawn back to brace, shoved out as
            // it strikes over the top of it, and raised high as it calls out: iron at its rim, a
            // red band across it and a boss in its middle
            let c = up(pose.at([0.3, 0.17, 1.1], [0.22, 0.22, 1.14], [0.48, 0.1, 1.08], [0.3, 0.12, 1.32]));
            p.push(ball([c[0] + 0.04, c[1], c[2]], 0.065, STEEL).on(SHIELD));
            p.push(ellipsoid([c[0] + 0.012, c[1], c[2]], [0.03, 0.065, 0.31], TEMPLE_RED).on(SHIELD));
            p.push(ellipsoid(c, [0.035, 0.3, 0.36], WOOD).on(SHIELD));
            p.push(ellipsoid([c[0] - 0.014, c[1], c[2]], [0.035, 0.33, 0.39], DARK_STEEL).on(SHIELD));
            arm(p, up([0.0, 0.22, 1.42]), [c[0] - 0.08, c[1] - 0.04, c[2] - 0.02], 0.1, 1);
        }
        let start = p.len();
        // The blade arm: hanging, raised overhead behind, or brought down in front
        let shoulder = up([0.0, -0.22, 1.42]);
        let zombie_reach = if bony || living { 0.0 } else { 0.35 };
        let elbow = pose.at([0.05 + zombie_reach - 0.15 * swing, -0.26, 1.15 + zombie_reach * 0.5], [-0.08, -0.3, 1.65], [0.38, -0.2, 1.3], [0.1, -0.32, 1.55]);
        let hand = pose.at([0.2 + zombie_reach * 1.4 - 0.25 * swing, -0.25, 0.95 + zombie_reach], [-0.18, -0.24, 2.0], [0.62, -0.1, 1.0], [0.3, -0.28, 1.9]);
        let blade = pose.at([0.55, 0.0, -0.75], [-0.5, 0.05, 0.85], [0.92, 0.1, -0.35], [0.3, 0.0, 0.95]);
        let n = (blade[0] * blade[0] + blade[1] * blade[1] + blade[2] * blade[2]).sqrt().max(0.1);
        if arms == Arms::Shield {
            // A flanged mace, short in the haft
            let head = [0, 1, 2].map(|i| hand[i] + blade[i] / n * 0.5);
            for k in 0..4 {
                let a = k as f64 * PI * 0.5;
                p.push(limb(head, [head[0] + 0.07 * a.cos(), head[1] + 0.07 * a.sin(), head[2] + 0.03], 0.03, 0.02, skin.extra));
            }
            p.push(ball(head, 0.07, skin.extra));
            p.push(limb(hand, head, 0.025, 0.025, WOOD));
        } else {
            let tip = [0, 1, 2].map(|i| hand[i] + blade[i] / n * 0.85);
            p.push(limb(hand, tip, 0.03, 0.012, skin.extra));
            p.push(limb([hand[0] - blade[1] / n * 0.1, hand[1] - 0.09, hand[2]], [hand[0] + blade[1] / n * 0.1, hand[1] + 0.09, hand[2]], 0.025, 0.025, DARK_STEEL));
        }
        tag(p, start, HELD);
        p.push(ball(hand, limb_r * 1.1, flesh).on(FORE[0]));
        p.push(limb(elbow, hand, limb_r * 0.9, limb_r * 0.8, flesh).on(FORE[0]));
        p.push(limb(shoulder, elbow, limb_r, limb_r * 0.9, upper).on(ARM[0]));
        // The other arm swinging, reaching out for a zombie, thrown out for balance as it strikes
        if arms == Arms::Blade {
            let shoulder = up([0.0, 0.22, 1.42]);
            let elbow = pose.at([0.05 + zombie_reach + 0.15 * swing, 0.26, 1.15 + zombie_reach * 0.5], [0.2, 0.3, 1.3], [-0.12, 0.3, 1.2], [0.1, 0.32, 1.55]);
            let hand = pose.at([0.15 + zombie_reach * 1.4 + 0.25 * swing, 0.27, 0.95 + zombie_reach], [0.42, 0.26, 1.35], [-0.25, 0.32, 1.0], [0.3, 0.28, 1.9]);
            p.push(ball(hand, limb_r * 1.1, flesh).on(FORE[1]));
            p.push(limb(elbow, hand, limb_r * 0.9, limb_r * 0.8, flesh).on(FORE[1]));
            p.push(limb(shoulder, elbow, limb_r, limb_r * 0.9, upper).on(ARM[1]));
        }
    }
    // Its legs, in rags or greaves
    for (y, s) in [(0.1, 1.0), (-0.1, -1.0)] {
        let side = (y > 0.0) as usize;
        leg(p, [0.0, y, 0.92], 0.88, limb_r * 1.1, 0.45 * swing * s, if armored { skin.accent } else if bony { flesh } else { dark }, dark, [THIGH[side], SHIN[side]]);
    }
}

/// A skeleton archer's bow and the arms drawing it, and the hood and quiver it wears: the bow held
/// low at its side, raised and drawn back to the cheek as it takes aim, the string snapping
/// straight as it looses, and a hand reaching back over its shoulder for the next arrow
fn bow(p: &mut Vec<Part>, up: impl Fn(Vec3) -> Vec3, arm: &impl Fn(&mut Vec<Part>, Vec3, Vec3, f64, usize), pose: Pose, skin: Skin) {
    use crate::baked::undead::*;
    let wood = skin.extra;
    // A hood over its skull, open in front
    p.push(ellipsoid(up([-0.04, 0.0, 1.66]), [0.15, 0.165, 0.16], skin.accent).on(HEAD));
    p.push(ellipsoid(up([-0.08, 0.0, 1.42]), [0.1, 0.22, 0.08], skin.accent).on(TORSO));
    let grip = up(pose.at([0.14, 0.27, 0.98], [0.62, 0.1, 1.43], [0.64, 0.08, 1.45], [0.3, 0.22, 1.12]));
    let draw = up(pose.at([0.12, -0.25, 0.95], [0.06, -0.03, 1.52], [0.3, -0.1, 1.44], [-0.16, -0.12, 1.66]));
    // How far it's bent, drawn
    let bent = pose.windup;
    let tip = |s: f64| [grip[0] - 0.1 - 0.12 * bent, grip[1], grip[2] + s * (0.62 - 0.05 * bent)];
    let mid = |s: f64| [grip[0] + 0.0 - 0.02 * bent, grip[1], grip[2] + s * 0.33];
    for s in [-1.0, 1.0] {
        p.push(limb(mid(s), tip(s), 0.028, 0.016, wood).on(HELD));
        p.push(limb(grip, mid(s), 0.032, 0.028, wood).on(HELD));
    }
    p.push(ball(grip, 0.04, LEATHER).on(HELD));
    // The string, drawn back to its hand as it aims, and the arrow on it
    if bent > 0.3 {
        let start = p.len();
        for s in [-1.0, 1.0] {
            p.push(limb(tip(s), draw, 0.008, 0.008, BOWSTRING));
        }
        let head = [grip[0] + 0.2, grip[1] - 0.01, grip[2] + 0.02];
        p.push(limb(head, [head[0] + 0.07, head[1], head[2]], 0.02, 0.006, STEEL));
        p.push(limb(draw, head, 0.012, 0.012, WOOD));
        p.push(limb(draw, [draw[0] + 0.1, draw[1], draw[2]], 0.025, 0.02, LINEN));
        tag(p, start, DRAWN);
    } else {
        p.push(limb(tip(-1.0), tip(1.0), 0.008, 0.008, BOWSTRING).on(STRING));
    }
    // A quiver of arrows on its back
    for k in 0..3 {
        p.push(ball(up([-0.2, -0.08 + 0.04 * k as f64, 1.55]), 0.025, LINEN).on(TORSO));
    }
    p.push(limb(up([-0.17, -0.08, 0.95]), up([-0.2, -0.04, 1.5]), 0.05, 0.055, LEATHER).on(TORSO));
    arm(p, up([0.0, 0.22, 1.42]), grip, 0.1, 1);
    arm(p, up([0.0, -0.22, 1.42]), draw, -0.1, 0);
}

/// A bloater: a corpse swollen up round as a barrel, too heavy for its stubby legs, its skin
/// blotched and split, venom glowing through the splits and in its eyes, the rags it died in
/// stretched over it. It waddles, rocking side to side, and swells up fit to burst as it comes
/// close, its arms flung back, then bursts in a cloud of venom.
fn bloater(p: &mut Vec<Part>, swing: f64, pose: Pose, skin: Skin) {
    use crate::baked::bloater::*;
    let (flesh, dark, glow) = (skin.body, skin.dark, skin.glow);
    // Swelling as it gets ready to burst
    let swell = 1.0 + 0.22 * pose.windup.max(pose.strike).max(pose.special);
    let rock = 0.05 * swing;
    let at = |v: Vec3| [v[0], v[1] + rock * (v[2] - 0.2).max(0.0), v[2]];
    // A small head sunk into its shoulders, eyes glowing, its jaw hanging open
    let head = at([0.26, 0.0, 1.55]);
    let start = p.len();
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.1, head[1] + s * 0.05, head[2] + 0.02], 0.03, glow));
    }
    p.push(ellipsoid([head[0] + 0.08, head[1], head[2] - 0.09], [0.06, 0.07, 0.05], dark));
    p.push(ball(head, 0.13, flesh));
    tag(p, start, HEAD);
    let start = p.len();
    // Glowing boils and splits all over its belly, and blotches
    let belly = at([0.05, 0.0, 0.95]);
    let r = [0.42 * swell, 0.48 * swell, 0.5 * swell];
    for (k, (a, z)) in [(0.3, 0.2), (-0.6, -0.1), (1.2, 0.35), (-1.4, 0.05), (2.2, -0.2), (0.8, -0.3), (-2.5, 0.3), (0.0, -0.05)].iter().enumerate() {
        let (c, s) = ((*a as f64).cos(), (*a as f64).sin());
        let q = [belly[0] + c * r[0] * 0.92, belly[1] + s * r[1] * 0.92, belly[2] + z * r[2]];
        let size = if k % 3 == 0 { 0.09 } else { 0.06 } * swell;
        p.push(ball(q, size, if k % 2 == 0 { glow } else { skin.extra }));
    }
    // Rags round its middle
    p.push(ellipsoid([belly[0], belly[1], belly[2] - 0.22 * swell], [r[0] * 1.02, r[1] * 1.02, 0.12], skin.accent));
    p.push(ellipsoid(belly, r, flesh).striped(dark));
    tag(p, start, BELLY);
    p.push(ellipsoid(at([0.0, 0.0, 1.32]), [0.24, 0.32, 0.18], flesh).on(CHEST));
    // Thin arms dangling, flung back as it swells
    for s in [-1.0, 1.0] {
        let side = (s > 0.0) as usize;
        let shoulder = at([0.02, s * 0.33, 1.32]);
        let hand = pose.at([0.22 + 0.1 * swing * s, s * 0.52, 0.85], [-0.3, s * 0.62, 1.15], [-0.35, s * 0.66, 1.2], [-0.3, s * 0.62, 1.15]);
        let elbow = [(shoulder[0] + hand[0]) * 0.5, (shoulder[1] + hand[1]) * 0.5 + s * 0.06, (shoulder[2] + hand[2]) * 0.5 - 0.08];
        p.push(ball(hand, 0.06, flesh).on(FORE[side]));
        p.push(limb(elbow, hand, 0.05, 0.045, flesh).on(FORE[side]));
        p.push(limb(shoulder, elbow, 0.06, 0.05, flesh).on(ARM[side]));
    }
    // Short legs, splayed
    for (y, s) in [(0.2, 1.0), (-0.2, -1.0)] {
        let side = (y > 0.0) as usize;
        leg(p, [0.0, y, 0.5], 0.48, 0.1, 0.35 * swing * s, dark, dark, [THIGH[side], SHIN[side]]);
    }
}

/// A ghoul: the dead gone lean and quick, grey as ash and hairless, crouched low on bent legs
/// with its back hunched, long arms ending in claws of bone, red light in its eyes. It runs at the
/// player and leaps on them from a few steps off: crouching lower to spring, lunging with both
/// claws, and flinging its arms up as it leaps.
fn ghoul(p: &mut Vec<Part>, angle: f64, swing: f64, moving: f64, pose: Pose, skin: Skin) {
    use crate::baked::ghoul::*;
    let (flesh, dark, glow, claw) = (skin.body, skin.dark, skin.glow, skin.extra);
    // Crouched lower to spring, or reaching forward in the lunge
    let crouch = pose.mix(0.0, 0.18, 0.04, 0.12);
    let lunge = pose.mix(0.0, -0.1, 0.3, 0.1);
    // Bobbing as it runs
    let bob = 0.04 * (angle * 2.0).sin().abs() * moving;
    let hips = [lunge * 0.4, 0.0, 0.82 - crouch + bob];
    let chest = [hips[0] + 0.32 + lunge, 0.0, hips[2] + 0.42 - crouch * 0.5];
    let head = [chest[0] + 0.2, 0.0, chest[2] + 0.12];
    let start = p.len();
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.1, s * 0.05, head[2] + 0.02], 0.026, glow));
    }
    // Its jaw, wide, and teeth
    p.push(ellipsoid([head[0] + 0.1, 0.0, head[2] - 0.08], [0.07, 0.07, 0.04], dark));
    p.push(ellipsoid([head[0] + 0.12, 0.0, head[2] - 0.05], [0.05, 0.06, 0.012], claw));
    p.push(ellipsoid(head, [0.13, 0.11, 0.12], flesh));
    p.push(limb(chest, head, 0.06, 0.05, flesh));
    tag(p, start, HEAD);
    let start = p.len();
    // A hunched back, the spine standing out along it, and ribs
    for k in 0..4 {
        let f = k as f64 / 3.0;
        let q = [hips[0] + (chest[0] - hips[0]) * f - 0.05, 0.0, hips[2] + (chest[2] - hips[2]) * f + 0.13];
        p.push(ball(q, 0.035, claw));
    }
    p.push(ellipsoid(chest, [0.15, 0.2, 0.15], flesh).striped(dark));
    p.push(limb(hips, chest, 0.12, 0.15, flesh));
    tag(p, start, TORSO);
    // Rags round its hips
    p.push(ellipsoid([hips[0], 0.0, hips[2] - 0.05], [0.14, 0.18, 0.1], skin.accent).on(PELVIS));
    // Long arms, swinging low as it runs, drawn back to spring, thrust out together, flung up
    for s in [-1.0, 1.0] {
        let side = (s > 0.0) as usize;
        let start = p.len();
        let shoulder = [chest[0] + 0.02, s * 0.2, chest[2] + 0.05];
        let hand = pose.at(
            [chest[0] + 0.25 - 0.3 * swing * s, s * 0.28, 0.25 + 0.1 * (swing * s).max(0.0)],
            [chest[0] - 0.35, s * 0.36, chest[2] - 0.15],
            [chest[0] + 0.7, s * 0.16, chest[2] - 0.05],
            [chest[0] + 0.35, s * 0.34, chest[2] + 0.55],
        );
        let elbow = [(shoulder[0] + hand[0]) * 0.5 - 0.05, (shoulder[1] + hand[1]) * 0.5 + s * 0.08, (shoulder[2] + hand[2]) * 0.5 + 0.06];
        let ahead = [hand[0] - elbow[0], hand[1] - elbow[1], hand[2] - elbow[2]];
        let n = (ahead[0] * ahead[0] + ahead[1] * ahead[1] + ahead[2] * ahead[2]).sqrt().max(0.1);
        for k in [-1.0, 0.0, 1.0] {
            let tip = [hand[0] + ahead[0] / n * 0.18, hand[1] + ahead[1] / n * 0.18 + k * 0.04, hand[2] + ahead[2] / n * 0.18 - 0.03];
            p.push(limb(hand, tip, 0.016, 0.006, claw));
        }
        p.push(ball(hand, 0.045, flesh));
        p.push(limb(elbow, hand, 0.04, 0.035, flesh));
        tag(p, start, FORE[side]);
        p.push(limb(shoulder, elbow, 0.05, 0.04, flesh).on(ARM[side]));
    }
    // Legs bent at the knee, its feet planted wide, tucked up as it leaps
    let tuck = pose.special;
    for (y, s) in [(0.13, 1.0), (-0.13, -1.0)] {
        let side = (y > 0.0) as usize;
        let step = 0.3 * swing * s;
        let hip = [hips[0], y, hips[2]];
        let foot = [hips[0] + step - 0.05, y * 1.4, 0.06 + 0.1 * (swing * s).max(0.0) + tuck * 0.35];
        let knee = [hips[0] + 0.25 + step * 0.5, y * 1.3, (hip[2] + foot[2]) * 0.5 + 0.02];
        p.push(ellipsoid([foot[0] + 0.07, foot[1], foot[2] - 0.02], [0.11, 0.05, 0.04], dark).on(SHIN[side]));
        p.push(limb(knee, foot, 0.05, 0.04, flesh).on(SHIN[side]));
        p.push(limb(hip, knee, 0.07, 0.055, flesh).on(THIGH[side]));
    }
}

/// A dune burrower, about two meters from its pincers to the root of its tail: a low, broad crawler
/// of sand-colored plates banded darker, a shovel-lipped shield over its head for digging, a row
/// of spines down its back, two heavy pincers, six legs, and a jointed tail arched up over its
/// back with a stinger glowing at the tip. It rears its pincers up wide to strike, snapping them
/// shut as its tail whips over its head to sting, and noses down with its tail laid out behind to
/// dig itself in.
fn burrower(p: &mut Vec<Part>, angle: f64, moving: f64, pose: Pose, skin: Skin) {
    use crate::baked::burrower::*;
    let (shell, dark, band, glow, black) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    // Nosing down into the sand, or rearing a little as it strikes
    let dip = pose.mix(0.0, -0.12, 0.04, 0.32);
    let tilt = |v: Vec3| [v[0], v[1], v[2] - dip * (v[0] + 0.3).max(0.0) * 0.5];
    let start = p.len();
    // Little black eyes on its head shield, and its mandibles under the lip
    for (x, y) in [(0.66, 0.07), (0.66, -0.07), (0.6, 0.15), (0.6, -0.15)] {
        p.push(ball(tilt([x, y, 0.43]), 0.026, black));
    }
    for s in [-1.0, 1.0] {
        p.push(limb(tilt([0.76, s * 0.08, 0.22]), tilt([0.87, s * 0.03, 0.17]), 0.03, 0.012, black));
    }
    p.push(ellipsoid(tilt([0.77, 0.0, 0.25]), [0.11, 0.25, 0.04], band));
    p.push(ellipsoid(tilt([0.55, 0.0, 0.33]), [0.25, 0.31, 0.1], dark));
    tag(p, start, HEAD);
    // Its pincers: raised and opened wide, snapped shut out in front, or tucked in to dig
    let open = pose.windup * 0.9 + pose.special * 0.2;
    for s in [-1.0, 1.0] {
        let side = (s > 0.0) as usize;
        let shoulder = tilt([0.58, s * 0.22, 0.3]);
        let elbow = pose.at([0.84, s * 0.44, 0.33], [0.8, s * 0.52, 0.62], [1.04, s * 0.32, 0.36], [0.82, s * 0.36, 0.2]);
        let hand = pose.at([1.08, s * 0.32, 0.3], [1.0, s * 0.44, 0.76], [1.34, s * 0.15, 0.33], [1.0, s * 0.26, 0.14]);
        let forward = (Vector3::from(hand) - Vector3::from(elbow)).normalize();
        let out = Vector3::new(-forward.y, forward.x, 0.0) * s;
        let at = |f: f64, o: f64, u: f64| -> Vec3 { (Vector3::from(hand) + forward * f + out * o + Vector3::z() * u).into() };
        // The fixed finger on the inside and the one that closes on it, tipped black
        p.push(limb(at(0.18, -0.03, 0.0), at(0.27, -0.06, -0.01), 0.025, 0.01, black).on(CLAW[side]));
        p.push(limb(at(0.17, 0.06 + 0.1 * open, 0.02), at(0.25, 0.02 + 0.1 * open, 0.0), 0.025, 0.01, black).on(PINCER[side]));
        p.push(limb(hand, at(0.18, -0.03, 0.0), 0.06, 0.03, shell).on(CLAW[side]));
        p.push(limb(at(0.0, 0.03, 0.01), at(0.17, 0.06 + 0.1 * open, 0.02), 0.045, 0.028, shell).on(PINCER[side]));
        p.push(ellipsoid(hand, [0.11, 0.08, 0.07], shell).on(CLAW[side]));
        p.push(limb(elbow, hand, 0.055, 0.06, shell).on(CLAW[side]));
        p.push(limb(shoulder, elbow, 0.06, 0.055, dark).on(ARM[side]));
    }
    let start = p.len();
    // Its back, in four plates narrowing to the tail, a spine rising from each
    for k in 0..4 {
        let x = 0.3 - k as f64 * 0.24;
        p.push(limb(tilt([x, 0.0, 0.47 - k as f64 * 0.01]), tilt([x - 0.07, 0.0, 0.6 - k as f64 * 0.02]), 0.035, 0.008, dark));
        p.push(ellipsoid(tilt([x, 0.0, 0.35 - k as f64 * 0.01]), [0.15, 0.3 - k as f64 * 0.035, 0.14], shell).striped(band));
    }
    tag(p, start, BODY);
    // Six legs, in two sets of three stepping by turns, bent up at the knee
    for side in [-1.0, 1.0] {
        for (k, x) in [0.3, 0.05, -0.2].into_iter().enumerate() {
            let leg = (side > 0.0) as usize * 3 + k;
            let step = (angle + k as f64 * PI * 2.0 / 3.0 + if side > 0.0 { PI } else { 0.0 }).sin() * moving;
            let hip = tilt([x, side * 0.22, 0.3]);
            let foot = [x + 0.12 * step + 0.06 * (1.0 - k as f64), side * 0.6, 0.12 * step.max(0.0)];
            let knee = [(hip[0] + foot[0]) * 0.5, side * 0.46, 0.46];
            p.push(limb(knee, foot, 0.035, 0.018, dark).on(SHIN[leg]));
            p.push(limb(hip, knee, 0.045, 0.035, shell).on(THIGH[leg]));
        }
    }
    // The tail: arched over its back, drawn back higher to sting, whipped forward over its head,
    // or laid out flat behind it as it digs, swaying a little as it goes
    let sway = 0.06 * (angle + 0.6).sin() * moving;
    let joints: [Vec3; 7] = [
        pose.at([-0.62, 0.0, 0.45], [-0.62, 0.0, 0.47], [-0.6, 0.0, 0.55], [-0.66, 0.0, 0.38]),
        pose.at([-0.8, 0.0, 0.62], [-0.86, 0.0, 0.7], [-0.62, 0.0, 0.86], [-0.88, 0.0, 0.42]),
        pose.at([-0.9, 0.0, 0.88], [-0.98, 0.0, 0.98], [-0.48, 0.0, 1.12], [-1.1, 0.0, 0.44]),
        pose.at([-0.86, 0.0, 1.12], [-0.92, 0.0, 1.26], [-0.2, 0.0, 1.2], [-1.3, 0.0, 0.45]),
        pose.at([-0.68, 0.0, 1.27], [-0.72, 0.0, 1.42], [0.1, 0.0, 1.1], [-1.46, 0.0, 0.43]),
        pose.at([-0.48, 0.0, 1.26], [-0.5, 0.0, 1.4], [0.34, 0.0, 0.9], [-1.58, 0.0, 0.4]),
        pose.at([-0.34, 0.0, 1.12], [-0.36, 0.0, 1.26], [0.46, 0.0, 0.7], [-1.66, 0.0, 0.33]),
    ];
    let joint = |k: usize| -> Vec3 { [joints[k][0], joints[k][1] + sway * k as f64 / 3.0, joints[k][2]] };
    p.push(limb(joint(5), joint(6), 0.04, 0.008, black).on(STINGER));
    p.push(ball(joint(5), 0.085, glow).on(STINGER));
    for k in 0..5 {
        let r = 0.095 - k as f64 * 0.01;
        p.push(limb(joint(k), joint(k + 1), r, r - 0.008, if k % 2 == 0 { shell } else { band }).on(TAIL[k]));
    }
}

/// A rime wraith, a little over two meters tall: a hooded shroud of dark rags drifting over the
/// ground with nothing under it, a pale mask in the shadow of its hood with frost burning in its
/// eyes, a crown of ice shards, a mantle of deep teal and a cold sigil glowing on its breast, and
/// long thin arms ending in icicle claws. The rags hanging under it stream out behind as it
/// drifts. It spreads its arms wide to gather the cold, thrusts its claws out together to strike,
/// and draws in on itself to vanish.
fn wraith(p: &mut Vec<Part>, angle: f64, moving: f64, pose: Pose, skin: Skin) {
    use crate::baked::wraith::*;
    let (shroud, shadow, mantle, glow, ice) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    // Rising and falling as it drifts, and hunched forward as it strikes
    let bob = 0.04 * angle.sin();
    let hunch = pose.mix(0.0, -0.05, 0.12, 0.06);
    let at = |v: Vec3| [v[0] + hunch * (v[2] - 1.0).max(0.0), v[1], v[2] + bob];
    let head = at([0.04, 0.0, 1.93]);
    let start = p.len();
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.13, s * 0.05, head[2] + 0.015], 0.024, glow));
    }
    p.push(ellipsoid([head[0] + 0.08, 0.0, head[2] - 0.03], [0.06, 0.09, 0.12], BONE));
    p.push(ellipsoid([head[0] + 0.03, 0.0, head[2]], [0.1, 0.13, 0.15], shadow));
    // A crown of ice shards rising back from its hood
    for (y, z, h) in [(0.0, 2.1, 0.3), (0.09, 2.07, 0.22), (-0.09, 2.07, 0.22), (0.16, 2.0, 0.15), (-0.16, 2.0, 0.15)] {
        p.push(limb(at([-0.02, y, z]), at([-0.1, y * 1.5, z + h]), 0.035, 0.008, ice));
    }
    p.push(ellipsoid([head[0] - 0.03, 0.0, head[2] + 0.02], [0.17, 0.18, 0.2], shroud));
    tag(p, start, HEAD);
    p.push(ellipsoid(at([0.16, 0.0, 1.4]), [0.025, 0.05, 0.09], glow).on(BODY));
    p.push(ellipsoid(at([-0.02, 0.0, 1.68]), [0.17, 0.31, 0.11], mantle).on(BODY));
    // Its arms: hanging and swaying, raised wide, thrust out together, or drawn in across it
    for s in [-1.0, 1.0] {
        let side = (s > 0.0) as usize;
        let start = p.len();
        let shoulder = at([0.0, s * 0.27, 1.65]);
        let swing = 0.06 * (angle + s).sin();
        let elbow = at(pose.at([0.18 + swing, s * 0.36, 1.3], [0.0, s * 0.62, 1.92], [0.48, s * 0.25, 1.56], [0.26, s * 0.16, 1.45]));
        let hand = at(pose.at([0.36 + swing, s * 0.32, 1.1], [0.1, s * 0.8, 2.24], [0.84, s * 0.12, 1.5], [0.32, -s * 0.06, 1.56]));
        let forward = (Vector3::from(hand) - Vector3::from(elbow)).normalize();
        for (k, spread) in [-1.0, 0.0, 1.0].into_iter().enumerate() {
            let tip = Vector3::from(hand) + forward * (0.22 + 0.04 * (k == 1) as u8 as f64) + Vector3::new(-forward.y, forward.x, 0.0) * spread * 0.05 + Vector3::z() * spread * 0.02;
            p.push(limb(hand, tip.into(), 0.022, 0.006, ice));
        }
        p.push(ball(hand, 0.04, shadow));
        p.push(limb(elbow, hand, 0.035, 0.03, shadow));
        tag(p, start, FORE[side]);
        p.push(limb(shoulder, elbow, 0.05, 0.04, shroud).on(ARM[side]));
    }
    // The shroud, narrowing then flaring as it hangs, and the rags under it streaming back
    let behind = 0.12 * moving + 0.1 * pose.special;
    p.push(limb(at([0.0, 0.0, 1.7]), at([-0.06 - behind * 0.5, 0.0, 0.95]), 0.2, 0.29, shroud).on(SHROUD));
    for k in 0..7 {
        let across = k as f64 / 6.0 * 2.0 - 1.0;
        let wave = (angle + k as f64 * 1.3).sin();
        let from = at([-0.06 - behind * 0.5, across * 0.22, 0.98]);
        let to = [-0.2 - behind * 2.0 + 0.05 * wave, across * 0.3 + 0.04 * wave, 0.3 + 0.07 * wave + 0.1 * (k % 2) as f64 + bob];
        p.push(limb(from, to, 0.075, 0.02, if k % 2 == 0 { shroud } else { shadow }).on(RAG[k]));
    }
}

/// A mire lurker, a meter and a half long: a squat, broad-backed thing of the jungle shallows, its
/// hide green and lumpy and grown over with fronds so it passes for a mossy log, a pale belly, a
/// flat head as wide as its body with bulging gold eyes on top and a mouth right across it, splayed
/// legs and a thick tail. It gapes as it gets ready, lunges and snaps its jaws shut, and flings
/// its mouth wide as it lashes out its tongue.
fn lurker(p: &mut Vec<Part>, angle: f64, swing: f64, moving: f64, pose: Pose, skin: Skin) {
    use crate::baked::lurker::*;
    let (hide, dark, belly, eye, tongue) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let gape = pose.mix(0.0, 0.7, 0.2, 1.0);
    let rear = pose.mix(0.0, 0.06, -0.03, 0.1);
    let lunge = pose.strike * 0.16 + pose.special * 0.06;
    let head = [0.62 + lunge, 0.0, 0.42 + rear];
    // Eyes bulging up out of the top of its head, so it can lie with only them above the water
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.07, s * 0.19, head[2] + 0.23], 0.045, eye).on(HEAD));
        p.push(ball([head[0] + 0.02, s * 0.19, head[2] + 0.18], 0.085, hide).on(HEAD));
        p.push(ball([head[0] + 0.29, s * 0.07, head[2] + 0.1 + 0.06 * gape], 0.02, dark).on(HEAD));
    }
    if gape > 0.5 {
        p.push(limb([head[0] + 0.02, 0.0, head[2] - 0.06], [head[0] + 0.26 + 0.12 * pose.special, 0.0, head[2] - 0.03], 0.055, 0.065, tongue).on(TONGUE));
    }
    p.push(ellipsoid([head[0] + 0.01, 0.0, head[2] - 0.06], [0.24, 0.31, 0.025 + 0.09 * gape], MEAT).on(MOUTH));
    p.push(ellipsoid([head[0], 0.0, head[2] + 0.05 + 0.08 * gape], [0.3, 0.37, 0.12], hide).on(HEAD));
    p.push(ellipsoid([head[0] - 0.03, 0.0, head[2] - 0.13 - 0.05 * gape], [0.28, 0.35, 0.08], belly).on(JAW));
    let start = p.len();
    // Lumps down its back, and fronds growing out of them
    for (x, y, r) in [(0.2, 0.18, 0.07), (0.05, -0.2, 0.08), (-0.15, 0.12, 0.07), (-0.3, -0.1, 0.06), (0.25, -0.05, 0.06), (-0.05, 0.25, 0.06)] {
        p.push(ball([x, y, 0.7 - 0.25 * (y * y + x * x * 0.2)], r, dark));
    }
    for (k, (x, y)) in [(0.15, 0.1), (-0.05, -0.15), (-0.25, 0.15), (0.0, 0.02)].into_iter().enumerate() {
        let wave = 0.05 * (angle + k as f64 * 1.7).sin();
        p.push(limb([x, y, 0.68], [x - 0.3 + wave, y * 1.8, 0.86 + 0.06 * k as f64], 0.035, 0.012, FERN).on(FROND[k]));
    }
    p.push(ellipsoid([0.0, 0.0, 0.27], [0.5, 0.4, 0.15], belly));
    p.push(ellipsoid([-0.05, 0.0, 0.45], [0.55, 0.45, 0.28], hide));
    tag(p, start, BODY);
    // Splayed legs, front and back, crawling by turns
    for s in [-1.0, 1.0] {
        let step = swing * s;
        let (fore, hind) = (crate::baked::leg_of(1.0, s), crate::baked::leg_of(-1.0, s));
        let (shoulder, elbow, foot) = ([0.32, s * 0.35, 0.34], [0.4 + 0.08 * step, s * 0.62, 0.3], [0.48 + 0.14 * step, s * 0.6, 0.02]);
        p.push(ellipsoid(foot, [0.09, 0.1, 0.03], dark).on(LOWER[fore]));
        p.push(limb(elbow, foot, 0.07, 0.06, hide).on(LOWER[fore]));
        p.push(limb(shoulder, elbow, 0.09, 0.07, hide).on(UPPER[fore]));
        let (hip, knee, foot) = ([-0.35, s * 0.37, 0.38], [-0.22, s * 0.68, 0.46], [-0.5 - 0.14 * step, s * 0.66, 0.02]);
        p.push(ellipsoid(foot, [0.11, 0.1, 0.03], dark).on(LOWER[hind]));
        p.push(limb(knee, foot, 0.08, 0.06, hide).on(LOWER[hind]));
        p.push(limb(hip, knee, 0.11, 0.08, hide).on(UPPER[hind]));
    }
    let sway = 0.12 * (angle + 0.5).sin() * moving;
    p.push(limb([-0.55, 0.0, 0.38], [-1.0, sway, 0.16], 0.17, 0.05, hide).on(TAIL));
}

/// A gloomwing, its wings well over a meter across: a cave flier of dark fur with a long pale bony
/// snout full of needle teeth, tall ears and red eyes, leathery wings spread on pale finger bones
/// beating fast, and hind claws dangling. It flies at about head height, raises its wings high
/// as it gets ready to dive, sweeps them back as it bites, and spreads them wide to screech.
fn gloomwing(p: &mut Vec<Part>, angle: f64, pose: Pose, skin: Skin) {
    let (fur, membrane, bone, eyes) = (skin.body, skin.dark, skin.accent, skin.glow);
    let h = 1.65;
    let flap = pose.mix(angle.sin(), 0.95, -0.25, 0.1);
    let sweep = pose.mix(0.0, 0.1, 0.4, -0.1);
    let dip = pose.mix(0.0, 0.06, 0.12, -0.08);
    let head = [0.17, 0.0, h + 0.03 - dip];
    use crate::baked::gloomwing::*;
    let start = p.len();
    for s in [-1.0, 1.0] {
        p.push(ball([head[0] + 0.07, s * 0.045, head[2] + 0.04], 0.02, eyes));
        p.push(limb([head[0] - 0.02, s * 0.05, head[2] + 0.06], [head[0] - 0.06, s * 0.09, head[2] + 0.19], 0.03, 0.01, membrane));
    }
    p.push(limb([head[0] + 0.15, 0.0, head[2] - 0.05], [head[0] + 0.12, 0.0, head[2] - 0.09], 0.012, 0.005, LINEN));
    p.push(limb([head[0] + 0.04, 0.0, head[2] - 0.01], [head[0] + 0.18, 0.0, head[2] - 0.04], 0.04, 0.02, skin.extra));
    p.push(ball(head, 0.075, fur));
    tag(p, start, HEAD);
    p.push(ellipsoid([0.0, 0.0, h], [0.15, 0.095, 0.085], fur).on(BODY));
    // Its wings: two bones from the shoulder out to the tip, and the membrane between them and
    // the fingers running back to its trailing edge, filled in with a fan of strips
    for s in [-1.0, 1.0] {
        let side = (s > 0.0) as usize;
        let shoulder = [0.04, s * 0.07, h + 0.03];
        let elbow = [-0.02 - sweep * 0.25, s * 0.32, h + 0.05 + 0.22 * flap];
        let tip = [-0.1 - sweep * 0.55, s * (0.7 - 0.25 * sweep.max(0.0)), h + 0.02 + 0.42 * flap];
        let edge = [
            [-0.17 - sweep * 0.2, s * 0.16, h - 0.02 + 0.05 * flap],
            [-0.24 - sweep * 0.4, s * 0.36, h + 0.0 + 0.16 * flap],
            [-0.22 - sweep * 0.55, s * 0.55, h + 0.0 + 0.3 * flap],
        ];
        p.push(limb(shoulder, elbow, 0.022, 0.018, bone).on(INNER[side]));
        p.push(limb(elbow, tip, 0.018, 0.01, bone).on(OUTER[side]));
        for (k, e) in edge.into_iter().enumerate() {
            p.push(limb(elbow, e, 0.012, 0.01, bone).on(if k == 0 { INNER[side] } else { OUTER[side] }));
        }
        let mid = |a: Vec3, b: Vec3, f: f64| -> Vec3 { [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * f) };
        // The strips between the shoulder and the elbow go with the inner wing, the rest the outer
        for (k, (a, b)) in [(shoulder, edge[0]), (mid(shoulder, elbow, 0.5), edge[0]), (elbow, edge[0]), (elbow, edge[1]), (mid(elbow, tip, 0.5), edge[1]), (mid(elbow, tip, 0.5), edge[2]), (tip, edge[2]), (edge[0], edge[1]), (edge[1], edge[2])].into_iter().enumerate() {
            let wing = if matches!(k, 0 | 1 | 2 | 7) { INNER[side] } else { OUTER[side] };
            p.push(limb(a, b, 0.04, 0.03, membrane).on(wing));
            p.push(limb(mid(a, b, 0.5), mid(elbow, mid(edge[1], tip, 0.5), 0.5), 0.035, 0.03, membrane).on(wing));
        }
    }
    // Hind claws dangling, and a little tail
    for s in [-1.0, 1.0] {
        p.push(limb([-0.08, s * 0.05, h - 0.05], [-0.15, s * 0.06, h - 0.18 + 0.04 * flap.abs()], 0.02, 0.012, bone).on(BODY));
    }
    p.push(limb([-0.14, 0.0, h], [-0.26, 0.0, h - 0.05], 0.025, 0.01, fur).on(BODY));
}

/// A tusker, two and a half meters long and as high at its humped shoulders as a man's chest: a
/// great boar-like forest beast with a shaggy dark mane, a ridge of black bristle spikes down its
/// back, a plate of bone over its brow, small burning eyes, and two great tusks curving up from
/// its lower jaw. It lowers its head and paws the ground before it charges or gores, then throws
/// its head up to toss whatever it caught.
fn tusker(p: &mut Vec<Part>, angle: f64, swing: f64, moving: f64, pose: Pose, skin: Skin) {
    let (hide, dark, snout, eye, tusk) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    // Its head turned about its neck: lowered to charge or paw, thrown up to gore
    let nod = pose.mix(0.0, -0.3, 0.45, -0.38);
    let neck = [0.55, 0.0, 0.95];
    let r = |v: Vec3| rear(v, neck, nod);
    use crate::baked::beast::*;
    let start = p.len();
    for s in [-1.0, 1.0] {
        p.push(ball(r([0.99, s * 0.16, 0.86]), 0.03, eye));
        p.push(limb(r([1.12, s * 0.11, 0.6]), r([1.27, s * 0.21, 0.67]), 0.042, 0.032, tusk));
        p.push(limb(r([1.27, s * 0.21, 0.67]), r([1.28, s * 0.17, 0.88]), 0.032, 0.008, tusk));
        p.push(limb(r([0.78, s * 0.14, 1.0]), r([0.68, s * 0.25, 1.12]), 0.05, 0.02, dark));
    }
    p.push(ellipsoid(r([1.3, 0.0, 0.62]), [0.035, 0.1, 0.08], NOSE));
    p.push(ellipsoid(r([0.9, 0.0, 0.96]), [0.16, 0.19, 0.06], tusk));
    p.push(limb(r([0.98, 0.0, 0.72]), r([1.26, 0.0, 0.62]), 0.15, 0.12, snout));
    p.push(ellipsoid(r([0.95, 0.0, 0.6]), [0.22, 0.17, 0.08], hide));
    p.push(ellipsoid(r([0.82, 0.0, 0.82]), [0.3, 0.25, 0.24], hide));
    tag(p, start, HEAD);
    let start = p.len();
    // A ridge of bristles down its back, tallest over its shoulders
    for k in 0..7 {
        let x = 0.55 - k as f64 * 0.18;
        let z = 1.3 - (x - 0.25).powi(2) * 0.8;
        p.push(limb([x, 0.0, z - 0.06], [x - 0.12, 0.0, z + 0.2 - k as f64 * 0.015], 0.05, 0.01, dark));
    }
    // A shaggy mane over its shoulders and chest, the hump, and its body
    p.push(ellipsoid([0.5, 0.0, 0.78], [0.3, 0.36, 0.34], dark));
    p.push(ellipsoid([0.25, 0.0, 1.0], [0.45, 0.42, 0.33], hide));
    p.push(ellipsoid([-0.15, 0.0, 0.86], [0.72, 0.4, 0.38], hide));
    tag(p, start, BODY);
    // A short tufted tail, swishing
    let sway = 0.08 * (angle + 1.0).sin() * moving;
    p.push(ball([-0.97, sway, 0.72], 0.05, dark).on(TAIL));
    p.push(limb([-0.85, 0.0, 0.95], [-0.97, sway, 0.74], 0.03, 0.02, hide).on(TAIL));
    // A front leg raised to paw the ground as it gets ready
    for (x, y, s) in [(0.5, 0.22, 1.0), (0.5, -0.22, -1.0), (-0.6, 0.22, -1.0), (-0.6, -0.22, 1.0)] {
        let paw = if x > 0.0 && s > 0.0 { 0.7 * pose.windup } else { 0.0 };
        let k = crate::baked::leg_of(x, y);
        leg(p, [x, y, 0.74], 0.74, if x > 0.0 { 0.1 } else { 0.11 }, 0.45 * swing * s + paw, hide, dark, [UPPER[k], LOWER[k]]);
    }
}

/// The shapes making up an animal, in meters: facing +x, with its feet on z = 0.
/// `phase` is how far through its walk it is, None standing still. A boss can strike a `pose`.
fn parts(look: Look, phase: Option<f64>, pose: Pose) -> Vec<Part> {
    let angle = phase.map_or(0.0, |p| p * TAU);
    let moving = phase.is_some() as u8 as f64;
    // Diagonal pairs of legs swing together
    let swing = angle.sin() * moving;
    let mut p = Vec::new();
    match look {
        // Fish and sharks, and what's left of them, see src\animals\sealife.rs
        _ if sealife::is_sea(look) || matches!(look, Look::RawFish | Look::CookedFish | Look::SharkTooth) => sealife::parts(&mut p, look, phase, [pose.windup, pose.strike, pose.special]),
        // Bosses and monsters, made at the size of their build, then grown or shrunk to their own
        _ if look.build().is_some() => {
            match look.build() {
                Some(Build::Spider) => spider(&mut p, angle, moving, pose, look.skin()),
                Some(Build::Wolf) => wolf(&mut p, angle, swing, moving, pose, look.skin(), look.finer()),
                Some(Build::Undead) if look == Look::Bloater => bloater(&mut p, swing, pose, look.skin()),
                Some(Build::Undead) if look == Look::Ghoul => ghoul(&mut p, angle, swing, moving, pose, look.skin()),
                Some(Build::Undead) => undead(&mut p, swing, pose, look.skin(), look.bony(), look.armored(), look.living(), look.arms()),
                Some(Build::Burrower) => burrower(&mut p, angle, moving, pose, look.skin()),
                Some(Build::Wraith) => wraith(&mut p, angle, moving, pose, look.skin()),
                Some(Build::Lurker) => lurker(&mut p, angle, swing, moving, pose, look.skin()),
                Some(Build::Bat) => gloomwing(&mut p, angle, pose, look.skin()),
                Some(Build::Boar) => tusker(&mut p, angle, swing, moving, pose, look.skin()),
                // Its pieces are made in src\dragons.rs, or src\avians.rs
                Some(Build::Dragon | Build::Winged) => {}
                Some(Build::Serpent) => {
                    if let Some(kind) = serpents::kind(look) {
                        serpents::parts(&mut p, kind, phase, [pose.windup, pose.strike, pose.special, pose.coil]);
                    }
                }
                // The rest are drawn out of jointed pieces, see src\rig.rs
                _ => {}
            }
            if look.scale() != 1.0 {
                p = p.iter().map(|part| part.scaled(look.scale())).collect();
            }
        }
        Look::Bear => {
            use crate::baked::beast::*;
            let (fur, dark) = (FUR_BROWN, FUR_DARK);
            p.push(ball([0.98, 0.12, 1.08], 0.035, EYE));
            p.push(ball([0.98, -0.12, 1.08], 0.035, EYE));
            p.push(ball([1.2, 0.0, 0.97], 0.05, EYE));
            p.push(ellipsoid([1.06, 0.0, 0.93], [0.16, 0.12, 0.1], FUR_CREAM));
            p.push(ball([0.78, 0.19, 1.24], 0.08, dark));
            p.push(ball([0.78, -0.19, 1.24], 0.08, dark));
            p.push(ball([0.84, 0.0, 1.02], 0.27, fur));
            tag(&mut p, 0, HEAD);
            p.push(ball([0.32, 0.0, 1.06], 0.32, fur).on(BODY));
            p.push(ellipsoid([0.0, 0.0, 0.88], [0.78, 0.42, 0.42], fur).on(BODY));
            p.push(ball([-0.78, 0.0, 0.96], 0.08, fur).on(TAIL));
            for (x, y, s) in [(0.48, 0.24, 1.0), (0.48, -0.24, -1.0), (-0.48, 0.24, -1.0), (-0.48, -0.24, 1.0)] {
                let k = crate::baked::leg_of(x, y);
                leg(&mut p, [x, y, 0.66], 0.66, 0.13, 0.35 * swing * s, fur, dark, [UPPER[k], LOWER[k]]);
            }
        }
        Look::Dog => {
            use crate::baked::beast::*;
            let (fur, white) = (FUR_TAN, FUR_WHITE);
            p.push(ball([0.51, 0.06, 0.75], 0.022, EYE));
            p.push(ball([0.51, -0.06, 0.75], 0.022, EYE));
            p.push(ball([0.635, 0.0, 0.685], 0.026, EYE));
            p.push(ellipsoid([0.55, 0.0, 0.66], [0.09, 0.06, 0.055], white));
            p.push(ellipsoid([0.39, 0.115, 0.68], [0.04, 0.025, 0.08], FUR_BROWN));
            p.push(ellipsoid([0.39, -0.115, 0.68], [0.04, 0.025, 0.08], FUR_BROWN));
            p.push(ball([0.42, 0.0, 0.71], 0.115, fur));
            tag(&mut p, 0, HEAD);
            p.push(limb([0.27, 0.0, 0.5], [0.38, 0.0, 0.66], 0.085, 0.07, fur).on(BODY));
            p.push(ellipsoid([0.2, 0.0, 0.41], [0.15, 0.12, 0.13], white).on(BODY));
            p.push(ellipsoid([0.0, 0.0, 0.47], [0.36, 0.15, 0.16], fur).on(BODY));
            // Wags while it walks
            let wag = 0.06 * (angle * 2.0).sin() * moving;
            p.push(limb([-0.32, 0.0, 0.53], [-0.5, wag, 0.7], 0.035, 0.025, fur).on(TAIL));
            for (x, y, s) in [(0.22, 0.09, 1.0), (0.22, -0.09, -1.0), (-0.24, 0.09, -1.0), (-0.24, -0.09, 1.0)] {
                let k = crate::baked::leg_of(x, y);
                leg(&mut p, [x, y, 0.4], 0.4, 0.042, 0.45 * swing * s, fur, white, [UPPER[k], LOWER[k]]);
            }
        }
        Look::GrayCat | Look::GingerCat => {
            use crate::baked::beast::*;
            let (fur, stripes) = if look == Look::GrayCat { (FUR_GRAY, FUR_DARK) } else { (FUR_GINGER, FUR_RED) };
            let white = FUR_WHITE;
            p.push(ball([0.3, 0.035, 0.38], 0.015, EYE));
            p.push(ball([0.3, -0.035, 0.38], 0.015, EYE));
            p.push(ball([0.327, 0.0, 0.345], 0.012, NOSE));
            p.push(ball([0.3, 0.0, 0.33], 0.038, white));
            p.push(limb([0.225, 0.045, 0.41], [0.22, 0.052, 0.47], 0.026, 0.006, fur));
            p.push(limb([0.225, -0.045, 0.41], [0.22, -0.052, 0.47], 0.026, 0.006, fur));
            p.push(ball([0.24, 0.0, 0.36], 0.08, fur));
            tag(&mut p, 0, HEAD);
            p.push(ellipsoid([0.03, 0.0, 0.21], [0.15, 0.06, 0.055], white).on(BODY));
            p.push(ellipsoid([0.0, 0.0, 0.25], [0.21, 0.085, 0.09], fur).striped(stripes).on(BODY));
            // The tail sways
            let sway = 0.04 * (angle + 1.0).sin() * moving;
            p.push(limb([-0.19, 0.0, 0.28], [-0.3, sway, 0.42], 0.022, 0.02, fur).striped(stripes).on(TAIL));
            p.push(limb([-0.3, sway, 0.42], [-0.28, sway * 1.5, 0.53], 0.02, 0.016, fur).striped(stripes).on(TAIL_TIP));
            for (x, y, s) in [(0.14, 0.05, 1.0), (0.14, -0.05, -1.0), (-0.13, 0.05, -1.0), (-0.13, -0.05, 1.0)] {
                let k = crate::baked::leg_of(x, y);
                leg(&mut p, [x, y, 0.21], 0.21, 0.024, 0.45 * swing * s, fur, white, [UPPER[k], LOWER[k]]);
            }
        }
        Look::Squirrel => {
            use crate::baked::beast::*;
            // Squirrels bound along: both pairs of legs together, the body bobbing up between
            let fur = FUR_RED;
            let lift = 0.04 * angle.sin().max(0.0) * moving;
            let up = |v: Vec3| [v[0], v[1], v[2] + lift];
            p.push(ball(up([0.13, 0.027, 0.175]), 0.011, EYE));
            p.push(ball(up([0.13, -0.027, 0.175]), 0.011, EYE));
            p.push(ball(up([0.146, 0.0, 0.16]), 0.009, EYE));
            p.push(limb(up([0.09, 0.022, 0.2]), up([0.088, 0.025, 0.23]), 0.012, 0.004, fur));
            p.push(limb(up([0.09, -0.022, 0.2]), up([0.088, -0.025, 0.23]), 0.012, 0.004, fur));
            p.push(ball(up([0.1, 0.0, 0.16]), 0.045, fur));
            tag(&mut p, 0, HEAD);
            p.push(ellipsoid(up([0.03, 0.0, 0.085]), [0.07, 0.035, 0.04], FUR_CREAM).on(BODY));
            p.push(ellipsoid(up([0.0, 0.0, 0.11]), [0.1, 0.05, 0.06], fur).on(BODY));
            // A big bushy tail curling up over its back
            let flick = 0.02 * angle.cos() * moving;
            for (k, (c, r)) in [([-0.1, 0.0, 0.11], 0.045), ([-0.15, 0.0, 0.17], 0.055), ([-0.15 + flick, 0.0, 0.25], 0.06), ([-0.11 + flick, 0.0, 0.31], 0.055)].into_iter().enumerate() {
                p.push(ball(up(c), r, fur).on(if k < 2 { TAIL } else { TAIL_TIP }));
            }
            for (x, y, length) in [(0.06, 0.03, 0.07), (0.06, -0.03, 0.07), (-0.06, 0.04, 0.08), (-0.06, -0.04, 0.08)] {
                let s = if x > 0.0 { 1.0 } else { -1.0 };
                let k = crate::baked::leg_of(x, y);
                leg(&mut p, up([x, y, length]), length, 0.018, 0.5 * swing * s, fur, fur, [UPPER[k], LOWER[k]]);
            }
        }
        Look::Apple => {
            p.push(limb([0.0, 0.0, 0.11], [0.01, 0.0, 0.145], 0.008, 0.006, WOOD));
            p.push(ellipsoid([0.028, 0.0, 0.138], [0.026, 0.012, 0.008], LEAVES));
            p.push(ellipsoid([0.0, 0.0, 0.062], [0.066, 0.066, 0.06], APPLE));
        }
        Look::Meat => {
            // A drumstick
            p.push(ball([-0.14, 0.022, 0.03], 0.024, BONE));
            p.push(ball([-0.14, -0.022, 0.03], 0.024, BONE));
            p.push(limb([-0.14, 0.0, 0.03], [-0.02, 0.0, 0.045], 0.016, 0.018, BONE));
            p.push(ellipsoid([0.05, 0.0, 0.055], [0.1, 0.07, 0.055], MEAT));
        }
        Look::Candle => {
            // A stubby candle with wax run down its side, and its flame
            p.push(ellipsoid([0.0, 0.0, 0.25], [0.028, 0.028, 0.05], FLAME));
            p.push(limb([0.0, 0.0, 0.18], [0.0, 0.0, 0.205], 0.008, 0.008, BARK_DARK));
            p.push(limb([0.045, 0.0, 0.06], [0.045, 0.0, 0.17], 0.018, 0.012, WAX));
            p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 0.18], 0.05, 0.045, WAX));
        }
        Look::Skull => {
            p.push(ball([0.085, 0.042, 0.115], 0.026, EYE));
            p.push(ball([0.085, -0.042, 0.115], 0.026, EYE));
            p.push(ball([0.105, 0.0, 0.075], 0.015, EYE));
            p.push(limb([0.075, -0.035, 0.045], [0.075, 0.035, 0.045], 0.008, 0.008, BARK_DARK));
            p.push(ellipsoid([0.055, 0.0, 0.045], [0.06, 0.06, 0.04], BONE));
            p.push(ellipsoid([0.0, 0.0, 0.115], [0.1, 0.085, 0.09], BONE));
        }
        Look::Merchant => {
            // A travelling pedlar in a hooded brown robe and green sash, a towering pack of wares
            // on its back, a lantern held out on a crook. It sways a little as it waits. Its shapes
            // are tagged with the bones it's posed by as it stands, see src\baked.rs: its head in
            // its hood, the arm holding out the lantern, the other, and the rest of it.
            use crate::baked::villager::*;
            let robe = BARK_DARK;
            // Its face in the shadow of the hood, eyes catching the lantern light
            for y in [-0.05, 0.05] {
                p.push(ball([0.2, y, 1.6], 0.018, EYE));
            }
            p.push(ellipsoid([0.15, 0.0, 1.57], [0.07, 0.1, 0.1], FUR_TAN));
            p.push(ellipsoid([0.06, 0.0, 1.62], [0.17, 0.17, 0.2], LEATHER));
            p.push(limb([0.0, 0.0, 1.5], [-0.08, 0.0, 1.8], 0.15, 0.04, LEATHER));
            tag(&mut p, 0, HEAD);
            // Its sash, and a coin purse at its belt
            p.push(ellipsoid([0.0, 0.0, 1.0], [0.21, 0.27, 0.06], MOSS).on(TORSO));
            p.push(ball([0.16, 0.18, 0.95], 0.06, GOLD).on(TORSO));
            // The robe, flaring to the ground
            p.push(limb([0.0, 0.0, 1.42], [0.0, 0.0, 0.05], 0.2, 0.32, robe).on(TORSO));
            // The arm holding out the lantern on its crook, the other at its side
            let start = p.len();
            p.push(limb([0.05, -0.22, 1.32], [0.32, -0.3, 1.1], 0.06, 0.05, robe));
            p.push(limb([0.32, -0.3, 1.1], [0.42, -0.3, 1.75], 0.025, 0.02, WOOD));
            p.push(limb([0.42, -0.3, 1.75], [0.55, -0.3, 1.7], 0.02, 0.02, WOOD));
            p.push(ball([0.55, -0.3, 1.55], 0.06, GLOW_FIRE));
            p.push(cuboid([0.5, -0.35, 1.48], [0.6, -0.25, 1.62], BRONZE));
            tag(&mut p, start, ARM[0]);
            p.push(limb([0.05, 0.22, 1.32], [0.1, 0.27, 0.95], 0.06, 0.05, robe).on(ARM[1]));
            // The pack: a frame stacked with a rolled rug, crates, pots and a bundle of tools
            let start = p.len();
            let back = -0.32;
            p.push(cuboid([back - 0.22, -0.26, 0.6], [back + 0.04, 0.26, 1.45], WOOD));
            p.push(limb([back - 0.1, -0.32, 1.55], [back - 0.1, 0.32, 1.55], 0.1, 0.1, CLOTH));
            p.push(cuboid([back - 0.2, -0.2, 1.62], [back + 0.02, 0.2, 1.88], WOOD));
            p.push(ball([back - 0.1, 0.12, 1.98], 0.1, CLAY_POT));
            p.push(ball([back - 0.12, -0.12, 1.96], 0.08, CLAY_POT));
            p.push(limb([back - 0.05, 0.3, 0.7], [back - 0.05, 0.36, 1.7], 0.025, 0.02, DARK_STEEL));
            p.push(limb([back - 0.1, -0.3, 0.7], [back - 0.1, -0.34, 1.6], 0.02, 0.02, WOOD));
            for z in [0.85, 1.25] {
                p.push(cuboid([back - 0.24, -0.27, z], [back + 0.06, 0.27, z + 0.04], LEATHER));
            }
            tag(&mut p, start, TORSO);
        }
        Look::Villager | Look::Townswoman => villager(&mut p, swing, look == Look::Townswoman),
        Look::Princess => princess(&mut p),
        Look::RoyalGuard => royal_guard(&mut p),
        Look::Bowyer | Look::Wandwright | Look::Bladesmith | Look::Gunsmith | Look::Jeweller | Look::Wingmaker => specialist(&mut p, look),
        Look::TradeStall => trade_stall(&mut p),
        Look::Coins => {
            // A little heap of gold coins, a few tipped up against it
            for (x, y, z) in [(0.0, 0.0, 0.02), (0.05, 0.04, 0.05), (-0.05, 0.03, 0.03), (0.03, -0.05, 0.03), (-0.02, -0.02, 0.07), (0.07, -0.02, 0.02)] {
                p.push(ellipsoid([x, y, z], [0.045, 0.045, 0.012], GOLD));
            }
            p.push(ellipsoid([-0.08, -0.05, 0.05], [0.012, 0.045, 0.045], GOLD));
            p.push(ellipsoid([0.0, 0.0, 0.03], [0.09, 0.09, 0.03], BRONZE));
        }
        Look::Potion => {
            // A round flask of red, glowing draught, stoppered with a cork
            p.push(limb([0.0, 0.0, 0.17], [0.0, 0.0, 0.2], 0.02, 0.018, WOOD));
            p.push(limb([0.0, 0.0, 0.11], [0.0, 0.0, 0.18], 0.025, 0.022, CRYSTAL));
            p.push(ball([0.0, 0.0, 0.07], 0.055, GLOW_BLOOD));
            p.push(ball([0.0, 0.0, 0.07], 0.07, CRYSTAL));
        }
        Look::Bread => {
            // A crusty loaf, scored across the top
            for x in [-0.06, 0.0, 0.06] {
                p.push(limb([x - 0.015, -0.03, 0.1], [x + 0.015, 0.03, 0.1], 0.012, 0.012, LINEN));
            }
            p.push(ellipsoid([0.0, 0.0, 0.055], [0.13, 0.07, 0.055], SANDSTONE_DARK));
        }
        Look::Chest | Look::OpenChest => {
            // A wooden chest bound with iron, its lock at the front. Open, its lid stands up
            // at the back and gold glints inside.
            let open = look == Look::OpenChest;
            if open {
                p.push(cuboid([-0.17, -0.3, 0.17], [0.17, 0.3, 0.21], GLOW_HOLY));
                p.push(cuboid([-0.18, -0.31, 0.04], [0.18, 0.31, 0.3], AIR));
            } else {
                p.push(cuboid([0.215, -0.04, 0.17], [0.245, 0.04, 0.26], GOLD));
            }
            for y in [-0.25, 0.25] {
                p.push(cuboid([-0.23, y - 0.025, 0.0], [0.23, y + 0.025, 0.29], DARK_STEEL));
            }
            p.push(cuboid([-0.22, -0.35, 0.0], [0.22, 0.35, 0.28], WOOD));
            let lid = |p: &mut Vec<Part>, lo: Vec3, hi: Vec3| {
                if open {
                    // Swung up and over on its hinge at the back
                    let turn = |q: Vec3| [-0.22 - (q[2] - 0.28), q[1], 0.28 + (q[0] + 0.22)];
                    let (a, b) = (turn(lo), turn(hi));
                    p.push(cuboid([a[0].min(b[0]), a[1], a[2].min(b[2])], [a[0].max(b[0]), b[1], a[2].max(b[2])], WOOD));
                } else {
                    p.push(cuboid(lo, hi, WOOD));
                }
            };
            if !open {
                for y in [-0.25, 0.25] {
                    p.push(cuboid([-0.23, y - 0.025, 0.28], [0.23, y + 0.025, 0.37], DARK_STEEL));
                    p.push(cuboid([-0.18, y - 0.025, 0.36], [0.18, y + 0.025, 0.41], DARK_STEEL));
                }
            }
            lid(&mut p, [-0.22, -0.35, 0.28], [0.22, 0.35, 0.36]);
            lid(&mut p, [-0.17, -0.35, 0.36], [0.17, 0.35, 0.4]);
        }
        Look::Teleporter | Look::RealmPortal => gate(&mut p, look == Look::Teleporter),
        // A pane of light across the opening of either
        Look::PortalVeil => p.push(cuboid([-0.03, -GATE_OPENING[0], GATE_OPENING[1]], [0.03, GATE_OPENING[0], GATE_OPENING[2]], GLOW_ARCANE)),
        Look::InnertBlock => {
            // A block a hand across, light burning all through it, its edges bound in crystal and
            // its corners in gold
            let h = 0.12;
            for x in [-h, h] {
                for y in [-h, h] {
                    for z in [0.0, 2.0 * h] {
                        p.push(ball([x, y, 0.02 + z], 0.032, GOLD));
                    }
                    p.push(limb([x, y, 0.02], [x, y, 0.02 + 2.0 * h], 0.022, 0.022, CRYSTAL));
                }
            }
            for z in [0.02, 0.02 + 2.0 * h] {
                for s in [-h, h] {
                    p.push(limb([-h, s, z], [h, s, z], 0.022, 0.022, CRYSTAL));
                    p.push(limb([s, -h, z], [s, h, z], 0.022, 0.022, CRYSTAL));
                }
            }
            p.push(cuboid([-h, -h, 0.02], [h, h, 0.02 + 2.0 * h], GLOW_ARCANE));
        }
        Look::GrappleHook => {
            // Three barbed prongs of bright steel curling back from the head of an iron shank,
            // their points catching the light, and a bronze ring at its tail for the rope, big
            // enough to be seen flying out a long way off
            let z = 0.22;
            for k in 0..3 {
                let a = k as f64 * TAU / 3.0 + 0.5;
                let (c, s) = (a.cos(), a.sin());
                p.push(ball([0.2, 0.17 * c, z + 0.17 * s], 0.025, GLOW_HOLY));
                p.push(limb([0.26, 0.0, z], [0.12, 0.16 * c, z + 0.16 * s], 0.028, 0.022, STEEL));
                p.push(limb([0.12, 0.16 * c, z + 0.16 * s], [0.2, 0.18 * c, z + 0.18 * s], 0.022, 0.01, STEEL));
            }
            p.push(ball([0.27, 0.0, z], 0.04, STEEL));
            p.push(limb([-0.2, 0.0, z], [0.26, 0.0, z], 0.03, 0.03, DARK_STEEL));
            p.push(ball([-0.25, 0.0, z], 0.05, BRONZE));
        }
        Look::Wings => {
            // A pair of wings of white feathers lying spread, sweeping down to their tips, on a
            // light frame of bone along their leading edges: wings to wear, of any kind, lying about
            for side in [-1.0, 1.0] {
                let edge = |f: f64| [0.05 - 0.23 * f, side * (0.15 + 1.2 * f), 0.75 - 0.45 * f];
                p.push(limb(edge(0.0), edge(0.5), 0.035, 0.03, BONE));
                p.push(limb(edge(0.5), edge(1.0), 0.03, 0.015, BONE));
                for k in 0..7 {
                    let f = 0.08 + 0.14 * k as f64;
                    let [x, y, z] = edge(f);
                    // Grey-tipped flight feathers out at the end
                    let tint = if k >= 5 { FUR_GRAY } else if k % 2 == 0 { LINEN } else { FUR_WHITE };
                    p.push(ellipsoid([x - 0.22, y, z - 0.02], [0.24 - 0.015 * k as f64, 0.1, 0.025], tint));
                }
            }
        }
        // A serpent's skin, shed whole and left lying in an S, pale and papery, its scales showing
        // in bands, the eye holes of its head dark
        Look::ShedSkin => {
            let at = |u: f64| [1.6 - 3.2 * u, 0.32 * (u * TAU * 1.3).sin() * (u * 4.0).min(1.0), 0.05];
            for s in [-1.0, 1.0] {
                p.push(ball([1.72, s * 0.06, 0.08], 0.025, BARK_DARK));
            }
            p.push(limb([1.55, 0.0, 0.06], [1.82, 0.0, 0.05], 0.1, 0.06, LINEN).stretched([1.0, 1.0, 0.5]));
            for k in 0..14 {
                let (u0, u1) = (k as f64 / 14.0, (k + 1) as f64 / 14.0);
                let r = 0.11 * (1.0 - 0.75 * u0);
                p.push(limb(at(u0), at(u1), r, r * 0.95, if k % 3 == 0 { FUR_CREAM } else { LINEN }).stretched([1.0, 1.0, 0.45]));
            }
        }
        // The bones of a great serpent: its backbone lying in a curve, ribs arching up off it, and
        // its long skull
        Look::SerpentBones => {
            let at = |u: f64| [2.6 - 5.2 * u, 0.6 * (u * TAU * 0.8).sin(), 0.14];
            p.push(ball([2.95, 0.12, 0.3], 0.07, BARK_DARK));
            p.push(ball([2.95, -0.12, 0.3], 0.07, BARK_DARK));
            p.push(limb([2.6, 0.0, 0.2], [3.45, 0.0, 0.14], 0.24, 0.1, BONE).stretched([1.0, 1.0, 0.7]));
            p.push(limb([2.65, 0.0, 0.05], [3.3, 0.0, 0.04], 0.15, 0.06, BONE).stretched([1.0, 1.0, 0.4]));
            for k in 0..18 {
                let u = k as f64 / 18.0;
                let c = at(u);
                let r = 0.1 * (1.0 - 0.6 * u);
                p.push(ball(c, r, BONE));
                // Ribs along the front of it, fewer and shorter towards its tail
                if u < 0.7 && k % 2 == 0 {
                    let rib = 0.55 * (1.0 - u);
                    for s in [-1.0, 1.0] {
                        let top = [c[0], c[1] + s * rib * 0.6, c[2] + rib * 0.9];
                        p.push(limb(c, top, 0.03, 0.025, BONE));
                        p.push(limb(top, [c[0] - 0.05, c[1] + s * rib, 0.03], 0.025, 0.02, BONE));
                    }
                }
                if k > 0 {
                    p.push(limb(at((k - 1) as f64 / 18.0), c, r * 0.5, r * 0.5, LIMESTONE));
                }
            }
        }
        Look::SerpentTongue => p.extend(serpents::tongue_shapes()),
        // Bosses, all built above
        _ => {}
    }
    p
}

/// The opening of the Innert Gate and the portals home, between their pillars: half as wide, and
/// from and to how high, in meters
pub const GATE_OPENING: [f64; 3] = [1.12, 0.52 + GATE_SINK, 4.98 + GATE_SINK];

/// How far down into the ground the Innert Gate and the portals go, in meters, so on a slope their
/// foundation's in the ground all round rather than their dais standing out over it: the models
/// stand that much lower than the ground they're on, and all of them's that much higher up in them
pub const GATE_SINK: f64 = 1.5;

/// How high each row of a realm's three sockets is up the front of the Innert Gate's pillars, over
/// its foundation, in meters: clear of the bands round them, up to three realms' to a pillar
const SOCKET_ROWS: [f64; 3] = [1.3, 2.15, 3.55];

/// Where the bottom middle of the socket for realm `realm`'s Innert block `slot` of three is on the
/// Innert Gate, in meters from the middle of its foot, facing along x: a row of three across the
/// front of a pillar for each realm, the realms taking turns, left and right, going up
pub fn socket(realm: usize, slot: usize) -> [f64; 3] {
    let side = if realm % 2 == 0 { -1.0 } else { 1.0 };
    let row = SOCKET_ROWS[(realm / 2).min(SOCKET_ROWS.len() - 1)];
    [0.36, side * (GATE_OPENING[0] + 0.48) + (slot as f64 - 1.0) * 0.295, GATE_SINK + row]
}

/// The Innert Gate, or with `gate` false a portal home in a realm: two square pillars banded in
/// obsidian, a heavy lintel across them crowned with a stone of glowing crystal, on a stepped dais.
/// The gate's of carved temple stone, with three sockets down the front of its pillars for each
/// realm's Innert blocks, each realm's three marked by a rune in its own color beside them; a
/// portal's of rough rock.
fn gate(p: &mut Vec<Part>, gate: bool) {
    let (stone, pillar, band) = if gate { (TEMPLE_STONE, TEMPLE_DARK, OBSIDIAN) } else { (ROCK, OBSIDIAN, TEMPLE_DARK) };
    if gate {
        for (realm, r) in crate::realms::REALMS.iter().enumerate() {
            for slot in 0..3 {
                let [x, y, z] = socket(realm, slot);
                // Hollowed out of the pillar, so it's left before the pillar's laid
                p.push(cuboid([x - 0.16, y - 0.14, z - 0.03], [x + 0.2, y + 0.14, z + 0.31], AIR));
            }
            // A bar of its rune's light under its row
            let ([x, left, low], right) = (socket(realm, 0), socket(realm, 2)[1]);
            p.push(cuboid([x + 0.04, left - 0.12, low - 0.13], [x + 0.11, right + 0.12, low - 0.07], r.rune));
        }
    }
    // What's below is built from the foot of it, and raised by GATE_SINK after, see below
    let first = p.len();
    let [half, _, high] = GATE_OPENING;
    let high = high - GATE_SINK;
    // The stone on top, the crest it sits in and the lintel under that
    p.push(ball([0.0, 0.0, 6.35], 0.3, GLOW_ARCANE));
    p.push(cuboid([-0.4, -1.0, high + 0.72], [0.4, 1.0, high + 1.07], pillar));
    p.push(cuboid([-0.6, -2.3, high + 0.02], [0.6, 2.3, high + 0.72], band));
    for side in [-1.0, 1.0] {
        let y = side * (half + 0.48);
        for z in [0.88, 3.02, 4.8] {
            p.push(cuboid([-0.5, y - 0.5, z], [0.5, y + 0.5, z + 0.16], band));
        }
        p.push(cuboid([-0.45, y - 0.45, 0.5], [0.45, y + 0.45, high + 0.02], pillar));
        p.push(cuboid([-0.62, y - 0.62, 0.5], [0.62, y + 0.62, 0.86], stone));
    }
    // The dais, in two steps
    p.push(cuboid([-1.8, -2.5, 0.25], [1.8, 2.5, 0.52], stone));
    p.push(cuboid([-2.4, -3.0, 0.0], [2.4, 3.0, 0.25], if gate { LIMESTONE } else { STONE }));
    // All of it up on the foundation going down into the ground under it
    for part in &mut p[first..] {
        part.a[2] += GATE_SINK;
        part.b[2] += GATE_SINK;
    }
    p.push(cuboid([-2.3, -2.9, 0.0], [2.3, 2.9, GATE_SINK], stone));
}

/// One of the folk of a town, about 1.75 meters tall in plain clothes, arms swinging as they walk:
/// a man in a green tunic, a belt and a straw hat, or a woman in a long red dress and white apron
/// with a basket of bread on her arm. Nothing in their hands, so they're plainly no threat.
fn villager(p: &mut Vec<Part>, swing: f64, woman: bool) {
    use crate::baked::villager::*;
    let skin = if woman { FUR_CREAM } else { FUR_TAN };
    // A straw hat or a kerchief over the hair, then the head under them. Its face is left plain:
    // at a few voxels across, eyes only make a band across it.
    if woman {
        p.push(ellipsoid([-0.04, 0.0, 1.68], [0.14, 0.15, 0.11], LINEN));
        p.push(ball([-0.16, 0.0, 1.57], 0.07, FUR_GINGER));
    } else {
        p.push(ellipsoid([0.01, 0.0, 1.735], [0.26, 0.26, 0.022], DRY_GRASS));
        p.push(ellipsoid([-0.005, 0.0, 1.762], [0.135, 0.135, 0.022], LEATHER));
        p.push(ellipsoid([-0.005, 0.0, 1.785], [0.13, 0.13, 0.065], DRY_GRASS));
        p.push(ellipsoid([-0.08, 0.0, 1.62], [0.1, 0.12, 0.1], FUR_BROWN));
    }
    p.push(ball([0.02, 0.0, 1.6], 0.135, skin));
    p.push(limb([0.0, 0.0, 1.4], [0.01, 0.0, 1.52], 0.05, 0.05, skin));
    tag(p, 0, HEAD);

    // The arms, swinging the other way to the legs on their side. She holds her basket on her
    // hip with one.
    let (body, sleeve) = if woman { (CLOTH, LINEN) } else { (MOSS, LINEN) };
    for s in [-1.0, 1.0] {
        let side = (s > 0.0) as usize;
        let start = p.len();
        let shoulder = [0.0, s * 0.2, 1.4];
        let (elbow, hand) = if woman && s < 0.0 {
            p.push(ellipsoid([0.2, -0.32, 1.03], [0.08, 0.05, 0.035], SANDSTONE_DARK));
            p.push(ball([0.25, -0.27, 1.02], 0.04, APPLE));
            p.push(limb([0.12, -0.32, 0.99], [0.29, -0.32, 0.99], 0.075, 0.075, DRY_GRASS));
            ([0.06, -0.27, 1.13], [0.17, -0.25, 1.03])
        } else {
            ([-0.13 * swing * s, s * 0.24, 1.13], [0.02 - 0.22 * swing * s, s * 0.25, 0.9])
        };
        p.push(ball(hand, 0.048, skin));
        p.push(limb(elbow, hand, 0.05, 0.045, sleeve));
        tag(p, start, FORE[side]);
        p.push(limb(shoulder, elbow, 0.06, 0.052, sleeve).on(ARM[side]));
    }

    if woman {
        // A bodice and apron over a long skirt, flaring out to the ground, her shoes peeping out
        // under it as she walks
        p.push(ellipsoid([0.0, 0.0, 1.22], [0.12, 0.18, 0.22], body).on(TORSO));
        p.push(ellipsoid([0.19, 0.0, 0.7], [0.07, 0.19, 0.32], LINEN).on(SKIRT));
        p.push(ellipsoid([0.0, 0.0, 1.01], [0.135, 0.19, 0.03], LINEN).on(TORSO));
        p.push(limb([0.0, 0.0, 1.02], [0.03 * swing, 0.0, 0.28], 0.16, 0.27, body).on(SKIRT));
        for (y, s) in [(0.09, 1.0), (-0.09, -1.0)] {
            let side = (y > 0.0) as usize;
            leg(p, [0.0, y, 0.8], 0.76, 0.06, 0.4 * swing * s, body, BARK_DARK, [THIGH[side], SHIN[side]]);
        }
    } else {
        // A tunic belted at the waist, trousers and boots
        p.push(ellipsoid([0.0, 0.0, 0.99], [0.15, 0.215, 0.045], LEATHER).on(TORSO));
        p.push(ellipsoid([0.0, 0.0, 1.2], [0.14, 0.2, 0.26], body).on(TORSO));
        p.push(limb([0.0, 0.0, 1.0], [0.0, 0.0, 0.8], 0.19, 0.22, body).on(TORSO));
        for (y, s) in [(0.1, 1.0), (-0.1, -1.0)] {
            let side = (y > 0.0) as usize;
            leg(p, [0.0, y, 0.9], 0.86, 0.075, 0.45 * swing * s, LEATHER, BARK_DARK, [THIGH[side], SHIN[side]]);
        }
    }
}

/// A specialist merchant, see src\merchants.rs, about 1.75 meters tall and standing by their
/// wares, each in the clothes of their trade with what they make in hand: the bowyer in a green
/// cloak and feathered cap, a longbow upright in hand and a quiver on the back; the wandwright in a
/// violet robe and pointed hat, a staff topped with a glowing crystal; the weaponsmith in a leather
/// apron over a red shirt, sleeves rolled, resting on a sword held point down; the gunsmith in a long
/// dark coat and tricorn with goggles on it, a musket on the shoulder and a powder horn at the hip;
/// the jeweller in a red robe and turban, holding out a tray of glowing stones; and the wingmaker in
/// sky blue, goggles up, a pair of white wings on the back and a quill in hand
fn specialist(p: &mut Vec<Part>, look: Look) {
    // Their shapes are tagged with the bones they're posed by as they stand, see src\baked.rs:
    // what's on their head goes with it, what's in a hand that's free with that forearm, and what
    // they hold to the ground or in both hands with their body, so it stays where it is
    use crate::baked::villager::*;
    let start = p.len();
    // What they wear and carry first, as earlier shapes win where they overlap
    let (skin, hair, top, legs, boots, robe) = match look {
        Look::Bowyer => {
            p.push(limb([-0.06, 0.09, 1.76], [-0.22, 0.14, 1.95], 0.025, 0.012, FLOWER_RED));
            p.push(ellipsoid([0.0, 0.0, 1.73], [0.15, 0.15, 0.065], MOSS));
            p.push(ellipsoid([0.05, 0.0, 1.69], [0.2, 0.17, 0.025], MOSS));
            tag(p, start, HEAD);
            // The longbow, upright in the right hand, bending forward, and its string
            let (bx, by) = (0.22, -0.33);
            let mut last = [bx, by, 0.2];
            for k in 1..=12 {
                let t = k as f64 / 12.0;
                let at = [bx + 0.14 * (t * PI).sin(), by, 0.2 + 1.75 * t];
                p.push(limb(last, at, 0.03, 0.03, WOOD));
                last = at;
            }
            p.push(limb([bx - 0.01, by, 0.22], [bx - 0.01, by, 1.93], 0.014, 0.014, BOWSTRING));
            // The quiver on the back, arrows standing up out of it
            for k in 0..4 {
                let y = -0.07 + k as f64 * 0.045;
                p.push(ball([-0.29, y, 1.72 + 0.02 * (k % 2) as f64], 0.035, FLOWER_WHITE));
                p.push(limb([-0.27, y, 1.45], [-0.29, y, 1.72], 0.014, 0.014, WOOD));
            }
            p.push(limb([-0.22, 0.04, 0.95], [-0.27, -0.02, 1.5], 0.08, 0.075, LEATHER));
            (FUR_TAN, FUR_BROWN, LEATHER, FUR_BROWN, LEATHER, Some((MOSS, 0.15, 0.24)))
        }
        Look::Wandwright => {
            // The pointed hat, its gold band and wide brim, and a white beard
            p.push(limb([-0.01, 0.0, 1.74], [-0.12, 0.0, 2.12], 0.14, 0.012, FLOWER_PURPLE));
            p.push(ellipsoid([0.0, 0.0, 1.745], [0.15, 0.15, 0.03], GOLD));
            p.push(ellipsoid([0.0, 0.0, 1.715], [0.26, 0.26, 0.025], FLOWER_PURPLE));
            p.push(ellipsoid([0.13, 0.0, 1.47], [0.06, 0.1, 0.13], FUR_WHITE));
            tag(p, start, HEAD);
            // The staff, a crystal glowing at its head between gold prongs
            let (sx, sy) = (0.24, -0.31);
            p.push(ball([sx, sy, 2.03], 0.07, GLOW_ARCANE));
            for a in [0.0f64, 2.1, 4.2] {
                p.push(limb([sx, sy, 1.92], [sx + 0.07 * a.cos(), sy + 0.07 * a.sin(), 2.07], 0.018, 0.012, GOLD));
            }
            p.push(limb([sx, sy, 0.02], [sx, sy, 1.94], 0.03, 0.028, WOOD));
            // A satchel at the hip, scrolls sticking out of it
            p.push(limb([0.02, 0.29, 1.05], [0.02, 0.33, 1.2], 0.025, 0.025, LINEN));
            p.push(ellipsoid([0.02, 0.27, 0.95], [0.1, 0.07, 0.1], LEATHER));
            (FUR_CREAM, FUR_WHITE, FLOWER_PURPLE, FLOWER_PURPLE, BARK_DARK, Some((FLOWER_PURPLE, 0.19, 0.35)))
        }
        Look::Bladesmith => {
            // A red headband, and a short beard
            p.push(ellipsoid([0.0, 0.0, 1.69], [0.14, 0.14, 0.03], CLOTH));
            p.push(ellipsoid([0.12, 0.0, 1.51], [0.05, 0.1, 0.06], FUR_BROWN));
            tag(p, start, HEAD);
            // The sword, point down in front, both hands resting on its pommel
            let sx = 0.32;
            p.push(ball([sx, 0.0, 1.17], 0.04, GOLD));
            p.push(limb([sx, 0.0, 1.0], [sx, 0.0, 1.14], 0.03, 0.03, LEATHER));
            p.push(cuboid([sx - 0.03, -0.15, 0.96], [sx + 0.03, 0.15, 1.01], GOLD));
            p.push(limb([sx, 0.0, 0.96], [sx, 0.0, 0.1], 0.04, 0.012, STEEL));
            // A hammer hung at the belt, and the leather apron
            p.push(cuboid([0.02, 0.2, 0.6], [0.12, 0.3, 0.68], DARK_STEEL));
            p.push(limb([0.07, 0.25, 0.65], [0.07, 0.24, 0.95], 0.02, 0.02, WOOD));
            p.push(ellipsoid([0.13, 0.0, 0.92], [0.05, 0.18, 0.45], LEATHER));
            (FUR_TAN, FUR_DARK, CLOTH, BARK_DARK, LEATHER, None)
        }
        Look::Gunsmith => {
            // The tricorn, goggles strapped round it, and a moustache
            for y in [-0.06, 0.06] {
                p.push(ball([0.14, y, 1.79], 0.035, CRYSTAL));
            }
            p.push(ellipsoid([0.0, 0.0, 1.79], [0.135, 0.135, 0.04], BRONZE));
            p.push(ellipsoid([0.0, 0.0, 1.8], [0.13, 0.13, 0.08], FUR_DARK));
            for k in 0..3 {
                let a = k as f64 * TAU / 3.0;
                p.push(limb([0.0, 0.0, 1.735], [0.22 * a.cos(), 0.22 * a.sin(), 1.77], 0.06, 0.03, FUR_DARK));
            }
            p.push(ellipsoid([0.13, 0.0, 1.55], [0.035, 0.08, 0.02], FUR_DARK));
            tag(p, start, HEAD);
            // The musket over the right shoulder, held at its stock, and a powder horn
            p.push(limb([0.18, -0.25, 1.06], [0.02, -0.26, 1.38], 0.05, 0.04, WOOD));
            p.push(limb([0.02, -0.26, 1.38], [-0.3, -0.27, 2.0], 0.028, 0.025, DARK_STEEL));
            p.push(limb([0.06, 0.25, 1.0], [0.16, 0.23, 0.86], 0.05, 0.02, BONE));
            // Brass buttons down the coat
            for z in [1.1, 1.22, 1.34] {
                p.push(ball([0.14, 0.0, z], 0.025, BRONZE));
            }
            (FUR_TAN, FUR_DARK, BARK_DARK, LEATHER, BARK_DARK, Some((BARK_DARK, 0.17, 0.27)))
        }
        Look::Jeweller => {
            // A turban with a red stone set in it, a loupe at one eye
            p.push(ball([0.14, 0.0, 1.76], 0.035, GLOW_BLOOD));
            p.push(ellipsoid([0.0, 0.0, 1.74], [0.15, 0.15, 0.09], LINEN));
            p.push(ball([0.13, 0.05, 1.61], 0.032, BRONZE));
            tag(p, start, HEAD);
            // The tray of stones held out in both hands
            for (k, gem) in [GLOW_BLOOD, GLOW_POISON, GLOW_FROST, GLOW_HOLY, GLOW_ARCANE].into_iter().enumerate() {
                p.push(ball([0.33 + 0.04 * (k % 2) as f64, -0.14 + k as f64 * 0.07, 1.08], 0.038, gem));
            }
            p.push(cuboid([0.22, -0.2, 1.0], [0.44, 0.2, 1.05], WOOD));
            // A gold sash and hem
            p.push(ellipsoid([0.0, 0.0, 1.0], [0.18, 0.24, 0.035], GOLD));
            p.push(ellipsoid([0.0, 0.0, 0.07], [0.33, 0.33, 0.04], GOLD));
            (FUR_BROWN, FUR_DARK, TEMPLE_RED, TEMPLE_RED, LEATHER, Some((TEMPLE_RED, 0.18, 0.31)))
        }
        _ => {
            // The wingmaker: goggles up on a leather cap, a scarf, and a quill in hand
            for y in [-0.06, 0.06] {
                p.push(ball([0.13, y, 1.72], 0.035, CRYSTAL));
            }
            p.push(ellipsoid([0.0, 0.0, 1.71], [0.14, 0.14, 0.03], BRONZE));
            p.push(ellipsoid([-0.01, 0.0, 1.66], [0.14, 0.145, 0.12], LEATHER));
            tag(p, start, HEAD);
            p.push(limb([0.2, -0.3, 1.05], [0.3, -0.36, 1.32], 0.03, 0.015, FUR_WHITE).on(FORE[0]));
            p.push(limb([-0.1, 0.12, 1.42], [-0.2, 0.18, 1.1], 0.04, 0.03, LINEN));
            p.push(ellipsoid([0.02, 0.0, 1.43], [0.12, 0.17, 0.045], LINEN));
            // A pair of white wings on the back, half spread
            for s in [-1.0, 1.0] {
                p.push(ellipsoid([-0.27, s * 0.42, 1.35], [0.035, 0.3, 0.38], FUR_WHITE));
                p.push(limb([-0.23, s * 0.12, 1.45], [-0.28, s * 0.68, 1.75], 0.05, 0.035, FUR_WHITE));
            }
            (FUR_CREAM, FUR_GINGER, FLOWER_BLUE, LINEN, LEATHER, None)
        }
    };
    tag(p, start, TORSO);
    // The head, the hair round the back of it, and the neck
    p.push(ellipsoid([-0.05, 0.0, 1.62], [0.11, 0.135, 0.11], hair).on(HEAD));
    p.push(ball([0.02, 0.0, 1.6], 0.13, skin).on(HEAD));
    p.push(limb([0.0, 0.0, 1.4], [0.01, 0.0, 1.52], 0.05, 0.05, skin).on(HEAD));
    // The arms: where each hand is, by what they hold
    let hands: [Vec3; 2] = match look {
        Look::Bowyer => [[0.22, -0.33, 1.05], [0.06, 0.26, 0.92]],
        Look::Wandwright => [[0.24, -0.31, 1.12], [0.08, 0.27, 0.95]],
        Look::Bladesmith => [[0.3, -0.05, 1.08], [0.3, 0.05, 1.08]],
        Look::Gunsmith => [[0.16, -0.25, 1.08], [0.08, 0.26, 0.93]],
        Look::Jeweller => [[0.26, -0.2, 1.03], [0.26, 0.2, 1.03]],
        _ => [[0.2, -0.3, 1.05], [0.06, 0.26, 0.92]],
    };
    // Rolled up, the weaponsmith's forearms are bare
    let (upper, fore) = match look {
        Look::Bladesmith => (CLOTH, skin),
        _ => (top, top),
    };
    for (k, s) in [-1.0f64, 1.0].into_iter().enumerate() {
        let hand = hands[k];
        let shoulder = [0.0, s * 0.2, 1.4];
        let elbow = [0.03 + (hand[0] - 0.03) * 0.3, s * 0.26, 1.14 + (hand[2] - 1.0) * 0.3];
        p.push(ball(hand, 0.047, skin).on(FORE[k]));
        p.push(limb(elbow, hand, 0.05, 0.045, fore).on(FORE[k]));
        p.push(limb(shoulder, elbow, 0.06, 0.052, upper).on(ARM[k]));
    }
    // A belt, the body, and a robe flaring to the ground or legs in boots
    let start = p.len();
    p.push(ellipsoid([0.0, 0.0, 0.99], [0.15, 0.215, 0.04], LEATHER));
    p.push(ellipsoid([0.0, 0.0, 1.21], [0.135, 0.195, 0.24], top));
    match robe {
        Some((cloth, waist, hem)) => p.push(limb([0.0, 0.0, 1.05], [0.0, 0.0, 0.04], waist, hem, cloth)),
        None => {
            p.push(limb([0.0, 0.0, 1.0], [0.0, 0.0, 0.82], 0.18, 0.2, top));
            for y in [-0.1, 0.1] {
                p.push(limb([0.03, y, 0.18], [0.07, y, 0.02], 0.075, 0.08, boots));
                p.push(limb([0.0, y, 0.88], [0.02, y, 0.15], 0.085, 0.07, legs));
            }
        }
    }
    tag(p, start, TORSO);
}

/// The stand a specialist sets up at the base, see src\merchants.rs: a table with a cloth hung down
/// its front, a striped awning on two posts at its back, and pots and a crate of goods on it
fn trade_stall(p: &mut Vec<Part>) {
    p.push(ball([0.05, -0.35, 1.0], 0.1, CLAY_POT));
    p.push(ball([0.12, 0.38, 0.98], 0.08, CLAY_POT));
    p.push(cuboid([-0.18, -0.12, 0.88], [0.12, 0.18, 1.1], WOOD));
    p.push(limb([0.0, -0.05, 0.93], [0.0, 0.1, 0.93], 0.06, 0.06, LINEN));
    // The awning, in stripes, sloping down to the front
    for k in 0..6 {
        let y0 = -0.72 + k as f64 * 0.24;
        p.push(cuboid([-0.42, y0, 1.82], [0.48, y0 + 0.24, 1.88], if k % 2 == 0 { CLOTH } else { LINEN }));
    }
    for y in [-0.62, 0.62] {
        p.push(limb([-0.34, y, 0.0], [-0.34, y, 1.84], 0.035, 0.035, WOOD));
    }
    // The cloth down the front, the top and the legs
    p.push(cuboid([0.27, -0.62, 0.35], [0.32, 0.62, 0.84], CLOTH));
    p.push(cuboid([-0.3, -0.65, 0.8], [0.3, 0.65, 0.88], WOOD));
    for (x, y) in [(-0.25, -0.58), (-0.25, 0.58), (0.25, -0.58), (0.25, 0.58)] {
        p.push(limb([x, y, 0.0], [x, y, 0.8], 0.03, 0.03, WOOD));
    }
}

/// The princess, about 1.7 meters tall: a long gown of deep teal flaring out to the ground over a
/// white underskirt, gold at her waist and round its hem, white bell sleeves, her hands folded in
/// front of her, and her red hair in a long braid down her back under a little gold crown with a
/// red stone in it
fn princess(p: &mut Vec<Part>) {
    use crate::baked::villager::*;
    let (skin, gown, hair) = (FUR_CREAM, TEMPLE_TEAL, FUR_GINGER);
    // The crown: a gold band round the top of her head with points rising from it
    p.push(ball([0.115, 0.0, 1.715], 0.022, GLOW_BLOOD));
    for k in 0..5 {
        let a = (k as f64 - 2.0) * 0.75;
        p.push(limb([0.09 * a.cos(), 0.09 * a.sin(), 1.71], [0.085 * a.cos(), 0.085 * a.sin(), 1.79], 0.022, 0.008, GOLD));
    }
    p.push(ellipsoid([0.0, 0.0, 1.705], [0.112, 0.112, 0.028], GOLD));
    // Her hair over the back of her head, and the braid down her back, which goes with her
    // body as she turns her head, see src\baked.rs
    p.push(limb([-0.12, 0.0, 1.55], [-0.17, 0.0, 0.98], 0.045, 0.03, hair).on(TORSO));
    p.push(ellipsoid([-0.035, 0.0, 1.64], [0.12, 0.14, 0.12], hair));
    p.push(ball([0.02, 0.0, 1.6], 0.125, skin));
    p.push(limb([0.0, 0.0, 1.4], [0.01, 0.0, 1.52], 0.045, 0.045, skin));
    tag(p, 0, HEAD);
    // A white collar, and the arms in their sleeves, hands folded at her waist
    p.push(ellipsoid([0.02, 0.0, 1.43], [0.1, 0.15, 0.03], LINEN).on(TORSO));
    for (k, s) in [-1.0, 1.0].into_iter().enumerate() {
        let (shoulder, elbow, hand) = ([0.0, s * 0.19, 1.4], [0.05, s * 0.24, 1.13], [0.18, s * 0.06, 1.04]);
        p.push(ball(hand, 0.045, skin).on(FORE[k]));
        p.push(limb(elbow, [0.15, s * 0.1, 1.05], 0.05, 0.075, LINEN).on(FORE[k]));
        p.push(limb(shoulder, elbow, 0.058, 0.05, LINEN).on(ARM[k]));
    }
    // The bodice, a gold girdle, and the gown with its white front and gold hem
    let start = p.len();
    p.push(ellipsoid([0.0, 0.0, 1.035], [0.13, 0.185, 0.03], GOLD));
    p.push(ellipsoid([0.0, 0.0, 1.22], [0.12, 0.17, 0.21], gown));
    p.push(ellipsoid([0.0, 0.0, 0.06], [0.38, 0.38, 0.045], GOLD));
    p.push(ellipsoid([0.235, 0.0, 0.5], [0.06, 0.13, 0.42], LINEN));
    p.push(limb([0.0, 0.0, 1.02], [0.02, 0.0, 0.05], 0.16, 0.36, gown));
    tag(p, start, TORSO);
}

/// A royal guard, about 1.85 meters tall, standing to attention: a steel helm with a red crest, a
/// breastplate over a red tabard banded in gold, a tall spear upright in one hand and a shield on
/// the other arm
fn royal_guard(p: &mut Vec<Part>) {
    use crate::baked::villager::*;
    let skin = FUR_TAN;
    // The helm, its crest, and the face under its brim
    p.push(limb([0.02, 0.0, 1.89], [-0.15, 0.0, 1.93], 0.045, 0.03, CLOTH));
    p.push(ellipsoid([0.0, 0.0, 1.77], [0.145, 0.14, 0.12], STEEL));
    p.push(ellipsoid([0.01, 0.0, 1.69], [0.165, 0.165, 0.022], STEEL));
    p.push(ball([0.02, 0.0, 1.62], 0.12, skin));
    p.push(limb([0.0, 0.0, 1.42], [0.01, 0.0, 1.54], 0.05, 0.05, skin));
    tag(p, 0, HEAD);
    // The spear, held upright beside it, and the shield on its other arm, both with its body, as
    // it stands to attention, see src\baked.rs
    let start = p.len();
    let spear = [0.17, -0.31];
    p.push(limb([spear[0], spear[1], 2.27], [spear[0], spear[1], 2.52], 0.04, 0.006, STEEL));
    p.push(limb([spear[0], spear[1], 2.22], [spear[0], spear[1], 2.27], 0.045, 0.045, GOLD));
    p.push(limb([spear[0], spear[1], 0.02], [spear[0], spear[1], 2.25], 0.025, 0.025, WOOD));
    p.push(ellipsoid([0.2, 0.31, 1.08], [0.045, 0.19, 0.27], CLOTH));
    p.push(ellipsoid([0.19, 0.31, 1.08], [0.04, 0.22, 0.3], GOLD));
    // The arms, one holding the spear, pauldrons on the shoulders
    for (s, hand) in [(-1.0, [spear[0] - 0.02, spear[1] + 0.03, 1.12]), (1.0, [0.13, 0.27, 1.1])] {
        let (shoulder, elbow) = ([0.0, s * 0.21, 1.42], [0.05, s * 0.27, 1.15]);
        p.push(ball(hand, 0.05, LEATHER));
        p.push(ball([0.0, s * 0.22, 1.43], 0.085, STEEL));
        p.push(limb(elbow, hand, 0.052, 0.047, DARK_STEEL));
        p.push(limb(shoulder, elbow, 0.062, 0.054, DARK_STEEL));
    }
    // Breastplate, the tabard under it with its gold band, a belt, and legs in greaves
    p.push(ellipsoid([0.0, 0.0, 1.27], [0.14, 0.205, 0.2], STEEL));
    p.push(ellipsoid([0.02, 0.0, 0.96], [0.15, 0.2, 0.035], LEATHER));
    p.push(ellipsoid([0.035, 0.0, 0.92], [0.13, 0.045, 0.28], GOLD));
    p.push(ellipsoid([0.02, 0.0, 0.92], [0.15, 0.2, 0.3], CLOTH));
    tag(p, start, TORSO);
    for y in [0.1, -0.1] {
        let side = (y > 0.0) as usize;
        leg(p, [0.0, y, 0.88], 0.86, 0.075, 0.0, DARK_STEEL, LEATHER, [THIGH[side], SHIN[side]]);
    }
}

/// A model rasterized into voxels: x + (y + z * size.y) * size.x
#[derive(Clone)]
pub struct Model {
    pub size: [u32; 3],
    pub voxels: Vec<u8>,
}

/// Turns shapes in meters into a model with `res` voxels per meter, sized to fit them however
/// they're placed. Returns where the shader puts the model in the world, the middle of its bottom,
/// in the same meters as the shapes.
pub fn voxelize(parts: &[Part], res: f64) -> (Model, Vector3<f64>) {
    let (model, anchor, _) = voxelize_tagged(parts, res);
    (model, anchor)
}

/// The same, with the tag of the shape each voxel was taken by, see Part::tagged(), 0 for air
pub fn voxelize_tagged(parts: &[Part], res: f64) -> (Model, Vector3<f64>, Vec<u8>) {
    let parts: Vec<Part> = parts.iter().map(|p| p.thickened(res)).collect();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for part in &parts {
        let (a, b) = part.reach();
        for i in 0..3 {
            lo[i] = lo[i].min(a[i]);
            hi[i] = hi[i].max(b[i]);
        }
    }
    let size = [0, 1, 2].map(|i| (((hi[i] - lo[i]) * res).ceil() as u32).clamp(1, 1023));
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    // Each shape only looks at the voxels around it, and takes those no earlier shape took
    let mut taken = vec![false; voxels.len()];
    let mut tags = vec![0u8; voxels.len()];
    for part in &parts {
        let (a, b) = part.reach();
        let from = [0, 1, 2].map(|i| (((a[i] - lo[i]) * res - 0.5).floor().max(0.0) as u32).min(size[i]));
        let to = [0, 1, 2].map(|i| (((b[i] - lo[i]) * res + 0.5).ceil().max(0.0) as u32).min(size[i]));
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let i = (x + (y + z * size[1]) * size[0]) as usize;
                    let p = [lo[0] + (x as f64 + 0.5) / res, lo[1] + (y as f64 + 0.5) / res, lo[2] + (z as f64 + 0.5) / res];
                    if !taken[i] && part.contains(p) {
                        taken[i] = true;
                        voxels[i] = part.material;
                        tags[i] = part.tag;
                    }
                }
            }
        }
    }
    let anchor = Vector3::new(lo[0] + size[0] as f64 * 0.5 / res, lo[1] + size[1] as f64 * 0.5 / res, lo[2]);
    (Model { size, voxels }, anchor, tags)
}

/// How big a frame made of `parts`, thickened, is at `res` voxels a meter: wide enough either side
/// of the middle to hold everything, as the shader puts the middle of the model where the animal
/// stands, and from its feet up
pub(crate) fn frame_size(parts: &[Part], res: f64) -> [u32; 3] {
    let (mut half_x, mut half_y, mut top) = (0.0f64, 0.0f64, 0.0f64);
    for part in parts {
        let (lo, hi) = part.reach();
        half_x = half_x.max(lo[0].abs()).max(hi[0].abs());
        half_y = half_y.max(lo[1].abs()).max(hi[1].abs());
        top = top.max(hi[2]);
    }
    [((2.0 * half_x * res).ceil() as u32).clamp(1, 255), ((2.0 * half_y * res).ceil() as u32).clamp(1, 255), ((top * res).ceil() as u32).clamp(1, 255)]
}

/// The voxels of a frame `size` big at `res` voxels a meter round its middle that `part`, thickened,
/// might take: from and to, in each direction
pub(crate) fn frame_span(part: &Part, size: [u32; 3], res: f64) -> ([u32; 3], [u32; 3]) {
    let origin = [-(size[0] as f64) * 0.5, -(size[1] as f64) * 0.5, 0.0];
    let (a, b) = part.reach();
    let from = [0, 1, 2].map(|i| ((a[i] * res - origin[i] - 0.5).floor().max(0.0) as u32).min(size[i]));
    let to = [0, 1, 2].map(|i| ((b[i] * res - origin[i] + 0.5).ceil().max(0.0) as u32).min(size[i]));
    (from, to)
}

/// The middle of voxel `x`, `y`, `z` of a frame `size` big, in meters from the middle of its feet
pub(crate) fn frame_voxel(size: [u32; 3], res: f64, x: u32, y: u32, z: u32) -> Vec3 {
    let origin = [-(size[0] as f64) * 0.5, -(size[1] as f64) * 0.5, 0.0];
    [(x as f64 + 0.5 + origin[0]) / res, (y as f64 + 0.5 + origin[1]) / res, (z as f64 + 0.5) / res]
}

/// What `part` makes voxel `x` of a frame: its own material, or a band of its stripes
pub(crate) fn banded(part: &Part, x: u32) -> u8 {
    if part.stripes != AIR && (x / 2) % 3 == 0 { part.stripes } else { part.material }
}

fn rasterize(look: Look, phase: Option<f64>, pose: Pose) -> Model {
    let res = look.resolution();
    let parts: Vec<Part> = parts(look, phase, pose).iter().map(|p| p.thickened(res)).collect();
    let size = frame_size(&parts, res);
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    // Each shape only looks at the voxels around it, and takes those no earlier shape took
    let mut taken = vec![false; voxels.len()];
    for part in &parts {
        let (from, to) = frame_span(part, size, res);
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let i = (x + (y + z * size[1]) * size[0]) as usize;
                    if !taken[i] && part.contains(frame_voxel(size, res, x, y, z)) {
                        taken[i] = true;
                        voxels[i] = banded(part, x);
                    }
                }
            }
        }
    }
    Model { size, voxels }
}

/// The shapes making up `look` in frame `frame`, see parts() and WALK_FRAMES
pub(crate) fn frame_parts(look: Look, frame: usize) -> Vec<Part> {
    let phase = (1..=WALK_FRAMES).contains(&frame).then(|| (frame - 1) as f64 / WALK_FRAMES as f64);
    parts(look, phase, Pose::of(frame))
}

/// Whether `look` is ever drawn in `frame`, see shown()
pub(crate) fn shown_in(look: Look, frame: usize) -> bool {
    shown(look, frame)
}

/// For a frame between two poses, see TWEENS: the pose it's from and the one it's going to, 0
/// being standing, and how far between
pub(crate) fn tween_ends(frame: usize) -> Option<(usize, usize, f64)> {
    (FIRST_TWEEN..FRAMES).contains(&frame).then(|| {
        let (t, f) = tween_at(frame);
        (t.from, t.to, f)
    })
}

/// The frames `look` goes through from standing into pose `pose`, each with how far along it is,
/// from 0 standing to 1 in the pose
pub(crate) fn path_to(look: Look, pose: usize) -> Vec<(f64, usize)> {
    let mut first = FIRST_TWEEN;
    for t in TWEENS {
        if (t.from, t.to) == (0, pose) || (t.from, t.to) == (pose, 0) {
            if !has_tween(look, t) {
                break;
            }
            let mut out: Vec<(f64, usize)> = (1..=t.steps).map(|k| (k as f64 / (t.steps + 1) as f64, first + k - 1)).collect();
            if t.from == pose {
                out = out.into_iter().map(|(f, frame)| (1.0 - f, frame)).rev().collect();
            }
            out.insert(0, (0.0, 0));
            out.push((1.0, pose));
            return out;
        }
        first += t.steps;
    }
    vec![(0.0, 0), (1.0, pose)]
}

fn model_index(look: Look, frame: usize) -> u32 {
    (Look::ALL.iter().position(|&l| l == look).unwrap() * FRAMES + frame) as u32
}

/// Where the jointed bosses' pieces start among the models animal_models() makes, after every
/// frame of every look, see src\rig.rs
pub const RIG_MODELS_AT: usize = Look::ALL.len() * FRAMES;

/// Where the dragons' jointed pieces start among the models animal_models() makes, after the
/// jointed bosses', see src\dragons.rs
pub const DRAGON_MODELS_AT: usize = RIG_MODELS_AT + crate::rig::MODELS;

/// Where the avians' and gargoyles' jointed pieces start, after the dragons', see src\avians.rs
pub const WINGED_MODELS_AT: usize = DRAGON_MODELS_AT + crate::dragons::MODELS;

/// Where the pieces the monsters' and animals' frames are baked from start, after the avians',
/// see src\baked.rs: drawn only in the model editor, as the frames are baked from them
pub const BAKED_MODELS_AT: usize = WINGED_MODELS_AT + crate::avians::MODELS;

/// Where the krakens' pieces start, after those, see src\bosses\kraken.rs
pub const KRAKEN_MODELS_AT: usize = BAKED_MODELS_AT + crate::baked::MODELS;

/// How many models animal_models() makes: every frame of every look, then the jointed bosses'
/// pieces, then the dragons' pieces and what they breathe, then the avians' and gargoyles' and
/// what they throw, then the pieces the baked frames are made of, then the great serpents'
/// pieces, last, see serpents::part_models(). Other models come after them.
pub const ANIMAL_MODELS: usize = KRAKEN_MODELS_AT + crate::bosses::kraken::MODELS + serpents::PART_MODELS;

/// Frame `frame` of `look` as it was drawn before it was baked from pieces, straight from its
/// shapes, see WALK_FRAMES: empty if it's never drawn in it. What every look not baked from pieces
/// is drawn in, see src\baked.rs.
pub(crate) fn drawn_frame(look: Look, frame: usize) -> Model {
    if !shown(look, frame) {
        return Model { size: [1, 1, 1], voxels: vec![AIR] };
    }
    let phase = (1..=WALK_FRAMES).contains(&frame).then(|| (frame - 1) as f64 / WALK_FRAMES as f64);
    rasterize(look, phase, Pose::of(frame))
}

/// Frame `frame` of `look`, see WALK_FRAMES: empty if it's never drawn in it
#[cfg(test)]
fn frame_model(look: Look, frame: usize) -> Model {
    match crate::baked::rig(look) {
        Some(rig) => crate::assets::copy(&rig.frames[frame]),
        None => drawn_frame(look, frame),
    }
}

/// Every animal's models with their files read in, made ahead while the loading screen's up, see
/// Loaded::load(), for the game to take as it's put together, see made_ahead()
static MADE_AHEAD: std::sync::Mutex<Option<Vec<Model>>> = std::sync::Mutex::new(None);

/// Makes the models the game starts with ahead of it, see MADE_AHEAD
pub fn make_ahead() {
    let models = crate::assets::apply_looks(animal_models());
    *MADE_AHEAD.lock().unwrap_or_else(|e| e.into_inner()) = Some(models);
}

/// The models made ahead, if they were and haven't been taken, or else made now: every frame of
/// every animal with their files read in
pub fn made_ahead() -> Vec<Model> {
    let made = MADE_AHEAD.lock().unwrap_or_else(|e| e.into_inner()).take();
    made.unwrap_or_else(|| crate::assets::apply_looks(animal_models()))
}

/// Every frame of every animal, and of everything else drawn like one: made on every core at once,
/// a frame at a time, so the big ones don't hold up the rest. The monsters and animals baked from
/// pieces are each one job, their pieces made and every frame baked from them, see src\baked.rs,
/// handed out first as they take longest.
pub fn animal_models() -> Vec<Model> {
    let rigs_made = crate::baked::made();
    let rig_jobs = if rigs_made { 0 } else { crate::baked::KINDS.len() };
    let frame_jobs: Vec<usize> = (0..Look::ALL.len() * FRAMES).filter(|&job| !crate::baked::baked(Look::ALL[job / FRAMES])).collect();
    let jobs = rig_jobs + frame_jobs.len();
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
    let (mut made, mut rigs): (Vec<(usize, Model)>, Vec<crate::baked::Rig>) = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let (mut out, mut rigs) = (Vec::new(), Vec::new());
                    loop {
                        let job = next.fetch_add(1, Ordering::Relaxed);
                        if job >= jobs {
                            break (out, rigs);
                        }
                        if job < rig_jobs {
                            rigs.push(crate::baked::build(crate::baked::KINDS[job]));
                        } else {
                            let j = frame_jobs[job - rig_jobs];
                            out.push((j, drawn_frame(Look::ALL[j / FRAMES], j % FRAMES)));
                        }
                    }
                })
            })
            .collect();
        let (mut made, mut rigs) = (Vec::new(), Vec::new());
        for h in handles {
            let (m, r) = h.join().expect("making a model failed");
            made.extend(m);
            rigs.extend(r);
        }
        (made, rigs)
    });
    if !rigs_made {
        rigs.sort_by_key(|r| crate::baked::kind(r.look));
        crate::baked::keep(rigs);
    }
    made.sort_unstable_by_key(|&(job, _)| job);
    let mut made = made.into_iter().peekable();
    let mut models: Vec<Model> = Vec::with_capacity(ANIMAL_MODELS);
    for job in 0..Look::ALL.len() * FRAMES {
        let (look, frame) = (Look::ALL[job / FRAMES], job % FRAMES);
        match crate::baked::rig(look) {
            Some(rig) => models.push(crate::assets::copy(&rig.frames[frame])),
            None => models.push(made.next().filter(|(j, _)| *j == job).expect("a frame for every look").1),
        }
    }
    // The jointed bosses' pieces, the dragons' pieces and what they breathe, the pieces the baked
    // frames are made of, then the great serpents' pieces, which have to be the last of them, see
    // serpents::part_models()
    models.extend(crate::rig::models());
    models.extend(crate::dragons::models());
    models.extend(crate::avians::models());
    models.extend(crate::baked::models());
    models.extend(crate::bosses::kraken::models());
    models.extend(serpents::part_models());
    models
}

/// Words in each model's header, see pack_models(). Has to match MODEL_HEADER in
/// shaders\traverse.comp.
pub const HEADER_WORDS: usize = 4;

/// Models packed for shaders\traverse.comp: HEADER_WORDS words of header per model, then the voxels,
/// four to a word. Each header: where its voxels start, its size, and the size of the box round
/// what's solid in it and where that box is in it, each packed x | y << 10 | z << 20. Only that box's
/// voxels are kept, the air round it left out, as most of every model's air is; and models alike,
/// as frames that come out the same, share theirs. Every model reads back exactly as it was, see
/// packed_voxel().
pub fn pack_models(models: &[&Model]) -> Vec<u32> {
    let pack = |v: [u32; 3]| v[0] | v[1] << 10 | v[2] << 20;
    let mut words = vec![0u32; models.len() * HEADER_WORDS];
    // Where the voxels kept so far start, by a hash of them, and the box they're of
    let mut stored: std::collections::HashMap<u64, Vec<(usize, u32)>> = std::collections::HashMap::new();
    let mut kept: Vec<u8> = Vec::new();
    for (i, model) in models.iter().enumerate() {
        let (from, size) = solid_box(model);
        kept.clear();
        let [sx, sy, _] = model.size.map(|v| v as usize);
        for z in from[2]..from[2] + size[2] {
            for y in from[1]..from[1] + size[1] {
                let row = from[0] as usize + (y as usize + z as usize * sy) * sx;
                kept.extend_from_slice(&model.voxels[row..row + size[0] as usize]);
            }
        }
        let packed = pack_voxels(&kept);
        let header = i * HEADER_WORDS;
        words[header + 1] = pack(model.size);
        words[header + 2] = pack(size);
        words[header + 3] = pack(from);
        let key = voxels_hash(size, &kept);
        let same = stored.get(&key).and_then(|those| those.iter().find(|&&(at, box_size)| box_size == pack(size) && words[at..at + packed.len()] == packed[..]));
        if let Some(&(at, _)) = same {
            words[header] = at as u32;
            continue;
        }
        let at = words.len();
        stored.entry(key).or_default().push((at, pack(size)));
        words[header] = at as u32;
        words.extend(packed);
    }
    words
}

/// Models packed by pack_models(), `packed`, followed by models each packed alone by it, `alone`,
/// as one list of them all, the same as packing all of them at once would make but for sharing
/// voxels: so a big model that comes and goes, as a tree coming down, is packed on a thread of its
/// own and only copied in here, see felling.rs
pub fn splice_packed<A: AsRef<[u32]>>(packed: &[u32], alone: &[A]) -> Vec<u32> {
    // How many headers there are: the first model's voxels start right after them
    let headers = if packed.is_empty() { 0 } else { packed[0] as usize / HEADER_WORDS };
    let shift = alone.len() * HEADER_WORDS;
    let alone: Vec<&[u32]> = alone.iter().map(|w| w.as_ref()).collect();
    let mut out = Vec::with_capacity(packed.len() + shift + alone.iter().map(|w| w.len() - w[0] as usize).sum::<usize>());
    out.extend_from_slice(&packed[..headers * HEADER_WORDS]);
    for k in 0..headers {
        out[k * HEADER_WORDS] += shift as u32;
    }
    out.resize(headers * HEADER_WORDS + shift, 0);
    out.extend_from_slice(&packed[headers * HEADER_WORDS..]);
    for (j, words) in alone.iter().enumerate() {
        let at = out.len();
        let header = (headers + j) * HEADER_WORDS;
        out[header..header + HEADER_WORDS].copy_from_slice(&words[..HEADER_WORDS]);
        out[header] = at as u32;
        out.extend_from_slice(&words[words[0] as usize..]);
    }
    out
}

/// Voxels four to a word
fn pack_voxels(voxels: &[u8]) -> Vec<u32> {
    voxels.chunks(4).map(|chunk| chunk.iter().enumerate().fold(0, |w, (j, &m)| w | (m as u32) << (j * 8))).collect()
}

/// Where the box round what's solid in `model` starts in it, and how big it is, in its voxels:
/// nothing at all for a model that's all air
fn solid_box(model: &Model) -> ([u32; 3], [u32; 3]) {
    let [sx, sy, _] = model.size.map(|v| v as usize);
    let (mut lo, mut hi) = ([u32::MAX; 3], [0u32; 3]);
    for (i, &v) in model.voxels.iter().enumerate() {
        if v != AIR {
            let c = [(i % sx) as u32, (i / sx % sy) as u32, (i / (sx * sy)) as u32];
            for a in 0..3 {
                lo[a] = lo[a].min(c[a]);
                hi[a] = hi[a].max(c[a] + 1);
            }
        }
    }
    if lo[0] == u32::MAX { ([0; 3], [0; 3]) } else { (lo, [0, 1, 2].map(|a| hi[a] - lo[a])) }
}

/// A quick hash of a box's size and voxels, eight at a time, to find those alike
fn voxels_hash(size: [u32; 3], voxels: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ (size[0] as u64 | (size[1] as u64) << 20 | (size[2] as u64) << 40);
    for chunk in voxels.chunks(8) {
        let mut w = [0u8; 8];
        w[..chunk.len()].copy_from_slice(chunk);
        h = (h ^ u64::from_le_bytes(w)).wrapping_mul(0x0000_0100_0000_01b3);
        h ^= h >> 31;
    }
    h
}

/// Voxel `c` of model `model` as shaders\traverse.comp reads it from what pack_models() made of
/// them, see modelVoxel() there: air outside the box round what's solid in it
#[cfg(test)]
pub fn packed_voxel(words: &[u32], model: usize, c: [u32; 3]) -> u8 {
    let unpack = |w: u32| [w & 0x3FF, w >> 10 & 0x3FF, w >> 20 & 0x3FF];
    let header = model * HEADER_WORDS;
    let (start, size, from) = (words[header] as usize, unpack(words[header + 2]), unpack(words[header + 3]));
    let k = [0, 1, 2].map(|a| c[a].wrapping_sub(from[a]));
    if (0..3).any(|a| k[a] >= size[a]) {
        return AIR;
    }
    let i = (k[0] + (k[1] + k[2] * size[1]) * size[0]) as usize;
    (words[start + i / 4] >> ((i % 4) * 8) & 0xFF) as u8
}

// How an instance moves, has to match shaders\traverse.comp
pub const STILL: u32 = 0;
pub const MOVING: u32 = 1;
/// In the player's hand
pub const HELD: u32 = 2;
/// The player's own body, which the eye never sees, only its shadow and reflections, see
/// character::body. Has to match BODY in shaders\traverse.comp.
pub const BODY: u32 = 3;
/// A piece of a townsperson, monster or animal near the camera posed afresh every frame from its
/// pieces, see src\baked.rs, seen only by rays from the camera; and its frame baked whole, its
/// stand-in, which only other rays meet, casting its shadow and seen in reflections. See
/// AnimalInstance::seen(). Have to match SEEN and STAND_IN in shaders\traverse.comp.
pub const SEEN: u32 = 4;
pub const STAND_IN: u32 = 5;
/// Drawn in the player's colours, see AnimalInstance::outfit(). Has to match TINT_OUTFIT in
/// shaders\traverse.comp.
pub const TINT_OUTFIT: u32 = 0x100;

/// What weapons did to an animal, shown on it. Has to match TINT_* in shaders\traverse.comp.
pub const TINT_NONE: u32 = 0;
pub const TINT_BURNING: u32 = 1;
pub const TINT_FROZEN: u32 = 2;
pub const TINT_POISONED: u32 = 3;
pub const TINT_SHOCKED: u32 = 4;
pub const TINT_CHILLED: u32 = 5;
/// Struck a moment ago
pub const TINT_HIT: u32 = 6;
/// Lit by its own light, as something hanging in the sky is: as clear by night as by day, and never
/// faded into the sky with what's on the land as far off, as Blood Moon's moon
pub const TINT_LIT: u32 = 7;

/// One animal as traverse.comp reads it, see the Animal struct there
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
pub struct AnimalInstance {
    position: [f32; 4],
    rotation: [f32; 4],
    /// How big its voxels are in voxels of the main world, then for a serpent how it's stretched
    /// and draped over the ground, see shaped()
    size: f32,
    stretch: u32,
    drape: [u32; 2],
    info: [u32; 4],
}

/// `x` as a half float, as unpackHalf2x16() reads it in the shaders: near enough for how far a
/// model's stretched or draped, nothing smaller than a ten-thousandth, nothing past 65000
fn half(x: f64) -> u32 {
    let bits = (x as f32).to_bits();
    let sign = (bits >> 16) & 0x8000;
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mantissa = bits & 0x7f_ffff;
    if exponent <= 0 {
        return sign;
    }
    if exponent >= 31 {
        return sign | 0x7bff;
    }
    // Rounded to the nearest, which may carry over into the exponent, as it should
    (sign | ((exponent as u32) << 10) | (mantissa >> 13)) + ((mantissa >> 12) & 1)
}

impl AnimalInstance {
    /// Model `model` turned by `rotation`, with the middle of its bottom at `position` and its
    /// voxels `scale` voxels of the main world across. `reach` is how far it reaches from there,
    /// in voxels of the main world, and `kind` STILL, MOVING or HELD.
    pub fn new(model: u32, position: Vector3<f64>, rotation: UnitQuaternion<f64>, scale: f64, reach: f64, kind: u32) -> Self {
        let q = rotation.coords;
        AnimalInstance {
            position: [position.x as f32, position.y as f32, position.z as f32, reach as f32],
            rotation: [q.x as f32, q.y as f32, q.z as f32, q.w as f32],
            size: scale as f32,
            stretch: 0,
            drape: [0, 0],
            info: [model, kind, TINT_NONE, 0],
        }
    }

    /// Drawn `stretch` times as long and as wide as its model, its height as it is, and draped:
    /// its bottom raised `drape` of its model's voxels at four points evenly along it, from its
    /// back end to its front, and in a straight line between them. A serpent's S-curve tighter or
    /// looser for how fast it goes, and its body lying along the ground under it, from one model.
    /// See shaped() in shaders\traverse.comp; its reach has to be wide enough for it.
    pub fn shaped(mut self, stretch: [f64; 2], drape: [f64; 4]) -> Self {
        self.stretch = half(stretch[0] - 1.0) | half(stretch[1] - 1.0) << 16;
        self.drape = [half(drape[0]) | half(drape[1]) << 16, half(drape[2]) | half(drape[3]) << 16];
        self
    }

    pub fn tinted(mut self, tint: u32) -> Self {
        self.info[2] = tint;
        self
    }

    /// With all of it turned towards one color, light and dark as it was, as a skin taken on: red,
    /// green and blue and how far, a byte each, packed little end first; 0 for its own colors. See
    /// tintAnimal() in shaders\traverse.comp.
    pub fn skinned(mut self, skin: u32) -> Self {
        self.info[3] = skin;
        self
    }

    /// In the colours the player chose for themselves: what's leather on it in one and what's cloth
    /// in the other, packed as character::outfit() packs them, light and dark as it was. 0 leaves
    /// it as it is.
    pub fn outfit(mut self, colours: u32) -> Self {
        if colours != 0 {
            self.info[2] |= TINT_OUTFIT;
            self.info[3] = colours;
        }
        self
    }

    /// The same as the stand-in for what's posed live, see STAND_IN: met only by rays that aren't
    /// from the camera
    pub fn stand_in(mut self) -> Self {
        self.info[1] = STAND_IN;
        self
    }

    /// The same as a piece of what's posed live, see SEEN: seen only from the camera, the instance
    /// at `stand_in` standing in for it otherwise, and lit with its own pieces, the `count` from
    /// `first`, in place of that. None of them past 255.
    pub fn seen(mut self, stand_in: usize, first: usize, count: usize) -> Self {
        debug_assert!(stand_in < 256 && first < 256 && count < 256);
        self.info[1] = SEEN | (stand_in as u32 & 0xFF) << 8 | (first as u32 & 0xFF) << 16 | (count as u32 & 0xFF) << 24;
        self
    }

    /// How it's drawn: STILL, MOVING, HELD, BODY, SEEN or STAND_IN
    #[cfg(test)]
    pub fn kind(&self) -> u32 {
        self.info[1] & 0xFF
    }

    /// For a piece of what's posed live, see seen(): where its stand-in is, and its first piece and
    /// how many there are, as the shader reads them
    #[cfg(test)]
    pub fn seen_of(&self) -> Option<(usize, usize, usize)> {
        let i = self.info[1];
        (i & 0xFF == SEEN).then_some(((i >> 8 & 0xFF) as usize, (i >> 16 & 0xFF) as usize, (i >> 24) as usize))
    }

    /// Where it's put, in voxels of the main world
    pub fn sphere_center(&self) -> Vector3<f64> {
        Vector3::new(self.position[0] as f64, self.position[1] as f64, self.position[2] as f64)
    }

    /// Which model it is
    pub fn model(&self) -> u32 {
        self.info[0]
    }

    /// The same drawn with model `model` in its place, alike in size
    pub fn remodeled(mut self, model: u32) -> Self {
        self.info[0] = model;
        self
    }

    /// Where the middle of its bottom is, how it's turned and how big its voxels are, for tests
    #[cfg(test)]
    pub fn placed(&self) -> (Vector3<f64>, UnitQuaternion<f64>, f64) {
        let [x, y, z, _] = self.position.map(f64::from);
        let [i, j, k, w] = self.rotation.map(f64::from);
        (Vector3::new(x, y, z), UnitQuaternion::new_normalize(nalgebra::Quaternion::new(w, i, j, k)), self.size as f64)
    }

    /// Where it is and how far it reaches from there, in voxels of the main world, for tests
    /// seeing whether it can be seen
    #[cfg(test)]
    pub fn sphere(&self) -> (Vector3<f64>, f64) {
        let [x, y, z, reach] = self.position.map(f64::from);
        (Vector3::new(x, y, z), reach)
    }
}

fn facing(heading: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), heading)
}

/// How far to turn in `dt` seconds towards what's `turn` radians round, at most `most` radians a
/// second: slower the nearer it's come round, so it eases to a stop rather than stopping dead
fn eased_turn(turn: f64, most: f64, dt: f64) -> f64 {
    let rate = most * (0.2 + turn.abs() / 0.7).min(1.0);
    turn.clamp(-rate * dt, rate * dt)
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Mood {
    Idle,
    Walk,
    Run,
}

/// Positions are in voxels of the main world
pub struct Animal {
    /// Different for every animal
    pub id: u32,
    pub look: Look,
    /// The middle of its feet
    pub position: Vector3<f64>,
    pub(crate) heading: f64,
    pub(crate) target_heading: f64,
    /// Which way it's drawn facing: easing after `heading`, so it doesn't jerk round
    drawn_heading: f64,
    /// Posed live from its pieces, see src\baked.rs: how much it's striding, 0 standing to 1,
    /// easing into and out of its walk; which way its head's turned from its body, round to its
    /// left and down, as townsfolk and merchants look about and at whoever's near; and how much
    /// it's talking, 0 to 1
    stride: f64,
    gaze: [f64; 2],
    talk: f64,
    pub(crate) speed: f64,
    pub(crate) mood: Mood,
    timer: f64,
    /// How far through its walk cycle, 0-1
    pub(crate) phase: f64,
    /// Seconds it's been about, for it to breathe and sway as it stands
    age: f64,
    pub health: f64,
    /// Its health when it's whole: its kind's, see Look::health(), made more in the realms further
    /// on and for their raid bosses, see Game::toughen()
    pub max_health: f64,
    /// Whether that's been done to it
    pub toughened: bool,
    /// Its level, for a boss or monster, see levels.rs: what it's toughened by and shown with over
    /// its health bar. 0 till it's given one, as it's toughened, unless it came with one, as night
    /// raiders do.
    pub level: u32,
    /// Burning, poisoned, frozen and such, see src\combat.rs
    pub status: Status,
    /// What a boss is up to, see src\bosses.rs
    pub boss: Option<Boss>,
    /// How a dragon flies, fights and holds itself, see src\dragons.rs: it thinks for itself
    /// rather than as bosses do, and only takes from `boss` what it shares with them
    pub dragon: Option<Box<crate::dragons::Dragon>>,
    /// The same for an avian or a gargoyle, see src\avians.rs
    pub winged: Option<Box<crate::avians::Flier>>,
    /// Where in the whole landscape a villager's town square is, in voxels, and how far from it
    /// in meters it strolls
    home: Option<(Vector3<f64>, f64)>,
    rng: u32,
    /// For a serpent, the part of its long body the player's aiming at, from where it is, in
    /// voxels: where blows on it land, see center() and serpents::focus()
    pub focus: Option<Vector3<f64>>,
    /// For a serpent lying on the ground, how its body's tipped to lie along the slope under it, in
    /// radians: its head down, and over onto its right, eased towards the ground it's on; and for
    /// a fish or a shark, how it's pitched and rolled as it swims
    lie: [f64; 2],
    /// For a demi-boss, which of demis::KINDS it is: drawn bigger and tinted, see src\demis.rs
    pub demi: Option<u8>,
    /// And how much longer and wider than its model it's drawn: its S-curve tight and quick going
    /// fast, fleeing or coming on to strike, loose and lazy going slowly, see serpents::stretch()
    stretch: [f64; 2],
    /// And how far above that slope its body's drawn at four points evenly along its model, from
    /// its tail end to its head, in meters: draped over the bumps and hollows under it, rather than
    /// lying across them in a straight line. Eased, as `lie` is.
    drape: [f64; 4],
    /// How a fish or a shark swims, see src\animals\sealife.rs: it steers itself by that rather
    /// than walking about
    pub swim: Option<Box<sealife::Swim>>,
}

/// The furthest above or below the slope it lies along a serpent's body is draped at any point, in
/// meters, see Animal::drape
const DRAPE: f64 = 0.6;

/// How hard a bare-handed hit is
pub const PUNCH: f64 = 35.0;

/// Meters a second it has to be going to be drawn walking, and how fast, as much as it walks, it
/// finishes its step once it's slowed below that
const STEPPING: f64 = 0.15;
const SETTLING: f64 = 0.6;
/// How quickly what's drawn turns after where it faces: about the time it takes, inverted
const TURN_EASE: f64 = 12.0;
/// Turning on the spot faster than this, in radians a second, townsfolk and animals shuffle their
/// feet round, a step a meter for every so many radians, see Animal::step()
const SHUFFLE_TURN: f64 = 0.9;
const SHUFFLE: f64 = 0.35;
/// Posed live, see src\baked.rs: how quickly it eases into and out of its stride, and its head
/// turns, about the time each takes, inverted; and how near the player townsfolk and merchants look
/// at them, and talk to them, in meters
const STRIDE_EASE: f64 = 9.0;
const GAZE_EASE: f64 = 4.5;
const LOOK_AT: f64 = 7.0;
const TALK_AT: f64 = 3.5;
/// The quickest merchants, the princess and her guards turn to face whoever's near, in radians a
/// second, slowing as they come round to them
const KEEPER_TURN: f64 = 2.2;

/// How much of a blow a shieldbearer's shield lets through, and how far round its front it
/// catches them, as the cosine of the angle off straight ahead: about 65 degrees either side
const SHIELD_THROUGH: f64 = 0.15;
const SHIELD_ARC: f64 = 0.42;
/// How much of a blow gets through the sand to a burrower dug in under it
const BURIED_THROUGH: f64 = 0.3;
/// And through a gargoyle's stone, asleep or hardened
const STONE_THROUGH: f64 = 0.12;
/// How far down what swims feels for the bottom of the water it's swimming over, in meters: deep
/// enough for the open sea of the Sunken Reach
const SWIM_DEPTH: f64 = 48.0;

/// Something in the world drawn like an animal that doesn't move, like food lying on the ground.
/// `position` is in voxels of the main world.
pub fn still_instance(look: Look, position: Vector3<f64>, heading: f64, world_voxels_per_meter: u32) -> AnimalInstance {
    let vpm = world_voxels_per_meter as f64;
    AnimalInstance::new(model_index(look, 0), position, facing(heading), vpm / look.resolution(), look.reach() * vpm, STILL)
}

/// Something of `look` walking, `phase` of the way through its stride, 0 to 1, or None standing, or
/// winding up and striking with `strike`, 0 to 1, as a necromancer's skeletons fight, see
/// combat\faith.rs: turned `rotation`, its feet at `position`
pub fn posed_instance(look: Look, position: Vector3<f64>, rotation: UnitQuaternion<f64>, phase: Option<f64>, strike: Option<f64>, world_voxels_per_meter: u32) -> AnimalInstance {
    let vpm = world_voxels_per_meter as f64;
    let frame = match (strike, phase) {
        (Some(s), _) if s < 0.55 => POSE_WINDUP,
        (Some(_), _) => POSE_STRIKE,
        (None, Some(p)) => 1 + (p * WALK_FRAMES as f64).round() as usize % WALK_FRAMES,
        (None, None) => 0,
    };
    AnimalInstance::new(model_index(look, frame), position, rotation, vpm / look.resolution(), look.reach() * vpm, MOVING)
}

/// The same in the player's hand, turned any way, like a grappling hook or wings spread to glide
pub fn held_instance(look: Look, position: Vector3<f64>, rotation: UnitQuaternion<f64>, world_voxels_per_meter: u32) -> AnimalInstance {
    let vpm = world_voxels_per_meter as f64;
    AnimalInstance::new(model_index(look, 0), position, rotation, vpm / look.resolution(), look.reach() * vpm, HELD)
}

/// The same for something moving about, turned any way, like a grappling hook flying out
pub fn moving_instance(look: Look, position: Vector3<f64>, rotation: UnitQuaternion<f64>, world_voxels_per_meter: u32) -> AnimalInstance {
    let vpm = world_voxels_per_meter as f64;
    AnimalInstance::new(model_index(look, 0), position, rotation, vpm / look.resolution(), look.reach() * vpm, MOVING)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

fn random(seed: &mut u32) -> f64 {
    *seed = hash(seed.wrapping_add(0x9e3779b9));
    (*seed >> 8) as f64 / 16777216.0
}

/// Height of the ground an animal standing at height `z` could step onto at x, y: up at most
/// `up` voxels, down at most `down`. None for walls, cliffs, water and outside the world.
fn ground_at(world: &World, water: WaterLevel, x: f64, y: f64, z: f64, up: f64, down: f64) -> Option<f64> {
    ground_wading(world, water, x, y, z, up, down, 0.2)
}

/// The ground under `at`, in voxels, within 60 meters down: its height, if there's any to stand on
pub fn ground_under(world: &World, water: WaterLevel, at: Vector3<f64>) -> Option<f64> {
    ground_at(world, water, at.x, at.y, at.z, 0.0, 60.0 * world.voxels_per_meter as f64)
}

/// The same for something that wades through water up to `wade` meters deep
#[allow(clippy::too_many_arguments)]
fn ground_wading(world: &World, water: WaterLevel, x: f64, y: f64, z: f64, up: f64, down: f64, wade: f64) -> Option<f64> {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let top = (z + up).floor() as i64;
    if !is_passable(world.material_at(xi, yi, top)) {
        return None;
    }
    let ground = ((z - down).floor() as i64..top)
        .rev()
        .find(|&zi| !is_passable(world.material_at(xi, yi, zi)))
        .map(|zi| zi as f64 + 1.0)?;
    // Paddling in the shallows is fine
    (water(x, y) < ground + wade * world.voxels_per_meter as f64).then_some(ground)
}

/// Shallow water near `x`, `y`, within `within` meters, `least` to `most` meters deep: where its
/// bottom is, in voxels. For what lies in wait in it.
fn shallows_near(world: &World, water: WaterLevel, x: f64, y: f64, within: f64, least: f64, most: f64) -> Option<Vector3<f64>> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    const STEP: f64 = 2.0;
    for ring in 0..(within / STEP) as usize {
        let count = (ring * 6).max(1);
        for i in 0..count {
            let a = i as f64 / count as f64 * TAU;
            let (px, py) = (x + a.cos() * ring as f64 * STEP * vpm, y + a.sin() * ring as f64 * STEP * vpm);
            if px < 2.0 || py < 2.0 || px >= sx - 2.0 || py >= sy - 2.0 {
                continue;
            }
            let surface = water(px, py);
            if !surface.is_finite() || surface <= 0.0 {
                continue;
            }
            let top = surface.floor() as i64;
            let Some(bottom) = (top - (most * vpm) as i64 - 1..=top).rev().find(|&z| !is_passable(world.material_at(px as i64, py as i64, z))) else {
                continue;
            };
            let depth = (surface - (bottom as f64 + 1.0)) / vpm;
            if (least..most).contains(&depth) {
                return Some(Vector3::new(px, py, bottom as f64 + 1.0));
            }
        }
    }
    None
}

/// Water at least `least` meters deep near `near`, in voxels, within `within` meters of it: the
/// bottom of it, for a great serpent to rise up out of
pub fn deep_water_near(world: &World, water: WaterLevel, near: Vector3<f64>, within: f64, least: f64) -> Option<Vector3<f64>> {
    shallows_near(world, water, near.x, near.y, within, least, 16.0)
}

/// The highest dry ground in a column with room to stand on it, if it isn't under water: the
/// ground itself, not a branch up in a tree or the top of a wall
fn surface_at(world: &World, water: WaterLevel, x: f64, y: f64) -> Option<f64> {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let room = (1.5 * world.voxels_per_meter as f64) as i64;
    let mut clear = 0;
    for z in (0..world.size[2] as i64).rev() {
        let m = world.material_at(xi, yi, z);
        if is_passable(m) {
            clear += 1;
            continue;
        }
        if clear >= room && is_ground(m) {
            let ground = z as f64 + 1.0;
            return (water(x, y) < ground + 0.2 * world.voxels_per_meter as f64).then_some(ground);
        }
        clear = 0;
    }
    None
}

/// Whether a material is the ground itself rather than something on it, like a tree or a wall
fn is_ground(material: u8) -> bool {
    matches!(material, GRASS | DIRT | SAND | SNOW | STONE | ROCK | GRAVEL | CLAY | CLAY_BANK | MOSS | JUNGLE_GRASS | ICE)
}

impl Animal {
    fn new(look: Look, position: Vector3<f64>, rng: u32) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        let mut rng = rng;
        let heading = random(&mut rng) * TAU;
        Animal {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            look,
            position,
            heading,
            target_heading: heading,
            drawn_heading: heading,
            stride: 0.0,
            gaze: [0.0; 2],
            talk: 0.0,
            speed: 0.0,
            mood: Mood::Idle,
            timer: random(&mut rng) * 3.0,
            phase: 0.0,
            age: random(&mut rng) * 10.0,
            health: look.health(),
            max_health: look.health(),
            toughened: false,
            level: 0,
            status: Status::default(),
            boss: None,
            dragon: None,
            winged: None,
            home: None,
            rng,
            focus: None,
            lie: [0.0; 2],
            demi: None,
            stretch: [1.0; 2],
            drape: [0.0; 4],
            swim: None,
        }
    }

    /// For a villager, where in the whole landscape its town's square is, in voxels, and how far
    /// from it it strolls, in meters
    pub fn home(&self) -> Option<(Vector3<f64>, f64)> {
        self.home
    }

    /// A boss guarding `home`, in voxels. `key` is where its lair is in the whole landscape, to
    /// remember it's been beaten.
    pub fn boss(look: Look, home: Vector3<f64>, key: [i64; 3], seed: u32) -> Self {
        let mut animal = Animal::new(look, home, hash(seed ^ 0xb055));
        animal.boss = Some(Boss::new(look, home, key));
        if look.is_dragon() {
            animal.dragon = Some(Box::new(crate::dragons::Dragon::new(look, home, animal.heading, hash(seed ^ 0xd4a6))));
        }
        if look.is_winged() {
            animal.winged = Some(Box::new(crate::avians::Flier::new(look, home, animal.heading, hash(seed ^ 0xa71a))));
        }
        animal
    }

    pub fn heading(&self) -> f64 {
        self.heading
    }

    /// Which way it's drawn facing, easing after its heading
    pub(crate) fn drawn_heading(&self) -> f64 {
        self.drawn_heading
    }

    /// Radians a second what's drawn of it is turning, to the left
    pub(crate) fn turning(&self) -> f64 {
        let off = (self.heading - self.drawn_heading + PI).rem_euclid(TAU) - PI;
        (off * TURN_EASE * (1.0 + 2.0 * off.abs())).clamp(-6.0, 6.0)
    }

    /// Where point `p` of a jointed boss's pose is in the world, in voxels: `p` in meters at its
    /// build's size from the middle of its feet, +x ahead, see src\rig.rs
    pub(crate) fn rig_point(&self, p: Vector3<f64>, vpm: f64) -> Vector3<f64> {
        let (position, turn) = self.rig_placement(vpm);
        position + turn * p * self.look.scale() * vpm
    }

    /// Where a jointed boss's feet are in the world, in voxels, and which way it's turned
    fn rig_placement(&self, vpm: f64) -> (Vector3<f64>, UnitQuaternion<f64>) {
        let [pitch, on, up] = self.boss.as_ref().map_or([0.0; 3], Boss::sway);
        let ahead = Vector3::new(self.drawn_heading.cos(), self.drawn_heading.sin(), 0.0);
        (self.position + (ahead * on + Vector3::new(0.0, 0.0, up)) * vpm, facing(self.drawn_heading) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), pitch))
    }

    /// For a jointed boss, each of its pieces to draw, into `out`, see rig::instances(): false if
    /// it's not one
    pub fn rig_instances(&self, world_voxels_per_meter: u32, out: &mut Vec<AnimalInstance>) -> bool {
        let Some(rig) = self.boss.as_ref().and_then(|b| b.rig.as_deref()) else {
            return false;
        };
        let vpm = world_voxels_per_meter as f64;
        let (position, turn) = self.rig_placement(vpm);
        crate::rig::instances(self, rig, position, turn, vpm, out);
        true
    }

    /// For looking at it: a villager set off strolling along `heading`, radians round from +x, for
    /// `seconds`, see stroll()
    pub fn stroll_off(&mut self, heading: f64, seconds: f64) {
        self.mood = Mood::Walk;
        self.timer = seconds;
        self.face(heading);
    }

    /// Turned to face `heading`, radians round from +x
    pub fn face(&mut self, heading: f64) {
        (self.heading, self.target_heading, self.drawn_heading) = (heading, heading, heading);
    }

    /// A number picked at random, 0-1, for what it does
    pub(crate) fn roll(&mut self) -> f64 {
        random(&mut self.rng)
    }

    /// The middle of its body, to aim at: lower as a flier dives, or under the sand as a burrower
    /// digs in
    pub fn center(&self, world_voxels_per_meter: u32) -> Vector3<f64> {
        // Whatever part of a dragon the player's striking at, see dragons::Dragon::aim()
        if let Some(dragon) = &self.dragon {
            return dragon.focus();
        }
        if let Some(winged) = &self.winged {
            return winged.focus();
        }
        // A serpent where the player's aiming at it, along the whole of its long body
        if let Some(focus) = self.focus {
            return self.position + focus;
        }
        let lifted = self.boss.as_ref().map_or(0.0, |b| b.sway()[2]);
        self.position + Vector3::new(0.0, 0.0, (self.look.body().0 * self.grown() + lifted) * world_voxels_per_meter as f64)
    }

    /// Where the boulder the Earthbreaker Colossus is tearing up out of the ground is, in voxels,
    /// and how far it's got with it, while it's at that: coming up out of the ground in front of
    /// its fists as it pries it loose, then in its hands, see bosses::Carry
    pub fn carried(&self, vpm: f64) -> Option<(Vector3<f64>, bosses::Carry)> {
        let boss = self.boss.as_ref()?;
        let carry = boss.carrying()?;
        // Between its fists as its jointed pose has them, see src\rig.rs
        let grip = self.rig_point(boss.rig.as_ref()?.sockets.grip, vpm);
        let at = match carry {
            bosses::Carry::Held => grip,
            // From all but buried, its top just under the ground, heaving up out of it as the
            // fists dig in, then more slowly up to where they grip it
            bosses::Carry::Prying(loose) => {
                let low = self.position.z - bosses::TORN_BOULDER * 1.05 * vpm;
                let k = 1.0 - (1.0 - loose) * (1.0 - loose);
                Vector3::new(grip.x, grip.y, low + (grip.z - low) * k)
            }
        };
        Some((at, carry))
    }

    /// How much of a blow from something at `from` gets through to it, 0-1: a shieldbearer facing
    /// it catches most of it on its shield, and a burrower dug in under the sand most of it in that
    pub fn guard(&self, from: Vector3<f64>) -> f64 {
        let Some(boss) = &self.boss else {
            return 1.0;
        };
        // A gargoyle turned to stone shrugs off most of a blow, see src\gargoyles.rs
        if self.winged.as_ref().is_some_and(|w| w.hardened()) {
            return STONE_THROUGH;
        }
        if self.look == Look::Shieldbearer && boss.guarding() {
            let to = (from - self.position).xy();
            let facing = Vector2::new(self.heading.cos(), self.heading.sin());
            if to.norm() > 1e-6 && to.normalize().dot(&facing) > SHIELD_ARC {
                return SHIELD_THROUGH;
            }
        }
        if boss.buried() { BURIED_THROUGH } else { 1.0 }
    }

    /// One of a pack roaming the country of its own, rather than keeping to a den: a monster of the
    /// country, or the dead walking the woods. They wander in to make up the numbers, see
    /// Game::recenter(). Those of dens take their den's place in the landscape as their key.
    pub fn roams(&self) -> bool {
        let wandering = self.boss.as_ref().is_some_and(|b| b.key[2] == -2);
        self.look.is_wild() || wandering && matches!(self.look, Look::Zombie | Look::Ghoul | Look::Bloater | Look::Skeleton | Look::BoneArcher)
    }

    /// How much bigger it's drawn than its kind: a demi-boss's grown, see src\demis.rs
    pub fn grown(&self) -> f64 {
        self.demi.map_or(1.0, |k| crate::demis::KINDS[k as usize % crate::demis::KINDS.len()].grown)
    }

    /// How much health it has left, 0-1
    pub fn vitality(&self) -> f64 {
        self.health / self.max_health
    }

    /// Made `by` times as hard to kill, as hurt as it was
    pub fn toughen(&mut self, by: f64) {
        self.health *= by;
        self.max_health *= by;
    }

    /// Hit bare-handed by the player standing at `from`. Runs off, and returns whether that was
    /// the last hit.
    pub fn hurt(&mut self, from: Vector3<f64>) -> bool {
        self.damage(PUNCH, from)
    }

    /// Takes `amount` of damage from something at `from`, and runs off, or a boss turns on
    /// whoever did it. Returns whether it died.
    pub fn damage(&mut self, amount: f64, from: Vector3<f64>) -> bool {
        self.health = (self.health - amount).max(0.0);
        // Which way the blow came from, in its own frame, for a jointed boss to flinch from it
        let to = from - self.position;
        let (c, s) = (self.drawn_heading.cos(), self.drawn_heading.sin());
        let local = Vector3::new(c * to.x + s * to.y, -s * to.x + c * to.y, 0.0);
        let share = amount / self.max_health.max(1.0);
        match &mut self.boss {
            Some(boss) => {
                boss.provoke();
                boss.struck_by(share, local);
            }
            None => self.flee(from, 4.0),
        }
        self.health <= 0.0
    }

    pub fn flee(&mut self, from: Vector3<f64>, seconds: f64) {
        let away = (self.position - from).xy();
        self.mood = Mood::Run;
        self.timer = seconds;
        self.target_heading = away.y.atan2(away.x);
    }

    /// Petted by the player at `from`: stops to enjoy it, facing them
    pub fn pet(&mut self, from: Vector3<f64>) {
        let towards = (from - self.position).xy();
        self.mood = Mood::Idle;
        self.timer = 4.0;
        self.target_heading = towards.y.atan2(towards.x);
    }

    /// `player` is where the player is, in voxels, so some animals can come over or run off
    pub fn update(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, dt: f64) {
        self.act(world, water, player, dt);
        self.step(dt);
        self.ease_live(world, player, dt);
        // What's drawn turns after where it faces, easing into and out of each turn, so snapping
        // round to face something doesn't jerk it about; quicker the further behind it is, so it
        // keeps up with a boss spinning round
        let off = (self.heading - self.drawn_heading + PI).rem_euclid(TAU) - PI;
        let rate = TURN_EASE * (1.0 + 2.0 * off.abs());
        self.drawn_heading = (self.drawn_heading + off * (1.0 - (-rate * dt).exp())).rem_euclid(TAU);
        // A serpent's long body lying along the slope it's on, rather than sticking out of it, and
        // draped over the bumps and hollows in it; its S-curve as tight as it's quick, slithering
        if let Some(kind) = serpents::kind(self.look) {
            let ease = 1.0 - (-8.0 * dt).exp();
            let lie = self.lie_on(kind, world, water);
            for (now, to) in self.lie.iter_mut().zip(lie) {
                *now += (to - *now) * ease;
            }
            let (frame, position, rotation, _) = self.placement(world.voxels_per_meter);
            let (walk, run) = (self.look.walk_speed(), self.look.run_speed());
            let stretch = if frame <= WALK_FRAMES { serpents::stretch((self.speed - walk) / (run - walk).max(0.1)) } else { [1.0; 2] };
            for (now, to) in self.stretch.iter_mut().zip(stretch) {
                *now += (to - *now) * (1.0 - (-5.0 * dt).exp());
            }
            let drape = self.drape_on(kind, world, water, frame, position, rotation);
            for (now, to) in self.drape.iter_mut().zip(drape) {
                *now += (to - *now) * ease;
            }
        }
        // A great serpent's long body following its head
        if let Some(mut coils) = self.boss.as_mut().and_then(|b| b.coils.take()) {
            let [_, on, up] = self.boss.as_ref().map_or([0.0; 3], Boss::sway);
            coils.update(self.position, self.drawn_heading, on, up, world, water, dt);
            if let Some(boss) = &mut self.boss {
                boss.coils = Some(coils);
            }
        }
        // A kraken's arms reaching for what it's doing, see src\bosses\kraken.rs
        if self.look.is_kraken() {
            crate::bosses::kraken::animate(self, world, water, player, dt);
        }
    }

    /// A kraken's or a squid's body and arms, see bosses::kraken::Body
    pub fn kraken(&self) -> Option<&crate::bosses::kraken::Body> {
        self.boss.as_ref().and_then(|b| b.kraken.as_deref())
    }

    /// A great serpent's long body, see serpents::Coils
    pub fn coils(&self) -> Option<&serpents::Coils> {
        self.boss.as_ref().and_then(|b| b.coils.as_deref())
    }

    /// How serpent `kind` is tipped to lie along the ground where it is, see `lie`: from how high
    /// the ground is under its head and tail ends and either side of its middle. Flat gliding over
    /// it or swimming, and where there's no ground to lie on.
    fn lie_on(&self, kind: &serpents::Kind, world: &World, water: WaterLevel) -> [f64; 2] {
        let vpm = world.voxels_per_meter as f64;
        let (x, y, z) = (self.position.x, self.position.y, self.position.z);
        if kind.aloft || water(x, y) > z - 0.05 * vpm {
            return [0.0; 2];
        }
        let (c, s) = (self.drawn_heading.cos(), self.drawn_heading.sin());
        let height = |along: f64, out: f64| ground_at(world, water, x + (c * along - s * out) * vpm, y + (s * along + c * out) * vpm, z, 1.2 * vpm, 1.5 * vpm);
        let slope = |a: Option<f64>, b: Option<f64>, apart: f64| a.zip(b).map_or(0.0, |(a, b)| ((a - b) / (apart * vpm)).atan());
        let half = 0.32 * kind.length;
        let down = -slope(height(half, 0.0), height(-half, 0.0), 2.0 * half);
        let over = slope(height(0.0, 0.3), height(0.0, -0.3), 0.6);
        [down.clamp(-0.5, 0.5), over.clamp(-0.4, 0.4)]
    }

    /// How far above or below the slope it lies along serpent `kind`'s drawn in `frame`, placed at
    /// `position` turned by `rotation`, has the ground at each of four points evenly along its
    /// model, see `drape`, in meters: up over a bump, down into a hollow. Flat gliding over it,
    /// swimming or dying, and where there's no ground to lie on.
    fn drape_on(&self, kind: &serpents::Kind, world: &World, water: WaterLevel, frame: usize, position: Vector3<f64>, rotation: UnitQuaternion<f64>) -> [f64; 4] {
        let vpm = world.voxels_per_meter as f64;
        let (x, y, z) = (self.position.x, self.position.y, self.position.z);
        let Some(posed) = serpents::posed(self.look, frame) else {
            return [0.0; 4];
        };
        if kind.aloft || self.health <= 0.0 || water(x, y) > z - 0.05 * vpm {
            return [0.0; 4];
        }
        let half = posed.half * self.stretch[0];
        std::array::from_fn(|k| {
            let at = position + rotation * Vector3::new(-half + k as f64 * 2.0 * half / 3.0, 0.0, 0.0) * vpm;
            ground_at(world, water, at.x, at.y, at.z, vpm, vpm).map_or(0.0, |g| ((g - at.z) / vpm).clamp(-DRAPE, DRAPE))
        })
    }

    /// Where point `p` of an ordinary serpent's model drawn in a frame `half` meters long either
    /// side of its middle is drawn, stretched and draped as it is now, see `stretch` and `drape`:
    /// as AnimalInstance::shaped() has the shader draw it, in meters of its model
    fn shaped(&self, p: Vector3<f64>, half: f64) -> Vector3<f64> {
        let f = ((p.x + half) / (2.0 * half).max(1e-6) * 3.0).clamp(0.0, 3.0);
        let k = (f as usize).min(2);
        let up = self.drape[k] + (self.drape[k + 1] - self.drape[k]) * (f - k as f64);
        Vector3::new(p.x * self.stretch[0], p.y * self.stretch[1], p.z + up)
    }

    /// An ordinary serpent's tongue flicking out of its mouth, to draw beside it, see
    /// serpents::flick(): None while it's in, or its jaws are open, or it's dying, frozen or gone
    /// under the ground
    pub fn tongue_instance(&self, world_voxels_per_meter: u32) -> Option<AnimalInstance> {
        let boss = self.boss.as_ref()?;
        if self.health <= 0.0 || self.status.pace() <= 0.0 || boss.buried() {
            return None;
        }
        // Tasting the air for whatever it's watching or waiting for, quicker than as it goes about
        let eager = boss.engaged || self.speed < 0.3;
        let out = serpents::flick(self.age, self.id, eager);
        if out < 0.05 {
            return None;
        }
        let (frame, position, rotation, scale) = self.placement(world_voxels_per_meter);
        let posed = serpents::posed(self.look, frame)?;
        if posed.gape > 0.3 {
            return None;
        }
        let (root, ahead, side, up) = posed.tongue;
        // Slid out of its mouth along the way its head points, from all of it inside; its model's
        // made a little up off its bottom, see serpents::tongue_shapes()
        let tip = Vector3::from(root) + Vector3::from(ahead) * (out - 1.0) * serpents::TONGUE - Vector3::from(up) * serpents::TONGUE_LIFT;
        // Out of its mouth where its head's drawn, stretched and draped
        let tip = self.shaped(tip, posed.half);
        let turn = UnitQuaternion::from_rotation_matrix(&nalgebra::Rotation3::from_basis_unchecked(&[Vector3::from(ahead), Vector3::from(side), Vector3::from(up)]));
        let size = scale * self.look.resolution() * self.look.scale();
        let at = position + rotation * tip * size;
        let look = Look::SerpentTongue;
        Some(AnimalInstance::new(model_index(look, 0), at, rotation * turn, size / look.resolution(), look.reach() * size, MOVING).tinted(self.status.tint()))
    }

    /// On through its walk cycle as it goes. Slowing to a stop, it finishes the step it's taking,
    /// so it comes to stand with its feet together, halfway through the cycle or at its end,
    /// rather than snapping to it; and starting off again, it starts from there.
    fn step(&mut self, dt: f64) {
        // Frozen solid or knocked down, it stays as it is
        if self.status.pace() <= 0.0 {
            return;
        }
        // A fish's tail beats as fast as it's swimming, see sealife::act()
        if self.swim.is_some() {
            return;
        }
        // A serpent's S-curve goes on just as far as it goes, so its body slides along the curve
        // and never over the ground; stopping, it lies where it stopped, still in the curve
        if self.look.is_serpent() {
            self.phase = (self.phase + self.speed * dt / (self.look.stride() * self.stretch[0])).fract();
            return;
        }
        let walking = self.speed > STEPPING;
        // Turning round on the spot, townsfolk and animals shuffle their feet round with them
        let shuffling = !walking && self.shuffles() && self.turning().abs() > SHUFFLE_TURN;
        let pace = if walking {
            self.speed
        } else if shuffling {
            (self.turning().abs() * SHUFFLE).max(SETTLING * self.look.walk_speed())
        } else if (self.phase * 2.0).fract() != 0.0 {
            self.speed.max(SETTLING * self.look.walk_speed())
        } else {
            return;
        };
        let next = self.phase + pace * dt / self.look.stride();
        self.phase = if !walking && !shuffling && (next * 2.0).floor() > (self.phase * 2.0).floor() { ((next * 2.0).floor() * 0.5).fract() } else { next.fract() };
    }

    /// Whether it shuffles its feet round as it turns on the spot: what walks and isn't a boss
    fn shuffles(&self) -> bool {
        self.boss.is_none() && self.look.walks() && crate::baked::baked(self.look)
    }

    /// Eases what it's drawn doing, posed live from its pieces, see src\baked.rs: into and out of
    /// its stride, and for townsfolk and merchants their head turning to look about, or at the
    /// player near them, `player` their feet, and talking once they're close
    fn ease_live(&mut self, world: &World, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let stepping = self.speed > STEPPING || (self.phase * 2.0).fract() != 0.0;
        // Shuffling round or finishing a step, it's a short one
        let stride = if self.speed > STEPPING { 1.0 } else if stepping { 0.55 } else { 0.0 };
        self.stride += (stride - self.stride) * (1.0 - (-STRIDE_EASE * dt).exp());
        let look = self.look;
        if !(look.is_villager() || look.is_merchant() || look.is_royal()) {
            return;
        }
        // Looking about now and then as it stands, straight ahead as it walks
        let own = self.id as f64 * 0.73;
        let about = (self.age * 0.23 + own).sin().powi(3) * 0.55 * (1.0 - self.stride);
        let mut want = [about, 0.08 * (self.age * 0.17 + own).sin()];
        let mut talking = 0.0;
        if let Some(player) = player.filter(|_| self.health > 0.0) {
            let to = player - self.position;
            let flat = to.xy().norm() / vpm;
            let off = (to.y.atan2(to.x) - self.drawn_heading + PI).rem_euclid(TAU) - PI;
            if flat < LOOK_AT && off.abs() < 1.9 {
                // At their eyes from its own, which are about as high over their feet
                want = [off, -(to.z / vpm).atan2(flat.max(0.4)).clamp(-0.5, 0.5)];
                if flat < TALK_AT {
                    talking = 1.0;
                }
            }
        }
        // Not so far round walking past, only a glance
        let most = 1.1 - 0.5 * self.stride;
        let ease = 1.0 - (-GAZE_EASE * dt).exp();
        for (now, to) in self.gaze.iter_mut().zip(want) {
            *now += (to.clamp(-most, most) - *now) * ease;
        }
        self.talk += (talking - self.talk) * (1.0 - (-2.5 * dt).exp());
    }

    /// What it's up to, to pose it live from its pieces, see baked::Rig::live()
    pub fn doing(&self) -> crate::baked::Doing {
        let (phase, stride) = match self.beating() {
            Some(phase) => (phase, 1.0),
            None if self.look.walks() => (self.phase, self.stride),
            None => (0.0, 0.0),
        };
        crate::baked::Doing { phase, stride, pose: self.boss.as_ref().and_then(Boss::pose_path), gaze: self.gaze, talk: self.talk, age: self.age, seed: self.id }
    }

    /// For a look baked from pieces near the camera, each of its pieces posed live as it is now,
    /// into `out`, see baked::Rig::live(): false if it isn't one that can be
    pub fn live_instances(&self, world_voxels_per_meter: u32, out: &mut Vec<AnimalInstance>) -> bool {
        if self.dragon.is_some() || self.winged.is_some() || self.swim.is_some() || !crate::baked::posable(self.look) {
            return false;
        }
        let Some(rig) = crate::baked::rig(self.look) else { return false };
        let (_, position, rotation, scale) = self.placement(world_voxels_per_meter);
        let poses = rig.live(&self.doing());
        if poses.is_empty() {
            return false;
        }
        let start = out.len();
        rig.draw_live(&poses, position, rotation, scale * self.look.resolution(), out);
        let skin = match self.demi {
            Some(k) => crate::demis::KINDS[k as usize % crate::demis::KINDS.len()].skin,
            None => 0,
        };
        let tint = self.status.tint();
        for piece in &mut out[start..] {
            *piece = piece.tinted(tint).skinned(skin);
        }
        out.len() > start
    }

    fn act(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let look = self.look;
        self.age += dt;
        // A fish or a shark swims by its own lights, see src\animals\sealife.rs
        if let Some(mut swim) = self.swim.take() {
            sealife::act(self, &mut swim, world, water, player, dt);
            self.swim = Some(swim);
            return;
        }

        // Bosses shrug off most of what would knock an animal about, monsters don't
        if look.is_boss() {
            let most = 1.2 * vpm;
            if self.status.push.norm() > most {
                self.status.push = self.status.push.normalize() * most;
            }
            self.status.limit_for_boss();
        }
        // A dragon flies, walks and fights by its own lights
        if let Some(mut dragon) = self.dragon.take() {
            crate::dragons::act(self, &mut dragon, world, water, player, dt);
            self.dragon = Some(dragon);
            return;
        }
        // And so does what flies on wings of feathers or of stone, see src\avians.rs
        if let Some(mut winged) = self.winged.take() {
            crate::avians::act(self, &mut winged, world, water, player, dt);
            self.winged = Some(winged);
            return;
        }
        if self.boss.is_some() {
            bosses::think(self, world, player, dt);
        } else {
            // With the ground gone from under it, blown away, it falls to what's left
            let below = (self.position.z - 1.0).floor() as i64;
            if is_passable(world.material_at(self.position.x as i64, self.position.y as i64, below)) {
                if let Some(ground) = ground_at(world, water, self.position.x, self.position.y, self.position.z, 0.0, 6.0 * vpm) {
                    self.position.z = ground.max(self.position.z - 9.0 * vpm * dt);
                }
            }
        }

        // A merchant stays by its wares, turning to whoever comes near
        if look.is_merchant() {
            self.speed = 0.0;
            self.mood = Mood::Idle;
            if let Some(player) = player {
                let to = (player - self.position).xy();
                if to.norm() < 8.0 * vpm {
                    let turn = (to.y.atan2(to.x) - self.heading + PI).rem_euclid(TAU) - PI;
                    self.heading += eased_turn(turn, KEEPER_TURN, dt);
                }
            }
            return;
        }
        // The princess stays by her throne, turning to whoever comes into the throne room, and her
        // guards stand at their posts, turning to whoever comes right up to them and back after
        if look.is_royal() {
            self.speed = 0.0;
            self.mood = Mood::Idle;
            let near = if look == Look::Princess { 14.0 } else { 4.0 };
            let to = player.map(|p| (p - self.position).xy()).filter(|to| to.norm() < near * vpm);
            if to.is_some() || look == Look::RoyalGuard {
                let want = to.map_or(self.target_heading, |to| to.y.atan2(to.x));
                let turn = (want - self.heading + PI).rem_euclid(TAU) - PI;
                self.heading += eased_turn(turn, KEEPER_TURN * 1.3, dt);
            }
            return;
        }
        // Villagers stroll about their town, stopping to face whoever comes up to them
        if look.is_villager() {
            self.stroll(world, player);
        }
        self.timer -= dt;
        if self.boss.is_none() && self.timer <= 0.0 && !look.is_villager() {
            let r = random(&mut self.rng);
            let (mood, time) = if r < 0.4 {
                (Mood::Idle, 2.0 + 4.0 * random(&mut self.rng))
            } else if r < 0.9 {
                (Mood::Walk, 3.0 + 5.0 * random(&mut self.rng))
            } else {
                (Mood::Run, 1.5 + 2.0 * random(&mut self.rng))
            };
            self.mood = mood;
            self.timer = time;
            self.target_heading = self.heading + (random(&mut self.rng) - 0.5) * 3.0;

            // Dogs like to come over to the player now and then
            if let Some(player) = player
                && look == Look::Dog
                && random(&mut self.rng) < 0.5
            {
                let offset = (player - self.position).xy() / vpm;
                if offset.norm() > 3.0 && offset.norm() < 25.0 {
                    self.mood = Mood::Walk;
                    self.target_heading = offset.y.atan2(offset.x);
                }
            }
        }

        // Squirrels dart away when the player gets close
        if let Some(player) = player
            && look == Look::Squirrel
        {
            let offset = (self.position - player).xy() / vpm;
            if offset.norm() < 4.0 && self.mood != Mood::Run {
                self.mood = Mood::Run;
                self.timer = 1.5;
                self.target_heading = offset.y.atan2(offset.x);
            }
        }

        // Thrown back by a blow, sliding to a stop
        let push = self.status.push * dt;
        if push.norm() > 1e-3 {
            let (x, y) = (self.position.x + push.x, self.position.y + push.y);
            let reach = look.climb() * vpm * 2.0;
            if let Some(z) = ground_wading(world, water, x, y, self.position.z, reach, reach * 2.0, look.wade()) {
                self.position = Vector3::new(x, y, z);
            } else {
                self.status.push = Vector3::zeros();
            }
        }
        self.status.push *= (-6.0 * dt).exp();

        // Frozen solid or knocked off its feet it can't move, slowed it crawls
        let pace = self.status.pace();
        if pace <= 0.0 {
            self.speed = 0.0;
            return;
        }

        // Turn towards where it's headed, the short way round: townsfolk and animals slowing as they
        // come round, rather than stopping dead, as monsters and bosses keep their aim
        let turn = (self.target_heading - self.heading + PI).rem_euclid(TAU) - PI;
        self.heading += if self.boss.is_none() { eased_turn(turn, 3.0 * pace, dt) } else { turn.clamp(-3.0 * dt * pace, 3.0 * dt * pace) };

        // A burrower tunnelling along under the sand, or a beast charging, goes faster than it runs
        let haste = self.boss.as_ref().map_or(1.0, Boss::haste);
        let wish = pace
            * haste
            * match self.mood {
                Mood::Idle => 0.0,
                Mood::Walk => look.walk_speed(),
                Mood::Run => look.run_speed(),
            };
        self.speed += (wish - self.speed) * (1.0 - (-4.0 * dt).exp());

        if self.speed > 0.01 {
            let step = Vector2::new(self.heading.cos(), self.heading.sin()) * self.speed * vpm * dt;
            // Feel ahead a little, so it turns before its nose is in a wall
            let ahead = Vector2::new(self.heading.cos(), self.heading.sin()) * 0.3 * vpm;
            let (x, y) = (self.position.x + step.x, self.position.y + step.y);
            let climb = look.climb() * vpm;
            // Over water, what flies looks further down for the bottom of it, and what swims, out
            // over the deep water of a lake or the sea, much further, to go on over it
            let z = self.position.z;
            let down = |x: f64, y: f64| {
                if look.swims() && water(x, y) > z - climb {
                    SWIM_DEPTH * vpm
                } else if look.flies() {
                    8.0 * vpm
                } else {
                    climb * 2.0
                }
            };
            let ground = ground_wading(world, water, x, y, z, climb, down(x, y), look.wade());
            let (ax, ay) = (x + ahead.x, y + ahead.y);
            let clear = ground_wading(world, water, ax, ay, z, climb, down(ax, ay), look.wade()).is_some();
            // What flies goes over water, not down to the bottom of it, and what swims along the top
            // of it, its back just out of it
            let ground = if look.flies() {
                ground.map(|z| z.max(water(x, y)))
            } else if look.swims() {
                ground.map(|z| z.max(water(x, y) - serpents::draft(look) * vpm))
            } else {
                ground
            };
            // A kraken keeps to water deep enough to hide it, never coming out of it
            let ground = if look.is_kraken() { ground.filter(|_| water(x, y) - (ground_at_floor(world, x, y, z) as f64) > crate::bosses::kraken::SHALLOW * 0.8 * vpm) } else { ground };
            match ground {
                Some(z) if clear => {
                    self.position.x = x;
                    self.position.y = y;
                    // Settle onto the new height quickly but not instantly
                    self.position.z += (z - self.position.z) * (1.0 - (-20.0 * dt).exp());
                }
                _ => {
                    // Something's in the way: turn around and try elsewhere. A boss charging
                    // into it is stopped dead, see bosses.rs.
                    if self.boss.is_none() {
                        self.target_heading = self.heading + PI + (random(&mut self.rng) - 0.5) * 1.5;
                    }
                    self.speed = 0.0;
                }
            }
        }
        // Floating up to the top of the water it's in, even lying still
        if look.swims() {
            let top = water(self.position.x, self.position.y) - serpents::draft(look) * vpm;
            if top.is_finite() && self.position.z < top {
                self.position.z += (top - self.position.z) * (1.0 - (-4.0 * dt).exp());
            }
        }
    }

    /// What a villager does next: stands about or strolls slowly, heading back towards its town's
    /// square once it's wandered as far as it goes, and stopping to face the player when they come
    /// up to it
    fn stroll(&mut self, world: &World, player: Option<Vector3<f64>>) {
        let vpm = world.voxels_per_meter as f64;
        if let Some(player) = player {
            let to = (player - self.position).xy();
            if to.norm() < 3.5 * vpm {
                self.mood = Mood::Idle;
                self.timer = self.timer.max(1.5);
                self.target_heading = to.y.atan2(to.x);
                return;
            }
        }
        let Some((home, roam)) = self.home else {
            return;
        };
        let origin = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
        let back = (home - origin - self.position).xy() / vpm;
        if self.timer <= 0.0 {
            let r = random(&mut self.rng);
            (self.mood, self.timer) = if r < 0.45 { (Mood::Idle, 2.5 + 5.0 * random(&mut self.rng)) } else { (Mood::Walk, 3.0 + 5.0 * random(&mut self.rng)) };
            let turn = random(&mut self.rng) - 0.5;
            self.target_heading = if back.norm() > roam * 0.6 { back.y.atan2(back.x) + turn * 1.2 } else { self.heading + turn * 2.5 };
        } else if self.mood == Mood::Walk && back.norm() > roam {
            self.target_heading = back.y.atan2(back.x);
        }
    }

    pub fn instance(&self, world_voxels_per_meter: u32) -> AnimalInstance {
        // A dragon's drawn in pieces, see dragons::instances(): this is the first of them
        if let Some(dragon) = &self.dragon {
            let mut pieces = Vec::new();
            crate::dragons::instances(self, dragon, world_voxels_per_meter as f64, &mut pieces);
            if let Some(&first) = pieces.first() {
                return first;
            }
        }
        // And a jointed boss's, see rig::instances()
        let mut pieces = Vec::new();
        if self.rig_instances(world_voxels_per_meter, &mut pieces)
            && let Some(&first) = pieces.first()
        {
            return first;
        }
        // And an avian's or a gargoyle's, see avians::instances()
        if let Some(winged) = &self.winged {
            let mut pieces = Vec::new();
            crate::avians::instances(self, winged, world_voxels_per_meter as f64, &mut pieces);
            if let Some(&first) = pieces.first() {
                return first;
            }
        }
        let (frame, position, rotation, scale) = self.placement(world_voxels_per_meter);
        let [_, on, up] = self.boss.as_ref().map_or([0.0; 3], Boss::sway);
        let vpm = world_voxels_per_meter as f64;
        let mut radius = (self.look.reach() * self.grown() + on.abs() + up.abs()) * vpm;
        // An ordinary serpent's S-curve stretched or squeezed for how fast it goes, and its body
        // draped over the ground under it, see AnimalInstance::shaped()
        let shaped = serpents::kind(self.look).is_some() && (self.stretch != [1.0; 2] || self.drape != [0.0; 4]);
        if shaped {
            let most = self.drape.iter().fold(0.0f64, |m, d| m.max(d.abs()));
            radius = radius * self.stretch[0].max(self.stretch[1]).max(1.0) + most * vpm;
        }
        // A school of reef fish in its own colours, see sealife::Swim::skin
        let skin = self.swim.as_ref().map_or(0, |s| s.skin);
        let instance = AnimalInstance::new(model_index(self.look, frame), position, rotation, scale, radius, MOVING).tinted(self.status.tint());
        if shaped {
            return instance.shaped(self.stretch, self.drape.map(|d| d * vpm / scale));
        }
        // A demi-boss in the skin it's taken on down in the dark
        match self.demi {
            Some(k) => instance.skinned(crate::demis::KINDS[k as usize % crate::demis::KINDS.len()].skin),
            None => instance.skinned(skin),
        }
    }

    /// For an ordinary serpent, points along its spine from its snout to the tip of its tail as it's
    /// drawn, in voxels: where it can be hit, see serpents::focus()
    pub fn spine(&self, world_voxels_per_meter: u32) -> Option<Vec<Vector3<f64>>> {
        serpents::kind(self.look)?;
        let (frame, position, rotation, _) = self.placement(world_voxels_per_meter);
        let posed = serpents::posed(self.look, frame)?;
        let vpm = world_voxels_per_meter as f64;
        Some(posed.spine.iter().map(|&p| position + rotation * self.shaped(Vector3::from(p), posed.half) * vpm).collect())
    }

    /// Which of its frames it's drawn in, where, turned how and how big
    /// Wings never stop beating, and a wraith's rags never stop stirring, however it's moving: how
    /// far through its walk's frames they are, if so
    fn beating(&self) -> Option<f64> {
        match self.look.build() {
            Some(Build::Bat) => Some((self.age * 3.4 + self.id as f64 * 0.37).fract()),
            Some(Build::Wraith) if self.speed <= 0.15 => Some((self.age * 0.45 + self.id as f64 * 0.37).fract()),
            _ => None,
        }
    }

    fn placement(&self, world_voxels_per_meter: u32) -> (usize, Vector3<f64>, UnitQuaternion<f64>, f64) {
        let beating = self.beating();
        // The frame of its walk nearest where it is in it, and on through it till it's finished
        // the step it was taking
        let walk = |phase: f64| 1 + (phase * WALK_FRAMES as f64).round() as usize % WALK_FRAMES;
        let stepping = self.speed > STEPPING || (self.phase * 2.0).fract() != 0.0;
        let walking = match beating {
            Some(phase) => walk(phase),
            None if stepping && self.look.walks() => walk(self.phase),
            None => 0,
        };
        let frame = self.boss.as_ref().and_then(Boss::pose).unwrap_or(walking);
        let mut scale = world_voxels_per_meter as f64 / self.look.resolution() * self.grown();
        let mut heading = self.drawn_heading;
        // Bosses and monsters breathe, swelling a little and settling, and sway as they stand,
        // less the faster they go, so they're never still as statues between frames; a jointed boss
        // breathes with its own joints
        // and so do the townsfolk, merchants, the princess and her guards, shifting their weight
        let folk = self.look.is_villager() || self.look.is_merchant() || self.look.is_royal();
        if (self.look.build().is_some() || folk) && !crate::rig::rigged(self.look) {
            let offset = self.id as f64 * 0.7;
            scale *= 1.0 + 0.012 * (self.age * 2.1 + offset).sin();
            let still = (1.0 - self.speed / 0.4).clamp(0.0, 1.0);
            heading += 0.035 * still * (self.age * 1.3 + offset).sin();
        }
        // Its whole body leaning into a blow and settling after, tipped about its feet, see
        // Boss::sway()
        let vpm = world_voxels_per_meter as f64;
        let [pitch, on, up] = self.boss.as_ref().map_or([0.0; 3], Boss::sway);
        let ahead = Vector3::new(self.drawn_heading.cos(), self.drawn_heading.sin(), 0.0);
        // A serpent rolled over on its back as it dies, lifted so it's still on the ground
        let roll = self.boss.as_ref().map_or(0.0, Boss::roll);
        let lift = serpents::kind(self.look).map_or(0.0, |k| 2.0 * k.girth * (1.0 - roll.cos()) * 0.5);
        let position = self.position + (ahead * on + Vector3::new(0.0, 0.0, up + lift)) * vpm;
        let [down, over] = self.lie;
        let rotation = facing(heading) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), pitch + down) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), over + roll);
        (frame, position, rotation, scale)
    }
}

/// A boss for the lair at `near`, in voxels with its z on the ground there: on open ground as near
/// it as there is, at about that height with room above, so not on a roof or in a wall. None if
/// there's nowhere within 25 meters.
pub fn spawn_boss(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, key: [i64; 3], seed: u32) -> Option<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    for ring in 0..26 {
        let count = (ring * 6).max(1);
        for i in 0..count {
            let a = i as f64 / count as f64 * TAU;
            let (x, y) = (near.x + a.cos() * ring as f64 * vpm, near.y + a.sin() * ring as f64 * vpm);
            if x < 2.0 || y < 2.0 || x >= sx - 2.0 || y >= sy - 2.0 {
                continue;
            }
            if let Some(z) = ground_at(world, water, x, y, near.z, 4.0 * vpm, 4.0 * vpm) {
                return Some(Animal::boss(look, Vector3::new(x, y, z), key, seed));
            }
        }
    }
    None
}

/// A merchant on open ground near `near`, in voxels, within `within` meters, if there's anywhere
pub fn spawn_merchant(world: &World, water: WaterLevel, near: Vector3<f64>, within: f64, seed: u32) -> Option<Animal> {
    spawn_keeper(world, water, Look::Merchant, near, within, seed)
}

/// A merchant standing at `at`, in voxels, on the ground there, facing `heading`, of `look`: the
/// travelling one or a specialist, see src\merchants.rs
pub fn keeper_at(look: Look, at: Vector3<f64>, heading: f64, seed: u32) -> Animal {
    let mut keeper = Animal::new(look, at, hash(seed ^ 0x3e7d));
    (keeper.heading, keeper.target_heading, keeper.drawn_heading) = (heading, heading, heading);
    keeper
}

/// The same of `look`, a specialist, see src\merchants.rs
pub fn spawn_keeper(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, within: f64, seed: u32) -> Option<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut rng = hash(seed ^ 0x3e7c);
    for _ in 0..40 {
        let a = random(&mut rng) * TAU;
        let r = random(&mut rng).sqrt() * within * vpm;
        let (x, y) = (near.x + a.cos() * r, near.y + a.sin() * r);
        if x < 2.0 || y < 2.0 || x >= sx - 2.0 || y >= sy - 2.0 {
            continue;
        }
        if let Some(z) = ground_at(world, water, x, y, near.z, 3.0 * vpm, 4.0 * vpm) {
            return Some(Animal::new(look, Vector3::new(x, y, z), hash(rng)));
        }
    }
    None
}

/// The princess or one of her guards, `look`, on the floor at `near`, in voxels, facing `heading`:
/// None if there's no floor there
pub fn spawn_royal(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, heading: f64, seed: u32) -> Option<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    if near.x < 2.0 || near.y < 2.0 || near.x >= sx - 2.0 || near.y >= sy - 2.0 {
        return None;
    }
    let z = ground_at(world, water, near.x, near.y, near.z, 2.0 * vpm, 3.0 * vpm)?;
    let mut royal = Animal::new(look, Vector3::new(near.x, near.y, z), hash(seed ^ 0x9071));
    (royal.heading, royal.target_heading) = (heading, heading);
    Some(royal)
}

/// A villager of `look` on open ground near `near`, in voxels, within `within` meters, if there's
/// anywhere: one of the folk of the town whose square is at `home`, in voxels of the whole
/// landscape, strolling as far as the meters it comes with from there
pub fn spawn_villager(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, within: f64, home: (Vector3<f64>, f64), seed: u32) -> Option<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut rng = hash(seed ^ 0x7111);
    for _ in 0..40 {
        let a = random(&mut rng) * TAU;
        let r = random(&mut rng).sqrt() * within * vpm;
        let (x, y) = (near.x + a.cos() * r, near.y + a.sin() * r);
        if x < 2.0 || y < 2.0 || x >= sx - 2.0 || y >= sy - 2.0 {
            continue;
        }
        // On the ground about as high as the square, with nothing over it, not up on a roof
        if let Some(z) = ground_at(world, water, x, y, near.z, 3.0 * vpm, 4.0 * vpm)
            && (z - near.z).abs() < 1.5 * vpm
            && is_passable(world.material_at(x as i64, y as i64, (z + 2.0 * vpm) as i64))
        {
            let mut villager = Animal::new(look, Vector3::new(x, y, z), hash(rng));
            villager.home = Some(home);
            return Some(villager);
        }
    }
    None
}

/// A monster of `look` on the ground near `near`, in voxels, about that high with room above it,
/// within `within` meters: None if there's nowhere
pub fn spawn_monster(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, within: f64, seed: u32) -> Option<Animal> {
    spawn_spaced(world, water, look, near, within, &[], seed)
}

/// The same, at least SPACING meters from each of `others`, in voxels, if there's room
fn spawn_spaced(world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, within: f64, others: &[Vector3<f64>], seed: u32) -> Option<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut rng = hash(seed ^ 0x5ca7);
    let mut crowded = None;
    for attempt in 0..32 {
        let a = random(&mut rng) * TAU;
        // Wider the more tries it's taken, so there's somewhere in the end
        let r = random(&mut rng).sqrt() * within * vpm * (1.0 + attempt as f64 / 24.0);
        let (x, y) = (near.x + a.cos() * r, near.y + a.sin() * r);
        if x < 2.0 || y < 2.0 || x >= sx - 2.0 || y >= sy - 2.0 {
            continue;
        }
        if let Some(z) = ground_wading(world, water, x, y, near.z, 2.0 * vpm, 3.0 * vpm, look.wade()) {
            let at = Vector3::new(x, y, z);
            let key = [x as i64, y as i64, -2];
            if others.iter().all(|o| (o - at).xy().norm() > SPACING * vpm) {
                return Some(Animal::boss(look, at, key, hash(rng)));
            }
            crowded.get_or_insert(Animal::boss(look, at, key, hash(rng)));
        }
    }
    crowded
}

/// The health of a bloater that burst by itself, rather than being killed, see Game::reap()
pub const BURST: f64 = -1.0e9;

/// How close together the members of a pack turn up, in meters at most
const SPACING: f64 = 0.9;

/// A new number for a pack, different from every other
pub fn new_pack() -> u32 {
    static NEXT_PACK: AtomicU32 = AtomicU32::new(1);
    NEXT_PACK.fetch_add(1, Ordering::Relaxed)
}

/// A pack of monsters, `looks`, close together round `near`, in voxels, within `within` meters of
/// it, none on top of another: they go about and fight together, see bosses::Flock
pub fn spawn_pack(world: &World, water: WaterLevel, looks: &[Look], near: Vector3<f64>, within: f64, seed: u32) -> Vec<Animal> {
    let pack = new_pack();
    let mut members: Vec<Animal> = Vec::new();
    for (k, &look) in looks.iter().enumerate() {
        let others: Vec<Vector3<f64>> = members.iter().map(|m| m.position).collect();
        if let Some(mut member) = spawn_spaced(world, water, look, near, within, &others, hash(seed ^ (k as u32).wrapping_mul(0x9e37))) {
            if let Some(boss) = &mut member.boss {
                boss.pack = pack;
            }
            members.push(member);
        }
    }
    members
}

/// Scatters animals over dry land: about half of them close to `near` so there are some to see
/// straight away, the rest anywhere.
pub fn spawn(world: &World, water: WaterLevel, seed: u32, near: Vector3<f64>) -> Vec<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut rng = hash(seed ^ 0xa11a);
    let mut animals = Vec::new();
    for (look, count) in [(Look::Bear, 6), (Look::Dog, 8), (Look::GrayCat, 5), (Look::GingerCat, 5), (Look::Squirrel, 14)] {
        for i in 0..count {
            for _ in 0..64 {
                let (x, y) = if i % 2 == 0 {
                    let angle = random(&mut rng) * TAU;
                    let distance = (4.0 + 40.0 * random(&mut rng)) * vpm;
                    (near.x + angle.cos() * distance, near.y + angle.sin() * distance)
                } else {
                    (sx * (0.1 + 0.8 * random(&mut rng)), sy * (0.1 + 0.8 * random(&mut rng)))
                };
                if x < 1.0 || y < 1.0 || x >= sx - 1.0 || y >= sy - 1.0 {
                    continue;
                }
                if let Some(z) = surface_at(world, water, x, y) {
                    animals.push(Animal::new(look, Vector3::new(x, y, z), hash(rng)));
                    break;
                }
            }
        }
    }
    // Packs of wolves, out on grass or snow, well away from the player to begin with
    for _ in 0..WOLF_PACKS {
        for _ in 0..64 {
            let (x, y) = (sx * (0.1 + 0.8 * random(&mut rng)), sy * (0.1 + 0.8 * random(&mut rng)));
            if (Vector2::new(x, y) - near.xy()).norm() < 35.0 * vpm {
                continue;
            }
            let Some(z) = surface_at(world, water, x, y) else {
                continue;
            };
            if matches!(world.material_at(x as i64, y as i64, z as i64 - 1), SAND | SANDSTONE | SANDSTONE_DARK | JUNGLE_GRASS | VINE) {
                continue;
            }
            let den = Vector3::new(x, y, z);
            animals.extend(spawn_pack(world, water, &[Look::Wolf; 3], den, PACK_SPREAD, hash(rng)));
            break;
        }
    }
    animals.extend(spawn_gangs(world, water, &mut rng, near, BANDIT_GANGS));
    animals.extend(spawn_wilds(world, water, &mut rng, near, WILD_PACKS));
    animals
}

/// The dead roaming the green country, see spawn_wilds()
const DEAD_PACKS: [&[Look]; 3] = [&[Look::Zombie, Look::Zombie, Look::Ghoul], &[Look::Zombie, Look::Bloater, Look::Zombie], &[Look::Skeleton, Look::BoneArcher, Look::Zombie]];

/// How many packs of wolves, gangs of bandits and packs of the monsters of each kind of country
/// roam the world at once
const WOLF_PACKS: usize = 2;
pub const BANDIT_GANGS: usize = 2;
pub const WILD_PACKS: usize = 2;
/// How far round the middle of a pack its members turn up, in meters
pub const PACK_SPREAD: f64 = 1.8;

/// The bandits making up a gang, picked by `roll`, 0-1: some with a brute, some with crossbowmen
/// hanging back behind a shieldbearer
pub fn gang(roll: f64) -> &'static [Look] {
    match (roll * 4.0) as usize {
        0 => &[Look::Bandit, Look::Bandit, Look::BanditBrute],
        1 => &[Look::Shieldbearer, Look::Crossbowman, Look::Crossbowman],
        2 => &[Look::Bandit, Look::Crossbowman, Look::Bandit],
        _ => &[Look::Shieldbearer, Look::Bandit, Look::Crossbowman],
    }
}

/// `count` gangs of bandits, out on dry land well away from the player at `near`, see gang()
pub fn spawn_gangs(world: &World, water: WaterLevel, rng: &mut u32, near: Vector3<f64>, count: usize) -> Vec<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut gangs = Vec::new();
    for _ in 0..count {
        for _ in 0..64 {
            let (x, y) = (sx * (0.1 + 0.8 * random(rng)), sy * (0.1 + 0.8 * random(rng)));
            if (Vector2::new(x, y) - near.xy()).norm() < 45.0 * vpm {
                continue;
            }
            let Some(z) = surface_at(world, water, x, y) else {
                continue;
            };
            let camp = Vector3::new(x, y, z);
            let members = gang(random(rng));
            gangs.extend(spawn_pack(world, water, members, camp, PACK_SPREAD + 0.4, hash(*rng)));
            break;
        }
    }
    gangs
}

/// How many packs of serpents roam the world at once, see spawn_serpents()
pub const SERPENT_PACKS: usize = 2;

/// `count` nests of the serpents of the land they turn up in, well away from the player at `near`:
/// in the normal world and Free Roam reed vipers in the grass and reeds, silt coilers in deep
/// water, drift serpents under the snow and gale serpents up on bare rock; gale serpents all over
/// the sky isles, bloodvein adders in the dreaming realm and drift serpents in the frozen one
pub fn spawn_serpents(world: &World, water: WaterLevel, rng: &mut u32, near: Vector3<f64>, land: Land, count: usize) -> Vec<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut nests = Vec::new();
    // Looking for deep water takes a while, so only so many times, not to hold up the game
    let mut searches = 2;
    for _ in 0..count {
        for _ in 0..48 {
            let (x, y) = (sx * (0.1 + 0.8 * random(rng)), sy * (0.1 + 0.8 * random(rng)));
            if (Vector2::new(x, y) - near.xy()).norm() < 35.0 * vpm {
                continue;
            }
            let Some(z) = surface_at(world, water, x, y) else {
                continue;
            };
            let ground = world.material_at(x as i64, y as i64, z as i64 - 1);
            let at = Vector3::new(x, y, z);
            let (look, at) = match land {
                Land::Sky => (Look::GaleSerpent, at),
                Land::Dream => (Look::BloodAdder, at),
                Land::Ice => (Look::DriftSerpent, at),
                // The Water realm's coilers in the sea round its islands, and vipers on them
                Land::Water if searches > 0 && random(rng) < 0.7 => {
                    searches -= 1;
                    match shallows_near(world, water, x, y, 30.0, 0.8, 12.0) {
                        Some(bottom) => (Look::SiltCoiler, bottom),
                        None => continue,
                    }
                }
                Land::Water => (Look::ReedViper, at),
                // None on the Cinder Crown's ash: its beasts are the dragons' own
                Land::Dragon => continue,
                Land::Normal => match ground {
                    SNOW | ICE => (Look::DriftSerpent, at),
                    STONE | ROCK if random(rng) < 0.5 => (Look::GaleSerpent, at),
                    // Swimming in deep water near the grass, if there's any
                    GRASS | DIRT | JUNGLE_GRASS | SAND if searches > 0 && random(rng) < 0.45 => {
                        searches -= 1;
                        match shallows_near(world, water, x, y, 30.0, 0.8, 12.0) {
                            Some(bottom) => (Look::SiltCoiler, bottom),
                            None => continue,
                        }
                    }
                    GRASS | DIRT | JUNGLE_GRASS | DRY_GRASS => (Look::ReedViper, at),
                    _ => continue,
                },
            };
            let size = look.pack_size() + (random(rng) < 0.35) as usize;
            nests.extend(spawn_pack(world, water, &vec![look; size], at, PACK_SPREAD + 0.6, hash(*rng)));
            break;
        }
    }
    nests
}

/// `count` packs of the monsters of the country they turn up in, well away from the player at
/// `near`: burrowers on desert sand, wraiths on the snow, tuskers or the walking dead in the green
/// woods, and lurkers in the shallows of jungle waters
pub fn spawn_wilds(world: &World, water: WaterLevel, rng: &mut u32, near: Vector3<f64>, count: usize) -> Vec<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut packs = Vec::new();
    // Looking for shallows takes a while, so only so many times, not to hold up the game
    let mut searches = 2;
    for _ in 0..count {
        for _ in 0..48 {
            let (x, y) = (sx * (0.1 + 0.8 * random(rng)), sy * (0.1 + 0.8 * random(rng)));
            if (Vector2::new(x, y) - near.xy()).norm() < 35.0 * vpm {
                continue;
            }
            let Some(z) = surface_at(world, water, x, y) else {
                continue;
            };
            // Now and then a pair of drakelings roosting on the high bare rock, see dragons.rs
            let ground = world.material_at(x as i64, y as i64, z as i64 - 1);
            if crate::dragons::roost(ground, z, world.size[2] as f64) && random(rng) < 0.2 {
                let roll = random(rng);
                packs.extend(crate::dragons::roosting(world, water, Vector3::new(x, y, z), ground, roll, hash(*rng)));
                break;
            }
            let (look, at) = match ground {
                SAND | SANDSTONE | SANDSTONE_DARK => (Look::DuneBurrower, Vector3::new(x, y, z)),
                SNOW | ICE => (Look::RimeWraith, Vector3::new(x, y, z)),
                // Out in the green country, tuskers, or the dead walking in packs of their own
                GRASS | DIRT if random(rng) < 0.4 => {
                    let dead = DEAD_PACKS[(random(rng) * DEAD_PACKS.len() as f64) as usize % DEAD_PACKS.len()];
                    packs.extend(spawn_pack(world, water, dead, Vector3::new(x, y, z), PACK_SPREAD, hash(*rng)));
                    break;
                }
                GRASS | DIRT => (Look::Tusker, Vector3::new(x, y, z)),
                // Lying in wait in the shallows, if there are any near
                JUNGLE_GRASS if searches > 0 => {
                    searches -= 1;
                    match shallows_near(world, water, x, y, 26.0, 0.25, 1.0) {
                        Some(at) => (Look::MireLurker, at),
                        None => continue,
                    }
                }
                _ => continue,
            };
            let size = look.pack_size() + (random(rng) < 0.4) as usize;
            packs.extend(spawn_pack(world, water, &vec![look; size], at, PACK_SPREAD, hash(*rng)));
            break;
        }
    }
    packs
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A snake lying across a bump in the ground is drawn draped over it, its middle raised up over
    /// the bump rather than its body running through it in a straight line, and where it can be
    /// hit raised with it; and its S-curve's tighter slithering fast than slowly
    #[test]
    fn a_snake_drapes_over_the_ground_and_tightens_its_curve_going_fast() {
        let [slow, fast] = [serpents::stretch(0.0), serpents::stretch(1.0)];
        assert!(fast[0] < 1.0 && fast[1] < 1.0 && slow[0] > 1.0 && slow[1] > 1.0, "slow {slow:?}, fast {fast:?}");
        // A flat field 32 m across at 8 voxels a meter, its ground 4 m up, a bump half a meter high
        // three across and two wide in the middle of it
        let size = [256u32, 256, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 8;
        let mut bump = [AIR; 512];
        bump[..256].fill(DIRT);
        for bx in 15..18 {
            for by in 15..17 {
                world.set_brick([bx, by, 4], &bump);
            }
        }
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let vpm = 8.0;
        let mut snake = Animal::new(Look::ReedViper, Vector3::new(16.5, 16.0, 4.0) * vpm, 7);
        (snake.heading, snake.target_heading, snake.drawn_heading) = (0.0, 0.0, 0.0);
        let kind = serpents::kind(Look::ReedViper).unwrap();
        let (frame, position, rotation, _) = snake.placement(8);
        let drape = snake.drape_on(kind, &world, &water, frame, position, rotation);
        assert!(drape[1] > 0.4 && drape[2] > 0.4, "its middle's up over the bump: {drape:?}");
        assert!(drape[0].abs() < 0.05 && drape[3].abs() < 0.05, "its ends lie on the ground either side: {drape:?}");
        let flat = snake.spine(8).unwrap();
        snake.drape = drape;
        let draped = snake.spine(8).unwrap();
        assert!(draped[4].z > flat[4].z + 0.4 * vpm, "where it's hit's up over the bump too");
        assert!(snake.instance(8).drape != [0, 0], "it's drawn draped");
    }

    /// Every frame of every monster added for the different kinds of country: solid enough to
    /// see, small enough for the shader, and inside the reach it's culled by
    #[test]
    fn new_monsters_have_whole_models_in_every_frame() {
        let mut looks = vec![Look::Crossbowman, Look::Shieldbearer, Look::DuneBurrower, Look::RimeWraith, Look::MireLurker, Look::Gloomwing, Look::Tusker, Look::GraveKnight, Look::CryptTitan, Look::Bloater, Look::Ghoul, Look::GraveBrute, Look::BoneArcher, Look::Necromancer];
        looks.extend(serpents::SERPENTS);
        for look in looks {
            assert!(look.build().is_some(), "{look:?} isn't a boss or a monster");
            for frame in (0..FRAMES).filter(|&f| shown(look, f)) {
                let model = frame_model(look, frame);
                assert!(model.size.iter().all(|&s| s < 255), "{look:?} frame {frame} is {:?} voxels", model.size);
                let mut solid = 0;
                let mut furthest = 0.0f64;
                for z in 0..model.size[2] {
                    for y in 0..model.size[1] {
                        for x in 0..model.size[0] {
                            if model.voxels[(x + (y + z * model.size[1]) * model.size[0]) as usize] == AIR {
                                continue;
                            }
                            solid += 1;
                            let at = Vector3::new(x as f64 + 0.5 - model.size[0] as f64 * 0.5, y as f64 + 0.5 - model.size[1] as f64 * 0.5, z as f64 + 0.5) / look.resolution();
                            furthest = furthest.max(at.norm());
                        }
                    }
                }
                assert!(solid > 100, "{look:?} frame {frame} has only {solid} voxels");
                assert!(furthest <= look.reach(), "{look:?} frame {frame} reaches {furthest:.2} m, past its {:.2} m", look.reach());
            }
        }
    }

    /// Every frame between poses, and every pose a boss can be eased into, has a model for every
    /// look that can be drawn in it, so a boss is never drawn with nothing
    #[test]
    fn tweens_only_go_through_frames_with_models() {
        let poses = [0, POSE_WINDUP, POSE_STRIKE, POSE_SPECIAL, POSE_COIL, POSE_SWEEP, POSE_RAISE, POSE_SPIKES];
        for look in Look::ALL.into_iter().filter(|l| l.build().is_some()) {
            for a in poses.into_iter().filter(|&f| shown(look, f)) {
                for b in poses.into_iter().filter(|&f| shown(look, f)) {
                    for k in 0..40 {
                        if let Some(frame) = tween(look, a, b, k as f64 * 0.012) {
                            assert!(shown(look, frame), "{look:?} going from {a} to {b} is drawn in frame {frame}, which it has no model for");
                        }
                    }
                }
            }
        }
    }

    /// Posed live, a monster goes from one pose to the next through the very frames its baked frames
    /// step through, only smoothly: from the first pose to the last, at the moment tween() is in
    /// each of them, and between poses with no frames between it eases across in a moment
    #[test]
    fn tween_paths_go_through_the_tweens() {
        let poses = [0, POSE_WINDUP, POSE_STRIKE, POSE_SPECIAL];
        for look in Look::ALL.into_iter().filter(|&l| crate::baked::baked(l) && shown(l, POSE_WINDUP)) {
            for a in poses {
                for b in poses.into_iter().filter(|&b| b != a) {
                    let (path, along) = tween_path(look, a, b, 0.0).unwrap();
                    assert_eq!((path[0], *path.last().unwrap(), along), (a, b, 0.0), "{look:?} from {a} to {b}");
                    assert!(path.iter().all(|&f| shown(look, f)), "{look:?} from {a} to {b} goes through {path:?}");
                    let mut seen = 0;
                    for k in 1..400 {
                        // Off the moments exactly halfway between two frames, either of which is
                        // as near
                        let since = (k as f64 + 0.37) * 0.002;
                        match (tween(look, a, b, since), tween_path(look, a, b, since)) {
                            (Some(frame), Some((path, along))) => {
                                // The baked frame is the one of the path nearest how far along it is
                                let nearest = path[(along * (path.len() - 1) as f64).round() as usize];
                                assert_eq!(frame, nearest, "{look:?} from {a} to {b} after {since} s");
                                seen += 1;
                            }
                            (None, Some((path, _))) => assert_eq!(path.len(), 2, "{look:?} from {a} to {b} only eases across"),
                            (_, None) => break,
                        }
                    }
                    assert!(seen > 0 || path.len() == 2, "{look:?} from {a} to {b}");
                }
            }
        }
    }

    /// All the models together, which are sent to the graphics card at the start: how big they
    /// are and how long they take to make. `cargo test --release animal_models -- --nocapture`
    #[test]
    fn animal_models_fit_in_memory() {
        let start = std::time::Instant::now();
        let models = animal_models();
        let took = start.elapsed();
        assert_eq!(models.len(), ANIMAL_MODELS);
        let bytes = pack_models(&models.iter().collect::<Vec<_>>()).len() * 4;
        let of = |look: Look| {
            let k = Look::ALL.iter().position(|&l| l == look).unwrap();
            models[k * FRAMES..(k + 1) * FRAMES].iter().map(|m| m.voxels.len()).sum::<usize>()
        };
        println!(
            "{} models in {FRAMES} frames each, {:.1} MB packed, made in {:.0} ms; the Grave Knight's {:.1} MB, the Crypt Titan's {:.1} MB",
            models.len(),
            bytes as f64 / 1e6,
            took.as_secs_f64() * 1000.0,
            of(Look::GraveKnight) as f64 / 1e6,
            of(Look::CryptTitan) as f64 / 1e6
        );
        // What the models made finer take, packed, see FINER and rig::res(): the wolves' frames and
        // the Grave Knight's pieces
        let packed = |from: usize, to: usize| pack_models(&models[from..to].iter().collect::<Vec<_>>()).len() * 4;
        let wolf = Look::ALL.iter().position(|&l| l == Look::Wolf).unwrap();
        let knight = RIG_MODELS_AT + crate::rig::kind(Look::GraveKnight).unwrap() * crate::rig::PIECES;
        let wolf_pieces = BAKED_MODELS_AT + crate::baked::kind(Look::Wolf).unwrap() * crate::baked::PIECES;
        println!(
            "made finer: the wolves' frames {:.2} MB packed and their pieces {:.2} MB, the Grave Knight's pieces {:.2} MB",
            packed(wolf * FRAMES, (wolf + 1) * FRAMES) as f64 / 1e6,
            packed(wolf_pieces, wolf_pieces + crate::baked::PIECES) as f64 / 1e6,
            packed(knight, knight + crate::rig::PIECES) as f64 / 1e6
        );
        assert!(bytes < 150_000_000, "the models take {bytes} bytes");
    }

    /// Packed for the shader, every frame of every creature and every dragon's piece reads back
    /// exactly as it was made, voxel for voxel, though only the box round what's solid in each is
    /// kept and those alike share theirs
    #[test]
    fn every_model_unpacks_as_it_was_made() {
        let models = animal_models();
        let words = pack_models(&models.iter().collect::<Vec<_>>());
        for (i, model) in models.iter().enumerate() {
            let size = words[i * HEADER_WORDS + 1];
            assert_eq!([size & 0x3FF, size >> 10 & 0x3FF, size >> 20 & 0x3FF], model.size, "model {i}'s size");
            let [sx, sy, sz] = model.size;
            for z in 0..sz {
                for y in 0..sy {
                    for x in 0..sx {
                        let at = (x + (y + z * sy) * sx) as usize;
                        assert_eq!(packed_voxel(&words, i, [x, y, z]), model.voxels[at], "model {i} at {x}, {y}, {z}");
                    }
                }
            }
        }
        let whole: usize = models.iter().map(|m| m.voxels.len().div_ceil(4) * 4 + 8).sum();
        println!("models {:.1} MB as they were, {:.1} MB packed", whole as f64 / 1e6, words.len() as f64 * 4.0 / 1e6);
    }

    /// The new monsters each have their own name, so they can be told apart in what's said
    #[test]
    fn monsters_have_their_own_names() {
        let monsters: Vec<Look> = Look::ALL.into_iter().filter(|l| l.is_monster()).collect();
        for (i, a) in monsters.iter().enumerate() {
            for b in &monsters[i + 1..] {
                assert_ne!(a.name(), b.name(), "{a:?} and {b:?} share a name");
            }
        }
    }
}

/// The bottom under `x`, `y`, in voxels: the first solid voxel looking down from `z`, or the bottom
/// of the world
fn ground_at_floor(world: &World, x: f64, y: f64, z: f64) -> i64 {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let top = z.floor() as i64 + 1;
    (0..=top).rev().find(|&k| !is_passable(world.material_at(xi, yi, k))).map_or(0, |k| k + 1)
}
