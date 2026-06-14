//! Bridges CV detection with the (rebuilt, real-rules) expectimax solver.
//!
//! Two pieces of game state are NOT visible in a single screenshot and must
//! be tracked across the loop by the caller:
//!
//! 1. `held_from_minus` — whether the centre atom was pulled from the ring
//!    with a Minus on the previous move (this is what makes the
//!    "convert to Plus" tap legal). The caller knows this because it knows
//!    its own previous decision was `MinusTake`.
//! 2. `moves` — how many atoms have been consumed from the spawner. The real
//!    game shifts its spawn window up every 40 moves, and the solver's
//!    chance nodes need that to model future spawns correctly.

use anyhow::Result;
use atomas_core::{Atom, GameState as CoreGameState};
use atomas_cv::{Decision, DetectionResult};

/// Convert a CV detection result to a solver game state.
pub fn detection_to_core_state(
    detection: &DetectionResult,
    moves: u32,
    held_from_minus: bool,
) -> Result<CoreGameState> {
    let mut ring_atoms = Vec::new();
    for (element, _bbox) in &detection.ring_elements {
        ring_atoms.push(Atom::new(element.element_type.to_numeric()));
    }

    if ring_atoms.is_empty() {
        anyhow::bail!("Cannot create game state: no atoms detected in ring");
    }

    let player_atom = if let Some((element, _bbox)) = &detection.player_atom {
        Atom::new(element.element_type.to_numeric())
    } else {
        log::warn!("Player atom not detected, defaulting to H (value=1)");
        Atom::new(1)
    };

    let mut state = CoreGameState::new(ring_atoms, player_atom);
    state.moves = moves;
    state.held_from_minus = held_from_minus && player_atom.is_regular();

    log::debug!(
        "Detection -> core state: ring_size={}, player={}, moves={}, held={}",
        state.ring_size(),
        state.player_atom.value,
        state.moves,
        state.held_from_minus
    );

    Ok(state)
}

/// Convert a solver action to an automation decision (1:1 by design).
pub fn solver_action_to_decision(action: &atomas_core::Action) -> Result<Decision> {
    Ok(match action {
        atomas_core::Action::Insert { gap_index } => Decision::Insert {
            gap_index: *gap_index,
        },
        atomas_core::Action::MinusTake { target_index } => Decision::MinusTake {
            target_index: *target_index,
        },
        atomas_core::Action::ConvertToPlus => Decision::ConvertToPlus,
        atomas_core::Action::NeutrinoCopy { target_index } => Decision::NeutrinoCopy {
            target_index: *target_index,
        },
    })
}

/// Format a solver action for logging.
pub fn format_solver_action(action: &atomas_core::Action) -> String {
    match action {
        atomas_core::Action::Insert { gap_index } => format!("INSERT at gap {gap_index}"),
        atomas_core::Action::MinusTake { target_index } => {
            format!("MINUS: take atom {target_index} into hand")
        }
        atomas_core::Action::ConvertToPlus => "CONVERT held atom to PLUS (tap centre)".to_string(),
        atomas_core::Action::NeutrinoCopy { target_index } => {
            format!("NEUTRINO: copy atom {target_index}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomas_core::{Element, ElementType};
    use atomas_cv::BBox;

    fn create_test_element(value: i16) -> Element<'static> {
        let element_type = ElementType::from_numeric(value).unwrap();
        Element {
            id: atomas_core::Id::Single('H'),
            element_type,
            name: "Test",
            rgb: (255, 255, 255),
        }
    }

    #[test]
    fn test_detection_to_core_state() {
        let ring_elements = vec![
            (create_test_element(1), BBox::new(0, 0, 10, 10)),
            (create_test_element(2), BBox::new(10, 10, 20, 20)),
            (create_test_element(3), BBox::new(20, 20, 30, 30)),
        ];
        let player_atom = Some((create_test_element(2), BBox::new(100, 100, 110, 110)));
        let detection = DetectionResult { ring_elements, player_atom };

        let core_state = detection_to_core_state(&detection, 57, true).unwrap();
        assert_eq!(core_state.ring_size(), 3);
        assert_eq!(core_state.player_atom.value, 2);
        assert_eq!(core_state.moves, 57);
        assert!(core_state.held_from_minus);
    }

    #[test]
    fn held_flag_dropped_for_special_player() {
        let ring_elements = vec![(create_test_element(1), BBox::new(0, 0, 10, 10))];
        let player_atom = Some((create_test_element(-1), BBox::new(100, 100, 110, 110)));
        let detection = DetectionResult { ring_elements, player_atom };
        let core_state = detection_to_core_state(&detection, 0, true).unwrap();
        assert!(!core_state.held_from_minus, "a Plus can never be 'held'");
    }

    #[test]
    fn action_decision_mapping_is_one_to_one() {
        let d = solver_action_to_decision(&atomas_core::Action::MinusTake { target_index: 3 })
            .unwrap();
        assert!(matches!(d, Decision::MinusTake { target_index: 3 }));
        let d = solver_action_to_decision(&atomas_core::Action::ConvertToPlus).unwrap();
        assert!(matches!(d, Decision::ConvertToPlus));
    }

    #[test]
    fn test_empty_ring_fails() {
        let detection = DetectionResult { ring_elements: vec![], player_atom: None };
        assert!(detection_to_core_state(&detection, 0, false).is_err());
    }
}
