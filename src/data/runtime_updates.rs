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

#[path = "runtime_updates/cache.rs"] mod cache;
pub(crate) use cache::cache_root;
use cache::{read_active_bundle, fetch_and_stage};
#[cfg(test)]
use cache::{read_bounded_cache, source_url, write_atomic};

#[path = "runtime_updates/bundle.rs"] mod bundle;
use bundle::*;

#[cfg(test)]
#[path = "runtime_updates/tests.rs"] mod tests;
