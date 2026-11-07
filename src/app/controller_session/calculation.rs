use super::*;

impl Calculator {
    /// Mark edited inputs stale without starting or scheduling a calculation.
    pub(crate) fn invalidate_calculation(&mut self) {
        self.input_needs_calculation = true;
        self.generation = self.generation.wrapping_add(1);
        self.active_generation.store(self.generation, Ordering::Relaxed);
        self.calculating = false;
        self.ci_ready_pending = None;
    }

    /// Only called for an explicit Run Simulation button request.
    pub(crate) fn recalculate(&mut self) {
        self.setup.set_targets(self.target_eid + 1, self.target_refine);
        self.generation = self.generation.wrapping_add(1);
        self.active_generation.store(self.generation, Ordering::Relaxed);
        self.calculating = true;
        self.ci_ready_pending = None;
        // Keep the last completed image on screen during background calculation.
        self.input_needs_calculation = false;
        self.error = None;
        let request = CalculationRequest {
            generation: self.generation,
            setup: self.setup.clone(),
            mode: self.mode,
            feedback_config: self.feedback_cfg.clone(),
            active_generation: Arc::clone(&self.active_generation),
        };
        if self.worker.send(request).is_err() {
            self.calculating = false;
            self.input_needs_calculation = true;
            self.error = Some("백그라운드 계산 작업을 시작할 수 없습니다.".to_owned());
            if self.ci_capture {
                eprintln!("GACHA_CI_SIM_FAILED game={:?} generation={}", self.setup.game, self.generation);
            }
        } else if self.ci_capture {
            eprintln!("GACHA_CI_SIM_STARTED game={:?} generation={}", self.setup.game, self.generation);
        }
    }

    pub(crate) fn poll_results(&mut self) {
        while let Ok(response) = self.results.try_recv() {
            // Ignore results from settings the user has since changed.
            if response.generation != self.generation {
                continue;
            }
            self.calculating = false;
            match response.result {
                Ok(stats) => {
                    let available = self.setup.available_flexible();
                    let raw = optimal_allocation(
                        &stats.character, &stats.weapon,
                        self.setup.character_dedicated, self.setup.weapon_dedicated, available,
                    );
                    let chance = stats.feedback.as_ref()
                        .map(|f| f.probability(available).0)
                        .unwrap_or(raw.success_probability);
                    self.history.insert(0, HistoryEntry {
                        game: self.setup.game, eid: self.target_eid,
                        refine: self.target_refine, chance,
                    });
                    self.history.truncate(5);
                    self.stats = Some(stats);
                    self.stats_game = Some(self.setup.game);
                    self.input_needs_calculation = false;
                    self.error = None;
                    if self.ci_capture { self.ci_ready_pending = Some(response.generation); }
                }
                Err(message) => {
                    self.input_needs_calculation = true;
                    self.error = Some(message);
                    if self.ci_capture {
                        eprintln!("GACHA_CI_SIM_FAILED game={:?} generation={}", self.setup.game, response.generation);
                    }
                }
            }
        }
    }


}
