//! Detection configuration

use crate::template::TemplateConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    pub template_config: TemplateConfig,
    pub template_dirs: Vec<PathBuf>,
    pub elements_file: PathBuf,
    pub output_dir: PathBuf,
    pub global_nms_threshold: f64,
    pub player_atom_detection: PlayerAtomConfig,
    pub ring_detection: RingDetectionConfig,
    pub visualization: VisualizationConfig,
}

/// Player atom detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAtomConfig {
    pub center_tolerance: f64,
    pub size_threshold: (u32, u32),
}

/// Ring detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingDetectionConfig {
    pub max_ring_elements: usize,
    pub angle_tolerance: f64,
    pub radius_range: (f64, f64),
}

/// Visualization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    pub draw_bboxes: bool,
    pub draw_labels: bool,
    pub draw_confidence: bool,
    pub save_intermediate: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            template_config: TemplateConfig::default(),
            template_dirs: vec!["assets/png".into()],
            elements_file: "assets/txt/elements.txt".into(),
            output_dir: "assets/png/outputs".into(),
            global_nms_threshold: 0.2,
            player_atom_detection: PlayerAtomConfig {
                center_tolerance: 0.1,
                size_threshold: (30, 200),
            },
            ring_detection: RingDetectionConfig {
                max_ring_elements: 18,
                angle_tolerance: 0.2,
                radius_range: (100.0, 400.0),
            },
            visualization: VisualizationConfig {
                draw_bboxes: true,
                draw_labels: true,
                draw_confidence: true,
                save_intermediate: false,
            },
        }
    }
}

impl DetectionConfig {
    /// Configuration optimized for uneven lighting (gradient-based)
    pub fn for_uneven_lighting() -> Self {
        let mut config = Self::default();
        config.template_config = TemplateConfig::gradient_matching();
        config.template_config.threshold = 0.55; // Slightly lower for gradient matching
        config
    }

    /// Configuration using squared difference with Laplacian
    pub fn with_sqdiff_laplacian() -> Self {
        let mut config = Self::default();
        config.template_config = TemplateConfig::sqdiff_laplacian();
        config
    }

    /// Configuration for edge-based matching
    pub fn with_edge_matching() -> Self {
        let mut config = Self::default();
        config.template_config = TemplateConfig::edge_matching();
        config
    }
}
