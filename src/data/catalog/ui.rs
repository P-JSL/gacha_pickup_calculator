use super::*;

type ImageResult = (u64, String, Option<egui::ColorImage>);
pub(crate) struct CatalogUi {
    textures: HashMap<String, Option<egui::TextureHandle>>,
    queued: HashSet<String>,
    send_url: Sender<(u64, String)>,
    generation: u64,
    recv_image: Receiver<ImageResult>,
}

impl CatalogUi {
    pub(crate) fn new(ctx: &egui::Context) -> Self {
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
    pub(super) fn fetch_icon(relative: &str) -> Option<egui::ColorImage> {
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
                let decoded = reader.decode().ok()?;
                // Compact release images may be stored below the original
                // 112px source size. Expand only once on the image worker,
                // using high-quality Lanczos3 followed by gentle edge sharpening;
                // texture upload and UI repaint never repeat this work.
                // Resampling improves perceived clarity but cannot recreate
                // details removed by lossy quantization.
                let image = if decoded.width().max(decoded.height()) < 112 {
                    let original_max = decoded.width().max(decoded.height()) as f64;
                    let target_width = ((decoded.width() as f64 * 112.0 / original_max)
                        .round() as u32).max(1);
                    let target_height = ((decoded.height() as f64 * 112.0 / original_max)
                        .round() as u32).max(1);
                    let upscaled = image::imageops::resize(&decoded.to_rgba8(),
                        target_width, target_height, image::imageops::FilterType::Lanczos3);
                    image::imageops::unsharpen(&upscaled, 0.55, 3)
                } else {
                    decoded.thumbnail(112, 112).to_rgba8()
                };
                Some(egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize], image.as_raw(),
                ))
            })() else { continue };
            return Some(decoded);
        }
        None
    }

    pub(crate) fn pump(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn invalidate_after_update(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.textures.clear();
        self.queued.clear();
    }

    /// Failed icon requests can be retried without clearing successfully cached images.
    pub(crate) fn retry_failed(&mut self) {
        self.textures.retain(|_, texture| texture.is_some());
    }

    pub(crate) fn image(&mut self, ui: &mut egui::Ui, entry: &Entry, size: f32) {
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
pub(crate) fn picker(
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

