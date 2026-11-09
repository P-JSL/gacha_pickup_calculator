//! Per-process, append-only diagnostics. Truncated exactly once at startup.
//! Calculation workers and the GUI share one synchronized writer; the window
//! only keeps a bounded, in-memory tail, while the on-disk file remains complete.
use eframe::egui;
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const VISIBLE_LINES: usize = 20_000;

static CURRENT_SESSION: OnceLock<Arc<SessionLogger>> = OnceLock::new();

/// Installed once by main(), before eframe starts any workers.
pub(crate) fn install(logger: Arc<SessionLogger>) {
    assert!(CURRENT_SESSION.set(logger).is_ok(), "session logger initialized twice");
}
pub(crate) fn current() -> Option<&'static Arc<SessionLogger>> {
    CURRENT_SESSION.get()
}
pub(crate) fn info(code: &str, message: impl AsRef<str>) {
    if let Some(logger) = current() { logger.info(code, message); }
}
pub(crate) fn warn(code: &str, message: impl AsRef<str>) {
    if let Some(logger) = current() { logger.warn(code, message); }
}
pub(crate) fn error(code: &str, message: impl AsRef<str>) {
    if let Some(logger) = current() { logger.error(code, message); }
}


struct LogState {
    file: Option<File>,
    lines: VecDeque<String>,
    revision: u64,
    warning: Option<String>,
}

pub(crate) struct SessionLogger {
    path: PathBuf,
    started: Instant,
    state: Mutex<LogState>,
}

impl SessionLogger {
    pub(crate) fn start_default() -> Self {
        let base = if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        } else {
            std::env::var_os("XDG_STATE_HOME").map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("state")))
        }.unwrap_or_else(std::env::temp_dir);
        Self::start_at(base.join("gacha_pickup_calculator").join("session.log"))
    }

    /// Do not follow a preexisting symlink while truncating last session's file.
    fn open_fresh(path: &Path) -> io::Result<File> {
        if path.symlink_metadata().is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(io::Error::other("Refusing to truncate a symbolic-link log"));
        }
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        OpenOptions::new().create(true).write(true).truncate(true).open(path)
    }

    fn start_at(path: PathBuf) -> Self {
        let (file, warning) = match Self::open_fresh(&path) {
            Ok(file) => (Some(file), None),
            Err(error) => (None, Some(format!(
                "로그 파일을 열 수 없습니다. 이 실행의 로그는 메모리에만 보관됩니다: {error}"
            ))),
        };
        let logger = Self {
            path,
            started: Instant::now(),
            state: Mutex::new(LogState {
                file, lines: VecDeque::new(), revision: 0, warning,
            }),
        };
        logger.info("SESSION_START", format!(
            "Gacha Pickup Calculator v{}; current-session log reset",
            env!("CARGO_PKG_VERSION"),
        ));
        logger
    }

    pub(crate) fn path(&self) -> &Path { &self.path }

    pub(crate) fn info(&self, code: &str, description: impl AsRef<str>) {
        self.record("INFO", code, description);
    }
    pub(crate) fn warn(&self, code: &str, description: impl AsRef<str>) {
        self.record("WARN", code, description);
    }
    pub(crate) fn error(&self, code: &str, description: impl AsRef<str>) {
        self.record("ERROR", code, description);
    }

    fn record(&self, level: &str, code: &str, description: impl AsRef<str>) {
        let timestamp = utc_stamp(SystemTime::now());
        let elapsed_ms = self.started.elapsed().as_millis();
        let safe = description.as_ref().replace(['\r', '\n'], " ");
        let safe = safe.chars().take(2_000).collect::<String>();
        let line = format!("[{timestamp}] [+{elapsed_ms:>8}ms] [{level}] [{code}] {safe}");
        let mut state = self.state.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Some(file) = &mut state.file {
            if let Err(error) = writeln!(file, "{line}").and_then(|_| file.flush()) {
                state.file = None;
                state.warning = Some(format!(
                    "로그 기록이 중단되어 이후 메시지는 메모리에만 보관됩니다: {error}"
                ));
            }
        }
        state.lines.push_back(line);
        if state.lines.len() > VISIBLE_LINES { state.lines.pop_front(); }
        state.revision = state.revision.wrapping_add(1);
    }

    fn revision(&self) -> u64 {
        self.state.lock().unwrap_or_else(|poison| poison.into_inner()).revision
    }
    fn snapshot(&self) -> (u64, Vec<String>, Option<String>) {
        let state = self.state.lock().unwrap_or_else(|poison| poison.into_inner());
        (state.revision, state.lines.iter().cloned().collect(), state.warning.clone())
    }
}

/// Civil date in UTC without a new dependency or platform-specific APIs.
/// Algorithm works for all nonnegative Unix timestamps.
fn utc_stamp(now: SystemTime) -> String {
    let duration = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = duration.as_secs() as i64;
    let mut z = seconds.div_euclid(86_400) + 719_468;
    let era = z / 146_097;
    z -= era * 146_097;
    let yoe = (z - z / 1_460 + z / 36_524 - z / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = z - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    if month <= 2 { year += 1; }
    let second_of_day = seconds.rem_euclid(86_400);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        second_of_day / 3_600, (second_of_day % 3_600) / 60,
        second_of_day % 60, duration.subsec_millis())
}

#[derive(Default)]
pub(crate) struct LogViewer {
    pub(crate) open: bool,
    filter: String,
    cached_filter: String,
    cached_revision: u64,
    entries: Vec<String>,
    matching: Vec<usize>,
    warning: Option<String>,
}

impl LogViewer {
    pub(crate) fn show(&mut self, ctx: &egui::Context, logger: &SessionLogger) {
        if !self.open { return; }
        if self.cached_revision != logger.revision() {
            let (revision, lines, warning) = logger.snapshot();
            self.cached_revision = revision;
            self.entries = lines;
            self.warning = warning;
            // Rebuild the visible index below; never scan every line per frame.
            self.cached_filter = "\0".to_owned();
        }
        let mut open = self.open;
        egui::Window::new("실행 로그 · Session log")
            .open(&mut open)
            .default_size(egui::vec2(920.0, 590.0))
            .resizable(true)
            .show(ctx, |ui| {
                ui.add(egui::Label::new(format!(
                    "실행할 때마다 초기화 · UTC 시각 · 전체 파일: {}",
                    logger.path().display()
                )).wrap());
                if let Some(warning) = &self.warning {
                    ui.colored_label(egui::Color32::YELLOW, warning);
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("검색");
                    ui.add(egui::TextEdit::singleline(&mut self.filter)
                        .desired_width(230.0).hint_text("CALC, ERROR, game, generation..."));
                    if ui.button("검색 초기화").clicked() { self.filter.clear(); }
                    if ui.button("표시된 로그 복사").clicked() {
                        let copied = self.matching.iter()
                            .map(|&index| self.entries[index].as_str())
                            .collect::<Vec<_>>().join("\n");
                        ui.ctx().copy_text(copied);
                    }
                });
                if self.cached_filter != self.filter {
                    self.cached_filter.clone_from(&self.filter);
                    let filter = self.filter.to_lowercase();
                    self.matching = self.entries.iter().enumerate()
                        .filter(|(_, line)| line.to_lowercase().contains(&filter))
                        .map(|(index, _)| index).collect();
                }
                ui.label(format!(
                    "{} / {}건 표시 · 최신 {}건까지 창에 보관, 전체 기록은 파일에 저장",
                    self.matching.len(), self.entries.len(), VISIBLE_LINES
                ));
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false])
                    .max_height(ui.available_height().max(150.0))
                    .show_rows(ui, 19.0, self.matching.len(), |ui, range| {
                        for index in range {
                            let line = &self.entries[self.matching[index]];
                            // Virtualized rows must retain a fixed height:
                            // wrapped multi-line labels would overlap following
                            // rows. Full content is accessible on hover/copy/file.
                            ui.add(egui::Label::new(
                                egui::RichText::new(line).monospace().size(11.0)
                            ).truncate()).on_hover_text(line);
                        }
                    });
            });
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn new_process_truncates_old_session_and_records_shutdown() {
        let path = std::env::temp_dir().join(format!(
            "gacha-session-test-{}-{}.log", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::write(&path, "OLD SESSION SECRET\n").unwrap();
        let logger = SessionLogger::start_at(path.clone());
        logger.info("CALC_STARTED", "generation=2");
        logger.info("SESSION_END", "window closed");
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("OLD SESSION SECRET"));
        assert!(content.contains("[SESSION_START]"));
        assert!(content.contains("[CALC_STARTED]"));
        assert!(content.contains("[SESSION_END]"));
        drop(logger);
        let second = SessionLogger::start_at(path.clone());
        assert!(!fs::read_to_string(&path).unwrap().contains("CALC_STARTED"));
        drop(second);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn worker_threads_share_ordered_lossless_file() {
        let path = std::env::temp_dir().join(format!(
            "gacha-threaded-log-{}-{}.log", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let logger = Arc::new(SessionLogger::start_at(path.clone()));
        let clones = (0..6).map(|worker| {
            let logger = Arc::clone(&logger);
            std::thread::spawn(move || {
                for iteration in 0..20 {
                    logger.info("WORKER", format!("thread={worker} iter={iteration}"));
                }
            })
        }).collect::<Vec<_>>();
        for join in clones { join.join().unwrap(); }
        let file = fs::read_to_string(&path).unwrap();
        assert_eq!(file.matches("[WORKER]").count(), 120);
        assert_eq!(logger.snapshot().1.len(), 121);
        drop(logger);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn utc_stamp_is_human_readable() {
        assert_eq!(utc_stamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        assert_eq!(utc_stamp(UNIX_EPOCH + std::time::Duration::from_secs(1_735_689_600)),
            "2025-01-01T00:00:00.000Z");
    }
}
