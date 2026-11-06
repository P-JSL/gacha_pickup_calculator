//! Game-specific target identity catalog (UI only). Probability models remain rarity-banner models.
//! Snapshot sources are documented next to catalog_data.tsv; icons load lazily and asynchronously.
use eframe::egui;
use crate::security;
use gacha_pickup_calculator::games::Game;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind { Character, Weapon }

#[derive(Clone, Debug)]
pub struct Entry {
    pub game: Game,
    pub kind: Kind,
    pub id: String,
    pub name: String,
    pub rarity: u8,
    pub icon_url: String,
    pub release_order: u32,
    pub status: &'static str,
}
impl Entry {
    pub fn has_verified_image(&self) -> bool { approved_icon_url(&self.icon_url) }

    /// Story-granted protagonists do not have a featured gacha banner even when 5★.
    pub fn is_featured_target(&self) -> bool {
        self.rarity >= 5 && !matches!(
            self.game,
            Game::StarRail if self.id.starts_with("800")
                || self.id == "8010"
        ) && !(self.game == Game::WutheringWaves && self.name.starts_with("Rover:"))
    }
}

// Image URLs are compiled into the catalog, but a compromised upstream asset or
// accidental future catalog change must not turn the app into a general URL client.
// A fixed repository allowlist blocks arbitrary hosts, credentials, ports, queries,
// fragments, protocol downgrades and redirects to internal network targets.
const ICON_REPOSITORIES: &[&str] = &[
    "https://raw.githubusercontent.com/Mar-7th/StarRailRes/",
    "https://raw.githubusercontent.com/ryanbenson/wuthering-waves-assets/",
    "https://raw.githubusercontent.com/JforPlay/sstoy/",
    "https://raw.githubusercontent.com/Waifus-Grace/NTE_Assets/",
];
const MAX_ICON_BYTES: u64 = 512 * 1024;
const MAX_ICON_SIDE: u32 = 256;

fn approved_icon_url(url: &str) -> bool {
    if url.len() > 512 || url.bytes().any(|byte| byte.is_ascii_control()) {
        return false;
    }
    let Some(relative) = ICON_REPOSITORIES.iter()
        .find_map(|prefix| url.strip_prefix(prefix)) else { return false };
    let extension_ok = [".png", ".jpg", ".jpeg"]
        .iter().any(|extension| relative.to_ascii_lowercase().ends_with(extension));
    extension_ok && relative.split('/').all(|part|
        !part.is_empty() && part != "." && part != ".."
        && part.bytes().all(|byte|
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'(' | b')')
        )
    )
}

fn approved_icon_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && (bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || (bytes.len() >= 3 && bytes.starts_with(b"\xff\xd8\xff")))
}

fn game_code(game: Game) -> &'static str {
    match game {
        Game::StarRail => "hsr",
        Game::WutheringWaves => "wuwa",
        Game::Neverness => "nte",
        Game::StellaSora => "sora",
    }
}
/// Only source-verified recent releases/announcements are prioritized.
/// Other roster entries remain available below these; unverified leaks are excluded.
fn recent_character(game: Game, id: &str) -> (u32, &'static str) {
    match (game, id) {
        (Game::StarRail, "upcoming_aeon_aha") => (20261111, "4.7 공식 발표"),
        (Game::StarRail, "1503") => (20261015, "최신 4.6"),
        (Game::WutheringWaves, "1312") => (20261022, "최신 3.7"),
        (Game::WutheringWaves, "1311") => (20260930, "신규 3.7"),
        (Game::Neverness, "upcoming_elyms") => (20261111, "공개 예정 1.5"),
        (Game::Neverness, "upcoming_exe") => (20261202, "공개 예정 1.5"),
        (Game::StellaSora, "firstlight_noya") => (20261020, "신규 2.0"),
        _ => (0, ""),
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
        let (release_order, status) = if kind == Kind::Character {
            recent_character(game, fields[2])
        } else if game == Game::StellaSora && fields[2] == "guiding_star_disc" {
            (20261020, "신규 레코드 2.0")
        } else { (0, "") };
        out.push(Entry {
            game, kind, id: fields[2].to_owned(), name: fields[3].to_owned(),
            rarity: fields[4].parse().unwrap_or(5),
            icon_url: fields[5].to_owned(),
            release_order, status,
        });
    }
    out.sort_by(|a, b| {
        game_code(a.game).cmp(game_code(b.game))
            .then_with(|| (a.kind == Kind::Weapon).cmp(&(b.kind == Kind::Weapon)))
            .then_with(|| b.release_order.cmp(&a.release_order))
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
    // Build the index once. Repeated picker redraws and signature selections
    // should not scan the entire four-game catalog.
    static INDEX: OnceLock<HashMap<(Game, Kind), HashMap<&'static str, &'static Entry>>> =
        OnceLock::new();
    let lookup = INDEX.get_or_init(|| {
        let mut lookup: HashMap<(Game, Kind), HashMap<&str, &Entry>> = HashMap::new();
        for entry in entries() {
            lookup.entry((entry.game, entry.kind))
                .or_default().insert(entry.id.as_str(), entry);
        }
        lookup
    });
    lookup.get(&(game, kind))?.get(id).copied()
}
pub fn first(game: Game, kind: Kind) -> String {
    entries().iter().find(|x| x.game == game && x.kind == kind && x.is_featured_target())
        .map(|e| e.id.clone()).unwrap_or_default()
}
pub fn count(game: Game, kind: Kind) -> usize {
    entries().iter().filter(|x| x.game == game && x.kind == kind).count()
}

/// Explicitly sourced banner pairing. Never guess a 5★ weapon from rarity,
/// sorted position, shared element, or similar naming.
pub fn signature_weapon(game: Game, character_id: &str) -> Option<&'static Entry> {
    static PAIRS: OnceLock<HashMap<Game, HashMap<&'static str, &'static str>>> =
        OnceLock::new();
    let pairs = PAIRS.get_or_init(|| {
        let mut pairs: HashMap<Game, HashMap<&str, &str>> = HashMap::new();
        for line in include_str!("signature_pairs.tsv").lines() {
            if line.starts_with('#') || line.is_empty() { continue; }
            let mut parts = line.split('\t');
            let (Some(g), Some(char_id), Some(weapon_id), Some(source)) =
                (parts.next(), parts.next(), parts.next(), parts.next()) else { continue; };
            if parts.next().is_some() || source.is_empty() { continue; }
            let game = Game::ALL.iter().copied()
                .find(|game| game_code(*game) == g);
            if let Some(game) = game {
                pairs.entry(game).or_default().insert(char_id, weapon_id);
            }
        }
        pairs
    });
    let weapon_id = *pairs.get(&game)?.get(character_id)?;
    by_id(game, Kind::Weapon, weapon_id).filter(|entry| entry.rarity >= 5)
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

    /// Only verified local, bounded PNGs are loaded. Never perform an
    /// HTTP request or parse scripts from the image catalog at runtime.
    fn fetch_icon(relative: &str) -> Option<egui::ColorImage> {
        let mut parts = relative.split('/');
        let game = parts.next()?;
        let file = parts.next()?;
        if parts.next().is_some()
            || !["HSR", "WutheringWaves", "StellaSora", "Neverness"].contains(&game)
            || !file.ends_with(".png")
            || !file.trim_end_matches(".png").chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        { return None; }

        let executable = std::env::current_exe().ok()?;
        let root = executable.parent()?.join("Image");
        let candidate = root.join(game).join(file);
        let metadata = fs::symlink_metadata(&candidate).ok()?;
        if !metadata.file_type().is_file() || metadata.len() > MAX_ICON_BYTES {
            return None;
        }
        let canonical = candidate.canonicalize().ok()?;
        if !canonical.starts_with(root.canonicalize().ok()?) { return None; }
        // Read the already-checked path through a single bounded file handle.
        // An attacker replacing the source between checks cannot supply a
        // different portrait: its content must match the embedded SHA-256.
        let handle = fs::File::open(&candidate).ok()?;
        if !handle.metadata().ok()?.is_file() { return None; }
        let mut bytes = Vec::new();
        handle.take(MAX_ICON_BYTES + 1).read_to_end(&mut bytes).ok()?;
        if !approved_icon_bytes(&bytes)
            || security::verify_image_bytes(relative, &bytes).is_err()
        { return None; }
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format().ok()?;
        if reader.format() != Some(image::ImageFormat::Png) { return None; }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_ICON_SIDE);
        limits.max_image_height = Some(MAX_ICON_SIDE);
        limits.max_alloc = Some(8 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().ok()?.thumbnail(112, 112).to_rgba8();
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
        let game = match entry.game {
            Game::StarRail => "HSR",
            Game::WutheringWaves => "WutheringWaves",
            Game::Neverness => "Neverness",
            Game::StellaSora => "StellaSora",
        };
        let role = if entry.kind == Kind::Character { "c" } else { "w" };
        let key = format!("{game}/{role}-{}.png", entry.id);
        if !self.textures.contains_key(&key) && !self.queued.contains(&key) {
            self.queued.insert(key.clone());
            if self.send_url.send(key.clone()).is_err() {
                self.queued.remove(&key);
            }
        }
        if let Some(Some(tex)) = self.textures.get(&key) {
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
                            let enabled = item.is_featured_target();
                            if ui.add_enabled(enabled, egui::Button::new(display)
                                .selected(current_id == item.id)).clicked() {
                                selected = Some(item.id.clone());
                            }
                            if !item.status.is_empty() {
                                ui.label(egui::RichText::new(item.status)
                                    .color(egui::Color32::LIGHT_GREEN).size(10.0));
                            }
                            if !enabled {
                                ui.label(egui::RichText::new("픽업 배너 없음")
                                    .color(egui::Color32::GRAY).size(9.0));
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
    fn upcoming_and_released_characters_are_prioritized() {
        assert_eq!(first(Game::StarRail, Kind::Character), "upcoming_aeon_aha");
        assert_eq!(first(Game::Neverness, Kind::Character), "upcoming_exe");
        assert_eq!(first(Game::StellaSora, Kind::Character), "firstlight_noya");
        assert_eq!(first(Game::WutheringWaves, Kind::Character), "1312");
    }
    #[test]
    fn image_download_policy_rejects_untrusted_servers_and_payloads() {
        assert!(approved_icon_url("https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.png"));
        for url in [
            "http://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.png",
            "https://raw.githubusercontent.com.evil.invalid/a.png",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/../private.png",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.png?token=1",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.svg",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/%2e%2e/icon.png",
            "https://127.0.0.1/image.png",
        ] { assert!(!approved_icon_url(url), "{url}"); }
        assert!(!approved_icon_bytes(b"<svg xmlns='http://www.w3.org/2000/svg'/>"));
        assert!(!approved_icon_bytes(b"MZ executable"));
        assert!(approved_icon_bytes(b"\x89PNG\r\n\x1a\nnot-a-valid-image"));
        assert!(CatalogUi::fetch_icon("HSR/../../evil.png").is_none());
        assert!(CatalogUi::fetch_icon("HSR/asset.svg").is_none());
    }
    #[test]
    fn signature_pairs_resolve_only_to_real_five_star_catalog_entries() {
        let mut pairs = HashSet::new();
        let mut count = 0;
        for line in include_str!("signature_pairs.tsv").lines() {
            if line.starts_with('#') || line.trim().is_empty() { continue; }
            let parts: Vec<_> = line.split('\t').collect();
            assert_eq!(parts.len(), 4, "invalid signature mapping: {line}");
            assert!(parts[3].starts_with("https://"), "unverified source: {line}");
            let game = match parts[0] {
                "hsr" => Game::StarRail, "wuwa" => Game::WutheringWaves,
                "nte" => Game::Neverness, "sora" => Game::StellaSora,
                _ => panic!("unsupported game: {line}"),
            };
            let character = by_id(game, Kind::Character, parts[1])
                .unwrap_or_else(|| panic!("unknown character: {line}"));
            assert!(character.is_featured_target(), "non-banner character: {line}");
            let weapon = by_id(game, Kind::Weapon, parts[2])
                .unwrap_or_else(|| panic!("unknown weapon: {line}"));
            assert_eq!(weapon.rarity, 5, "non-5-star weapon: {line}");
            assert!(pairs.insert((parts[0], parts[1])), "duplicated mapping: {line}");
            assert_eq!(signature_weapon(game, parts[1]).unwrap().id, parts[2]);
            count += 1;
        }
        assert!(count >= 100, "verified signature coverage unexpectedly dropped: {count}");
        assert_eq!(signature_weapon(Game::Neverness, "1056").map(|e| &e.id), None);
        assert!(signature_weapon(Game::StarRail, "8001").is_none());
    }

    #[test]
    fn portraits_and_names_do_not_cross_link_between_character_identities() {
        let mut names = HashSet::new();
        let mut source_urls = HashSet::new();
        for e in entries() {
            assert!(names.insert((game_code(e.game), e.kind as u8, e.name.as_str())),
                "duplicate visible name for different IDs: {} {}", game_code(e.game), e.name);
            if !e.icon_url.is_empty() {
                assert!(source_urls.insert((game_code(e.game), e.kind as u8, e.icon_url.as_str())),
                    "same upstream portrait assigned to multiple identities: {}", e.name);
            }
            assert!(!e.name.trim().is_empty());
            assert!(!e.name.starts_with('{') && !e.name.chars().all(|ch| ch.is_ascii_digit()),
                "placeholder name leaked into catalog: {}", e.name);
        }
        assert_eq!(by_id(Game::Neverness, Kind::Character, "1091").map(|e| e.name.as_str()), None);
        assert_eq!(by_id(Game::Neverness, Kind::Character, "1051").map(|e| e.name.as_str()), None);
        let latest_female = by_id(Game::StarRail, Kind::Character, "8010").unwrap();
        assert_eq!(latest_female.name, "개척자 (여성 · 환락 · 번개)");
        assert!(!latest_female.is_featured_target());
    }

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
            if !item.icon_url.is_empty() {
                assert!(item.has_verified_image(),
                    "{} {} must use an approved image source", item.id, item.name);
            }
            assert!(unique.insert((game_code(item.game), item.kind as u8, item.id.clone())),
                "duplicate catalog id: {}", item.id);
        }
        assert_eq!(count(Game::Neverness, Kind::Weapon), 51);
        assert_eq!(count(Game::Neverness, Kind::Character), 26);
        assert!(count(Game::WutheringWaves, Kind::Character) >= 60);
    }
}
