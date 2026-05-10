use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    pub circle_detection: CircleDetectionConfig,
    pub elements_file: PathBuf,
    pub output_dir: PathBuf,
    pub color_matching: ColorMatchingConfig,
    pub player_atom_detection: PlayerAtomConfig,
    pub ring_detection: RingDetectionConfig,
    pub visualization: VisualizationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircleDetectionConfig {
    pub dp: f64,
    pub min_dist: f64,
    pub param1: f64,
    pub param2: f64,
    pub min_radius: i32,
    pub max_radius: i32,
    pub blur_kernel_size: i32,
    pub preprocessing: PreprocessingMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreprocessingMethod {
    None,
    GaussianBlur,
    MedianBlur,
    BilateralFilter,
    CLAHE,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorMatchingConfig {
    pub method: ColorMatchMethod,
    pub tolerance: f64,
    pub use_hsv: bool,
    pub hue_weight: f64,
    pub saturation_weight: f64,
    pub value_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMatchMethod {
    Mean,
    Median,
    DominantColor,
    Combined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAtomConfig {
    pub center_tolerance: f64,
    pub size_factor_range: (f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingDetectionConfig {
    pub max_ring_elements: usize,
    pub min_ring_radius: f64,
    pub max_ring_radius: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    pub draw_circles: bool,
    pub draw_centers: bool,
    pub draw_labels: bool,
    pub draw_confidence: bool,
    pub save_intermediate: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            circle_detection: CircleDetectionConfig::default(),
            elements_file: "assets/txt/elements.txt".into(),
            output_dir: "assets/png/outputs".into(),
            color_matching: ColorMatchingConfig::default(),
            player_atom_detection: PlayerAtomConfig {
                center_tolerance: 0.15,
                size_factor_range: (1.2, 2.0),
            },
            ring_detection: RingDetectionConfig {
                max_ring_elements: 18,
                min_ring_radius: 100.0,
                max_ring_radius: 400.0,
            },
            visualization: VisualizationConfig {
                draw_circles: true,
                draw_centers: true,
                draw_labels: true,
                draw_confidence: true,
                save_intermediate: false,
            },
        }
    }
}

impl Default for CircleDetectionConfig {
    fn default() -> Self {
        Self {
            dp: 1.2,
            min_dist: 35.0,
            param1: 50.0,
            param2: 28.0,
            min_radius: 20,
            max_radius: 80,
            blur_kernel_size: 9,
            preprocessing: PreprocessingMethod::GaussianBlur,
        }
    }
}

impl Default for ColorMatchingConfig {
    fn default() -> Self {
        Self {
            method: ColorMatchMethod::Mean,
            tolerance: 0.20,
            use_hsv: true,
            hue_weight: 3.0,
            saturation_weight: 1.5,
            value_weight: 0.2,
        }
    }
}

impl DetectionConfig {
    pub fn for_small_atoms() -> Self {
        let mut c = Self::default();
        c.circle_detection.min_radius = 10;
        c.circle_detection.max_radius = 40;
        c.circle_detection.min_dist = 15.0;
        c.circle_detection.param2 = 20.0;
        c
    }

    pub fn for_large_atoms() -> Self {
        let mut c = Self::default();
        c.circle_detection.min_radius = 40;
        c.circle_detection.max_radius = 150;
        c.circle_detection.min_dist = 50.0;
        c
    }

    pub fn high_sensitivity() -> Self {
        let mut c = Self::default();
        c.circle_detection.param1 = 40.0;
        c.circle_detection.param2 = 20.0;
        c.circle_detection.min_dist = 25.0;
        c
    }

    pub fn low_sensitivity() -> Self {
        let mut c = Self::default();
        c.circle_detection.param1 = 70.0;
        c.circle_detection.param2 = 40.0;
        c.circle_detection.min_dist = 50.0;
        c
    }

    pub fn accurate_color_matching() -> Self {
        let mut c = Self::default();
        c.color_matching.use_hsv = true;
        c.color_matching.method = ColorMatchMethod::Mean;
        c.color_matching.tolerance = 0.20;
        c.color_matching.hue_weight = 3.0;
        c.color_matching.saturation_weight = 1.5;
        c.color_matching.value_weight = 0.2;
        c.circle_detection.param2 = 28.0;
        c.circle_detection.min_dist = 35.0;
        c
    }

    pub fn high_contrast() -> Self {
        let mut c = Self::default();
        c.color_matching.use_hsv = false;
        c.color_matching.tolerance = 40.0;
        c.color_matching.method = ColorMatchMethod::Median;
        c
    }
}
