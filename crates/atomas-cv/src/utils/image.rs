//! Image processing utilities using opencv-match conversions

use crate::Result;
use anyhow::Context;
use opencv::{
    core::Mat,
    imgcodecs::{self, IMREAD_GRAYSCALE, IMREAD_COLOR},
};
use opencv_match::prelude::*;
use std::path::Path;

/// Image utility functions leveraging opencv-match conversions
pub struct ImageUtils;

impl ImageUtils {
    /// Load image as grayscale Mat using opencv-match
    pub fn load_grayscale<P: AsRef<Path>>(path: P) -> Result<Mat> {
        let img = image::open(&path)
            .with_context(|| format!("Failed to open image: {:?}", path.as_ref()))?
            .to_rgba8();

        opencv_match::convert::mat_to_grayscale(&img.try_into_cv()?, true)
            .context("Failed to convert image to grayscale")
    }

    /// Load image as color Mat (BGR) using opencv-match
    pub fn load_color<P: AsRef<Path>>(path: P) -> Result<Mat> {
        let img = image::open(&path)
            .with_context(|| format!("Failed to open image: {:?}", path.as_ref()))?
            .to_rgb8();

        img.try_into_cv()
            .context("Failed to convert image to OpenCV Mat")
    }

    /// Save Mat as image
    pub fn save_image<P: AsRef<Path>>(mat: &Mat, path: P) -> Result<()> {
        let path_str = path.as_ref().to_string_lossy();
        
        imgcodecs::imwrite(&path_str, mat, &opencv::core::Vector::new())
            .with_context(|| format!("Failed to save image: {}", path_str))?;
            
        Ok(())
    }

    /// Convert image::RgbaImage to OpenCV Mat using opencv-match
    pub fn rgba_to_mat(rgba_image: &image::RgbaImage) -> Result<Mat> {
        rgba_image.try_into_cv()
            .context("Failed to convert RGBA image to OpenCV Mat")
    }

    /// Convert image::RgbImage to OpenCV Mat using opencv-match
    pub fn rgb_to_mat(rgb_image: &image::RgbImage) -> Result<Mat> {
        rgb_image.try_into_cv()
            .context("Failed to convert RGB image to OpenCV Mat")
    }

    /// Convert OpenCV Mat to image::RgbImage using opencv-match
    pub fn mat_to_rgb(mat: &Mat) -> Result<image::RgbImage> {
        mat.try_into_cv()
            .context("Failed to convert OpenCV Mat to RGB image")
    }

    /// Convert OpenCV Mat to image::RgbaImage using opencv-match
    pub fn mat_to_rgba(mat: &Mat) -> Result<image::RgbaImage> {
        mat.try_into_cv()
            .context("Failed to convert OpenCV Mat to RGBA image")
    }

    /// Load image directly from path as OpenCV Mat (grayscale)
    pub fn load_mat_grayscale<P: AsRef<Path>>(path: P) -> Result<Mat> {
        let path_str = path.as_ref().to_string_lossy();
        
        imgcodecs::imread(&path_str, IMREAD_GRAYSCALE)
            .with_context(|| format!("Failed to load grayscale image: {}", path_str))
    }

    /// Load image directly from path as OpenCV Mat (color)
    pub fn load_mat_color<P: AsRef<Path>>(path: P) -> Result<Mat> {
        let path_str = path.as_ref().to_string_lossy();
        
        imgcodecs::imread(&path_str, IMREAD_COLOR)
            .with_context(|| format!("Failed to load color image: {}", path_str))
    }

    /// Convert RGBA to RGB using image crate
    pub fn rgba_to_rgb(rgba_image: &image::RgbaImage) -> Result<image::RgbImage> {
        let (width, height) = rgba_image.dimensions();
        let mut rgb_image = image::RgbImage::new(width, height);
        
        for y in 0..height {
            for x in 0..width {
                let rgba_pixel = rgba_image.get_pixel(x, y);
                let rgb_pixel = image::Rgb([rgba_pixel[0], rgba_pixel[1], rgba_pixel[2]]);
                rgb_image.put_pixel(x, y, rgb_pixel);
            }
        }
        
        Ok(rgb_image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_conversions() -> Result<()> {
        // Create a test RGB image
        let rgb_img = image::RgbImage::new(100, 100);
        
        // Convert to Mat and back
        let mat = ImageUtils::rgb_to_mat(&rgb_img)?;
        let rgb_back = ImageUtils::mat_to_rgb(&mat)?;
        
        assert_eq!(rgb_img.dimensions(), rgb_back.dimensions());
        Ok(())
    }

    #[test]
    fn test_rgba_to_rgb_conversion() -> Result<()> {
        let rgba_img = image::RgbaImage::new(50, 50);
        let rgb_img = ImageUtils::rgba_to_rgb(&rgba_img)?;
        
        assert_eq!(rgba_img.dimensions(), rgb_img.dimensions());
        Ok(())
    }
}
