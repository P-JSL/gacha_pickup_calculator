//! Game-specific target identity catalog (UI only). Probability models remain rarity-banner models.
//! Snapshot sources are documented next to catalog_data.tsv; icons load lazily and asynchronously.
use eframe::egui;
use gacha_pickup_calculator::games::Game;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
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
    pub availability: Availability,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability { Released, Announced, Beta, Unverified }
impl Availability {
    pub fn label(self) -> &'static str {
        match self { Self::Released => "출시", Self::Announced => "출시 예정 · 공식", Self::Beta => "베타 확인", Self::Unverified => "미확인" }
    }
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
    for line in include_str!("catalog_data.tsv").lines()
        .chain(include_str!("catalog_future.tsv").lines()) {
        if line.starts_with('#') { continue; }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 && fields.len() != 7 { continue; }
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
            availability: match fields.get(6).copied().unwrap_or("released") {
                "announced" => Availability::Announced,
                "beta" => Availability::Beta,
                "unverified" => Availability::Unverified,
                _ => Availability::Released,
            },
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
    rarity_filter: u8,
    availability_filter: u8,
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
            send_url, recv_image, rarity_filter: 0, availability_filter: 0,
        }
    }

    /// On-disk cache avoids repeatedly downloading the full image gallery after restart.
    /// The key is a stable digest of the verified source URL, never a user-supplied path.
    fn cache_file(url: &str) -> Option<PathBuf> {
        let root = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in url.as_bytes() {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
        Some(root.join("gacha_pickup_calculator").join("catalog_icons")
            .join(format!("{hash:016x}.img")))
    }

    fn decode_icon(bytes: &[u8]) -> Option<egui::ColorImage> {
        let image = image::load_from_memory(bytes).ok()?.thumbnail(112, 112).to_rgba8();
        Some(egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize], image.as_raw(),
        ))
    }

    fn fetch_icon(url: &str) -> Option<egui::ColorImage> {
        if !url.starts_with("https://") { return None }
        let cache = Self::cache_file(url);
        if let Some(file) = &cache {
            if let Ok(bytes) = std::fs::read(file) {
                if bytes.len() <= 3_000_000 {
                    if let Some(image) = Self::decode_icon(&bytes) { return Some(image); }
                }
                let _ = std::fs::remove_file(file);
            }
        }
        let response = ureq::get(url)
            .timeout(Duration::from_secs(6))
            .set("User-Agent", "gacha-pickup-calculator/desktop")
            .call().ok()?;
        let mut bytes = Vec::new();
        response.into_reader().take(3_000_001).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 3_000_000 { return None }
        let decoded = Self::decode_icon(&bytes)?;
        if let Some(file) = cache {
            if let Some(parent) = file.parent() {
                if std::fs::create_dir_all(parent).is_ok() {
                    let _ = std::fs::write(file, &bytes);
                }
            }
        }
        Some(decoded)
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
/// Searchable illustrated gallery for every indexed character or banner equipment.
/// Four-star items appear in the library but cannot be treated as five-star pickup targets.
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
        .default_size(egui::vec2(620.0, 570.0))
        .min_width(430.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("총 {}종", count(game, kind)));
                ui.add(egui::TextEdit::singleline(filter)
                    .hint_text("이름 또는 ID 검색")
                    .desired_width(225.0));
                if ui.button("이미지 다시 불러오기").clicked() { art.retry_failed(); }
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("등급").color(egui::Color32::GRAY).size(11.0));
                for (rarity, title) in [(0, "전체"), (5, "★5"), (4, "★4"), (3, "★3")] {
                    ui.selectable_value(&mut art.rarity_filter, rarity, title);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("출시 상태").color(egui::Color32::GRAY).size(11.0));
                for (value, label) in [(0, "전체"), (1, "출시"), (2, "공식 예고"), (3, "베타"), (4, "미확인")] {
                    ui.selectable_value(&mut art.availability_filter, value, label);
                }
            });
            ui.label(egui::RichText::new(
                "이미지는 최초 열람 시 비동기로 불러오고 로컬에 저장합니다. 출시 전 항목은 공식/베타 상태를 구분하며, ★3/★4는 조회 전용입니다."
            ).color(egui::Color32::from_rgb(166, 194, 209)).size(11.0));
            ui.separator();
            let needle = filter.trim().to_lowercase();
            let items: Vec<&Entry> = entries().iter().filter(|entry| {
                entry.game == game && entry.kind == kind
                    && (art.rarity_filter == 0 || entry.rarity == art.rarity_filter)
                    && (art.availability_filter == 0 || match art.availability_filter {
                        1 => entry.availability == Availability::Released,
                        2 => entry.availability == Availability::Announced,
                        3 => entry.availability == Availability::Beta,
                        4 => entry.availability == Availability::Unverified,
                        _ => true,
                    })
                    && (needle.is_empty() || entry.name.to_lowercase().contains(&needle)
                        || entry.id.to_lowercase().contains(&needle))
            }).collect();
            ui.label(format!("검색 결과 {}종", items.len()));
            let columns = if ui.available_width() < 500.0 { 2 } else { 3 };
            let rows = items.len().div_ceil(columns);
            egui::ScrollArea::vertical().max_height(440.0)
                .show_rows(ui, 79.0, rows, |ui, visible| {
                    for row in visible {
                        ui.columns(columns, |cols| {
                            for column in 0..columns {
                                let Some(item) = items.get(row * columns + column).copied() else {
                                    continue;
                                };
                                cols[column].horizontal(|ui| {
                                    art.image(ui, item, 47.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(
                                            format!("★{} · {}", item.rarity, item.availability.label())
                                        ).color(if item.rarity >= 5 {
                                            egui::Color32::from_rgb(244, 210, 75)
                                        } else {
                                            egui::Color32::from_rgb(166, 194, 209)
                                        }).size(10.0));
                                        let enabled = item.rarity >= 5;
                                        let display = egui::RichText::new(&item.name).size(11.0);
                                        if ui.add_enabled(enabled,
                                            egui::Button::new(display)
                                                .selected(current_id == item.id)
                                        ).on_hover_text(&item.name).clicked() {
                                            selected = Some(item.id.clone());
                                        }
                                    });
                                });
                            }
                        });
                    }
                });
            if items.is_empty() {
                ui.label("일치하는 항목이 없습니다.");
            }
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
            assert!(item.has_verified_image() || item.availability != Availability::Released,
                "released {} {} must have an existing image source", item.id, item.name);
            assert!(unique.insert((game_code(item.game), item.kind as u8, item.id.clone())),
                "duplicate catalog id: {}", item.id);
        }
        assert_eq!(count(Game::Neverness, Kind::Weapon), 51);
        assert!(count(Game::WutheringWaves, Kind::Character) >= 60);
    }
}
