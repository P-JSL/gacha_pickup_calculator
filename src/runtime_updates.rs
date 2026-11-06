//! Opt-in-safe background update transport. Network responses are untrusted until
//! parsed as a complete, ID-consistent catalog and SHA-256 verified artwork set.
//! The running executable is never replaced by remote content.
use crate::{catalog, security};
use eframe::egui;
use gacha_pickup_calculator::games::Game;
use image::ImageReader;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

const REPO_RAW: &str = "https://raw.githubusercontent.com/P-JSL/gacha_pickup_calculator/";
const HEAD_REF: &str = "https://api.github.com/repos/P-JSL/gacha_pickup_calculator/git/ref/heads/main";
const RELEASE_API: &str = "https://api.github.com/repos/P-JSL/gacha_pickup_calculator/releases/latest";
pub const RELEASE_PAGE: &str = "https://github.com/P-JSL/gacha_pickup_calculator/releases/latest";
const MAX_TEXT: usize = 1_048_576;
const MAX_IMAGE: usize = 512 * 1024;
const MAX_RECORDS: usize = 3000;

#[derive(Clone, Debug)]
pub struct VerifiedBundle {
    pub entries: Vec<catalog::Entry>,
    pub pairs: HashMap<(Game, String), String>,
    pub hashes: HashMap<String, [u8; 32]>,
    pub revision: String,
    pub image_dir: PathBuf,
}

/// Separate a fully parsed, immutable state from network and disk I/O.
#[derive(Debug)]
enum UpdateEvent {
    Cached(VerifiedBundle),
    Updated(VerifiedBundle),
    NoChange,
    Latest(String),
    Failed(String),
    Finished,
}

pub struct UpdateManager {
    send: Sender<UpdateEvent>,
    recv: Receiver<UpdateEvent>,
    running: bool,
    checked: bool,
    pub message: String,
    pub release: Option<String>,
    pub revision: Option<String>,
    started: Option<Instant>,
}

impl UpdateManager {
    pub fn new() -> Self {
        let (send, recv) = mpsc::channel();
        Self {
            send, recv, running: false, checked: false,
            message: "시작 시 게임 데이터 및 앱 버전을 확인합니다.".to_owned(),
            release: None, revision: None, started: None,
        }
    }

    pub fn start(&mut self, ctx: &egui::Context) {
        if self.running { return; }
        self.checked = true;
        self.running = true;
        self.started = Some(Instant::now());
        self.message = "게임 데이터 업데이트 확인 중…".to_owned();
        let send = self.send.clone();
        let context = ctx.clone();
        std::thread::spawn(move || {
            run_update(&send);
            let _ = send.send(UpdateEvent::Finished);
            context.request_repaint();
        });
    }

    /// Called on the UI thread. A poisoning error never bypasses a damaged lock.
    pub fn poll(&mut self, ctx: &egui::Context) -> bool {
        if !self.checked {
            self.start(ctx);
        }
        let mut changed = false;
        while let Ok(event) = self.recv.try_recv() {
            match event {
                UpdateEvent::Cached(bundle) | UpdateEvent::Updated(bundle) => {
                    let revision = bundle.revision.clone();
                    match catalog::publish_verified(bundle) {
                        Ok(()) => {
                            self.revision = Some(revision[..12].to_owned());
                            self.message = "검증된 최신 게임 데이터 적용 완료".to_owned();
                            changed = true;
                        }
                        Err(error) => {
                            self.message = format!("카탈로그 동기화 중단: {error}");
                        }
                    }
                }
                UpdateEvent::NoChange => {
                    self.message = "게임 데이터가 최신 상태입니다.".to_owned();
                }
                UpdateEvent::Latest(tag) => {
                    if newer_version(&tag, env!("CARGO_PKG_VERSION")) {
                        self.release = Some(tag);
                    }
                }
                UpdateEvent::Failed(reason) => {
                    self.message = format!("업데이트 실패(기존 데이터 유지): {reason}");
                }
                UpdateEvent::Finished => { self.running = false; self.started = None; }
            }
        }
        if self.running {
            // Poll only while work is outstanding; no busy rendering or UI blocking.
            ctx.request_repaint_after(Duration::from_millis(300));
            if self.started.is_some_and(|t| t.elapsed() > Duration::from_secs(150)) {
                self.message = "네트워크 지연: 기존 오프라인 데이터를 사용하고 있습니다.".to_owned();
            }
        }
        changed
    }

    pub fn is_running(&self) -> bool { self.running }
}

fn notify(send: &Sender<UpdateEvent>, event: UpdateEvent) {
    let _ = send.send(event);
}

fn run_update(send: &Sender<UpdateEvent>) {
    let cached = read_active_bundle();
    let previous = match cached {
        Ok(Some(bundle)) => {
            let rev = bundle.revision.clone();
            notify(send, UpdateEvent::Cached(bundle));
            Some(rev)
        }
        Ok(None) => None,
        Err(error) => {
            notify(send, UpdateEvent::Failed(format!("캐시 검증 오류: {error}")));
            None
        }
    };
    if let Ok(tag) = fetch_latest_tag() {
        notify(send, UpdateEvent::Latest(tag));
    }
    match fetch_head_sha().and_then(|revision| {
        if previous.as_deref() == Some(revision.as_str()) {
            return Ok(None);
        }
        fetch_and_stage(&revision).map(Some)
    }) {
        Ok(Some(bundle)) => notify(send, UpdateEvent::Updated(bundle)),
        Ok(None) => notify(send, UpdateEvent::NoChange),
        Err(error) => notify(send, UpdateEvent::Failed(error)),
    }
}

fn fetch_text(url: &str, max_bytes: usize) -> Result<String, String> {
    String::from_utf8(fetch_https(url, max_bytes)?)
        .map_err(|_| "서버 응답이 UTF-8 텍스트가 아닙니다.".to_owned())
}

fn fetch_https(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    if url != HEAD_REF && url != RELEASE_API && !url.starts_with(REPO_RAW) {
        return Err("승인되지 않은 업데이트 출처입니다.".to_owned());
    }
    // No shell, no redirects, strict HTTPS, bounded response and timeout.
    // curl ships with modern Windows and common Linux installations.
    let mut child = Command::new("curl")
        .args(["--fail", "--silent", "--show-error",
            "--proto", "=https", "--max-redirs", "0",
            "--connect-timeout", "6", "--max-time", "22",
            "--user-agent", "GachaPickupCalculator/verified-updates",
            "--header", "Accept: application/vnd.github+json",
            "--max-filesize", &max_bytes.to_string(), url])
        .stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e| format!("HTTPS 다운로드 도구를 실행할 수 없습니다: {e}"))?;
    let mut bytes = Vec::new();
    let result = child.stdout.take().ok_or("다운로드 출력이 없습니다.")?
        .take(max_bytes as u64 + 1).read_to_end(&mut bytes);
    if result.is_err() || bytes.len() > max_bytes {
        let _ = child.kill();
        let _ = child.wait();
        return Err("업데이트 파일이 허용 크기를 초과했습니다.".to_owned());
    }
    let status = child.wait().map_err(|e| format!("다운로드 종료 확인 실패: {e}"))?;
    if !status.success() { return Err(format!("HTTPS 서버 응답 오류: {status}")); }
    Ok(bytes)
}

/// Accept only flat unescaped ASCII fields from trusted GitHub REST responses.
fn json_field(body: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let offset = body.find(&key)? + key.len();
    let rest = body.get(offset..)?.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    let value = &rest[..end];
    (!value.contains('\\') && value.is_ascii()).then(|| value.to_owned())
}
fn is_commit_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn fetch_head_sha() -> Result<String, String> {
    let response = fetch_text(HEAD_REF, 16 * 1024)?;
    let sha = json_field(&response, "sha").ok_or("Git 참조 SHA가 없습니다.")?;
    if !is_commit_sha(&sha) { return Err("Git 참조 형식 오류".to_owned()); }
    Ok(sha)
}
fn fetch_latest_tag() -> Result<String, String> {
    let response = fetch_text(RELEASE_API, 128 * 1024)?;
    json_field(&response, "tag_name").ok_or("릴리즈 태그 누락".to_owned())
}
fn newer_version(tag: &str, installed: &str) -> bool {
    fn parse(v: &str) -> Option<[u32; 3]> {
        let mut x = v.trim_start_matches('v').split('.');
        let parts = [x.next()?.parse().ok()?, x.next()?.parse().ok()?, x.next()?.parse().ok()?];
        x.next().is_none().then_some(parts)
    }
    matches!((parse(tag), parse(installed)), (Some(a), Some(b)) if a > b)
}

pub fn cache_root() -> PathBuf {
    #[cfg(windows)]
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(path).join("GachaPickupCalculator").join("Updates");
    }
    #[cfg(not(windows))]
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        if Path::new(&path).is_absolute() {
            return PathBuf::from(path).join("gacha_pickup_calculator");
        }
    }
    if let Some(home) = std::env::var_os("HOME").filter(|h| Path::new(h).is_absolute()) {
        return PathBuf::from(home).join(".cache").join("gacha_pickup_calculator");
    }
    std::env::temp_dir().join("gacha_pickup_calculator_updates")
}

fn revision_path(revision: &str) -> PathBuf {
    cache_root().join("revisions").join(revision)
}

fn active_revision() -> Option<String> {
    let revision = fs::read_to_string(cache_root().join("active-revision")).ok()?;
    let sha = revision.trim();
    is_commit_sha(sha).then(|| sha.to_owned())
}

fn read_active_bundle() -> Result<Option<VerifiedBundle>, String> {
    let Some(revision) = active_revision() else { return Ok(None); };
    let root = revision_path(&revision);
    let catalog = fs::read_to_string(root.join("catalog.tsv")).map_err(|e| e.to_string())?;
    let pairs = fs::read_to_string(root.join("pairs.tsv")).map_err(|e| e.to_string())?;
    let manifest = fs::read_to_string(root.join("manifest.txt")).map_err(|e| e.to_string())?;
    let bundle = parse_bundle(&catalog, &pairs, &manifest, revision, root.join("Image"))?;
    verify_incremental_images(&bundle)?;
    Ok(Some(bundle))
}

fn source_url(revision: &str, path: &str) -> String {
    format!("{REPO_RAW}{revision}/{path}")
}

fn fetch_and_stage(revision: &str) -> Result<VerifiedBundle, String> {
    if !is_commit_sha(revision) { return Err("잘못된 revision".to_owned()); }
    let catalog = fetch_text(&source_url(revision, "src/catalog_data.tsv"), MAX_TEXT)?;
    let pairs = fetch_text(&source_url(revision, "src/signature_pairs.tsv"), MAX_TEXT)?;
    let manifest = fetch_text(&source_url(revision, "Image/SHA256SUMS.txt"), MAX_TEXT)?;
    let root = revision_path(revision);
    let bundle = parse_bundle(&catalog, &pairs, &manifest, revision.to_owned(), root.join("Image"))?;
    fs::create_dir_all(&root).map_err(|e| format!("캐시 디렉터리 생성 실패: {e}"))?;

    for key in bundle.hashes.keys() {
        let expected = bundle.hashes[key];
        if security::baseline_digest(key) == Some(expected) { continue; }
        let destination = bundle.image_dir.join(key);
        if image_file_is_valid(&destination, expected) { continue; }
        let bytes = fetch_https(&source_url(revision, &format!("Image/{key}")), MAX_IMAGE)?;
        verify_downloaded_image(&bytes, expected)?;
        write_atomic(&destination, &bytes)?;
    }
    // Validate the complete candidate before writing the activation marker.
    verify_incremental_images(&bundle)?;
    write_atomic(&root.join("catalog.tsv"), catalog.as_bytes())?;
    write_atomic(&root.join("pairs.tsv"), pairs.as_bytes())?;
    write_atomic(&root.join("manifest.txt"), manifest.as_bytes())?;
    write_atomic(&cache_root().join("active-revision"), revision.as_bytes())?;
    Ok(bundle)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("저장 경로에 상위 디렉터리가 없습니다.")?;
    fs::create_dir_all(parent).map_err(|e| format!("저장 경로 생성 실패: {e}"))?;
    let partial = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&partial, bytes).map_err(|e| format!("임시 파일 저장 실패: {e}"))?;
    // Existing cache is never trusted before a fresh digest verification.
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("기존 캐시 교체 실패: {e}"))?;
    }
    fs::rename(&partial, path).map_err(|e| format!("캐시 확정 실패: {e}"))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 96
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn game_code(game: Game) -> &'static str {
    match game {
        Game::StarRail => "hsr",
        Game::WutheringWaves => "wuwa",
        Game::Neverness => "nte",
        Game::StellaSora => "sora",
    }
}
fn parse_game(code: &str) -> Option<Game> {
    Game::ALL.iter().copied().find(|game| game_code(*game) == code)
}
fn image_key(game: Game, kind: catalog::Kind, id: &str) -> String {
    let dir = match game {
        Game::StarRail => "HSR",
        Game::WutheringWaves => "WutheringWaves",
        Game::Neverness => "Neverness",
        Game::StellaSora => "StellaSora",
    };
    let role = if kind == catalog::Kind::Character { "c" } else { "w" };
    format!("{dir}/{role}-{id}.png")
}
fn parse_hex(sha: &str) -> Option<[u8; 32]> {
    if sha.len() != 64 || !sha.is_ascii() { return None; }
    let mut value = [0_u8; 32];
    for (i, byte) in value.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&sha[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(value)
}

/// Parse-don't-validate: only this constructor can produce an updateable bundle.
/// Unknown weapon identities never get an inferred signature pairing.
pub fn parse_bundle(
    catalog_tsv: &str, pairs_tsv: &str, manifest: &str,
    revision: String, image_dir: PathBuf,
) -> Result<VerifiedBundle, String> {
    if !is_commit_sha(&revision) { return Err("커밋 식별자가 잘못되었습니다.".to_owned()); }
    let mut entries = Vec::new();
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    for line in catalog_tsv.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 6 { return Err(format!("카탈로그 필드 수 오류: {line}")); }
        let game = parse_game(f[0]).ok_or("알 수 없는 게임 ID")?;
        let kind = match f[1] {
            "c" => catalog::Kind::Character, "w" => catalog::Kind::Weapon,
            _ => return Err("알 수 없는 아이템 종류".to_owned()),
        };
        if !valid_id(f[2]) || f[3].trim().is_empty() || f[3].chars().any(char::is_control)
            || f[3].len() > 240 {
            return Err("캐릭터/장비 식별자가 올바르지 않습니다.".to_owned());
        }
        let rarity = f[4].parse::<u8>().map_err(|_| "희귀도 형식 오류")?;
        if !(1..=5).contains(&rarity) { return Err("희귀도가 범위를 벗어났습니다.".to_owned()); }
        if !f[5].is_empty() && !catalog::approved_icon_url(f[5]) {
            return Err(format!("허용되지 않은 이미지 출처: {}", f[2]));
        }
        if !ids.insert((game, kind, f[2].to_owned())) ||
            !names.insert((game, kind, f[3].to_owned())) {
            return Err(format!("중복 카탈로그 항목: {}", f[2]));
        }
        let previous = catalog::by_id(game, kind, f[2]);
        let status = previous.map_or("업데이트", |entry| entry.status);
        let release_order = previous.map_or(u32::MAX, |entry| entry.release_order);
        entries.push(catalog::Entry {
            game, kind, id: f[2].to_owned(), name: f[3].to_owned(),
            rarity, icon_url: f[5].to_owned(), release_order, status,
        });
        if entries.len() > MAX_RECORDS { return Err("최대 카탈로그 개수 초과".to_owned()); }
    }
    // Do not let an incomplete update silently erase the bundled baseline.
    if entries.len() < catalog::entries().len() {
        return Err("기본 카탈로그보다 적은 업데이트는 적용하지 않습니다.".to_owned());
    }
    let mut lookup: HashMap<(Game, catalog::Kind, String), u8> = HashMap::new();
    let mut image_keys = HashSet::new();
    for entry in &entries {
        lookup.insert((entry.game, entry.kind, entry.id.clone()), entry.rarity);
        image_keys.insert(image_key(entry.game, entry.kind, &entry.id));
    }
    let mut pairs = HashMap::new();
    for line in pairs_tsv.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 4 || !f[3].starts_with("https://")
            || f[3].len() > 512 || f[3].chars().any(char::is_control) {
            return Err("전용 장비 연결 출처 또는 TSV 형식 오류".to_owned());
        }
        let game = parse_game(f[0]).ok_or("연결 게임 ID 오류")?;
        if lookup.get(&(game, catalog::Kind::Character, f[1].to_owned())) != Some(&5)
            || lookup.get(&(game, catalog::Kind::Weapon, f[2].to_owned())) != Some(&5) {
            return Err(format!("존재하지 않는 5★ 전용 장비 연결: {line}"));
        }
        if pairs.insert((game, f[1].to_owned()), f[2].to_owned()).is_some() {
            return Err(format!("캐릭터 ID 중복 연결: {line}"));
        }
        if pairs.len() > MAX_RECORDS { return Err("전용 장비 연결 수 초과".to_owned()); }
    }
    let mut hashes = HashMap::new();
    for line in manifest.lines().filter(|l| !l.is_empty()) {
        let (hash, key) = line.split_once("  ").ok_or("이미지 해시 줄 형식 오류")?;
        let digest = parse_hex(hash).ok_or("이미지 SHA-256 형식 오류")?;
        if !image_keys.contains(key) || hashes.insert(key.to_owned(), digest).is_some() {
            return Err(format!("알 수 없거나 중복된 이미지 키: {key}"));
        }
        if hashes.len() > MAX_RECORDS { return Err("이미지 개수 초과".to_owned()); }
    }
    if hashes.len() != image_keys.len() {
        return Err("이미지 매니페스트와 카탈로그 개수가 일치하지 않습니다.".to_owned());
    }
    entries.sort_by(|a, b| {
        game_code(a.game).cmp(game_code(b.game))
            .then_with(|| (a.kind == catalog::Kind::Weapon).cmp(&(b.kind == catalog::Kind::Weapon)))
            .then_with(|| b.release_order.cmp(&a.release_order))
            .then_with(|| b.rarity.cmp(&a.rarity))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(VerifiedBundle { entries, pairs, hashes, revision, image_dir })
}

fn image_file_is_valid(path: &Path, expected: [u8; 32]) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else { return false };
    if !metadata.file_type().is_file() || metadata.len() > MAX_IMAGE as u64 { return false; }
    let Ok(bytes) = fs::read(path) else { return false };
    verify_downloaded_image(&bytes, expected).is_ok()
}
fn verify_downloaded_image(bytes: &[u8], expected: [u8; 32]) -> Result<(), String> {
    if bytes.len() > MAX_IMAGE || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || Sha256::digest(bytes).as_slice() != expected {
        return Err("이미지 형식 또는 SHA-256 검증 실패".to_owned());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format().map_err(|e| e.to_string())?;
    if reader.format() != Some(image::ImageFormat::Png) {
        return Err("PNG 이외 이미지 거부".to_owned());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(256);
    limits.max_image_height = Some(256);
    limits.max_alloc = Some(8 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| format!("이미지 디코드 실패: {e}"))?;
    Ok(())
}
fn verify_incremental_images(bundle: &VerifiedBundle) -> Result<(), String> {
    for (key, expected) in &bundle.hashes {
        if security::baseline_digest(key) == Some(*expected) { continue; }
        if !image_file_is_valid(&bundle.image_dir.join(key), *expected) {
            return Err(format!("검증된 이미지 캐시가 없습니다: {key}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_poisoned_or_partial_candidate_data() {
        let catalog = include_str!("catalog_data.tsv");
        let pairs = include_str!("signature_pairs.tsv");
        let manifest = include_str!("../Image/SHA256SUMS.txt");
        let rev = "a".repeat(40);
        assert!(parse_bundle(catalog, pairs, manifest, rev.clone(), PathBuf::new()).is_ok());
        assert!(parse_bundle(catalog, pairs, "", rev.clone(), PathBuf::new()).is_err());
        assert!(parse_bundle(catalog, pairs, manifest, "invalid".into(), PathBuf::new()).is_err());
        let broken = pairs.replace("hsr\t1511\t23065\t", "hsr\t1511\t999999\t");
        assert!(parse_bundle(catalog, &broken, manifest, rev, PathBuf::new()).is_err());
    }
    #[test]
    fn rejects_unexpected_release_versions_and_unsafe_urls() {
        assert!(newer_version("v0.2.3", "0.2.2"));
        assert!(!newer_version("v0.2.2", "0.2.2"));
        assert!(!newer_version("v0.2.4-evil", "0.2.2"));
        assert!(!is_commit_sha("../main"));
        assert!(valid_id("upcoming_astra_yao"));
        assert!(!valid_id("../bad"));
        assert_eq!(json_field(r#"{"tag_name":"v0.2.3"}"#, "tag_name").as_deref(), Some("v0.2.3"));
    }
}
