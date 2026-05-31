use super::config::DetectionConfig;
use crate::bbox::{BBox, BBoxCollection};
use crate::circle::{CircleDetector, DetectedCircle};
use crate::utils::ImageUtils;
use crate::Result;
use anyhow::Context;
use atomas_core::{elements::Data, Element};
use opencv::{
    core::{Mat, Point, Scalar, Size, Vec3b},
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

        // 1b. Reject circles whose interior is just the playfield background.
        //     Some boards render faint placement-hint / "ghost" circles over
        //     empty playfield; their dark interior otherwise mislabels as a
        //     dark element (Carbon, etc.). We sample the background colour
        //     from the image corners and drop any circle close to it.
        let circles = self.reject_background_circles(color_image, circles)?;
        println!("After background rejection: {} circles", circles.len());

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

    /// Sample the playfield background colour as the median of several corner
    /// patches. These patches sit at the far-left/right edges at mid-height,
    /// where the ring never reaches, and avoid the top HUD and bottom nav bar.
    /// Returns an RGB tuple (matching the rest of the detector's convention).
    fn sample_background(&self, color_image: &Mat) -> Result<(u8, u8, u8)> {
        let w = color_image.cols();
        let h = color_image.rows();
        let patch = 10i32;
        // (fractional x, fractional y) of each sample centre.
        let spots: [(f64, f64); 6] = [
            (0.03, 0.45),
            (0.03, 0.55),
            (0.97, 0.45),
            (0.97, 0.55),
            (0.03, 0.35),
            (0.97, 0.35),
        ];

        let mut rs: Vec<u8> = Vec::new();
        let mut gs: Vec<u8> = Vec::new();
        let mut bs: Vec<u8> = Vec::new();

        for (fx, fy) in spots {
            let cx = (fx * w as f64) as i32;
            let cy = (fy * h as f64) as i32;
            let x0 = (cx - patch).max(0);
            let y0 = (cy - patch).max(0);
            let x1 = (cx + patch).min(w - 1);
            let y1 = (cy + patch).min(h - 1);
            for row in y0..=y1 {
                for col in x0..=x1 {
                    let bgr: &Vec3b = color_image.at_2d(row, col)?;
                    rs.push(bgr[2]);
                    gs.push(bgr[1]);
                    bs.push(bgr[0]);
                }
            }
        }

        if rs.is_empty() {
            return Ok((0, 0, 0));
        }
        rs.sort_unstable();
        gs.sort_unstable();
        bs.sort_unstable();
        let mid = rs.len() / 2;
        Ok((rs[mid], gs[mid], bs[mid]))
    }

    /// Drop circles whose interior colour matches the playfield background.
    /// No-op when `bg_reject_threshold <= 0`.
    fn reject_background_circles(
        &self,
        color_image: &Mat,
        circles: Vec<DetectedCircle>,
    ) -> Result<Vec<DetectedCircle>> {
        let threshold = self.config.circle_detection.bg_reject_threshold;
        if threshold <= 0.0 {
            return Ok(circles);
        }

        let bg = self.sample_background(color_image)?;
        println!("Estimated background RGB({},{},{})", bg.0, bg.1, bg.2);

        let kept: Vec<DetectedCircle> = circles
            .into_iter()
            .filter(|c| {
                // Use the median interior colour — robust to the bright glyph
                // text/number drawn over each atom.
                let d = self.rgb_distance(&c.median_color, &bg);
                if d < threshold {
                    println!(
                        "  drop ghost circle ({},{}) r={} RGB({},{},{}) bg_dist={:.1} < {:.1}",
                        c.circle.center.0, c.circle.center.1, c.circle.radius,
                        c.median_color.0, c.median_color.1, c.median_color.2,
                        d, threshold,
                    );
                    false
                } else {
                    true
                }
            })
            .collect();

        Ok(kept)
    }

    fn match_circles_to_elements<'a>(
        &self,
        circles: &[DetectedCircle],
        elements_data: &'a Data,
    ) -> Result<Vec<ScoredMatch<'a>>> {
        let use_hsv = self.config.color_matching.use_hsv;
        let tol = self.config.color_matching.tolerance;

        let is_gray = |rgb: &(u8, u8, u8)| -> bool {
            let mx = rgb.0.max(rgb.1).max(rgb.2) as f64;
            if mx < 1.0 {
                return true;
            }
            let mn = rgb.0.min(rgb.1).min(rgb.2) as f64;
            (mx - mn) / mx < 0.12 // saturation < 12%
        };

        // Provisional per-circle result. We keep the full ranked candidate
        // list (as element indices) so a circle can be re-matched against a
        // restricted element set in the reachability pass below.
        struct Prov {
            circle_idx: usize,
            ranked: Vec<(usize, f64)>, // (element index, distance) sorted asc
            best_elem_idx: usize,
            best_dist: f64,
        }
        let mut provs: Vec<Prov> = Vec::new();

        // ----- Phase 1: rank every element per circle, pick provisional best
        // (with the gray tie-break) -------------------------------------------
        for (ci, circle) in circles.iter().enumerate() {
            let raw_color = circle.mean_color;
            let color = Self::normalize_brightness(raw_color);

            println!(
                "\nMatching circle at ({},{}) r={} | RGB({},{},{})",
                circle.circle.center.0, circle.circle.center.1, circle.circle.radius,
                color.0, color.1, color.2,
            );

            let mut ranked: Vec<(usize, f64)> = elements_data
                .elements
                .iter()
                .enumerate()
                .map(|(ei, e)| {
                    // HSV distance uses RAW colors (it normalizes hue/sat
                    // internally but keeps raw brightness so grays don't
                    // collapse together). The legacy RGB path uses normalized.
                    let d = if use_hsv {
                        self.hsv_distance(&raw_color, &e.rgb)
                    } else {
                        self.rgb_distance(&color, &Self::normalize_brightness(e.rgb))
                    };
                    (ei, d)
                })
                .collect();
            ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            println!("  Top 8:");
            for (i, (ei, d)) in ranked.iter().take(8).enumerate() {
                let e = &elements_data.elements[*ei];
                println!(
                    "    {}. {:>15}  RGB({:>3},{:>3},{:>3})  dist={:.4}",
                    i + 1, e.name, e.rgb.0, e.rgb.1, e.rgb.2, d,
                );
            }

            // Provisional best + gray tie-break (prefer lowest atomic number
            // among near-tied GRAYS — see note below).
            let (mut best_ei, mut best_d) = ranked[0];
            let base = ranked[0].1;
            if is_gray(&elements_data.elements[best_ei].rgb) {
                let tie_eps = 0.12;
                let mut chosen_z = elements_data.elements[best_ei].element_type.to_numeric();
                for &(ei, d) in ranked.iter() {
                    if d > base + tie_eps {
                        break;
                    }
                    let e = &elements_data.elements[ei];
                    if !is_gray(&e.rgb) {
                        continue;
                    }
                    let z = e.element_type.to_numeric();
                    if z > 0 && (chosen_z <= 0 || z < chosen_z) {
                        best_ei = ei;
                        best_d = d;
                        chosen_z = z;
                    }
                }
            }

            provs.push(Prov {
                circle_idx: ci,
                ranked,
                best_elem_idx: best_ei,
                best_dist: best_d,
            });
        }

        // ----- Phase 2: reachability ceiling ---------------------------------
        // Many elements share near-identical colors across the periodic table
        // (Fluorine 236,196,111 vs Gold 237,196,116; Nitrogen vs Protactinium).
        // Pure color cannot separate them — but Atomas can: atom values on the
        // ring form a low, roughly contiguous cluster, so a match dozens of
        // numbers above everything else is a color collision, not a real atom.
        //
        // We take the confident provisional matches, find the contiguous low
        // cluster (extending upward while each gap is <= GAP), and set the
        // ceiling to that cluster's max + GAP. GAP both tolerates legitimate
        // progression jumps and excludes the far-away collisions (Au=79,
        // Pa=91 are 70+ above a Fluorine-era board).
        const GAP: i16 = 18;
        let mut zs: Vec<i16> = provs
            .iter()
            .filter(|p| p.best_dist < tol)
            .map(|p| elements_data.elements[p.best_elem_idx].element_type.to_numeric())
            .filter(|z| *z > 0)
            .collect();
        zs.sort_unstable();
        zs.dedup();
        let ceiling: i16 = if zs.is_empty() {
            i16::MAX
        } else {
            let mut cmax = zs[0];
            for &z in zs.iter().skip(1) {
                if z - cmax > GAP {
                    break;
                }
                cmax = z;
            }
            cmax.saturating_add(GAP)
        };
        println!("\nReachability ceiling (atomic number) = {}", ceiling);

        // ----- Phase 3: re-match over-ceiling circles, then tolerance filter --
        let mut matches: Vec<ScoredMatch> = Vec::new();
        for p in &provs {
            let mut best_ei = p.best_elem_idx;
            let mut best_d = p.best_dist;

            // A circle that confidently matched *some* element — even an
            // unreachable one — is definitely a real atom. So if we have to
            // relabel it to a reachable element, accept that relabel even if
            // its color distance exceeds the normal tolerance (the gate is
            // there to reject non-atoms, and this is already known to be one).
            let was_confident = p.best_dist < tol;
            let best_z = elements_data.elements[best_ei].element_type.to_numeric();
            let mut forced = false;
            if best_z > ceiling {
                if let Some(&(ei, d)) = p.ranked.iter().find(|&&(ei, _)| {
                    let z = elements_data.elements[ei].element_type.to_numeric();
                    z > 0 && z <= ceiling
                }) {
                    println!(
                        "  reachability: {} (Z={}) unreachable -> {} (dist={:.4})",
                        elements_data.elements[best_ei].name, best_z,
                        elements_data.elements[ei].name, d,
                    );
                    best_ei = ei;
                    best_d = d;
                    forced = true;
                }
            }

            let e = &elements_data.elements[best_ei];
            let accept = best_d < tol || (forced && was_confident);
            if accept {
                println!("  [OK] -> {} (dist={:.4})", e.name, best_d);
                matches.push(ScoredMatch {
                    element: e.clone(),
                    circle: circles[p.circle_idx].clone(),
                    color_dist: best_d,
                });
            } else {
                println!("  [--] {} dist={:.4} > tol={:.4}", e.name, best_d, tol);
            }
        }

        Ok(matches)
    }

    /// Suppress duplicate detections from the same on-screen atom.
    ///
    /// Two checks, in order:
    ///   * **Bbox IoU**: standard NMS â€” if two detections' bounding boxes
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

        // Best (smallest) color_dist first â†’ wins ties.
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

    /// HSV color distance between two **raw** (un-normalized) RGB colors.
    ///
    /// Hue and saturation are computed after brightness-normalizing each
    /// color (so lighting variation doesn't shift colored atoms), but the
    /// **value/brightness term uses the raw colors**. This is essential for
    /// gray atoms: `normalize_brightness` scales both dark Carbon
    /// (59,59,59) and bright Beryllium (200,200,200) to (200,200,200),
    /// making them identical — so brightness must be compared *before*
    /// normalization or the two collapse onto each other.
    fn hsv_distance(&self, raw1: &(u8, u8, u8), raw2: &(u8, u8, u8)) -> f64 {
        let n1 = Self::normalize_brightness(*raw1);
        let n2 = Self::normalize_brightness(*raw2);
        let (h1, s1, _) = rgb_to_hsv(&n1);
        let (h2, s2, _) = rgb_to_hsv(&n2);
        // Value from the RAW colors.
        let (_, _, v1) = rgb_to_hsv(raw1);
        let (_, _, v2) = rgb_to_hsv(raw2);

        let raw_dh = (h1 - h2).abs();
        let dh = raw_dh.min(360.0 - raw_dh) / 180.0;
        let ds = (s1 - s2).abs();
        let dv = (v1 - v2).abs();

        let hw = self.config.color_matching.hue_weight;
        let sw = self.config.color_matching.saturation_weight;
        let vw = self.config.color_matching.value_weight;

        // Saturation-gate the hue term. Hue is meaningless for desaturated
        // (gray) colors — a near-gray pixel's hue is numerical noise — so a
        // gray atom otherwise lands a huge spurious hue distance from its
        // own reference and is dropped. Scaling the hue contribution by the
        // *minimum* of the two saturations makes hue matter only when both
        // colors are actually colorful; for grays the term vanishes.
        let sat_gate = s1.min(s2);
        let dh_eff = dh * sat_gate;

        // For grays, raw brightness is the only discriminator (bright Be vs
        // dark C). Boost the value term in that regime so they don't merge;
        // the boost fades out as either color gains saturation.
        let grayness = 1.0 - sat_gate.min(1.0);
        let vw_eff = vw + grayness * (sw - vw).max(0.0);

        ((dh_eff * hw).powi(2) + (ds * sw).powi(2) + (dv * vw_eff).powi(2)).sqrt()
    }

    fn circle_to_bbox(&self, circle: &DetectedCircle, element: &Element) -> BBox {
        let bbox = circle_to_bbox_geom(circle);
        BBox::new(bbox.x, bbox.y, bbox.width, bbox.height, circle.circle.confidence)
            .with_class(element.name.to_string(), element.rgb)
    }

    /// Weiszfeld spatial median of the match centres. More robust than the
    /// arithmetic mean when one detection (the off-centre player atom) would
    /// otherwise drag the centre estimate away from the true ring centre.
    fn spatial_median(&self, matches: &[ScoredMatch]) -> (f32, f32) {
        let pts: Vec<(f32, f32)> = matches
            .iter()
            .map(|m| {
                let (x, y) = m.circle.circle.center;
                (x as f32, y as f32)
            })
            .collect();
        let n = pts.len() as f32;
        let mut cx = pts.iter().map(|p| p.0).sum::<f32>() / n;
        let mut cy = pts.iter().map(|p| p.1).sum::<f32>() / n;
        for _ in 0..50 {
            let mut sx = 0.0f32;
            let mut sy = 0.0f32;
            let mut sw = 0.0f32;
            for &(x, y) in &pts {
                let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() + 1e-6;
                let w = 1.0 / d;
                sx += x * w;
                sy += y * w;
                sw += w;
            }
            let nx = sx / sw;
            let ny = sy / sw;
            if (nx - cx).abs() < 0.1 && (ny - cy).abs() < 0.1 {
                cx = nx;
                cy = ny;
                break;
            }
            cx = nx;
            cy = ny;
        }
        (cx, cy)
    }

    fn classify_detections<'a>(
        &self,
        matches: Vec<ScoredMatch<'a>>,
        image_width: u32,
        image_height: u32,
    ) -> Result<(Vec<(Element<'a>, BBox)>, Option<(Element<'a>, BBox)>)> {
        // The ring is not always centred on the image (the game can offset it
        // vertically). Estimate the ring centre from the detections themselves
        // via a Weiszfeld spatial median, which is robust to the off-centre
        // player atom. Fall back to the image centre if we have too few points.
        let (cx, cy) = if matches.len() >= 3 {
            self.spatial_median(&matches)
        } else {
            (image_width as f32 / 2.0, image_height as f32 / 2.0)
        };

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
    /// circle has a matching label and vice versa â€” fixing the "floating
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

/// Geometry-only bbox builder â€” no class info attached. Used for IoU
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
