//! Expectimax solver integration (rebuilt for the real rules).
//!
//! This wrapper is STATEFUL on purpose: two facts about the game are not
//! visible in a single screenshot, so they are tracked across loop
//! iterations here:
//!
//! - whether the current centre atom was grabbed with a Minus last move
//!   (`held_from_minus`) — makes the "convert to Plus" tap legal;
//! - how many spawner atoms have been consumed (`moves`) — drives the spawn
//!   window the solver's chance nodes use.

use anyhow::{Context, Result};
use atomas_core::{Solver, SolverConfig};
use atomas_cv::{Decision, DetectionResult};

use crate::solver_integration::{
    detection_to_core_state, format_solver_action, solver_action_to_decision,
};

/// Solver strategy selection.
#[derive(Debug, Clone, Copy)]
pub enum SolverStrategy {
    Expectimax,
    ExpectimaxFast,
    ExpectimaxThorough,
}

impl SolverStrategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "expectimax" => Some(Self::Expectimax),
            "expectimax-fast" | "fast" => Some(Self::ExpectimaxFast),
            "expectimax-thorough" | "thorough" => Some(Self::ExpectimaxThorough),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Expectimax => "Expectimax",
            Self::ExpectimaxFast => "ExpectimaxFast",
            Self::ExpectimaxThorough => "ExpectimaxThorough",
        }
    }
}

/// Expectimax solver for game automation.
pub struct ExpectimaxSolver {
    solver: Solver,
    strategy: SolverStrategy,
    /// Spawner atoms consumed so far this game (drives the spawn window).
    moves: u32,
    /// True iff the PREVIOUS executed decision was a MinusTake, meaning the
    /// centre atom seen in the next screenshot is held and convertible.
    next_is_held: bool,
}

impl ExpectimaxSolver {
    pub fn new(strategy: SolverStrategy) -> Self {
        let solver = match strategy {
            SolverStrategy::Expectimax => Solver::default(),
            SolverStrategy::ExpectimaxFast => Solver::fast(),
            SolverStrategy::ExpectimaxThorough => Solver::thorough(),
        };

        log::info!("=================================================");
        log::info!("Using EXPECTIMAX solver (real rules): {}", strategy.as_str());
        log::info!("  Depth: {}", solver.config().expectimax_config.max_depth);
        log::info!(
            "  Spawn samples: {}",
            solver.config().expectimax_config.max_spawns_per_node
        );
        log::info!("=================================================");

        Self {
            solver,
            strategy,
            moves: 0,
            next_is_held: false,
        }
    }

    pub fn with_depth(depth: usize) -> Self {
        let mut config = SolverConfig::default();
        config.expectimax_config.max_depth = depth;
        Self {
            solver: Solver::new(config),
            strategy: SolverStrategy::Expectimax,
            moves: 0,
            next_is_held: false,
        }
    }

    /// Reset per-game tracking (call when a new game starts).
    pub fn reset(&mut self) {
        self.moves = 0;
        self.next_is_held = false;
    }

    /// Choose a move using expectimax search.
    pub fn choose_move(&mut self, detection_result: &DetectionResult) -> Result<Decision> {
        let core_state = detection_to_core_state(detection_result, self.moves, self.next_is_held)
            .context("Failed to convert detection to game state")?;

        let solver_result = self
            .solver
            .solve(&core_state)
            .map_err(anyhow::Error::msg)
            .context("Solver failed to find a move")?;

        log::info!(
            "  Solver action: {} (value={:.2}, nodes={})",
            format_solver_action(&solver_result.best_action),
            solver_result.expected_value,
            solver_result.nodes_evaluated
        );

        // Update cross-move tracking based on what we are about to execute.
        match solver_result.best_action {
            atomas_core::Action::Insert { .. } => {
                self.moves += 1;
                self.next_is_held = false;
            }
            atomas_core::Action::MinusTake { .. } => {
                self.moves += 1;
                self.next_is_held = true; // centre atom next frame is held
            }
            atomas_core::Action::ConvertToPlus => {
                // Free tap: no spawn consumed; the centre will show a Plus.
                self.next_is_held = false;
            }
            atomas_core::Action::NeutrinoCopy { .. } => {
                self.moves += 1;
                self.next_is_held = false;
            }
        }

        let decision = solver_action_to_decision(&solver_result.best_action)
            .context("Failed to convert solver action to decision")?;
        log::info!("  Decision: {:?}", decision);
        Ok(decision)
    }

    pub fn strategy(&self) -> SolverStrategy {
        self.strategy
    }
}

impl Default for ExpectimaxSolver {
    fn default() -> Self {
        Self::new(SolverStrategy::Expectimax)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_parsing() {
        assert!(matches!(
            SolverStrategy::from_str("expectimax"),
            Some(SolverStrategy::Expectimax)
        ));
        assert!(matches!(
            SolverStrategy::from_str("fast"),
            Some(SolverStrategy::ExpectimaxFast)
        ));
    }

    #[test]
    fn test_solver_creation() {
        let solver = ExpectimaxSolver::new(SolverStrategy::Expectimax);
        assert!(matches!(solver.strategy(), SolverStrategy::Expectimax));
        assert_eq!(solver.moves, 0);
        assert!(!solver.next_is_held);
    }
}
