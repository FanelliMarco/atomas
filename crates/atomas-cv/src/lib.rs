//! Atomas Computer Vision Library
//!
//! Circle-based game state detection using OpenCV HoughCircles.

pub mod action;
pub mod bbox;
pub mod circle;
pub mod detection;
pub mod overlay;
pub mod template;
pub mod utils;

// Re-export commonly used types
pub use action::{Decision, ActionCoordinates, map_decision_to_coordinates};
pub use bbox::{BBox, BBoxCollection};
pub use circle::{Circle, CircleDetector, DetectedCircle};
pub use detection::{DetectionConfig, DetectionResult, GameStateDetector, ColorMatchingConfig};
pub use overlay::{draw_action_overlay, draw_decision_on_detection};

// Error type alias
pub type Result<T> = anyhow::Result<T>;

