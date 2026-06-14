//! Decision → screen-coordinate mapping for the REAL Atomas gestures.
//!
//! Real UI gestures (one tap per loop iteration):
//! - Regular atom / Plus / Dark Plus in the centre → tap the GAP you want it
//!   thrown into.
//! - Minus in the centre → tap THE ATOM you want pulled into your hand
//!   (exactly one atom leaves the ring; it becomes the next centre atom).
//! - Holding a minus-taken atom → tap the CENTRE to convert it to a Plus,
//!   or tap a gap to throw it back in.
//! - Neutrino in the centre → tap the atom to copy.

use crate::bbox::BBox;
use crate::detection::DetectionResult;
use atomas_core::Element;
use serde::{Deserialize, Serialize};

/// A decision to perform an action in the game. Mirrors
/// `atomas_core::Action` plus a legacy `Remove` used by the SimpleSolver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    /// Throw the centre atom into a gap between ring elements.
    Insert { gap_index: usize },
    /// Minus atom: tap ring atom `target_index` to pull it into the hand.
    MinusTake { target_index: usize },
    /// Holding a minus-taken atom: tap the centre to convert it to a Plus.
    ConvertToPlus,
    /// Neutrino: tap ring atom `target_index` to copy it.
    NeutrinoCopy { target_index: usize },
    /// Legacy: tap an atom directly (only used by the placeholder solver).
    Remove { atom_index: usize },
}

/// Screen coordinates for executing a decision.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ActionCoordinates {
    pub x: i32,
    pub y: i32,
}

impl ActionCoordinates {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Maps a Decision to concrete screen coordinates.
pub fn map_decision_to_coordinates(
    decision: &Decision,
    detection_result: &DetectionResult,
) -> anyhow::Result<ActionCoordinates> {
    match decision {
        Decision::Insert { gap_index } => {
            calculate_gap_coordinates(&detection_result.ring_elements, *gap_index)
        }
        Decision::MinusTake { target_index } | Decision::NeutrinoCopy { target_index } => {
            get_atom_coordinates(&detection_result.ring_elements, *target_index)
        }
        Decision::ConvertToPlus => {
            // Tap the held atom in the middle of the ring. Prefer the
            // detected player-atom bbox; fall back to the ring centroid.
            if let Some((_, bbox)) = &detection_result.player_atom {
                let c = bbox.center();
                Ok(ActionCoordinates::new(c.x, c.y))
            } else {
                ring_centroid(&detection_result.ring_elements)
            }
        }
        Decision::Remove { atom_index } => {
            get_atom_coordinates(&detection_result.ring_elements, *atom_index)
        }
    }
}

/// The geometric centre of the ring (where the centre atom sits).
fn ring_centroid(ring_elements: &[(Element, BBox)]) -> anyhow::Result<ActionCoordinates> {
    if ring_elements.is_empty() {
        anyhow::bail!("Cannot compute ring centroid: ring is empty");
    }
    let (mut sx, mut sy) = (0i64, 0i64);
    for (_, bbox) in ring_elements {
        let c = bbox.center();
        sx += c.x as i64;
        sy += c.y as i64;
    }
    let n = ring_elements.len() as i64;
    Ok(ActionCoordinates::new((sx / n) as i32, (sy / n) as i32))
}

/// Midpoint of the gap between ring[g] and ring[(g + 1) % n].
///
/// IMPORTANT: this convention (gap g = between atom g and atom g+1) must
/// match `atomas_core::Action::Insert`, which inserts at position g + 1.
/// Both layers agree; do not change one without the other.
fn calculate_gap_coordinates(
    ring_elements: &[(Element, BBox)],
    gap_index: usize,
) -> anyhow::Result<ActionCoordinates> {
    let n = ring_elements.len();

    if n == 0 {
        anyhow::bail!("Cannot calculate gap coordinates: ring is empty");
    }
    if n == 1 {
        // One atom → one gap: tap diametrically opposite is unnecessary;
        // tapping anywhere beside the atom works. Use a point offset from
        // the atom towards the screen centre mirror.
        let c = ring_elements[0].1.center();
        return Ok(ActionCoordinates::new(c.x, c.y - 120));
    }

    let gap_idx = gap_index % n;
    let atom1_idx = gap_idx;
    let atom2_idx = (gap_idx + 1) % n;

    let center1 = ring_elements[atom1_idx].1.center();
    let center2 = ring_elements[atom2_idx].1.center();

    // Tap ON the ring circle at the angular midpoint between the two atoms —
    // NOT the straight-line chord midpoint. With few atoms the chord midpoint
    // collapses toward the ring centre (2 opposite atoms → it IS the centre,
    // i.e. the player atom, and the tap becomes a no-op). Angles must advance
    // in the same rotational direction as the angular sort used to order
    // ring_elements (ascending atan2 around the centroid).
    let (ccx, ccy) = {
        let c = ring_centroid(ring_elements)?;
        (c.x as f64, c.y as f64)
    };
    let radius = {
        let mut sum = 0.0f64;
        for (_, bbox) in ring_elements {
            let c = bbox.center();
            sum += ((c.x as f64 - ccx).powi(2) + (c.y as f64 - ccy).powi(2)).sqrt();
        }
        sum / n as f64
    };

    let a1 = (center1.y as f64 - ccy).atan2(center1.x as f64 - ccx);
    let a2 = (center2.y as f64 - ccy).atan2(center2.x as f64 - ccx);
    let delta = (a2 - a1).rem_euclid(std::f64::consts::TAU);
    let gap_angle = a1 + delta / 2.0;

    let gap_x = (ccx + radius * gap_angle.cos()).round() as i32;
    let gap_y = (ccy + radius * gap_angle.sin()).round() as i32;

    log::debug!(
        "INSERT gap: gap_index={} between atoms {} and {} -> ({}, {}) [on-circle]",
        gap_index, atom1_idx, atom2_idx, gap_x, gap_y
    );

    Ok(ActionCoordinates::new(gap_x, gap_y))
}

/// Centre of a specific ring atom.
fn get_atom_coordinates(
    ring_elements: &[(Element, BBox)],
    atom_index: usize,
) -> anyhow::Result<ActionCoordinates> {
    if ring_elements.is_empty() {
        anyhow::bail!("Cannot get atom coordinates: ring is empty");
    }
    if atom_index >= ring_elements.len() {
        anyhow::bail!(
            "Invalid atom_index: {} (ring has {} atoms)",
            atom_index,
            ring_elements.len()
        );
    }

    let center = ring_elements[atom_index].1.center();
    Ok(ActionCoordinates::new(center.x, center.y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomas_core::elements::Data;
    use crate::bbox::BBox;

    fn mock_ring_elements() -> Vec<(Element<'static>, BBox)> {
        let data = Data::default();
        vec![
            (data.elements[0].clone(), BBox::new(100, 100, 50, 50, 1.0)),
            (data.elements[1].clone(), BBox::new(200, 100, 50, 50, 1.0)),
            (data.elements[2].clone(), BBox::new(200, 200, 50, 50, 1.0)),
            (data.elements[3].clone(), BBox::new(100, 200, 50, 50, 1.0)),
        ]
    }

    #[test]
    fn test_gap_coordinates() {
        let ring = mock_ring_elements();
        let coords = calculate_gap_coordinates(&ring, 0).unwrap();
        assert_eq!(coords.x, 175);
        let coords = calculate_gap_coordinates(&ring, 3).unwrap();
        assert!(coords.x > 0);
    }

    #[test]
    fn test_atom_coordinates() {
        let ring = mock_ring_elements();
        let coords = get_atom_coordinates(&ring, 0).unwrap();
        assert_eq!(coords.x, 125);
        assert_eq!(coords.y, 125);
    }

    #[test]
    fn test_centroid_used_for_convert() {
        let ring = mock_ring_elements();
        let c = ring_centroid(&ring).unwrap();
        assert_eq!(c.x, 162);
        assert_eq!(c.y, 162);
    }

    #[test]
    fn test_empty_ring_error() {
        let ring: Vec<(Element, BBox)> = vec![];
        assert!(calculate_gap_coordinates(&ring, 0).is_err());
        assert!(get_atom_coordinates(&ring, 0).is_err());
    }
}
