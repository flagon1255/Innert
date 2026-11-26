//! The window a new character's made in, between picking New from the menu and the world it's
//! played in: their name, how they look, what they set out as and how hard it's to be, with the
//! body turning round beside it as it's chosen, see body::picture(). Start begins the game with
//! them, Back goes back to the menu. It fits a short window: what doesn't fit scrolls, the body and
//! the buttons staying where they are.

use egui_winit_vulkano::egui::{self, Align2, Color32, RichText, Sense, Stroke, TextureOptions, pos2, vec2};
use nalgebra::Vector3;

use super::body;
use super::{BUILDS, CLOTHS, Character, Class, Difficulty, FACES, GLOVES, HAIR_COLORS, HAIRS, Looks, NAME_LENGTH, SKINS};
use crate::animals::Model;
use crate::save::Mode;

/// How wide the picture of the body is drawn, in pixels, before it's scaled to fit
const PICTURE: usize = 260;

/// What the window keeps between frames
pub struct Creator {
    /// The mode the new game's to be of
    pub mode: Mode,
    pub character: Character,
    /// How far round the body's turned in its picture, in radians
    pub turn: f64,
    /// Turning by itself, till it's dragged
    spinning: bool,
    models: Option<(Looks, Vec<(Model, Vector3<f64>)>)>,
    texture: Option<(egui::TextureHandle, Looks, i64)>,
    /// What the dice give next
    rolls: u32,
    /// What each family of weapons is called, for what each class starts with
    families: std::collections::HashMap<String, String>,
    /// Scrolled down this far through what's chosen next frame, for screenshot runs
    pub scroll_to: Option<f32>,
}

/// What's picked in it
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    Back,
    Start,
}

impl Creator {
    /// For a new game of `mode`, starting from a character made up from `seed`
    pub fn new(mode: Mode, seed: u32) -> Creator {
        // A fighter to start with: the wanderer of old saves isn't offered any longer
        let character = Character { name: super::random_name(seed), class: Class::Fighter, ..Default::default() };
        Creator { mode, character, turn: 0.45, spinning: true, models: None, texture: None, rolls: seed, families: family_names(), scroll_to: None }
    }

    /// For screenshot runs: starting with `character`, turned `turn` radians round, still
    pub fn with(mode: Mode, mut character: Character, turn: f64) -> Creator {
        if !character.class.offered() {
            character.class = Class::Fighter;
        }
        Creator { mode, character, turn, spinning: false, models: None, texture: None, rolls: 1, families: family_names(), scroll_to: None }
    }

    fn roll(&mut self) -> u32 {
        self.rolls = self.rolls.wrapping_mul(0x9e37_79b9).wrapping_add(0x7f4a_7c15) ^ (self.rolls >> 13);
        self.rolls
    }

    /// The picture of the body as it looks now, drawn again only when it's changed or turned a
    /// little further
    fn picture(&mut self, ctx: &egui::Context) -> egui::TextureId {
        let looks = self.character.looks;
        let step = (self.turn * 48.0).round() as i64;
        if self.models.as_ref().is_none_or(|(l, _)| *l != looks) {
            self.models = Some((looks, body::models(&looks)));
        }
        let fresh = self.texture.as_ref().is_some_and(|(_, l, s)| *l == looks && *s == step);
        if !fresh {
            let models = &self.models.as_ref().unwrap().1;
            let (rgba, size) = body::picture(models, &looks, step as f64 / 48.0, PICTURE);
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
            match &mut self.texture {
                Some((handle, l, s)) => {
                    handle.set(image, TextureOptions::LINEAR);
                    (*l, *s) = (looks, step);
                }
                None => self.texture = Some((ctx.load_texture("character", image, TextureOptions::LINEAR), looks, step)),
            }
        }
        self.texture.as_ref().unwrap().0.id()
    }
}

/// What each family of weapons is called, by its id
fn family_names() -> std::collections::HashMap<String, String> {
    crate::weapons::Library::load().families.iter().map(|f| (f.id.clone(), f.display_name.clone())).collect()
}

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// A heading over a part of the window
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(text).size(19.0).strong().color(Color32::from_rgb(255, 215, 130)));
    ui.add_space(2.0);
}

/// A row of swatches of `colors` to pick one of, `picked` being which: its name shown on hover
fn swatches(ui: &mut egui::Ui, label: &str, colors: &[(&str, [u8; 3])], picked: &mut u8, bare: Option<[u8; 3]>) {
    ui.horizontal_wrapped(|ui| {
        ui.add_sized([86.0, 22.0], egui::Label::new(RichText::new(label).size(14.0).color(Color32::from_gray(200))));
        for (k, &(name, rgb)) in colors.iter().enumerate() {
            let (rect, response) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
            let fill = if k == 0 { bare.unwrap_or(rgb) } else { rgb };
            ui.painter().rect_filled(rect.shrink(2.0), 4.0, color(fill));
            // Bare hands are the skin's colour, crossed with a line of the glove there isn't
            if k == 0 && bare.is_some() {
                ui.painter().line_segment([rect.left_bottom() + vec2(5.0, -5.0), rect.right_top() + vec2(-5.0, 5.0)], Stroke::new(1.5f32, Color32::from_gray(40)));
            }
            let chosen = *picked as usize == k;
            ui.painter().rect_stroke(rect.shrink(1.0), 5.0, Stroke::new(if chosen { 2.5f32 } else { 1.0 }, if chosen { Color32::WHITE } else { Color32::from_gray(70) }), egui::StrokeKind::Inside);
            if response.on_hover_text(name).clicked() {
                *picked = k as u8;
            }
        }
    });
}

/// A row of words to pick one of
fn choices(ui: &mut egui::Ui, label: &str, names: &[&str], picked: &mut u8) {
    ui.horizontal_wrapped(|ui| {
        ui.add_sized([86.0, 22.0], egui::Label::new(RichText::new(label).size(14.0).color(Color32::from_gray(200))));
        for (k, name) in names.iter().enumerate() {
            if ui.selectable_label(*picked as usize == k, RichText::new(*name).size(14.0)).clicked() {
                *picked = k as u8;
            }
        }
    });
}

/// The window, over everything, its picks given back
pub fn window(ctx: &egui::Context, creator: &mut Creator) -> Option<Pick> {
    let mut picked = None;
    let screen = ctx.screen_rect();
    // Dim the world behind it, more than the menu does, as there's more to read
    ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha(150));
    let margin = 24.0;
    let wide = (screen.width() - 2.0 * margin).clamp(320.0, 1120.0);
    let tall = (screen.height() - 2.0 * margin).max(260.0);
    if creator.spinning {
        creator.turn += ctx.input(|i| i.stable_dt as f64).min(0.1) * 0.5;
        ctx.request_repaint();
    }
    let texture = creator.picture(ctx);
    egui::Area::new(egui::Id::new("character creation")).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        egui::Frame::new().fill(Color32::from_black_alpha(205)).corner_radius(16.0).inner_margin(20.0).show(ui, |ui| {
            ui.set_width(wide - 40.0);
            ui.set_max_height(tall - 40.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("A new character").size(28.0).strong().color(Color32::WHITE));
                ui.add_space(10.0);
                ui.label(RichText::new(format!("for a new {} game", creator.mode.name())).size(15.0).color(Color32::from_gray(180)));
            });
            ui.add_space(6.0);
            // The picture, as tall as the window lets it be, beside everything that's chosen
            let footer = 54.0;
            let body_height = (tall - 40.0 - 44.0 - footer).max(150.0);
            let picture_height = body_height.min(470.0);
            let picture_width = picture_height * 5.0 / 7.0;
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(picture_width);
                    let (rect, response) = ui.allocate_exact_size(vec2(picture_width, picture_height - 34.0), Sense::drag());
                    ui.painter().image(texture, rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0f32, Color32::from_gray(60)), egui::StrokeKind::Inside);
                    if response.dragged() {
                        creator.turn += response.drag_delta().x as f64 * 0.012;
                        creator.spinning = false;
                    }
                    ui.painter().text(rect.center_bottom() - vec2(0.0, 10.0), Align2::CENTER_BOTTOM, "drag to turn", egui::FontId::proportional(12.0), Color32::from_gray(150));
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("⟲").size(16.0)).on_hover_text("Turn left").clicked() {
                            creator.turn -= 0.4;
                            creator.spinning = false;
                        }
                        if ui.button(RichText::new("⟳").size(16.0)).on_hover_text("Turn right").clicked() {
                            creator.turn += 0.4;
                            creator.spinning = false;
                        }
                        if ui.button(RichText::new("Random looks").size(14.0)).clicked() {
                            let seed = creator.roll();
                            creator.character.looks = Looks::random(seed);
                        }
                    });
                });
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    // Its scroll bar always there and solid, not only once the mouse is over it, so
                    // in a short window it's plain there's more below: the last classes and how hard
                    // it's to be
                    ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
                    let mut area = egui::ScrollArea::vertical().max_height(body_height).auto_shrink([false, false]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);
                    if let Some(y) = creator.scroll_to.take() {
                        area = area.vertical_scroll_offset(y);
                    }
                    area.show(ui, |ui| {
                        choose(ui, creator);
                    });
                });
            });
            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_sized([130.0, 38.0], egui::Button::new(RichText::new("Back").size(18.0))).clicked() {
                    picked = Some(Pick::Back);
                }
                let c = &creator.character;
                let summary = format!("{} · {} · {}", c.name, c.class.name(), c.difficulty.name());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let start = egui::Button::new(RichText::new("Start").size(20.0).strong()).fill(Color32::from_rgb(70, 110, 60));
                    if ui.add_sized([160.0, 40.0], start).clicked() {
                        picked = Some(Pick::Start);
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new(summary).size(15.0).color(Color32::from_gray(200)));
                });
            });
        });
    });
    if picked == Some(Pick::Start) {
        creator.character.mend();
    }
    picked
}

/// Everything there is to choose, in the order it's chosen
fn choose(ui: &mut egui::Ui, creator: &mut Creator) {
    let gray = Color32::from_gray(185);
    heading(ui, "Name");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut creator.character.name).char_limit(NAME_LENGTH).desired_width(240.0).font(egui::TextStyle::Heading));
        if ui.button(RichText::new("🎲 Random name").size(14.0)).clicked() {
            let seed = creator.roll();
            creator.character.name = super::random_name(seed);
        }
    });

    heading(ui, "Looks");
    let looks = &mut creator.character.looks;
    swatches(ui, "Skin", &SKINS, &mut looks.skin, None);
    choices(ui, "Hair", &HAIRS, &mut looks.hair);
    swatches(ui, "Hair colour", &HAIR_COLORS, &mut looks.hair_color, None);
    choices(ui, "Face", &FACES, &mut looks.face);
    choices(ui, "Build", &BUILDS, &mut looks.build);
    swatches(ui, "Tunic", &CLOTHS, &mut looks.tunic, None);
    swatches(ui, "Sleeves", &CLOTHS, &mut looks.sleeves, None);
    let skin = looks.skin_rgb();
    swatches(ui, "Gloves", &GLOVES, &mut looks.gloves, Some(skin));

    heading(ui, "Starting class");
    ui.columns(2, |columns| {
        for (k, class) in Class::OFFERED.into_iter().enumerate() {
            let ui = &mut columns[k % 2];
            let chosen = creator.character.class == class;
            let frame = egui::Frame::new()
                .fill(if chosen { Color32::from_rgba_unmultiplied(60, 70, 90, 220) } else { Color32::from_rgba_unmultiplied(30, 32, 38, 200) })
                .stroke(Stroke::new(if chosen { 2.0f32 } else { 1.0 }, if chosen { color(class.color()) } else { Color32::from_gray(60) }))
                .corner_radius(8.0)
                .inner_margin(8.0);
            let response = frame
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(class.name()).size(17.0).strong().color(color(class.color())));
                    ui.label(RichText::new(class.about()).size(13.0).color(Color32::from_gray(210)));
                    let kit: Vec<String> = class
                        .kit()
                        .iter()
                        .take(6)
                        .map(|id| creator.families.get(*id).cloned().unwrap_or_else(|| id.to_string()))
                        .collect();
                    let more = class.kit().len().saturating_sub(6);
                    let kit = if more > 0 { format!("{}, and {more} more", kit.join(", ")) } else { kit.join(", ") };
                    ui.label(RichText::new(format!("Starts with: {kit}")).size(12.5).color(gray));
                    let lean: Vec<String> = class.lean().iter().map(|&(stat, value)| stat.describe(value)).collect();
                    ui.label(RichText::new(format!("Leans: {}", lean.join(", "))).size(12.5).color(Color32::from_rgb(160, 210, 255)));
                })
                .response
                .interact(Sense::click());
            if response.clicked() {
                creator.character.class = class;
            }
            ui.add_space(6.0);
        }
    });

    heading(ui, "Difficulty");
    for d in Difficulty::ALL {
        ui.horizontal(|ui| {
            if ui.radio(creator.character.difficulty == d, RichText::new(d.name()).size(16.0).strong()).clicked() {
                creator.character.difficulty = d;
            }
            ui.label(RichText::new(d.about()).size(13.0).color(gray));
        });
    }
    ui.add_space(8.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In a short window, the size of the one played in and shorter, the whole window's on screen,
    /// Back and Start along its foot, what's chosen scrolling within it
    #[test]
    fn it_fits_a_short_window() {
        for [w, h] in [[1548.0, 872.0], [1600.0, 734.0], [1238.0, 698.0], [1280.0, 600.0]] {
            let ctx = egui::Context::default();
            let mut creator = Creator::with(Mode::Realms, Character::default(), 0.5);
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h))), ..Default::default() };
            for _ in 0..3 {
                let _ = ctx.run(input.clone(), |ctx| {
                    window(ctx, &mut creator);
                });
            }
            let shown = ctx.memory(|m| m.area_rect(egui::Id::new("character creation"))).expect("the window's drawn");
            assert!(shown.min.y >= 0.0 && shown.max.y <= h && shown.min.x >= 0.0 && shown.max.x <= w, "at {w}×{h} it's at {shown:?}");
        }
    }
}
