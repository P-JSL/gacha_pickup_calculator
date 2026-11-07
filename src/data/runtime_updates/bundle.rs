use super::*;

pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 96
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
pub(super) fn game_code(game: Game) -> &'static str {
    match game {
        Game::StarRail => "hsr",
        Game::WutheringWaves => "wuwa",
        Game::Neverness => "nte",
        Game::StellaSora => "sora",
    }
}
pub(super) fn parse_game(code: &str) -> Option<Game> {
    Game::ALL.iter().copied().find(|game| game_code(*game) == code)
}
pub(super) fn image_key(game: Game, kind: catalog::Kind, id: &str) -> String {
    let dir = match game {
        Game::StarRail => "HSR",
        Game::WutheringWaves => "WutheringWaves",
        Game::Neverness => "Neverness",
        Game::StellaSora => "StellaSora",
    };
    let role = if kind == catalog::Kind::Character { "c" } else { "w" };
    format!("{dir}/{role}-{id}.png")
}
pub(super) fn parse_hex(sha: &str) -> Option<[u8; 32]> {
    if sha.len() != 64 || !sha.is_ascii() { return None; }
    let mut value = [0_u8; 32];
    for (i, byte) in value.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&sha[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(value)
}

/// Parse-don't-validate: only this constructor can produce an updateable bundle.
/// Unknown weapon identities never get an inferred signature pairing.
pub(crate) fn parse_bundle(
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
    let featured_characters: HashSet<(Game, &str)> = entries.iter()
        .filter(|entry| entry.kind == catalog::Kind::Character && entry.is_featured_target())
        .map(|entry| (entry.game, entry.id.as_str())).collect();
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
        // O(1) expected membership replaces the old O(entries × pairs) scan.
        let featured_character = featured_characters.contains(&(game, f[1]));
        if !featured_character
            || lookup.get(&(game, catalog::Kind::Character, f[1].to_owned())) != Some(&5)
            || lookup.get(&(game, catalog::Kind::Weapon, f[2].to_owned())) != Some(&5) {
            return Err(format!("비픽업 캐릭터 또는 존재하지 않는 5★ 전용 장비 연결: {line}"));
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

pub(super) fn image_file_is_valid(path: &Path, expected: [u8; 32]) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else { return false };
    if !metadata.file_type().is_file() || metadata.len() > MAX_IMAGE as u64 { return false; }
    let Ok(bytes) = fs::read(path) else { return false };
    verify_downloaded_image(&bytes, expected).is_ok()
}
pub(super) fn verify_downloaded_image(bytes: &[u8], expected: [u8; 32]) -> Result<(), String> {
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
pub(super) fn verify_incremental_images(bundle: &VerifiedBundle) -> Result<(), String> {
    for (key, expected) in &bundle.hashes {
        if security::baseline_digest(key) == Some(*expected) { continue; }
        if !image_file_is_valid(&bundle.image_dir.join(key), *expected) {
            return Err(format!("검증된 이미지 캐시가 없습니다: {key}"));
        }
    }
    Ok(())
}

