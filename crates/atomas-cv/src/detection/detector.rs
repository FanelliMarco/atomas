use super::config::DetectionConfig;
use crate::bbox::{BBox, BBoxCollection};
use crate::circle::{CircleDetector, DetectedCircle};
use crate::utils::ImageUtils;
use crate::Result;
use anyhow::Context;
use atomas_core::{elements::Data, Element};
use opencv::{
    core::{Mat, Point, Scalar, Size},
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

/// Internal type used through the post-processing pipeline.
/// Carries both the geometric circle and the color-match score so NMS
/// can pick the *best-matched* survivor when duplicates collide, rather
/// than just the first-encountered one.
#[derive(Clone)]
struct ScoredMatch<'a> {
    element: Element<'a>,
    circle: DetectedCircle,
    /// Color-match distance: lower = better match.
    color_dist: f64,
}

impl GameStateDetector {
    pub fn new(config: DetectionConfig) -> Result<Self> {
        let circle_detector = CircleDetector::new(config.circle_detection.clone());
        Ok(Self {
            config,
            circle_detector,
        })
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

        // 1. Raw Hough circle detection.
        let circles = self.circle_detector.detect(gray_image, color_image)?;
        println!("Detected {} raw circles", circles.len());

        self.print_diagnostic(&circles, elements_data);

        // 2. Color-match each circle to its best element candidate.
        let raw_matches = self.match_circles_to_elements(&circles, elements_data)?;
        println!("Color-matched {} of {} circles", raw_matches.len(), circles.len());

        // 3. Deduplicate: NMS by bbox-IoU + concentric-circle suppression.
        //    This is the step that fixes the stacked overlapping circles
        //    in the "next atom" preview region.
        let deduped = self.deduplicate_matches(raw_matches);
        println!("After NMS: {} unique detections", deduped.len());

        // 4. Build the canonical bbox collection now that duplicates are gone.
        let mut all_detections = BBoxCollection::new();
        for sm in &deduped {
            all_detections.push(self.circle_to_bbox(&sm.circle, &sm.element));
        }

        // 5. Split into ring atoms vs. player (center) atom by geometry.
        let image_size = gray_image.size()?;
        let (ring_elements, player_atom) = self.classify_detections(
            deduped,
            image_size.width as u32,
            image_size.height as u32,
        )?;

        // 6. Visualization.
        if self.config.visualization.draw_circles {
            self.create_visualization(color_image, &all_detections)?;
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
    ) -> Result<Vec<ScoredMatch<'a>>> {
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
                matches.push(ScoredMatch {
                    element: best_element.clone(),
                    circle: circle.clone(),
                    color_dist: best_dist,
                });
            } else {
                println!(
                    "  ✗ {} dist={:.4} > tol={:.4}",
                    best_element.name, best_dist, self.config.color_matching.tolerance
                );
            }
        }

        Ok(matches)
    }

    /// Suppress duplicate detections from the same on-screen atom.
    ///
    /// Two checks, in order:
    ///   * **Bbox IoU**: standard NMS — if two detections' bounding boxes
    ///     overlap by more than `nms_iou_threshold`, they're duplicates.
    ///   * **Center proximity**: if two circle centers are within
    ///     `min(r_a, r_b) * nms_center_distance_ratio` pixels of each
    ///     other, they're duplicates even if their bboxes don't overlap
    ///     enough. This catches concentric circles (atom + halo).
    ///
    /// When duplicates collide, the one with the **lower color-match
    /// distance** wins. This is what fixes the "Mendelevium label on a
    /// Samarium circle" symptom: when three Hough hits stack on the
    /// preview atom, only the one whose sampled color best matches an
    /// element survives, and its label is correct by construction.
    fn deduplicate_matches<'a>(&self, mut matches: Vec<ScoredMatch<'a>>) -> Vec<ScoredMatch<'a>> {
        if matches.is_empty() {
            return matches;
        }

        // Best (smallest) color_dist first → wins ties.
        matches.sort_by(|a, b| a.color_dist.partial_cmp(&b.color_dist).unwrap());

        let iou_thresh = self.config.circle_detection.nms_iou_threshold;
        let center_ratio = self.config.circle_detection.nms_center_distance_ratio;

        let n = matches.len();
        let mut keep = vec![true; n];

        for i in 0..n {
            if !keep[i] {
                continue;
            }
            let bi = circle_to_bbox_geom(&matches[i].circle);

            for j in (i + 1)..n {
                if !keep[j] {
                    continue;
                }
                let bj = circle_to_bbox_geom(&matches[j].circle);

                let iou_dup = bi.iou(&bj) > iou_thresh;
                let center_dup = if center_ratio > 0.0 {
                    let ci = &matches[i].circle.circle;
                    let cj = &matches[j].circle.circle;
                    let dx = (ci.center.0 - cj.center.0) as f64;
                    let dy = (ci.center.1 - cj.center.1) as f64;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let min_r = (ci.radius.min(cj.radius)) as f64;
                    dist < min_r * center_ratio
                } else {
                    false
                };

                if iou_dup || center_dup {
                    keep[j] = false;
                }
            }
        }

        matches
            .into_iter()
            .zip(keep)
            .filter_map(|(m, k)| if k { Some(m) } else { None })
            .collect()
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
        let bbox = circle_to_bbox_geom(circle);
        BBox::new(bbox.x, bbox.y, bbox.width, bbox.height, circle.circle.confidence)
            .with_class(element.name.to_string(), element.rgb)
    }

    fn classify_detections<'a>(
        &self,
        matches: Vec<ScoredMatch<'a>>,
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
                .map(|m| m.circle.circle.radius as f32)
                .sum::<f32>()
                / matches.len() as f32
        };

        let max_center_dist = image_width.min(image_height) as f32
            * self.config.player_atom_detection.center_tolerance as f32;

        let mut ring: Vec<(Element, BBox)> = Vec::new();
        let mut player_cands: Vec<(Element, BBox, f32)> = Vec::new();

        for sm in matches {
            let (ex, ey) = sm.circle.circle.center;
            let dist = ((ex as f32 - cx).powi(2) + (ey as f32 - cy).powi(2)).sqrt();
            let bbox = self.circle_to_bbox(&sm.circle, &sm.element);

            let is_centered = dist < max_center_dist;
            let is_larger = sm.circle.circle.radius as f32
                > avg_r * self.config.player_atom_detection.size_factor_range.0 as f32;

            if is_centered || is_larger {
                player_cands.push((sm.element.clone(), bbox.clone(), dist));
            }
            if dist > max_center_dist * 0.5 {
                ring.push((sm.element, bbox));
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

    /// Visualization.
    ///
    /// We draw straight from `BBoxCollection` (the post-NMS canonical set)
    /// instead of from the raw Hough output. That guarantees every drawn
    /// circle has a matching label and vice versa — fixing the "floating
    /// FLUORINE label with no green ring" symptom in the original output.
    /// Labels are also centered on the bbox rather than anchored to the
    /// top-left corner, so when atoms are close together the text doesn't
    /// drift away from its circle.
    fn create_visualization(
        &self,
        image: &Mat,
        detections: &BBoxCollection,
    ) -> Result<()> {
        let mut out = image.clone();
        let green = Scalar::new(0.0, 255.0, 0.0, 255.0);
        let red = Scalar::new(0.0, 0.0, 255.0, 255.0);

        for bbox in detections.iter() {
            let center = bbox.center();
            let radius = (bbox.width.min(bbox.height) / 2).max(1);

            // Green outline circle.
            imgproc::circle(&mut out, center, radius, green, 2, LINE_8, 0)?;

            // Red center dot.
            if self.config.visualization.draw_centers {
                imgproc::circle(&mut out, center, 3, red, -1, LINE_8, 0)?;
            }

            // Label: centered horizontally on the circle. By default we
            // place it above; if there isn't enough headroom (e.g. an atom
            // near the very top of the screen), we flip and draw below
            // instead. This avoids stomping on HUD text like the score in
            // the top bar.
            if self.config.visualization.draw_labels && !bbox.class_id.is_empty() {
                let font_scale = 0.6;
                let thickness = 2;
                let mut baseline = 0;
                let text_size: Size = imgproc::get_text_size(
                    &bbox.class_id,
                    FONT_HERSHEY_SIMPLEX,
                    font_scale,
                    thickness,
                    &mut baseline,
                )?;

                // OpenCV's put_text uses the text *baseline* as the Y
                // anchor. For an atom at (cx, cy) with radius r and text
                // height h:
                //   above-anchor:  baseline_y = cy - r - 6
                //                  (top of glyphs at  baseline_y - h)
                //   below-anchor:  baseline_y = cy + r + 6 + h
                let padding = 6;
                let above_baseline = center.y - radius - padding;
                let above_top = above_baseline - text_size.height;

                let baseline_y = if above_top >= 2 {
                    above_baseline
                } else {
                    center.y + radius + padding + text_size.height
                };

                let text_org = Point::new(
                    center.x - text_size.width / 2,
                    baseline_y,
                );
                imgproc::put_text(
                    &mut out,
                    &bbox.class_id,
                    text_org,
                    FONT_HERSHEY_SIMPLEX,
                    font_scale,
                    bbox.get_bgr_scalar(),
                    thickness,
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

/// Geometry-only bbox builder — no class info attached. Used for IoU
/// calculations during NMS where the class hasn't been finalized yet.
fn circle_to_bbox_geom(circle: &DetectedCircle) -> BBox {
    let (cx, cy) = circle.circle.center;
    let r = circle.circle.radius;
    BBox::new(cx - r, cy - r, r * 2, r * 2, circle.circle.confidence)
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
