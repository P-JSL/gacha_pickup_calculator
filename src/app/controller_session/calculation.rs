use super::*;

impl Calculator {
    /// Mark edited inputs stale without starting or scheduling a calculation.
    pub(crate) fn invalidate_calculation(&mut self) {
        self.input_needs_calculation = true;
        self.generation = self.generation.wrapping_add(1);
        self.active_generation.store(self.generation, Ordering::Relaxed);
        self.calculating = false;
        self.ci_ready_pending = None;
        session_log::info("INPUT_CHANGED", format!(
            "generation={} game={:?}; simulation invalidated, awaiting explicit Run",
            self.generation, self.setup.game
        ));
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
        session_log::info("CALC_REQUEST", format!(
            "generation={} game={:?} model={:?} target_character={} target_secondary={}              character_owned={} weapon_owned={} shared_tickets={} dedicated_character={}              dedicated_weapon={} currency={} character_pity={} weapon_pity={}              character_guaranteed={} weapon_guaranteed={} refunds_enabled={}              refund_samples={} exchange_points={} points_per_ticket={}",
            self.generation, self.setup.game, self.mode, self.target_eid + 1,
            self.target_refine, self.setup.character_owned, self.setup.weapon_owned,
            self.setup.available_flexible(), self.setup.character_dedicated,
            self.setup.weapon_dedicated, self.setup.premium_currency,
            self.setup.character.pity, self.setup.weapon.pity,
            self.setup.character.guaranteed, self.setup.weapon.guaranteed,
            self.feedback_cfg.enabled, self.feedback_cfg.samples,
            self.feedback_cfg.initial_points, self.feedback_cfg.points_per_ticket,
        ));
        // Capture the exact simulation assumptions once per run so a
        // differing outcome can be reproduced without storing RNG draws.
        session_log::info("CALC_ASSUMPTIONS", format!(
            "generation={} character_rule={:?} secondary_rule={:?} refund_config={:?}",
            self.generation, self.setup.character.rule,
            self.setup.weapon.rule, self.feedback_cfg
        ));
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
            session_log::error("CALC_DISPATCH_FAILED", format!(
                "generation={} worker channel disconnected", self.generation
            ));
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
                session_log::info("CALC_RESULT_DISCARDED", format!(
                    "outdated generation={} current_generation={}",
                    response.generation, self.generation
                ));
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
                    let returns = stats.feedback.as_ref().map_or(0.0, |f|
                        f.total_returned_points_average
                            / self.feedback_cfg.points_per_ticket.max(1) as f64);
                    session_log::info("CALC_RESULT", format!(
                        "generation={} game={:?} final_joint={:.4}% refund_included={}                          mean_raw_draws={:.2} mean_earned_refund_tickets={:.2}",
                        response.generation, self.setup.game, chance * 100.0,
                        stats.feedback.is_some(),
                        stats.feedback.as_ref().map_or(stats.combined.mean, |f| f.average_raw),
                        returns,
                    ));
                    if let Some(breakdown) = &stats.budget_breakdown {
                        session_log::info("CALC_RESULT_STAGES", format!(
                            "generation={} original={:.4}% refund_extra={:.4}% failed={:.4}%",
                            response.generation, breakdown.original_probability() * 100.0,
                            breakdown.recycling_probability() * 100.0,
                            breakdown.failure_probability() * 100.0,
                        ));
                    }
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
                    session_log::error("CALC_RESULT_ERROR", format!(
                        "generation={} error={message}", response.generation
                    ));
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
