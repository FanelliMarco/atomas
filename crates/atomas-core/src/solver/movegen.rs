//! Legal move generation for the real Atomas action set.

use crate::{Action, GameState};

/// Generate all legal actions for a given game state.
pub fn generate_legal_actions(state: &GameState) -> Vec<Action> {
    let mut actions = Vec::new();
    if state.is_game_over() {
        return actions;
    }

    let n = state.ring_size();
    let p = state.player_atom;

    if p.is_regular() || p.is_plus() || p.is_dark_plus() {
        // Everything throwable is thrown into one of the n gaps. With an
        // empty ring there is conceptually one gap; the engine's insert
        // handles n == 0 by pushing.
        let gaps = n.max(1);
        for gap_index in 0..gaps {
            actions.push(Action::Insert { gap_index });
        }
        // A minus-taken regular atom can instead be converted to a Plus.
        if state.held_from_minus && p.is_regular() {
            actions.push(Action::ConvertToPlus);
        }
    } else if p.is_minus() {
        for target_index in 0..n {
            actions.push(Action::MinusTake { target_index });
        }
    } else if p.is_neutrino() {
        for target_index in 0..n {
            if state.ring[target_index].is_regular() {
                actions.push(Action::NeutrinoCopy { target_index });
            }
        }
    }

    // Keep only actions the engine agrees are legal (cheap belt-and-braces).
    actions.retain(|a| state.is_legal_action(a));
    actions
}

/// Count the number of legal actions for a state.
pub fn count_legal_actions(state: &GameState) -> usize {
    generate_legal_actions(state).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Atom;

    #[test]
    fn regular_atom_inserts_one_per_gap() {
        let state = GameState::new(
            vec![Atom::new(1), Atom::new(2), Atom::new(3)],
            Atom::new(4),
        );
        let actions = generate_legal_actions(&state);
        assert_eq!(actions.len(), 3);
        assert!(actions.iter().all(|a| matches!(a, Action::Insert { .. })));
    }

    #[test]
    fn plus_atom_inserts_into_gaps_not_onto_atoms() {
        let state = GameState::new(
            vec![Atom::new(1), Atom::new(2), Atom::new(3)],
            Atom::PLUS,
        );
        let actions = generate_legal_actions(&state);
        assert_eq!(actions.len(), 3);
        assert!(actions.iter().all(|a| matches!(a, Action::Insert { .. })));
    }

    #[test]
    fn minus_atom_takes_one_target() {
        let state = GameState::new(
            vec![Atom::new(1), Atom::new(2), Atom::new(3)],
            Atom::MINUS,
        );
        let actions = generate_legal_actions(&state);
        assert_eq!(actions.len(), 3); // n choices, NOT n*(n-1)
        assert!(actions.iter().all(|a| matches!(a, Action::MinusTake { .. })));
    }

    #[test]
    fn held_atom_can_convert_to_plus() {
        let mut state = GameState::new(
            vec![Atom::new(1), Atom::new(2), Atom::new(3)],
            Atom::new(5),
        );
        state.held_from_minus = true;
        let actions = generate_legal_actions(&state);
        assert!(actions.contains(&Action::ConvertToPlus));
        assert_eq!(actions.len(), 4); // 3 gaps + convert
    }
}
