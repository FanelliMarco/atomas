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

    /// IoU threshold for non-maximum suppression on detected circles.
    /// Two circles whose bounding boxes overlap by more than this ratio are
    /// considered duplicates and the one with the worse color match is dropped.
    /// Typical range: 0.2 (aggressive) â€“ 0.5 (permissive). 0.3 is a good default.
    pub nms_iou_threshold: f64,

    /// If > 0, NMS will additionally suppress any pair of circles whose
    /// centers are closer than `min(r_a, r_b) * nms_center_distance_ratio`
    /// pixels apart, regardless of bbox-IoU. This catches concentric circles
    /// at very different radii (e.g. a ring atom + a halo around it) that
    /// IoU alone may not flag. Set to 0.0 to disable.
    pub nms_center_distance_ratio: f64,

    /// Background rejection. Some boards render faint "ghost"/placement-hint
    /// circles whose interior is just the dark playfield. Hough finds them
    /// and, because their dark interior is close to dark elements like Carbon,
    /// they get mislabelled. To reject them we sample the playfield background
    /// colour from the image corners at runtime and drop any circle whose
    /// sampled color is within `bg_reject_threshold` (RGB euclidean distance)
    /// of that background. Set the threshold to 0.0 to disable.
    pub bg_reject_threshold: f64,
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
            nms_iou_threshold: 0.3,
            nms_center_distance_ratio: 0.6,
            // RGB euclidean distance. Ghost/hint circles sample within ~5-10
            // of the playfield background; the darkest real atom (Carbon)
            // sits ~20-25 away. 15 cleanly separates them. Raise toward 25 if
            // ghosts still leak through; lower if dark atoms get dropped.
            bg_reject_threshold: 15.0,
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
    /// Tuned for small atoms (e.g. ring elements at lower zoom).
    pub fn for_small_atoms() -> Self {
        let mut c = Self::default();
        c.circle_detection.min_radius = 10;
        c.circle_detection.max_radius = 40;
        c.circle_detection.min_dist = 15.0;
        c.circle_detection.param2 = 20.0;
        c
    }

    /// Tuned for large atoms (e.g. central player atom).
    pub fn for_large_atoms() -> Self {
        let mut c = Self::default();
        c.circle_detection.min_radius = 40;
        c.circle_detection.max_radius = 150;
        c.circle_detection.min_dist = 50.0;
        c
    }

    /// More permissive Hough thresholds â€” finds more circles but also more
    /// false positives. NMS downstream handles the duplicates.
    pub fn high_sensitivity() -> Self {
        let mut c = Self::default();
        c.circle_detection.param1 = 40.0;
        c.circle_detection.param2 = 20.0;
        c.circle_detection.min_dist = 25.0;
        c
    }

    /// Stricter Hough thresholds â€” only confident circles get through.
    pub fn low_sensitivity() -> Self {
        let mut c = Self::default();
        c.circle_detection.param1 = 70.0;
        c.circle_detection.param2 = 35.0;
        c.circle_detection.min_dist = 45.0;
        c
    }

    /// Baseline preset emphasizing color-match accuracy: brightness-normalized
    /// HSV comparison with tighter color tolerance. This is the first config
    /// `parser.rs` tries and the fallback if all others fail.
    ///
    /// Hough radius is clamped tightly:
    ///   - `min_radius: 24` filters small UI badges like the `+Sm` indicator
    ///     in the HUD (observed at r=23) without losing real atoms (r >= 25).
    ///   - `max_radius: 31` filters the larger halo that Hough sometimes
    ///     finds around the next-atom preview (observed at r ~ 45) and also
    ///     prevents CLAHE's stronger edges from latching an oversized circle
    ///     on a normal atom. Real ring atoms and the player atom come in at
    ///     r ~ 26-27, so this leaves margin while keeping circles atom-tight
    ///     (an inflated radius would also push color sampling into the
    ///     background).
    ///
    /// Edge handling for low-contrast atoms:
    ///   - CLAHE preprocessing + a lower `param2` accumulator threshold so
    ///     very dark atoms (Carbon ~59,59,59) sitting on the dark-red
    ///     playfield — almost zero luminance contrast — still produce a
    ///     strong enough Hough edge to be detected. Without this they are
    ///     silently missed (no circle, hence no label). The extra weak
    ///     circles this admits are cleaned up downstream by background
    ///     rejection, color matching, and NMS.
    pub fn accurate_color_matching() -> Self {
        let mut c = Self::default();
        c.color_matching.use_hsv = true;
        c.color_matching.tolerance = 0.18;
        c.color_matching.hue_weight = 3.0;
        c.color_matching.saturation_weight = 1.5;
        c.color_matching.value_weight = 0.2;
        c.circle_detection.min_radius = 24;
        c.circle_detection.max_radius = 31;
        c.circle_detection.preprocessing = PreprocessingMethod::CLAHE;
        c.circle_detection.param1 = 50.0;
        c.circle_detection.param2 = 18.0;
        c
    }

    /// Adds CLAHE preprocessing to bring out edges in low-contrast screenshots
    /// (e.g. when the game's dark vignette suppresses atom outlines). Useful
    /// when the default and high-sensitivity presets both miss circles.
    pub fn high_contrast() -> Self {
        let mut c = Self::default();
        c.circle_detection.preprocessing = PreprocessingMethod::CLAHE;
        c.circle_detection.param1 = 60.0;
        c.circle_detection.param2 = 30.0;
        c
    }
}
