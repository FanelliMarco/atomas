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
}

struct ColorAnalysisResult {
    mean: (u8, u8, u8),
    median: (u8, u8, u8),
    dominant: (u8, u8, u8),
}

impl CircleDetector {
    pub fn new(config: CircleDetectionConfig) -> Self {
        Self { config }
    }

    pub fn detect(&self, gray_image: &Mat, color_image: &Mat) -> Result<Vec<DetectedCircle>> {
        let processed = self.preprocess(gray_image)?;

        let mut circles = Mat::default();
        imgproc::hough_circles(
            &processed,
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
            println!("HoughCircles: 0 circles found");
            return Ok(Vec::new());
        }

        let mut detected = Vec::new();

        for i in 0..circles.cols() {
            let d: &Vec3f = circles.at(i)?;
            let x = d[0] as i32;
            let y = d[1] as i32;
            let r = d[2] as i32;

            let ca = self.analyze_color(color_image, x, y, r)?;

            detected.push(DetectedCircle {
                circle: Circle::new((x, y), r, 1.0),
                mean_color: ca.mean,
                median_color: ca.median,
                dominant_color: ca.dominant,
            });
        }

        println!("HoughCircles detected {} circles", detected.len());
        Ok(detected)
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

        println!(
            "  Circle ({:>4},{:>4}) r={:>3} | Mean RGB({:>3},{:>3},{:>3}) | Median RGB({:>3},{:>3},{:>3}) | Dominant RGB({:>3},{:>3},{:>3})",
            cx, cy, radius,
            mean.0, mean.1, mean.2,
            median.0, median.1, median.2,
            dominant.0, dominant.1, dominant.2,
        );

        Ok(ColorAnalysisResult {
            mean,
            median,
            dominant,
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
