use super::*;

pub(crate) struct Computed {
    pub(crate) character: Distribution,
    pub(crate) weapon: Distribution,
    pub(crate) combined: Distribution,
    pub(crate) feedback: Option<FeedbackEstimate>,
    pub(crate) wuwa_strategies: Option<Vec<FeedbackEstimate>>,
    pub(crate) exact_shared_required: Option<[usize; 6]>,
}

pub(crate) struct CalculationRequest {
    pub(crate) generation: u64,
    pub(crate) setup: GameSetup,
    pub(crate) mode: RateMode,
    pub(crate) feedback_config: FeedbackConfig,
}

pub(crate) struct CalculationResponse {
    pub(crate) generation: u64,
    pub(crate) result: Result<Computed, String>,
}

pub(crate) fn compute(request: &CalculationRequest) -> Result<Computed, String> {
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
        // Construct the exact residual-ticket distribution once, then reuse
        // its monotone CDF for all six O(log n) quantile lookups.
        let demand = remaining_shared_distribution(
            &character, &weapon, request.setup.character_dedicated,
            request.setup.weapon_dedicated,
        );
        Some(CONFIDENCE_LEVELS.map(|confidence|
            demand.quantile(confidence as f64 / 100.0)))
    } else { None };
    Ok(Computed {
        character, weapon, combined, feedback, wuwa_strategies,
        exact_shared_required,
    })
}

