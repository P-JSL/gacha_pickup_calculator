impl Calculator {

    fn tracker_sources(&self, ui: &mut egui::Ui) {
        section(ui, "07", "실측 트래커 교차 검증",
                "관측 평균과 현재 확률 모델을 비교합니다. 표본은 공식 서버 확률이 아니며 자동 보정하지 않습니다.", |ui| {
            let observed = references(self.setup.game);
            for source in observed {
                ui.horizontal_wrapped(|ui| {
                    ui.hyperlink_to(source.name, source.url);
                    ui.label(RichText::new(format!("관측 {} (고정 스냅샷)", source.observed_on))
                        .color(MUTED).size(11.0));
                });
                if let Some(mean) = source.mean_rare_pulls {
                    let banner = match source.banner {
                        TrackedBanner::Character => Some(&self.setup.character),
                        TrackedBanner::Weapon => Some(&self.setup.weapon),
                        TrackedBanner::General => None,
                    };
                    if let Some(pred) = banner.and_then(|b| expected_for_banner(b, self.mode)) {
                        let diff = (pred - mean) / mean * 100.0;
                        ui.label(format!("5성 간격: 트래커 평균 {:.1}뽑 | 모델 기대 {:.1}뽑 | 편차 {:+.1}%",
                            mean, pred, diff));
                        ui.small("같은 배너·천장 0부터의 5성 획득 간격 비교입니다. 픽업 확정·중복 환급은 포함되지 않습니다.");
                    }
                }
                if let Some(count) = source.total_pulls {
                    ui.small(format!("사이트 표기 표본 뽑기 횟수: {count}"));
                }
                ui.label(RichText::new(source.notes).color(MUTED).size(11.0));
                ui.separator();
            }
            ui.colored_label(GOLD,
                "평균 스택 하나만으로 스택별 확률이나 소프트 천장 함수를 역산할 수 없습니다. 이탈·불완전 이력·표본 편향을 확인해야 합니다.");
        });
    }

    /// The user explicitly provides a local CSV path; data remains on device.
    /// This is offline inspection only: never fetch undocumented tracker APIs
    /// or accept account-authenticated URLs and tokens.
    fn observation_validation(&mut self, ui: &mut egui::Ui) {
        section(ui, "08", "스택별 실측 분포 비교",
                "CSV를 로컬에서 분석합니다. 완전한 이력·검열 구간이 없는 트래커 그래프는 편향 가능성을 표시합니다.", |ui| {
            let mut banner_changed = false;
            ui.horizontal_wrapped(|ui| {
                ui.label("데이터 배너");
                egui::ComboBox::from_id_salt("observed_banner")
                    .selected_text(match self.observation_banner {
                        TrackedBanner::Character => "캐릭터",
                        TrackedBanner::Weapon => "무기·광추",
                        TrackedBanner::General => "선택 필요",
                    })
                    .show_ui(ui, |ui| {
                        banner_changed |= ui.selectable_value(
                            &mut self.observation_banner, TrackedBanner::Character, "캐릭터").changed();
                        banner_changed |= ui.selectable_value(
                            &mut self.observation_banner, TrackedBanner::Weapon, "무기·광추").changed();
                    });
            });
            if banner_changed {
                self.observation_data = None;
                self.observation_message = "배너가 변경되었습니다. 해당 배너의 이력을 다시 불러오세요.".into();
            }
            let rule = match self.observation_banner {
                TrackedBanner::Character => &self.setup.character.rule,
                TrackedBanner::Weapon => &self.setup.weapon.rule,
                TrackedBanner::General => &self.setup.character.rule,
            };
            // Only a one-dimensional rarity hazard can be compared directly.
            // NTE Arc is a two-counter, 10-pull batch with nontrivial guarantee
            // substitution. Stella Sora uses direct pickup counters and a free
            // one-time 120-pull reward, not ordinary 5★ rarity pity.
            let candidate = match rule {
                BannerRule::Standard { rarity, .. } => Some(rarity.clone()),
                BannerRule::FeaturedCounter { .. } | BannerRule::NteArcBatch => None,
            };
            ui.horizontal_wrapped(|ui| {
                ui.label("신뢰할 수 있는 CSV SHA-256 (선택)");
                ui.add(egui::TextEdit::singleline(&mut self.observation_expected_sha256)
                    .hint_text("신뢰한 원본의 SHA-256 64자리").desired_width(460.0));
            });
            ui.small("해시를 지정하면 파일 내용까지 일치해야 합니다. 해시가 없으면 일반 CSV 형식 검사만 수행하며 데이터 출처의 진위를 보장하지 않습니다.");
            ui.horizontal_wrapped(|ui| {
                ui.label("CSV 파일 경로");
                ui.add(egui::TextEdit::singleline(&mut self.observation_path)
                    .hint_text("예: C:\\tracker\\pity_history.csv").desired_width(460.0));
                if ui.button("로컬 CSV 분석").clicked() {
                    if let Some(ref schedule) = candidate {
                        let load = (|| -> Result<ObservationDataset, String> {
                            let contents = security::read_csv_secure(
                                std::path::Path::new(&self.observation_path),
                                Some(&self.observation_expected_sha256),
                            )?;
                            parse_observation_csv(&contents, schedule.hard)
                        })();
                        match load {
                            Ok(data) => {
                                self.observation_message = format!(
                                    "가져오기 성공 · {}개 관측 · {}개 획득 · {}개 검열",
                                    data.observations, data.events, data.censored);
                                self.observation_data = Some(data);
                            }
                            Err(error) => {
                                self.observation_data = None;
                                self.observation_message = error;
                            }
                        }
                    } else {
                        self.observation_data = None;
                        self.observation_message = "이 배너는 단순 최고 등급 대기시간 모델이 아니어서 직접 비교를 지원하지 않습니다.".into();
                    }
                }
                if ui.button("데이터 지우기").clicked() {
                    self.observation_data = None;
                    self.observation_message.clear();
                }
            });
            ui.label(RichText::new(
                "지원 CSV: duration,event (개별: 획득=1/중단=0), pity,at_risk,events,censored (위험집합), pity,count (완료 기록만)"
            ).color(MUTED).size(11.0));
            ui.small("한 배너, 천장 0부터 시작한 완전한 구간만 사용해야 합니다. 기록 앞부분이 잘렸다면 해당 구간을 버리세요.");
            if !self.observation_message.is_empty() {
                ui.label(RichText::new(&self.observation_message).color(PRIMARY));
            }
            let (Some(data), Some(model)) = (&self.observation_data, candidate) else {
                ui.colored_label(GOLD, "분석 데이터가 없거나 선택한 배너를 직접 비교할 수 없습니다.");
                return;
            };
            ui.separator();
            ui.label(RichText::new(data.quality.label()).strong());
            if !data.quality.is_censor_aware() {
                ui.colored_label(GOLD,
                    "경고: 성공 스택만 기록된 자료는 미완료 이력의 검열을 보정할 수 없습니다. 아래 비교는 기술 통계이며 모델의 적합성을 확정하지 않습니다.");
            } else {
                ui.label(RichText::new("독립적인 우측 검열 가정에 따른 Kaplan–Meier 추정, Greenwood 95% 구간").color(MINT));
            }
            let Ok(comparison) = data.compare(&model, self.mode) else {
                ui.colored_label(Color32::LIGHT_RED, "배너 모델과 관측 데이터가 호환되지 않습니다.");
                return;
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "관측 범위 1~{}스택 | 관측 {} | 5성 획득 {} | 검열 {}",
                    data.max_pity(), data.observations, data.events, data.censored));
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("제한 평균: 관측 {:.2}회 | 모델 {:.2}회",
                    comparison.restricted_observed_mean,
                    comparison.restricted_model_mean));
                ui.separator();
                if comparison.reliable_risk_bins > 0 {
                    ui.label(format!("최대 누적확률 편차 {:.2}%p | 위험률 평균 절대오차 {:.2}%p",
                        comparison.max_cdf_delta * 100.0,
                        comparison.mean_abs_hazard_delta * 100.0));
                    ui.small(format!("위험집합 20개 이상인 {}개 스택만 편차 계산",
                        comparison.reliable_risk_bins));
                } else {
                    ui.colored_label(GOLD, "위험집합이 20개 이상인 구간이 없어 정량 편차 판정을 보류합니다.");
                }
            });
            let mut model_survival = 1.0_f64;
            let mut observed_pts = vec![[0.0, 0.0]];
            let mut predicted_pts = vec![[0.0, 0.0]];
            let mut lower_pts = vec![[0.0, 0.0]];
            let mut upper_pts = vec![[0.0, 0.0]];
            let mut observed_hazard = Vec::new();
            let mut predicted_hazard = Vec::new();
            for row in &data.rows {
                let x = row.pity as f64;
                let chance = model.chance(row.pity, self.mode);
                model_survival *= 1.0 - chance;
                observed_pts.push([x, (1.0 - row.survival) * 100.0]);
                predicted_pts.push([x, (1.0 - model_survival) * 100.0]);
                if let Some((low, high)) = row.cdf_ci_95 {
                    lower_pts.push([x, low * 100.0]);
                    upper_pts.push([x, high * 100.0]);
                }
                if row.at_risk >= 20 {
                    observed_hazard.push([x, row.hazard * 100.0]);
                    predicted_hazard.push([x, chance * 100.0]);
                }
            }
            Plot::new("survival_observed_cdf").legend(Legend::default())
                .height(280.0).include_y(0.0).include_y(100.0)
                .include_x(0.0).include_x(model.hard as f64)
                .allow_zoom(true).allow_drag(true)
                .show(ui, |plot| {
                    plot.line(Line::new(PlotPoints::from_iter(observed_pts))
                        .name(if data.quality.is_censor_aware() {
                            "실측 누적 확률 · KM"
                        } else {
                            "완료 이력 분포 · 편향 가능"
                        }).color(MINT).width(3.0_f32));
                    plot.line(Line::new(PlotPoints::from_iter(predicted_pts))
                        .name("선택한 모델").color(PRIMARY).width(2.5_f32));
                    if data.quality.is_censor_aware() {
                        plot.line(Line::new(PlotPoints::from_iter(lower_pts))
                            .name("실측 95% 하한").color(GOLD).width(1.0_f32));
                        plot.line(Line::new(PlotPoints::from_iter(upper_pts))
                            .name("실측 95% 상한").color(GOLD).width(1.0_f32));
                    }
                });
            egui::CollapsingHeader::new("스택별 순간 5성 확률 비교 (위험집합 20건 이상)")
                .default_open(false).show(ui, |ui| {
                    Plot::new("survival_observed_hazard").legend(Legend::default())
                        .height(220.0).include_y(0.0).include_y(100.0)
                        .show(ui, |plot| {
                            plot.line(Line::new(PlotPoints::from_iter(observed_hazard))
                                .name("실측 위험률").color(MINT).width(2.0_f32));
                            plot.line(Line::new(PlotPoints::from_iter(predicted_hazard))
                                .name("모델 위험률").color(PRIMARY).width(2.0_f32));
                        });
                });
            ui.colored_label(GOLD, "주의: Greenwood 95% 구간은 최초 획득 이전(F=0)과 전원 획득 이후(F=1)의 경계에서 표시하지 않습니다. 독립적 기록 중단만 보정하며 임의 계정 업로드, 누락·혼합된 이력은 보정하지 않습니다.");
        });
    }
}

/// Build a monotonic, endpoint-complete CDF curve for both paint and tests.
/// The DP distribution remains the source of truth; sampling only reduces
/// screen-level line geometry, not numerical probabilities.
fn sampled_cdf(distribution: &Distribution, end: usize) -> Vec<(usize, f64)> {
    let end = end.max(1);
    let n = end.min(400);
    (0..=n).map(|i| {
        let x = i * end / n;
        (x, distribution.at(x).clamp(0.0, 1.0))
    }).collect()
}

