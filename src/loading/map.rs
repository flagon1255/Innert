//! The map beside the menu Innert starts on, of the world about to be entered: the land of the
//! loading screen, wider and coarser, see land.rs, with what's on it standing up from it, towns,
//! castles, bandits' camps, crypts, ruins and lairs, and the spot the player's at or sets out from
//! marked. Which world goes by what was last pointed at in the menu, see App::map_wanted(): the
//! saved game Continue goes on with, round where it was left, or a new one, round where it sets
//! out, on the seed in the seed box, drawn afresh as that changes.
//!
//! It's worked out on threads of its own, see Backdrop, in two goes: the land and what's built on
//! it, a few tenths of a second, see build(); then the roads, the names of the towns and castles,
//! and which way the nearest of them are when they're further off than it reaches, a few seconds,
//! see far(). What's under the mouse is named, see Map::under().

use std::f32::consts::{PI, TAU};

use egui_winit_vulkano::egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};

use super::land::{Land, Stand};
use crate::realms::{self, REALMS};
use crate::save::RealmsSave;
use crate::worldgen::{self, CampKind, RiverCache, TerrainSettings};

/// Columns across each way of the map, and how many meters of the landscape each is across
pub const ACROSS: usize = 48;
pub const CELL: f32 = 10.0;

/// What the map's to show, see App::map_wanted()
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    /// The landscape's, of its seed
    pub settings: TerrainSettings,
    /// Where the player is, or sets out, in meters of the whole landscape: None for in front of the
    /// Innert Gate, or the portal home, of `realm`, which is found on the way
    pub at: Option<[f32; 2]>,
    /// In Realms, the realm
    pub realm: Option<Realm>,
    /// What it's of, said under it
    pub caption: String,
    /// Whether it's a game gone on with, rather than a new one
    pub saved: bool,
}

/// The realm a map's of, in Realms: which, where its places are looked for from and which way that
/// looks, see realms::Places::find(), and how far the journey's got, for whether the gate stands
#[derive(Clone, Debug, PartialEq)]
pub struct Realm {
    pub realm: usize,
    pub start: [f64; 2],
    pub ahead: [f64; 2],
    pub progress: Option<RealmsSave>,
}

/// What a place on the map is
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Town,
    /// A town down in a cavern under a hill
    CaveTown,
    Castle,
    /// A castle fallen to the dead
    FallenCastle,
    Camp(CampKind),
    /// A dungeon's crypt, its boss's hall down under it
    Crypt,
    /// The ruins out on the land, as worldgen calls them: "castle", "watchtower" and so on
    Ruin(&'static str),
    WaterfallCave,
    /// A city fallen deep under the land, its gate
    Undercity,
    DragonLair,
    Shrine,
    /// The Innert Gate, its old foundations while it's still to be built, and a portal home
    Gate,
    Foundations,
    Portal,
    /// A realm's raid boss, who, and where its lair's said to be
    RaidBoss(&'static str, &'static str),
    Guardian(&'static str),
    /// A realm's finale, who, and where that's said to be
    Finale(&'static str, &'static str),
}

impl Kind {
    /// What it's called on the map, when it has no name of its own
    pub fn title(self) -> String {
        match self {
            Kind::Town => "Town".into(),
            Kind::CaveTown => "Town in a cavern".into(),
            Kind::Castle => "Royal castle".into(),
            Kind::FallenCastle => "Fallen castle".into(),
            Kind::Camp(_) => "Bandit camp".into(),
            Kind::Crypt => "Crypt".into(),
            Kind::Ruin(what) => match what {
                "castle" => "Ruined castle".into(),
                "watchtower" => "Old watchtower".into(),
                "colonnade" => "Colonnade".into(),
                "desert ruins" => "Desert ruins".into(),
                "flooded church" => "Flooded church".into(),
                "pyramid" => "Pyramid".into(),
                "shrine" => "Jungle shrine".into(),
                other => capitalised(other),
            },
            Kind::WaterfallCave => "Cave behind the falls".into(),
            Kind::Undercity => "Way down to a fallen city".into(),
            Kind::DragonLair => "Dragon's lair".into(),
            Kind::Shrine => "Wing shrine".into(),
            Kind::Gate => "The Innert Gate".into(),
            Kind::Foundations => "Old foundations".into(),
            Kind::Portal => "The portal home".into(),
            Kind::RaidBoss(who, _) => who.into(),
            Kind::Guardian(who) => who.into(),
            Kind::Finale(who, _) => who.into(),
        }
    }

    /// What kind of place it is, said under its name
    pub fn about(self) -> String {
        match self {
            Kind::Town => "town".into(),
            Kind::CaveTown => "town in a cavern".into(),
            Kind::Castle => "royal castle".into(),
            Kind::FallenCastle => "castle fallen to the dead".into(),
            Kind::Camp(kind) => match kind {
                CampKind::Fort => "a bandits' fort",
                CampKind::Cabins => "bandits' cabins",
                CampKind::Ruin => "a ruin held by bandits",
                CampKind::Hideout => "a bandits' hideout",
                CampKind::Raiders => "raiders in the woods",
                CampKind::Awnings => "bandits under awnings",
                CampKind::Yurts => "bandits' yurts",
            }
            .into(),
            Kind::Crypt => "a dungeon under it".into(),
            Kind::Ruin(_) => "ruins".into(),
            Kind::WaterfallCave => "a cave".into(),
            Kind::Undercity => "deep under the land".into(),
            Kind::DragonLair => "a great dragon".into(),
            Kind::Shrine => "wings in its nest".into(),
            Kind::Gate | Kind::Portal => "the way through the realms".into(),
            Kind::Foundations => "where the Innert Gate stood".into(),
            Kind::RaidBoss(_, at) => format!("raid boss, {at}"),
            Kind::Guardian(_) => "guardian".into(),
            Kind::Finale(_, at) => format!("{at}"),
        }
    }

    /// How far from where it's marked it still counts as there for the mouse, in meters
    fn reach(self) -> f32 {
        match self {
            Kind::Town => 32.0,
            Kind::Castle | Kind::FallenCastle => 38.0,
            Kind::CaveTown | Kind::RaidBoss(..) | Kind::Finale(..) | Kind::DragonLair => 16.0,
            Kind::Camp(_) | Kind::Ruin(_) | Kind::Gate | Kind::Foundations | Kind::Portal => 13.0,
            _ => 9.0,
        }
    }

    /// The pin over it on the map, if it has one: how big, and how light, the lightest for where
    /// people live, dark for where what's to be feared is. Ruins and caves have none, so the map
    /// isn't stuck all over with them; they're named under the mouse all the same.
    pub fn pin(self) -> Option<(f32, u8)> {
        match self {
            Kind::Town | Kind::CaveTown | Kind::Castle | Kind::Gate | Kind::Portal | Kind::Foundations => Some((4.5, 250)),
            Kind::FallenCastle | Kind::RaidBoss(..) | Kind::Finale(..) | Kind::DragonLair => Some((4.5, 20)),
            Kind::Guardian(_) | Kind::Crypt | Kind::Undercity => Some((3.5, 20)),
            Kind::Camp(_) => Some((3.5, 120)),
            Kind::Shrine => Some((3.0, 190)),
            Kind::Ruin(_) | Kind::WaterfallCave => None,
        }
    }

    /// Whether it's a town or a castle, which have names, see far()
    fn named(self) -> Option<bool> {
        match self {
            Kind::Town | Kind::CaveTown => Some(false),
            Kind::Castle | Kind::FallenCastle => Some(true),
            _ => None,
        }
    }
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

/// A place on the map: what, what it's called if it has a name, and where, in meters of the whole
/// landscape
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub kind: Kind,
    pub name: Option<String>,
    pub at: [f32; 2],
}

impl Place {
    fn new(kind: Kind, at: [f32; 2]) -> Place {
        Place { kind, name: None, at }
    }

    /// What it's called on the map, its own name or what it is
    pub fn title(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.kind.title())
    }
}

/// The map of a world, see build()
#[derive(Clone)]
pub struct Map {
    pub land: Land,
    /// What's marked on it, and further off, which way the nearest towns and castles are
    pub places: Vec<Place>,
    pub far: Vec<Place>,
    /// Where the player is or sets out, in meters, its middle
    pub player: [f32; 2],
    pub settings: TerrainSettings,
    pub caption: String,
    /// Whether it's a game gone on with, rather than a new one
    pub saved: bool,
}

/// The roads over the land within the square `across` meters across round `middle`, and the
/// towns and castles in it with their names, for the map: each road as the points along it in
/// meters, and each place where its middle is, its name, and whether it's a castle, see
/// worldgen\roads.rs. Worked out if they haven't been, which can take a second or two.
fn roads_near(settings: &TerrainSettings, middle: [f32; 2], across: f32) -> (Vec<Vec<[f32; 2]>>, Vec<([f32; 2], String, bool)>) {
    worldgen::map_roads(settings, middle, across)
}

/// How much further round than the map reaches the roads and the named towns and castles are
/// looked for, to point to those beyond it
const FAR_ACROSS: f32 = 4.0;

/// The map `wanted` asks for, its land and what's built on it, without the roads, the names or the
/// places further off, which far() works out, as Backdrop has it worked out a step at a time
#[cfg(test)]
pub fn build(wanted: &Wanted) -> Map {
    build_land(wanted).with_sites()
}

/// The map `wanted` asks for, its land alone, and in Realms the realm's own places, the gate and
/// its raid bosses: a few milliseconds, for the land to rise straight away while the rest of
/// what's on it is found, see Map::with_sites()
pub fn build_land(wanted: &Wanted) -> Map {
    let settings = wanted.settings;
    let found = wanted.realm.as_ref().map(|r| realms::Places::find(r.realm, &settings, r.start, r.ahead));
    // The gate or the portal home where the player built it, or its old foundations, see gates.rs
    let (places_in_realm, standing) = match (&found, wanted.realm.as_ref().and_then(|r| r.progress.as_ref())) {
        (Some(found), Some(progress)) => {
            let (places, standing) = crate::gates::stand(found, progress);
            (Some(places), standing)
        }
        (Some(found), None) => (Some(found.clone()), true),
        _ => (None, true),
    };
    let player = wanted.at.or_else(|| places_in_realm.as_ref().map(|p| [p.arrival[0] as f32, p.arrival[1] as f32])).unwrap_or(worldgen::FREE_ROAM_START);
    let mut land = Land::sample(&settings, player, ACROSS, CELL, 0);
    let across = ACROSS as f32 * CELL;
    let mut places = Vec::new();
    if let Some(p) = &places_in_realm {
        let r = &REALMS[p.realm];
        let gate = if !standing {
            Kind::Foundations
        } else if p.realm == 0 {
            Kind::Gate
        } else {
            Kind::Portal
        };
        let at = |v: [f64; 2]| [v[0] as f32, v[1] as f32];
        places.push(Place::new(gate, at(p.gate)));
        for (k, lair) in p.lairs.iter().enumerate() {
            places.push(Place::new(Kind::RaidBoss(r.raid_bosses[k].0.name(), r.lair_names[k]), at(*lair)));
        }
        for (k, g) in p.guardians.iter().enumerate() {
            places.push(Place::new(Kind::Guardian(realms::guardian_look(p.realm, k).name()), at(*g)));
        }
        if let (Some(spot), Some((look, _, said))) = (p.finale, r.finale) {
            places.push(Place::new(Kind::Finale(look.name(), said), at(spot)));
        }
    }
    let half = across * 0.5;
    places.retain(|p| (p.at[0] - player[0]).abs() < half && (p.at[1] - player[1]).abs() < half);
    for p in &places {
        stand_on(&mut land, p);
    }
    Map { land, places, far: Vec::new(), player, settings, caption: wanted.caption.clone(), saved: wanted.saved }
}

/// What worldgen's placed within the square `across` meters across round `middle` that's marked on
/// the map, once each
fn sites(settings: &TerrainSettings, middle: [f32; 2], across: f32) -> Vec<Place> {
    // As placed for a world built at any resolution, see worldgen::place_for()
    let vpm = 1u32;
    let rough = TerrainSettings { voxels_per_meter: vpm, grass_tufts: false, ..*settings };
    let lo = [middle[0] - across * 0.5, middle[1] - across * 0.5];
    let origin = lo.map(|v| (v * vpm as f32).floor() as i64);
    let size = [(across * vpm as f32).ceil() as u32; 2];
    let terrain = worldgen::place_for(&rough, origin, size, &RiverCache::default());
    let mut places: Vec<Place> = Vec::new();
    let mut put = |kind: Kind, at: [f32; 2]| {
        // Once, though some leave more than one site
        if !places.iter().any(|p| p.kind == kind && (p.at[0] - at[0]).hypot(p.at[1] - at[1]) < 24.0) {
            places.push(Place::new(kind, at));
        }
    };
    for site in terrain.sites() {
        let at = [site.position[0], site.position[1]];
        let kind = match site.kind {
            "town" => Kind::Town,
            "cave town" => Kind::CaveTown,
            "royal court" => Kind::Castle,
            "fallen court" => Kind::FallenCastle,
            "dungeon" => Kind::Crypt,
            "waterfall cave" => Kind::WaterfallCave,
            "undercity gate" => Kind::Undercity,
            "dragon lair" => Kind::DragonLair,
            "wing shrine" => Kind::Shrine,
            what @ ("castle" | "watchtower" | "colonnade" | "desert ruins" | "flooded church" | "pyramid" | "shrine") => Kind::Ruin(what),
            _ => continue,
        };
        put(kind, at);
    }
    for camp in terrain.bandit_camps() {
        put(Kind::Camp(camp.kind), [camp.fire[0], camp.fire[1]]);
    }
    places
}

/// Has what's at `place` stand on the columns of `land` it covers
fn stand_on(land: &mut Land, place: &Place) {
    let (x, y) = land.columns(place.at);
    let Some(middle) = land.column((x, y)) else { return };
    let cell = land.cell();
    let round = |reach: f32, stand: Stand, land: &mut Land| {
        let r = (reach / cell).ceil() as i32;
        for dj in -r..=r {
            for di in -r..=r {
                let c = (middle.0 + di, middle.1 + dj);
                let (cx, cy) = land.middle_of(c);
                if (cx - x).hypot(cy - y) * cell <= reach && land.height(c.0, c.1).is_some_and(|h| h > 0.0) {
                    land.build(c, stand);
                }
            }
        }
    };
    match place.kind {
        Kind::Town => {
            round(28.0, Stand::Houses, land);
            land.build(middle, Stand::Square);
        }
        Kind::Castle | Kind::FallenCastle => {
            let fallen = place.kind == Kind::FallenCastle;
            // Its walls round, its keep in the middle
            for dj in -2..=2i32 {
                for di in -2..=2i32 {
                    if di.abs().max(dj.abs()) == 2 {
                        land.build((middle.0 + di, middle.1 + dj), Stand::Wall(fallen));
                    }
                }
            }
            land.build(middle, Stand::Keep(fallen));
        }
        Kind::CaveTown | Kind::WaterfallCave => land.build(middle, Stand::Cave),
        Kind::Camp(_) => land.build(middle, Stand::Camp),
        Kind::Crypt | Kind::Undercity => land.build(middle, Stand::Crypt),
        Kind::Ruin(_) => land.build(middle, Stand::Ruin),
        Kind::DragonLair | Kind::RaidBoss(..) | Kind::Finale(..) | Kind::Guardian(_) => land.build(middle, Stand::Spire),
        Kind::Shrine => land.build(middle, Stand::Shrine),
        Kind::Gate | Kind::Portal | Kind::Foundations => land.build(middle, Stand::Gate),
    }
}

/// What's further off than `map` reaches, or only known later: the roads over it and the names of
/// its towns and castles, see roads_near(), and which way the nearest town and castle are if
/// none's on it. Takes a few seconds, so it's worked out on a thread of its own after build().
pub struct Far {
    pub roads: Vec<Vec<[f32; 2]>>,
    pub named: Vec<([f32; 2], String, bool)>,
    pub beyond: Vec<Place>,
}

/// Works out what's further off round `map`, see Far: given up between its steps if `stale` says
pub fn far(map: &Map, stale: &dyn Fn() -> bool) -> Option<Far> {
    let across = ACROSS as f32 * CELL;
    let (roads, named) = roads_near(&map.settings, map.player, across * FAR_ACROSS);
    let half = across * 0.5;
    let on_map = |at: [f32; 2]| (at[0] - map.player[0]).abs() < half && (at[1] - map.player[1]).abs() < half;
    // The nearest town and the nearest castle of those named, if they're off the map
    let mut beyond: Vec<Place> = Vec::new();
    for castle in [false, true] {
        let nearest = named.iter().filter(|(_, _, c)| *c == castle).min_by(|a, b| distance(a.0, map.player).total_cmp(&distance(b.0, map.player)));
        if let Some((at, name, _)) = nearest
            && !on_map(*at)
        {
            beyond.push(Place { kind: if castle { Kind::Castle } else { Kind::Town }, name: Some(name.clone()), at: *at });
        }
    }
    // Till then, or where there are no roads, the nearest of each kind by itself, if it's not on the
    // map already: there may be none on the land for kilometers
    if named.is_empty() && !matches!(worldgen::land_of(map.settings.seed), worldgen::Land::Dream | worldgen::Land::Water | worldgen::Land::Dragon | worldgen::Land::Sky) {
        let rough = TerrainSettings { voxels_per_meter: 1, grass_tufts: false, ..map.settings };
        let terrain = worldgen::place_for(&rough, [0, 0], [8, 8], &RiverCache::default());
        let has = |kinds: &[Kind]| map.places.iter().any(|p| kinds.contains(&p.kind));
        let mut look = |kind: Kind, find: &dyn Fn() -> Option<[f32; 2]>| -> Option<()> {
            if stale() {
                return None;
            }
            if !has(&[kind])
                && let Some(at) = find()
                && !on_map(at)
            {
                beyond.push(Place::new(kind, at));
            }
            Some(())
        };
        let near = map.player;
        look(Kind::Town, &|| terrain.nearest_town(near, false, None).map(|t| [t.0[0], t.0[1]]))?;
        look(Kind::Castle, &|| terrain.nearest_castle(near, Some("living")).map(|c| [c.middle[0], c.middle[1]]))?;
        look(Kind::CaveTown, &|| terrain.nearest_town(near, true, None).map(|t| [t.0[0], t.0[1]]))?;
        look(Kind::FallenCastle, &|| terrain.nearest_castle(near, Some("fallen")).map(|c| [c.middle[0], c.middle[1]]))?;
    }
    Some(Far { roads, named, beyond })
}

impl Map {
    /// The map with what worldgen's placed on its land, towns, castles, camps and the rest, found
    /// as for a world built there, which takes a second or so
    pub fn with_sites(&self) -> Map {
        let mut land = self.land.clone();
        let mut places = sites(&self.settings, self.player, ACROSS as f32 * CELL);
        for p in &places {
            stand_on(&mut land, p);
        }
        // The realm's own over what's there
        for p in &self.places {
            stand_on(&mut land, p);
        }
        places.extend(self.places.iter().cloned());
        Map { land, places, far: self.far.clone(), player: self.player, settings: self.settings, caption: self.caption.clone(), saved: self.saved }
    }

    /// The map with what's further off, see far(): the roads drawn over it, its towns and castles
    /// named, and those beyond it pointed to
    pub fn with(&self, far: &Far) -> Map {
        let mut land = self.land.clone();
        let mut places = self.places.clone();
        let cell = land.cell();
        for road in &far.roads {
            for pair in road.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / (cell * 0.3)).ceil().max(1.0) as usize;
                for s in 0..=steps {
                    let t = s as f32 / steps as f32;
                    let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                    if let Some(c) = land.column(land.columns(p)) {
                        land.build(c, Stand::Road);
                    }
                }
            }
        }
        // Each town and castle by the name of the nearest of those named there
        for place in places.iter_mut() {
            let Some(castle) = place.kind.named() else { continue };
            let nearest = far.named.iter().filter(|(_, _, c)| *c == castle).min_by(|a, b| distance(a.0, place.at).total_cmp(&distance(b.0, place.at)));
            if let Some((at, name, _)) = nearest
                && distance(*at, place.at) < 70.0
            {
                place.name = Some(name.clone());
            }
        }
        // And any named there not marked yet
        let half = ACROSS as f32 * CELL * 0.5;
        for (at, name, castle) in &far.named {
            let on_map = (at[0] - self.player[0]).abs() < half && (at[1] - self.player[1]).abs() < half;
            if on_map && !places.iter().any(|p| p.name.as_ref() == Some(name)) {
                let place = Place { kind: if *castle { Kind::Castle } else { Kind::Town }, name: Some(name.clone()), at: *at };
                stand_on(&mut land, &place);
                places.push(place);
            }
        }
        for place in &places {
            // Over the road going in
            if place.kind.named().is_some() {
                stand_on(&mut land, place);
            }
        }
        Map { land, places, far: far.beyond.clone(), player: self.player, settings: self.settings, caption: self.caption.clone(), saved: self.saved }
    }

    /// What's at the spot `at` columns from the middle of the land, for the mouse over it: the
    /// place there, if any's near enough, the nearest, or the player's own spot
    pub fn place_at(&self, at: (f32, f32)) -> Option<&Place> {
        let spot = self.land.meters(at);
        let cell = self.land.cell();
        self.places
            .iter()
            .map(|p| (distance(p.at, spot), p))
            .filter(|(d, p)| *d <= p.kind.reach().max(cell * 0.8))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p)
    }

    /// What the mouse over the spot `at` columns from the middle of the land names: a title, and
    /// under it what it is and how far from the player's spot, and which way
    pub fn under(&self, at: (f32, f32)) -> (String, String) {
        let spot = self.land.meters(at);
        if distance(spot, self.player) < self.land.cell() * 0.75 {
            return (if self.saved { "You are here" } else { "You set out here" }.into(), self.ground(at).0.to_lowercase());
        }
        let away = self.away(spot);
        match self.place_at(at) {
            Some(place) => (place.title(), format!("{} · {}", place.kind.about(), self.away(place.at))),
            None => {
                let (kind, about) = self.ground(at);
                (kind, format!("{about} · {away}"))
            }
        }
    }

    /// What's said of `place`, further off than the map reaches, pointed to from its edge
    pub fn under_far(&self, place: &Place) -> String {
        let what = if place.name.is_some() { place.kind.about() } else { place.kind.title().to_lowercase() };
        format!("the nearest {what} · {}", self.away(place.at))
    }

    /// How far the spot `at` in meters is from the player's, and which way
    fn away(&self, at: [f32; 2]) -> String {
        let (dx, dy) = (at[0] - self.player[0], at[1] - self.player[1]);
        let d = dx.hypot(dy);
        let how_far = if d >= 1000.0 { format!("{:.1} km", d / 1000.0) } else { format!("{:.0} m", (d / 10.0).round() * 10.0) };
        format!("{how_far} {}", way_of(dx, dy))
    }

    /// What the ground's like at the spot `at` columns from the middle of the land: what kind of
    /// land, and how high or deep
    pub fn ground(&self, at: (f32, f32)) -> (String, String) {
        let Some(column) = self.land.column(at) else { return ("Open sky".into(), String::new()) };
        let Some(h) = self.land.height(column.0, column.1) else { return ("Open sky".into(), "far over the sea".into()) };
        let [x, y] = self.land.meters(at);
        let land = worldgen::land_of(self.settings.seed);
        if h < 0.0 {
            let kind = match land {
                worldgen::Land::Ice => "Frozen sea",
                worldgen::Land::Dragon => "Lava",
                worldgen::Land::Dream if worldgen::dream_blood(x, y) > 0.5 => "Blood",
                _ if h > -2.0 => "Shallows",
                _ => "Sea",
            };
            return (kind.into(), format!("{:.0} m deep", -h));
        }
        let c = worldgen::climate(&self.settings, x, y);
        let woods = self.land.woods(column);
        let kind = match land {
            worldgen::Land::Sky => "Sky isle",
            worldgen::Land::Dragon => "Ash and cinder",
            worldgen::Land::Dream => {
                if worldgen::dream_blood(x, y) > 0.5 {
                    "The red"
                } else {
                    "Dreaming wilds"
                }
            }
            worldgen::Land::Ice => "Ice fields",
            _ if h < 1.5 => {
                if c.snow > 0.5 {
                    "Frozen shore"
                } else {
                    "Shore"
                }
            }
            _ if c.snow > 0.5 => {
                if woods > 0.3 {
                    "Snowy pines"
                } else {
                    "Snowfields"
                }
            }
            _ if c.desert > 0.5 => "Desert",
            _ if c.jungle > 0.5 => "Jungle",
            _ if h > self.settings.height_m as f32 * 0.42 => "Mountains",
            _ if woods > 0.45 => "Forest",
            _ if woods > 0.15 => "Woodland",
            _ if h > self.settings.height_m as f32 * 0.3 => "Hills",
            _ => "Meadow",
        };
        (kind.into(), format!("{:.0} m above the sea", h))
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Which way `dx` meters east and `dy` north is, as the points of the compass
fn way_of(dx: f32, dy: f32) -> &'static str {
    const WAYS: [&str; 8] = ["east", "north-east", "north", "north-west", "west", "south-west", "south", "south-east"];
    let a = dy.atan2(dx).rem_euclid(TAU);
    WAYS[((a / (PI / 4.0)).round() as usize) % 8]
}

/// A label on the map, as in the theme: `title` over `about` in a small dark panel over `at`, a
/// fine line down to it, kept inside `room`, `alpha` of the way opaque
pub fn label(painter: &Painter, at: Pos2, room: Rect, title: &str, about: &str, alpha: f32) {
    use crate::theme;
    let fade = |c: Color32| c.gamma_multiply(alpha);
    let heading = painter.layout_no_wrap(title.to_owned(), theme::display(15.0), fade(theme::HEADING));
    let under = painter.layout_no_wrap(about.to_owned(), FontId::proportional(12.0), fade(theme::WEAK));
    let size = vec2(heading.size().x.max(under.size().x) + 18.0, heading.size().y + if about.is_empty() { 0.0 } else { under.size().y + 2.0 } + 10.0);
    let lift = 30.0;
    let mut rect = Rect::from_center_size(at - vec2(0.0, lift + size.y * 0.5), size);
    // Kept in the room, and under the mark if there's no room over it
    if rect.top() < room.top() {
        rect = rect.translate(vec2(0.0, lift * 2.0 + size.y));
    }
    let rect = rect.translate(vec2((room.left() - rect.left()).max(0.0) + (room.right() - rect.right()).min(0.0), 0.0));
    let foot = if rect.center().y < at.y { rect.center_bottom() } else { rect.center_top() };
    painter.line_segment([at, pos2(at.x.clamp(rect.left() + 4.0, rect.right() - 4.0), foot.y)], Stroke::new(1.0f32, fade(Color32::from_white_alpha(150))));
    theme::diamond(painter, at, 2.5, fade(theme::ACCENT));
    painter.rect(rect, 3.0, fade(theme::panel_fill()), Stroke::new(1.0f32, fade(theme::EDGE)), StrokeKind::Inside);
    theme::accent(painter, rect, alpha);
    painter.galley(pos2(rect.center().x - heading.size().x * 0.5, rect.top() + 5.0), heading, Color32::WHITE);
    if !about.is_empty() {
        painter.galley(pos2(rect.center().x - under.size().x * 0.5, rect.bottom() - 5.0 - under.size().y), under, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As the game has them
    const SETTINGS: TerrainSettings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };

    fn wanted(seed: u32, at: [f32; 2]) -> Wanted {
        Wanted { settings: TerrainSettings { seed, ..SETTINGS }, at: Some(at), realm: None, caption: String::new(), saved: false }
    }

    #[test]
    fn the_map_is_round_where_the_player_is() {
        let at = [1500.0, 1500.0];
        // Its land quick enough to follow the seed box, what's on it coming after, see Backdrop
        let started = std::time::Instant::now();
        let land = build_land(&wanted(1, at));
        assert!(started.elapsed().as_millis() < 400, "took {:?}", started.elapsed());
        assert_eq!(land.player, at);
        let map = land.with_sites();
        assert_eq!(map.player, at);
        assert_eq!(map.land.across(), ACROSS);
        // Its middle column is the land there
        let (x, y) = map.land.middle_of((ACROSS as i32 / 2, ACROSS as i32 / 2));
        let [mx, my] = map.land.meters((x, y));
        assert!((mx - at[0] - CELL * 0.5).abs() < 1e-3 && (my - at[1] - CELL * 0.5).abs() < 1e-3);
        let there = worldgen::seen_from_above(&map.settings, mx, my).unwrap();
        assert!((map.land.height(ACROSS as i32 / 2, ACROSS as i32 / 2).unwrap() - there).abs() < 1e-3);
    }

    #[test]
    fn what_worldgen_places_is_marked_where_it_is() {
        // Round a castle and a few bandits' camps on the default seed
        let at = [1500.0, 1500.0];
        let map = build(&wanted(1, at));
        let rough = TerrainSettings { voxels_per_meter: 1, ..SETTINGS };
        let across = ACROSS as f32 * CELL;
        let terrain = worldgen::place_for(&rough, [(at[0] - across * 0.5) as i64, (at[1] - across * 0.5) as i64], [across as u32; 2], &RiverCache::default());
        let camps = terrain.bandit_camps();
        assert!(!camps.is_empty(), "camps round {at:?}");
        for camp in camps {
            let spot = [camp.fire[0], camp.fire[1]];
            let marked = map.places.iter().find(|p| matches!(p.kind, Kind::Camp(_)) && distance(p.at, spot) < 1.0).unwrap_or_else(|| panic!("no camp marked at {spot:?}"));
            assert_eq!(marked.kind, Kind::Camp(camp.kind));
            // And stood on its column
            let c = map.land.column(map.land.columns(spot)).unwrap();
            assert_eq!(map.land.stand(c), Stand::Camp);
            // And named when the mouse is over it
            let (title, about) = map.under(map.land.columns(spot));
            assert_eq!(title, "Bandit camp");
            assert!(about.starts_with(&Kind::Camp(camp.kind).about()), "{about}");
        }
        assert!(map.places.iter().any(|p| matches!(p.kind, Kind::Ruin("castle" | "watchtower"))), "{:?}", map.places);
    }

    #[test]
    fn another_seed_is_another_map() {
        let at = [195.0, 169.0];
        let (a, b) = (build(&wanted(1, at)), build(&wanted(2, at)));
        let n = ACROSS as i32;
        let differ = (0..n * n).filter(|k| a.land.height(k % n, k / n) != b.land.height(k % n, k / n)).count();
        assert!(differ > (n * n) as usize / 2, "only {differ} columns differ");
    }

    #[test]
    fn the_mouse_names_what_it_is_over() {
        let mut land = Land::flat_map_for_tests(12.0, [1000.0, 1000.0], ACROSS, CELL);
        let place = Place { kind: Kind::Town, name: Some("Ashford".into()), at: [1100.0, 1000.0] };
        stand_on(&mut land, &place);
        let map = Map { land, places: vec![place], far: Vec::new(), player: [1000.0, 1000.0], settings: SETTINGS, caption: String::new(), saved: false };
        // Over the town, or near enough its middle
        for off in [[0.0, 0.0], [12.0, -9.0], [-20.0, 15.0]] {
            let at = map.land.columns([1100.0 + off[0], 1000.0 + off[1]]);
            assert_eq!(map.under(at), ("Ashford".to_owned(), "town · 100 m east".to_owned()), "{off:?}");
        }
        // Its houses round its square
        let square = map.land.column(map.land.columns([1100.0, 1000.0])).unwrap();
        assert_eq!(map.land.stand(square), Stand::Square);
        assert_eq!(map.land.stand((square.0 + 1, square.1)), Stand::Houses);
        // Away from it, the land
        let (kind, about) = map.under(map.land.columns([1000.0, 1150.0]));
        assert_eq!(kind, "Meadow");
        assert_eq!(about, "12 m above the sea · 150 m north");
        // And where the player is
        assert_eq!(map.under(map.land.columns([1001.0, 1002.0])).0, "You set out here");
    }

    #[test]
    fn roads_are_drawn_and_towns_named_once_known() {
        let land = Land::flat_map_for_tests(12.0, [0.0, 0.0], ACROSS, CELL);
        let map = Map { land, places: vec![Place::new(Kind::Town, [60.0, 40.0]), Place::new(Kind::Castle, [-150.0, -100.0])], far: Vec::new(), player: [0.0, 0.0], settings: SETTINGS, caption: String::new(), saved: true };
        let far = Far {
            roads: vec![vec![[60.0, 40.0], [60.0, 200.0], [150.0, 200.0]]],
            named: vec![([62.0, 41.0], "Ashford".into(), false), ([-148.0, -99.0], "Ravencrag Castle".into(), true), ([3000.0, 0.0], "Frostholm".into(), false)],
            beyond: vec![Place { kind: Kind::Town, name: Some("Frostholm".into()), at: [3000.0, 0.0] }],
        };
        let named = map.with(&far);
        assert_eq!(named.places[0].name.as_deref(), Some("Ashford"));
        assert_eq!(named.places[1].name.as_deref(), Some("Ravencrag Castle"));
        assert_eq!(named.far.len(), 1);
        // The road along its way, but not over the town's square
        for p in [[60.0, 120.0], [60.0, 190.0], [100.0, 200.0]] {
            let c = named.land.column(named.land.columns(p)).unwrap();
            assert_eq!(named.land.stand(c), Stand::Road, "at {p:?}");
        }
        let square = named.land.column(named.land.columns([60.0, 40.0])).unwrap();
        assert_eq!(named.land.stand(square), Stand::Square);
    }

    #[test]
    fn the_towns_and_castles_are_called_what_the_signposts_call_them() {
        // Round where Free Roam starts, as far off as the roads are looked for
        let settings = TerrainSettings { voxels_per_meter: 1, ..SETTINGS };
        let middle = worldgen::FREE_ROAM_START;
        let across = ACROSS as f32 * CELL * FAR_ACROSS;
        let (roads, named) = roads_near(&settings, middle, across);
        assert!(!roads.is_empty() && !named.is_empty(), "{} roads, {} places", roads.len(), named.len());
        let wanted = Wanted { settings, at: Some(middle), realm: None, caption: String::new(), saved: false };
        let map = build(&wanted);
        let far = far(&map, &|| false).unwrap();
        let map = map.with(&far);
        // Those named on the map by the names worldgen gives them, the rest pointed to from its edge
        for (at, name, _) in &named {
            if let Some(place) = map.places.iter().find(|p| p.name.as_ref() == Some(name)) {
                assert!(distance(place.at, *at) < 70.0, "{name}");
                assert_eq!(map.under(map.land.columns(place.at)).0, *name);
            }
        }
        for place in &map.far {
            let name = place.name.as_ref().expect("named");
            assert!(named.iter().any(|(_, n, _)| n == name), "{name}");
            assert!(map.under_far(place).starts_with("the nearest "), "{}", map.under_far(place));
        }
        assert!(!map.far.is_empty() || map.places.iter().any(|p| p.name.is_some()));
    }

    #[test]
    fn realms_maps_mark_the_gate_and_the_raid_bosses() {
        let start = [195.0f64, 169.0];
        let wanted = Wanted {
            settings: TerrainSettings { seed: REALMS[0].seed, ..SETTINGS },
            at: None,
            realm: Some(Realm { realm: 0, start, ahead: [0.0, 1.0], progress: None }),
            caption: String::new(),
            saved: false,
        };
        let map = build(&wanted);
        let places = realms::Places::find(0, &wanted.settings, start, [0.0, 1.0]);
        // Round where a new journey sets out, in front of the gate
        assert!(distance(map.player, [places.arrival[0] as f32, places.arrival[1] as f32]) < 1e-3);
        let gate = map.places.iter().find(|p| p.kind == Kind::Gate).expect("the gate");
        assert!(distance(gate.at, [places.gate[0] as f32, places.gate[1] as f32]) < 1e-3);
        assert_eq!(map.under(map.land.columns(gate.at)).0, "The Innert Gate");
        // Those of its raid bosses near enough to be on it
        let half = ACROSS as f32 * CELL * 0.5;
        for (k, lair) in places.lairs.iter().enumerate() {
            let at = [lair[0] as f32, lair[1] as f32];
            let on = (at[0] - map.player[0]).abs() < half && (at[1] - map.player[1]).abs() < half;
            let marked = map.places.iter().any(|p| p.kind == Kind::RaidBoss(REALMS[0].raid_bosses[k].0.name(), REALMS[0].lair_names[k]));
            assert_eq!(on, marked, "raid boss {k} at {at:?}");
        }
    }

    #[test]
    fn ways_are_said_as_the_compass_has_them() {
        assert_eq!(way_of(10.0, 0.0), "east");
        assert_eq!(way_of(0.0, 10.0), "north");
        assert_eq!(way_of(-7.0, -7.0), "south-west");
        assert_eq!(way_of(7.0, -7.0), "south-east");
    }
}
