//! Opt-in-safe background update transport. Network responses are untrusted until
//! parsed as a complete, ID-consistent catalog and SHA-256 verified artwork set.
//! The running executable is never replaced by remote content.
use crate::{catalog, security};
use eframe::egui;
use gacha_pickup_calculator::games::Game;
use image::ImageReader;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

const REPO_RAW: &str = "https://raw.githubusercontent.com/P-JSL/gacha_pickup_calculator/";
const HEAD_REF: &str = "https://api.github.com/repos/P-JSL/gacha_pickup_calculator/git/ref/heads/main";
const RELEASE_API: &str = "https://api.github.com/repos/P-JSL/gacha_pickup_calculator/releases/latest";
const COMMIT_API: &str = "https://api.github.com/repos/P-JSL/gacha_pickup_calculator/commits/";
pub(crate) const RELEASE_PAGE: &str = "https://github.com/P-JSL/gacha_pickup_calculator/releases/latest";
const MAX_TEXT: usize = 1_048_576;
const MAX_IMAGE: usize = 512 * 1024;
const MAX_RECORDS: usize = 3000;

#[derive(Clone, Debug)]
pub(crate) struct VerifiedBundle {
    pub(crate) entries: Vec<catalog::Entry>,
    pub(crate) pairs: HashMap<(Game, String), String>,
    pub(crate) hashes: HashMap<String, [u8; 32]>,
    pub(crate) revision: String,
    pub(crate) image_dir: PathBuf,
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

pub(crate) struct UpdateManager {
    send: Sender<UpdateEvent>,
    recv: Receiver<UpdateEvent>,
    running: bool,
    checked: bool,
    pub(crate) message: String,
    pub(crate) release: Option<String>,
    pub(crate) revision: Option<String>,
    started: Option<Instant>,
}

impl UpdateManager {
    pub(crate) fn new() -> Self {
        let (send, recv) = mpsc::channel();
        Self {
            send, recv, running: false, checked: false,
            message: "시작 시 게임 데이터 및 앱 버전을 확인합니다.".to_owned(),
            release: None, revision: None, started: None,
        }
    }

    pub(crate) fn start(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn poll(&mut self, ctx: &egui::Context) -> bool {
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

    pub(crate) fn is_running(&self) -> bool { self.running }
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
        // A commit SHA alone does not authenticate the owner of its contents.
        // Reject unsigned or unverified upstream commits before network staging.
        // This supplements, but does not replace, offline signed metadata:
        // a compromised GitHub account can still publish verified commits.
        if previous.as_deref() == Some(revision.as_str()) {
            return Ok(None);
        }
        verify_remote_commit(&revision)?;
        fetch_and_stage(&revision).map(Some)
    }) {
        Ok(Some(bundle)) => notify(send, UpdateEvent::Updated(bundle)),
        Ok(None) => notify(send, UpdateEvent::NoChange),
        Err(error) => notify(send, UpdateEvent::Failed(error)),
    }
}

#[path = "runtime_updates/network.rs"] mod network;
use network::*;

pub(crate) fn cache_root() -> PathBuf {
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

/// Bound cached text independently of its origin. A corrupt or replaced cache
/// must not allocate unbounded memory when the application starts offline.
fn read_bounded_cache(path: &Path, max_bytes: usize) -> Result<String, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("캐시 파일 검사 실패: {e}"))?;
    if !meta.file_type().is_file() || meta.len() > max_bytes as u64 {
        return Err("캐시 파일이 일반 파일이 아니거나 크기 제한을 초과했습니다.".to_owned());
    }
    let file = File::open(path).map_err(|e| format!("캐시 파일 열기 실패: {e}"))?;
    let opened = file.metadata().map_err(|e| format!("캐시 메타데이터 실패: {e}"))?;
    if !opened.is_file() || opened.len() > max_bytes as u64 {
        return Err("캐시 파일 크기가 읽기 도중 변경되었습니다.".to_owned());
    }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if meta.dev() != opened.dev() || meta.ino() != opened.ino() {
            return Err("캐시 파일이 검사와 열기 사이 교체되었습니다.".to_owned());
        }
    }
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)
        .map_err(|e| format!("캐시 파일 읽기 실패: {e}"))?;
    if bytes.len() > max_bytes {
        return Err("캐시 파일이 읽기 중 크기 제한을 초과했습니다.".to_owned());
    }
    String::from_utf8(bytes).map_err(|e| format!("캐시 파일 UTF-8 오류: {e}"))
}

fn active_revision() -> Option<String> {
    let revision = read_bounded_cache(&cache_root().join("active-revision"), 128).ok()?;
    let sha = revision.trim();
    is_commit_sha(sha).then(|| sha.to_owned())
}

fn read_active_bundle() -> Result<Option<VerifiedBundle>, String> {
    let Some(revision) = active_revision() else { return Ok(None); };
    let root = revision_path(&revision);
    let catalog = read_bounded_cache(&root.join("catalog.tsv"), MAX_TEXT)?;
    let pairs = read_bounded_cache(&root.join("pairs.tsv"), MAX_TEXT)?;
    let manifest = read_bounded_cache(&root.join("manifest.txt"), MAX_TEXT)?;
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
    // A pre-existing same-account symlink is not a safe cache destination.
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.file_type().is_file() {
            return Err("캐시 저장 위치가 일반 파일이 아닙니다.".to_owned());
        }
    }
    // create_new prevents following a pre-planted symlink with the temporary
    // name; varying the sequence allows recovery after an interrupted update.
    let mut partial = None;
    for attempt in 0..32_u32 {
        let candidate = path.with_extension(format!("tmp-{}-{attempt}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(file) => { partial = Some((candidate, file)); break; }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("임시 캐시 생성 실패: {error}")),
        }
    }
    let (temporary, mut file) = partial.ok_or("임시 캐시 파일 이름을 확보할 수 없습니다.")?;
    let result = (|| {
        file.write_all(bytes).map_err(|e| format!("임시 캐시 저장 실패: {e}"))?;
        file.sync_all().map_err(|e| format!("임시 캐시 동기화 실패: {e}"))?;
        drop(file);
        // Unix rename atomically replaces a regular file. Windows does not
        // replace existing destinations, so remove only after staging succeeds.
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(path).map_err(|e| format!("기존 캐시 교체 실패: {e}"))?;
        }
        fs::rename(&temporary, path).map_err(|e| format!("캐시 확정 실패: {e}"))
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result
}

#[path = "runtime_updates/bundle.rs"] mod bundle;
use bundle::*;

#[cfg(test)]
#[path = "runtime_updates/tests.rs"] mod tests;
