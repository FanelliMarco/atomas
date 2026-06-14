pub mod expectimax;
mod heuristic;
mod movegen;
mod spawn;

#[cfg(test)]
mod tests;

pub use expectimax::{expectimax_search, ExpectimaxConfig, ExpectimaxResult};
pub use heuristic::HeuristicWeights;
pub use movegen::{count_legal_actions, generate_legal_actions};
pub use spawn::SpawnConfig;

use crate::{Action, GameState};

/// Configuration for the solver
#[derive(Debug, Clone)]
pub struct SolverConfig {
    /// Expectimax search configuration
    pub expectimax_config: ExpectimaxConfig,
    /// Spawn probability configuration
    pub spawn_config: SpawnConfig,
    /// Heuristic evaluation weights
    pub heuristic_weights: HeuristicWeights,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            expectimax_config: ExpectimaxConfig::default(),
            spawn_config: SpawnConfig::default(),
            heuristic_weights: HeuristicWeights::default(),
        }
    }
}

impl SolverConfig {
    /// Create a fast solver configuration (lower depth, fewer spawns)
    pub fn fast() -> Self {
        Self {
            expectimax_config: ExpectimaxConfig {
                max_depth: 1,
                max_spawns_per_node: 4,
                ..ExpectimaxConfig::default()
            },
            spawn_config: SpawnConfig::default(),
            heuristic_weights: HeuristicWeights::default(),
        }
    }

    /// Create a thorough solver configuration (higher depth, more spawns)
    pub fn thorough() -> Self {
        Self {
            expectimax_config: ExpectimaxConfig {
                max_depth: 3,
                max_spawns_per_node: 7,
                ..ExpectimaxConfig::default()
            },
            spawn_config: SpawnConfig::default(),
            heuristic_weights: HeuristicWeights::default(),
        }
    }
}

/// Result from solver evaluation
#[derive(Debug, Clone)]
pub struct SolverResult {
    /// The best action to take
    pub best_action: Action,
    /// Expected value of the action
    pub expected_value: f64,
    /// Number of nodes evaluated during search
    pub nodes_evaluated: usize,
    /// Number of legal actions considered
    pub actions_considered: usize,
}

/// Atomas game solver using expectimax search
pub struct Solver {
    config: SolverConfig,
}

impl Solver {
    /// Create a new solver with the given configuration
    pub fn new(config: SolverConfig) -> Self {
        Self { config }
    }

    /// Create a solver with default configuration
    pub fn default() -> Self {
        Self::new(SolverConfig::default())
    }

    /// Create a fast solver (lower depth, quicker decisions)
    pub fn fast() -> Self {
        Self::new(SolverConfig::fast())
    }

    /// Create a thorough solver (higher depth, better decisions)
    pub fn thorough() -> Self {
        Self::new(SolverConfig::thorough())
    }

    /// Solve for the best action given a game state
    pub fn solve(&self, state: &GameState) -> Result<SolverResult, String> {
        // Generate legal actions
        let actions = generate_legal_actions(state);

        if actions.is_empty() {
            return Err("No legal actions available in the current state".to_string());
        }

        // Run expectimax search
        let expectimax_result = expectimax::expectimax_search(
            state,
            &self.config.spawn_config,
            &self.config.heuristic_weights,
            &self.config.expectimax_config,
        )?;

        Ok(SolverResult {
            best_action: expectimax_result.best_action,
            expected_value: expectimax_result.expected_value,
            nodes_evaluated: expectimax_result.nodes_evaluated,
            actions_considered: actions.len(),
        })
    }

    /// Get the solver configuration
    pub fn config(&self) -> &SolverConfig {
        &self.config
    }

    /// Update the solver configuration
    pub fn set_config(&mut self, config: SolverConfig) {
        self.config = config;
    }
}

/// Quick solve function using default configuration
pub fn solve(state: &GameState) -> Result<SolverResult, String> {
    let solver = Solver::default();
    solver.solve(state)
}

/// Quick solve function using fast configuration
pub fn solve_fast(state: &GameState) -> Result<SolverResult, String> {
    let solver = Solver::fast();
    solver.solve(state)
}



