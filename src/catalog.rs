//! Game-specific target identity catalog (UI only). Probability models remain rarity-banner models.
//! Snapshot sources are documented next to catalog_data.tsv; icons load lazily and asynchronously.
use eframe::egui;
use gacha_pickup_calculator::games::Game;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Character, Weapon }

#[derive(Clone, Debug)]
pub struct Entry {
    pub game: Game,
    pub kind: Kind,
    pub id: String,
    pub name: String,
    pub rarity: u8,
    pub icon_url: String,
}
impl Entry {
    pub fn has_verified_image(&self) -> bool { self.icon_url.starts_with("https://") }
}
fn game_code(game: Game) -> &'static str {
    match game {
        Game::StarRail => "hsr",
        Game::WutheringWaves => "wuwa",
        Game::Neverness => "nte",
        Game::StellaSora => "sora",
    }
}
fn parse() -> Vec<Entry> {
    let mut out = Vec::new();
    for line in include_str!("catalog_data.tsv").lines() {
        if line.starts_with('#') { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 { continue; }
        let game = match fields[0] {
            "hsr" => Game::StarRail,
            "wuwa" => Game::WutheringWaves,
            "nte" => Game::Neverness,
            "sora" => Game::StellaSora,
            _ => continue,
        };
        let kind = match fields[1] { "c" => Kind::Character, "w" => Kind::Weapon, _ => continue };
        out.push(Entry {
            game, kind, id: fields[2].to_owned(), name: fields[3].to_owned(),
            rarity: fields[4].parse().unwrap_or(5),
            icon_url: fields[5].to_owned(),
        });
    }
    out.sort_by(|a, b| {
        game_code(a.game).cmp(game_code(b.game))
            .then_with(|| (a.kind == Kind::Weapon).cmp(&(b.kind == Kind::Weapon)))
            .then_with(|| b.rarity.cmp(&a.rarity))
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}
pub fn entries() -> &'static [Entry] {
    static DATA: OnceLock<Vec<Entry>> = OnceLock::new();
    DATA.get_or_init(parse)
}
pub fn by_id(game: Game, kind: Kind, id: &str) -> Option<&'static Entry> {
    entries().iter().find(|x| x.game == game && x.kind == kind && x.id == id)
}
pub fn first(game: Game, kind: Kind) -> String {
    entries().iter().find(|x| x.game == game && x.kind == kind && x.rarity >= 5)
        .map(|e| e.id.clone()).unwrap_or_default()
}
pub fn count(game: Game, kind: Kind) -> usize {
    entries().iter().filter(|x| x.game == game && x.kind == kind).count()
}

type ImageResult = (String, Option<egui::ColorImage>);
pub struct CatalogUi {
    textures: HashMap<String, Option<egui::TextureHandle>>,
    queued: HashSet<String>,
    send_url: Sender<String>,
    recv_image: Receiver<ImageResult>,
}

impl CatalogUi {
    pub fn new(ctx: &egui::Context) -> Self {
        let (send_url, recv_url) = mpsc::channel::<String>();
        let (send_image, recv_image) = mpsc::channel::<ImageResult>();
        // Bounded image I/O: one slow icon must not block the rest of the visible catalog.
        // Each worker locks the queue only while receiving its next URL.
        let incoming = Arc::new(Mutex::new(recv_url));
        for _ in 0..4 {
            let incoming = Arc::clone(&incoming);
            let send_image = send_image.clone();
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                loop {
                    let url = match incoming.lock() {
                        Ok(receiver) => match receiver.recv() {
                            Ok(url) => url,
                            Err(_) => break,
                        },
                        Err(_) => break,
                    };
                    let pixels = Self::fetch_icon(&url);
                    if send_image.send((url, pixels)).is_err() { break }
                    repaint.request_repaint();
                }
            });
        }
        Self {
            textures: HashMap::new(), queued: HashSet::new(),
            send_url, recv_image,
        }
    }

    fn fetch_icon(url: &str) -> Option<egui::ColorImage> {
        if !url.starts_with("https://") { return None }
        let response = ureq::get(url)
            .timeout(Duration::from_secs(6))
            .set("User-Agent", "gacha-pickup-calculator/desktop")
            .call().ok()?;
        let mut bytes = Vec::new();
        response.into_reader().take(3_000_000).read_to_end(&mut bytes).ok()?;
        let image = image::load_from_memory(&bytes).ok()?.thumbnail(112, 112).to_rgba8();
        Some(egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize], image.as_raw(),
        ))
    }

    pub fn pump(&mut self, ctx: &egui::Context) {
        while let Ok((url, decoded)) = self.recv_image.try_recv() {
            self.queued.remove(&url);
            let texture = decoded.map(|img| {
                ctx.load_texture(format!("catalog:{}", url), img, egui::TextureOptions::LINEAR)
            });
            self.textures.insert(url, texture);
        }
    }

    /// Failed icon requests can be retried without clearing successfully cached images.
    pub fn retry_failed(&mut self) {
        self.textures.retain(|_, texture| texture.is_some());
    }

    pub fn image(&mut self, ui: &mut egui::Ui, entry: &Entry, size: f32) {
        if entry.has_verified_image()
            && !self.textures.contains_key(&entry.icon_url)
            && !self.queued.contains(&entry.icon_url)
        {
            self.queued.insert(entry.icon_url.clone());
            if self.send_url.send(entry.icon_url.clone()).is_err() {
                self.queued.remove(&entry.icon_url);
            }
        }
        if let Some(Some(tex)) = self.textures.get(&entry.icon_url) {
            ui.image((tex.id(), egui::vec2(size, size)));
        } else {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            ui.painter().rect_filled(rect, 7.0,
                egui::Color32::from_rgb(36, 71, 104));
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER,
                if entry.kind == Kind::Character { "★" } else { "◆" },
                egui::FontId::proportional(size * 0.42),
                egui::Color32::from_rgb(111, 226, 241));
        }
    }
}

/// Select from the entire catalog. Items below five stars are displayed but not
/// selectable in the current featured-five-star probability model.
pub fn picker(
    ctx: &egui::Context, art: &mut CatalogUi, game: Game, kind: Kind,
    open: &mut bool, filter: &mut String, current_id: &str,
) -> Option<String> {
    let mut selected = None;
    let title = match kind {
        Kind::Character => "목표 캐릭터 선택",
        Kind::Weapon => match game {
            Game::StarRail => "목표 광추 선택",
            Game::WutheringWaves => "목표 무기 선택",
            Game::Neverness => "목표 아크 선택",
            Game::StellaSora => "목표 레코드 선택",
        },
    };
    egui::Window::new(title).open(open)
        .default_size(egui::vec2(510.0, 510.0))
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("등록 {}개", count(game, kind)));
                ui.add(egui::TextEdit::singleline(filter)
                    .hint_text("캐릭터/광추/무기/아크/레코드 검색")
                    .desired_width(245.0));
                if ui.button("이미지 재시도").clicked() {
                    art.retry_failed();
                }
            });
            ui.label(egui::RichText::new(
                "★5 목표 기준 확률입니다. ★4 목록은 표시되지만 확률 계산 대상은 아닙니다."
            ).color(egui::Color32::from_rgb(165, 190, 208)).size(11.0));
            ui.separator();
            let needle = filter.trim().to_lowercase();
            let items: Vec<&Entry> = entries().iter().filter(|entry| {
                entry.game == game && entry.kind == kind
                    && (needle.is_empty() || entry.name.to_lowercase().contains(&needle)
                        || entry.id.contains(&needle))
            }).collect();
            egui::ScrollArea::vertical().max_height(440.0)
                .show_rows(ui, 43.0, items.len(), |ui, range| {
                    for index in range {
                        let item = items[index];
                        ui.horizontal(|ui| {
                            art.image(ui, item, 31.0);
                            let display = format!("{}   ★{}", item.name, item.rarity);
                            let enabled = item.rarity >= 5;
                            if ui.add_enabled(enabled, egui::Button::new(display)
                                .selected(current_id == item.id)).clicked() {
                                selected = Some(item.id.clone());
                            }
                            if !item.has_verified_image() {
                                ui.label(egui::RichText::new("이미지 미확보")
                                    .color(egui::Color32::GRAY).size(9.0));
                            }
                        });
                    }
                });
        });
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_games_have_both_named_catalogs() {
        let mut unique = HashSet::new();
        for game in Game::ALL {
            assert!(count(game, Kind::Character) > 0);
            assert!(count(game, Kind::Weapon) > 0);
            let first = first(game, Kind::Character);
            assert!(by_id(game, Kind::Character, &first).is_some());
        }
        for item in entries() {
            assert!(!item.name.trim().is_empty());
            assert!(item.has_verified_image(),
                "{} {} must have an existing image source", item.id, item.name);
            assert!(unique.insert((game_code(item.game), item.kind as u8, item.id.clone())),
                "duplicate catalog id: {}", item.id);
        }
        assert_eq!(count(Game::Neverness, Kind::Weapon), 51);
        assert!(count(Game::WutheringWaves, Kind::Character) >= 60);
    }
}
