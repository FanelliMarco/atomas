//! Heuristic evaluation for the real Atomas rules.
//!
//! Since atoms only fuse through Plus atoms, the dominant strategic signals
//! are:
//! 1. Don't die: the ring holds 18 atoms, so ring pressure must dominate as
//!    the field fills.
//! 2. Chain potential: how good is the best Plus placement available right
//!    now? Long symmetric runs (a b c | c b a) are how big scores happen.
//! 3. Structure: adjacent equal pairs and near-symmetric neighbourhoods make
//!    future Pluses productive.
//! 4. Dead weight: atoms far below the current spawn window can never pair
//!    with spawns again; they clog the ring until a Minus or chain absorbs
//!    them.
//!
//! All terms are smooth and modest — the search (real score deltas from the
//! engine) provides the sharp signal; the heuristic only breaks ties and
//! gives the leaves direction.

use super::spawn::SpawnConfig;
use crate::game::{resolve_reactions, MAX_RING_SIZE};
use crate::{Atom, GameState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicWeights {
    /// Weight of the best immediate Plus opportunity (simulated real score).
    pub chain_potential: f64,
    /// Reward per adjacent equal regular pair.
    pub pair_bonus: f64,
    /// Penalty per atom as the ring fills (scaled quadratically).
    pub ring_pressure: f64,
    /// Penalty per atom whose value is below the current spawn window.
    pub dead_atom_penalty: f64,
    /// Penalty per Plus parked in the ring (it occupies a slot).
    pub parked_plus_penalty: f64,
    /// Terminal penalty for a lost game.
    pub game_over_penalty: f64,
}

impl Default for HeuristicWeights {
    fn default() -> Self {
        Self {
            // Deliberately < discount-adjusted real score: a fusion taken
            // NOW must always beat the same fusion promised by the eval at a
            // leaf, otherwise the solver procrastinates Plus placement.
            chain_potential: 0.4,
            pair_bonus: 3.0,
            ring_pressure: 4.0,
            dead_atom_penalty: 5.0,
            parked_plus_penalty: 8.0,
            game_over_penalty: 1.0e6,
        }
    }
}

/// Evaluate a state (NOT including the accumulated score — the search adds
/// real score deltas separately so they are never double-counted).
pub fn evaluate_state(
    state: &GameState,
    weights: &HeuristicWeights,
    spawn_config: &SpawnConfig,
) -> f64 {
    if state.is_game_over() {
        return -weights.game_over_penalty;
    }

    // 1. Chain potential: the real score of the best hypothetical Plus throw.
    //    This term simulates a Plus into every gap (O(n) reaction
    //    resolutions), so it's by far the most expensive component — see
    //    `evaluate_state_cheap` for the orderings that don't need it.
    weights.chain_potential * best_plus_payoff(&state.ring)
        + evaluate_state_cheap(state, weights, spawn_config)
}

/// Everything in `evaluate_state` EXCEPT the chain-potential term.
///
/// Used for beam ORDERING inside the search, where we only need a ranking
/// good enough to pick the few branches worth expanding; simulated-Plus
/// payoffs there cost O(n) reaction resolutions per candidate for little
/// ranking benefit. Leaf evaluations keep the full heuristic.
pub fn evaluate_state_cheap(
    state: &GameState,
    weights: &HeuristicWeights,
    spawn_config: &SpawnConfig,
) -> f64 {
    if state.is_game_over() {
        return -weights.game_over_penalty;
    }

    let ring = &state.ring;
    let n = ring.len();
    let mut value = 0.0;

    // 2. Adjacent equal regular pairs (future fusion fodder).
    if n >= 2 {
        let mut pairs = 0u32;
        for i in 0..n {
            let a = ring[i];
            let b = ring[(i + 1) % n];
            if a.is_regular() && b.is_regular() && a.value == b.value {
                pairs += 1;
            }
        }
        value += weights.pair_bonus * pairs as f64;
    }

    // 3. Ring pressure: grows quadratically; brutal close to the limit.
    let fill = n as f64 / MAX_RING_SIZE as f64;
    value -= weights.ring_pressure * (n as f64) * fill * fill;
    if n >= MAX_RING_SIZE - 2 {
        // Near-death emphasis on top of the smooth term.
        value -= weights.ring_pressure * 40.0 * (n + 3 - MAX_RING_SIZE) as f64;
    }

    // 4. Dead atoms below the spawn window.
    let (min_spawn, _) = spawn_config.spawn_range(state.moves);
    let dead = ring
        .iter()
        .filter(|a| a.is_regular() && a.value < min_spawn)
        .count();
    value -= weights.dead_atom_penalty * dead as f64;

    // 5. Parked Pluses occupy slots without being playable.
    let parked = ring.iter().filter(|a| a.is_plus() || a.is_dark_plus()).count();
    value -= weights.parked_plus_penalty * parked as f64;

    value
}

/// The best real-rules payoff of throwing a Plus into any gap of this ring
/// (0.0 if no gap would fire). This both measures chain potential and tells
/// the solver how valuable it is to *save* structure for the next Plus.
pub fn best_plus_payoff(ring: &[Atom]) -> f64 {
    let n = ring.len();
    if n < 2 {
        return 0.0;
    }
    let mut best = 0u64;
    for gap in 0..n {
        let a = ring[gap];
        let b = ring[(gap + 1) % n];
        // Only simulate gaps that would actually fire.
        if !(a.is_regular() && b.is_regular() && a.value == b.value) {
            continue;
        }
        let mut trial = ring.to_vec();
        let pos = gap + 1;
        if pos >= trial.len() {
            trial.push(Atom::PLUS);
        } else {
            trial.insert(pos, Atom::PLUS);
        }
        let (_, gained) = resolve_reactions(trial);
        best = best.max(gained);
    }
    best as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_over_is_terrible() {
        let state = GameState::new(vec![Atom::new(1); MAX_RING_SIZE + 1], Atom::new(1));
        let v = evaluate_state(&state, &HeuristicWeights::default(), &SpawnConfig::default());
        assert!(v <= -1.0e5);
    }

    #[test]
    fn symmetric_ring_beats_scrambled_ring() {
        let w = HeuristicWeights::default();
        let s = SpawnConfig::default();
        // Same multiset of atoms, different arrangements.
        let symmetric = GameState::new(
            vec![
                Atom::new(3),
                Atom::new(2),
                Atom::new(1),
                Atom::new(1),
                Atom::new(2),
                Atom::new(3),
            ],
            Atom::new(1),
        );
        let scrambled = GameState::new(
            vec![
                Atom::new(1),
                Atom::new(2),
                Atom::new(3),
                Atom::new(1),
                Atom::new(2),
                Atom::new(3),
            ],
            Atom::new(1),
        );
        assert!(
            evaluate_state(&symmetric, &w, &s) > evaluate_state(&scrambled, &w, &s),
            "symmetric arrangement should evaluate higher"
        );
    }

    #[test]
    fn chain_potential_sees_deep_chains() {
        // 2 2 around a fireable pair 1 1 → plus into the middle chains.
        let deep = vec![Atom::new(2), Atom::new(1), Atom::new(1), Atom::new(2)];
        let shallow = vec![Atom::new(5), Atom::new(1), Atom::new(1), Atom::new(7)];
        assert!(best_plus_payoff(&deep) > best_plus_payoff(&shallow));
    }
}
