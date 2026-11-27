//! The land answering the mouse, see land.rs. The column under it rises and whitens, those round
//! it a little less, as if pressed up from below, and settles back once it's gone. Coming onto a
//! column sets it bobbing, which spreads out over the land as a ripple, through the sea too, and a
//! click knocks a bigger ring out round it.
//!
//! The ripples are waves on the grid of columns, each pulled by those round it and back to where
//! it rests, worked out a set number of times a second so they look the same however often it's
//! drawn. Once it's all settled and the mouse is off the land nothing's worked out at all.

use std::time::{Duration, Instant};

use egui_winit_vulkano::egui::{self, Pos2, pos2};

use super::land::{HEADROOM, Land, Spot};
use crate::settings_from_args;

/// How high the column under the mouse is lifted, in columns, and how far round it the hump
/// spreads, in columns
const HOVER_LIFT: f32 = 1.1;
const HOVER_SPREAD: f32 = 0.9;
/// How quickly what's under the mouse comes up, and what it's left goes back down, each a second
const RISE_RATE: f32 = 16.0;
const SETTLE_RATE: f32 = 4.5;
/// How hard a column's set bobbing as the mouse comes onto it, and knocked by a click, in columns
/// a second
const NUDGE: f32 = 3.2;
const KNOCK: f32 = 22.0;
/// How far round where it's clicked the knock's felt, in columns
const KNOCK_REACH: f32 = 2.5;
/// The waves: how hard a column's pulled level with those round it, which sets how fast a ripple
/// spreads, about the root of it in columns a second; how hard it's pulled back to where it rests;
/// and how quickly its bobbing dies away
const PULL: f32 = 60.0;
const SPRING: f32 = 10.0;
const DAMPING: f32 = 2.6;
/// The waves are worked out this many times a second, however often it's drawn
const STEPS_PER_SECOND: f32 = 120.0;
/// Less than this up or down, it's all settled
const STILL: f32 = 2e-3;
/// How long after the mouse last moved over it the land's still drawn as often as the screen
/// shows, see lively()
const LIVELY: Duration = Duration::from_millis(1200);

/// How the land's been touched: how far each column's lifted, row after row as Land has them
pub struct Touch {
    /// Columns across each way of the land it's for
    n: usize,
    /// Lifted by the mouse being over or near it, in columns
    raised: Vec<f32>,
    /// Lifted by the ripples going through it, in columns, and how fast that's changing
    wave: Vec<f32>,
    speed: Vec<f32>,
    /// Where on the land the mouse is, if it's over it
    over: Option<Spot>,
    /// Where on the screen the mouse was last
    pointer: Option<Pos2>,
    /// Seconds still to be worked out, less than a step
    owed: f32,
    /// Whether anything's still moving, or the mouse is over it
    awake: bool,
    /// When it was last moved on, and when the mouse last moved over it
    last: Option<Instant>,
    stirred: Option<Instant>,
    /// Seconds it's been moved on, which a scripted mouse is timed by
    clock: f32,
    /// A mouse moved by a script, in screenshot runs, and where it is, over a window or not
    script: Option<Script>,
    scripted: Option<Pos2>,
}

impl Default for Touch {
    fn default() -> Self {
        Touch::new(Script::from_args("hover"))
    }
}

impl Touch {
    fn new(script: Option<Script>) -> Self {
        Touch {
            n: 0,
            raised: Vec::new(),
            wave: Vec::new(),
            speed: Vec::new(),
            over: None,
            pointer: None,
            owed: 0.0,
            awake: false,
            last: None,
            stirred: None,
            clock: 0.0,
            script,
            scripted: None,
        }
    }

    /// Gone behind the menu, as it was, though a scripted mouse starts over with the menu's own
    /// script, `--set menu_hover_path=...` and so on, see Script
    pub fn behind_menu(mut self) -> Self {
        if self.script.is_some() || Script::from_args("menu_hover").is_some() {
            self.script = Script::from_args("menu_hover");
            self.clock = 0.0;
        }
        self
    }

    /// Marks where a scripted mouse is, in screenshot runs, where no real one's drawn
    pub fn mark(&self, painter: &egui::Painter) {
        if let (Some(_), Some(p)) = (&self.script, self.scripted) {
            let arrow = [p, p + egui::vec2(0.0, 16.0), p + egui::vec2(11.0, 11.0)];
            painter.add(egui::Shape::convex_polygon(arrow.to_vec(), egui::Color32::WHITE, egui::Stroke::new(1.0f32, egui::Color32::BLACK)));
        }
    }

    /// How far column `k` is lifted, in columns
    pub fn lift(&self, k: usize) -> f32 {
        match (self.raised.get(k), self.wave.get(k)) {
            (Some(r), Some(w)) => (r + w).clamp(-0.6, HEADROOM + 0.6),
            _ => 0.0,
        }
    }

    /// How far a ripple's lifted column `k`, in columns, to ripple the sea over it
    pub fn ripple(&self, k: usize) -> f32 {
        self.wave.get(k).copied().unwrap_or(0.0)
    }

    /// How much column `k`'s lit by the mouse being over it or a ripple lifting it, 0 to 1
    pub fn glow(&self, k: usize) -> f32 {
        (self.raised.get(k).copied().unwrap_or(0.0) / HOVER_LIFT).clamp(0.0, 1.0).powf(1.5)
    }

    /// Where on the land the mouse is, if it's over it
    pub fn over(&self) -> Option<Spot> {
        self.over
    }

    /// Where on the screen the mouse is, if it's over the land or near it, as a scripted one too
    pub fn pointer(&self) -> Option<Pos2> {
        self.pointer
    }

    /// Made for a land `n` columns across, all settled, if it was for another
    fn fit(&mut self, n: usize) {
        if self.n != n {
            self.n = n;
            for v in [&mut self.raised, &mut self.wave, &mut self.speed] {
                *v = vec![0.0; n * n];
            }
            self.over = None;
            self.awake = false;
        }
    }

    /// Whether the mouse has lately moved over it, or a ripple's still big, when it's worth drawing
    /// as often as the screen shows rather than as it's drawn while nothing's touched
    pub fn lively(&self) -> bool {
        self.awake && (self.stirred.is_some_and(|s| s.elapsed() < LIVELY) || self.wave.iter().any(|w| w.abs() > 0.05))
    }

    /// Feels the mouse over `land` seen as `view`, `phase` of the way built, and moves on to now.
    /// The mouse is only felt when it's `free`, not taken by the game to look round with, and not
    /// over a window, as the menu.
    pub fn feel(&mut self, ctx: &egui::Context, land: &Land, view: &super::land::View, phase: super::land::Phase, free: bool) {
        let (pointer, pressed) = match &self.script {
            Some(script) => script.at(self.clock, ctx.screen_rect()),
            None if free => ctx.input(|i| (i.pointer.hover_pos(), i.pointer.primary_pressed())),
            None => (None, false),
        };
        self.scripted = pointer.filter(|_| self.script.is_some());
        // What's on the background, as the land itself, doesn't count
        let pointer = pointer.filter(|&p| ctx.layer_id_at(p).is_none_or(|layer| layer.order == egui::Order::Background));
        let over = pointer.and_then(|p| land.hit(view, phase, p));
        self.step(land, pointer, over, pressed && over.is_some(), None);
    }

    /// Moves it on to now, or by `dt` seconds if given, the mouse at `pointer` on the screen and
    /// over `over` on the land, which it's just `pressed` on
    fn step(&mut self, land: &Land, pointer: Option<Pos2>, over: Option<Spot>, pressed: bool, dt: Option<f32>) {
        self.fit(land.across());
        let now = Instant::now();
        let dt = dt.unwrap_or(if self.script.is_some() { 1.0 / 60.0 } else { self.last.map_or(0.0, |l| (now - l).as_secs_f32()).min(0.1) });
        self.last = Some(now);
        self.clock += dt;
        if over.is_some() && (pressed || pointer != self.pointer) {
            self.stirred = Some(now);
        }
        self.pointer = pointer;
        if let Some(spot) = over {
            // Coming onto a column sets it bobbing
            if self.over.is_none_or(|was| was.column != spot.column) {
                let (i, j) = spot.column;
                self.speed[(i + j * self.n as i32) as usize] += NUDGE;
                self.awake = true;
            }
            // And a click knocks a ring out round it
            if pressed {
                self.knock(land, spot.at);
            }
        }
        self.over = over;
        if !self.awake && over.is_none() {
            self.owed = 0.0;
            return;
        }
        self.awake = true;
        const H: f32 = 1.0 / STEPS_PER_SECOND;
        self.owed += dt;
        let mut most = None;
        // Not a step short for the time being added up a little under it
        while self.owed >= H - 1e-5 {
            self.owed -= H;
            most = Some(self.tick(land, H));
        }
        if over.is_none() && most.is_some_and(|m| m < STILL) {
            // All settled: nothing's worked out till it's touched again
            self.awake = false;
            self.raised.fill(0.0);
            self.wave.fill(0.0);
            self.speed.fill(0.0);
        }
    }

    /// A click at `at`, in columns from the middle: the columns round it knocked upward, the more
    /// the nearer
    fn knock(&mut self, land: &Land, at: (f32, f32)) {
        for k in 0..self.n * self.n {
            let (x, y) = middle_of(self.n, k);
            let d = (x - at.0).hypot(y - at.1);
            if d < KNOCK_REACH && land.solid(k) {
                self.speed[k] += KNOCK * (1.0 - d / KNOCK_REACH).powi(2);
            }
        }
        self.awake = true;
    }

    /// Works it all out `h` seconds on, giving back how far anything's still from resting
    fn tick(&mut self, land: &Land, h: f32) -> f32 {
        let n = self.n as i32;
        let (rise, settle) = (1.0 - (-RISE_RATE * h).exp(), 1.0 - (-SETTLE_RATE * h).exp());
        let mut most = 0.0f32;
        // The hump under the mouse
        for k in 0..self.n * self.n {
            let target = match self.over {
                Some(spot) if land.solid(k) => {
                    let (x, y) = middle_of(self.n, k);
                    let d2 = (x - spot.at.0).powi(2) + (y - spot.at.1).powi(2);
                    if d2 < 16.0 * HOVER_SPREAD * HOVER_SPREAD { HOVER_LIFT * (-d2 / (2.0 * HOVER_SPREAD * HOVER_SPREAD)).exp() } else { 0.0 }
                }
                _ => 0.0,
            };
            let r = &mut self.raised[k];
            *r += (target - *r) * if target > *r { rise } else { settle };
            most = most.max(r.abs());
        }
        // The ripples: each column pulled level with those round it that are there, so a ripple
        // comes back off the land's edges, and back to where it rests
        for k in 0..self.n * self.n {
            if !land.solid(k) {
                continue;
            }
            let (i, j) = ((k % self.n) as i32, (k / self.n) as i32);
            let mut level = 0.0;
            for (a, b) in [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)] {
                if a >= 0 && b >= 0 && a < n && b < n && land.solid((a + b * n) as usize) {
                    level += self.wave[(a + b * n) as usize] - self.wave[k];
                }
            }
            self.speed[k] += (PULL * level - SPRING * self.wave[k] - DAMPING * self.speed[k]) * h;
        }
        for k in 0..self.n * self.n {
            self.wave[k] += self.speed[k] * h;
            most = most.max(self.wave[k].abs()).max(self.speed[k].abs() * 0.1);
        }
        most
    }
}

/// The middle of column `k` of a land `n` columns across, in columns from its middle
fn middle_of(n: usize, k: usize) -> (f32, f32) {
    let half = n as f32 * 0.5;
    ((k % n) as f32 + 0.5 - half, (k / n) as f32 + 0.5 - half)
}

/// A mouse moved by a script, to see the land answer it in screenshot runs, where there's no
/// mouse: `--set hover_path=0.3,0.6/0.5,0.5/0.7,0.4` takes it through those points, each that far
/// across and down the window, `hover_leg=1.5` seconds from each to the next (1 if not given),
/// staying on the last as long, then off the window. It starts `hover_from=8` seconds after the
/// land was first drawn, and clicks `hover_click=1.2/3` seconds after that. Behind the menu it's
/// `menu_hover_path` and so on, timed from when the menu's first drawn.
#[derive(Clone, Debug, PartialEq)]
struct Script {
    points: Vec<[f32; 2]>,
    leg: f32,
    from: f32,
    clicks: Vec<f32>,
}

impl Script {
    /// As the settings starting `name` have it
    fn from_args(name: &str) -> Option<Script> {
        let settings = settings_from_args();
        let get = |key: &str| settings.iter().find(|(k, _)| *k == format!("{name}_{key}")).map(|(_, v)| v.as_str());
        Script::parse(get("path")?, get("leg"), get("from"), get("click"))
    }

    fn parse(path: &str, leg: Option<&str>, from: Option<&str>, clicks: Option<&str>) -> Option<Script> {
        let list = |s: &str| s.split(['/', ';', ' ']).filter(|v| !v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let points: Vec<[f32; 2]> = list(path)
            .iter()
            .filter_map(|p| {
                let v: Vec<f32> = p.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                (v.len() == 2).then(|| [v[0], v[1]])
            })
            .collect();
        if points.is_empty() {
            return None;
        }
        let number = |s: Option<&str>, default: f32| s.and_then(|s| s.trim().parse().ok()).unwrap_or(default);
        let clicks = clicks.map_or(Vec::new(), |c| list(c).iter().filter_map(|n| n.parse().ok()).collect());
        Some(Script { points, leg: number(leg, 1.0).max(0.05), from: number(from, 0.0), clicks })
    }

    /// Where the mouse is `clock` seconds on, in a window `screen` big, and whether it's clicked
    /// in the next sixtieth of a second
    fn at(&self, clock: f32, screen: egui::Rect) -> (Option<Pos2>, bool) {
        let s = clock - self.from;
        let pressed = self.clicks.iter().any(|&c| s >= c && s < c + 1.0 / 60.0);
        let legs = self.points.len() - 1;
        if s < 0.0 || s > (legs + 1) as f32 * self.leg {
            return (None, false);
        }
        let leg = ((s / self.leg) as usize).min(legs);
        let (a, b) = (self.points[leg], self.points[(leg + 1).min(legs)]);
        let along = super::smoothstep(s / self.leg - leg as f32);
        let at = [a[0] + (b[0] - a[0]) * along, a[1] + (b[1] - a[1]) * along];
        (Some(pos2(screen.min.x + at[0] * screen.width(), screen.min.y + at[1] * screen.height())), pressed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Columns across each way of the loading screen's land
    const N: usize = 34;

    fn flat() -> Land {
        Land::flat_for_tests(6.0)
    }

    fn column(k: (i32, i32)) -> usize {
        (k.0 + k.1 * N as i32) as usize
    }

    fn over(k: (i32, i32)) -> Option<Spot> {
        let (x, y) = middle_of(N, column(k));
        Some(Spot { column: k, at: (x, y) })
    }

    #[test]
    fn the_column_under_the_mouse_rises_most_and_settles_back() {
        let land = flat();
        let mut touch = Touch::new(None);
        let under = (17, 17);
        for _ in 0..30 {
            touch.step(&land, Some(pos2(1.0, 1.0)), over(under), false, Some(1.0 / 60.0));
        }
        let lift = |t: &Touch, k: (i32, i32)| t.raised[column(k)];
        assert!(lift(&touch, under) > 0.7 * HOVER_LIFT, "{}", lift(&touch, under));
        // Those next to it less, those further off hardly at all
        assert!(lift(&touch, (18, 17)) < lift(&touch, under) && lift(&touch, (18, 17)) > 0.3 * HOVER_LIFT);
        assert!(lift(&touch, (22, 17)) < 0.02);
        assert!(touch.glow(column(under)) > touch.glow(column((19, 17))));
        // Gone, it all settles, and then nothing's worked out at all
        for _ in 0..600 {
            touch.step(&land, None, None, false, Some(1.0 / 60.0));
        }
        assert!(!touch.awake);
        assert!((0..N * N).all(|k| touch.lift(k) == 0.0));
    }

    #[test]
    fn a_click_sends_a_ring_out() {
        let land = flat();
        let mut touch = Touch::new(None);
        let at = (17, 17);
        touch.step(&land, Some(pos2(1.0, 1.0)), over(at), true, Some(1.0 / 60.0));
        touch.step(&land, None, None, false, Some(1.0 / 60.0));
        // When the ripple's highest, near where it was clicked and further off
        let mut peak = [(0.0f32, 0usize); 2];
        for frame in 0..240 {
            touch.step(&land, None, None, false, Some(1.0 / 60.0));
            for (p, k) in peak.iter_mut().zip([(18, 17), (23, 17)]) {
                let w = touch.wave[column(k)];
                if w > p.0 {
                    *p = (w, frame);
                }
            }
        }
        assert!(peak[0].0 > 0.1 && peak[1].0 > 0.02, "{peak:?}");
        assert!(peak[1].1 > peak[0].1, "further off, it comes later: {peak:?}");
        assert!(peak[1].0 < peak[0].0, "and smaller: {peak:?}");
    }

    #[test]
    fn it_never_runs_away() {
        // However hard and long it's stirred, however unevenly it's drawn
        let land = flat();
        let mut touch = Touch::new(None);
        for frame in 0..2000 {
            let k = (((frame * 7) % 30 + 2) as i32, ((frame * 3) % 30 + 2) as i32);
            let dt = [0.004, 0.016, 0.033, 0.1][frame % 4];
            touch.step(&land, Some(pos2(frame as f32, 0.0)), over(k), frame % 5 == 0, Some(dt));
            assert!((0..N * N).all(|k| touch.wave[k].is_finite() && touch.wave[k].abs() < 3.0), "at {frame}");
        }
    }

    #[test]
    fn the_same_however_often_its_drawn() {
        let land = flat();
        let (mut slow, mut fast) = (Touch::new(None), Touch::new(None));
        slow.step(&land, Some(pos2(1.0, 1.0)), over((10, 12)), true, Some(1.0 / 30.0));
        fast.step(&land, Some(pos2(1.0, 1.0)), over((10, 12)), true, Some(1.0 / 30.0));
        for _ in 0..30 {
            slow.step(&land, None, None, false, Some(1.0 / 30.0));
            fast.step(&land, None, None, false, Some(1.0 / 120.0));
            fast.step(&land, None, None, false, Some(1.0 / 120.0));
            fast.step(&land, None, None, false, Some(1.0 / 120.0));
            fast.step(&land, None, None, false, Some(1.0 / 120.0));
        }
        let k = column((14, 12));
        assert!((slow.wave[k] - fast.wave[k]).abs() < 1e-4, "{} against {}", slow.wave[k], fast.wave[k]);
    }

    #[test]
    fn a_scripted_mouse_goes_along_its_path() {
        let script = Script::parse("0.25,0.5/0.75,0.5", Some("2"), Some("1"), Some("0.5/3")).unwrap();
        let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        assert_eq!(script.at(0.5, screen), (None, false));
        assert_eq!(script.at(1.0, screen).0, Some(pos2(200.0, 300.0)));
        assert_eq!(script.at(2.0, screen).0, Some(pos2(400.0, 300.0)));
        // Staying on the last a leg's time, then gone
        assert_eq!(script.at(4.5, screen).0, Some(pos2(600.0, 300.0)));
        assert_eq!(script.at(5.5, screen).0, None);
        assert!(script.at(1.5, screen).1 && !script.at(1.6, screen).1 && script.at(4.0, screen).1);
        assert!(Script::parse("nothing", None, None, None).is_none());
    }
}
