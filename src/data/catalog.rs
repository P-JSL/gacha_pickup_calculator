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
pub(crate) enum Kind { Character, Weapon }

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) game: Game,
    pub(crate) kind: Kind,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) rarity: u8,
    pub(crate) icon_url: String,
    pub(crate) release_order: u32,
    pub(crate) status: &'static str,
}
impl Entry {
    pub(crate) fn has_verified_image(&self) -> bool { approved_icon_url(&self.icon_url) }

    /// Story-granted protagonists do not have a featured gacha banner even when 5★.
    pub(crate) fn is_featured_target(&self) -> bool {
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
    for line in include_str!("../catalog_data.tsv").lines() {
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
pub(crate) fn entries() -> &'static [Entry] {
    static DATA: OnceLock<Vec<Entry>> = OnceLock::new();
    DATA.get_or_init(parse)
}
type CatalogIndex = HashMap<(Game, Kind), HashMap<&'static str, &'static Entry>>;
type SignatureIndex = HashMap<Game, HashMap<&'static str, &'static str>>;

pub(crate) fn by_id(game: Game, kind: Kind, id: &str) -> Option<&'static Entry> {
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
pub(crate) fn first(game: Game, kind: Kind) -> String {
    entries().iter().find(|x| x.game == game && x.kind == kind && x.is_featured_target())
        .map(|e| e.id.clone()).unwrap_or_default()
}
#[cfg(test)]
pub(crate) fn count(game: Game, kind: Kind) -> usize {
    entries().iter().filter(|x| x.game == game && x.kind == kind).count()
}

/// Explicitly sourced banner pairing. Never guess a 5★ weapon from rarity,
/// sorted position, shared element, or similar naming.
pub(crate) fn signature_weapon(game: Game, character_id: &str) -> Option<&'static Entry> {
    static PAIRS: OnceLock<SignatureIndex> = OnceLock::new();
    let pairs = PAIRS.get_or_init(|| {
        let mut pairs: HashMap<Game, HashMap<&str, &str>> = HashMap::new();
        for line in include_str!("../signature_pairs.tsv").lines() {
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

pub(crate) fn publish_verified(bundle: VerifiedBundle) -> Result<(), &'static str> {
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
pub(crate) fn live_entries() -> Arc<Vec<Entry>> {
    static BASE: OnceLock<Arc<Vec<Entry>>> = OnceLock::new();
    if let Ok(Some(snapshot)) = active() { return Arc::clone(&snapshot.entries); }
    Arc::clone(BASE.get_or_init(|| Arc::new(entries().to_vec())))
}
pub(crate) fn live_by_id(game: Game, kind: Kind, id: &str) -> Option<Entry> {
    match active() {
        Ok(Some(snapshot)) => snapshot.ids.get(&(game, kind, id.to_owned()))
            .and_then(|index| snapshot.entries.get(*index)).cloned(),
        Ok(None) => by_id(game, kind, id).cloned(),
        Err(_) => None, // poisoned state: do not use potentially corrupt data
    }
}
pub(crate) fn live_first(game: Game, kind: Kind) -> String {
    live_entries().iter()
        .find(|e| e.game == game && e.kind == kind && e.is_featured_target())
        .map(|e| e.id.clone()).unwrap_or_default()
}
pub(crate) fn live_count(game: Game, kind: Kind) -> usize {
    live_entries().iter().filter(|e| e.game == game && e.kind == kind).count()
}
pub(crate) fn live_signature_weapon(game: Game, character_id: &str) -> Option<Entry> {
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

// Catalog metadata and immutable lookup remain here; image loading and
// picker widgets live in their own UI lifecycle module.
#[path = "catalog/ui.rs"] mod ui;
pub(crate) use ui::{CatalogUi, picker};
#[cfg(test)]
#[path = "catalog/tests.rs"] mod tests;
