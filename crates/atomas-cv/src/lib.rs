//! Atomas Computer Vision Library
//! 
//! A multi-level abstraction system for game state detection using OpenCV.

pub mod bbox;
pub mod template;
pub mod detection;
pub mod utils;

// Re-export commonly used types
pub use bbox::{BBox, BBoxCollection};
pub use detection::{GameStateDetector, DetectionConfig, DetectionResult};
pub use template::{TemplateLoader, TemplateMatcher};

// Re-export opencv-match for convenience
pub use opencv_match::prelude::*;

// Error handling
pub type Result<T> = anyhow::Result<T>;

/// Core traits for the CV system
pub mod traits {
    use super::*;
    use opencv::core::Mat;

    /// Trait for objects that can be detected in images
    pub trait Detectable {
        fn get_templates(&self) -> Vec<String>;
        fn get_color(&self) -> (u8, u8, u8);
        fn get_name(&self) -> &str;
    }

    /// Trait for template matching implementations
    pub trait TemplateMatchable {
        fn match_template(&self, image: &Mat, template: &Mat, threshold: f64) -> Result<Vec<BBox>>;
    }

    /// Trait for non-maximum suppression implementations
    pub trait NonMaxSuppression {
        fn apply_nms(&self, boxes: Vec<BBox>, threshold: f64) -> Vec<BBox>;
    }
}
