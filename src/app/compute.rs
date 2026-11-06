struct Computed {
    character: Distribution,
    weapon: Distribution,
    combined: Distribution,
    feedback: Option<FeedbackEstimate>,
    wuwa_strategies: Option<Vec<FeedbackEstimate>>,
    exact_shared_required: Option<[usize; 6]>,
}

struct CalculationRequest {
    generation: u64,
    setup: GameSetup,
    mode: RateMode,
    feedback_config: FeedbackConfig,
}

struct CalculationResponse {
    generation: u64,
    result: Result<Computed, String>,
}

fn compute(request: &CalculationRequest) -> Result<Computed, String> {
    let character = request.setup.character.distribution(request.mode)?;
    let weapon = request.setup.weapon.distribution(request.mode)?;
    let combined = character.combine(&weapon);
    let (feedback, wuwa_strategies) = if request.feedback_config.enabled
        && request.setup.game == Game::WutheringWaves
    {
        let mut strategies = Vec::with_capacity(3);
        for max_wavebands in 0..=2 {
            let mut variant = request.feedback_config.clone();
            variant.wuwa_waveband_max = max_wavebands;
            strategies.push(estimate(&request.setup, request.mode, &variant)?);
        }
        let selected = strategies[request.feedback_config.wuwa_waveband_max.min(2)].clone();
        (Some(selected), Some(strategies))
    } else if request.feedback_config.enabled {
        (Some(estimate(&request.setup, request.mode, &request.feedback_config)?), None)
    } else {
        (None, None)
    };
    let exact_shared_required = if feedback.is_none() {
        Some(CONFIDENCE_LEVELS.map(|confidence| minimum_shared_pulls(
            &character, &weapon,
            request.setup.character_dedicated,
            request.setup.weapon_dedicated,
            confidence as f64 / 100.0,
        )))
    } else { None };
    Ok(Computed {
        character, weapon, combined, feedback, wuwa_strategies,
        exact_shared_required,
    })
}

