//! High-level detection module

pub mod config;
pub mod detector;

pub use config::{DetectionConfig, ColorMatchingConfig};
pub use detector::{GameStateDetector, DetectionResult};
