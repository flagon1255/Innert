//! Saving the game, so it goes on where it was left: one file a mode, Free Roam and Realms, in
//! %LOCALAPPDATA%\Innert\saves\, as JSON. Written after what matters, like a boss beaten or
//! something bought, every minute or so, and on the way out, see App::save_game().
//!
//! Each file is written to one beside it first and then put in its place, so a game stopped
//! partway through writing never leaves half a save. One that can't be read, or is from an older
//! Innert, is never loaded: it's put aside as `<name>.bad.json` and the mode starts afresh, saying
//! so, see read().
//!
//! It's made to grow: every part of it has `#[serde(default)]`, so what a save doesn't have comes
//! out as it would for a new game, and what it has that this build doesn't know is passed over. A
//! system with state of its own, like what the player builds, keeps it in a section of its own,
//! see Save::section() and Save::set_section(), or in a new field with `#[serde(default)]`; either
//! way older saves load as they are, with nothing of it yet, and VERSION needn't change. VERSION
//! only goes up when what's there already changes meaning, and saves from before can't be read
//! any more.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The version of what's written. Saves of an earlier one start afresh, see read().
pub const VERSION: u32 = 1;

/// The two ways to play, picked from the menu, each with a save of its own
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// The endless wilderness, any seed
    #[default]
    FreeRoam,
    /// A journey through the realms, from the normal world, see realms.rs
    Realms,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::FreeRoam, Mode::Realms];

    pub fn name(self) -> &'static str {
        match self {
            Mode::FreeRoam => "Free Roam",
            Mode::Realms => "Realms",
        }
    }

    /// For `--set mode=realms` and the like
    pub fn parse(name: &str) -> Option<Mode> {
        match name.to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
            "freeroam" | "free" => Some(Mode::FreeRoam),
            "realms" | "realm" => Some(Mode::Realms),
            _ => None,
        }
    }

    fn file_name(self) -> &'static str {
        match self {
            Mode::FreeRoam => "freeroam",
            Mode::Realms => "realms",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// A whole saved game
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Save {
    pub version: u32,
    pub mode: Mode,
    /// Of the landscape the player's in now
    pub seed: u32,
    /// Seconds since 1970 when it was written, and how long it's been played in all, in seconds
    pub saved_at: u64,
    pub played: f64,
    pub player: PlayerSave,
    pub carried: CarriedSave,
    /// Which world the player's in, of `worlds`: "normal", or a realm's, see realms.rs
    pub world: String,
    /// What's been done in each world that stays done: chests opened, bosses beaten and the like
    pub worlds: BTreeMap<String, WorldSave>,
    /// How far the journey through the realms has got, for Realms
    pub realms: Option<RealmsSave>,
    /// What other systems keep, each under its own name, see section()
    pub sections: BTreeMap<String, serde_json::Value>,
}

/// Where the player is and how they are
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct PlayerSave {
    /// Where their feet are, in meters from the corner of the whole landscape, or the camera when
    /// flying
    pub position: [f64; 3],
    /// Whether they're still to be stood on the dry ground nearest there, as for a new game saved
    /// before the world round its start was there to stand them on
    pub find_ground: bool,
    /// Which way they look, round and up, in radians
    pub yaw: f64,
    pub pitch: f64,
    pub flying: bool,
    pub health: f64,
    pub hunger: f64,
    pub mana: f64,
}

impl Default for PlayerSave {
    fn default() -> Self {
        PlayerSave { position: [0.0; 3], find_ground: true, yaw: 0.0, pitch: 0.0, flying: false, health: 100.0, hunger: 80.0, mana: 100.0 }
    }
}

/// What the player carries, in the order it's in, and which slot's in hand
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct CarriedSave {
    pub gold: u32,
    pub selected: usize,
    /// Cubic meters of water in the bucket
    pub bucket: f64,
    pub slots: Vec<SlotSave>,
    /// The wings worn on the back, by their kind's id, see wings::KINDS
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worn: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SlotSave {
    pub item: ItemSave,
    #[serde(default = "one")]
    pub count: u32,
}

fn one() -> u32 {
    1
}

/// Something carried, or on a merchant's stall, by what it is rather than where it's kept, so
/// saves stay good as weapons and ammo are added
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemSave {
    Bucket,
    Apple,
    Meat,
    Bread,
    Potion,
    Gold {
        amount: u32,
    },
    Weapon(WeaponSave),
    /// Special ammo of Library::ammo, by its id, and how many rounds
    Ammo {
        id: String,
        rounds: u32,
    },
    /// An Innert block a raid boss of realm `realm` left, see realms.rs
    InnertBlock {
        realm: u8,
    },
    /// The grappling hook of the sky isles, and wings of kind `of`, by its id in wings::KINDS: the
    /// plain ones where it's not said, as in saves from before there were kinds of them
    Hook,
    Wings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        of: Option<String>,
    },
    /// Something from a later Innert this one doesn't know: left out
    #[serde(other)]
    Unknown,
}

/// A weapon by what it's made of: its family's id, each of its parts', and the seed it was made
/// with, all it takes to make it again, see Weapon::assemble()
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(default)]
pub struct WeaponSave {
    pub family: String,
    pub parts: Vec<String>,
    pub seed: u32,
    /// What a weapon like no other is called, as the Coil Unending's fang, see
    /// serpents::FANG: the rest are called what their parts make them
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// What's been done in one world that stays done, each by where it is in the whole landscape, as
/// Game keeps them
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct WorldSave {
    pub opened: Vec<[i64; 3]>,
    pub defeated: Vec<[i64; 3]>,
    pub cleared: Vec<[i64; 3]>,
    /// What each merchant looked at has left on their stall
    pub stocks: Vec<StockSave>,
    /// Holes blown in the land, in meters from its corner, and how big, and stains splashed on it,
    /// how big, of what, and whether they burnt what grew there
    pub craters: Vec<([f64; 3], f64)>,
    pub stains: Vec<([f64; 3], f64, u8, bool)>,
    /// What other systems keep of this world alone, like what's been built in it, each under its
    /// own name, see WorldSave::section()
    pub sections: BTreeMap<String, serde_json::Value>,
}

impl WorldSave {
    /// What another system keeps of this world under `name`, if it's there and can be read
    #[allow(dead_code)]
    pub fn section<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        serde_json::from_value(self.sections.get(name)?.clone()).ok()
    }

    #[allow(dead_code)]
    pub fn set_section<T: Serialize>(&mut self, name: &str, value: &T) {
        if let Ok(value) = serde_json::to_value(value) {
            self.sections.insert(name.to_owned(), value);
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct StockSave {
    pub key: [i64; 3],
    pub wares: Vec<WareSave>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WareSave {
    Weapon(WeaponSave),
    /// Food, potions or bundles of ammo, and how many are left
    Goods { item: ItemSave, left: u32 },
    #[serde(other)]
    Unknown,
}

/// How far a journey through the realms has got. Realm 0 is the normal world, see realms.rs.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct RealmsSave {
    /// The realm the player's in
    pub realm: u32,
    /// Those that can be travelled to, and those that have been
    pub unlocked: Vec<u32>,
    pub visited: Vec<u32>,
    /// How many Innert blocks have been set into the Teleporter for each realm, by its number
    pub placed: Vec<u32>,
    /// Times the journey's been made through to the end
    pub completed: u32,
    /// Where the Innert Gate stands in the normal world. A game from before it was built by the
    /// player has it standing where it was found.
    #[serde(default = "GateSave::found")]
    pub gate: GateSave,
    /// The portals home the player's built in the realms past the normal world, one a realm
    pub portals: Vec<PortalSave>,
    /// Whether the old foundation stones are laid where the gate once stood, see session.rs
    pub ruins: bool,
}

impl Default for RealmsSave {
    fn default() -> Self {
        RealmsSave { realm: 0, unlocked: vec![0], visited: vec![0], placed: Vec::new(), completed: 0, gate: GateSave::Unbuilt, portals: Vec::new(), ruins: false }
    }
}

/// Where the Innert Gate stands, see realms.rs. What's been set into it is kept with the journey,
/// not with it, so building it anew elsewhere moves it with all that.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GateSave {
    /// Not built yet, as in a new game
    Unbuilt,
    /// Where it stood when it was found, before the player could build it
    Found,
    /// Built by the player: where its foot is, in meters from the corner of the whole landscape,
    /// and which way its front faces, in radians
    Built { at: [f64; 2], facing: f64 },
}

impl GateSave {
    fn found() -> GateSave {
        GateSave::Found
    }
}

/// A portal home the player's built in realm `realm`, where it is as for GateSave::Built
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct PortalSave {
    pub realm: u32,
    pub at: [f64; 2],
    pub facing: f64,
}

impl Save {
    /// What other systems keep under `name`, if this save has it and it can be read; None
    /// otherwise, for them to start as in a new game
    #[allow(dead_code)]
    pub fn section<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        serde_json::from_value(self.sections.get(name)?.clone()).ok()
    }

    /// Keeps `value` under `name`, in place of what was there
    #[allow(dead_code)]
    pub fn set_section<T: Serialize>(&mut self, name: &str, value: &T) {
        if let Ok(value) = serde_json::to_value(value) {
            self.sections.insert(name.to_owned(), value);
        }
    }

    /// The world the player's in
    pub fn current_world(&self) -> WorldSave {
        self.worlds.get(&self.world).cloned().unwrap_or_default()
    }

    /// Mends what a hand-edited or damaged save might have that the game can't take: numbers
    /// that aren't, or out of their range
    fn mend(&mut self) {
        let p = &mut self.player;
        if !p.position.iter().all(|v| v.is_finite()) {
            p.position = [0.0; 3];
            p.find_ground = true;
        }
        let finite = |v: f64, default: f64| if v.is_finite() { v } else { default };
        p.yaw = finite(p.yaw, 0.0);
        p.pitch = finite(p.pitch, 0.0).clamp(-1.5, 1.5);
        p.health = finite(p.health, 100.0).clamp(1.0, 100.0);
        p.hunger = finite(p.hunger, 80.0).clamp(0.0, 100.0);
        p.mana = finite(p.mana, 100.0).max(0.0);
        self.carried.bucket = finite(self.carried.bucket, 0.0).max(0.0);
        self.played = finite(self.played, 0.0).max(0.0);
        if self.world.is_empty() {
            self.world = "normal".to_owned();
        }
        for world in self.worlds.values_mut() {
            world.craters.retain(|(c, r)| c.iter().all(|v| v.is_finite()) && r.is_finite() && *r > 0.0);
            world.stains.retain(|(c, r, ..)| c.iter().all(|v| v.is_finite()) && r.is_finite() && *r > 0.0);
        }
    }
}

/// What's in one mode's save file
#[derive(Clone, Debug)]
pub enum Slot {
    Empty,
    Saved(Box<Save>),
    /// There was one, but it couldn't be read, for this reason: it's been put aside
    Unreadable(String),
}

/// Takes what can be read of `bytes`, as written for `mode`: an error saying why not if nothing
pub fn parse(bytes: &[u8], mode: Mode) -> Result<Save, String> {
    let mut save: Save = serde_json::from_slice(bytes).map_err(|e| format!("it's damaged ({e})"))?;
    if save.version < VERSION {
        return Err(format!("it's from an older version of Innert (save version {})", save.version));
    }
    if save.mode != mode {
        return Err(format!("it's a {} save", save.mode.name()));
    }
    save.mend();
    Ok(save)
}

/// Where saves are kept, one file a mode, and writes them in the background
pub struct Saves {
    /// None where nothing's saved, as in screenshot runs that weren't given a folder
    dir: Option<PathBuf>,
    /// What each mode's file had when last looked at
    slots: [Slot; 2],
    writing: Option<std::thread::JoinHandle<()>>,
}

impl Saves {
    /// In `dir`, or in %LOCALAPPDATA%\Innert\saves\ for None with `default` set, otherwise
    /// nowhere, with what's there read
    pub fn new(dir: Option<PathBuf>, default: bool) -> Self {
        let dir = dir.or_else(|| default.then(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Innert").join("saves")));
        let mut saves = Saves { dir, slots: [Slot::Empty, Slot::Empty], writing: None };
        for mode in Mode::ALL {
            saves.slots[mode.index()] = saves.path(mode).map_or(Slot::Empty, |path| read(&path, mode));
        }
        saves
    }

    pub fn enabled(&self) -> bool {
        self.dir.is_some()
    }

    fn path(&self, mode: Mode) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(format!("{}.json", mode.file_name())))
    }

    pub fn slot(&self, mode: Mode) -> &Slot {
        &self.slots[mode.index()]
    }

    /// The save of `mode` there is, if there is one
    pub fn saved(&self, mode: Mode) -> Option<&Save> {
        match self.slot(mode) {
            Slot::Saved(save) => Some(save),
            _ => None,
        }
    }

    /// Writes `save` in the background, after any being written already, or straight away with
    /// `wait`, as on the way out
    pub fn write(&mut self, save: Save, wait: bool) {
        let Some(path) = self.path(save.mode) else {
            return;
        };
        if let Some(writing) = self.writing.take() {
            let _ = writing.join();
        }
        let mode = save.mode;
        let bytes = match serde_json::to_vec_pretty(&save) {
            Ok(bytes) => bytes,
            Err(e) => {
                println!("The {} game couldn't be saved: {e}", mode.name());
                return;
            }
        };
        self.slots[mode.index()] = Slot::Saved(Box::new(save));
        let job = move || {
            if let Err(e) = write_file(&path, &bytes) {
                println!("The {} game couldn't be saved to {}: {e}", mode.name(), path.display());
            }
        };
        if wait {
            job();
        } else {
            self.writing = std::thread::Builder::new().name("saving".into()).spawn(job).ok();
        }
    }

    /// Waits for what's being written
    pub fn finish(&mut self) {
        if let Some(writing) = self.writing.take() {
            let _ = writing.join();
        }
    }
}

impl Drop for Saves {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Reads the save of `mode` at `path`. One that can't be read is put aside, as `<name>.bad.json`,
/// so it's not lost but nor is it in the way.
pub fn read(path: &Path, mode: Mode) -> Slot {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Slot::Empty,
        Err(e) => return Slot::Unreadable(format!("it couldn't be opened ({e})")),
    };
    match parse(&bytes, mode) {
        Ok(save) => Slot::Saved(Box::new(save)),
        Err(why) => {
            let aside = path.with_extension("bad.json");
            let _ = std::fs::rename(path, &aside);
            println!("The {} save at {} can't be used: {why}. Put aside as {}", mode.name(), path.display(), aside.display());
            Slot::Unreadable(why)
        }
    }
}

/// Writes `bytes` to a file beside `path`, then puts it in place of whatever's there, so there's
/// always either the old save or the new one whole
fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("json.partial");
    {
        let mut file = std::fs::File::create(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&partial, path)
}

/// Seconds since 1970
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// How long ago a save was written, said simply
pub fn ago(saved_at: u64) -> String {
    let seconds = now().saturating_sub(saved_at);
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} days ago", seconds / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Save {
        let mut worlds = BTreeMap::new();
        worlds.insert(
            "normal".to_owned(),
            WorldSave {
                opened: vec![[1, 2, 3]],
                defeated: vec![[40, -50, 60]],
                cleared: vec![[7, 8, 9], [-1, -2, -3]],
                stocks: vec![StockSave {
                    key: [5, 5, 12],
                    wares: vec![WareSave::Weapon(WeaponSave { family: "sword".into(), parts: vec!["a".into(), "b".into()], seed: 99, name: None }), WareSave::Goods { item: ItemSave::Bread, left: 4 }],
                }],
                craters: vec![([10.0, 20.5, 30.25], 2.5)],
                stains: vec![([1.0, 2.0, 3.0], 1.5, 7, true)],
                sections: BTreeMap::new(),
            },
        );
        worlds.get_mut("normal").unwrap().set_section("doors", &vec![(1, true)]);
        let mut save = Save {
            version: VERSION,
            mode: Mode::Realms,
            seed: 1,
            saved_at: 1_700_000_000,
            played: 321.5,
            player: PlayerSave { position: [112.5, -40.25, 22.0], find_ground: false, yaw: 1.25, pitch: -0.3, flying: false, health: 37.0, hunger: 55.5, mana: 80.0 },
            carried: CarriedSave {
                gold: 1234,
                selected: 2,
                bucket: 0.5,
                slots: vec![
                    SlotSave { item: ItemSave::Weapon(WeaponSave { family: "bow".into(), parts: vec!["x".into()], seed: 7, name: Some("Fang of Ysveth".into()) }), count: 1 },
                    SlotSave { item: ItemSave::Ammo { id: "broadhead_arrows".into(), rounds: 12 }, count: 1 },
                    SlotSave { item: ItemSave::Apple, count: 3 },
                    SlotSave { item: ItemSave::Bucket, count: 1 },
                ],
                worn: Some("gull".into()),
            },
            world: "normal".into(),
            worlds,
            realms: Some(RealmsSave { realm: 0, unlocked: vec![0, 1], visited: vec![0, 1], placed: vec![3], completed: 0, ..Default::default() }),
            sections: BTreeMap::new(),
        };
        save.set_section("bases", &vec![[1, 2, 3]]);
        save
    }

    #[test]
    fn a_save_comes_back_the_same() {
        let save = sample();
        let bytes = serde_json::to_vec_pretty(&save).unwrap();
        assert_eq!(parse(&bytes, Mode::Realms).unwrap(), save);
        // And through a file, written beside it first then put in place
        let dir = std::env::temp_dir().join(format!("innert-save-test-{}", std::process::id()));
        let path = dir.join("realms.json");
        write_file(&path, &bytes).unwrap();
        write_file(&path, &bytes).unwrap();
        match read(&path, Mode::Realms) {
            Slot::Saved(read) => assert_eq!(*read, save),
            other => panic!("read back {other:?}"),
        }
        assert!(!path.with_extension("json.partial").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_save_is_put_aside_not_loaded() {
        let dir = std::env::temp_dir().join(format!("innert-bad-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("freeroam.json");
        let good = serde_json::to_vec(&Save { version: VERSION, ..Default::default() }).unwrap();
        // Garbage, a save cut off partway, one of the wrong mode, one from before saves had
        // versions, and nothing at all
        let half = good[..good.len() / 2].to_vec();
        let realms = serde_json::to_vec(&Save { version: VERSION, mode: Mode::Realms, ..Default::default() }).unwrap();
        for bad in [b"\x00\xffnot json at all".to_vec(), half, realms, b"{\"seed\": 4}".to_vec(), Vec::new(), b"[1, 2, 3]".to_vec()] {
            std::fs::write(&path, &bad).unwrap();
            assert!(matches!(read(&path, Mode::FreeRoam), Slot::Unreadable(_)), "{}", String::from_utf8_lossy(&bad));
            assert!(!path.exists(), "put aside");
            assert!(path.with_extension("bad.json").exists());
        }
        assert!(matches!(read(&path, Mode::FreeRoam), Slot::Empty));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saves_take_what_they_dont_know_and_lack_in_their_stride() {
        // From a later Innert: a field, a section, an item and a ware this one doesn't know
        let later = r#"{
            "version": 7, "mode": "free_roam", "seed": 3, "weather": "rain",
            "player": {"position": [1.0, 2.0, 3.0], "wings": true},
            "carried": {"gold": 5, "slots": [{"item": {"kind": "apple"}, "count": 2}, {"item": {"kind": "door", "oak": true}}]},
            "worlds": {"normal": {"defeated": [[1, 2, 3]], "stocks": [{"key": [0, 0, 7], "wares": [{"kind": "spell", "x": 1}]}]}},
            "sections": {"bases": {"blocks": 12}}
        }"#;
        let save = parse(later.as_bytes(), Mode::FreeRoam).unwrap();
        assert_eq!(save.seed, 3);
        assert_eq!(save.player.position, [1.0, 2.0, 3.0]);
        assert_eq!(save.player.health, 100.0, "what's not there comes out as in a new game");
        assert_eq!(save.carried.slots[0], SlotSave { item: ItemSave::Apple, count: 2 });
        assert_eq!(save.carried.slots[1].item, ItemSave::Unknown);
        assert_eq!(save.current_world().defeated, vec![[1, 2, 3]]);
        assert_eq!(save.current_world().stocks[0].wares, vec![WareSave::Unknown]);
        assert!(save.realms.is_none());
        // Another system's section is kept as it was, through saving again
        let again = parse(&serde_json::to_vec(&save).unwrap(), Mode::FreeRoam).unwrap();
        assert_eq!(again.sections["bases"]["blocks"], 12);
        #[derive(Deserialize)]
        struct Bases {
            blocks: u32,
        }
        assert_eq!(again.section::<Bases>("bases").map(|b| b.blocks), Some(12));
        assert!(again.section::<Bases>("doors").is_none());
    }

    /// Wings saved before there were kinds of them, and before they were worn, still load, as the
    /// plain ones, and those of a kind come back that kind, worn or not
    #[test]
    fn wings_from_before_kinds_still_load() {
        let old: CarriedSave = serde_json::from_str(r#"{"gold": 5, "slots": [{"item": {"kind": "wings"}}]}"#).unwrap();
        assert_eq!(old.slots[0].item, ItemSave::Wings { of: None });
        assert_eq!(old.worn, None);
        let carried = CarriedSave { worn: Some("rimeglass".into()), slots: vec![SlotSave { item: ItemSave::Wings { of: Some("veilmoth".into()) }, count: 1 }], ..Default::default() };
        let again: CarriedSave = serde_json::from_slice(&serde_json::to_vec(&carried).unwrap()).unwrap();
        assert_eq!(again, carried);
    }

    #[test]
    fn mended_numbers_stay_in_their_range() {
        let mut save = Save { version: VERSION, ..Default::default() };
        save.player.health = 1e9;
        save.player.pitch = 9.0;
        save.player.hunger = -4.0;
        let save = parse(&serde_json::to_vec(&save).unwrap(), Mode::FreeRoam).unwrap();
        assert_eq!(save.player.health, 100.0);
        assert_eq!(save.player.pitch, 1.5);
        assert_eq!(save.player.hunger, 0.0);
        assert_eq!(save.world, "normal");
    }
}
