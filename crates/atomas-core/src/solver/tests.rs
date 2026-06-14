//! Integration tests for the solver over the real rules.

use super::*;
use crate::{Action, Atom, GameState};

#[test]
fn solver_default_works() {
    let state = GameState::test_state();
    let result = solve(&state).unwrap();
    assert!(state.is_legal_action(&result.best_action));
}

#[test]
fn solver_fast_works() {
    let state = GameState::test_state();
    let result = solve_fast(&state).unwrap();
    assert!(state.is_legal_action(&result.best_action));
}

#[test]
fn solver_uses_plus_on_best_pair() {
    // 7 7 pair is worth far more than the 1 1 pair.
    let state = GameState::new(
        vec![
            Atom::new(7),
            Atom::new(7),
            Atom::new(3),
            Atom::new(1),
            Atom::new(1),
            Atom::new(4),
        ],
        Atom::PLUS,
    );
    let result = solve(&state).unwrap();
    assert_eq!(result.best_action, Action::Insert { gap_index: 0 });
}

#[test]
fn solver_avoids_immediate_game_over_when_possible() {
    // 18 atoms with a fireable pair: the Plus MUST fire (shrinking the ring)
    // rather than park (overflowing it).
    let mut ring = vec![Atom::new(9); 2];
    for i in 0..16 {
        ring.push(Atom::new(20 + i as i16));
    }
    let state = GameState::new(ring, Atom::PLUS);
    let result = solve(&state).unwrap();
    let next = state.apply_action(&result.best_action).unwrap();
    assert!(!next.is_game_over(), "solver must pick the fusing gap");
}

#[test]
fn solver_understands_minus_grab_combo() {
    let state = GameState::new(
        vec![
            Atom::new(8),
            Atom::new(8),
            Atom::new(2),
            Atom::new(3),
            Atom::new(4),
        ],
        Atom::MINUS,
    );
    let result = solve(&state).unwrap();
    // Whatever the target, it must be a single-atom take.
    assert!(matches!(result.best_action, Action::MinusTake { .. }));
    // And following the line: taking then converting to a Plus to fuse the
    // 8-8 pair must be at least as good as any direct option. Just sanity
    // check that the full line is playable in the engine.
    let took = state
        .apply_action(&Action::MinusTake { target_index: 2 })
        .unwrap();
    let plus = took.apply_action(&Action::ConvertToPlus).unwrap();
    let fused = plus.apply_action(&Action::Insert { gap_index: 0 }).unwrap();
    assert!(fused.score > 0);
}
