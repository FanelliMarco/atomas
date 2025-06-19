//! High-level detection module

pub mod config;
pub mod detector;

pub use config::DetectionConfig;
pub use detector::{GameStateDetector, DetectionResult};
