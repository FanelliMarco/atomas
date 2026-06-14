//! Expectimax search over the real Atomas rules.
//!
//! Structure:
//! - Decision node: max over legal actions; the value of an action is the
//!   REAL score it gains (engine-computed) plus the discounted value of what
//!   follows.
//! - After actions that consume the spawned atom (Insert, NeutrinoCopy from
//!   spawn), what follows is a chance node over the spawn distribution.
//! - After MinusTake and ConvertToPlus the next centre atom is KNOWN
//!   (deterministic), so the search continues straight into another decision
//!   node without burning a chance layer. This is what makes the solver
//!   understand the grab → convert → place-Plus combo, the single biggest
//!   scoring tool in the real game.
//!
//! Depth is counted in chance layers (spawns looked ahead), so `max_depth: 2`
//! means "my move, the next spawn, my reply, the spawn after that, my reply".

use super::heuristic::{evaluate_state, evaluate_state_cheap, HeuristicWeights};
use super::movegen::generate_legal_actions;
use super::spawn::SpawnConfig;
use crate::{Action, GameState};

#[derive(Debug, Clone)]
pub struct ExpectimaxResult {
    pub best_action: Action,
    pub expected_value: f64,
    pub nodes_evaluated: usize,
}

#[derive(Debug, Clone)]
pub struct ExpectimaxConfig {
    /// Number of chance (spawn) layers to look ahead.
    pub max_depth: usize,
    /// Spawn outcomes considered per chance node (renormalised).
    pub max_spawns_per_node: usize,
    /// Discount applied per chance layer (uncertain future is worth less).
    pub discount: f64,
    /// Hard cap on nodes per search; past it, subtrees collapse to static
    /// eval so latency stays bounded no matter the position.
    pub node_budget: usize,
}

impl Default for ExpectimaxConfig {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_spawns_per_node: 6,
            discount: 0.9,
            node_budget: 300_000,
        }
    }
}

struct Ctx<'a> {
    spawn: &'a SpawnConfig,
    weights: &'a HeuristicWeights,
    config: &'a ExpectimaxConfig,
    nodes: usize,
}

impl Ctx<'_> {
    #[inline]
    fn over_budget(&self) -> bool {
        self.nodes >= self.config.node_budget
    }
}

/// Find the best action for `state`.
pub fn expectimax_search(
    state: &GameState,
    spawn_config: &SpawnConfig,
    heuristic_weights: &HeuristicWeights,
    config: &ExpectimaxConfig,
) -> Result<ExpectimaxResult, String> {
    let actions = generate_legal_actions(state);
    if actions.is_empty() {
        return Err("No legal actions available".to_string());
    }

    let mut ctx = Ctx {
        spawn: spawn_config,
        weights: heuristic_weights,
        config,
        nodes: 0,
    };

    let mut best_action = actions[0];
    let mut best_value = f64::NEG_INFINITY;

    for action in &actions {
        let Ok(next) = state.apply_action(action) else {
            continue;
        };
        let value = continuation_value(&mut ctx, state, action, &next, config.max_depth);
        if value > best_value {
            best_value = value;
            best_action = *action;
        }
    }

    Ok(ExpectimaxResult {
        best_action,
        expected_value: best_value,
        nodes_evaluated: ctx.nodes,
    })
}

/// Value of taking `action` from `state` with `depth` chance layers left.
/// Value of an action whose resulting state has ALREADY been computed.
/// States are applied exactly once (during beam ordering) and reused here.
fn continuation_value(
    ctx: &mut Ctx,
    state: &GameState,
    action: &Action,
    next: &GameState,
    depth: usize,
) -> f64 {
    ctx.nodes += 1;

    let gained = (next.score - state.score) as f64;

    if next.is_game_over() {
        return gained - ctx.weights.game_over_penalty;
    }

    match action {
        // Deterministic follow-ups: the centre atom is known, keep deciding
        // at the SAME depth (no spawn happened).
        Action::MinusTake { .. } | Action::ConvertToPlus | Action::NeutrinoCopy { .. } => {
            gained + best_decision_value(ctx, next, depth)
        }
        // The spawned/held atom was thrown: a new spawn follows.
        Action::Insert { .. } => gained + ctx.config.discount * chance_value(ctx, next, depth),
    }
}

/// Max over decisions when the centre atom is known.
///
/// Interior nodes are beam-pruned: every action gets a cheap one-step look
/// (real score gained + static eval), and only the most promising few are
/// expanded recursively. The root in `expectimax_search` stays full-width.
fn best_decision_value(ctx: &mut Ctx, state: &GameState, depth: usize) -> f64 {
    if ctx.over_budget() {
        return evaluate_state(state, ctx.weights, ctx.spawn);
    }

    let actions = generate_legal_actions(state);
    if actions.is_empty() {
        return evaluate_state(state, ctx.weights, ctx.spawn);
    }

    // Narrower beam deeper in the tree: precision matters most near the root.
    let beam: usize = if depth >= ctx.config.max_depth { 6 } else { 4 };

    // Apply each action exactly ONCE. Order by the cheap heuristic (no
    // chain-potential simulation: that term costs O(n) reaction resolutions
    // per candidate and barely changes the ranking); recurse on the stored
    // states so nothing is re-applied.
    let mut applied: Vec<(f64, Action, GameState)> = actions
        .iter()
        .filter_map(|a| {
            let next = state.apply_action(a).ok()?;
            let cheap = (next.score - state.score) as f64
                + evaluate_state_cheap(&next, ctx.weights, ctx.spawn);
            Some((cheap, *a, next))
        })
        .collect();
    if applied.len() > beam {
        applied.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        applied.truncate(beam);
    }

    let mut best = f64::NEG_INFINITY;
    for (_, action, next) in &applied {
        let v = continuation_value(ctx, state, action, next, depth);
        best = best.max(v);
    }
    best
}

/// Expectation over the next spawn. Consumes one depth layer.
fn chance_value(ctx: &mut Ctx, state: &GameState, depth: usize) -> f64 {
    if depth == 0 || ctx.over_budget() {
        return evaluate_state(state, ctx.weights, ctx.spawn);
    }

    // Full spawn fan-out only at the first chance layer; deeper layers keep
    // just the most probable outcomes (renormalised by sample_likely_spawns).
    let n_spawns = if depth >= ctx.config.max_depth {
        ctx.config.max_spawns_per_node
    } else {
        ctx.config.max_spawns_per_node.div_ceil(2).max(2)
    };

    let spawns = ctx.spawn.sample_likely_spawns(state.moves, n_spawns);
    if spawns.is_empty() {
        return evaluate_state(state, ctx.weights, ctx.spawn);
    }

    let mut expected = 0.0;
    for (atom, prob) in spawns {
        let mut spawn_state = state.clone();
        spawn_state.player_atom = atom;
        spawn_state.held_from_minus = false;
        expected += prob * best_decision_value(ctx, &spawn_state, depth - 1);
    }
    expected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Atom;

    fn run(state: &GameState) -> ExpectimaxResult {
        expectimax_search(
            state,
            &SpawnConfig::default(),
            &HeuristicWeights::default(),
            &ExpectimaxConfig::default(),
        )
        .unwrap()
    }

    #[test]
    fn returns_legal_action() {
        let state = GameState::new(vec![Atom::new(1), Atom::new(2), Atom::new(3)], Atom::new(1));
        let result = run(&state);
        assert!(state.is_legal_action(&result.best_action));
        assert!(result.nodes_evaluated > 0);
    }

    #[test]
    fn plus_fires_on_the_matching_pair() {
        // Ring: 3 3 5 7 — only gap 0 (between the two 3s... careful: gap g is
        // between ring[g] and ring[g+1]) fires.
        let state = GameState::new(
            vec![Atom::new(3), Atom::new(3), Atom::new(5), Atom::new(7)],
            Atom::PLUS,
        );
        let result = run(&state);
        assert_eq!(result.best_action, Action::Insert { gap_index: 0 });
    }

    #[test]
    fn plus_prefers_the_longer_chain() {
        // Two fireable pairs: 1-1 with 2..2 wrapped symmetry vs lone 1-1.
        // Ring: [2, 1, 1, 2, 9, 1, 1, 8]
        //   gap 1 (between idx1,idx2) fires and chains through the 2s.
        //   gap 5 (between idx5,idx6) fires with no chain.
        let state = GameState::new(
            vec![
                Atom::new(2),
                Atom::new(1),
                Atom::new(1),
                Atom::new(2),
                Atom::new(9),
                Atom::new(1),
                Atom::new(1),
                Atom::new(8),
            ],
            Atom::PLUS,
        );
        let result = run(&state);
        assert_eq!(result.best_action, Action::Insert { gap_index: 1 });
    }

    #[test]
    fn minus_actions_are_takes() {
        let state = GameState::new(
            vec![Atom::new(1), Atom::new(2), Atom::new(3)],
            Atom::MINUS,
        );
        let result = run(&state);
        assert!(matches!(result.best_action, Action::MinusTake { .. }));
    }

    #[test]
    fn deterministic() {
        let state = GameState::new(vec![Atom::new(1), Atom::new(2), Atom::new(3)], Atom::new(1));
        let a = run(&state);
        let b = run(&state);
        assert_eq!(a.best_action, b.best_action);
        assert_eq!(a.expected_value, b.expected_value);
    }

    #[test]
    fn handles_two_atom_ring() {
        let state = GameState::new(vec![Atom::new(1), Atom::new(2)], Atom::new(1));
        let result = run(&state);
        assert!(state.is_legal_action(&result.best_action));
    }
}
