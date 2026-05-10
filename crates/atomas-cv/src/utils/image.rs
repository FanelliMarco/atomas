use crate::Result;
use anyhow::Context;
use opencv::{
    core::Mat,
    imgcodecs::{self, IMREAD_COLOR, IMREAD_GRAYSCALE},
    prelude::*,
};
use std::path::Path;

pub struct ImageUtils;

impl ImageUtils {
    pub fn load_grayscale<P: AsRef<Path>>(path: P) -> Result<Mat> {
        let path_str = path.as_ref().to_string_lossy();
        let mat = imgcodecs::imread(&path_str, IMREAD_GRAYSCALE)
            .with_context(|| format!("Failed to load grayscale image: {}", path_str))?;

        if mat.empty() {
            anyhow::bail!("Loaded grayscale image is empty: {}", path_str);
        }

        Ok(mat)
    }

    pub fn load_color<P: AsRef<Path>>(path: P) -> Result<Mat> {
        let path_str = path.as_ref().to_string_lossy();
        let mat = imgcodecs::imread(&path_str, IMREAD_COLOR)
            .with_context(|| format!("Failed to load color image: {}", path_str))?;

        if mat.empty() {
            anyhow::bail!("Loaded color image is empty: {}", path_str);
        }

        Ok(mat)
    }

    pub fn save_image<P: AsRef<Path>>(mat: &Mat, path: P) -> Result<()> {
        let path_str = path.as_ref().to_string_lossy();

        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create output dir: {:?}", parent))?;
        }

        imgcodecs::imwrite(&path_str, mat, &opencv::core::Vector::new())
            .with_context(|| format!("Failed to save image: {}", path_str))?;

        Ok(())
    }

    pub fn rgb_to_mat(rgb_image: &image::RgbImage) -> Result<Mat> {
        let (width, height) = rgb_image.dimensions();
        let mut bgr_mat = Mat::new_rows_cols_with_default(
            height as i32,
            width as i32,
            opencv::core::CV_8UC3,
            opencv::core::Scalar::all(0.0),
        )?;

        for y in 0..height {
            for x in 0..width {
                let pixel = rgb_image.get_pixel(x, y);
                let bgr = opencv::core::Vec3b::from([pixel[2], pixel[1], pixel[0]]);
                *bgr_mat.at_2d_mut::<opencv::core::Vec3b>(y as i32, x as i32)? = bgr;
            }
        }

        Ok(bgr_mat)
    }
}

