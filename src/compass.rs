//! Finding the way: in Realms the compass at the foot of the view, pointing to the nearest of the
//! realm's raid bosses still to beat, or once they're all beaten to the Innert Gate, where their
//! blocks are set in, or from a realm past the normal world to the portal home; and in either mode
//! the map on M, of the land round about seen from high above, with the gate or the portal, the
//! raid bosses' lairs and the player on it.

use std::f64::consts::{FRAC_PI_4, PI};
use std::sync::mpsc::{self, Receiver};

use egui_winit_vulkano::egui::{Color32, FontId, LayerId, Order, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle, TextureOptions, pos2, vec2};

use crate::realms::{self, REALMS};
use crate::worldgen::{self, Land};

use super::*;

/// What the compass points to: what it's called, how far off it is in meters, and how far round
/// it is from straight ahead, in radians clockwise, with which way the view faces, in radians
/// counterclockwise from east
pub struct Bearing {
    pub label: String,
    pub distance: f64,
    pub turn: f64,
    pub heading: f64,
}

/// How far round from straight ahead the compass shows either side, in radians
const COMPASS_SPAN: f64 = 100.0 * PI / 180.0;

/// The map's pixels across, and how many meters across it shows
const MAP_PIXELS: usize = 192;
const MAP_ACROSS_M: f64 = 720.0;

/// The map on M, see draw_map(): drawn away from the frame round where the player is when it's
/// opened, and again whenever they've gone a good way from there
#[derive(Default)]
pub struct Map {
    pub open: bool,
    drawing: Option<Receiver<MapImage>>,
    image: Option<MapImage>,
    texture: Option<TextureHandle>,
}

/// The land seen from above: its pixels, the middle of what they show in meters of the landscape,
/// and of which landscape
struct MapImage {
    pixels: Vec<u8>,
    middle: [f64; 2],
    seed: u32,
}

/// What's marked on the map, in meters of the landscape: the player and which way they face, the
/// gate or the portal, and each raid boss's lair with who's there and whether they're beaten
pub struct Marks {
    pub title: String,
    pub player: [f64; 2],
    pub heading: f64,
    pub gate: Option<([f64; 2], &'static str)>,
    pub lairs: Vec<([f64; 2], &'static str, bool)>,
    pub settings: TerrainSettings,
}

/// An angle brought into -PI to PI
fn wrapped(a: f64) -> f64 {
    (a + PI).rem_euclid(2.0 * PI) - PI
}

impl App {
    /// Where the player is, in meters of the whole landscape
    fn feet_m(&self) -> [f64; 2] {
        let vpm = self.world.voxels_per_meter as f64;
        let at = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        [(at.x + self.world.origin[0] as f64) / vpm, (at.y + self.world.origin[1] as f64) / vpm]
    }

    /// Which way the view faces, in radians counterclockwise from east
    fn heading(&self) -> f64 {
        self.camera.rotation.z + FRAC_PI_2
    }

    /// What the compass points to, in Realms: the nearest raid boss of the realm the player's in
    /// still to beat, or with all three beaten, the gate in the normal world, where their blocks are
    /// set in and the way on to the next realm is, or the portal home from a realm past it
    pub(super) fn compass(&self) -> Option<Bearing> {
        self.session.realms.as_ref()?;
        let places = self.session.places.as_ref()?;
        let at = self.feet_m();
        let far = |p: [f64; 2]| (p[0] - at[0]).hypot(p[1] - at[1]);
        let realm = &REALMS[places.realm];
        // Then, once they're all beaten, the realm's finale where it's risen, see on_raids_beaten()
        let finale = places.finale.zip(places.finale_key()).zip(realm.finale).filter(|((_, key), _)| !self.game.is_defeated(key)).map(|((spot, _), (look, _, at))| (spot, format!("{}, {at}", look.name())));
        let (spot, label) = (0..realms::RAIDS)
            .filter(|&k| !self.game.is_defeated(&places.lair_key(k)))
            .map(|k| (places.lairs[k], format!("Raid boss: {}, {}", realm.raid_bosses[k].0.name(), realm.lair_names[k])))
            .min_by(|a, b| far(a.0).total_cmp(&far(b.0)))
            .or(finale)
            .unwrap_or_else(|| (places.gate, if places.realm == 0 { "The Innert Gate" } else { "The portal home" }.to_owned()));
        // The realm's great serpent's lair instead, while it's still to beat and nearer, within a
        // couple of hundred meters, see serpents::realm_serpent()
        let serpent = places.serpent.zip(places.serpent_key()).filter(|(_, key)| !self.game.is_defeated(key)).map(|((look, spot), _)| (spot, format!("Serpent: {}", look.name())));
        let (spot, label) = match serpent {
            Some((lair, name)) if far(lair) < 200.0 && far(lair) < far(spot) => (lair, name),
            _ => (spot, label),
        };
        let heading = self.heading();
        let toward = (spot[1] - at[1]).atan2(spot[0] - at[0]);
        Some(Bearing { label, distance: far(spot), turn: wrapped(heading - toward), heading })
    }

    /// What's marked on the map
    pub(super) fn map_marks(&self) -> Marks {
        let places = self.session.places.as_ref().filter(|_| self.session.realms.is_some());
        let title = match (self.session.mode, places) {
            (Some(save::Mode::Realms), Some(p)) => REALMS[p.realm].name.to_owned(),
            _ => "Free Roam".to_owned(),
        };
        Marks {
            title,
            player: self.feet_m(),
            heading: self.heading(),
            gate: places.map(|p| (p.gate, if p.realm == 0 { "Innert Gate" } else { "Portal home" })),
            lairs: places.map_or(Vec::new(), |p| {
                let mut lairs: Vec<([f64; 2], &'static str, bool)> = (0..realms::RAIDS).map(|k| (p.lairs[k], REALMS[p.realm].raid_bosses[k].0.name(), self.game.is_defeated(&p.lair_key(k)))).collect();
                // The finale too, once it's risen
                if let (Some(spot), Some(key), Some((look, ..))) = (p.finale, p.finale_key(), REALMS[p.realm].finale)
                    && lairs.iter().all(|&(_, _, beaten)| beaten)
                {
                    lairs.push((spot, look.name(), self.game.is_defeated(&key)));
                }
                lairs
            })
            .into_iter()
            // The great serpents' lairs, see serpent_lairs.rs
            .chain(self.serpent_marks())
            .collect(),
            settings: self.terrain_settings(),
        }
    }

    /// Opens the map, or closes it
    pub(super) fn toggle_map(&mut self) {
        self.map.open = !self.map.open;
    }
}

impl Map {
    /// Has the land drawn afresh round `marks.player` if it's never been, or it's of another
    /// landscape, or they've gone a quarter of the way to its edge since
    fn redraw_if_wanted(&mut self, marks: &Marks) {
        if let Some(receiver) = &self.drawing {
            if let Ok(image) = receiver.try_recv() {
                self.image = Some(image);
                self.texture = None;
                self.drawing = None;
            }
            return;
        }
        // Another realm's land isn't shown at all while this one's is drawn
        if self.image.as_ref().is_some_and(|i| i.seed != marks.settings.seed) {
            (self.image, self.texture) = (None, None);
        }
        let stale = self.image.as_ref().is_none_or(|i| (i.middle[0] - marks.player[0]).abs().max((i.middle[1] - marks.player[1]).abs()) > MAP_ACROSS_M / 4.0);
        if stale {
            let (sender, receiver) = mpsc::channel();
            let (settings, middle) = (marks.settings, marks.player);
            std::thread::spawn(move || {
                let _ = sender.send(MapImage { pixels: land_seen_from_above(&settings, middle), middle, seed: settings.seed });
            });
            self.drawing = Some(receiver);
        }
    }
}

/// The land within MAP_ACROSS_M / 2 of `middle`, north up, MAP_PIXELS across: sea, ice and the
/// open air under the sky isles, and the land in the colors of its kind, lit from the north-west
/// so its hills stand out, and paler the higher it is. RGBA, a row at a time from the north.
fn land_seen_from_above(settings: &TerrainSettings, middle: [f64; 2]) -> Vec<u8> {
    let n = MAP_PIXELS;
    let cell = MAP_ACROSS_M / n as f64;
    let spot = |i: i64, j: i64| [middle[0] - MAP_ACROSS_M / 2.0 + (i as f64 + 0.5) * cell, middle[1] + MAP_ACROSS_M / 2.0 - (j as f64 + 0.5) * cell];
    // A pixel more all round, for lighting the edges
    let heights: Vec<Option<f32>> = (0..(n + 2) * (n + 2))
        .map(|k| {
            let [x, y] = spot((k % (n + 2)) as i64 - 1, (k / (n + 2)) as i64 - 1);
            worldgen::seen_from_above(settings, x as f32, y as f32)
        })
        .collect();
    let land = worldgen::land_of(settings.seed);
    let mut pixels = Vec::with_capacity(n * n * 4);
    for j in 0..n {
        for i in 0..n {
            let h = |di: i32, dj: i32| heights[(i as i32 + 1 + di) as usize + (j as i32 + 1 + dj) as usize * (n + 2)];
            let [x, y] = spot(i as i64, j as i64).map(|v| v as f32);
            let color: [f32; 3] = match h(0, 0) {
                // The sea far below the sky isles
                None => [0.16, 0.3, 0.5],
                Some(over) if over < 0.0 => {
                    if land == Land::Ice {
                        [0.78, 0.88, 0.95]
                    } else if land == Land::Dragon {
                        // Lava
                        [1.0, 0.42, 0.08]
                    } else {
                        let deep = (-over / 8.0).min(1.0);
                        let water = [0.18 - 0.08 * deep, 0.45 - 0.15 * deep, 0.7 - 0.15 * deep];
                        // Blood in the dreaming realm's red half
                        let blood = if land == Land::Dream { worldgen::dream_blood(x, y) } else { 0.0 };
                        [0, 1, 2].map(|k| water[k] + ([0.35, 0.02, 0.04][k] - water[k]) * blood)
                    }
                }
                Some(over) => {
                    let base = match land {
                        // Ash, darker up the peaks
                        Land::Dragon => [0.36, 0.33, 0.32],
                        Land::Dream => {
                            let blood = worldgen::dream_blood(x, y);
                            [0.78 - 0.2 * blood, 0.7 - 0.58 * blood, 0.9 - 0.72 * blood]
                        }
                        _ => {
                            let c = worldgen::climate(settings, x, y);
                            let temperate = [0.36, 0.6, 0.28];
                            let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t.clamp(0.0, 1.0));
                            let c1 = mix(temperate, [0.16, 0.45, 0.2], c.jungle);
                            let c2 = mix(c1, [0.85, 0.76, 0.52], c.desert);
                            mix(c2, [0.93, 0.95, 0.97], c.snow)
                        }
                    };
                    // Sand along the shore, rock and then snow up high, but on the sky isles, high
                    // over the sea and all of them green
                    let grounded = !matches!(land, Land::Sky | Land::Ice | Land::Dragon);
                    let shore = if grounded && over < 1.0 { 0.6 } else { 0.0 };
                    let high = if land == Land::Sky { 0.0 } else { ((over - settings.height_m as f32 * 0.25) / (settings.height_m as f32 * 0.3)).clamp(0.0, 1.0) };
                    let b = [0, 1, 2].map(|k| base[k] + ([0.85, 0.8, 0.6][k] - base[k]) * shore);
                    [0, 1, 2].map(|k| b[k] + ([0.8, 0.8, 0.82][k] - b[k]) * high * 0.6)
                }
            };
            // Lit from the north-west: brighter on slopes facing it, rising to the south-east
            let slope = match (h(-1, 0), h(1, 0), h(0, -1), h(0, 1)) {
                (Some(west), Some(east), Some(north), Some(south)) => ((east - west) + (south - north)) / cell as f32,
                _ => 0.0,
            };
            let light = (1.0 + slope.max(0.0).min(1.0) * 0.6 - (-slope).max(0.0).min(1.0) * 0.45).max(0.4);
            let sea = h(0, 0).is_none_or(|o| o < 0.0);
            let light = if sea { 1.0 } else { light };
            pixels.extend(color.map(|c| ((c * light).clamp(0.0, 1.0) * 255.0) as u8));
            pixels.push(255);
        }
    }
    pixels
}

/// The compass strip at the foot of the view: the points of the compass going by as the view
/// turns, and a mark where `bearing` is, kept to the strip's ends while it's behind, with what it
/// is and how far under it
pub fn draw_compass(ctx: &egui::Context, bearing: &Bearing) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(LayerId::new(Order::Background, egui::Id::new("compass")));
    let width = 440.0f32;
    let middle = pos2(screen.center().x, screen.bottom() - 74.0);
    let strip = Rect::from_center_size(middle, vec2(width, 24.0));
    painter.rect_filled(strip, 12.0, Color32::from_black_alpha(120));
    let x_of = |turn: f64| middle.x + (turn / COMPASS_SPAN) as f32 * width * 0.5;
    for k in 0..16 {
        let way = k as f64 * FRAC_PI_4 / 2.0;
        let turn = wrapped(bearing.heading - way);
        if turn.abs() > COMPASS_SPAN * 0.95 {
            continue;
        }
        let x = x_of(turn);
        let fade = (1.0 - (turn.abs() / COMPASS_SPAN) as f32 * 0.7) * 255.0;
        if k % 2 == 1 {
            painter.line_segment([pos2(x, middle.y - 4.0), pos2(x, middle.y + 4.0)], Stroke::new(1.0f32, Color32::from_white_alpha((fade * 0.5) as u8)));
            continue;
        }
        let name = ["E", "NE", "N", "NW", "W", "SW", "S", "SE"][k / 2];
        let (size, color) = if name == "N" { (15.0, Color32::from_rgba_unmultiplied(255, 120, 100, fade as u8)) } else { (if k % 4 == 0 { 14.0 } else { 11.0 }, Color32::from_white_alpha(fade as u8)) };
        painter.text(pos2(x, middle.y), Align2::CENTER_CENTER, name, FontId::proportional(size), color);
    }
    // Where it is: a gold mark over the strip, at its end pointing on round while it's behind
    let gold = Color32::from_rgb(255, 210, 110);
    let behind = bearing.turn.abs() > COMPASS_SPAN;
    let x = x_of(bearing.turn.clamp(-COMPASS_SPAN, COMPASS_SPAN));
    let top = strip.top() - 3.0;
    let mark = if behind {
        let s = bearing.turn.signum() as f32;
        vec![pos2(x + s * 9.0, middle.y), pos2(x - s * 3.0, middle.y - 9.0), pos2(x - s * 3.0, middle.y + 9.0)]
    } else {
        vec![pos2(x, top + 6.0), pos2(x - 8.0, top - 6.0), pos2(x + 8.0, top - 6.0)]
    };
    painter.add(Shape::convex_polygon(mark, gold, Stroke::new(1.5f32, Color32::from_black_alpha(170))));
    painter.text(pos2(middle.x, strip.bottom() + 13.0), Align2::CENTER_CENTER, format!("{}  ·  {:.0} m", bearing.label, bearing.distance), FontId::proportional(14.0), Color32::from_rgba_unmultiplied(255, 236, 200, 230));
}

/// The map, over the middle of the view: the land round about, north up, with the gate or the
/// portal, the raid bosses' lairs, beaten or not, and the player and which way they face on it
pub fn draw_map(ctx: &egui::Context, map: &mut Map, marks: &Marks) {
    map.redraw_if_wanted(marks);
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::new("map")));
    // Under the quick bar and what's in hand, its name over it and how to close it under it, all
    // in its frame
    let top = screen.top() + 190.0;
    let side = (screen.bottom() - top - 60.0).min(screen.width() * 0.8).min(680.0).max(120.0);
    let rect = Rect::from_min_size(pos2(screen.center().x - side / 2.0, top), vec2(side, side));
    let frame = Rect::from_min_max(rect.min - vec2(14.0, 44.0), rect.max + vec2(14.0, 34.0));
    painter.rect_filled(frame, 14.0, Color32::from_black_alpha(200));
    painter.text(pos2(rect.center().x, rect.top() - 22.0), Align2::CENTER_CENTER, &marks.title, FontId::proportional(22.0), Color32::WHITE);
    painter.text(pos2(rect.center().x, rect.bottom() + 18.0), Align2::CENTER_CENTER, "M to close", FontId::proportional(13.0), Color32::from_gray(170));
    let Some(image) = &map.image else {
        painter.text(rect.center(), Align2::CENTER_CENTER, "Drawing the map...", FontId::proportional(18.0), Color32::from_gray(200));
        return;
    };
    let texture = map.texture.get_or_insert_with(|| ctx.load_texture("map", egui::ColorImage::from_rgba_unmultiplied([MAP_PIXELS, MAP_PIXELS], &image.pixels), TextureOptions::LINEAR));
    painter.image(texture.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    painter.rect_stroke(rect, 2.0, Stroke::new(1.5f32, Color32::from_white_alpha(90)), StrokeKind::Outside);
    let to_screen = |p: [f64; 2]| -> Pos2 {
        let u = (p[0] - image.middle[0]) / MAP_ACROSS_M + 0.5;
        let v = 0.5 - (p[1] - image.middle[1]) / MAP_ACROSS_M;
        pos2(rect.left() + u as f32 * side, rect.top() + v as f32 * side)
    };
    let inside = |at: Pos2| rect.shrink(4.0).contains(at);
    let label = |at: Pos2, text: &str, color: Color32| {
        painter.text(at + vec2(0.0, 15.0), Align2::CENTER_CENTER, text, FontId::proportional(13.0), Color32::from_black_alpha(200));
        painter.text(at + vec2(0.0, 14.0), Align2::CENTER_CENTER, text, FontId::proportional(13.0), color);
    };
    if let Some((spot, name)) = marks.gate {
        let at = to_screen(spot);
        if inside(at) {
            painter.circle_filled(at, 7.0, Color32::from_rgb(150, 110, 255));
            painter.circle_stroke(at, 10.0, Stroke::new(2.0f32, Color32::from_rgb(230, 215, 255)));
            label(at, name, Color32::from_rgb(230, 215, 255));
        }
    }
    for &(spot, name, beaten) in &marks.lairs {
        let at = to_screen(spot);
        if !inside(at) {
            continue;
        }
        if beaten {
            for d in [vec2(6.0, 6.0), vec2(6.0, -6.0)] {
                painter.line_segment([at - d, at + d], Stroke::new(2.5f32, Color32::from_gray(150)));
            }
            label(at, name, Color32::from_gray(170));
        } else {
            painter.circle_filled(at, 10.5, Color32::from_black_alpha(170));
            painter.circle_filled(at, 7.0, Color32::from_rgb(230, 40, 40));
            painter.circle_filled(at, 2.5, Color32::WHITE);
            painter.circle_stroke(at, 9.0, Stroke::new(1.5f32, Color32::from_rgb(255, 210, 110)));
            label(at, name, Color32::from_rgb(255, 210, 110));
        }
    }
    // The player, an arrow the way they face
    let at = to_screen(marks.player);
    if inside(at) {
        let way = vec2(marks.heading.cos() as f32, -marks.heading.sin() as f32);
        let across = vec2(-way.y, way.x);
        let arrow = vec![at + way * 11.0, at - way * 7.0 + across * 7.0, at - way * 7.0 - across * 7.0];
        painter.add(Shape::convex_polygon(arrow, Color32::WHITE, Stroke::new(1.5f32, Color32::BLACK)));
    }
    // North
    painter.text(pos2(rect.right() - 16.0, rect.top() + 16.0), Align2::CENTER_CENTER, "N", FontId::proportional(16.0), Color32::from_rgb(255, 120, 100));
}
