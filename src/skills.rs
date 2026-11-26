//! The passive skill web: a point to spend for every level gone up past the first, and a few more
//! for a realm's raid bosses all beaten and for its finale, see Points. Spent on the nodes of a web
//! that branches out from one start in the middle, each node next to one already taken: small ones
//! a little, notables more, and keystones at the ends of the branches a lot, at a price. What the
//! web is, its nodes, where they're drawn and what each gives, is data, in assets\skills.json, see
//! Web. What all those taken add up to is Totals, which the game's stats read, see Stat.
//!
//! Its window, opened with P, is drawn by window() only while it's open.

use std::collections::VecDeque;
use std::path::PathBuf;

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use serde::Deserialize;

/// What a node can give. Most are percent: of damage, speed and so on; see describe() for each.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    /// More damage with weapons of each kind, and with all of them, percent
    MeleeDamage,
    RangedDamage,
    MagicDamage,
    Damage,
    /// Quicker swings, draws and reloads, and casts, percent
    MeleeSpeed,
    RangedSpeed,
    MagicSpeed,
    /// Critical chance, percentage points, and critical damage, times more
    CritChance,
    CritDamage,
    /// More of a weapon's elemental damage, percent
    ElementalDamage,
    /// A melee weapon's reach, percent
    MeleeReach,
    /// How fast shots and spells fly, percent
    ProjectileSpeed,
    /// Mana a cast costs, percent (less is better)
    ManaCost,
    /// How wide blows, blasts and spells reach, percent
    Area,
    /// How strong a weapon's effects are, percent
    StatusPotency,
    /// How hard blows throw back what they hit, percent
    Knockback,
    /// How quickly wood, stone and clay are gathered, percent
    GatherSpeed,
    /// Health at full: so much more, and percent more
    MaxHealth,
    MaxHealthPct,
    /// Less damage taken, percent
    Armour,
    /// Health coming back quicker, percent
    Regen,
    /// Burning, poison and frost lasting less, percent
    ResistFire,
    ResistPoison,
    ResistFrost,
    /// More health from a potion, percent
    Potion,
    /// Moving quicker, percent
    MoveSpeed,
    /// Gliding further and quicker on wings, percent
    Glide,
    /// The grappling hook thrown further, percent
    HookReach,
    /// The special attack charging quicker, percent
    SpecialRecharge,
    /// Of every blow taken, this percent pricked back at what landed it, as thorned wings do
    Thorns,
    /// Health back for every monster or boss slain
    Leech,
    /// More gold found, percent
    GoldFind,
    /// More experience, percent
    Experience,
    /// More damage struck from behind, or at what hasn't seen the player, percent
    Backstab,
}

impl Stat {
    pub const ALL: [Stat; 34] = [
        Stat::MeleeDamage,
        Stat::RangedDamage,
        Stat::MagicDamage,
        Stat::Damage,
        Stat::MeleeSpeed,
        Stat::RangedSpeed,
        Stat::MagicSpeed,
        Stat::CritChance,
        Stat::CritDamage,
        Stat::ElementalDamage,
        Stat::MeleeReach,
        Stat::ProjectileSpeed,
        Stat::ManaCost,
        Stat::Area,
        Stat::StatusPotency,
        Stat::Knockback,
        Stat::GatherSpeed,
        Stat::MaxHealth,
        Stat::MaxHealthPct,
        Stat::Armour,
        Stat::Regen,
        Stat::ResistFire,
        Stat::ResistPoison,
        Stat::ResistFrost,
        Stat::Potion,
        Stat::MoveSpeed,
        Stat::Glide,
        Stat::HookReach,
        Stat::SpecialRecharge,
        Stat::Thorns,
        Stat::Leech,
        Stat::GoldFind,
        Stat::Experience,
        Stat::Backstab,
    ];

    /// What `value` of it says, as on a node's card: "+4% melee damage"
    pub fn describe(self, value: f64) -> String {
        let pct = |what: &str| format!("{value:+.0}% {what}");
        match self {
            Stat::MeleeDamage => pct("melee damage"),
            Stat::RangedDamage => pct("ranged damage"),
            Stat::MagicDamage => pct("magic damage"),
            Stat::Damage => pct("damage with every weapon"),
            Stat::MeleeSpeed => pct("melee attack speed"),
            Stat::RangedSpeed => pct("draw and reload speed"),
            Stat::MagicSpeed => pct("cast speed"),
            Stat::CritChance => format!("{value:+.0}% critical chance"),
            Stat::CritDamage => format!("{value:+.2}× critical damage"),
            Stat::ElementalDamage => pct("elemental damage"),
            Stat::MeleeReach => pct("melee reach"),
            Stat::ProjectileSpeed => pct("projectile speed"),
            Stat::ManaCost => pct("mana cost"),
            Stat::Area => pct("area"),
            Stat::StatusPotency => pct("effect potency"),
            Stat::Knockback => pct("knockback"),
            Stat::GatherSpeed => pct("gathering speed"),
            Stat::MaxHealth => format!("{value:+.0} health at full"),
            Stat::MaxHealthPct => pct("health at full"),
            Stat::Armour => format!("{:.0}% less damage taken", value),
            Stat::Regen => pct("health regeneration"),
            Stat::ResistFire => format!("burning lasts {value:.0}% less"),
            Stat::ResistPoison => format!("poison lasts {value:.0}% less"),
            Stat::ResistFrost => format!("frost slows you {value:.0}% less long"),
            Stat::Potion => pct("health from potions"),
            Stat::MoveSpeed => pct("movement speed"),
            Stat::Glide => pct("glide speed and lift"),
            Stat::HookReach => pct("grappling hook reach"),
            Stat::SpecialRecharge => pct("special attack charge rate"),
            Stat::Thorns => format!("{value:.0}% of blows taken pricked back"),
            Stat::Leech => format!("{value:+.0} health for every kill"),
            Stat::GoldFind => pct("gold found"),
            Stat::Experience => pct("experience"),
            Stat::Backstab => pct("damage from behind or unseen"),
        }
    }
}

/// What kind of node: the start everything's reached from, a small one, a named notable, or a
/// keystone with a price to it
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Start,
    Small,
    Notable,
    Keystone,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Effect {
    pub stat: Stat,
    pub value: f64,
}

/// A node as assets\skills.json has it
#[derive(Deserialize, Clone, Debug)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub branch: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// Those it's linked to, by id: either end of a link may name it
    #[serde(default)]
    links: Vec<String>,
}

#[derive(Deserialize)]
struct File {
    nodes: Vec<Node>,
}

/// The whole web: its nodes, which each is linked to both ways, and which is the start
pub struct Web {
    pub nodes: Vec<Node>,
    pub links: Vec<Vec<usize>>,
    pub start: usize,
}

const BUILT_IN: &str = include_str!("../assets/skills.json");

impl Web {
    /// Reads assets\skills.json, so it can be changed without building again, or if that's
    /// missing or broken the copy built in
    pub fn load() -> Web {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets").join("skills.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|_| BUILT_IN.to_owned());
        Web::parse(&text).unwrap_or_else(|e| {
            eprintln!("{}: {e}. Using the skill web built in.", path.display());
            Web::parse(BUILT_IN).expect("the built-in skill web is broken")
        })
    }

    fn parse(text: &str) -> Result<Web, String> {
        let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let nodes = file.nodes;
        let index = |id: &str| nodes.iter().position(|n| n.id == id).ok_or_else(|| format!("no node {id}"));
        let mut links = vec![Vec::new(); nodes.len()];
        for (k, n) in nodes.iter().enumerate() {
            for other in &n.links {
                let j = index(other)?;
                if !links[k].contains(&j) {
                    links[k].push(j);
                    links[j].push(k);
                }
            }
        }
        let start = nodes.iter().position(|n| n.kind == Kind::Start).ok_or("no start")?;
        Ok(Web { nodes, links, start })
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Whether node `k` can be taken with `taken` taken: it's not yet, and it's next to one that is
    pub fn can_take(&self, taken: &[bool], k: usize) -> bool {
        !taken[k] && self.links[k].iter().any(|&j| taken[j])
    }

    /// Whether node `k` can be given back: it's taken, it's not the start, and every other node
    /// taken can still be reached from the start without it
    pub fn can_refund(&self, taken: &[bool], k: usize) -> bool {
        if !taken[k] || k == self.start {
            return false;
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut queue = VecDeque::from([self.start]);
        seen[self.start] = true;
        while let Some(i) = queue.pop_front() {
            for &j in &self.links[i] {
                if j != k && taken[j] && !seen[j] {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        (0..self.nodes.len()).all(|i| i == k || !taken[i] || seen[i])
    }

    /// What every node taken adds up to
    pub fn totals(&self, taken: &[bool]) -> Totals {
        let mut totals = Totals::default();
        for (n, _) in self.nodes.iter().zip(taken).filter(|(_, t)| **t) {
            for e in &n.effects {
                totals.0[e.stat as usize] += e.value;
            }
        }
        totals
    }
}

/// What the nodes taken add up to, for each Stat
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Totals(pub [f64; Stat::ALL.len()]);

impl Default for Totals {
    fn default() -> Self {
        Totals([0.0; Stat::ALL.len()])
    }
}

impl Totals {
    /// So much of `stat`, as it's given: percent for most
    pub fn get(&self, stat: Stat) -> f64 {
        self.0[stat as usize]
    }

    /// The same as a fraction, for a percent: 0.04 for 4%
    pub fn share(&self, stat: Stat) -> f64 {
        self.get(stat) / 100.0
    }

    /// What a weapon of `kind`, or a tool, is made: see weapons::Boost
    pub fn boost(&self, kind: crate::weapons::Kind, tool: bool) -> crate::weapons::Boost {
        use crate::weapons::Kind as K;
        let (damage, speed) = match kind {
            K::Melee => (Stat::MeleeDamage, Stat::MeleeSpeed),
            K::Ranged => (Stat::RangedDamage, Stat::RangedSpeed),
            K::Magic => (Stat::MagicDamage, Stat::MagicSpeed),
        };
        if tool {
            // A tool's what it gathers with, not what it fights with: how much quicker it gathers is
            // Game::gather_speed()'s
            return crate::weapons::Boost::default();
        }
        crate::weapons::Boost {
            damage: self.share(damage) + self.share(Stat::Damage),
            speed: self.share(speed),
            crit_chance: self.share(Stat::CritChance),
            crit_damage: self.get(Stat::CritDamage),
            elemental: self.share(Stat::ElementalDamage),
            reach: if kind == K::Melee { self.share(Stat::MeleeReach) } else { 0.0 },
            projectile_speed: self.share(Stat::ProjectileSpeed),
            mana_cost: self.share(Stat::ManaCost),
            area: self.share(Stat::Area),
            potency: self.share(Stat::StatusPotency),
            knockback: self.share(Stat::Knockback),
        }
    }
}

/// How many points there are: one a level past the first and the bonus ones, and how many of them
/// are spent
pub fn points_free(level: u32, bonus: u32, taken: &[bool], start: usize) -> i64 {
    let spent = taken.iter().enumerate().filter(|(k, t)| **t && *k != start).count() as i64;
    crate::levels::points_for(level) as i64 + bonus as i64 - spent
}

/// The colours of the branches, as the web draws them
pub fn branch_color(branch: &str) -> Color32 {
    match branch {
        "melee" => Color32::from_rgb(232, 92, 74),
        "ranged" => Color32::from_rgb(110, 210, 110),
        "magic" => Color32::from_rgb(100, 150, 255),
        "defence" => Color32::from_rgb(236, 196, 92),
        "mobility" => Color32::from_rgb(90, 214, 214),
        _ => Color32::from_rgb(210, 210, 220),
    }
}

/// Where the window's looking and how close, kept while it's shut, and the node last clicked
#[derive(Default)]
pub struct View {
    pub offset: egui::Vec2,
    pub zoom: f32,
    /// For screenshots: the node whose card shows as if the pointer were over it
    pub hover: Option<String>,
}

impl View {
    pub fn new() -> Self {
        View { offset: egui::Vec2::ZERO, zoom: 0.4, hover: None }
    }
}

/// What was done in the window
pub enum Action {
    Take(usize),
    Refund(usize),
    /// All of them given back, for gold, see refund_price()
    Forget,
    Close,
}

/// What forgetting every node taken costs, in gold: so much a point spent
pub fn refund_price(spent: usize) -> u32 {
    15 * spent as u32
}

/// The web drawn over the whole view, dimming the game behind it: dragged to look about, scrolled
/// to come closer, a node's card shown on hovering over it, a click taking it if it's next to one
/// taken and a point's free, a right click giving it back if nothing taken hangs on it. `forget`
/// is whether all of them can be given back here, at a merchant, and what that costs.
pub fn window(ctx: &egui::Context, web: &Web, view: &mut View, taken: &[bool], free: i64, level: u32, forget: Option<u32>) -> Option<Action> {
    let mut action = None;
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("skill web")).fixed_pos(screen.min).order(egui::Order::Foreground).show(ctx, |ui| {
        let (rect, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(12, 10, 9, 234));
        if response.dragged() {
            view.offset += response.drag_delta();
        }
        let hovered_pos = response.hover_pos();
        if let Some(at) = hovered_pos {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let before = view.zoom;
                view.zoom = (view.zoom * (1.0 + scroll * 0.0015)).clamp(0.25, 2.0);
                // Closer about the pointer
                let about = at - rect.center() - view.offset;
                view.offset -= about * (view.zoom / before - 1.0);
            }
        }
        // A little under the middle, clear of what's said across the top
        let middle = rect.center() + vec2(0.0, 40.0) + view.offset;
        let place = |n: &Node| middle + vec2(n.x, n.y) * view.zoom;
        let radius = |n: &Node| {
            view.zoom
                * match n.kind {
                    Kind::Start => 26.0,
                    Kind::Keystone => 24.0,
                    Kind::Notable => 17.0,
                    Kind::Small => 10.0,
                }
        };
        // The links: bright between two taken, lit from one taken to what can be taken next
        for (k, links) in web.links.iter().enumerate() {
            for &j in links.iter().filter(|&&j| j > k) {
                let (a, b) = (place(&web.nodes[k]), place(&web.nodes[j]));
                if !rect.expand(40.0).contains(a) && !rect.expand(40.0).contains(b) {
                    continue;
                }
                let (stroke, tint) = if taken[k] && taken[j] {
                    (4.0, Color32::from_rgb(255, 214, 120))
                } else if taken[k] || taken[j] {
                    (2.5, Color32::from_rgb(150, 140, 110))
                } else {
                    (2.0, Color32::from_rgb(70, 62, 52))
                };
                painter.line_segment([a, b], Stroke::new(stroke * view.zoom.sqrt(), tint));
            }
        }
        // The nodes
        let mut hovered = None;
        for (k, n) in web.nodes.iter().enumerate() {
            let at = place(n);
            let r = radius(n);
            if !rect.expand(r).contains(at) {
                continue;
            }
            let color = branch_color(&n.branch);
            let open = web.can_take(taken, k);
            let near = hovered_pos.is_some_and(|p| (p - at).length() <= r + 4.0);
            if near {
                hovered = Some(k);
            }
            let fill = if taken[k] { color } else if open && free > 0 { color.gamma_multiply(0.35) } else { Color32::from_rgb(32, 28, 24) };
            let rim = if taken[k] {
                Color32::from_rgb(255, 230, 160)
            } else if open {
                color
            } else {
                color.gamma_multiply(0.45)
            };
            match n.kind {
                Kind::Keystone => {
                    let pts = (0..8).map(|i| at + egui::Vec2::angled(i as f32 * std::f32::consts::TAU / 8.0 + 0.39) * r).collect::<Vec<Pos2>>();
                    painter.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(3.0f32, rim)));
                }
                Kind::Notable => {
                    painter.circle_filled(at, r, fill);
                    painter.circle_stroke(at, r, Stroke::new(3.0f32, rim));
                    painter.circle_stroke(at, r + 4.0 * view.zoom, Stroke::new(1.0f32, rim.gamma_multiply(0.6)));
                }
                Kind::Start => {
                    painter.circle_filled(at, r, Color32::from_rgb(60, 52, 36));
                    painter.circle_stroke(at, r, Stroke::new(3.0f32, Color32::from_rgb(255, 214, 120)));
                }
                Kind::Small => {
                    painter.circle_filled(at, r, fill);
                    painter.circle_stroke(at, r, Stroke::new(2.0f32, rim));
                }
            }
            if near {
                painter.circle_stroke(at, r + 3.0, Stroke::new(2.0f32, Color32::WHITE));
            }
        }
        let hovered = hovered.or_else(|| view.hover.as_deref().and_then(|id| web.index(id)));
        // A click takes, a right click gives back
        if let Some(k) = hovered {
            if response.clicked() && web.can_take(taken, k) && free > 0 {
                action = Some(Action::Take(k));
            }
            if response.secondary_clicked() && web.can_refund(taken, k) {
                action = Some(Action::Refund(k));
            }
        }

        // Across the top: what it is, the points, and how
        let top = rect.top() + 18.0;
        painter.text(pos2(rect.center().x, top), Align2::CENTER_TOP, "Skill Web", crate::theme::display(30.0), crate::theme::HEADING);
        crate::theme::flourish(&painter, pos2(rect.center().x, top + 37.0), 110.0);
        let points = if free == 1 { "1 point to spend".to_owned() } else { format!("{free} points to spend") };
        painter.text(pos2(rect.center().x, top + 46.0), Align2::CENTER_TOP, format!("Level {level}  ·  {points}"), FontId::proportional(18.0), if free > 0 { crate::theme::INFO } else { crate::theme::TEXT });
        painter.text(
            pos2(rect.center().x, rect.bottom() - 16.0),
            Align2::CENTER_BOTTOM,
            "Click a lit node next to one taken to take it  ·  right click to give it back  ·  drag to look about, scroll to come closer  ·  P or Esc to close",
            FontId::proportional(14.0),
            crate::theme::WEAK,
        );
        // What each branch is, at its end
        let legend = [("Melee", "melee"), ("Ranged", "ranged"), ("Magic", "magic"), ("Movement and gathering", "mobility"), ("Defence", "defence")];
        for (k, (name, id)) in legend.iter().enumerate() {
            let y = rect.top() + 90.0 + k as f32 * 22.0;
            painter.circle_filled(pos2(rect.left() + 26.0, y + 8.0), 6.0, branch_color(id));
            painter.text(pos2(rect.left() + 40.0, y), Align2::LEFT_TOP, *name, FontId::proportional(15.0), crate::theme::TEXT);
        }
        // What all those taken add up to, down the right
        let totals = web.totals(taken);
        let mut y = rect.top() + 90.0;
        painter.text(pos2(rect.right() - 24.0, y), Align2::RIGHT_TOP, "All together", crate::theme::display(16.0), crate::theme::HEADING);
        y += 24.0;
        for stat in Stat::ALL {
            let v = totals.get(stat);
            if v.abs() > 1e-9 {
                painter.text(pos2(rect.right() - 24.0, y), Align2::RIGHT_TOP, stat.describe(v), FontId::proportional(13.0), crate::theme::TEXT);
                y += 18.0;
            }
        }
        // Forgetting them all, at a merchant
        if let Some(price) = forget {
            let button = Rect::from_min_size(pos2(rect.left() + 20.0, rect.bottom() - 70.0), vec2(270.0, 34.0));
            let over = hovered_pos.is_some_and(|p| button.contains(p));
            painter.rect_filled(button, 6.0, if over { Color32::from_rgb(90, 70, 40) } else { Color32::from_rgb(60, 48, 30) });
            painter.rect_stroke(button, 6.0, Stroke::new(1.5f32, Color32::from_rgb(255, 205, 70)), StrokeKind::Inside);
            painter.text(button.center(), Align2::CENTER_CENTER, format!("Forget them all  ·  {price} gold"), FontId::proportional(15.0), Color32::from_rgb(255, 205, 70));
            if over && response.clicked() {
                action = Some(Action::Forget);
            }
        }

        // The card of the node under the pointer
        if let Some(k) = hovered {
            let n = &web.nodes[k];
            let at = place(n) + vec2(radius(n) + 14.0, -10.0);
            let at = at.min(rect.right_bottom() - vec2(320.0, 160.0));
            egui::Area::new(egui::Id::new("skill card")).fixed_pos(at).order(egui::Order::Tooltip).interactable(false).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(crate::theme::panel_fill()).stroke(Stroke::new(1.5f32, branch_color(&n.branch))).show(ui, |ui| {
                    ui.set_max_width(300.0);
                    let kind = match n.kind {
                        Kind::Start => "Where every path begins",
                        Kind::Small => "",
                        Kind::Notable => "Notable",
                        Kind::Keystone => "Keystone",
                    };
                    ui.label(egui::RichText::new(&n.name).font(crate::theme::display(17.0)).color(branch_color(&n.branch)));
                    if !kind.is_empty() {
                        ui.label(egui::RichText::new(kind).italics().color(Color32::from_gray(170)));
                    }
                    for e in &n.effects {
                        let tint = if e.value < 0.0 && e.stat != Stat::ManaCost { Color32::from_rgb(240, 120, 100) } else { Color32::WHITE };
                        ui.label(egui::RichText::new(e.stat.describe(e.value)).color(tint));
                    }
                    let how = if taken[k] {
                        if k == web.start { "" } else if web.can_refund(taken, k) { "Taken  ·  right click to give it back" } else { "Taken  ·  others hang on it" }
                    } else if web.can_take(taken, k) {
                        if free > 0 { "Click to take it" } else { "No points to spend" }
                    } else {
                        "Take one next to it first"
                    };
                    if !how.is_empty() {
                        ui.label(egui::RichText::new(how).weak());
                    }
                });
            });
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = Some(Action::Close);
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_web_hangs_together() {
        let web = Web::load();
        let kinds = |k: Kind| web.nodes.iter().filter(|n| n.kind == k).count();
        assert!((120..=170).contains(&web.nodes.len()), "{} nodes", web.nodes.len());
        assert_eq!(kinds(Kind::Start), 1);
        assert!(kinds(Kind::Keystone) >= 8 && kinds(Kind::Notable) >= 20, "{} keystones, {} notables", kinds(Kind::Keystone), kinds(Kind::Notable));
        // Every node can be reached from the start, and every one but it gives something
        let mut taken = vec![false; web.nodes.len()];
        taken[web.start] = true;
        loop {
            let next = (0..web.nodes.len()).find(|&k| web.can_take(&taken, k));
            match next {
                Some(k) => taken[k] = true,
                None => break,
            }
        }
        assert!(taken.iter().all(|t| *t), "some nodes can't be reached");
        assert!(web.nodes.iter().all(|n| n.kind == Kind::Start || !n.effects.is_empty()));
        // Ids are each one's own
        for (k, n) in web.nodes.iter().enumerate() {
            assert_eq!(web.index(&n.id), Some(k));
        }
        // Every keystone has a price
        for n in web.nodes.iter().filter(|n| n.kind == Kind::Keystone) {
            assert!(n.effects.iter().any(|e| e.value < 0.0 && e.stat != Stat::ManaCost), "{} has no price", n.name);
        }
        // The branches link up away from the start: there's a way across from each to the next
        // that doesn't go through the start
        let mut without = vec![false; web.nodes.len()];
        let melee = web.index("melee_6").unwrap();
        without[melee] = true;
        let mut queue = VecDeque::from([melee]);
        while let Some(i) = queue.pop_front() {
            for &j in &web.links[i] {
                if j != web.start && !without[j] {
                    without[j] = true;
                    queue.push_back(j);
                }
            }
        }
        assert!(without.iter().enumerate().all(|(k, s)| *s || k == web.start), "the branches don't link away from the start");
        // A player at the cap with every bonus point fills about a third to a half of it
        let most = crate::levels::points_for(crate::levels::LEVEL_CAP) + crate::game::BONUS_POINTS_MOST;
        let share = most as f64 / (web.nodes.len() - 1) as f64;
        assert!((0.33..=0.47).contains(&share), "{most} points fill {:.0}% of the web", share * 100.0);
    }

    #[test]
    fn only_what_strands_nothing_is_given_back() {
        let web = Web::load();
        let mut taken = vec![false; web.nodes.len()];
        taken[web.start] = true;
        let path = ["melee_0", "melee_1", "melee_2", "melee_n3"].map(|id| web.index(id).unwrap());
        assert!(!web.can_take(&taken, path[1]), "not next to one taken");
        for &k in &path {
            assert!(web.can_take(&taken, k));
            taken[k] = true;
        }
        assert!(!web.can_refund(&taken, path[0]) && !web.can_refund(&taken, path[2]));
        assert!(web.can_refund(&taken, path[3]));
        assert!(!web.can_refund(&taken, web.start));
        assert_eq!(points_free(5, 1, &taken, web.start), 4 + 1 - 4);
        let totals = web.totals(&taken);
        assert!(totals.get(Stat::MeleeDamage) > 0.0 && totals.get(Stat::MeleeSpeed) > 0.0);
    }
}
