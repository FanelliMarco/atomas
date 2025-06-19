//! Template matching module

pub mod loader;
pub mod matcher;

pub use loader::TemplateLoader;
pub use matcher::TemplateMatcher;

use opencv::core::Mat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Template data structure
#[derive(Debug, Clone)]
pub struct Template {
    pub name: String,
    pub image: Mat,
    pub metadata: HashMap<String, String>,
}

impl Template {
    pub fn new(name: String, image: Mat) -> Self {
        Self {
            name,
            image,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Template matching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub threshold: f64,
    pub max_detections_per_template: usize,
    pub nms_threshold: f64,
    pub scale_factors: Vec<f64>,
    pub use_normalized_correlation: bool,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            threshold: 0.7,
            max_detections_per_template: 10,
            nms_threshold: 0.5,
            scale_factors: vec![1.0],
            use_normalized_correlation: true,
        }
    }
}
