//! Lightweight benchmarks: numerical contract always checked; timings only
//! when explicitly requested by CI with --ignored --nocapture --release.
use gacha_pickup_calculator::Distribution;
use std::hint::black_box;
use std::time::Instant;

fn pmf(len: usize, factor: usize) -> Vec<f64> {
    let mut values: Vec<f64> = (0..len)
        .map(|i| ((i * factor + 17) % 53 + 1) as f64)
        .collect();
    let total: f64 = values.iter().sum();
    for v in &mut values { *v /= total; }
    values
}
fn direct_convolution(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            out[i+j] += x * y;
        }
    }
    out
}
#[test]
fn direct_and_fft_convolution_agree_for_sparse_and_dense_inputs() {
    for (n, m) in [(1, 1), (32, 24), (256, 192), (768, 512)] {
        let a = pmf(n, 13);
        let b = pmf(m, 23);
        let got = Distribution::new(a.clone()).combine(&Distribution::new(b.clone()));
        let reference = direct_convolution(&a, &b);
        assert_eq!(got.pmf.len(), reference.len());
        for (i, (&x, &y)) in got.pmf.iter().zip(&reference).enumerate() {
            assert!((x-y).abs() < 1e-9, "convolution mismatch at {i}");
        }
        assert!((got.pmf.iter().sum::<f64>() - 1.0).abs() < 1e-8);
    }
}
#[test]
#[ignore = "Intended for release-optimized timing; runner variability prohibits absolute thresholds"]
fn benchmark_dense_fft_and_binary_search_quantiles() {
    let a = pmf(2048, 19);
    let b = pmf(2048, 31);
    let left = Distribution::new(a.clone());
    let right = Distribution::new(b.clone());
    let start = Instant::now();
    let expected = black_box(direct_convolution(black_box(&a), black_box(&b)));
    let direct = start.elapsed();

    let start = Instant::now();
    let combined = black_box(left.combine(&right));
    let fft = start.elapsed();
    for (&x, &y) in expected.iter().zip(&combined.pmf) {
        assert!((x-y).abs() < 1e-9);
    }

    let loops = 20_000;
    let start = Instant::now();
    let mut slow = 0usize;
    for i in 0..loops {
        let q = ((i % 997) + 1) as f64 / 998.0;
        slow = slow.wrapping_add(black_box(combined.cdf.iter()
            .position(|&v| v >= q).unwrap_or(combined.cdf.len()-1)));
    }
    let linear = start.elapsed();

    let start = Instant::now();
    let mut fast = 0usize;
    for i in 0..loops {
        let q = ((i % 997) + 1) as f64 / 998.0;
        fast = fast.wrapping_add(black_box(combined.quantile(q)));
    }
    let binary = start.elapsed();
    assert_eq!(slow, fast);

    println!("PERF_REFERENCE direct_ns={} fft_ns={} convolution_speedup_x={:.2}",
        direct.as_nanos(), fft.as_nanos(),
        direct.as_secs_f64()/fft.as_secs_f64().max(f64::MIN_POSITIVE));
    println!("PERF_REFERENCE linear_quantile_ns={} binary_quantile_ns={} quantile_speedup_x={:.2}",
        linear.as_nanos(), binary.as_nanos(),
        linear.as_secs_f64()/binary.as_secs_f64().max(f64::MIN_POSITIVE));
}


/// End-to-end throughput reference for the refund path. Measurements depend on
/// CI runner load; numerical integrity is asserted, timing is informational.
#[test]
#[ignore = "Release-only Monte Carlo throughput reference"]
fn benchmark_bounded_parallel_refunds() {
    use gacha_pickup_calculator::feedback::{estimate, FeedbackConfig};
    use gacha_pickup_calculator::games::{Game, GameSetup};
    use gacha_pickup_calculator::RateMode;

    let mut setup = GameSetup::new(Game::StarRail);
    setup.set_targets(1, 1);
    let mut config = FeedbackConfig::for_game(Game::StarRail);
    config.samples = 100_000;
    let start = Instant::now();
    let result = black_box(estimate(&setup, RateMode::PublishedOnly, &config).unwrap());
    let elapsed = start.elapsed();
    assert_eq!(result.scenarios.len(), config.samples);
    assert_eq!(result.raw_pulls.len(), config.samples);
    assert_eq!(result.budgets.len(), config.samples);
    assert!(result.average_raw.is_finite());
    assert!(result.budgets.windows(2).all(|v| v[0] <= v[1]));
    let retained_bytes = config.samples * (
        std::mem::size_of::<gacha_pickup_calculator::feedback::RefundScenario>()
        + 2 * std::mem::size_of::<usize>());
    println!("PERF_MONTE_CARLO samples={} elapsed_ms={} retained_observation_bytes={} throughput_samples_per_s={:.0}",
        config.samples, elapsed.as_millis(), retained_bytes,
        config.samples as f64 / elapsed.as_secs_f64().max(f64::MIN_POSITIVE));
}
