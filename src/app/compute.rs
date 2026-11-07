use super::*;

pub(crate) struct Computed {
    pub(crate) character: Distribution,
    pub(crate) weapon: Distribution,
    pub(crate) combined: Distribution,
    pub(crate) feedback: Option<Arc<FeedbackEstimate>>,
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
    let cancelled = || request.active_generation.load(Ordering::Relaxed) != request.generation;
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    let character = request.setup.character.distribution(request.mode)?;
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    let weapon = request.setup.weapon.distribution(request.mode)?;
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    let combined = character.combine(&weapon);
    let cpu_budget = std::thread::available_parallelism()
        .map_or(1, |count| count.get()).min(8);
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
                        crate::feedback::estimate_with_options(
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
                strategies.push(Arc::new(crate::feedback::estimate_with_options(
                    &request.setup, request.mode, &variant, cpu_budget, cancellation
                )?));
            }
            strategies
        };
        // Share the selected observation set: do not clone up to 1M paths.
        let selected = Arc::clone(&strategies[request.feedback_config.wuwa_waveband_max.min(2)]);
        (Some(selected), Some(strategies))
    } else if request.feedback_config.enabled {
        (Some(Arc::new(crate::feedback::estimate_with_options(
            &request.setup, request.mode, &request.feedback_config, cpu_budget, cancellation
        )?)), None)
    } else {
        (None, None)
    };
    if cancelled() { return Err("계산 요청이 취소되었습니다.".to_owned()); }
    let exact_shared_required = if feedback.is_none() {
        // Construct the exact residual-ticket distribution once, then reuse
        // its monotone CDF for all six O(log n) quantile lookups.
        let demand = if request.setup.character_dedicated == 0
            && request.setup.weapon_dedicated == 0
        {
            // Most new sessions have no dedicated tickets. The already-built
            // combined PMF is exactly the residual demand, so reuse it.
            combined.clone()
        } else {
            remaining_shared_distribution(
                &character, &weapon, request.setup.character_dedicated,
                request.setup.weapon_dedicated,
            )
        };
        Some(CONFIDENCE_LEVELS.map(|confidence|
            demand.quantile(confidence as f64 / 100.0)))
    } else { None };
    Ok(Computed {
        character, weapon, combined, feedback, wuwa_strategies,
        exact_shared_required,
    })
}



#[cfg(test)]
mod parallel_result_tests {
    use super::*;

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
}
