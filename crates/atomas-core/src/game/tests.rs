//! Engine tests asserting REAL Atomas behaviour, including the
//! wiki-documented chain-reaction example with its exact score sequence.

use super::*;

fn ring(values: &[i16]) -> Vec<Atom> {
    values.iter().map(|&v| Atom::new(v)).collect()
}

fn values(ring: &[Atom]) -> Vec<i16> {
    ring.iter().map(|a| a.value).collect()
}

/// Compare two rings as circles (rotation-invariant).
fn same_circle(a: &[Atom], b: &[i16]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let av = values(a);
    (0..av.len()).any(|k| (0..av.len()).all(|i| av[(i + k) % av.len()] == b[i]))
}

// ---------------------------------------------------------------------------
// Rule 1: adjacent equal atoms NEVER auto-merge.
// ---------------------------------------------------------------------------

#[test]
fn adjacent_equal_atoms_do_not_merge_on_their_own() {
    let state = GameState::new(ring(&[2, 3, 4]), Atom::new(2));
    // Insert the 2 right next to the existing 2 (gap 2 = between idx2 and idx0).
    let next = state.apply_action(&Action::Insert { gap_index: 2 }).unwrap();
    assert_eq!(next.ring.len(), 4, "no fusion without a Plus");
    assert_eq!(next.score, 0, "no score without a Plus");
}

// ---------------------------------------------------------------------------
// Rule 2: a Plus is inserted into a gap (atom count +1 when it parks).
// ---------------------------------------------------------------------------

#[test]
fn plus_parks_in_ring_when_neighbours_differ() {
    let state = GameState::new(ring(&[1, 2, 3]), Atom::PLUS);
    let next = state.apply_action(&Action::Insert { gap_index: 0 }).unwrap();
    assert_eq!(next.ring.len(), 4, "Plus joins the ring, replaces nothing");
    assert!(next.ring.iter().any(|a| a.is_plus()));
    assert_eq!(next.score, 0);
}

#[test]
fn plus_fuses_equal_neighbours() {
    // H H X — plus into gap 0 (between the two H) → He.
    let state = GameState::new(ring(&[1, 1, 5]), Atom::PLUS);
    let next = state.apply_action(&Action::Insert { gap_index: 0 }).unwrap();
    assert_eq!(values(&next.ring), vec![2, 5]);
    // score = floor(1.5 * (1 + 1)) = 3
    assert_eq!(next.score, 3);
}

#[test]
fn parked_plus_fires_when_neighbours_become_equal() {
    // Ring: 4 + 9 ... throwing a 9 next to the 4? No — make 4 + 4 by
    // inserting a 4 so the parked plus gets equal neighbours.
    let state = GameState::new(ring(&[4, -1, 9, 7]), Atom::new(9));
    // Insert 9 between idx1(+) and idx2(9): gap 1 → ring 4 + 9 9 7? That
    // doesn't trigger. Instead insert a 9... we want X + X. Put the 9 on the
    // other side of the plus: gap 0 → 4 9 + 9 7 → plus has 9|9 → fires.
    let next = state.apply_action(&Action::Insert { gap_index: 0 }).unwrap();
    assert!(
        !next.ring.iter().any(|a| a.is_plus()),
        "parked plus must consume itself in the fusion"
    );
    assert!(same_circle(&next.ring, &[4, 10, 7]), "got {:?}", values(&next.ring));
    // floor(1.5 * (9 + 1)) = 15
    assert_eq!(next.score, 15);
}

// ---------------------------------------------------------------------------
// Rule 3 + 4: chain values and the EXACT wiki score sequence.
// Wiki example: plus between H,H inside Li Li (He He (H H) ...) layers:
// ring = H H | Li Li | He He | H H | He He (symmetric around the plus gap)
// Reactions: 3 → 17 → 32 → 53 → ... values H→He→B→C→N→...
// ---------------------------------------------------------------------------

#[test]
fn wiki_chain_reaction_score_sequence() {
    // Symmetric layers around the fused pair (inner → outer):
    //   pair H(1); then Li(3), He(2), H(1), He(2)
    // Ring laid out so the plus goes between the two H at the centre:
    // [He, H, He, Li, H, H, Li, He, H, He]
    let state = GameState::new(ring(&[2, 1, 2, 3, 1, 1, 3, 2, 1, 2]), Atom::PLUS);
    // gap 4 = between idx4(H) and idx5(H)
    let next = state.apply_action(&Action::Insert { gap_index: 4 }).unwrap();

    // Reaction 1: H+H → He, +3 (total 3)
    // Reaction 2: Li,Li (3 >= 2) → B, Sr=floor(2*3)=6, B=2*2*(3-2+1)=8 (total 17)
    // Reaction 3: He,He (2 < 5) → C, floor(2.5*6)=15 (total 32)
    // Reaction 4: H,H (1 < 6) → N, floor(3*7)=21 (total 53)
    // Reaction 5: He,He (2 < 7) → O, floor(3.5*8)=28 (total 81)
    assert_eq!(next.score, 81);
    assert_eq!(values(&next.ring), vec![8], "everything chains into one O(8)");
}

#[test]
fn plus_two_reaction_raises_value_to_outer_plus_two() {
    // pair He(2) inside F(9) F(9): He+He → Li(3); outer 9 >= 3 → value 9+2=11
    let state = GameState::new(ring(&[9, 2, 2, 9]), Atom::PLUS);
    let next = state.apply_action(&Action::Insert { gap_index: 1 }).unwrap();
    assert_eq!(values(&next.ring), vec![11]);
}

// ---------------------------------------------------------------------------
// Rule 5: Minus takes ONE atom which becomes the held centre atom; it can be
// converted to a Plus.
// ---------------------------------------------------------------------------

#[test]
fn minus_takes_one_atom_into_hand() {
    let state = GameState::new(ring(&[1, 7, 3]), Atom::MINUS);
    let next = state.apply_action(&Action::MinusTake { target_index: 1 }).unwrap();
    assert_eq!(values(&next.ring), vec![1, 3], "exactly ONE atom leaves the ring");
    assert_eq!(next.player_atom, Atom::new(7), "taken atom is now held");
    assert!(next.held_from_minus);
    assert_eq!(next.score, 0, "taking an atom scores nothing");
}

#[test]
fn held_atom_converts_to_plus_and_can_fuse() {
    let state = GameState::new(ring(&[5, 5, 9, 7]), Atom::MINUS);
    let took = state.apply_action(&Action::MinusTake { target_index: 3 }).unwrap();
    let plus = took.apply_action(&Action::ConvertToPlus).unwrap();
    assert!(plus.player_atom.is_plus());
    assert!(!plus.held_from_minus);
    let fused = plus.apply_action(&Action::Insert { gap_index: 0 }).unwrap();
    assert_eq!(values(&fused.ring), vec![6, 9]);
}

#[test]
fn convert_to_plus_illegal_without_minus_take() {
    let state = GameState::new(ring(&[1, 2, 3]), Atom::new(2));
    assert!(state.apply_action(&Action::ConvertToPlus).is_err());
}

#[test]
fn minus_take_can_trigger_parked_plus() {
    // 5 + 5 with a 9 in between one side: removing the 9 makes + see 5|5.
    // Ring: [5, -1, 9, 5] — take the 9 → [5, +, 5] → fires → [6]
    let state = GameState::new(ring(&[5, -1, 9, 5]), Atom::MINUS);
    let next = state.apply_action(&Action::MinusTake { target_index: 2 }).unwrap();
    assert_eq!(values(&next.ring), vec![6]);
    assert_eq!(next.player_atom, Atom::new(9));
}

// ---------------------------------------------------------------------------
// Rule 6: Dark Plus.
// ---------------------------------------------------------------------------

#[test]
fn dark_plus_fuses_any_two_neighbours() {
    // S(16) and B(5) → K(19) per the wiki example.
    let state = GameState::new(ring(&[16, 5, 1]), Atom::DARK_PLUS);
    let next = state.apply_action(&Action::Insert { gap_index: 0 }).unwrap();
    assert!(next.ring.iter().any(|a| a.value == 19));
}

// ---------------------------------------------------------------------------
// Rule 8: 18-atom field, overflow = game over.
// ---------------------------------------------------------------------------

#[test]
fn nineteenth_atom_is_game_over() {
    let state = GameState::new(ring(&vec![5; 18]), Atom::new(7));
    let next = state.apply_action(&Action::Insert { gap_index: 0 }).unwrap();
    assert!(next.is_game_over());
}

#[test]
fn no_actions_legal_after_game_over() {
    let state = GameState::new(ring(&vec![5; 19]), Atom::new(7));
    assert!(state.is_game_over());
    assert!(!state.is_legal_action(&Action::Insert { gap_index: 0 }));
}

// ---------------------------------------------------------------------------
// Insert geometry: gap g sits between ring[g] and ring[g+1].
// ---------------------------------------------------------------------------

#[test]
fn insert_gap_convention_matches_cv_layer() {
    let state = GameState::new(ring(&[10, 20, 30]), Atom::new(99));
    let next = state.apply_action(&Action::Insert { gap_index: 1 }).unwrap();
    assert_eq!(values(&next.ring), vec![10, 20, 99, 30]);
    let next = state.apply_action(&Action::Insert { gap_index: 2 }).unwrap();
    assert_eq!(values(&next.ring), vec![10, 20, 30, 99], "wrap-around gap");
}
