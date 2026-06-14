//! Spawn model matching the real game.
//!
//! Verified behaviour of the real game:
//! - Regular atoms spawn uniformly from a moving window of values. The window
//!   starts at [1, 3] (H..Li) and its lower bound increases by 1 every
//!   `period` (= 40) consumed atoms.
//! - A Plus spawns roughly 1 in 5, with a pity timer: you are guaranteed a
//!   Plus at least every 5 moves.
//! - A Minus spawns roughly 1 in 20 (guaranteed roughly every 20 moves /
//!   every 3-4 Pluses).
//! - Dark Plus: ~1/80 once score > 750. Neutrino: ~1/60 once score > 1500.
//!   (Both default to off here because the CV layer must detect them before
//!   the solver can act on them; enable once detection supports them.)

use crate::Atom;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnConfig {
    /// Probability of spawning a Plus atom (real game ≈ 1/5).
    pub plus_probability: f64,
    /// Probability of spawning a Minus atom (real game ≈ 1/20).
    pub minus_probability: f64,
    /// Probability of a Dark Plus (real game ≈ 1/80 once score > 750).
    pub dark_plus_probability: f64,
    /// Probability of a Neutrino (real game ≈ 1/60 once score > 1500).
    pub neutrino_probability: f64,
    /// Width of the regular-atom window (real game: 3 values).
    pub range_width: u16,
    /// Moves per upward shift of the window's lower bound (real game: 40).
    pub period: u32,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            plus_probability: 0.20,
            minus_probability: 0.05,
            dark_plus_probability: 0.0,
            neutrino_probability: 0.0,
            range_width: 3,
            period: 40,
        }
    }
}

impl SpawnConfig {
    /// Uniform distribution over values 1..=max_value, no specials (tests).
    pub fn uniform(max_value: usize) -> Self {
        Self {
            plus_probability: 0.0,
            minus_probability: 0.0,
            dark_plus_probability: 0.0,
            neutrino_probability: 0.0,
            range_width: max_value as u16,
            period: u32::MAX,
        }
    }

    /// Default behaviour but without special atoms.
    pub fn regular_only() -> Self {
        Self {
            plus_probability: 0.0,
            minus_probability: 0.0,
            ..Self::default()
        }
    }

    /// The window of regular values that can spawn after `moves` moves.
    pub fn spawn_range(&self, moves: u32) -> (i16, i16) {
        let min = 1 + (moves / self.period.max(1)) as i16;
        (min, min + self.range_width as i16 - 1)
    }

    /// Full normalised spawn distribution at a given move count.
    pub fn distribution(&self, moves: u32) -> Vec<(Atom, f64)> {
        let mut dist = Vec::new();

        let special_total = self.plus_probability
            + self.minus_probability
            + self.dark_plus_probability
            + self.neutrino_probability;
        let regular_total = (1.0 - special_total).max(0.0);

        let (min, max) = self.spawn_range(moves);
        let count = (max - min + 1).max(1) as f64;
        for v in min..=max {
            dist.push((Atom::new(v), regular_total / count));
        }
        if self.plus_probability > 0.0 {
            dist.push((Atom::PLUS, self.plus_probability));
        }
        if self.minus_probability > 0.0 {
            dist.push((Atom::MINUS, self.minus_probability));
        }
        if self.dark_plus_probability > 0.0 {
            dist.push((Atom::DARK_PLUS, self.dark_plus_probability));
        }
        if self.neutrino_probability > 0.0 {
            dist.push((Atom::NEUTRINO, self.neutrino_probability));
        }
        dist
    }

    /// Probability of a specific atom spawning at a given move count.
    pub fn get_probability(&self, atom: &Atom, moves: u32) -> f64 {
        self.distribution(moves)
            .into_iter()
            .find(|(a, _)| a == atom)
            .map(|(_, p)| p)
            .unwrap_or(0.0)
    }

    /// The most likely spawns, truncated to `max_samples` and RENORMALISED so
    /// the chance node still computes a true expectation.
    pub fn sample_likely_spawns(&self, moves: u32, max_samples: usize) -> Vec<(Atom, f64)> {
        let mut dist = self.distribution(moves);
        dist.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        dist.truncate(max_samples);
        let total: f64 = dist.iter().map(|(_, p)| p).sum();
        if total > 0.0 {
            for (_, p) in dist.iter_mut() {
                *p /= total;
            }
        }
        dist
    }

    /// Sample a random spawn (for self-play benchmarks).
    pub fn sample<R: rand::Rng>(&self, moves: u32, rng: &mut R) -> Atom {
        let dist = self.distribution(moves);
        let mut x: f64 = rng.gen_range(0.0..1.0);
        for (atom, p) in &dist {
            if x < *p {
                return *atom;
            }
            x -= p;
        }
        dist.last().map(|(a, _)| *a).unwrap_or(Atom::new(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_sums_to_one() {
        let config = SpawnConfig::default();
        for moves in [0, 39, 40, 200] {
            let total: f64 = config.distribution(moves).iter().map(|(_, p)| p).sum();
            assert!((total - 1.0).abs() < 1e-9, "total={total} at moves={moves}");
        }
    }

    #[test]
    fn range_shifts_every_period() {
        let config = SpawnConfig::default();
        assert_eq!(config.spawn_range(0), (1, 3));
        assert_eq!(config.spawn_range(39), (1, 3));
        assert_eq!(config.spawn_range(40), (2, 4));
        assert_eq!(config.spawn_range(120), (4, 6));
    }

    #[test]
    fn likely_spawns_renormalised() {
        let config = SpawnConfig::default();
        let samples = config.sample_likely_spawns(0, 3);
        assert_eq!(samples.len(), 3);
        let total: f64 = samples.iter().map(|(_, p)| p).sum();
        assert!((total - 1.0).abs() < 1e-9);
    }
}
