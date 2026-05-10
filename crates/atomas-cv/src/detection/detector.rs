use super::config::{ColorMatchMethod, DetectionConfig};
use crate::bbox::{BBox, BBoxCollection};
use crate::circle::{CircleDetector, DetectedCircle};
use crate::utils::ImageUtils;
use crate::Result;
use atomas_core::{elements::Data, Element};
use anyhow::Context;
use opencv::{
    core::{Mat, Point, Scalar},
    imgproc::{self, FONT_HERSHEY_SIMPLEX, LINE_8},
    prelude::*,
};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct DetectionResult<'a> {
    pub ring_elements: Vec<(Element<'a>, BBox)>,
    pub player_atom: Option<(Element<'a>, BBox)>,
    pub all_detections: BBoxCollection,
    pub confidence_stats: DetectionStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectionStats {
    pub total_detections: usize,
    pub ring_detections: usize,
    pub player_detections: usize,
    pub avg_confidence: f64,
    pub processing_time_ms: u64,
}

pub struct GameStateDetector {
    config: DetectionConfig,
    circle_detector: CircleDetector,
}

impl GameStateDetector {
    pub fn new(config: DetectionConfig) -> Result<Self> {
        let circle_detector = CircleDetector::new(config.circle_detection.clone());
        Ok(Self { config, circle_detector })
    }

    pub fn detect_from_file<'a, P: AsRef<Path>>(
        &self,
        image_path: P,
        elements_data: &'a Data,
    ) -> Result<DetectionResult<'a>> {
        let gray = ImageUtils::load_grayscale(&image_path)
            .with_context(|| format!("Failed to load grayscale: {:?}", image_path.as_ref()))?;
        let color = ImageUtils::load_color(&image_path)
            .with_context(|| format!("Failed to load color: {:?}", image_path.as_ref()))?;

        self.detect_from_mat(&gray, &color, elements_data)
    }

    pub fn detect_from_rgb_image<'a>(
        &self,
        rgb_image: &image::RgbImage,
        elements_data: &'a Data,
    ) -> Result<DetectionResult<'a>> {
        let color_mat = ImageUtils::rgb_to_mat(rgb_image)?;
        let mut gray_mat = Mat::default();
        opencv::imgproc::cvt_color(
            &color_mat,
            &mut gray_mat,
            opencv::imgproc::COLOR_BGR2GRAY,
            0,
        )?;
        self.detect_from_mat(&gray_mat, &color_mat, elements_data)
    }

    pub fn detect_from_mat<'a>(
        &self,
        gray_image: &Mat,
        color_image: &Mat,
        elements_data: &'a Data,
    ) -> Result<DetectionResult<'a>> {
        let start = std::time::Instant::now();

        let circles = self.circle_detector.detect(gray_image, color_image)?;
        println!("Detected {} circles total", circles.len());

        self.print_diagnostic(&circles, elements_data);

        let element_matches = self.match_circles_to_elements(&circles, elements_data)?;

        let mut all_detections = BBoxCollection::new();
        for (element, circle, _) in &element_matches {
            all_detections.push(self.circle_to_bbox(circle, element));
        }

        let image_size = gray_image.size()?;
        let (ring_elements, player_atom) = self.classify_detections(
            element_matches,
            image_size.width as u32,
            image_size.height as u32,
        )?;

        if self.config.visualization.draw_circles {
            self.create_visualization(color_image, &all_detections, &circles)?;
        }

        let elapsed = start.elapsed().as_millis() as u64;
        let stats = DetectionStats {
            total_detections: all_detections.len(),
            ring_detections: ring_elements.len(),
            player_detections: if player_atom.is_some() { 1 } else { 0 },
            avg_confidence: 1.0,
            processing_time_ms: elapsed,
        };

        Ok(DetectionResult {
            ring_elements,
            player_atom,
            all_detections,
            confidence_stats: stats,
        })
    }

    fn print_diagnostic(&self, circles: &[DetectedCircle], elements_data: &Data) {
        println!("\n=== Detected Colors (Raw vs Normalized) ===");

        for c in circles {
            let mean = c.mean_color;
            let mean_norm = Self::normalize_brightness(mean);
            let best_raw = self.nearest_element(mean, elements_data);
            let best_norm = self.nearest_element(mean_norm, elements_data);

            println!(
                "  Circle ({:>4},{:>4}) r={:>3}  Raw RGB({:>3},{:>3},{:>3})->{:<15}  Norm RGB({:>3},{:>3},{:>3})->{:<15}",
                c.circle.center.0, c.circle.center.1, c.circle.radius,
                mean.0, mean.1, mean.2, best_raw,
                mean_norm.0, mean_norm.1, mean_norm.2, best_norm,
            );
        }
        println!("===========================================\n");
    }

    fn normalize_brightness(color: (u8, u8, u8)) -> (u8, u8, u8) {
        let max_ch = color.0.max(color.1).max(color.2) as f32;
        if max_ch < 1.0 {
            return (0, 0, 0);
        }
        let scale = 200.0 / max_ch;
        (
            (color.0 as f32 * scale).round().min(255.0) as u8,
            (color.1 as f32 * scale).round().min(255.0) as u8,
            (color.2 as f32 * scale).round().min(255.0) as u8,
        )
    }

    fn nearest_element(&self, color: (u8, u8, u8), elements_data: &Data) -> String {
        elements_data
            .elements
            .iter()
            .min_by(|a, b| {
                self.rgb_distance(&color, &a.rgb)
                    .partial_cmp(&self.rgb_distance(&color, &b.rgb))
                    .unwrap()
            })
            .map(|e| e.name.to_string())
            .unwrap_or_default()
    }

    fn match_circles_to_elements<'a>(
        &self,
        circles: &[DetectedCircle],
        elements_data: &'a Data,
    ) -> Result<Vec<(Element<'a>, DetectedCircle, f64)>> {
        let mut matches = Vec::new();

        for circle in circles {
            let color = Self::normalize_brightness(circle.mean_color);

            println!(
                "\nMatching circle at ({},{}) r={} | RGB({},{},{})",
                circle.circle.center.0, circle.circle.center.1, circle.circle.radius,
                color.0, color.1, color.2,
            );

            let mut candidates: Vec<(&Element, f64, f64)> = elements_data
                .elements
                .iter()
                .map(|e| {
                    let elem_color_norm = Self::normalize_brightness(e.rgb);
                    let hsv_d = self.hsv_distance(&color, &elem_color_norm);
                    let rgb_d = self.rgb_distance(&color, &elem_color_norm);
                    (e, hsv_d, rgb_d)
                })
                .collect();

            if self.config.color_matching.use_hsv {
                candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            } else {
                candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
            }

            println!("  Top 8:");
            for (i, (e, hsv_d, rgb_d)) in candidates.iter().take(8).enumerate() {
                println!(
                    "    {}. {:>15}  RGB({:>3},{:>3},{:>3})  hsv={:.4}  rgb={:.2}",
                    i + 1, e.name, e.rgb.0, e.rgb.1, e.rgb.2, hsv_d, rgb_d,
                );
            }

            let (best_element, best_dist) = if self.config.color_matching.use_hsv {
                let f = candidates.first().unwrap();
                (f.0, f.1)
            } else {
                let f = candidates.first().unwrap();
                (f.0, f.2)
            };

            if best_dist < self.config.color_matching.tolerance {
                println!("  ✓ -> {} (dist={:.4})", best_element.name, best_dist);
                matches.push((best_element.clone(), circle.clone(), best_dist));
            } else {
                println!(
                    "  ✗ {} dist={:.4} > tol={:.4}",
                    best_element.name, best_dist, self.config.color_matching.tolerance
                );
            }
        }

        println!("\nMatched: {} / {}", matches.len(), circles.len());
        Ok(matches)
    }

    fn rgb_distance(&self, c1: &(u8, u8, u8), c2: &(u8, u8, u8)) -> f64 {
        let dr = c1.0 as f64 - c2.0 as f64;
        let dg = c1.1 as f64 - c2.1 as f64;
        let db = c1.2 as f64 - c2.2 as f64;
        (dr * dr + dg * dg + db * db).sqrt()
    }

    fn hsv_distance(&self, c1: &(u8, u8, u8), c2: &(u8, u8, u8)) -> f64 {
        let (h1, s1, v1) = rgb_to_hsv(c1);
        let (h2, s2, v2) = rgb_to_hsv(c2);

        let raw_dh = (h1 - h2).abs();
        let dh = raw_dh.min(360.0 - raw_dh) / 180.0;
        let ds = (s1 - s2).abs();
        let dv = (v1 - v2).abs();

        let hw = self.config.color_matching.hue_weight;
        let sw = self.config.color_matching.saturation_weight;
        let vw = self.config.color_matching.value_weight;

        ((dh * hw).powi(2) + (ds * sw).powi(2) + (dv * vw).powi(2)).sqrt()
    }

    fn circle_to_bbox(&self, circle: &DetectedCircle, element: &Element) -> BBox {
        let (cx, cy) = circle.circle.center;
        let r = circle.circle.radius;
        BBox::new(cx - r, cy - r, r * 2, r * 2, circle.circle.confidence)
            .with_class(element.name.to_string(), element.rgb)
    }

    fn classify_detections<'a>(
        &self,
        matches: Vec<(Element<'a>, DetectedCircle, f64)>,
        image_width: u32,
        image_height: u32,
    ) -> Result<(Vec<(Element<'a>, BBox)>, Option<(Element<'a>, BBox)>)> {
        let cx = image_width as f32 / 2.0;
        let cy = image_height as f32 / 2.0;

        let avg_r = if matches.is_empty() {
            0.0f32
        } else {
            matches
                .iter()
                .map(|(_, c, _)| c.circle.radius as f32)
                .sum::<f32>()
                / matches.len() as f32
        };

        let max_center_dist = image_width.min(image_height) as f32
            * self.config.player_atom_detection.center_tolerance as f32;

        let mut ring: Vec<(Element, BBox)> = Vec::new();
        let mut player_cands: Vec<(Element, BBox, f32)> = Vec::new();

        for (element, circle, _) in matches {
            let (ex, ey) = circle.circle.center;
            let dist = ((ex as f32 - cx).powi(2) + (ey as f32 - cy).powi(2)).sqrt();
            let bbox = self.circle_to_bbox(&circle, &element);

            let is_centered = dist < max_center_dist;
            let is_larger = circle.circle.radius as f32
                > avg_r * self.config.player_atom_detection.size_factor_range.0 as f32;

            if is_centered || is_larger {
                player_cands.push((element.clone(), bbox.clone(), dist));
            }
            if dist > max_center_dist * 0.5 {
                ring.push((element, bbox));
            }
        }

        let player_atom = player_cands
            .into_iter()
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
            .map(|(e, b, _)| (e, b));

        if let Some((ref pe, ref pb)) = player_atom {
            ring.retain(|(e, b)| e.name != pe.name || b.x != pb.x || b.y != pb.y);
        }

        ring.sort_by(|a, b| {
            let ang = |bbox: &BBox| {
                let bc = bbox.center();
                (bc.y as f32 - cy).atan2(bc.x as f32 - cx)
            };
            ang(&a.1).partial_cmp(&ang(&b.1)).unwrap()
        });

        ring.truncate(self.config.ring_detection.max_ring_elements);
        Ok((ring, player_atom))
    }

    fn create_visualization(
        &self,
        image: &Mat,
        detections: &BBoxCollection,
        circles: &[DetectedCircle],
    ) -> Result<()> {
        let mut out = image.clone();

        for circle in circles {
            let (cx, cy) = circle.circle.center;
            imgproc::circle(
                &mut out,
                Point::new(cx, cy),
                circle.circle.radius,
                Scalar::new(0.0, 255.0, 0.0, 255.0),
                2,
                LINE_8,
                0,
            )?;
            if self.config.visualization.draw_centers {
                imgproc::circle(
                    &mut out,
                    Point::new(cx, cy),
                    3,
                    Scalar::new(0.0, 0.0, 255.0, 255.0),
                    -1,
                    LINE_8,
                    0,
                )?;
            }
        }

        if self.config.visualization.draw_labels {
            for bbox in detections.iter() {
                imgproc::put_text(
                    &mut out,
                    &bbox.class_id,
                    Point::new(bbox.x + 5, bbox.y + 20),
                    FONT_HERSHEY_SIMPLEX,
                    0.6,
                    bbox.get_bgr_scalar(),
                    2,
                    LINE_8,
                    false,
                )?;
            }
        }

        let path = self.config.output_dir.join("circle_detection.png");
        ImageUtils::save_image(&out, &path)?;
        println!("Visualization saved: {:?}", path);
        Ok(())
    }
}

fn rgb_to_hsv(rgb: &(u8, u8, u8)) -> (f64, f64, f64) {
    let r = rgb.0 as f64 / 255.0;
    let g = rgb.1 as f64 / 255.0;
    let b = rgb.2 as f64 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        let h = 60.0 * ((g - b) / delta);
        if h < 0.0 {
            h + 360.0
        } else {
            h % 360.0
        }
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let s = if max == 0.0 { 0.0 } else { delta / max };
    (h, s, max)
}
