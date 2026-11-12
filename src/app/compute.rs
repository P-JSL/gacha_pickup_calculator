use super::*;

pub(crate) struct Computed {
    pub(crate) character: Distribution,
    pub(crate) weapon: Distribution,
    pub(crate) combined: Distribution,
    /// Exact CDF of additional *shared* tickets after dedicated inventory.
    /// Same X-axis units as the refund Monte Carlo CDF.
    pub(crate) raw_shared: Distribution,
    pub(crate) feedback: Option<Arc<FeedbackEstimate>>,
    /// Prepared on the worker so the GUI never scans up to 1M paths per frame.
    pub(crate) successful_budget_means: Option<SuccessfulBudgetMeans>,
    /// Computed once in worker, not across 1M paths on every frame.
    pub(crate) budget_breakdown: Option<BudgetSuccessBreakdown>,
    pub(crate) wuwa_strategies: Option<Vec<Arc<FeedbackEstimate>>>,
    pub(crate) exact_shared_required: Option<[usize; 6]>,
}

pub(crate) struct CalculationRequest {
    pub(crate) generation: u64,
    pub(crate) setup: GameSetup,
    pub(crate) mode: RateMode,
    pub(crate) feedback_config: FeedbackConfig,
    pub(crate) active_generation: Arc<AtomicU64>,
}

pub(crate) struct CalculationResponse {
    pub(crate) generation: u64,
    pub(crate) result: Result<Computed, String>,
}

pub(crate) fn compute(request: &CalculationRequest) -> Result<Computed, String> {
    let budget = std::thread::available_parallelism()
        .map_or(1, |count| count.get()).min(8);
    compute_with_cpu_budget(request, budget)
}

fn compute_with_cpu_budget(request: &CalculationRequest, budget: usize)
                           -> Result<Computed, String> {
    let cpu_budget = budget.clamp(1, 8);
    let cancelled = || request.active_generation.load(Ordering::Relaxed) != request.generation;
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    session_log::info("CALC_CHARACTER_DP", format!(
        "generation={} calculating character probability distribution", request.generation
    ));
    let character = request.setup.character.distribution(request.mode)?;
    session_log::info("CALC_CHARACTER_DP_DONE", format!(
        "generation={} mean_raw={:.3} support={}",
        request.generation, character.mean, character.pmf.len()
    ));
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    session_log::info("CALC_WEAPON_DP", format!(
        "generation={} calculating secondary probability distribution", request.generation
    ));
    let weapon = request.setup.weapon.distribution(request.mode)?;
    session_log::info("CALC_WEAPON_DP_DONE", format!(
        "generation={} mean_raw={:.3} support={}",
        request.generation, weapon.mean, weapon.pmf.len()
    ));
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    session_log::info("CALC_COMBINE", format!(
        "generation={} composing joint distribution", request.generation
    ));
    let combined = character.combine(&weapon);
    let cancellation = Some((&*request.active_generation, request.generation));
    if request.feedback_config.enabled {
        session_log::info("CALC_REFUND_START", format!(
            "generation={} samples={} cpu_workers={} game={:?}; iterative refund recycling",
            request.generation, request.feedback_config.samples,
            cpu_budget, request.setup.game
        ));
    } else {
        session_log::info("CALC_REFUND_DISABLED", format!(
            "generation={} exact DP without recycling", request.generation
        ));
    }
    let (feedback, wuwa_strategies) = if request.feedback_config.enabled
        && request.setup.game == Game::WutheringWaves
    {
        // Keep the total worker budget bounded across three independent
        // waveband strategies; nested unbounded pools oversubscribe CPUs.
        let strategies = if cpu_budget >= 3 && request.feedback_config.samples >= 4_096 {
            std::thread::scope(|scope| {
                let mut handles = Vec::with_capacity(3);
                for max_wavebands in 0..3 {
                    let mut variant = request.feedback_config.clone();
                    variant.wuwa_waveband_max = max_wavebands;
                    let worker_limit = cpu_budget / 3 + usize::from(max_wavebands < cpu_budget % 3);
                    handles.push(scope.spawn(move || {
                        let started = std::time::Instant::now();
                        let result = estimate_with_options(
                            &request.setup, request.mode, &variant, worker_limit, cancellation
                        ).map(Arc::new);
                        session_log::info("WUWA_STRATEGY_TIMING", format!(
                            "generation={} wavebands={} workers={} samples={} elapsed_ms={} outcome={}",
                            request.generation, max_wavebands, worker_limit, variant.samples,
                            started.elapsed().as_millis(),
                            if result.is_ok() { "ok" } else { "cancelled_or_failed" }
                        ));
                        result
                    }));
                }
                handles.into_iter().map(|handle| {
                    handle.join()
                        .map_err(|_| "명조 환급 전략 작업이 중단되었습니다.".to_owned())?
                }).collect::<Result<Vec<Arc<FeedbackEstimate>>, String>>()
            })?
        } else {
            let mut strategies = Vec::with_capacity(3);
            for max_wavebands in 0..3 {
                let mut variant = request.feedback_config.clone();
                variant.wuwa_waveband_max = max_wavebands;
                let started = std::time::Instant::now();
                let result = estimate_with_options(
                    &request.setup, request.mode, &variant, cpu_budget, cancellation
                );
                session_log::info("WUWA_STRATEGY_TIMING", format!(
                    "generation={} wavebands={} workers={} samples={} elapsed_ms={} outcome={}",
                    request.generation, max_wavebands, cpu_budget, variant.samples,
                    started.elapsed().as_millis(),
                    if result.is_ok() { "ok" } else { "cancelled_or_failed" }
                ));
                strategies.push(Arc::new(result?));
            }
            strategies
        };
        // Share the selected observation set: do not clone up to 1M paths.
        let selected = Arc::clone(&strategies[request.feedback_config.wuwa_waveband_max.min(2)]);
        (Some(selected), Some(strategies))
    } else if request.feedback_config.enabled {
        (Some(Arc::new(estimate_with_options(
            &request.setup, request.mode, &request.feedback_config, cpu_budget, cancellation
        )?)), None)
    } else {
        (None, None)
    };
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    if let Some(refund) = &feedback {
        session_log::info("CALC_REFUND_DONE", format!(
            "generation={} scenarios={} avg_raw={:.3} avg_initial_shared={:.3}              avg_earned_points={:.3} strategies={}",
            request.generation, refund.scenarios.len(), refund.average_raw,
            refund.average_initial_flexible, refund.total_returned_points_average,
            wuwa_strategies.as_ref().map_or(1, Vec::len)
        ));
    }
    // Prepare the refund-free reference distribution even when recycling is
    // enabled. Both exact and simulated curves now use *initial shared passes*
    // as their X-axis; independently adding banner quantiles is not valid.
    let raw_shared = if request.setup.character_dedicated == 0
        && request.setup.weapon_dedicated == 0 {
        combined.clone()
    } else {
        remaining_shared_distribution(
            &character, &weapon, request.setup.character_dedicated,
            request.setup.weapon_dedicated,
        )
    };
    session_log::info("CALC_SHARED_CDF", format!(
        "generation={} shared_support={} available_shared={}",
        request.generation, raw_shared.pmf.len(), request.setup.available_flexible()
    ));
    let exact_shared_required = Some(CONFIDENCE_LEVELS.map(|confidence|
        raw_shared.quantile(confidence as f64 / 100.0)));
    let successful_budget_means = feedback.as_ref()
        .and_then(|estimate| estimate.successful_means(request.setup.available_flexible()));
    // Only Star Rail has the simple original-ticket then Starlight-funded
    // continuation represented by the three-stage UI. Other games may have
    // independent record credits or waveband exchanges.
    let budget_breakdown = if request.setup.game == Game::StarRail {
        feedback.as_ref().map(|estimate|
            estimate.success_breakdown(request.setup.available_flexible(),
                request.feedback_config.points_per_ticket))
    } else { None };
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    Ok(Computed {
        character, weapon, combined, raw_shared, feedback, wuwa_strategies,
        successful_budget_means, budget_breakdown, exact_shared_required,
    })
}



#[cfg(test)]
mod parallel_result_tests {
    use super::*;

    #[test]
    fn refund_enabled_worker_retains_an_exact_shared_ticket_baseline() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.set_targets(1, 1);
        setup.character_dedicated = 3;
        setup.weapon_dedicated = 2;
        let mut config = FeedbackConfig::for_game(Game::StarRail);
        config.samples = 128;
        let request = CalculationRequest {
            generation: 3, setup, mode: RateMode::PublishedOnly,
            feedback_config: config,
            active_generation: Arc::new(AtomicU64::new(3)),
        };
        let stats = compute_with_cpu_budget(&request, 1).unwrap();
        assert!(stats.feedback.is_some());
        let thresholds = stats.exact_shared_required.unwrap();
        for (i, target) in CONFIDENCE_LEVELS.iter().enumerate() {
            assert_eq!(thresholds[i],
                stats.raw_shared.quantile(*target as f64 / 100.0));
        }
        assert!((stats.raw_shared.at(0)
            - optimal_allocation(&stats.character, &stats.weapon,
                request.setup.character_dedicated,
                request.setup.weapon_dedicated, 0).success_probability).abs() < 1e-10);
    }

    #[test]
    fn chosen_wuwa_strategy_shares_trial_storage_without_cloning() {
        let mut setup = GameSetup::new(Game::WutheringWaves);
        setup.set_targets(1, 1);
        let mut config = FeedbackConfig::for_game(Game::WutheringWaves);
        config.samples = 64;
        config.wuwa_waveband_max = 1;
        let result = compute(&CalculationRequest {
            generation: 1, setup, mode: RateMode::PublishedOnly, feedback_config: config,
            active_generation: Arc::new(AtomicU64::new(1)),
        }).unwrap();
        let strategies = result.wuwa_strategies.as_ref().unwrap();
        assert_eq!(strategies.len(), 3);
        assert!(Arc::ptr_eq(result.feedback.as_ref().unwrap(), &strategies[1]));
    }

    #[test]
    fn wuwa_strategies_match_bitwise_with_serial_and_shared_cpu_budget() {
        let mut setup = GameSetup::new(Game::WutheringWaves);
        setup.set_targets(1, 1);
        let mut config = FeedbackConfig::for_game(Game::WutheringWaves);
        config.samples = 8_192;
        config.wuwa_waveband_max = 2;
        let request = CalculationRequest {
            generation: 7, setup, mode: RateMode::PublishedOnly,
            feedback_config: config, active_generation: Arc::new(AtomicU64::new(7)),
        };
        let serial = compute_with_cpu_budget(&request, 1).unwrap();
        let parallel = compute_with_cpu_budget(&request, 8).unwrap();
        for (left, right) in serial.wuwa_strategies.unwrap().iter()
            .zip(parallel.wuwa_strategies.unwrap().iter()) {
            assert_eq!(left.budgets, right.budgets);
            assert_eq!(left.raw_pulls, right.raw_pulls);
            assert_eq!(left.scenarios, right.scenarios);
            assert_eq!(left.average_raw.to_bits(), right.average_raw.to_bits());
            assert_eq!(left.total_returned_points_average.to_bits(),
                       right.total_returned_points_average.to_bits());
        }
    }

    #[test]
    fn rapid_recalculation_generations_reject_obsolete_requests_without_changing_current_result() {
        // G-02: Run A, edit/cancel it, Run B, cancel again, then Run C.
        // Older workers may still have a response in the channel, but only
        // the active generation is eligible to produce current results.
        let mut setup = GameSetup::new(Game::StarRail);
        setup.set_targets(1, 1);
        let mut config = FeedbackConfig::for_game(Game::StarRail);
        config.samples = 64;
        let active = Arc::new(AtomicU64::new(1));
        let make = |generation| CalculationRequest {
            generation,
            setup: setup.clone(),
            mode: RateMode::PublishedOnly,
            feedback_config: config.clone(),
            active_generation: Arc::clone(&active),
        };

        let first = make(1);
        let baseline = compute_with_cpu_budget(&first, 1).unwrap();
        active.store(2, Ordering::Relaxed);
        assert!(compute_with_cpu_budget(&first, 1).is_err());

        let cancelled = make(2);
        active.store(3, Ordering::Relaxed);
        assert!(compute_with_cpu_budget(&cancelled, 1).is_err());

        let current = make(3);
        let latest = compute_with_cpu_budget(&current, 1).unwrap();
        assert_eq!(baseline.raw_shared.pmf, latest.raw_shared.pmf);
        assert_eq!(baseline.feedback.as_ref().unwrap().scenarios,
                   latest.feedback.as_ref().unwrap().scenarios);

        // Another edit immediately invalidates a finished response, but
        // never changes the saved values of the next successful generation.
        active.store(4, Ordering::Relaxed);
        assert!(compute_with_cpu_budget(&current, 1).is_err());
        let resumed = compute_with_cpu_budget(&make(4), 1).unwrap();
        assert_eq!(resumed.feedback.as_ref().unwrap().budgets,
                   latest.feedback.as_ref().unwrap().budgets);
    }

    #[test]
    fn obsolete_generation_skips_computation_before_allocating_scenarios() {
        let mut setup = GameSetup::new(Game::WutheringWaves);
        setup.set_targets(1, 1);
        let mut config = FeedbackConfig::for_game(Game::WutheringWaves);
        config.samples = 8_192;
        let request = CalculationRequest {
            generation: 2, setup, mode: RateMode::PublishedOnly,
            feedback_config: config, active_generation: Arc::new(AtomicU64::new(3)),
        };
        assert!(compute_with_cpu_budget(&request, 8).is_err());
    }
}
