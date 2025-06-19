//! High-level game state detector using opencv-match

use super::config::DetectionConfig;
use crate::bbox::{BBox, BBoxCollection};
use crate::template::{TemplateLoader, TemplateMatcher};
use crate::traits::Detectable;
use crate::utils::ImageUtils;
use crate::Result;
use atomas_core::{elements::Data, Element};
use anyhow::Context;
use opencv::{
    core::{Mat, Point},
    imgproc::{self, LINE_8, FONT_HERSHEY_SIMPLEX},
    prelude::*,
};
use serde::Serialize;
use std::path::Path;

/// Game state detection result (Serialize only due to lifetime issues)
#[derive(Debug, Clone, Serialize)]
pub struct DetectionResult<'a> {
    pub ring_elements: Vec<(Element<'a>, BBox)>,
    pub player_atom: Option<(Element<'a>, BBox)>,
    pub all_detections: BBoxCollection,
    pub confidence_stats: DetectionStats,
}

/// Detection statistics
#[derive(Debug, Clone, Serialize)]
pub struct DetectionStats {
    pub total_detections: usize,
    pub ring_detections: usize,
    pub player_detections: usize,
    pub avg_confidence: f64,
    pub processing_time_ms: u64,
}

/// Implement Detectable trait for Element
impl<'a> Detectable for Element<'a> {
    fn get_templates(&self) -> Vec<String> {
        vec![
            self.name.to_string(),
            self.name.to_lowercase(),
            format!("_{}", self.name),
            format!("_{}", self.name.to_lowercase()),
        ]
    }

    fn get_color(&self) -> (u8, u8, u8) {
        self.rgb
    }

    fn get_name(&self) -> &str {
        self.name
    }
}

/// Main game state detector leveraging opencv-match
pub struct GameStateDetector {
    config: DetectionConfig,
    template_loader: TemplateLoader,
    template_matcher: TemplateMatcher,
}

impl GameStateDetector {
    /// Create new detector
    pub fn new(config: DetectionConfig) -> Result<Self> {
        let mut template_loader = TemplateLoader::new();
        for dir in &config.template_dirs {
            template_loader = template_loader.add_template_dir(dir);
        }

        let template_matcher = TemplateMatcher::new(config.template_config.clone());

        Ok(Self {
            config,
            template_loader,
            template_matcher,
        })
    }

    /// Detect game state from image file using opencv-match conversions
    pub fn detect_from_file<'a, P: AsRef<Path>>(
        &self,
        image_path: P,
        elements_data: &'a Data,
    ) -> Result<DetectionResult<'a>> {
        // Load using opencv-match for consistent handling
        let image = ImageUtils::load_grayscale(&image_path)
            .with_context(|| format!("Failed to load image: {:?}", image_path.as_ref()))?;

        let color_image = ImageUtils::load_color(&image_path)
            .with_context(|| format!("Failed to load color image: {:?}", image_path.as_ref()))?;

        self.detect_from_mat(&image, &color_image, elements_data)
    }

    /// Detect from image::RgbImage using opencv-match conversions
    pub fn detect_from_rgb_image<'a>(
        &self,
        rgb_image: &image::RgbImage,
        elements_data: &'a Data,
    ) -> Result<DetectionResult<'a>> {
        let color_mat = ImageUtils::rgb_to_mat(rgb_image)?;
        let grayscale_mat = opencv_match::convert::mat_to_grayscale(&color_mat, true)?;
        
        self.detect_from_mat(&grayscale_mat, &color_mat, elements_data)
    }

    /// Detect from image::RgbaImage using opencv-match conversions
    pub fn detect_from_rgba_image<'a>(
        &self,
        rgba_image: &image::RgbaImage,
        elements_data: &'a Data,
    ) -> Result<DetectionResult<'a>> {
        let rgba_mat = ImageUtils::rgba_to_mat(rgba_image)?;
        let grayscale_mat = opencv_match::convert::mat_to_grayscale(&rgba_mat, true)?;
        
        // Convert RGBA to RGB for color visualization
        let rgb_image = ImageUtils::rgba_to_rgb(rgba_image)?;
        let color_mat = ImageUtils::rgb_to_mat(&rgb_image)?;
        
        self.detect_from_mat(&grayscale_mat, &color_mat, elements_data)
    }

    /// Core detection from OpenCV Mat
    pub fn detect_from_mat<'a>(
        &self,
        image: &Mat,
        color_image: &Mat,
        elements_data: &'a Data,
    ) -> Result<DetectionResult<'a>> {
        let start_time = std::time::Instant::now();

        let mut all_detections = BBoxCollection::new();
        let mut element_bbox_pairs = Vec::new();

        // Process each element
        for element in &elements_data.elements {
            if let Some(template) = self.template_loader.load_template(element.name)? {
                let mut detections = self.template_matcher.match_single(image, &template)?;
                
                // Add element metadata to detections
                for bbox in detections.as_mut_slice() {
                    bbox.class_id = element.name.to_string();
                    bbox.color = element.get_color();
                    bbox.metadata.insert("element_type".to_string(), 
                        format!("{:?}", element.element_type));
                    
                    // Store element-bbox pairs for classification
                    element_bbox_pairs.push((element.clone(), bbox.clone()));
                }

                all_detections.extend(detections);
            }
        }

        // Apply global NMS
        all_detections = all_detections.apply_global_nms(self.config.global_nms_threshold);

        // Classify detections as ring elements or player atom
        let image_size = image.size()?;
        let (ring_elements, player_atom) = self.classify_detections(
            element_bbox_pairs,
            image_size.width as u32,
            image_size.height as u32,
        )?;

        let processing_time = start_time.elapsed().as_millis() as u64;

        // Create visualization if enabled
        if self.config.visualization.draw_bboxes {
            self.create_visualization(color_image, &all_detections)?;
        }

        let stats = DetectionStats {
            total_detections: all_detections.len(),
            ring_detections: ring_elements.len(),
            player_detections: if player_atom.is_some() { 1 } else { 0 },
            avg_confidence: all_detections.stats().avg_confidence,
            processing_time_ms: processing_time,
        };

        Ok(DetectionResult {
            ring_elements,
            player_atom,
            all_detections,
            confidence_stats: stats,
        })
    }

    /// Classify detections as ring elements or player atom
    fn classify_detections<'a>(
        &self,
        element_bbox_pairs: Vec<(Element<'a>, BBox)>,
        image_width: u32,
        image_height: u32,
    ) -> Result<(Vec<(Element<'a>, BBox)>, Option<(Element<'a>, BBox)>)> {
        let center_x = image_width as f32 / 2.0;
        let center_y = image_height as f32 / 2.0;

        let mut ring_elements = Vec::new();
        let mut player_candidates = Vec::new();

        for (element, bbox) in element_bbox_pairs {
            let bbox_center = bbox.center();
            let distance_from_center = (
                (bbox_center.x as f32 - center_x).powi(2) +
                (bbox_center.y as f32 - center_y).powi(2)
            ).sqrt();

            // Determine if this is likely a player atom (center) or ring element
            let tolerance = self.config.player_atom_detection.center_tolerance as f32;
            let max_center_distance = (image_width.min(image_height) as f32) * tolerance;

            if distance_from_center < max_center_distance {
                player_candidates.push((element, bbox.clone(), bbox.confidence));
            } else {
                ring_elements.push((element, bbox));
            }
        }

        // Select best player atom candidate
        let player_atom = player_candidates
            .into_iter()
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
            .map(|(element, bbox, _)| (element, bbox));

        // Sort ring elements by angle for consistent ordering
        ring_elements.sort_by(|a, b| {
            let angle_a = self.calculate_angle_from_center(&a.1, center_x, center_y);
            let angle_b = self.calculate_angle_from_center(&b.1, center_x, center_y);
            angle_a.partial_cmp(&angle_b).unwrap()
        });

        // Limit ring elements
        ring_elements.truncate(self.config.ring_detection.max_ring_elements);

        Ok((ring_elements, player_atom))
    }

    /// Calculate angle from image center
    fn calculate_angle_from_center(&self, bbox: &BBox, center_x: f32, center_y: f32) -> f32 {
        let bbox_center = bbox.center();
        let dx = bbox_center.x as f32 - center_x;
        let dy = bbox_center.y as f32 - center_y;
        dy.atan2(dx)
    }

    /// Create visualization using opencv-match for saving
    fn create_visualization(&self, image: &Mat, detections: &BBoxCollection) -> Result<()> {
        let mut output = image.clone();

        for bbox in detections.iter() {
            // Draw bounding box
            if self.config.visualization.draw_bboxes {
                imgproc::rectangle(
                    &mut output,
                    bbox.to_rect(),
                    bbox.get_bgr_scalar(),
                    3,
                    LINE_8,
                    0,
                )?;
            }

            // Draw label
            if self.config.visualization.draw_labels {
                let label = if self.config.visualization.draw_confidence {
                    format!("{} ({:.2})", bbox.class_id, bbox.confidence)
                } else {
                    bbox.class_id.clone()
                };

                imgproc::put_text(
                    &mut output,
                    &label,
                    Point::new(bbox.x + 5, bbox.y + 25),
                    FONT_HERSHEY_SIMPLEX,
                    0.8,
                    bbox.get_bgr_scalar(),
                    2,
                    LINE_8,
                    false,
                )?;
            }
        }

        // Save visualization using opencv-match if possible, fallback to OpenCV
        let output_path = self.config.output_dir.join("game_state_detection.png");
        
        // Try to convert to image crate format for consistent saving
        match ImageUtils::mat_to_rgb(&output) {
            Ok(rgb_image) => {
                rgb_image.save(&output_path)
                    .with_context(|| format!("Failed to save visualization: {:?}", output_path))?;
            }
            Err(_) => {
                // Fallback to OpenCV saving
                ImageUtils::save_image(&output, &output_path)?;
            }
        }

        println!("Visualization saved: {:?}", output_path);
        Ok(())
    }

    /// Export detection results in JSON format
    pub fn export_json<'a>(&self, results: &DetectionResult<'a>, output_path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(results)
            .context("Failed to serialize detection results")?;
        
        std::fs::write(output_path, json)
            .with_context(|| format!("Failed to write JSON to: {:?}", output_path))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atomas_core::elements::Data;

    #[test]
    fn test_detector_creation() -> Result<()> {
        let config = DetectionConfig::default();
        let _detector = GameStateDetector::new(config)?;
        Ok(())
    }

    #[test]
    fn test_rgb_image_detection() -> Result<()> {
        let config = DetectionConfig::default();
        let detector = GameStateDetector::new(config)?;
        
        // Create dummy data and image
        let dummy_data = Data { elements: Vec::new() };
        let rgb_image = image::RgbImage::new(100, 100);
        
        let _result = detector.detect_from_rgb_image(&rgb_image, &dummy_data)?;
        Ok(())
    }
}
