//! The keys and mouse buttons the game is played with, in one table: every action the player
//! takes, each on up to two keys or buttons, the ones it's always had to begin with. What's
//! pressed in play is read through it, see App::pressed() and App::held(), and the prompts on
//! screen name the keys from it, see key(), so they say what's really bound.
//!
//! Kept from one run to the next in %LOCALAPPDATA%\Innert\keys.txt, a line an action, as
//! `jump = Space`. A line that can't be read leaves that action as it was to begin with, and no
//! file at all is every one of them so.
//!
//! The Controls window, opened from the menu or the settings, lists them all, grouped, each with
//! what it does, and changes one on a click: the next key or mouse button pressed takes its place,
//! Esc leaving it as it was, see Keys::window(). Should another action that can be used at the
//! same time be on that key, the two swap, and what's become of it is said.
//!
//! Esc always opens the menu and closes windows, and F12 always takes a screenshot, so neither
//! can be bound to anything else; the model editor's own keys are its own, see editor.rs.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use egui_winit_vulkano::egui::{self, Align, Color32, Layout, RichText, Stroke};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;
use winit_input_helper::WinitInputHelper;

use crate::theme;

/// A key, or a button of the mouse
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Input {
    Key(KeyCode),
    Mouse(MouseButton),
}

use Input::{Key, Mouse};

/// The keys that can be bound, by the names they're shown and kept by
const KEYS: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "A"),
    (KeyCode::KeyB, "B"),
    (KeyCode::KeyC, "C"),
    (KeyCode::KeyD, "D"),
    (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"),
    (KeyCode::KeyG, "G"),
    (KeyCode::KeyH, "H"),
    (KeyCode::KeyI, "I"),
    (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"),
    (KeyCode::KeyL, "L"),
    (KeyCode::KeyM, "M"),
    (KeyCode::KeyN, "N"),
    (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"),
    (KeyCode::KeyQ, "Q"),
    (KeyCode::KeyR, "R"),
    (KeyCode::KeyS, "S"),
    (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"),
    (KeyCode::KeyV, "V"),
    (KeyCode::KeyW, "W"),
    (KeyCode::KeyX, "X"),
    (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"),
    (KeyCode::Digit1, "1"),
    (KeyCode::Digit2, "2"),
    (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
    (KeyCode::Digit0, "0"),
    (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="),
    (KeyCode::Space, "Space"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::CapsLock, "Caps Lock"),
    (KeyCode::ShiftLeft, "Left Shift"),
    (KeyCode::ShiftRight, "Right Shift"),
    (KeyCode::ControlLeft, "Left Ctrl"),
    (KeyCode::ControlRight, "Right Ctrl"),
    (KeyCode::AltLeft, "Left Alt"),
    (KeyCode::AltRight, "Right Alt"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::Backquote, "`"),
    (KeyCode::BracketLeft, "["),
    (KeyCode::BracketRight, "]"),
    (KeyCode::Backslash, "\\"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Quote, "'"),
    // Not "," as it's shown, the commas in the file being what's between an action's keys
    (KeyCode::Comma, "Comma"),
    (KeyCode::Period, "."),
    (KeyCode::Slash, "/"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"),
    (KeyCode::ArrowUp, "Up"),
    (KeyCode::ArrowDown, "Down"),
    (KeyCode::ArrowLeft, "Left"),
    (KeyCode::ArrowRight, "Right"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "Page Up"),
    (KeyCode::PageDown, "Page Down"),
    (KeyCode::Numpad0, "Num 0"),
    (KeyCode::Numpad1, "Num 1"),
    (KeyCode::Numpad2, "Num 2"),
    (KeyCode::Numpad3, "Num 3"),
    (KeyCode::Numpad4, "Num 4"),
    (KeyCode::Numpad5, "Num 5"),
    (KeyCode::Numpad6, "Num 6"),
    (KeyCode::Numpad7, "Num 7"),
    (KeyCode::Numpad8, "Num 8"),
    (KeyCode::Numpad9, "Num 9"),
    (KeyCode::NumpadAdd, "Num +"),
    (KeyCode::NumpadSubtract, "Num -"),
    (KeyCode::NumpadMultiply, "Num *"),
    (KeyCode::NumpadDivide, "Num /"),
    (KeyCode::NumpadDecimal, "Num ."),
    (KeyCode::NumpadEnter, "Num Enter"),
];

/// The mouse's buttons that can be bound, by their names
const BUTTONS: &[(MouseButton, &str)] = &[
    (MouseButton::Left, "Left click"),
    (MouseButton::Right, "Right click"),
    (MouseButton::Middle, "Middle click"),
    (MouseButton::Back, "Mouse 4"),
    (MouseButton::Forward, "Mouse 5"),
];

/// Keys that are what they are whatever's bound: Esc for the menu, F12 for a screenshot
const KEPT: [(KeyCode, &str); 2] = [(KeyCode::Escape, "Esc"), (KeyCode::F12, "F12")];

impl Input {
    /// Its name, as it's shown in the Controls window and kept in the file
    pub fn name(self) -> String {
        match self {
            Key(k) => KEYS.iter().chain(KEPT.iter()).find(|(c, _)| *c == k).map_or_else(|| format!("{k:?}"), |(_, n)| (*n).to_owned()),
            Mouse(b) => BUTTONS.iter().find(|(c, _)| *c == b).map_or_else(|| format!("{b:?}"), |(_, n)| (*n).to_owned()),
        }
    }

    /// Its name as the prompts on screen say it: either Shift is Shift, and a left click a click
    pub fn short(self) -> String {
        match self {
            Key(KeyCode::ShiftLeft | KeyCode::ShiftRight) => "Shift".into(),
            Key(KeyCode::ControlLeft | KeyCode::ControlRight) => "Ctrl".into(),
            Key(KeyCode::AltLeft | KeyCode::AltRight) => "Alt".into(),
            Key(KeyCode::Comma) => ",".into(),
            Mouse(MouseButton::Left) => "Click".into(),
            _ => self.name(),
        }
    }

    /// Its name in two letters at most, for the little tab on a slot of the quick bar
    pub fn tiny(self) -> String {
        match self {
            Mouse(MouseButton::Left) => "LM".into(),
            Mouse(MouseButton::Right) => "RM".into(),
            Mouse(MouseButton::Middle) => "MM".into(),
            Mouse(MouseButton::Back) => "M4".into(),
            Mouse(MouseButton::Forward) => "M5".into(),
            Key(KeyCode::Space) => "Sp".into(),
            _ => {
                let name = self.short();
                match name.strip_prefix("Num ") {
                    Some(rest) => format!("n{}", rest.chars().next().unwrap_or(' ')),
                    None => name.chars().take(2).collect(),
                }
            }
        }
    }

    /// Read back from its name, as kept in the file, or as winit calls it
    pub fn parse(text: &str) -> Option<Input> {
        let text = text.trim();
        let same = |n: &str| n.eq_ignore_ascii_case(text);
        if let Some((k, _)) = KEYS.iter().find(|(k, n)| same(n) || same(&format!("{k:?}"))) {
            return Some(Key(*k));
        }
        if text == "," {
            return Some(Key(KeyCode::Comma));
        }
        BUTTONS.iter().find(|(b, n)| same(n) || same(&format!("Mouse{b:?}"))).map(|(b, _)| Mouse(*b))
    }

    /// The key egui knows it by, for what's read in the GUI's own windows, as the inventory's
    /// slots: a mouse button has none
    pub fn egui_key(self) -> Option<egui::Key> {
        let Key(k) = self else {
            return None;
        };
        let name = format!("{k:?}");
        egui::Key::from_name(name.strip_prefix("Key").filter(|l| l.len() == 1).unwrap_or(&name))
    }
}

/// What's pressed and held, as the game reads it: winit's, or made up in the tests
pub trait Buttons {
    fn key_pressed(&self, key: KeyCode) -> bool;
    fn key_held(&self, key: KeyCode) -> bool;
    fn mouse_pressed(&self, button: MouseButton) -> bool;
    fn mouse_held(&self, button: MouseButton) -> bool;
}

impl Buttons for WinitInputHelper {
    fn key_pressed(&self, key: KeyCode) -> bool {
        WinitInputHelper::key_pressed(self, key)
    }
    fn key_held(&self, key: KeyCode) -> bool {
        WinitInputHelper::key_held(self, key)
    }
    fn mouse_pressed(&self, button: MouseButton) -> bool {
        WinitInputHelper::mouse_pressed(self, button)
    }
    fn mouse_held(&self, button: MouseButton) -> bool {
        WinitInputHelper::mouse_held(self, button)
    }
}

/// When an action can be used, for which others it can't share a key with: one that's always
/// there shares with none, one with what's in hand not with another of those, but with one while
/// building, as those two never are at once, nor with one while flying
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum When {
    Always,
    /// Not while building: what's in hand, and what's about
    Hands,
    Building,
    Flying,
}

impl When {
    fn meets(self, other: When) -> bool {
        self == other || self == When::Always || other == When::Always
    }
}

/// The groups the Controls window lists them in
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Moving,
    Fighting,
    Magic,
    Items,
    Wings,
    Building,
    Menus,
}

impl Group {
    pub const ALL: [Group; 7] = [Group::Moving, Group::Fighting, Group::Magic, Group::Items, Group::Wings, Group::Building, Group::Menus];

    pub fn title(self) -> &'static str {
        match self {
            Group::Moving => "Moving",
            Group::Fighting => "Fighting",
            Group::Magic => "Magic and powers",
            Group::Items => "Items and trading",
            Group::Wings => "Wings",
            Group::Building => "Gathering and building",
            Group::Menus => "Menus",
        }
    }
}

/// Something the player does, on a key or a mouse button
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Run,
    Fly,
    FlyUp,
    FlyDown,
    Attack,
    Heavy,
    Reload,
    Ammo,
    Special,
    Interact,
    Inventory,
    /// The quick bar's slots, from the first
    Hotbar(u8),
    Gather,
    Build,
    Place,
    TakeDown,
    Turn,
    Mend,
    NextTab,
    /// The pieces of the tab open while building, from the first
    Piece(u8),
    Map,
    Skills,
    Settings,
}

use Action::*;

/// How many slots of the quick bar have keys, and pieces while building
pub const HOTBAR_KEYS: usize = 9;
pub const PIECE_KEYS: usize = 11;

/// Every action, in the order they're listed and kept
pub const ALL: [Action; 46] = [
    Forward, Back, Left, Right, Jump, Run, Fly, FlyUp, FlyDown,
    Attack, Heavy, Reload, Ammo,
    Special,
    Interact, Inventory,
    Hotbar(0), Hotbar(1), Hotbar(2), Hotbar(3), Hotbar(4), Hotbar(5), Hotbar(6), Hotbar(7), Hotbar(8),
    Gather, Build, Place, TakeDown, Turn, Mend, NextTab,
    Piece(0), Piece(1), Piece(2), Piece(3), Piece(4), Piece(5), Piece(6), Piece(7), Piece(8), Piece(9), Piece(10),
    Map, Skills, Settings,
];

/// The digits along the top of the keyboard, the quick bar's and the pieces' keys to begin with
const DIGITS: [KeyCode; PIECE_KEYS] = [
    KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
    KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9, KeyCode::Digit0, KeyCode::Minus,
];

impl Action {
    fn index(self) -> usize {
        ALL.iter().position(|&a| a == self).unwrap_or(0)
    }

    /// What it's called in the file
    pub fn id(self) -> String {
        match self {
            Hotbar(k) => format!("hotbar{}", k + 1),
            Piece(k) => format!("piece{}", k + 1),
            FlyUp => "fly_up".into(),
            FlyDown => "fly_down".into(),
            TakeDown => "take_down".into(),
            NextTab => "next_tab".into(),
            a => format!("{a:?}").to_lowercase(),
        }
    }

    /// The action called `id` in the file
    pub fn by_id(id: &str) -> Option<Action> {
        ALL.iter().copied().find(|a| a.id().eq_ignore_ascii_case(id.trim()))
    }

    /// What it's called in plain words
    pub fn name(self) -> String {
        match self {
            Forward => "Walk forward",
            Back => "Walk back",
            Left => "Step left",
            Right => "Step right",
            Jump => "Jump",
            Run => "Run",
            Fly => "Fly, or walk again",
            FlyUp => "Fly up",
            FlyDown => "Fly down",
            Attack => "Attack, or use",
            Heavy => "Heavy swing, aim, or eat",
            Reload => "Reload",
            Ammo => "Change ammo",
            Special => "Special",
            Interact => "Pick up, open, trade, pet",
            Inventory => "Inventory",
            Hotbar(k) => return format!("Quick bar {}", k + 1),
            Gather => "Gather",
            Build => "Build",
            Place => "Build the piece",
            TakeDown => "Take it down",
            Turn => "Turn the piece",
            Mend => "Mend",
            NextTab => "Next tab of pieces",
            Piece(k) => return format!("Piece {}", k + 1),
            Map => "Map",
            Skills => "Skill web",
            Settings => "Settings",
        }
        .into()
    }

    /// What it does, said under its name
    pub fn hint(self) -> &'static str {
        match self {
            Forward | Back | Left | Right => "",
            Jump => "Swim up in water, and let go of a grappling hook's rope. After falling, rise again",
            Run => "Hold while you move. Flying, it goes faster",
            Fly => "Free flight over the world, then back on foot",
            FlyUp | FlyDown => "While flying",
            Attack => "Click swing or cast; crossbows and guns shoot. Three swings in a row make a combo. Fills a bucket, throws a grappling hook",
            Heavy => "A heavy swing, or eat what's in hand. A bow: hold to draw, then click to shoot. Pours a bucket out",
            Reload => "Crossbow or gun: loads it early, as the heavy button does",
            Ammo => "What a bow, crossbow or gun shoots",
            Special => "A legendary weapon's special, once its bar's full",
            Interact => "Talk to villagers, open doors and chests, light and feed a fire, smelt at a furnace, use the gate",
            Inventory => "Craft tools there (Inventory › Craft), and change wings",
            Hotbar(_) => "Picks what's in hand. Over a slot in the inventory, puts it on the quick bar",
            Gather => "Hold at a tree, rock or clay to gather it. Bare hands work; tools are 2–4× faster (Inventory › Craft)",
            Build => "Start or stop building. Each piece's cost is beside it",
            Place => "While building",
            TakeDown => "What you built, getting all of it back",
            Turn => "The piece about to be built",
            Mend => "What's damaged, while building",
            NextTab => "With Run held, the tab before",
            Piece(_) => "Picks a piece of the tab, its cost beside it; the mouse wheel goes through them too",
            Map => "Again to close it",
            Skills => "Spend the points that going up levels gives",
            Settings => "Quality, the view, light and sound",
        }
    }

    pub fn when(self) -> When {
        match self {
            Attack | Heavy | Reload | Special | Interact | Hotbar(_) => When::Hands,
            Place | TakeDown | Turn | Mend | NextTab | Piece(_) => When::Building,
            FlyUp | FlyDown => When::Flying,
            _ => When::Always,
        }
    }

    pub fn group(self) -> Group {
        match self {
            Forward | Back | Left | Right | Jump | Run | Fly | FlyUp | FlyDown => Group::Moving,
            Attack | Heavy | Reload | Ammo => Group::Fighting,
            Special => Group::Magic,
            Interact | Inventory | Hotbar(_) => Group::Items,
            Gather | Build | Place | TakeDown | Turn | Mend | NextTab | Piece(_) => Group::Building,
            Map | Skills | Settings => Group::Menus,
        }
    }

    /// The keys it's had all along
    pub fn defaults(self) -> [Option<Input>; 2] {
        let key = |k| [Some(Key(k)), None];
        match self {
            Forward => key(KeyCode::KeyW),
            Back => key(KeyCode::KeyS),
            Left => key(KeyCode::KeyA),
            Right => key(KeyCode::KeyD),
            Jump => key(KeyCode::Space),
            Run => [Some(Key(KeyCode::ShiftLeft)), Some(Key(KeyCode::ShiftRight))],
            Fly => key(KeyCode::KeyF),
            FlyUp => key(KeyCode::KeyQ),
            FlyDown => key(KeyCode::KeyE),
            Attack => [Some(Mouse(MouseButton::Left)), None],
            Heavy => [Some(Mouse(MouseButton::Right)), None],
            Reload => key(KeyCode::KeyQ),
            Ammo => key(KeyCode::KeyT),
            Special => [Some(Key(KeyCode::KeyR)), Some(Mouse(MouseButton::Middle))],
            Interact => key(KeyCode::KeyE),
            Inventory => [Some(Key(KeyCode::KeyI)), Some(Key(KeyCode::Tab))],
            Hotbar(k) => key(DIGITS[k as usize]),
            Gather => key(KeyCode::KeyG),
            Build => key(KeyCode::KeyB),
            Place => [Some(Mouse(MouseButton::Left)), None],
            TakeDown => [Some(Mouse(MouseButton::Right)), None],
            Turn => key(KeyCode::KeyR),
            Mend => key(KeyCode::KeyE),
            NextTab => key(KeyCode::KeyC),
            Piece(k) => key(DIGITS[k as usize]),
            Map => key(KeyCode::KeyM),
            Skills => key(KeyCode::KeyP),
            Settings => key(KeyCode::F1),
        }
    }
}

/// What's pressed and held for walking, see Bindings::walk()
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Walk {
    /// Ahead less back, and right less left, each 0 or 1
    pub forward: f64,
    pub right: f64,
    pub jump: bool,
    pub run: bool,
}

/// Every action's keys
#[derive(Clone, PartialEq, Debug)]
pub struct Bindings {
    keys: [[Option<Input>; 2]; ALL.len()],
}

impl Default for Bindings {
    fn default() -> Self {
        Bindings { keys: ALL.map(Action::defaults) }
    }
}

impl Bindings {
    pub fn get(&self, action: Action) -> [Option<Input>; 2] {
        self.keys[action.index()]
    }

    pub fn set(&mut self, action: Action, keys: [Option<Input>; 2]) {
        self.keys[action.index()] = keys;
    }

    /// Whether one of `action`'s keys went down this frame
    pub fn pressed(&self, input: &impl Buttons, action: Action) -> bool {
        self.get(action).into_iter().flatten().any(|i| match i {
            Key(k) => input.key_pressed(k),
            Mouse(b) => input.mouse_pressed(b),
        })
    }

    /// Whether one of `action`'s keys is held down
    pub fn held(&self, input: &impl Buttons, action: Action) -> bool {
        self.get(action).into_iter().flatten().any(|i| match i {
            Key(k) => input.key_held(k),
            Mouse(b) => input.mouse_held(b),
        })
    }

    /// Which way the player's walking, and whether they jump and run
    pub fn walk(&self, input: &impl Buttons) -> Walk {
        let on = |a| self.held(input, a) as u8 as f64;
        Walk { forward: on(Forward) - on(Back), right: on(Right) - on(Left), jump: self.held(input, Jump), run: self.held(input, Run) }
    }

    /// Which slot of the quick bar's key went down this frame, if any
    pub fn hotbar_pressed(&self, input: &impl Buttons) -> Option<usize> {
        (0..HOTBAR_KEYS).find(|&k| self.pressed(input, Hotbar(k as u8)))
    }

    /// Which piece's key went down this frame, while building
    pub fn piece_pressed(&self, input: &impl Buttons) -> Option<usize> {
        (0..PIECE_KEYS).find(|&k| self.pressed(input, Piece(k as u8)))
    }

    /// The name of `action`'s first key, as the prompts on screen say it, or "unbound"
    pub fn short(&self, action: Action) -> String {
        self.get(action).into_iter().flatten().next().map_or_else(|| "unbound".to_owned(), Input::short)
    }

    /// The others that can be used at the same time as `action` and are on one of its keys
    pub fn clashes_with(&self, action: Action) -> Vec<Action> {
        let mine: Vec<Input> = self.get(action).into_iter().flatten().collect();
        ALL.iter()
            .copied()
            .filter(|&b| b != action && b.when().meets(action.when()) && self.get(b).into_iter().flatten().any(|i| mine.contains(&i)))
            .collect()
    }

    /// Every pair of actions on the same key that could be wanted at once
    pub fn clashes(&self) -> Vec<(Action, Action)> {
        ALL.iter().enumerate().flat_map(|(i, &a)| self.clashes_with(a).into_iter().filter(move |b| b.index() > i).map(move |b| (a, b))).collect()
    }

    /// Puts `action`'s key `slot` (0 or 1) on `to`. An action that can be used at the same time
    /// and was on it takes the key `action` had there instead, so they swap; those are given back,
    /// each with the key it's on now, if any.
    pub fn rebind(&mut self, action: Action, slot: usize, to: Input) -> Vec<(Action, Option<Input>)> {
        let slot = slot.min(1);
        let mine = self.get(action);
        let old = mine[slot];
        if old == Some(to) {
            return Vec::new();
        }
        let mut moved = Vec::new();
        // Its own other key: the two change places
        if mine[1 - slot] == Some(to) {
            self.keys[action.index()][1 - slot] = old;
        }
        for &b in ALL.iter().filter(|&&b| b != action && b.when().meets(action.when())) {
            let keys = &mut self.keys[b.index()];
            for s in 0..2 {
                if keys[s] == Some(to) {
                    keys[s] = old;
                    moved.push((b, old));
                }
            }
            // Never one key twice, nor a second with no first
            if keys[1].is_some() && keys[0] == keys[1] {
                keys[1] = None;
            }
            if keys[0].is_none() {
                keys.swap(0, 1);
            }
        }
        self.keys[action.index()][slot] = Some(to);
        moved
    }

    /// Takes `action`'s key `slot` off it
    pub fn clear(&mut self, action: Action, slot: usize) {
        let keys = &mut self.keys[action.index()];
        keys[slot.min(1)] = None;
        if keys[0].is_none() {
            keys.swap(0, 1);
        }
    }

    /// As it's kept in the file: a line an action, its keys split by commas, a heading for each
    /// group
    pub fn to_text(&self) -> String {
        let mut text = String::from(
            "# Innert's keys and mouse buttons, changed in the game's Controls window (Esc, then Controls).\n\
             # A line an action: its name, =, then up to two keys split by commas, nothing after the = for none.\n\
             # Take out a line, or this whole file, for the keys it had to begin with.\n",
        );
        let mut group = None;
        for a in ALL {
            if group != Some(a.group()) {
                group = Some(a.group());
                text += &format!("\n# {}\n", a.group().title());
            }
            let keys: Vec<String> = self.get(a).into_iter().flatten().map(Input::name).collect();
            text += &format!("{} = {}\n", a.id(), keys.join(", "));
        }
        text
    }

    /// Read back from the file's words: a line that isn't an action with keys that can be read
    /// leaves that action as it was to begin with
    pub fn from_text(text: &str) -> Bindings {
        let mut bindings = Bindings::default();
        for line in text.lines().map(str::trim).filter(|l| !l.starts_with('#')) {
            let Some((id, keys)) = line.split_once('=') else {
                continue;
            };
            let Some(action) = Action::by_id(id) else {
                continue;
            };
            let read: Option<Vec<Input>> = keys.split(',').map(str::trim).filter(|k| !k.is_empty()).map(Input::parse).collect();
            match read {
                Some(read) if read.len() <= 2 => bindings.set(action, [read.first().copied(), read.get(1).copied()]),
                _ => {}
            }
        }
        bindings
    }

    /// From `file`, or as they were to begin with if there's none or it can't be read
    pub fn load(file: &Path) -> Bindings {
        std::fs::read_to_string(file).map_or_else(|_| Bindings::default(), |text| Bindings::from_text(&text))
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, self.to_text())
    }
}

/// Where they're kept: %LOCALAPPDATA%\Innert\keys.txt
pub fn kept_file() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("Innert").join("keys.txt")
}

thread_local! {
    /// The keys in use, for the prompts on screen to name, see key()
    static CURRENT: RefCell<Bindings> = RefCell::new(Bindings::default());
}

/// Makes `bindings` the ones the prompts on screen name
pub fn publish(bindings: &Bindings) {
    CURRENT.with(|c| c.borrow_mut().clone_from(bindings));
}

/// The name of `action`'s key as the prompts on screen say it, "E" or "Right click"
pub fn key(action: Action) -> String {
    CURRENT.with(|c| c.borrow().short(action))
}

/// The same, said in the middle of a sentence: "click" and "right click" for the mouse's buttons,
/// a key's name as it is
pub fn words(action: Action) -> String {
    let said = key(action);
    if said.ends_with("click") || said == "Click" { said.to_lowercase() } else { said }
}

/// The keys of `actions` said together, as the prompts do: "WASD" when each is a letter, "Up Left
/// Down Right" when not
pub fn keys_together(actions: &[Action]) -> String {
    let names: Vec<String> = actions.iter().map(|&a| key(a)).collect();
    names.join(if names.iter().all(|n| n.chars().count() == 1) { "" } else { " " })
}

/// The keys of the quick bar's slots as egui knows them, for putting what's under the pointer in
/// the inventory onto one
pub fn hotbar_egui_keys() -> [Option<egui::Key>; HOTBAR_KEYS] {
    CURRENT.with(|c| {
        let c = c.borrow();
        std::array::from_fn(|k| c.get(Hotbar(k as u8))[0].and_then(Input::egui_key))
    })
}

/// The little name on the tab of the quick bar's slot `k`
pub fn hotbar_tab(k: usize) -> String {
    CURRENT.with(|c| c.borrow().get(Hotbar(k as u8))[0].map_or_else(String::new, Input::tiny))
}

/// A line of the Controls window
enum Row {
    /// An action, with its two keys
    One(Action),
    /// Actions like the quick bar's slots, a key each, along one line
    Many(&'static str, &'static str, &'static [Action]),
    /// Done with another's key, its name in plain words, what it does, and which
    Same(&'static str, &'static str, Action),
    /// On a key that can't be changed
    Kept(&'static str, &'static str, &'static str),
    /// Something to know
    Note(&'static str),
}

const HOTBAR_ROW: [Action; HOTBAR_KEYS] = [Hotbar(0), Hotbar(1), Hotbar(2), Hotbar(3), Hotbar(4), Hotbar(5), Hotbar(6), Hotbar(7), Hotbar(8)];
const PIECE_ROW: [Action; PIECE_KEYS] = [Piece(0), Piece(1), Piece(2), Piece(3), Piece(4), Piece(5), Piece(6), Piece(7), Piece(8), Piece(9), Piece(10)];

/// What the Controls window lists under `group`
fn rows(group: Group) -> Vec<Row> {
    use Row::*;
    match group {
        Group::Moving => vec![
            One(Forward), One(Back), One(Left), One(Right),
            Kept("Look about", "", "Mouse"),
            One(Jump), One(Run), One(Fly), One(FlyUp), One(FlyDown),
        ],
        Group::Fighting => vec![One(Attack), One(Heavy), One(Reload), One(Ammo)],
        Group::Magic => vec![
            One(Special),
            Same("Cast", "Staff, sceptre or wand: click to cast. Orb: hold for a beam. Tome: conjure. Totem: hex", Attack),
            Same("A weapon's power", "Hold to aim, let go to set it off; its bar under the weapon's name fills as it comes round again", Heavy),
            Same("Send the raised dead", "The Necromancer's summonings: while the power comes round again, this sends all you've raised at what's aimed at", Heavy),
        ],
        Group::Items => vec![
            One(Interact), One(Inventory),
            Many("Quick bar", "Picks what's in hand. Over a slot in the inventory, puts it on the quick bar. The mouse wheel goes through them too", &HOTBAR_ROW),
            Same("Eat", "With food in hand", Heavy),
        ],
        Group::Wings => vec![
            Same("Glide", "Jump, then hold in the air to glide", Jump),
            Same("Flap", "Wings that flap: press in the air", Jump),
            Same("Dash", "Wings that dash: press while gliding", Run),
            Note("Merchants sell wings; bosses and chests leave them. Wear them from the inventory."),
        ],
        Group::Building => vec![
            One(Gather), One(Build), One(Place), One(TakeDown), One(Turn), One(Mend), One(NextTab),
            Many("Pick a piece", "The pieces of the tab open, its cost beside each; the mouse wheel goes through them too", &PIECE_ROW),
        ],
        Group::Menus => vec![
            One(Map), One(Skills), One(Settings),
            Kept("Menu", "Closes a window first, if one's open", "Esc"),
            Kept("Screenshot", "Saved beside the game", "F12"),
        ],
    }
}

/// The Controls window and the keys: what's bound, which key's being changed, what was last said
/// of it, and the file they're kept in, if any
pub struct Keys {
    pub map: Bindings,
    /// The window's open
    pub open: bool,
    /// The action whose key `slot` takes the next key pressed, and whether that's from the frame
    /// it was clicked, so the click's own button isn't taken for it
    changing: Option<(Action, usize, bool)>,
    /// What became of the last change, and whether it's a warning
    notice: Option<(String, bool)>,
    /// A mouse button just bound, so its click let go over the key it's bound to doesn't start
    /// changing it again
    let_go: bool,
    file: Option<PathBuf>,
    /// How far down the list's held, for screenshots
    pub scroll_to: Option<f32>,
}

impl Keys {
    /// The keys kept in `file`, saved back there as they're changed; with no file, as they were to
    /// begin with, and not kept
    pub fn new(file: Option<PathBuf>) -> Keys {
        let map = file.as_deref().map_or_else(Bindings::default, Bindings::load);
        publish(&map);
        Keys { map, open: false, changing: None, notice: None, let_go: false, file, scroll_to: None }
    }

    /// Takes the next key pressed for `action`'s key `slot`
    pub fn change(&mut self, action: Action, slot: usize) {
        self.changing = Some((action, slot.min(1), true));
        self.notice = None;
    }

    /// Called each frame before anything else reads the keys: while one's being changed, takes the
    /// key or button pressed for it, Esc leaving it as it was. True while one's being changed, or
    /// was this frame, when the game reads no keys.
    pub fn take(&mut self, input: &WinitInputHelper) -> bool {
        let Some((action, slot, fresh)) = self.changing else {
            return false;
        };
        if fresh {
            self.changing = Some((action, slot, false));
            return true;
        }
        if input.key_pressed(KeyCode::Escape) {
            self.changing = None;
            self.notice = Some((format!("{} left as it was", action.name()), false));
            return true;
        }
        if input.key_pressed(KeyCode::F12) {
            self.notice = Some(("F12 always takes a screenshot: press another key".into(), true));
            return true;
        }
        let key = KEYS.iter().find(|(k, _)| input.key_pressed(*k)).map(|(k, _)| Key(*k));
        let button = BUTTONS.iter().find(|(b, _)| input.mouse_pressed(*b)).map(|(b, _)| Mouse(*b));
        if let Some(to) = key.or(button) {
            self.let_go = button.is_some();
            self.bind(action, slot, to);
        }
        true
    }

    /// Puts `action`'s key `slot` on `to`, saying what became of whatever was on it, and keeps it
    pub fn bind(&mut self, action: Action, slot: usize, to: Input) {
        self.changing = None;
        let moved = self.map.rebind(action, slot, to);
        let mut said = format!("{} is on {}", action.name(), to.name());
        for (b, now) in &moved {
            said += &match now {
                Some(k) => format!(". {} was, and is on {} now", b.name(), k.name()),
                None => format!(". {} was, and has no key now", b.name()),
            };
        }
        let clash = self.map.clashes_with(action).into_iter().chain(moved.iter().flat_map(|(b, _)| self.map.clashes_with(*b))).next();
        let warn = moved.iter().any(|(_, now)| now.is_none()) || clash.is_some();
        if let Some(c) = clash {
            said += &format!(". {} shares a key with something it can be used with", c.name());
        }
        self.notice = Some((said, warn));
        self.changed();
    }

    /// Every key as it was to begin with
    pub fn reset(&mut self) {
        self.changing = None;
        self.map = Bindings::default();
        self.notice = Some(("Every key's as it was to begin with".into(), false));
        self.changed();
    }

    fn changed(&mut self) {
        publish(&self.map);
        if let Some(file) = &self.file
            && let Err(e) = self.map.save(file)
        {
            self.notice = Some((format!("The keys couldn't be kept in {}: {e}", file.display()), true));
        }
    }

    /// The Controls window, while it's open: every action grouped, its keys clicked to change
    /// them, right clicked to take one off. Shut, its own way or with Esc, it's said by `open`.
    pub fn window(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let screen = ctx.screen_rect();
        // Room for the list between the window's head and foot, scrolling through the rest
        let room = (screen.height() - 214.0).max(160.0);
        let mut open = true;
        let mut clicked = None;
        let mut cleared = None;
        let mut reset = false;
        let changing = self.changing.map(|(a, s, _)| (a, s));
        let scroll_to = self.scroll_to;
        let clashing: Vec<Action> = self.map.clashes().into_iter().flat_map(|(a, b)| [a, b]).collect();
        egui::Window::new("Controls")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(WINDOW_WIDTH)
            .max_width(WINDOW_WIDTH)
            .order(egui::Order::Foreground)
            // Solid, so the menu under it doesn't show through
            .frame(egui::Frame::window(&ctx.style()).fill(theme::panel_fill().to_opaque()))
            .show(ctx, |ui| {
                let says = if changing.is_some() {
                    "Press the key or mouse button to put it on. Esc leaves it as it was."
                } else {
                    "Click a key to change it, then press the new one. Right click a key to take it off."
                };
                ui.label(RichText::new(says).size(13.5).color(if changing.is_some() { theme::HEADING } else { theme::WEAK }));
                match &self.notice {
                    Some((said, warn)) => ui.label(RichText::new(said).size(13.0).color(if *warn { theme::BAD } else { theme::INFO })),
                    None => ui.label(RichText::new(" ").size(13.0)),
                };
                ui.add_space(4.0);
                ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                let mut list = egui::ScrollArea::vertical().max_height(room).min_scrolled_height(room).auto_shrink([false, true]);
                if let Some(down) = scroll_to {
                    list = list.vertical_scroll_offset(down);
                }
                list.show(ui, |ui| {
                    ui.set_width(ui.available_width() - 14.0);
                    for group in Group::ALL {
                        ui.add_space(8.0);
                        theme::heading(ui, group.title(), 17.0);
                        theme::rule(ui, theme::EDGE);
                        for row in rows(group) {
                            let shown = row_ui(ui, &self.map, &row, changing, &clashing);
                            if let Some(pick) = shown.clicked {
                                clicked = Some(pick);
                            }
                            if let Some(pick) = shown.cleared {
                                cleared = Some(pick);
                            }
                        }
                    }
                    ui.add_space(6.0);
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add_sized([170.0, 30.0], egui::Button::new(RichText::new("Reset to defaults").size(15.0))).clicked() {
                        reset = true;
                    }
                    // Where they're kept, said short, the whole of it under the pointer
                    let (kept, whole) = match &self.file {
                        Some(file) if *file == kept_file() => ("Kept in %LOCALAPPDATA%\\Innert\\keys.txt".to_owned(), file.display().to_string()),
                        Some(file) => (format!("Kept in {}", file.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned())), file.display().to_string()),
                        None => ("Not kept in this run".to_owned(), "A screenshot run keeps them only with --set keys=<file>".to_owned()),
                    };
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(kept).size(11.5).color(theme::WEAK)).on_hover_text(whole));
                });
            });
        // A mouse button just bound: its click let go isn't another
        let pointer_down = ctx.input(|i| i.pointer.any_down());
        if std::mem::take(&mut self.let_go) && (pointer_down || clicked.is_some()) {
            self.let_go = pointer_down;
            clicked = None;
        }
        if reset {
            self.reset();
        } else if let Some((action, slot)) = clicked {
            if changing == Some((action, slot)) {
                // Clicked again, a click's what goes on it
                self.bind(action, slot, Mouse(MouseButton::Left));
            } else {
                self.change(action, slot);
            }
            // Nothing in the window keeps the keys for itself, Space or Enter clicking it again
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        } else if let Some((action, slot)) = cleared.filter(|_| changing.is_none()) {
            self.map.clear(action, slot);
            self.notice = Some((format!("{} has {} now", action.name(), self.map.get(action)[0].map_or("no key".to_owned(), |k| format!("only {}", k.name()))), self.map.get(action)[0].is_none()));
            self.changed();
        }
        if !open {
            self.open = false;
            self.changing = None;
        }
    }

    /// Closes the window, or stops changing a key if one's being changed
    pub fn close(&mut self) {
        if self.changing.take().is_none() {
            self.open = false;
        }
    }
}

/// What was clicked on a row: a key to change, or one to take off
#[derive(Default)]
struct RowClicks {
    clicked: Option<(Action, usize)>,
    cleared: Option<(Action, usize)>,
}

/// How wide the window is, the left of a row, its name and what it does, and a key's button
const WINDOW_WIDTH: f32 = 620.0;
const NAME_WIDTH: f32 = 330.0;
const KEY_WIDTH: f32 = 112.0;

/// A button showing a key: lit white while it's the one being changed, edged in red while it
/// clashes, faint when it's another's, or can't be changed
fn key_button(ui: &mut egui::Ui, text: &str, size: [f32; 2], lit: bool, clash: bool, faint: bool) -> egui::Response {
    let button = if lit {
        egui::Button::new(RichText::new(text).size(13.5).color(theme::PRIMARY_INK)).fill(theme::PRIMARY)
    } else if faint {
        egui::Button::new(RichText::new(text).size(13.0).color(theme::WEAK)).fill(Color32::TRANSPARENT).stroke(Stroke::new(1.0f32, theme::EDGE.gamma_multiply(0.6))).sense(egui::Sense::hover())
    } else {
        let b = egui::Button::new(RichText::new(text).size(14.0).color(theme::TEXT));
        if clash { b.stroke(Stroke::new(1.5f32, theme::BAD)) } else { b }
    };
    ui.add_sized(size, button)
}

/// A line of the Controls window: its name and what it does on the left, its keys on the right
fn row_ui(ui: &mut egui::Ui, map: &Bindings, row: &Row, changing: Option<(Action, usize)>, clashing: &[Action]) -> RowClicks {
    let mut clicks = RowClicks::default();
    let shown = |a: Action, s: usize| -> (String, bool) {
        if changing == Some((a, s)) {
            ("press a key…".into(), true)
        } else {
            (map.get(a)[s].map_or("—".to_owned(), Input::name), false)
        }
    };
    let label = |ui: &mut egui::Ui, name: &str, hint: &str, clash: Option<String>| {
        ui.vertical(|ui| {
            ui.set_width(NAME_WIDTH);
            ui.label(RichText::new(name).size(14.5).color(theme::TEXT));
            if !hint.is_empty() {
                ui.add(egui::Label::new(RichText::new(hint).size(12.0).color(theme::WEAK)).wrap());
            }
            if let Some(clash) = clash {
                ui.label(RichText::new(clash).size(12.0).color(theme::BAD));
            }
        });
    };
    ui.add_space(3.0);
    match *row {
        Row::One(action) => {
            ui.horizontal(|ui| {
                let clash = clashing.contains(&action).then(|| {
                    let with: Vec<String> = map.clashes_with(action).into_iter().map(Action::name).collect();
                    format!("Shares a key with {}", with.join(", "))
                });
                label(ui, &action.name(), action.hint(), clash.clone());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for slot in [1, 0] {
                        let (text, lit) = shown(action, slot);
                        let response = key_button(ui, &text, [KEY_WIDTH, 26.0], lit, clash.is_some() && slot == 0, false);
                        if response.clicked() {
                            clicks.clicked = Some((action, slot));
                        }
                        if response.secondary_clicked() {
                            clicks.cleared = Some((action, slot));
                        }
                    }
                });
            });
        }
        Row::Many(name, hint, actions) => {
            let clash: Vec<String> = actions.iter().filter(|a| clashing.contains(*a)).map(|a| a.name()).collect();
            label(ui, name, hint, (!clash.is_empty()).then(|| format!("{} share keys with others", clash.join(", "))));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for &action in actions {
                    let (text, lit) = shown(action, 0);
                    let text = if lit { "…".to_owned() } else { text };
                    let response = key_button(ui, &text, [44.0, 26.0], lit, clashing.contains(&action), false).on_hover_text(action.name());
                    if response.clicked() {
                        clicks.clicked = Some((action, 0));
                    }
                    if response.secondary_clicked() {
                        clicks.cleared = Some((action, 0));
                    }
                }
            });
        }
        Row::Same(name, hint, of) => {
            ui.horizontal(|ui| {
                label(ui, name, hint, None);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("as {}", of.name().split(',').next().unwrap_or(""))).size(11.5).color(theme::WEAK));
                    key_button(ui, &map.get(of)[0].map_or("—".to_owned(), Input::name), [KEY_WIDTH, 26.0], false, false, true);
                });
            });
        }
        Row::Kept(name, hint, key) => {
            ui.horizontal(|ui| {
                label(ui, name, hint, None);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(KEY_WIDTH + ui.spacing().item_spacing.x);
                    key_button(ui, key, [KEY_WIDTH, 26.0], false, false, true);
                });
            });
        }
        Row::Note(text) => {
            ui.add(egui::Label::new(RichText::new(text).size(12.5).color(theme::WEAK).italics()).wrap());
        }
    }
    clicks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What's pressed and held, made up
    #[derive(Default)]
    struct Fake {
        pressed: Vec<Input>,
        held: Vec<Input>,
    }

    impl Buttons for Fake {
        fn key_pressed(&self, key: KeyCode) -> bool {
            self.pressed.contains(&Key(key))
        }
        fn key_held(&self, key: KeyCode) -> bool {
            self.held.contains(&Key(key))
        }
        fn mouse_pressed(&self, button: MouseButton) -> bool {
            self.pressed.contains(&Mouse(button))
        }
        fn mouse_held(&self, button: MouseButton) -> bool {
            self.held.contains(&Mouse(button))
        }
    }

    fn pressing(i: Input) -> Fake {
        Fake { pressed: vec![i], held: vec![i] }
    }

    /// To begin with, every action's on the key it always was, read as the game read it
    #[test]
    fn the_defaults_are_the_old_keys() {
        let b = Bindings::default();
        let old: [(KeyCode, &[Action]); 22] = [
            (KeyCode::KeyW, &[Forward]),
            (KeyCode::KeyS, &[Back]),
            (KeyCode::KeyA, &[Left]),
            (KeyCode::KeyD, &[Right]),
            (KeyCode::Space, &[Jump]),
            (KeyCode::ShiftLeft, &[Run]),
            (KeyCode::ShiftRight, &[Run]),
            (KeyCode::KeyF, &[Fly]),
            (KeyCode::KeyQ, &[FlyUp, Reload]),
            (KeyCode::KeyE, &[FlyDown, Interact, Mend]),
            (KeyCode::KeyT, &[Ammo]),
            (KeyCode::KeyR, &[Special, Turn]),
            (KeyCode::KeyI, &[Inventory]),
            (KeyCode::Tab, &[Inventory]),
            (KeyCode::KeyG, &[Gather]),
            (KeyCode::KeyB, &[Build]),
            (KeyCode::KeyC, &[NextTab]),
            (KeyCode::KeyM, &[Map]),
            (KeyCode::KeyP, &[Skills]),
            (KeyCode::F1, &[Settings]),
            (KeyCode::Digit0, &[Piece(9)]),
            (KeyCode::Minus, &[Piece(10)]),
        ];
        for (key, actions) in old {
            let on: Vec<Action> = ALL.iter().copied().filter(|&a| b.pressed(&pressing(Key(key)), a)).collect();
            assert_eq!(on, actions.to_vec(), "{key:?}");
        }
        for k in 0..9 {
            let on: Vec<Action> = ALL.iter().copied().filter(|&a| b.pressed(&pressing(Key(DIGITS[k])), a)).collect();
            assert_eq!(on, vec![Hotbar(k as u8), Piece(k as u8)]);
        }
        for (button, actions) in [(MouseButton::Left, vec![Attack, Place]), (MouseButton::Right, vec![Heavy, TakeDown]), (MouseButton::Middle, vec![Special])] {
            let on: Vec<Action> = ALL.iter().copied().filter(|&a| b.pressed(&pressing(Mouse(button)), a)).collect();
            assert_eq!(on, actions, "{button:?}");
        }
        // Not one of them clashing: those on the same key are never wanted at once
        assert_eq!(b.clashes(), Vec::new());
        // And every action's listed in the window once, under its own group
        let lists = |g: Group, a: Action| rows(g).iter().any(|r| matches!(r, Row::One(x) if *x == a) || matches!(r, Row::Many(_, _, xs) if xs.contains(&a)));
        for a in ALL {
            assert_eq!(Group::ALL.iter().filter(|&&g| lists(g, a)).count(), 1, "{a:?}");
            assert!(lists(a.group(), a), "{a:?} in its group");
        }
    }

    /// Written to the file and read back, they're the same, changed or not, and every key and
    /// button's name reads back as itself
    #[test]
    fn the_file_reads_back() {
        let dir = std::env::temp_dir().join(format!("innert-keys-{}", std::process::id()));
        let file = dir.join("keys.txt");
        let mut b = Bindings::default();
        b.rebind(Jump, 0, Key(KeyCode::KeyJ));
        b.rebind(Special, 1, Mouse(MouseButton::Back));
        b.rebind(Map, 1, Key(KeyCode::Comma));
        b.clear(Ammo, 0);
        b.save(&file).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("jump = J\n"), "{text}");
        assert!(text.contains("run = Left Shift, Right Shift\n"), "{text}");
        assert!(text.contains("ammo = \n"), "{text}");
        assert_eq!(Bindings::load(&file), b);
        std::fs::remove_dir_all(&dir).unwrap();
        for &(k, _) in KEYS {
            assert_eq!(Input::parse(&Key(k).name()), Some(Key(k)), "{k:?}");
        }
        for &(m, _) in BUTTONS {
            assert_eq!(Input::parse(&Mouse(m).name()), Some(Mouse(m)), "{m:?}");
        }
    }

    /// No file, or one that's nonsense, is every key as it was to begin with; a line that can't be
    /// read leaves only its own action so
    #[test]
    fn a_broken_file_is_the_defaults() {
        assert_eq!(Bindings::load(Path::new("Z:\\nowhere\\keys.txt")), Bindings::default());
        assert_eq!(Bindings::from_text("\u{0}\u{1}garbage\n= = =\njump\nfly = \u{7f}"), Bindings::default());
        let b = Bindings::from_text("jump = Nonsense\nrun = Q, W, E\nmap = N\nwhatever = K");
        assert_eq!(b.get(Jump), Jump.defaults());
        assert_eq!(b.get(Run), Run.defaults());
        assert_eq!(b.get(Map), [Some(Key(KeyCode::KeyN)), None]);
    }

    /// Changing a key moves the action to it, and off what it was on
    #[test]
    fn rebinding_moves_an_action() {
        let mut b = Bindings::default();
        let moved = b.rebind(Jump, 0, Key(KeyCode::KeyJ));
        assert!(moved.is_empty());
        assert!(b.pressed(&pressing(Key(KeyCode::KeyJ)), Jump));
        assert!(!b.pressed(&pressing(Key(KeyCode::Space)), Jump));
        // A mouse button too, as the second key
        b.rebind(Reload, 1, Mouse(MouseButton::Forward));
        assert!(b.pressed(&pressing(Mouse(MouseButton::Forward)), Reload) && b.pressed(&pressing(Key(KeyCode::KeyQ)), Reload));
        // Its own other key: they change places
        b.rebind(Run, 0, Key(KeyCode::ShiftRight));
        assert_eq!(b.get(Run), [Some(Key(KeyCode::ShiftRight)), Some(Key(KeyCode::ShiftLeft))]);
        // Taken off, the second becomes the first
        b.clear(Inventory, 0);
        assert_eq!(b.get(Inventory), [Some(Key(KeyCode::Tab)), None]);
        assert_eq!(b.clashes(), Vec::new());
    }

    /// Put on a key another action that can be used at the same time is on, the two swap; one
    /// that's never wanted at once keeps it
    #[test]
    fn a_clash_is_caught() {
        let mut b = Bindings::default();
        let moved = b.rebind(Jump, 0, Key(KeyCode::KeyG));
        assert_eq!(moved, vec![(Gather, Some(Key(KeyCode::Space)))]);
        assert_eq!(b.get(Gather)[0], Some(Key(KeyCode::Space)));
        assert_eq!(b.clashes(), Vec::new());
        // A weapon's special and turning a piece share R, never being wanted at once: put the
        // reload there too and only the special moves off, onto Q
        let moved = b.rebind(Reload, 0, Key(KeyCode::KeyR));
        assert_eq!(moved, vec![(Special, Some(Key(KeyCode::KeyQ)))]);
        assert_eq!(b.get(Turn)[0], Some(Key(KeyCode::KeyR)));
        b.rebind(Special, 0, Key(KeyCode::KeyR));
        assert_eq!(b.get(Reload)[0], Some(Key(KeyCode::KeyQ)));
        // Changing ammo's wanted all the time, so put the special on its key and it moves off
        let moved = b.rebind(Special, 0, Key(KeyCode::KeyT));
        assert_eq!(moved, vec![(Ammo, Some(Key(KeyCode::KeyR)))]);
        // Ammo's now on R, which turning a piece is on too, both wanted while building
        assert_eq!(b.clashes(), vec![(Ammo, Turn)]);
        assert_eq!(b.clashes_with(Turn), vec![Ammo]);
        // The window says so
        let mut keys = Keys::new(None);
        keys.bind(Jump, 0, Key(KeyCode::KeyG));
        let (said, warn) = keys.notice.clone().unwrap();
        assert!(said.contains("Gather was, and is on Space now") && !warn, "{said}");
        keys.bind(Special, 0, Key(KeyCode::KeyT));
        let (said, warn) = keys.notice.clone().unwrap();
        assert!(warn && said.contains("shares a key"), "{said}");
        // And the prompts on screen name the keys as they are now
        assert_eq!(key(Gather), "Space");
        keys.reset();
        assert_eq!(key(Gather), "G");
        assert_eq!(key(Interact), "E");
        assert_eq!(key(Heavy), "Right click");
        assert_eq!(key(Run), "Shift");
    }

    /// A flat world at 16 voxels a meter, the ground 4 m deep
    fn flat() -> crate::worldgen::World {
        let size = [256u32, 256, 128];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 8].fill(u128::MAX);
        let mut world = crate::worldgen::World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 16;
        world
    }

    /// The player walks and jumps as the keys say, read as the game reads them: with jump put on
    /// J, J jumps and Space doesn't, and W still walks
    #[test]
    fn a_rebound_jump_key_jumps() {
        let world = flat();
        let mut b = Bindings::default();
        b.rebind(Jump, 0, Key(KeyCode::KeyJ));
        let highest = |held: Vec<Input>| {
            let mut player = crate::player::Player::new(nalgebra::Vector3::new(128.0, 128.0, 100.0), &world);
            let input = Fake { pressed: Vec::new(), held };
            let (mut top, mut start, mut walked) = (0.0f64, None, 0.0);
            for frame in 0..240 {
                // Settled on the ground first
                let walk = if frame < 60 { Walk::default() } else { b.walk(&input) };
                let before = player.position;
                player.update(&world, &|_, _| f64::NEG_INFINITY, nalgebra::Vector2::new(walk.right, walk.forward), walk.jump, walk.run, 1.0 / 60.0);
                if frame == 60 {
                    start = Some(player.position.z);
                }
                if let Some(start) = start {
                    top = top.max(player.position.z - start);
                    walked += (player.position - before).xy().norm();
                }
            }
            (top / 16.0, walked / 16.0)
        };
        let (jumped, _) = highest(vec![Key(KeyCode::KeyJ)]);
        assert!(jumped > 0.4, "J jumped {jumped:.2} m");
        let (jumped, _) = highest(vec![Key(KeyCode::Space)]);
        assert!(jumped < 0.05, "Space jumped {jumped:.2} m");
        let (_, walked) = highest(vec![Key(KeyCode::KeyW)]);
        assert!(walked > 1.0, "W walked {walked:.2} m");
    }

    /// The Controls window fits the smallest window played in, Reset at its foot, the list
    /// scrolling, and the key being changed says so
    #[test]
    fn the_window_fits() {
        for [w, h] in [[1548.0, 872.0], [1280.0, 600.0]] {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let mut keys = Keys::new(None);
            keys.open = true;
            keys.change(Jump, 0);
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h))), ..Default::default() };
            let mut shown = egui::Rect::NOTHING;
            for _ in 0..4 {
                let out = ctx.run(input.clone(), |ctx| keys.window(ctx));
                let _ = out;
                shown = ctx.memory(|m| m.area_rect(egui::Id::new("Controls"))).expect("the window's drawn");
            }
            assert!(shown.min.y >= 0.0 && shown.max.y <= h && shown.min.x >= 0.0 && shown.max.x <= w, "at {w}×{h} it's at {shown:?}");
            // And it's most of the window's height, the list of keys not squeezed to nothing
            assert!(shown.height() > 0.75 * h, "at {w}×{h} it's only {:.0} high", shown.height());
            assert!(keys.changing.is_some() && keys.open);
        }
    }
}
