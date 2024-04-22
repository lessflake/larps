//! egui overlay -- graphical representation of [`crate::meter::Data`].

use std::{
    borrow::Cow,
    fs,
    sync::{mpsc::Sender, Arc},
};

use egui::PointerButton;
use parking_lot::Mutex;

use crate::meter::{Data, Player};

const CLASS_ICON_PATH: &str = "resources/class.png";
const FONT_PATH: &str = "resources/font.ttf";
const FONT_SIZE: f32 = 12.0;

/// Spawn an overlay window displaying `data`.
pub fn run(
    ctx_oneshot_tx: Sender<egui::Context>,
    data: Arc<Mutex<Data>>,
    bar_count: usize,
) -> anyhow::Result<()> {
    win32_overlay::run(move |ctx| {
        let _ = ctx_oneshot_tx.send(ctx.clone());

        let icons = load_class_icons(ctx);
        setup_font(ctx);

        let mut style = (*ctx.style()).clone();
        style.interaction.show_tooltips_only_when_still = false;
        style.visuals.window_rounding = egui::Rounding::ZERO;
        style.visuals.menu_rounding = egui::Rounding::ZERO;
        style.visuals.window_shadow.extrusion = 0.0;
        style.visuals.popup_shadow.extrusion = 0.0;
        for (_, id) in style.text_styles.iter_mut() {
            id.size = FONT_SIZE;
        }
        ctx.set_style(style);

        Ui {
            data,
            state: State::Dps(EncounterChoice::Current),
            icons,
            dragging: false,
            count: bar_count,
        }
    })
}

enum State {
    Dps(EncounterChoice),
    Breakdown(usize, u64, EncounterChoice),
    EncounterList,
}

#[derive(Copy, Clone)]
enum EncounterChoice {
    Current,
    Previous(usize),
}

struct Ui {
    state: State,
    icons: egui::TextureHandle,
    data: Arc<Mutex<Data>>,
    dragging: bool,
    count: usize,
}

impl win32_overlay::App for Ui {
    fn update(&mut self, ctx: &egui::Context) {
        if let Some(pressed) = ctx.input(|i| {
            i.raw.events.iter().rev().find_map(|ev| match ev {
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    modifiers: egui::Modifiers { ctrl: true, .. },
                    pressed,
                    ..
                } => Some(*pressed),
                _ => None,
            })
        }) {
            self.dragging = pressed;
        }

        egui::Window::new("overlay")
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .movable(self.dragging)
            .auto_sized()
            .default_width(300.0)
            .show(ctx, |ui| {
                self.render(ctx, ui);
            });
    }
}

impl Ui {
    fn class_icon_for(&self, player: &Player, size: f32) -> Option<egui::Image> {
        let idx = player.class.icon_index()?;
        let uv_x = (idx % 16) as f32 / 16.0;
        let uv_y = (idx / 16) as f32 / 3.0;
        let uv_rect =
            egui::Rect::from_min_size(egui::pos2(uv_x, uv_y), egui::vec2(1.0 / 16.0, 1.0 / 3.0));

        let tex = egui::load::SizedTexture::new(&self.icons, [size, size]);
        Some(egui::Image::new(tex).uv(uv_rect))
    }

    fn dps_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, choice: EncounterChoice) {
        if ctx.input(|i| i.pointer.button_released(PointerButton::Secondary)) {
            self.state = State::EncounterList;
            ctx.request_repaint();
        }

        let text_color = egui::Color32::WHITE;
        let secondary_text_color = egui::Color32::from_gray(0xcc);

        let data = self.data.lock();
        let mut encounters = data.recent_encounters();
        let encounter = match choice {
            EncounterChoice::Current => encounters.next(),
            EncounterChoice::Previous(idx) => encounters.find(|&(i, _)| i == idx),
        };

        let Some((idx, encounter)) = encounter else {
            ui.set_min_width(ui.available_width());
            ui.label("No data.");
            return;
        };

        let duration = encounter.duration().as_secs_f64();

        if let Some(boss_info) = &data
            .live
            .recently_tracked
            .and_then(|id| data.live.tracked.get(&id))
        {
            let cur_hp = 0.max(boss_info.cur_hp);
            let percentage = cur_hp as f32 / boss_info.max_hp as f32;
            let (bar, _) = Bar::new(ui, percentage, egui::Sense::hover(), (145, 18, 1));
            let text_color = egui::Color32::WHITE;
            if let Some(max_bars) = boss_info.bar_count {
                let bar_count = percentage * max_bars as f32;
                let bar_count_text = format!("{:.1}x", bar_count);
                bar.paint_text_at(&bar_count_text, BarTextPosition::Left(0.0), text_color);
            }

            let hp = format!(
                "{}/{}",
                to_human_readable(cur_hp as f64),
                to_human_readable(boss_info.max_hp as f64)
            );
            bar.paint_text_at(&hp, BarTextPosition::Center, text_color);

            let percent_text = format!("{:.1}%", percentage * 100.0);
            bar.paint_text_at(&percent_text, BarTextPosition::Right, text_color);
        }

        let mut sorted: Vec<_> = encounter.players.iter().collect();
        sorted.sort_by_key(|(_, p)| -p.dmg_dealt);
        let highest_dmg = sorted.first().unwrap().1.dmg_dealt;

        let env = &data.environments[encounter.environment];

        for (id, player, player_info) in sorted
            .iter()
            .filter_map(|(&id, p)| env.players.get(&id).map(|i| (id, p, i)))
            .take(self.count)
        {
            let percentage = player.dmg_dealt as f32 / highest_dmg as f32;
            let color = player_info.class.color();
            let (mut bar, resp) = Bar::new(ui, percentage, egui::Sense::click(), color);

            if let Some(icon) = self.class_icon_for(player_info, bar.size.y) {
                bar.paint_icon(icon);
            }

            let name_text = make_player_name(&player_info, text_color, secondary_text_color);
            bar.paint_text_job_at(name_text, BarTextPosition::Left(1.3), text_color);

            let dps_text = to_human_readable(player.dmg_dealt as f64 / duration);

            let brand_text = if player.brand_dmg > 0 {
                format!(
                    "{}%",
                    (player.brand_dmg as f64 / player.dmg_dealt as f64 * 100.0).round()
                )
            } else {
                "".to_string()
            };

            let ap_text = if player.ap_dmg > 0 {
                format!(
                    "{}%",
                    (player.ap_dmg as f64 / player.dmg_dealt as f64 * 100.0).round()
                )
            } else {
                "".to_string()
            };

            let ident_text = if player.ident_dmg > 0 {
                format!(
                    "{}%",
                    (player.ident_dmg as f64 / player.dmg_dealt as f64 * 100.0).round()
                )
            } else {
                "".to_string()
            };

            let text = format!(
                "{:>4} {:>4} {:>4}  {:>5}",
                ident_text, ap_text, brand_text, dps_text
            );
            bar.paint_text_at(&text, BarTextPosition::Right, text_color);

            if resp.clicked() {
                println!("clicked {}", player_info.class);
                self.state = State::Breakdown(idx, id, choice);
                ctx.request_repaint();
            }
        }
    }

    fn breakdown_view(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        idx: usize,
        id: u64,
        prev_state: EncounterChoice,
    ) {
        if ctx.input(|i| i.pointer.button_released(PointerButton::Secondary)) {
            self.state = State::Dps(prev_state);
            ctx.request_repaint();
        }

        let data = self.data.lock();
        let encounter = &data.encounters[idx];
        let player = encounter.players.get(&id).unwrap();
        let player_info = data.environments[encounter.environment]
            .players
            .get(&id)
            .unwrap();

        if player.skills.is_empty() {
            ui.set_min_width(ui.available_width());
            ui.label("No data.");
            return;
        }

        let mut sorted: Vec<_> = player.skills.iter().collect();
        sorted.sort_by_key(|(_, s)| -s.damage);
        let highest_dmg = sorted[0].1.damage;

        for (id, skill) in sorted.iter().take(8) {
            let percentage = skill.damage as f32 / highest_dmg as f32;
            let color = player_info.class.color();
            let (bar, resp) = Bar::new(ui, percentage, egui::Sense::hover(), color);

            let name = match skill.name.as_ref() {
                Some(name) => Cow::Borrowed(name),
                None => Cow::Owned(id.to_string()),
            };

            let damage = HumanReadable(skill.damage as f64);

            resp.on_hover_ui_at_pointer(|ui| {
                ui.label(&*name);
                ui.monospace(format!("hits  {}", skill.count));
                ui.monospace(format!(
                    "crits {} ({}%)",
                    skill.crits,
                    HumanReadable(skill.crits as f64 / skill.count as f64 * 100.0)
                ));
                if skill.back > 0 {
                    ui.monospace(format!(
                        "back  {} ({}%)",
                        skill.back,
                        HumanReadable(skill.back as f64 / skill.count as f64 * 100.0)
                    ));
                }
                if skill.front > 0 {
                    ui.monospace(format!(
                        "front {} ({}%)",
                        skill.front,
                        HumanReadable(skill.front as f64 / skill.count as f64 * 100.0)
                    ));
                }
                if skill.brand > 0 {
                    ui.monospace(format!(
                        "brand {} ({}%)",
                        skill.brand,
                        HumanReadable(skill.brand as f64 / skill.count as f64 * 100.0)
                    ));
                }
                if skill.ap_buff > 0 {
                    ui.monospace(format!(
                        "ap    {} ({}%)",
                        skill.ap_buff,
                        HumanReadable(skill.ap_buff as f64 / skill.count as f64 * 100.0)
                    ));
                }
                if skill.ident_buff > 0 {
                    ui.monospace(format!(
                        "ident    {} ({}%)",
                        skill.ident_buff,
                        HumanReadable(skill.ident_buff as f64 / skill.count as f64 * 100.0)
                    ));
                }
            });

            bar.paint_text_at(&name, BarTextPosition::Left(0.3), egui::Color32::WHITE);
            let percent = skill.damage as f64 / player.dmg_dealt as f64 * 100.0;

            let text = format!("{} ({}%)", damage, HumanReadable(percent),);

            bar.paint_text_at(&text, BarTextPosition::Right, egui::Color32::WHITE);
        }
    }

    fn encounter_view(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.set_min_width(ui.available_width());
        if ui.button("Current").clicked() {
            self.state = State::Dps(EncounterChoice::Current);
            ctx.request_repaint();
        }
        let data = self.data.lock();
        let encounters = data.recent_encounters();
        for (i, _) in encounters.take(7) {
            if ui.button(i.to_string()).clicked() {
                self.state = State::Dps(EncounterChoice::Previous(i));
                ctx.request_repaint();
            }
        }
    }

    fn render(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        match self.state {
            State::Dps(choice) => self.dps_view(ctx, ui, choice),
            State::Breakdown(idx, id, prev) => self.breakdown_view(ctx, ui, idx, id, prev),
            State::EncounterList => self.encounter_view(ctx, ui),
        }
    }
}

fn slice_at_nth_char(s: &str, idx: usize) -> &str {
    let idx = s
        .char_indices()
        .skip(1)
        .nth(idx)
        .map(|(i, _)| i)
        .unwrap_or_else(|| s.len());
    &s[0..idx]
}

fn make_player_name(
    player: &Player,
    color: egui::Color32,
    offcolor: egui::Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();

    let format = egui::TextFormat {
        color,
        font_id: egui::FontId::monospace(FONT_SIZE),
        ..Default::default()
    };

    let name = player.name.as_deref().unwrap_or("?");
    job.append(slice_at_nth_char(name, 8), 0.0, format);

    let format = egui::TextFormat {
        color: offcolor,
        font_id: egui::FontId::monospace(FONT_SIZE),
        ..Default::default()
    };
    let ilvl_text = format!(" {}", player.ilvl as u32);
    if player.ilvl as u32 > 0 {
        job.append(&ilvl_text, 0.0, format);
    }

    job
}

enum BarTextPosition {
    Left(f32),
    Center,
    Right,
}

/// Percentage bar
struct Bar<'a> {
    ui: &'a mut egui::Ui,
    size: egui::Vec2,
    clip_rect: egui::Rect,
    outer: egui::Rect,
}

impl<'a> Bar<'a> {
    fn new(
        ui: &'a mut egui::Ui,
        percentage: f32,
        sense: egui::Sense,
        (r, g, b): (u8, u8, u8),
    ) -> (Self, egui::Response) {
        let height = ui.text_style_height(&egui::TextStyle::Monospace);
        let size = egui::Vec2 {
            x: ui.available_width(),
            y: height,
        };

        let (rect, response) = ui.allocate_exact_size(size, sense);
        let outer_bar = egui::Rect::from_min_size(rect.min, egui::vec2(size.x, height));
        let dps_bar = egui::Rect::from_min_size(rect.min, egui::vec2(size.x * percentage, height));

        ui.painter().rect_filled(
            dps_bar,
            egui::Rounding::ZERO,
            egui::Color32::from_rgb(r, g, b).linear_multiply(0.3),
        );

        let bar = Self {
            ui,
            size,
            clip_rect: rect,
            outer: outer_bar,
        };

        (bar, response)
    }

    fn paint_icon(&mut self, image: egui::Image) {
        let container = egui::vec2(self.size.y, self.size.y);
        image.paint_at(
            self.ui,
            egui::Rect::from_min_size(self.outer.min, container),
        );
    }

    fn pos_for(&self, pos: BarTextPosition, text_width: f32) -> egui::Pos2 {
        match pos {
            BarTextPosition::Left(x) => self.outer.left_top() + egui::vec2(self.size.y * x, 0.0),
            BarTextPosition::Center => self.outer.center_top() - egui::vec2(0.5 * text_width, 0.0),
            BarTextPosition::Right => self.outer.right_top() - egui::vec2(text_width, 0.0),
        }
    }

    fn paint_text_at(&self, text: &str, pos: BarTextPosition, color: egui::Color32) {
        let text = egui::WidgetText::from(text);
        let galley = text.into_galley(
            self.ui,
            Some(false),
            f32::INFINITY,
            egui::TextStyle::Monospace,
        );
        let text_width = galley.size().x;
        self.ui.painter().with_clip_rect(self.clip_rect).galley(
            self.pos_for(pos, text_width),
            galley,
            color,
        );
    }

    fn paint_text_job_at(
        &self,
        job: egui::text::LayoutJob,
        pos: BarTextPosition,
        color: egui::Color32,
    ) {
        let galley = self.ui.ctx().fonts(|f| f.layout_job(job));
        self.ui
            .painter()
            .galley(self.pos_for(pos, galley.size().x), galley, color);
    }
}

fn load_class_icons(ctx: &egui::Context) -> egui::TextureHandle {
    let decoder =
        png::Decoder::new(fs::File::open(CLASS_ICON_PATH).expect("class icon resource is missing"));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let bytes = &buf[..info.buffer_size()];

    ctx.load_texture(
        "class_icons",
        egui::ColorImage::from_rgba_unmultiplied(
            [info.width as usize, info.height as usize],
            bytes,
        ),
        egui::TextureOptions::LINEAR,
    )
}

fn setup_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let font = fs::read(FONT_PATH).expect("font resource is missing");
    let font_name = "JetBrains Mono";
    fonts
        .font_data
        .insert(font_name.to_owned(), egui::FontData::from_owned(font));
    for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, font_name.to_owned());
    }
    ctx.set_fonts(fonts);
}

struct HumanReadable(f64);

impl std::fmt::Display for HumanReadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.0.is_finite() {
            return f.write_str("???");
        }
        let mut n = self.0;
        let mut suffix = 0;
        while n > 1000.0 {
            n /= 1000.0;
            suffix += 1;
        }
        let suffix = match suffix {
            0 => "",
            1 => "K",
            2 => "M",
            3 => "B",
            4 => "T",
            _ => return f.write_str("big"),
        };

        match n {
            n if n < 9.5 => write!(f, "{:.2}", n)?,
            n if n < 99.5 => write!(f, "{:.1}", n)?,
            n if n < 1000.0 => write!(f, "{:.0}", n)?,
            _ => {
                // println!("??? is {} digits and num is {}", digits, self.0);
                return f.write_str("???");
            }
        }
        f.write_str(suffix)
    }
}

fn to_human_readable(x: f64) -> String {
    HumanReadable(x).to_string()
}
