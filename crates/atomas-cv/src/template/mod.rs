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

/// Template matching method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchingMethod {
    /// Normalized cross-correlation (default, good for general use)
    CCorrNormed,
    /// Correlation coefficient (robust to linear lighting changes)
    CCoeffNormed,
    /// Squared difference (inverted: lower is better)
    SqDiffNormed,
    /// Raw squared difference
    SqDiff,
}

impl MatchingMethod {
    pub fn to_opencv(&self) -> i32 {
        use opencv::imgproc::*;
        match self {
            MatchingMethod::CCorrNormed => TM_CCORR_NORMED,
            MatchingMethod::CCoeffNormed => TM_CCOEFF_NORMED,
            MatchingMethod::SqDiffNormed => TM_SQDIFF_NORMED,
            MatchingMethod::SqDiff => TM_SQDIFF,
        }
    }

    pub fn is_inverted(&self) -> bool {
        matches!(self, MatchingMethod::SqDiff | MatchingMethod::SqDiffNormed)
    }
}

/// Preprocessing method for robust matching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreprocessingMethod {
    /// No preprocessing
    None,
    /// Histogram equalization (normalize brightness)
    HistogramEqualization,
    /// Laplacian gradient (edge-based matching, robust to lighting)
    Laplacian,
    /// Sobel gradient magnitude (edge-based)
    SobelMagnitude,
    /// Canny edges (binary edge matching)
    Canny,
    /// Adaptive histogram equalization (CLAHE)
    CLAHE,
}

/// Template matching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub threshold: f64,
    pub max_detections_per_template: usize,
    pub nms_threshold: f64,
    pub scale_factors: Vec<f64>,
    pub matching_method: MatchingMethod,
    pub preprocessing: PreprocessingMethod,
    pub preprocessing_params: PreprocessingParams,
}

/// Parameters for preprocessing methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingParams {
    /// Canny low threshold
    pub canny_low: f64,
    /// Canny high threshold
    pub canny_high: f64,
    /// Laplacian kernel size (must be odd: 1, 3, 5, 7, etc.)
    pub laplacian_ksize: i32,
    /// CLAHE clip limit
    pub clahe_clip_limit: f64,
    /// CLAHE tile grid size
    pub clahe_tile_size: (i32, i32),
}

impl Default for PreprocessingParams {
    fn default() -> Self {
        Self {
            canny_low: 50.0,
            canny_high: 150.0,
            laplacian_ksize: 3,
            clahe_clip_limit: 2.0,
            clahe_tile_size: (8, 8),
        }
    }
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            threshold: 0.65,
            max_detections_per_template: 18,
            nms_threshold: 0.2,
            scale_factors: vec![1.0],
            matching_method: MatchingMethod::CCoeffNormed,
            preprocessing: PreprocessingMethod::None,
            preprocessing_params: PreprocessingParams::default(),
        }
    }
}

impl TemplateConfig {
    /// Configuration for gradient-based matching (robust to lighting)
    pub fn gradient_matching() -> Self {
        Self {
            threshold: 0.6,
            matching_method: MatchingMethod::CCoeffNormed,
            preprocessing: PreprocessingMethod::Laplacian,
            ..Default::default()
        }
    }

    /// Configuration using squared difference with Laplacian
    pub fn sqdiff_laplacian() -> Self {
        Self {
            threshold: 0.15, // Lower threshold for inverted matching
            matching_method: MatchingMethod::SqDiffNormed,
            preprocessing: PreprocessingMethod::Laplacian,
            ..Default::default()
        }
    }

    /// Configuration for edge-based matching
    pub fn edge_matching() -> Self {
        Self {
            threshold: 0.6,
            matching_method: MatchingMethod::CCoeffNormed,
            preprocessing: PreprocessingMethod::SobelMagnitude,
            ..Default::default()
        }
    }
}
