use crate::fft::convolve_dense;

#[derive(Clone, Debug)]
pub struct Distribution {
    pub pmf: Vec<f64>,
    pub cdf: Vec<f64>,
    pub mean: f64,
}

impl Distribution {
    pub fn new(pmf: Vec<f64>) -> Self {
        let mut cdf = Vec::with_capacity(pmf.len());
        let mut acc = 0.0;
        let mut mean = 0.0;
        for (n, &p) in pmf.iter().enumerate() {
            acc += p;
            mean += n as f64 * p;
            cdf.push(acc.min(1.0));
        }
        Self { pmf, cdf, mean }
    }

    pub fn at(&self, pulls: usize) -> f64 {
        self.cdf.get(pulls).copied().unwrap_or_else(|| self.cdf.last().copied().unwrap_or(0.0))
    }

    pub fn quantile(&self, chance: f64) -> usize {
        // A mathematical guarantee must never be shortened by floating-point
        // accumulation or by a tolerance intended for ordinary quantiles.
        if chance.is_nan() { return self.cdf.len().saturating_sub(1); }
        if chance >= 1.0 { return self.pmf.len().saturating_sub(1); }
        if chance <= 0.0 { return 0; }
        self.cdf.partition_point(|&p| p < chance).min(self.cdf.len().saturating_sub(1))
    }

    /// Exact convolution of independent *first-passage* (stopping-time) distributions.
    pub fn combine(&self, other: &Self) -> Self {
        if self.pmf.is_empty() || other.pmf.is_empty() {
            return Self::new(Vec::new());
        }
        // Use O(N log N) FFT only for large, dense, well-formed distributions.
        if let Some(dense) = convolve_dense(&self.pmf, &other.pmf) {
            return Self::new(dense);
        }
        let mut combined = vec![0.0; self.pmf.len() + other.pmf.len() - 1];
        for (i, &a) in self.pmf.iter().enumerate() {
            if a == 0.0 { continue; }
            for (j, &b) in other.pmf.iter().enumerate() {
                // Preserve exact first-passage arithmetic while skipping
                // unreachable (zero-probability) states in sparse banners.
                if b != 0.0 { combined[i + j] += a * b; }
            }
        }
        Self::new(combined)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Allocation {
    pub character_pulls: usize,
    pub weapon_pulls: usize,
    pub success_probability: f64,
}

/// Optimal split of universally exchangeable pulls between two independent banners.
/// Dedicated tickets may not be transferred between banner types.
pub fn optimal_allocation(
    character: &Distribution,
    weapon: &Distribution,
    dedicated_character: usize,
    dedicated_weapon: usize,
    flexible: usize,
) -> Allocation {
    // Tickets can be reassigned once a banner goal is met. A fixed split is
    // suboptimal: success iff (Tc-Dc)+ + (Tw-Dw)+ <= flexible.
    // Condition on Tc to compute the exact adaptive success probability.
    let mut success_probability = 0.0;
    for (draws, &mass) in character.pmf.iter().enumerate() {
        if mass == 0.0 { continue; }
        let used_flexible = draws.saturating_sub(dedicated_character);
        if used_flexible <= flexible {
            success_probability += mass * weapon.at(dedicated_weapon.saturating_add(flexible - used_flexible));
        }
    }
    // Show the best static split as an illustration; success_probability above
    // is the adaptive strategy, not the static allocation's probability.
    let mut answer = Allocation { success_probability: success_probability.clamp(0.0, 1.0), ..Allocation::default() };
    let mut best_fixed = -1.0_f64;
    // Once the character banner's maximum stopping time is fully funded,
    // further character allocation cannot improve its probability. Searching
    // beyond that point only wastes O(flexible) CPU and removes weapon tickets.
    let useful_character_budget = character.pmf.len().saturating_sub(1)
        .saturating_sub(dedicated_character).min(flexible);
    for allotted in 0..=useful_character_budget {
        let c = dedicated_character.saturating_add(allotted);
        let w = dedicated_weapon.saturating_add(flexible - allotted);
        let fixed = character.at(c) * weapon.at(w);
        if fixed > best_fixed {
            best_fixed = fixed;
            answer.character_pulls = c;
            answer.weapon_pulls = w;
        }
    }
    answer
}

