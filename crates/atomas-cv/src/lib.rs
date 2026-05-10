//! Atomas Computer Vision Library
//!
//! Circle-based game state detection using OpenCV HoughCircles.

pub mod bbox;
pub mod circle;
pub mod detection;
pub mod template;
pub mod utils;

// Re-export commonly used types
pub use bbox::{BBox, BBoxCollection};
pub use circle::{Circle, CircleDetector, DetectedCircle};
pub use detection::{DetectionConfig, DetectionResult, GameStateDetector, ColorMatchingConfig};

// Error type alias
pub type Result<T> = anyhow::Result<T>;

