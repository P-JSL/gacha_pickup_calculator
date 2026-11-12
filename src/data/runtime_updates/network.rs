use super::*;

pub(super) fn fetch_text(url: &str, max_bytes: usize) -> Result<String, String> {
    String::from_utf8(fetch_https(url, max_bytes)?)
        .map_err(|_| "서버 응답이 UTF-8 텍스트가 아닙니다.".to_owned())
}

/// Only fetch version metadata or allowlisted assets pinned to a full commit SHA.
/// No user-supplied path, query, encoded traversal or unapproved URL can cross
/// this trust boundary even if future callers accidentally expose the helper.
pub(super) fn approved_update_url(url: &str) -> bool {
    if url == HEAD_REF || url == RELEASE_API { return true; }
    if url.strip_prefix(COMMIT_API).is_some_and(is_commit_sha) { return true; }
    let Some((revision, path)) = url.strip_prefix(REPO_RAW)
        .and_then(|remainder| remainder.split_once('/')) else { return false; };
    if !is_commit_sha(revision) { return false; }
    match path {
        "src/catalog_data.tsv" | "src/signature_pairs.tsv" | "Image/SHA256SUMS.txt" => true,
        _ => {
            let Some(relative) = path.strip_prefix("Image/") else { return false; };
            let mut parts = relative.split('/');
            let (Some(game), Some(file), None) = (parts.next(), parts.next(), parts.next())
                else { return false; };
            if !["HSR", "WutheringWaves", "Neverness", "StellaSora"].contains(&game) {
                return false;
            }
            let Some((role, filename)) = file.split_once('-') else { return false; };
            if role != "c" && role != "w" { return false; }
            filename.strip_suffix(".png").is_some_and(valid_id)
        }
    }
}

pub(super) fn fetch_https(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    if !approved_update_url(url) {
        return Err("승인되지 않은 업데이트 출처입니다.".to_owned());
    }
    // No shell, no redirects, strict HTTPS, bounded response and timeout.
    // curl ships with modern Windows and common Linux installations.
    let mut command = Command::new("curl");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Do not flash a console window when the egui app starts on Windows.
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        // Must be the FIRST argument: prevent ~/.curlrc from enabling redirects,
        // insecure TLS, alternate output paths, or unintended transfer options.
        .args(["--disable", "--fail", "--silent", "--show-error",
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
pub(super) fn json_field(body: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let offset = body.find(&key)? + key.len();
    let rest = body.get(offset..)?.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let end = rest.find('"')?;
    let value = &rest[..end];
    (!value.contains('\\') && value.is_ascii()).then(|| value.to_owned())
}
pub(super) fn is_commit_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(super) fn fetch_head_sha() -> Result<String, String> {
    let response = fetch_text(HEAD_REF, 16 * 1024)?;
    let sha = json_field(&response, "sha").ok_or("Git 참조 SHA가 없습니다.")?;
    if !is_commit_sha(&sha) { return Err("Git 참조 형식 오류".to_owned()); }
    Ok(sha)
}
// A small bounded JSON reader for GitHub commit attestation. This avoids a
// security-sensitive substring search without adding a new dependency.
// It validates the whole response, checks nesting/duplicate keys, and rejects
// ambiguous escaped field names. GitHub's API uses plain ASCII field names.
enum AttestationValue<'a> {
    Object(Vec<(&'a str, AttestationValue<'a>)>),
    Text(&'a str),
    Boolean(bool),
    Other,
}

impl<'a> AttestationValue<'a> {
    fn field(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Object(fields) => fields.iter()
                .find(|(field, _)| *field == name).map(|(_, value)| value),
            _ => None,
        }
    }
    fn text(&self) -> Option<&str> {
        if let Self::Text(value) = self { Some(value) } else { None }
    }
    fn boolean(&self) -> Option<bool> {
        if let Self::Boolean(value) = self { Some(*value) } else { None }
    }
}

struct AttestationJson<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> AttestationJson<'a> {
    fn parse(body: &'a str) -> Option<AttestationValue<'a>> {
        let mut parser = Self { bytes: body.as_bytes(), cursor: 0 };
        let value = parser.value(0)?;
        parser.whitespace();
        (parser.cursor == parser.bytes.len()).then_some(value)
    }
    fn whitespace(&mut self) {
        while matches!(self.bytes.get(self.cursor),
            Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.cursor += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        if self.bytes.get(self.cursor) == Some(&byte) {
            self.cursor += 1;
            true
        } else { false }
    }
    fn word(&mut self, literal: &[u8]) -> bool {
        if self.bytes.get(self.cursor..self.cursor + literal.len()) == Some(literal) {
            self.cursor += literal.len();
            true
        } else { false }
    }
    fn string(&mut self, key: bool) -> Option<&'a str> {
        if !self.take(b'"') { return None; }
        let start = self.cursor;
        while let Some(&byte) = self.bytes.get(self.cursor) {
            match byte {
                b'"' => {
                    let end = self.cursor;
                    self.cursor += 1;
                    return std::str::from_utf8(&self.bytes[start..end]).ok();
                }
                b'\\' => {
                    // An escaped object key could duplicate a security-relevant
                    // field under a different spelling: fail closed.
                    if key { return None; }
                    self.cursor += 1;
                    let escaped = *self.bytes.get(self.cursor)?;
                    self.cursor += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                        b'u' => {
                            for _ in 0..4 {
                                if !self.bytes.get(self.cursor)?.is_ascii_hexdigit() {
                                    return None;
                                }
                                self.cursor += 1;
                            }
                        }
                        _ => return None,
                    }
                }
                0..=31 => return None,
                _ => self.cursor += 1,
            }
        }
        None
    }
    fn number(&mut self) -> bool {
        self.take(b'-');
        if !self.take(b'0') {
            if !matches!(self.bytes.get(self.cursor), Some(b'1'..=b'9')) {
                return false;
            }
            self.cursor += 1;
            while matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                self.cursor += 1;
            }
        }
        if self.take(b'.') {
            if !matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                return false;
            }
            while matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                self.cursor += 1;
            }
        }
        if self.take(b'e') || self.take(b'E') {
            if !self.take(b'+') { self.take(b'-'); }
            if !matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                return false;
            }
            while matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                self.cursor += 1;
            }
        }
        true
    }
    fn value(&mut self, depth: usize) -> Option<AttestationValue<'a>> {
        if depth > 48 { return None; }
        self.whitespace();
        let byte = *self.bytes.get(self.cursor)?;
        match byte {
            b'{' => {
                self.cursor += 1;
                self.whitespace();
                let mut fields = Vec::new();
                if self.take(b'}') { return Some(AttestationValue::Object(fields)); }
                loop {
                    self.whitespace();
                    let name = self.string(true)?;
                    if fields.iter().any(|(previous, _)| *previous == name) {
                        return None;
                    }
                    self.whitespace();
                    if !self.take(b':') { return None; }
                    let value = self.value(depth + 1)?;
                    fields.push((name, value));
                    self.whitespace();
                    if self.take(b'}') { break; }
                    if !self.take(b',') { return None; }
                }
                Some(AttestationValue::Object(fields))
            }
            b'[' => {
                self.cursor += 1;
                self.whitespace();
                if self.take(b']') { return Some(AttestationValue::Other); }
                loop {
                    self.value(depth + 1)?;
                    self.whitespace();
                    if self.take(b']') { break; }
                    if !self.take(b',') { return None; }
                }
                Some(AttestationValue::Other)
            }
            b'"' => Some(AttestationValue::Text(self.string(false)?)),
            b't' if self.word(b"true") => Some(AttestationValue::Boolean(true)),
            b'f' if self.word(b"false") => Some(AttestationValue::Boolean(false)),
            b'n' if self.word(b"null") => Some(AttestationValue::Other),
            b'-' | b'0'..=b'9' if self.number() => Some(AttestationValue::Other),
            _ => None,
        }
    }
}

/// Require GitHub's verified commit signature and the repository's expected
/// signing/author identity. This is an additional online GitHub trust check,
/// not a standalone vendor signature or protection against an admin takeover.
pub(super) fn verified_github_commit(response: &str, expected_sha: &str) -> bool {
    if !is_commit_sha(expected_sha) { return false; }
    let Some(root) = AttestationJson::parse(response) else { return false; };
    let commit = root.field("commit");
    let verification = commit.and_then(|c| c.field("verification"));
    let committer = commit.and_then(|c| c.field("committer"));
    root.field("sha").and_then(AttestationValue::text) == Some(expected_sha)
        && verification.and_then(|v| v.field("verified"))
            .and_then(AttestationValue::boolean) == Some(true)
        && verification.and_then(|v| v.field("reason"))
            .and_then(AttestationValue::text) == Some("valid")
        && committer.and_then(|c| c.field("name"))
            .and_then(AttestationValue::text) == Some("GitHub")
        && committer.and_then(|c| c.field("email"))
            .and_then(AttestationValue::text) == Some("noreply@github.com")
        && root.field("author").and_then(|a| a.field("login"))
            .and_then(AttestationValue::text) == Some("P-JSL")
}

pub(super) fn verify_remote_commit(revision: &str) -> Result<(), String> {
    if !is_commit_sha(revision) {
        return Err("허용되지 않은 Git 커밋 SHA입니다.".to_owned());
    }
    let url = format!("{COMMIT_API}{revision}");
    let response = fetch_text(&url, 128 * 1024)?;
    if !verified_github_commit(&response, revision) {
        return Err("서명이 검증되지 않았거나 예상 게시자가 아닌 커밋입니다.".to_owned());
    }
    Ok(())
}

pub(super) fn fetch_latest_tag() -> Result<String, String> {
    let response = fetch_text(RELEASE_API, 128 * 1024)?;
    json_field(&response, "tag_name").ok_or("릴리즈 태그 누락".to_owned())
}
pub(super) fn newer_version(tag: &str, installed: &str) -> bool {
    fn parse(v: &str) -> Option<[u32; 3]> {
        let mut x = v.trim_start_matches('v').split('.');
        let parts = [x.next()?.parse().ok()?, x.next()?.parse().ok()?, x.next()?.parse().ok()?];
        x.next().is_none().then_some(parts)
    }
    matches!((parse(tag), parse(installed)), (Some(a), Some(b)) if a > b)
}

