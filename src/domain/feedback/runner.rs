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

/// In-place MSD radix partition for integer (initial budget, raw draws).
/// Memory is O(256 * key_bytes) instead of allocating another N scenarios.
/// For small partitions, Rust's optimized comparison sort is faster.
fn sort_scenarios_by_funding(scenarios: &mut [RefundScenario]) {
    const INSERTION_THRESHOLD: usize = 128;
    const RADIX_THRESHOLD: usize = 16_384;
    if scenarios.len() < RADIX_THRESHOLD {
        scenarios.sort_unstable_by_key(|s| (s.initial_flexible_required, s.raw_pulls));
        return;
    }
    let mut max_budget = 0usize;
    let mut max_raw = 0usize;
    for trial in scenarios.iter() {
        max_budget = max_budget.max(trial.initial_flexible_required);
        max_raw = max_raw.max(trial.raw_pulls);
    }
    // Most gacha budgets fit in 2 bytes. Skip all zero leading key bytes.
    let mut bytes = Vec::with_capacity(2 * std::mem::size_of::<usize>());
    for (budget, maximum) in [(true, max_budget), (false, max_raw)] {
        let width = (usize::BITS - maximum.leading_zeros()).div_ceil(8) as usize;
        for index in (0..width).rev() {
            bytes.push((budget, index * 8));
        }
    }
    if bytes.is_empty() { return; }

    fn recurse(items: &mut [RefundScenario], bytes: &[(bool, usize)], depth: usize) {
        if depth >= bytes.len() || items.len() <= 1 { return; }
        if items.len() <= INSERTION_THRESHOLD {
            items.sort_unstable_by_key(|s| (s.initial_flexible_required, s.raw_pulls));
            return;
        }
        let (budget, shift) = bytes[depth];
        let digit = |item: &RefundScenario| -> usize {
            let value = if budget { item.initial_flexible_required } else { item.raw_pulls };
            (value >> shift) & 255
        };
        let mut counts = [0usize; 256];
        for item in items.iter() { counts[digit(item)] += 1; }
        let mut starts = [0usize; 256];
        let mut cumulative = 0usize;
        for (index, count) in counts.iter().enumerate() {
            starts[index] = cumulative;
            cumulative += count;
        }
        let mut cursors = starts;
        // American-flag distribution swaps each record directly into its
        // bucket. No per-trial clone, scratch vector, or double-buffer.
        for bucket in 0..256 {
            let end = starts[bucket] + counts[bucket];
            while cursors[bucket] < end {
                let destination = digit(&items[cursors[bucket]]);
                if destination == bucket {
                    cursors[bucket] += 1;
                } else {
                    items.swap(cursors[bucket], cursors[destination]);
                    cursors[destination] += 1;
                }
            }
        }
        if depth + 1 < bytes.len() {
            for bucket in 0..256 {
                let start = starts[bucket];
                let end = start + counts[bucket];
                if end > start + 1 {
                    recurse(&mut items[start..end], bytes, depth + 1);
                }
            }
        }
    }
    recurse(scenarios, &bytes, 0);
}

fn estimate_with_workers_cancel(setup: &GameSetup, mode: RateMode, model: &FeedbackConfig,
                                requested_workers: usize,
                                cancellation: Option<(&AtomicU64, u64)>)
                                -> Result<FeedbackEstimate, String> {
    if is_cancelled(cancellation) {
        return Err("계산 요청이 취소되었습니다.".to_owned());
    }
    // Opt-in stage profiling only. Never change RNG draw order, sorting, or
    // the public FeedbackEstimate representation to collect these timings.
    let profile = std::env::var_os("GACHA_PROFILE_MONTE_CARLO")
        .is_some_and(|value| value == "1");
    let total_started = std::time::Instant::now();
    let mut scenarios = Vec::new();
    scenarios.try_reserve_exact(model.samples).map_err(|_| {
        "시뮬레이션 결과 배열을 위한 메모리를 확보할 수 없습니다.".to_owned()
    })?;
    scenarios.resize_with(model.samples, RefundScenario::default);
    let allocation_ns = total_started.elapsed().as_nanos();
    let sample_started = std::time::Instant::now();
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
    let sample_ns = sample_started.elapsed().as_nanos();

    // The public v1.x contract exposes all raw trial observations. Construct
    // only those required arrays; avoid duplicate intermediate path storage.
    finalize_estimate(scenarios, totals, setup, model, EstimateProfile {
        workers, profile, allocation_ns, sample_ns, total_started,
    })
}



struct EstimateProfile {
    workers: usize,
    profile: bool,
    allocation_ns: u128,
    sample_ns: u128,
    total_started: std::time::Instant,
}

fn finalize_estimate(
    mut scenarios: Vec<RefundScenario>, totals: TrialTotals,
    setup: &GameSetup, model: &FeedbackConfig, timing: EstimateProfile,
) -> Result<FeedbackEstimate, String> {
    let EstimateProfile {
        workers, profile, allocation_ns, sample_ns, total_started,
    } = timing;
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
    let sorting_started = std::time::Instant::now();
    sort_scenarios_by_funding(&mut scenarios);
    let sorting_ns = sorting_started.elapsed().as_nanos();
    // Sorting scenarios by budget already orders the budget CDF. Avoid a
    // redundant second O(N log N) sort for large trial counts.
    for scenario in &scenarios {
        budgets.push(scenario.initial_flexible_required);
    }

    // Integer aggregation makes the public means bit-identical for every
    // worker count; floating-point division happens only once per metric.
    let n = model.samples as f64;
    if profile {
        let total_ns = total_started.elapsed().as_nanos();
        eprintln!(
            "GACHA_MC_PROFILE game={:?} samples={} workers={} alloc_ns={} sample_ns={} sort_ns={} remainder_ns={} total_ns={} retained_bytes={}",
            setup.game, model.samples, workers, allocation_ns, sample_ns, sorting_ns,
            total_ns.saturating_sub(allocation_ns + sample_ns + sorting_ns),
            total_ns, model.samples.saturating_mul(
                std::mem::size_of::<RefundScenario>()
                    + 2 * std::mem::size_of::<usize>()
            ),
        );
    }
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


/// Adaptive strategy scheduling: one bounded worker pool consumes interleaved
/// fixed-index trial chunks from all three waveband strategies. Idle workers
/// take the next unfinished chunk instead of being permanently reserved for
/// one strategy. RNG is indexed by trial, not worker/scheduling order.
struct StrategyTask<'a> {
    strategy: usize,
    first: usize,
    slots: &'a mut [RefundScenario],
}

pub fn estimate_waveband_strategies(
    setup: &GameSetup, mode: RateMode, config: &FeedbackConfig,
    worker_budget: usize, cancellation: Option<(&AtomicU64, u64)>,
) -> Result<Vec<FeedbackEstimate>, String> {
    if setup.game != Game::WutheringWaves {
        return Err("명조의 산호 교환 전략에만 공용 작업자 풀을 적용할 수 있습니다.".to_owned());
    }
    if is_cancelled(cancellation) {
        return Err("계산 요청이 취소되었습니다.".to_owned());
    }
    setup.character.validate_inputs()?;
    setup.weapon.validate_inputs()?;
    let models: [FeedbackConfig; 3] = std::array::from_fn(|index| {
        let mut model = config.clone();
        model.wuwa_waveband_max = index;
        model
    });
    for model in &models {
        model.validate()?;
        for (banner, other_rate) in [
            (&setup.character, model.character_other_five_star_rate),
            (&setup.weapon, model.weapon_other_five_star_rate),
        ] {
            if let BannerRule::FeaturedCounter { featured_rate, .. } = &banner.rule {
                if *featured_rate + other_rate > 1.0 + f64::EPSILON {
                    return Err("픽업·비픽업 최고 등급 기본 확률의 합은 100%를 넘을 수 없습니다.".into());
                }
            }
        }
    }
    let count = config.samples;
    let mut storage = Vec::with_capacity(3);
    for _ in 0..3 {
        let mut trials = Vec::new();
        trials.try_reserve_exact(count).map_err(|_| {
            "명조 환급 전략의 시뮬레이션 결과 메모리를 확보할 수 없습니다.".to_owned()
        })?;
        trials.resize_with(count, RefundScenario::default);
        storage.push(trials);
    }
    let available = std::thread::available_parallelism().map_or(1, |n| n.get());
    let workers = worker_budget.clamp(1, MAX_MONTE_CARLO_WORKERS)
        .min(available).min((count.saturating_mul(3) / MIN_SAMPLES_PER_WORKER).max(1));
    const CHUNK: usize = 4_096;
    let chunks_per_strategy = count.div_ceil(CHUNK);
    let started = std::time::Instant::now();

    let totals: [TrialTotals; 3] = std::thread::scope(|scope| {
        let (sender, receiver) = std::sync::mpsc::channel::<StrategyTask<'_>>();
        // Workers own Arc clones, so the receiver outlives every scoped
        // callback without borrowing a short-lived local Mutex.
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let receiver = std::sync::Arc::clone(&receiver);
            let models = &models;
            handles.push(scope.spawn(move || -> Result<[TrialTotals; 3], String> {
                let mut partial = std::array::from_fn(|_| TrialTotals::default());
                loop {
                    // Lock only for dequeue; Monte Carlo work executes unlocked.
                    let incoming = receiver.lock()
                        .unwrap_or_else(|poison| poison.into_inner()).recv();
                    let Ok(task) = incoming else { break; };
                    let Some(done) = simulate_chunk(
                        task.slots, task.first, setup, mode,
                        &models[task.strategy], cancellation,
                    ) else {
                        return Err("계산 요청이 취소되었습니다.".to_owned());
                    };
                    partial[task.strategy].merge(done);
                }
                Ok(partial)
            }));
        }

        // Round-robin chunks prevent long strategy zero from monopolizing
        // the queue. Any free CPU worker can claim any strategy's next chunk.
        let mut chunks = storage.iter_mut()
            .map(|trials| trials.chunks_mut(CHUNK)).collect::<Vec<_>>();
        for index in 0..chunks_per_strategy {
            for (strategy, queue) in chunks.iter_mut().enumerate() {
                if let Some(slots) = queue.next() {
                    sender.send(StrategyTask {
                        strategy, first: index * CHUNK, slots,
                    }).map_err(|_| "명조 공유 작업자 큐가 종료되었습니다.".to_owned())?;
                }
            }
        }
        drop(sender);
        let mut collected = std::array::from_fn(|_| TrialTotals::default());
        for handle in handles {
            let partial = handle.join()
                .map_err(|_| "명조 공용 시뮬레이션 작업자가 중단되었습니다.".to_owned())??;
            for (dest, src) in collected.iter_mut().zip(partial) {
                dest.merge(src);
            }
        }
        Ok::<_, String>(collected)
    })?;

    if is_cancelled(cancellation) {
        return Err("계산 요청이 취소되었습니다.".to_owned());
    }
    let sampling_ms = started.elapsed().as_millis();
    let mut estimates = Vec::with_capacity(3);
    for (strategy, (trials, totals)) in storage.into_iter().zip(totals).enumerate() {
        estimates.push(finalize_estimate(
            trials, totals, setup, &models[strategy], EstimateProfile {
                workers, profile: false, allocation_ns: 0, sample_ns: 0,
                total_started: std::time::Instant::now(),
            },
        )?);
    }
    let elapsed = started.elapsed().as_millis();
    eprintln!(
        "GACHA_WUWA_SHARED_POOL samples={} workers={} chunks_per_strategy={} sampling_ms={} total_ms={}",
        count, workers, chunks_per_strategy, sampling_ms, elapsed,
    );
    Ok(estimates)
}

#[cfg(test)]
mod radix_sort_regression {
    use super::*;

    #[test]
    fn integer_radix_preserves_entire_scenarios_and_lexicographic_budget_order() {
        for size in [0usize, 1, 127, 128, 255, 16_383, 16_384, 100_000] {
            let mut input = (0..size).map(|i| RefundScenario {
                raw_pulls: i.wrapping_mul(47) % 4_800,
                initial_flexible_required: i.wrapping_mul(41) % 3_000,
                earned_points: i,
                ..RefundScenario::default()
            }).collect::<Vec<_>>();
            let mut reference = input.clone();
            reference.sort_unstable_by_key(|s| (s.initial_flexible_required, s.raw_pulls));
            sort_scenarios_by_funding(&mut input);
            let keys = |items: &[RefundScenario]| items.iter()
                .map(|s| (s.initial_flexible_required, s.raw_pulls))
                .collect::<Vec<_>>();
            assert_eq!(keys(&input), keys(&reference), "size={size}");
            let mut original_ids = input.iter().map(|s| s.earned_points).collect::<Vec<_>>();
            original_ids.sort_unstable();
            assert_eq!(original_ids, (0..size).collect::<Vec<_>>());
        }
    }

    #[test]
    fn radix_handles_wide_usize_keys_and_many_equal_pairs() {
        let mut trials = (0..50_000).map(|i| RefundScenario {
            raw_pulls: (i % 17) + (usize::MAX - 8_000),
            initial_flexible_required: if i % 3 == 0 { usize::MAX - 1 } else { i % 3 },
            earned_points: i,
            ..RefundScenario::default()
        }).collect::<Vec<_>>();
        let mut reference = trials.clone();
        reference.sort_unstable_by_key(|s| (s.initial_flexible_required, s.raw_pulls));
        sort_scenarios_by_funding(&mut trials);
        assert_eq!(trials.iter().map(|s| (s.initial_flexible_required, s.raw_pulls))
            .collect::<Vec<_>>(),
            reference.iter().map(|s| (s.initial_flexible_required, s.raw_pulls))
                .collect::<Vec<_>>());
    }
}
