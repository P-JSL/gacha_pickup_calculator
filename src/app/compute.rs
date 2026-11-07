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
    let character = request.setup.character.distribution(request.mode)?;
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    let weapon = request.setup.weapon.distribution(request.mode)?;
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    let combined = character.combine(&weapon);
    let cancellation = Some((&*request.active_generation, request.generation));
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
                        estimate_with_options(
                            &request.setup, request.mode, &variant, worker_limit, cancellation
                        ).map(Arc::new)
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
                strategies.push(Arc::new(estimate_with_options(
                    &request.setup, request.mode, &variant, cpu_budget, cancellation
                )?));
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
    let exact_shared_required = Some(CONFIDENCE_LEVELS.map(|confidence|
        raw_shared.quantile(confidence as f64 / 100.0)));
    let successful_budget_means = feedback.as_ref()
        .and_then(|estimate| estimate.successful_means(request.setup.available_flexible()));
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    Ok(Computed {
        character, weapon, combined, raw_shared, feedback, wuwa_strategies,
        successful_budget_means, exact_shared_required,
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
        assert_eq!(stats.raw_shared.at(0),
            optimal_allocation(&stats.character, &stats.weapon,
                request.setup.character_dedicated,
                request.setup.weapon_dedicated, 0).success_probability);
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
