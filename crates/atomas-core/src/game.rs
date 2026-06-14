//! Atomas game engine — rebuilt to match the REAL game rules.
//!
//! Key rules (verified against the actual game / Atomas wiki):
//!
//! 1. Regular atoms NEVER fuse on their own. Two adjacent equal atoms just sit
//!    there. Fusion happens ONLY through a Plus atom.
//! 2. A Plus atom is *thrown into a gap* like any other atom (it does not
//!    replace an atom). If the two atoms adjacent to that gap are equal
//!    regular atoms, a fusion fires immediately; otherwise the Plus stays in
//!    the ring and may fire later, the moment some move makes its two
//!    neighbours equal.
//! 3. Fusion chains outward while the ring stays symmetric around the fused
//!    atom:
//!      - outer pair value  < current centre value  → centre value += 1
//!      - outer pair value >= current centre value  → centre value = outer + 2
//! 4. Scoring (wiki-verified):
//!      - reaction multiplier  M = 1 + 0.5 * r   (r = 1 for the first
//!        reaction of a combo, incremented for every further reaction)
//!      - each reaction scores floor(M * (Z + 1)) where Z is the current
//!        centre value *before* the reaction
//!      - a "+2" reaction (outer >= centre) additionally scores
//!        floor(2 * M * (Zo - Z + 1)) where Zo is the outer pair value
//! 5. A Minus atom does NOT remove two atoms. Tapping an atom with a Minus
//!    *pulls it out of the ring into your hand*: it becomes the next atom you
//!    throw. While holding it you may instead tap the centre once more to
//!    convert it into a Plus atom (which you then throw normally).
//! 6. Dark Plus (score > 750): fuses ANY two neighbours; result is
//!    max(left, right) + 3; then a normal chain may continue. Scores only the
//!    average of the two values.
//! 7. Neutrino (score > 1500): tap an atom to turn the Neutrino into a copy
//!    of it (it becomes your held atom; it cannot be converted to a Plus).
//! 8. The field holds at most 18 atoms; a move that leaves more than 18 atoms
//!    in the ring ends the game.

use crate::elements::ElementType;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Maximum number of atoms the field can hold. Exceeding this is game over.
pub const MAX_RING_SIZE: usize = 18;

/// Represents an atom (ring or centre).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Atom {
    pub value: i16,
}

impl Atom {
    pub const PLUS: Atom = Atom { value: -1 };
    pub const MINUS: Atom = Atom { value: -2 };
    pub const DARK_PLUS: Atom = Atom { value: -3 };
    pub const NEUTRINO: Atom = Atom { value: -4 };

    pub fn new(value: i16) -> Self {
        Self { value }
    }

    pub fn from_element_type(element_type: ElementType) -> Self {
        Self { value: element_type.to_numeric() }
    }

    pub fn is_plus(&self) -> bool {
        self.value == -1
    }

    pub fn is_minus(&self) -> bool {
        self.value == -2
    }

    pub fn is_dark_plus(&self) -> bool {
        self.value == -3
    }

    pub fn is_neutrino(&self) -> bool {
        self.value == -4
    }

    pub fn is_special(&self) -> bool {
        self.value < 0
    }

    pub fn is_regular(&self) -> bool {
        self.value > 0
    }
}

/// A game action, matching the real UI gestures one-to-one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Throw the centre atom (regular, Plus or Dark Plus) into a gap.
    /// `gap_index` g is the gap between ring[g] and ring[(g + 1) % n].
    Insert { gap_index: usize },
    /// With a Minus in the centre: tap ring[target_index]; that atom leaves
    /// the ring and becomes the held centre atom.
    MinusTake { target_index: usize },
    /// While holding an atom taken with a Minus: tap the centre to convert
    /// the held atom into a Plus.
    ConvertToPlus,
    /// With a Neutrino in the centre: tap ring[target_index]; the Neutrino
    /// becomes a copy of it (held, NOT convertible to Plus).
    NeutrinoCopy { target_index: usize },
}

/// The complete game state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameState {
    /// The ring of atoms (circular arrangement, index 0 .. n-1).
    pub ring: Vec<Atom>,
    /// The current centre atom (the atom to be played).
    pub player_atom: Atom,
    /// Game score.
    pub score: u64,
    /// Number of atoms consumed from the spawner (drives the spawn range,
    /// which shifts up every 40 moves in the real game).
    pub moves: u32,
    /// True iff `player_atom` was pulled out of the ring with a Minus, which
    /// makes `ConvertToPlus` legal.
    pub held_from_minus: bool,
}

impl GameState {
    pub fn new(ring: Vec<Atom>, player_atom: Atom) -> Self {
        Self { ring, player_atom, score: 0, moves: 0, held_from_minus: false }
    }

    /// Typical opening board: six atoms drawn from H/He/Li.
    pub fn test_state() -> Self {
        Self::new(
            vec![
                Atom::new(1),
                Atom::new(2),
                Atom::new(3),
                Atom::new(1),
                Atom::new(2),
                Atom::new(3),
            ],
            Atom::new(1),
        )
    }

    pub fn ring_size(&self) -> usize {
        self.ring.len()
    }

    /// Game over when the ring has overflowed the field.
    pub fn is_game_over(&self) -> bool {
        self.ring.len() > MAX_RING_SIZE
    }

    pub fn is_valid_gap(&self, gap_index: usize) -> bool {
        // An empty ring (possible after a Minus takes the last atom) has one
        // conceptual gap.
        gap_index < self.ring.len() || (self.ring.is_empty() && gap_index == 0)
    }

    pub fn is_valid_index(&self, index: usize) -> bool {
        index < self.ring.len()
    }

    pub fn is_legal_action(&self, action: &Action) -> bool {
        if self.is_game_over() {
            return false;
        }
        match action {
            Action::Insert { gap_index } => {
                // Anything you can throw goes into a gap: regular atoms,
                // Plus, Dark Plus, and atoms held after a Minus/Neutrino.
                let throwable = self.player_atom.is_regular()
                    || self.player_atom.is_plus()
                    || self.player_atom.is_dark_plus();
                throwable && self.is_valid_gap(*gap_index)
            }
            Action::MinusTake { target_index } => {
                self.player_atom.is_minus()
                    && self.is_valid_index(*target_index)
                    // taking the last atom would empty the ring; the real game
                    // allows it, and you then throw the atom back in — we allow
                    // it too as long as the ring is non-empty
                    && !self.ring.is_empty()
            }
            Action::ConvertToPlus => self.held_from_minus && self.player_atom.is_regular(),
            Action::NeutrinoCopy { target_index } => {
                self.player_atom.is_neutrino()
                    && self.is_valid_index(*target_index)
                    && self.ring[*target_index].is_regular()
            }
        }
    }

    pub fn apply_action(&self, action: &Action) -> Result<GameState, String> {
        if !self.is_legal_action(action) {
            return Err(format!("Illegal action: {:?}", action));
        }
        match action {
            Action::Insert { gap_index } => Ok(self.apply_insert(*gap_index)),
            Action::MinusTake { target_index } => Ok(self.apply_minus_take(*target_index)),
            Action::ConvertToPlus => Ok(self.apply_convert_to_plus()),
            Action::NeutrinoCopy { target_index } => Ok(self.apply_neutrino_copy(*target_index)),
        }
    }

    /// Throw the centre atom into a gap, then resolve any reactions.
    fn apply_insert(&self, gap_index: usize) -> GameState {
        let mut ring = self.ring.clone();
        let insert_pos = gap_index + 1; // between ring[g] and ring[g+1]
        if insert_pos >= ring.len() {
            ring.push(self.player_atom);
        } else {
            ring.insert(insert_pos, self.player_atom);
        }

        let (ring, gained) = resolve_reactions(ring);

        GameState {
            ring,
            // The next centre atom comes from the spawner; the solver's
            // chance node replaces this placeholder with each possible spawn.
            player_atom: Atom::new(0),
            score: self.score + gained,
            moves: self.moves + 1,
            held_from_minus: false,
        }
    }

    /// Pull an atom out of the ring with a Minus; it becomes the held atom.
    /// Removing an atom can make an existing in-ring Plus fire, so reactions
    /// are resolved afterwards.
    fn apply_minus_take(&self, target_index: usize) -> GameState {
        let mut ring = self.ring.clone();
        let taken = ring.remove(target_index);
        let (ring, gained) = resolve_reactions(ring);

        GameState {
            ring,
            player_atom: taken,
            score: self.score + gained,
            moves: self.moves + 1,
            held_from_minus: taken.is_regular(),
        }
    }

    /// Convert the held (minus-taken) atom into a Plus.
    fn apply_convert_to_plus(&self) -> GameState {
        GameState {
            ring: self.ring.clone(),
            player_atom: Atom::PLUS,
            score: self.score,
            moves: self.moves, // free tap, no spawn consumed
            held_from_minus: false,
        }
    }

    /// Turn the Neutrino into a copy of a ring atom (held).
    fn apply_neutrino_copy(&self, target_index: usize) -> GameState {
        GameState {
            ring: self.ring.clone(),
            player_atom: self.ring[target_index],
            score: self.score,
            moves: self.moves + 1,
            held_from_minus: false, // a copied atom canNOT become a Plus
        }
    }
}

/// Resolve every reaction currently pending on the ring (a Plus or Dark Plus
/// whose neighbours satisfy its trigger), including outward chains and
/// cascades into other Pluses. Returns the settled ring and the score gained.
///
/// The combo multiplier `M = 1 + 0.5 * r` keeps growing across cascades.
pub fn resolve_reactions(mut ring: Vec<Atom>) -> (Vec<Atom>, u64) {
    let mut total: u64 = 0;
    let mut r: u32 = 0; // reaction counter within this combo

    loop {
        let trigger = find_trigger(&ring);
        let Some((plus_idx, dark)) = trigger else { break };
        let gained = fuse_at(&mut ring, plus_idx, dark, &mut r);
        total += gained;
    }

    (ring, total)
}

/// Find a Plus that can fire: a normal Plus needs equal regular neighbours; a
/// Dark Plus fires between any two atoms. Requires at least 3 atoms so that
/// the neighbours are distinct. Scans counter-clockwise-first like the game.
fn find_trigger(ring: &[Atom]) -> Option<(usize, bool)> {
    let n = ring.len();
    if n < 3 {
        return None;
    }
    for i in 0..n {
        let a = ring[i];
        if !(a.is_plus() || a.is_dark_plus()) {
            continue;
        }
        let l = ring[(i + n - 1) % n];
        let rgt = ring[(i + 1) % n];
        if a.is_dark_plus() {
            // Dark Plus fuses any two distinct neighbours (specials too are
            // consumed in the real game; we require at least one regular to
            // avoid degenerate special-special fusions the CV never sees).
            if l.is_regular() || rgt.is_regular() {
                return Some((i, true));
            }
        } else if l.is_regular() && rgt.is_regular() && l.value == rgt.value {
            return Some((i, false));
        }
    }
    None
}

/// Perform one fusion (with its outward chain) centred on the plus at `idx`.
fn fuse_at(ring: &mut Vec<Atom>, idx: usize, dark: bool, r: &mut u32) -> u64 {
    let n = ring.len();
    // Rotate the ring so the plus sits at index 1 with its left neighbour at
    // index 0. Rotation is semantics-preserving on a circle.
    let left = (idx + n - 1) % n;
    ring.rotate_left(left);
    // ring[0] = left neighbour, ring[1] = plus, ring[2] = right neighbour

    let lv = ring[0];
    let rv = ring[2];

    let mut gained: u64 = 0;
    *r += 1;
    let m = 1.0 + 0.5 * (*r as f64);

    let mut center: i16;
    if dark {
        // result = max + 3; score = average of the two values
        center = lv.value.max(rv.value) + 3;
        gained += (((lv.value.max(0) + rv.value.max(0)) as f64) / 2.0).floor() as u64;
    } else {
        let z = lv.value;
        center = z + 1;
        gained += (m * (z as f64 + 1.0)).floor() as u64;
    }

    // Remove left, plus, right; put the fused atom where the reaction was.
    ring.drain(0..3);
    ring.insert(0, Atom::new(center));

    // Outward chain: with the centre at index 0, the outer pair is
    // ring[1] (clockwise) and ring[last] (counter-clockwise).
    while ring.len() >= 3 {
        let outer_cw = ring[1];
        let outer_ccw = ring[ring.len() - 1];
        if !(outer_cw.is_regular()
            && outer_ccw.is_regular()
            && outer_cw.value == outer_ccw.value)
        {
            break;
        }
        let zo = outer_cw.value;
        let z = center;
        *r += 1;
        let m = 1.0 + 0.5 * (*r as f64);

        gained += (m * (z as f64 + 1.0)).floor() as u64;
        if zo >= z {
            // "+2" reaction: bonus and the centre jumps to outer + 2
            gained += (2.0 * m * ((zo - z + 1) as f64)).floor() as u64;
            center = zo + 2;
        } else {
            center += 1;
        }

        let last = ring.len() - 1;
        ring.remove(last);
        ring.remove(1);
        ring[0] = Atom::new(center);
    }

    ring[0] = Atom::new(center);
    gained
}
