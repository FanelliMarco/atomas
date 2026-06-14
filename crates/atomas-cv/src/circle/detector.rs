use super::Circle;
use crate::detection::config::{CircleDetectionConfig, PreprocessingMethod};
use crate::Result;
use anyhow::Context;
use opencv::{
    core::{self, Mat, Point, Scalar, Vec3b, Vec3f},
    imgproc::{self, HOUGH_GRADIENT},
    prelude::*,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct CircleDetector {
    config: CircleDetectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedCircle {
    pub circle: Circle,
    pub mean_color: (u8, u8, u8),
    pub median_color: (u8, u8, u8),
    pub dominant_color: (u8, u8, u8),
    /// True when the atom prints an atomic-number row (light digits in the
    /// lower third of the glyph). Special atoms (_Plus/_Minus/etc.) draw a
    /// bare symbol with no number, so this is false for them. Used by the
    /// matcher to veto a special label on a circle that clearly has a number
    /// — the reds (_Plus/Phosphorus/Manganese) are inseparable by colour, so
    /// "does it have a number?" is the only cheap, template-free signal that
    /// keeps a real atom from being read as the special plus.
    pub has_number: bool,
}

struct ColorAnalysisResult {
    mean: (u8, u8, u8),
    median: (u8, u8, u8),
    dominant: (u8, u8, u8),
    has_number: bool,
}

impl CircleDetector {
    pub fn new(config: CircleDetectionConfig) -> Self {
        Self { config }
    }

    /// Detect circles via two Hough passes:
    /// 1. On the preprocessed grayscale (original behaviour) â€” catches every
    ///    atom with luminance contrast against the playfield.
    /// 2. On a chromaticity-distance map, when `background` is provided â€”
    ///    catches atoms whose *brightness* matches the playfield but whose
    ///    *colour* doesn't (Carbon: dark grey on dark maroon is invisible in
    ///    grayscale but chroma-distinct).
    /// Near-duplicate circles between the passes are merged by centre
    /// distance; downstream NMS handles the rest.
    ///
    /// NOTE: blob recovery deliberately does NOT happen here. It must run
    /// AFTER ghost rejection (see `recover_missing`), otherwise a doomed
    /// Hough circle can shadow a real atom out of recovery and then get
    /// rejected, losing the atom entirely.
    pub fn detect(
        &self,
        gray_image: &Mat,
        color_image: &Mat,
        background: Option<(u8, u8, u8)>,
    ) -> Result<Vec<DetectedCircle>> {
        println!("CircleDetector v7 (gray hough; masked-bg blob recovery post-rejection)");
        let _ = background; // recovery of color-distinct atoms happens in
                            // `recover_missing` (post-rejection), which covers
                            // everything the old chroma-Hough pass found and
                            // saves one full map build per frame.
        let processed = self.preprocess(gray_image)?;
        let raw = self.hough_pass(&processed, "gray")?;

        let mut detected = Vec::new();
        for (x, y, r) in raw {
            let ca = self.analyze_color(color_image, x, y, r)?;
            detected.push(DetectedCircle {
                circle: Circle::new((x, y), r, 1.0),
                mean_color: ca.mean,
                median_color: ca.median,
                dominant_color: ca.dominant,
                has_number: ca.has_number,
            });
        }

        println!("HoughCircles detected {} circles", detected.len());
        Ok(detected)
    }

    /// Gradient-free recovery, to be called AFTER ghost rejection with the
    /// SURVIVING circles: binarise the chromaticity map, erase the survivors,
    /// and treat the remaining round blobs as atoms. Deduping against
    /// survivors only means a rejected ghost can never shadow a real atom.
    pub fn recover_missing(
        &self,
        color_image: &Mat,
        existing: &[DetectedCircle],
    ) -> Result<Vec<DetectedCircle>> {
        let coords: Vec<(i32, i32, i32)> = existing
            .iter()
            .map(|c| (c.circle.center.0, c.circle.center.1, c.circle.radius))
            .collect();
        // Survivors are excluded from the local-background estimate so a
        // crowded neighbourhood can't camouflage the leftover atom.
        let dist_map = Self::background_distance_map(color_image, &coords)?;
        let blobs = self.recover_blobs(&dist_map, &coords)?;

        let min_sep = (self.config.min_dist * 0.5).max(1.0);
        let mut recovered = Vec::new();
        for (x, y, r) in blobs {
            let dup = coords.iter().any(|(ex, ey, _)| {
                let dx = (ex - x) as f64;
                let dy = (ey - y) as f64;
                (dx * dx + dy * dy).sqrt() < min_sep
            });
            if dup {
                continue;
            }
            println!("  blob pass recovered circle ({}, {}) r={}", x, y, r);
            let ca = self.analyze_color(color_image, x, y, r)?;
            recovered.push(DetectedCircle {
                circle: Circle::new((x, y), r, 1.0),
                mean_color: ca.mean,
                median_color: ca.median,
                dominant_color: ca.dominant,
                has_number: ca.has_number,
            });
        }
        Ok(recovered)
    }

    /// One HoughCircles run over an 8UC1 image; returns raw (x, y, r).
    fn hough_pass(&self, image_8uc1: &Mat, label: &str) -> Result<Vec<(i32, i32, i32)>> {
        let mut circles = Mat::default();
        imgproc::hough_circles(
            image_8uc1,
            &mut circles,
            HOUGH_GRADIENT,
            self.config.dp,
            self.config.min_dist,
            self.config.param1,
            self.config.param2,
            self.config.min_radius,
            self.config.max_radius,
        )
        .context("HoughCircles failed")?;

        if circles.empty() {
            println!("HoughCircles[{}]: 0 circles found", label);
            return Ok(Vec::new());
        }

        let mut out = Vec::with_capacity(circles.cols() as usize);
        for i in 0..circles.cols() {
            let d: &Vec3f = circles.at(i)?;
            out.push((d[0] as i32, d[1] as i32, d[2] as i32));
        }
        Ok(out)
    }

    /// Gradient-free recovery: binarise the background-distance map, erase
    /// every circle Hough already found, and treat the remaining round blobs
    /// as atoms. A dark atom (Carbon) that Canny can't outline is still a
    /// solid blob well above the field here.
    fn recover_blobs(
        &self,
        dist_map: &Mat,
        existing: &[(i32, i32, i32)],
    ) -> Result<Vec<(i32, i32, i32)>> {
        // Threshold on the chromaticity scale: field ~7, Plus glow ~29,
        // Boron ~89, gray Carbon ~144, teal Nitrogen ~165. 65 clears the glow
        // with margin while keeping every known atom chroma.
        let mut binary = Mat::default();
        imgproc::threshold(dist_map, &mut binary, 65.0, 255.0, imgproc::THRESH_BINARY)?;

        // Erase known circles (slightly inflated) so leftover atoms stand
        // alone even when they touch an already-detected neighbour.
        for (x, y, r) in existing {
            imgproc::circle(
                &mut binary,
                Point::new(*x, *y),
                r + 6,
                Scalar::all(0.0),
                imgproc::FILLED,
                imgproc::LINE_8,
                0,
            )?;
        }

        // Open: drops glyph specks, thin ring outlines, label text.
        let kernel = imgproc::get_structuring_element(
            imgproc::MORPH_ELLIPSE,
            core::Size::new(5, 5),
            Point::new(-1, -1),
        )?;
        let mut opened = Mat::default();
        imgproc::morphology_ex(
            &binary,
            &mut opened,
            imgproc::MORPH_OPEN,
            &kernel,
            Point::new(-1, -1),
            1,
            core::BORDER_CONSTANT,
            imgproc::morphology_default_border_value()?,
        )?;

        let mut contours: core::Vector<core::Vector<Point>> = core::Vector::new();
        imgproc::find_contours(
            &opened,
            &mut contours,
            imgproc::RETR_EXTERNAL,
            imgproc::CHAIN_APPROX_SIMPLE,
            Point::new(0, 0),
        )?;

        let r_min = (self.config.min_radius - 2).max(3) as f32;
        let r_max = (self.config.max_radius + 4) as f32;

        let mut recovered = Vec::new();
        for contour in contours.iter() {
            let area = imgproc::contour_area(&contour, false)?;
            let mut center = core::Point2f::new(0.0, 0.0);
            let mut radius = 0f32;
            imgproc::min_enclosing_circle(&contour, &mut center, &mut radius)?;

            if radius < r_min || radius > r_max {
                continue;
            }
            // Disk-ness: a filled circle has area â‰ˆ Ï€rÂ²; text and ring arcs
            // fill a small fraction of their enclosing circle.
            let fill = area / (std::f64::consts::PI * (radius as f64) * (radius as f64));
            if fill < 0.55 {
                continue;
            }

            recovered.push((center.x as i32, center.y as i32, radius as i32));
        }
        Ok(recovered)
    }

    /// Per-pixel CHROMATICITY distance from the local background. Both pixel
    /// and local background are normalised to unit RGB vectors before
    /// differencing, so pure brightness differences (vignette, Plus glow)
    /// vanish while color-distinct atoms (gray Carbon, teal Nitrogen) pop.
    ///
    /// The local background is a MASKED box blur: pixels inside `exclude`
    /// circles (inflated +10) don't contribute â€” blur(imgÂ·mask)/blur(mask).
    /// Without this, crowded neighbourhoods drag the local average toward
    /// "desaturated atom soup" and a gray atom in a dense corner measures
    /// chroma-close to its own neighbours (observed: Carbon at 41 vs
    /// threshold 65; masked it returns to ~89).
    fn background_distance_map(
        color_image: &Mat,
        exclude: &[(i32, i32, i32)],
    ) -> Result<Mat> {
        let rows = color_image.rows();
        let cols = color_image.cols();

        let mut as_f32 = Mat::default();
        color_image.convert_to(&mut as_f32, core::CV_32FC3, 1.0, 1.0)?;

        // Validity mask: 1.0 everywhere, 0.0 inside excluded circles.
        let mut mask = Mat::new_rows_cols_with_default(
            rows,
            cols,
            core::CV_32FC1,
            Scalar::all(1.0),
        )?;
        for (x, y, r) in exclude {
            imgproc::circle(
                &mut mask,
                Point::new(*x, *y),
                r + 10,
                Scalar::all(0.0),
                imgproc::FILLED,
                imgproc::LINE_8,
                0,
            )?;
        }

        // blur(imgÂ·mask) and blur(mask), then divide per-pixel below.
        let mut mask3 = Mat::default();
        let mut channels: core::Vector<Mat> = core::Vector::new();
        channels.push(mask.clone());
        channels.push(mask.clone());
        channels.push(mask.clone());
        core::merge(&channels, &mut mask3)?;

        let mut masked_img = Mat::default();
        core::multiply(&as_f32, &mask3, &mut masked_img, 1.0, -1)?;

        let ksize = core::Size::new(101, 101);
        let anchor = Point::new(-1, -1);
        let mut blurred_img = Mat::default();
        imgproc::blur(&masked_img, &mut blurred_img, ksize, anchor, core::BORDER_DEFAULT)?;
        let mut blurred_mask = Mat::default();
        imgproc::blur(&mask, &mut blurred_mask, ksize, anchor, core::BORDER_DEFAULT)?;

        let mut map = Mat::new_rows_cols_with_default(
            rows,
            cols,
            core::CV_8UC1,
            Scalar::all(0.0),
        )?;

        const GAIN: f32 = 600.0;
        let total = (rows as usize) * (cols as usize);
        if as_f32.is_continuous()
            && blurred_img.is_continuous()
            && blurred_mask.is_continuous()
            && map.is_continuous()
        {
            // Fast path: linear slice access â€” one bounds-checked call per
            // Mat instead of one per pixel (4 Ã— ~384k checked `at_2d` calls
            // otherwise dominate this function).
            let px_s: &[core::Vec3f] = as_f32.data_typed()?;
            let bi_s: &[core::Vec3f] = blurred_img.data_typed()?;
            let bm_s: &[f32] = blurred_mask.data_typed()?;
            let out_s: &mut [u8] = map.data_typed_mut()?;
            for i in 0..total {
                let px = px_s[i];
                let bi = bi_s[i];
                let wv = bm_s[i].max(0.05);
                let lb = [bi[0] / wv, bi[1] / wv, bi[2] / wv];
                let pn = (px[0] * px[0] + px[1] * px[1] + px[2] * px[2])
                    .sqrt()
                    .max(1e-6);
                let ln = (lb[0] * lb[0] + lb[1] * lb[1] + lb[2] * lb[2])
                    .sqrt()
                    .max(1e-6);
                let d0 = px[0] / pn - lb[0] / ln;
                let d1 = px[1] / pn - lb[1] / ln;
                let d2 = px[2] / pn - lb[2] / ln;
                let dist = (d0 * d0 + d1 * d1 + d2 * d2).sqrt() * GAIN;
                out_s[i] = dist.min(255.0) as u8;
            }
        } else {
            for row in 0..rows {
                for col in 0..cols {
                    let px: &core::Vec3f = as_f32.at_2d(row, col)?;
                    let bi: &core::Vec3f = blurred_img.at_2d(row, col)?;
                    let wv = blurred_mask.at_2d::<f32>(row, col)?.max(0.05);
                    let lb = [bi[0] / wv, bi[1] / wv, bi[2] / wv];
                    let pn = (px[0] * px[0] + px[1] * px[1] + px[2] * px[2])
                        .sqrt()
                        .max(1e-6);
                    let ln = (lb[0] * lb[0] + lb[1] * lb[1] + lb[2] * lb[2])
                        .sqrt()
                        .max(1e-6);
                    let d0 = px[0] / pn - lb[0] / ln;
                    let d1 = px[1] / pn - lb[1] / ln;
                    let d2 = px[2] / pn - lb[2] / ln;
                    let dist = (d0 * d0 + d1 * d1 + d2 * d2).sqrt() * GAIN;
                    *map.at_2d_mut::<u8>(row, col)? = dist.min(255.0) as u8;
                }
            }
        }

        let mut blurred = Mat::default();
        imgproc::gaussian_blur(
            &map,
            &mut blurred,
            core::Size::new(3, 3),
            0.0,
            0.0,
            core::BORDER_DEFAULT,
        )?;
        Ok(blurred)
    }

    fn preprocess(&self, image: &Mat) -> Result<Mat> {
        use PreprocessingMethod::*;
        Ok(match self.config.preprocessing {
            None => image.clone(),
            GaussianBlur => {
                let mut out = Mat::default();
                imgproc::gaussian_blur(
                    image,
                    &mut out,
                    core::Size::new(
                        self.config.blur_kernel_size,
                        self.config.blur_kernel_size,
                    ),
                    0.0,
                    0.0,
                    core::BORDER_DEFAULT,
                )?;
                out
            }
            MedianBlur => {
                let mut out = Mat::default();
                imgproc::median_blur(image, &mut out, self.config.blur_kernel_size)?;
                out
            }
            BilateralFilter => {
                let mut out = Mat::default();
                imgproc::bilateral_filter(
                    image,
                    &mut out,
                    9,
                    75.0,
                    75.0,
                    core::BORDER_DEFAULT,
                )?;
                out
            }
            CLAHE => {
                let mut clahe = imgproc::create_clahe(2.0, core::Size::new(8, 8))?;
                let mut out = Mat::default();
                clahe.apply(image, &mut out)?;
                out
            }
        })
    }

    fn analyze_color(
        &self,
        color_image: &Mat,
        cx: i32,
        cy: i32,
        radius: i32,
    ) -> Result<ColorAnalysisResult> {
        let sr = ((radius as f32) * 0.70) as i32;

        let empty = || ColorAnalysisResult {
            mean: (0, 0, 0),
            median: (0, 0, 0),
            dominant: (0, 0, 0),
            has_number: false,
        };

        if sr < 1 {
            return Ok(empty());
        }

        let mut mask = Mat::new_rows_cols_with_default(
            color_image.rows(),
            color_image.cols(),
            core::CV_8UC1,
            Scalar::all(0.0),
        )?;

        imgproc::circle(
            &mut mask,
            Point::new(cx, cy),
            sr,
            Scalar::all(255.0),
            -1,
            imgproc::LINE_8,
            0,
        )?;

        let mut pixels: Vec<(u8, u8, u8)> = Vec::new();
        let x0 = (cx - sr).max(0);
        let y0 = (cy - sr).max(0);
        let x1 = (cx + sr).min(color_image.cols() - 1);
        let y1 = (cy + sr).min(color_image.rows() - 1);

        for row in y0..=y1 {
            for col in x0..=x1 {
                if *mask.at_2d::<u8>(row, col)? > 0 {
                    let bgr: &Vec3b = color_image.at_2d(row, col)?;
                    let r = bgr[2];
                    let g = bgr[1];
                    let b = bgr[0];
                    pixels.push((r, g, b));
                }
            }
        }

        if pixels.is_empty() {
            return Ok(empty());
        }

        let n = pixels.len() as u32;
        let mean = (
            (pixels.iter().map(|(r, _, _)| *r as u32).sum::<u32>() / n) as u8,
            (pixels.iter().map(|(_, g, _)| *g as u32).sum::<u32>() / n) as u8,
            (pixels.iter().map(|(_, _, b)| *b as u32).sum::<u32>() / n) as u8,
        );

        let mut rv: Vec<u8> = pixels.iter().map(|(r, _, _)| *r).collect();
        let mut gv: Vec<u8> = pixels.iter().map(|(_, g, _)| *g).collect();
        let mut bv: Vec<u8> = pixels.iter().map(|(_, _, b)| *b).collect();
        rv.sort_unstable();
        gv.sort_unstable();
        bv.sort_unstable();
        let mid = pixels.len() / 2;
        let median = (rv[mid], gv[mid], bv[mid]);

        let dominant = self.find_dominant_color(&pixels);

        // Atomic-number detection (template-free). Numbered atoms print light
        // digits in the lower third of the glyph; specials (_Plus/_Minus/...)
        // draw a bare symbol and leave that band empty. We count near-WHITE
        // pixels (high in every channel) in the lower band — keying on white
        // rather than "bright" makes this independent of fill colour, so a
        // blue _Minus fill or a purple Argon fill never trips it; only the
        // white digit text does.
        const WHITE_THR: u8 = 140; // a pixel is "text" when R,G,B all exceed this
        const NUMBER_ROW_MIN: u32 = 5; // digits present when >= this many such pixels
        let nr_in = ((radius as f32) * 0.18) as i32;
        let nr_out = ((radius as f32) * 0.80) as i32;
        let band_r = ((radius as f32) * 0.72) as i32;
        let band_r2 = (band_r * band_r) as i64;
        let mut white_count: u32 = 0;
        let by0 = (cy + nr_in).max(0);
        let by1 = (cy + nr_out).min(color_image.rows() - 1);
        let bx0 = (cx - band_r).max(0);
        let bx1 = (cx + band_r).min(color_image.cols() - 1);
        for row in by0..=by1 {
            for col in bx0..=bx1 {
                let dx = (col - cx) as i64;
                let dy = (row - cy) as i64;
                if dx * dx + dy * dy > band_r2 {
                    continue;
                }
                let bgr: &Vec3b = color_image.at_2d(row, col)?;
                if bgr[0] > WHITE_THR && bgr[1] > WHITE_THR && bgr[2] > WHITE_THR {
                    white_count += 1;
                }
            }
        }
        let has_number = white_count >= NUMBER_ROW_MIN;

        println!(
            "  Circle ({:>4},{:>4}) r={:>3} | Mean RGB({:>3},{:>3},{:>3}) | Median RGB({:>3},{:>3},{:>3}) | Dominant RGB({:>3},{:>3},{:>3}) | numrow={:>3} ({})",
            cx, cy, radius,
            mean.0, mean.1, mean.2,
            median.0, median.1, median.2,
            dominant.0, dominant.1, dominant.2,
            white_count, if has_number { "numbered" } else { "special?" },
        );

        Ok(ColorAnalysisResult {
            mean,
            median,
            dominant,
            has_number,
        })
    }

    fn find_dominant_color(&self, pixels: &[(u8, u8, u8)]) -> (u8, u8, u8) {
        if pixels.is_empty() {
            return (0, 0, 0);
        }

        let mut counts: HashMap<(u8, u8, u8), usize> = HashMap::new();
        for &(r, g, b) in pixels {
            let key = ((r / 16) * 16, (g / 16) * 16, (b / 16) * 16);
            *counts.entry(key).or_insert(0) += 1;
        }

        counts
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|((r, g, b), _)| (r.saturating_add(8), g.saturating_add(8), b.saturating_add(8)))
            .unwrap_or((0, 0, 0))
    }
}

impl Default for CircleDetector {
    fn default() -> Self {
        Self::new(CircleDetectionConfig::default())
    }
}
