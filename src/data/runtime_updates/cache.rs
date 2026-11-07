use super::*;

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

pub(super) fn revision_path(revision: &str) -> PathBuf {
    cache_root().join("revisions").join(revision)
}

/// Bound cached text independently of its origin. A corrupt or replaced cache
/// must not allocate unbounded memory when the application starts offline.
pub(super) fn read_bounded_cache(path: &Path, max_bytes: usize) -> Result<String, String> {
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

pub(super) fn active_revision() -> Option<String> {
    let revision = read_bounded_cache(&cache_root().join("active-revision"), 128).ok()?;
    let sha = revision.trim();
    is_commit_sha(sha).then(|| sha.to_owned())
}

pub(super) fn read_active_bundle() -> Result<Option<VerifiedBundle>, String> {
    let Some(revision) = active_revision() else { return Ok(None); };
    let root = revision_path(&revision);
    let catalog = read_bounded_cache(&root.join("catalog.tsv"), MAX_TEXT)?;
    let pairs = read_bounded_cache(&root.join("pairs.tsv"), MAX_TEXT)?;
    let manifest = read_bounded_cache(&root.join("manifest.txt"), MAX_TEXT)?;
    let bundle = parse_bundle(&catalog, &pairs, &manifest, revision, root.join("Image"))?;
    verify_incremental_images(&bundle)?;
    Ok(Some(bundle))
}

pub(super) fn source_url(revision: &str, path: &str) -> String {
    format!("{REPO_RAW}{revision}/{path}")
}

pub(super) fn fetch_and_stage(revision: &str) -> Result<VerifiedBundle, String> {
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

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
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

