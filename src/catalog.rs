//! Game-specific target identity catalog (UI only). Probability models remain rarity-banner models.
//! Snapshot sources are documented next to catalog_data.tsv; icons load lazily and asynchronously.
use eframe::egui;
use crate::security;
use gacha_pickup_calculator::games::Game;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use crate::runtime_updates::VerifiedBundle;
use std::path::PathBuf;

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
// Only these independently checked Gachabase asset URLs are approved.
// Do not generalize to a host-wide allowlist or an arbitrary URL.
const PINNED_GACHABASE_ICONS: &[&str] = &[
    "https://i.gachabase.net/-LuzQkoz_xxThpfhIoggZyPwP5qGH2PNb7JagsfT7JE/q%3A50/f%3Awebp/plain/hsr/assets/2be49dbbe7cfe8f77187dcfea51d9863.png",
    "https://i.gachabase.net/uBAzqOHCvKf2Lw_wZbXzjj80cTLw831sAmvOKy7ROS8/q%3A50/f%3Awebp/plain/hsr/assets/e4d45ac64a652c8e44f9a7c7da2d4e07.png",
    "https://i.gachabase.net/i6VciWWsCxBhjhKd0bBK-E_qf9P-VDTdz-I-8-zUPeY/q%3A50/f%3Awebp/plain/hsr/assets/ff88fd079e093ae22ccf24269eb85a17.png",
];
const MAX_ICON_BYTES: u64 = 512 * 1024;
const MAX_ICON_SIDE: u32 = 256;

pub(crate) fn approved_icon_url(url: &str) -> bool {
    if url.len() > 512 || url.bytes().any(|byte| byte.is_ascii_control()) {
        return false;
    }
    if PINNED_GACHABASE_ICONS.contains(&url) { return true; }
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
        (Game::StarRail, "1511") => (20261111, "4.7 공식 발표"),
        (Game::StarRail, "upcoming_astra_yao") => (20261028, "4.8 공식 공개 · 배너 미확정"),
        (Game::StarRail, "upcoming_ellen_joe") => (20261027, "4.8 공식 공개 · 배너 미확정"),
        (Game::StarRail, "1503") => (20261015, "최신 4.6"),
        (Game::WutheringWaves, "1312") => (20261022, "최신 3.7"),
        (Game::WutheringWaves, "1311") => (20260930, "신규 3.7"),
        (Game::Neverness, "upcoming_elyms") => (20261111, "공개 예정 1.5"),
        (Game::Neverness, "upcoming_exe") => (20261202, "공개 예정 1.5"),
        (Game::StellaSora, "162") => (20261020, "신규 2.0"),
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
        } else if game == Game::StarRail && fields[2] == "23065" {
            (20261111, "4.7 아하 전용")
        } else if game == Game::StarRail && fields[2] == "22009" {
            (20261111, "4.7 무료 광추")
        } else if game == Game::StellaSora && fields[2] == "214066" {
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
type CatalogIndex = HashMap<(Game, Kind), HashMap<&'static str, &'static Entry>>;
type SignatureIndex = HashMap<Game, HashMap<&'static str, &'static str>>;

pub fn by_id(game: Game, kind: Kind, id: &str) -> Option<&'static Entry> {
    // Build the index once. Repeated picker redraws and signature selections
    // should not scan the entire four-game catalog.
    static INDEX: OnceLock<CatalogIndex> = OnceLock::new();
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
#[cfg(test)]
pub fn first(game: Game, kind: Kind) -> String {
    entries().iter().find(|x| x.game == game && x.kind == kind && x.is_featured_target())
        .map(|e| e.id.clone()).unwrap_or_default()
}
#[cfg(test)]
pub fn count(game: Game, kind: Kind) -> usize {
    entries().iter().filter(|x| x.game == game && x.kind == kind).count()
}

/// Explicitly sourced banner pairing. Never guess a 5★ weapon from rarity,
/// sorted position, shared element, or similar naming.
pub fn signature_weapon(game: Game, character_id: &str) -> Option<&'static Entry> {
    static PAIRS: OnceLock<SignatureIndex> = OnceLock::new();
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

/// Runtime updates use a single immutable snapshot, published only after the
/// entire manifest, roster and signature graph have been validated.
struct ActiveCatalog {
    entries: Arc<Vec<Entry>>,
    ids: HashMap<(Game, Kind, String), usize>,
    pairs: HashMap<(Game, String), String>,
    hashes: HashMap<String, [u8; 32]>,
    image_dir: PathBuf,
}
static LIVE: OnceLock<RwLock<Option<Arc<ActiveCatalog>>>> = OnceLock::new();

fn snapshot_from_lock(
    lock: &RwLock<Option<Arc<ActiveCatalog>>>,
) -> Result<Option<Arc<ActiveCatalog>>, &'static str> {
    // Never call PoisonError::into_inner: a panicking writer might have left
    // the active catalog in an inconsistent state.
    Ok(lock.read().map_err(|_| "카탈로그 공유 상태가 poisoned 되었습니다.")?.clone())
}
fn active() -> Result<Option<Arc<ActiveCatalog>>, &'static str> {
    snapshot_from_lock(LIVE.get_or_init(|| RwLock::new(None)))
}

pub fn publish_verified(bundle: VerifiedBundle) -> Result<(), &'static str> {
    let mut ids = HashMap::with_capacity(bundle.entries.len());
    for (index, entry) in bundle.entries.iter().enumerate() {
        ids.insert((entry.game, entry.kind, entry.id.clone()), index);
    }
    let snapshot = Arc::new(ActiveCatalog {
        entries: Arc::new(bundle.entries), ids,
        pairs: bundle.pairs, hashes: bundle.hashes, image_dir: bundle.image_dir,
    });
    let lock = LIVE.get_or_init(|| RwLock::new(None));
    *lock.write().map_err(|_| "Poisoned RwLock: 갱신을 중단했습니다.")? = Some(snapshot);
    Ok(())
}
pub fn live_entries() -> Arc<Vec<Entry>> {
    static BASE: OnceLock<Arc<Vec<Entry>>> = OnceLock::new();
    if let Ok(Some(snapshot)) = active() { return Arc::clone(&snapshot.entries); }
    Arc::clone(BASE.get_or_init(|| Arc::new(entries().to_vec())))
}
pub fn live_by_id(game: Game, kind: Kind, id: &str) -> Option<Entry> {
    match active() {
        Ok(Some(snapshot)) => snapshot.ids.get(&(game, kind, id.to_owned()))
            .and_then(|index| snapshot.entries.get(*index)).cloned(),
        Ok(None) => by_id(game, kind, id).cloned(),
        Err(_) => None, // poisoned state: do not use potentially corrupt data
    }
}
pub fn live_first(game: Game, kind: Kind) -> String {
    live_entries().iter()
        .find(|e| e.game == game && e.kind == kind && e.is_featured_target())
        .map(|e| e.id.clone()).unwrap_or_default()
}
pub fn live_count(game: Game, kind: Kind) -> usize {
    live_entries().iter().filter(|e| e.game == game && e.kind == kind).count()
}
pub fn live_signature_weapon(game: Game, character_id: &str) -> Option<Entry> {
    match active() {
        Ok(Some(snapshot)) => {
            let weapon_id = snapshot.pairs.get(&(game, character_id.to_owned()))?;
            let index = snapshot.ids.get(&(game, Kind::Weapon, weapon_id.clone()))?;
            snapshot.entries.get(*index).cloned().filter(|e| e.rarity >= 5)
        }
        Ok(None) => signature_weapon(game, character_id).cloned(),
        Err(_) => None,
    }
}
pub(crate) fn runtime_digest(key: &str) -> Result<Option<[u8; 32]>, &'static str> {
    Ok(active()?.and_then(|snapshot| snapshot.hashes.get(key).copied()))
}
pub(crate) fn active_image_dir() -> Option<PathBuf> {
    active().ok().flatten().map(|snapshot| snapshot.image_dir.clone())
}

type ImageResult = (u64, String, Option<egui::ColorImage>);
pub struct CatalogUi {
    textures: HashMap<String, Option<egui::TextureHandle>>,
    queued: HashSet<String>,
    send_url: Sender<(u64, String)>,
    generation: u64,
    recv_image: Receiver<ImageResult>,
}

impl CatalogUi {
    pub fn new(ctx: &egui::Context) -> Self {
        let (send_url, recv_url) = mpsc::channel::<(u64, String)>();
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
                    let pixels = Self::fetch_icon(&url.1);
                    if send_image.send((url.0, url.1, pixels)).is_err() { break }
                    repaint.request_repaint();
                }
            });
        }
        Self {
            textures: HashMap::new(), queued: HashSet::new(),
            send_url, recv_image, generation: 0,
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
        let local = executable.parent()?.join("Image");
        let roots = [active_image_dir(), Some(local)];
        for root in roots.into_iter().flatten() {
            let candidate = root.join(game).join(file);
            let Some(decoded) = (|| {
                let metadata = fs::symlink_metadata(&candidate).ok()?;
                if !metadata.file_type().is_file() || metadata.len() > MAX_ICON_BYTES {
                    return None;
                }
                if !candidate.canonicalize().ok()?.starts_with(root.canonicalize().ok()?) {
                    return None;
                }
                // No lock is held during disk I/O or decompression.
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
            })() else { continue };
            return Some(decoded);
        }
        None
    }

    pub fn pump(&mut self, ctx: &egui::Context) {
        while let Ok((generation, url, decoded)) = self.recv_image.try_recv() {
            if generation != self.generation { continue; }
            self.queued.remove(&url);
            let texture = decoded.map(|img| {
                ctx.load_texture(format!("catalog:{}", url), img, egui::TextureOptions::LINEAR)
            });
            self.textures.insert(url, texture);
        }
    }

    /// Drop previous-version textures and suppress late image-worker responses.
    pub fn invalidate_after_update(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.textures.clear();
        self.queued.clear();
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
            if self.send_url.send((self.generation, key.clone())).is_err() {
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
                ui.label(format!("등록 {}개", live_count(game, kind)));
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
            if game == Game::StarRail && kind == Kind::Character {
                ui.label(egui::RichText::new(
                    "4.8 공개 캐릭터는 실제 배너와 전용 광추가 미확정입니다. 현재 5성 픽업 규칙으로 가정 계산합니다."
                ).color(egui::Color32::from_rgb(225, 197, 128)).size(11.0));
            }
            ui.separator();
            let needle = filter.trim().to_lowercase();
            let snapshot = live_entries();
            let items: Vec<&Entry> = snapshot.iter().filter(|entry| {
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
    fn poisoned_catalog_lock_refuses_to_read_unvalidated_state() {
        let isolated = Arc::new(RwLock::new(None));
        let writer = Arc::clone(&isolated);
        assert!(std::thread::spawn(move || {
            let _guard = writer.write().expect("fresh lock");
            panic!("intentional write-lock poisoning");
        }).join().is_err());
        assert!(snapshot_from_lock(&isolated).is_err());
    }
    #[test]
    fn upcoming_and_released_characters_are_prioritized() {
        assert_eq!(first(Game::StarRail, Kind::Character), "1511");
        assert_eq!(first(Game::Neverness, Kind::Character), "upcoming_exe");
        assert_eq!(first(Game::StellaSora, Kind::Character), "162");
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
        assert_eq!(count, 141, "signature mapping coverage changed unexpectedly");
        assert_eq!(signature_weapon(Game::StarRail, "1511").unwrap().id, "23065");
        assert!(signature_weapon(Game::StarRail, "upcoming_ellen_joe").is_none());
        assert!(signature_weapon(Game::StarRail, "upcoming_astra_yao").is_none());
        assert_eq!(by_id(Game::StarRail, Kind::Weapon, "22009").unwrap().rarity, 4);
        assert_eq!(signature_weapon(Game::Neverness, "1056").map(|e| &e.id), None);
        assert!(signature_weapon(Game::StarRail, "8001").is_none());
        // IDs come from the official 2.0 data snapshot, not earlier aliases.
        assert_eq!(signature_weapon(Game::StellaSora, "162").unwrap().id, "214066");
        assert!(by_id(Game::StellaSora, Kind::Character, "firstlight_noya").is_none());
        assert!(by_id(Game::StellaSora, Kind::Weapon, "guiding_star_disc").is_none());
        assert_eq!(signature_weapon(Game::StellaSora, "139").unwrap().id, "214061");
    }

    #[test]
    fn all_unpaired_five_star_targets_are_explicitly_classified() {
        let mut exceptions = HashSet::new();
        for line in include_str!("unpaired_five_star_targets.tsv").lines() {
            if line.starts_with('#') || line.is_empty() { continue; }
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 4, "bad unpaired target: {line}");
            let game = match fields[0] {
                "hsr" => Game::StarRail, "wuwa" => Game::WutheringWaves,
                "nte" => Game::Neverness, "sora" => Game::StellaSora,
                _ => panic!("unknown game in exception: {line}"),
            };
            let ch = by_id(game, Kind::Character, fields[1]).expect("unknown exception identity");
            assert_eq!(ch.name, fields[3]);
            assert_eq!(ch.rarity, 5);
            assert!(signature_weapon(game, fields[1]).is_none(),
                "exception has a mapped signature: {line}");
            assert!(exceptions.insert((game, fields[1])), "duplicate exception: {line}");
        }
        assert_eq!(exceptions.len(), 30, "unmapped target coverage changed");
        for entry in entries().iter().filter(|entry|
            entry.kind == Kind::Character && entry.rarity == 5
        ) {
            if signature_weapon(entry.game, &entry.id).is_none() {
                assert!(exceptions.contains(&(entry.game, entry.id.as_str())),
                    "five-star character missing a signature and an explicit exception: {}",
                    entry.id);
            }
        }
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
