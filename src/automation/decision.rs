//! Simple placeholder decision logic for Milestone 2 testing
//!
//! This is NOT the expectimax solver - that's Milestone 3.
//! This module provides basic heuristics for testing the automation loop.

use atomas_cv::{Decision, DetectionResult};
use rand::Rng;

/// Simple decision strategies for testing
#[derive(Debug, Clone, Copy)]
pub enum SimpleStrategy {
    /// Choose random valid move
    Random,
    /// Prefer INSERT over REMOVE
    PreferInsert,
    /// Prefer REMOVE over INSERT (when possible)
    PreferRemove,
}

impl SimpleStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "random" => Some(Self::Random),
            "prefer-insert" => Some(Self::PreferInsert),
            "prefer-remove" => Some(Self::PreferRemove),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Random => "Random",
            Self::PreferInsert => "PreferInsert",
            Self::PreferRemove => "PreferRemove",
        }
    }
}

/// Simple solver for testing automation loop
///
/// NOTE: This is a PLACEHOLDER for Milestone 2 testing only.
/// The real expectimax solver will be implemented in Milestone 3.
pub struct SimpleSolver {
    strategy: SimpleStrategy,
    rng: rand::rngs::ThreadRng,
}

impl SimpleSolver {
    /// Create a new simple solver with the given strategy
    pub fn new(strategy: SimpleStrategy) -> Self {
        log::warn!("=================================================");
        log::warn!("Using PLACEHOLDER decision logic: {:?}", strategy);
        log::warn!("Expectimax solver will be Milestone 3");
        log::warn!("=================================================");

        Self {
            strategy,
            rng: rand::thread_rng(),
        }
    }

    /// Choose a move based on the detected game state
    ///
    /// This uses simple heuristics, NOT game tree search or expectimax
    pub fn choose_move(&mut self, detection_result: &DetectionResult) -> anyhow::Result<Decision> {
        let num_atoms = detection_result.ring_elements.len();

        if num_atoms == 0 {
            anyhow::bail!("Cannot choose move: no atoms detected in ring");
        }

        let decision = match self.strategy {
            SimpleStrategy::Random => self.random_move(num_atoms),
            SimpleStrategy::PreferInsert => self.prefer_insert_move(num_atoms),
            SimpleStrategy::PreferRemove => self.prefer_remove_move(num_atoms),
        };

        log::info!(
            "  Decision: {} (strategy: {})",
            format_decision(&decision),
            self.strategy.as_str()
        );

        Ok(decision)
    }

    /// Choose a random valid move
    fn random_move(&mut self, num_atoms: usize) -> Decision {
        // 50% chance INSERT, 50% chance REMOVE (if >= 2 atoms)
        if num_atoms >= 2 && self.rng.gen_bool(0.5) {
            Decision::Remove {
                atom_index: self.rng.gen_range(0..num_atoms),
            }
        } else {
            Decision::Insert {
                gap_index: self.rng.gen_range(0..num_atoms),
            }
        }
    }

    /// Prefer INSERT moves
    fn prefer_insert_move(&mut self, num_atoms: usize) -> Decision {
        // Always INSERT (unless only 1 atom, then must REMOVE)
        if num_atoms == 1 {
            Decision::Remove { atom_index: 0 }
        } else {
            Decision::Insert {
                gap_index: self.rng.gen_range(0..num_atoms),
            }
        }
    }

    /// Prefer REMOVE moves
    fn prefer_remove_move(&mut self, num_atoms: usize) -> Decision {
        // Prefer REMOVE if >= 2 atoms, otherwise INSERT
        if num_atoms >= 2 {
            Decision::Remove {
                atom_index: self.rng.gen_range(0..num_atoms),
            }
        } else {
            Decision::Insert { gap_index: 0 }
        }
    }
}

impl Default for SimpleSolver {
    fn default() -> Self {
        Self::new(SimpleStrategy::PreferInsert)
    }
}

/// Format a decision for logging
fn format_decision(decision: &Decision) -> String {
    match decision {
        Decision::Insert { gap_index } => format!("INSERT at gap_index={}", gap_index),
        Decision::MinusTake { target_index } => {
            format!("MINUS TAKE atom at target_index={}", target_index)
        }
        Decision::ConvertToPlus => "CONVERT held atom to PLUS (tap centre)".to_string(),
        Decision::NeutrinoCopy { target_index } => {
            format!("NEUTRINO COPY atom at target_index={}", target_index)
        }
        Decision::Remove { atom_index } => format!("REMOVE at atom_index={}", atom_index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_parsing() {
        assert!(matches!(
            SimpleStrategy::from_str("random"),
            Some(SimpleStrategy::Random)
        ));
        assert!(matches!(
            SimpleStrategy::from_str("prefer-insert"),
            Some(SimpleStrategy::PreferInsert)
        ));
    }
}



