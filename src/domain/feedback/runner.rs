use super::*;

/// Deterministic for a fixed seed, but stochastic as an estimator.
/// Pull order: character banner to goal, then weapon banner to goal.
/// A common store balance is shared throughout both phases.
pub fn estimate(setup: &GameSetup, mode: RateMode,
                model: &FeedbackConfig) -> Result<FeedbackEstimate, String> {
    estimate_with_options(setup, mode, model, MAX_MONTE_CARLO_WORKERS, None)
}

/// Bound workers assigned to one estimate and optionally cancel obsolete GUI
/// requests. The original estimate() API retains its deterministic behavior.
pub fn estimate_with_options(setup: &GameSetup, mode: RateMode,
                             model: &FeedbackConfig, worker_limit: usize,
                             cancellation: Option<(&AtomicU64, u64)>)
                             -> Result<FeedbackEstimate, String> {
    model.validate()?;
    if is_cancelled(cancellation) {
        return Err("계산 요청이 취소되었습니다.".to_owned());
    }
    // This estimator is public and can be called without the UI's exact
    // DP. Validate the same input bounds without redundantly constructing two
    // full distributions (or doing so three times for WuWa strategies).
    setup.character.validate_inputs()?;
    setup.weapon.validate_inputs()?;
    for (banner, extra) in [
        (&setup.character, model.character_other_five_star_rate),
        (&setup.weapon, model.weapon_other_five_star_rate),
    ] {
        if let BannerRule::FeaturedCounter { featured_rate, .. } = &banner.rule {
            if *featured_rate + extra > 1.0 + f64::EPSILON {
                return Err("픽업·비픽업 최고 등급 기본 확률의 합은 100%를 넘을 수 없습니다.".into());
            }
        }
    }
    let available = std::thread::available_parallelism().map_or(1, |count| count.get());
    let workers = available.min(MAX_MONTE_CARLO_WORKERS).min(worker_limit.max(1))
        .min((model.samples / MIN_SAMPLES_PER_WORKER).max(1));
    estimate_with_workers_cancel(setup, mode, model, workers, cancellation)
}

fn is_cancelled(cancellation: Option<(&AtomicU64, u64)>) -> bool {
    cancellation.is_some_and(|(active, generation)|
        active.load(Ordering::Relaxed) != generation)
}

/// The seed depends on the trial index, not on how workers are scheduled.
/// A one-worker run and an N-worker run therefore generate the same paths.
fn sample_seed(index: usize) -> u64 {
    let mut seed = SplitMix64(0xF17E_1234_ABCD_0905
        ^ (index as u64).wrapping_mul(0xD134_2543_DE82_EF95));
    seed.next()
}

#[derive(Default)]
struct TrialTotals {
    raw: u128,
    flexible: u128,
    returned: u128,
    wavebands: u128,
    waveband_cost: u128,
    character_raw: u128,
    weapon_raw: u128,
    character_points: u128,
    weapon_points: u128,
    board_dice: u128,
    record_discs: u128,
}
impl TrialTotals {
    fn merge(&mut self, other: Self) {
        self.raw += other.raw;
        self.flexible += other.flexible;
        self.returned += other.returned;
        self.wavebands += other.wavebands;
        self.waveband_cost += other.waveband_cost;
        self.character_raw += other.character_raw;
        self.weapon_raw += other.weapon_raw;
        self.character_points += other.character_points;
        self.weapon_points += other.weapon_points;
        self.board_dice += other.board_dice;
        self.record_discs += other.record_discs;
    }
}

fn simulate_chunk(slots: &mut [RefundScenario], start_index: usize, setup: &GameSetup,
                  mode: RateMode, model: &FeedbackConfig,
                  cancellation: Option<(&AtomicU64, u64)>) -> Option<TrialTotals> {
    let mut totals = TrialTotals::default();
    for (offset, slot) in slots.iter_mut().enumerate() {
        // One atomic check per 256 paths bounds cancellation overhead.
        if offset.is_multiple_of(256) && is_cancelled(cancellation) { return None; }
        let mut rng = SplitMix64(sample_seed(start_index + offset));
        let mut wallet = Wallet::new(model);
        let mut dedicated_character = setup.character_dedicated;
        let mut dedicated_weapon = setup.weapon_dedicated;
        for (kind, banner, owned, dedicated) in [
            (BannerKind::Character, &setup.character, setup.character_owned, &mut dedicated_character),
            (BannerKind::Weapon, &setup.weapon, setup.weapon_owned, &mut dedicated_weapon),
        ] {
            // Record-only credits can pay only for the weapon/record phase.
            if matches!(kind, BannerKind::Weapon) && setup.game == Game::StellaSora {
                let disc_units = wallet.record_disc_units
                    .saturating_add(setup.weapon_ticket_remainder);
                *dedicated = dedicated.saturating_add(
                    disc_units / setup.game.weapon_ticket_units_per_pull());
            }
            let raw_before = wallet.raw_pulls;
            let points_before = wallet.points;
            match &banner.rule {
                BannerRule::Standard { .. } => {
                    let max_wavebands = if setup.game == Game::WutheringWaves
                        && matches!(kind, BannerKind::Character) {
                        model.wuwa_waveband_max
                    } else { 0 };
                    roll_standard(&mut rng, banner, (kind, setup.game), mode, model, &mut wallet,
                        (owned, dedicated, max_wavebands));
                }
                BannerRule::FeaturedCounter { .. } => {
                    roll_featured(&mut rng, banner, kind, model, &mut wallet, owned, dedicated);
                }
                BannerRule::NteArcBatch => {
                    roll_arc(&mut rng, banner, model, &mut wallet, owned, dedicated);
                }
            }
            let draws = (wallet.raw_pulls - raw_before) as u128;
            let points = (wallet.points - points_before) as u128;
            match kind {
                BannerKind::Character => {
                    totals.character_raw += draws;
                    totals.character_points += points;
                    wallet.reward_multiplier = 1;
                }
                BannerKind::Weapon => {
                    totals.weapon_raw += draws;
                    totals.weapon_points += points;
                }
            }
        }
        totals.board_dice += wallet.board_dice_earned as u128;
        totals.record_discs += wallet.record_disc_units as u128;
        totals.raw += wallet.raw_pulls as u128;
        totals.flexible += wallet.initial_flexible_required as u128;
        totals.returned += (wallet.points - model.initial_points) as u128;
        totals.wavebands += wallet.wavebands_bought as u128;
        totals.waveband_cost += wallet.waveband_points_spent as u128;
        *slot = RefundScenario::from_wallet(&wallet, model.initial_points);
    }
    if is_cancelled(cancellation) { None } else { Some(totals) }
}

/// A single preallocated scenario array is partitioned into non-overlapping
/// mutable slices. Workers only return integer aggregates: no result cloning,
/// locks, per-worker result arrays or scheduling-dependent floating sums.
#[cfg(test)]
pub(super) fn estimate_with_workers(setup: &GameSetup, mode: RateMode, model: &FeedbackConfig,
                         requested_workers: usize) -> Result<FeedbackEstimate, String> {
    estimate_with_workers_cancel(setup, mode, model, requested_workers, None)
}

fn estimate_with_workers_cancel(setup: &GameSetup, mode: RateMode, model: &FeedbackConfig,
                                requested_workers: usize,
                                cancellation: Option<(&AtomicU64, u64)>)
                                -> Result<FeedbackEstimate, String> {
    if is_cancelled(cancellation) {
        return Err("계산 요청이 취소되었습니다.".to_owned());
    }
    let mut scenarios = Vec::new();
    scenarios.try_reserve_exact(model.samples).map_err(|_| {
        "시뮬레이션 결과 배열을 위한 메모리를 확보할 수 없습니다.".to_owned()
    })?;
    scenarios.resize_with(model.samples, RefundScenario::default);
    let workers = requested_workers.max(1).min(model.samples);
    let chunk_size = model.samples.div_ceil(workers);
    let partials = if workers == 1 {
        vec![simulate_chunk(&mut scenarios, 0, setup, mode, model, cancellation)]
    } else {
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for (chunk_index, slots) in scenarios.chunks_mut(chunk_size).enumerate() {
                handles.push(scope.spawn(move || {
                    simulate_chunk(slots, chunk_index * chunk_size, setup, mode, model, cancellation)
                }));
            }
            handles.into_iter().map(|handle| {
                handle.join().map_err(|_| "병렬 시뮬레이션 작업이 중단되었습니다.".to_owned())
            }).collect::<Result<Vec<Option<TrialTotals>>, String>>()
        })?
    };
    let mut totals = TrialTotals::default();
    for part in partials {
        totals.merge(part.ok_or_else(|| "계산 요청이 취소되었습니다.".to_owned())?);
    }
    if is_cancelled(cancellation) {
        return Err("계산 요청이 취소되었습니다.".to_owned());
    }

    // The public v1.x contract exposes all raw trial observations. Construct
    // only those required arrays; avoid duplicate intermediate path storage.
    let mut budgets = Vec::new();
    let mut raw_pulls = Vec::new();
    budgets.try_reserve_exact(model.samples).map_err(|_| {
        "시뮬레이션 결과 배열을 위한 메모리를 확보할 수 없습니다.".to_owned()
    })?;
    raw_pulls.try_reserve_exact(model.samples).map_err(|_| {
        "시뮬레이션 결과 배열을 위한 메모리를 확보할 수 없습니다.".to_owned()
    })?;
    // Retain the original per-trial raw-pull order before sorting scenarios.
    for scenario in &scenarios {
        raw_pulls.push(scenario.raw_pulls);
    }
    scenarios.sort_unstable_by_key(|scenario|
        (scenario.initial_flexible_required, scenario.raw_pulls));
    // Sorting scenarios by budget already orders the budget CDF. Avoid a
    // redundant second O(N log N) sort for large trial counts.
    for scenario in &scenarios {
        budgets.push(scenario.initial_flexible_required);
    }

    // Integer aggregation makes the public means bit-identical for every
    // worker count; floating-point division happens only once per metric.
    let n = model.samples as f64;
    Ok(FeedbackEstimate { budgets, raw_pulls, scenarios,
        average_raw: totals.raw as f64 / n,
        average_initial_flexible: totals.flexible as f64 / n,
        total_returned_points_average: totals.returned as f64 / n,
        average_wavebands_purchased: totals.wavebands as f64 / n,
        average_waveband_corals_spent: totals.waveband_cost as f64 / n,
        average_character_raw: totals.character_raw as f64 / n,
        average_weapon_raw: totals.weapon_raw as f64 / n,
        average_character_return_points: totals.character_points as f64 / n,
        average_weapon_return_points: totals.weapon_points as f64 / n,
        average_board_dice_earned: totals.board_dice as f64 / n,
        average_record_disc_units_earned: totals.record_discs as f64 / n })
}

