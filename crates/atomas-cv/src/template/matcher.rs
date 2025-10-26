//! Template matching implementation using OpenCV with gradient-based preprocessing

use super::{Template, TemplateConfig, PreprocessingMethod};
use crate::bbox::{BBox, BBoxCollection};
use crate::traits::TemplateMatchable;
use crate::utils::ImageUtils;
use crate::Result;
use anyhow::Context;
use opencv::{
    core::{self, Mat, Size, CV_64F},
    imgproc,
    prelude::*,
};

/// OpenCV-based template matcher with gradient preprocessing
pub struct TemplateMatcher {
    config: TemplateConfig,
}

impl TemplateMatcher {
    /// Create new template matcher
    pub fn new(config: TemplateConfig) -> Self {
        Self { config }
    }

    /// Match single template with preprocessing
    pub fn match_single(&self, image: &Mat, template: &Template) -> Result<BBoxCollection> {
        // Preprocess image and template
        let processed_image = self.preprocess(image)?;
        let processed_template = self.preprocess(&template.image)?;

        let mut all_matches = BBoxCollection::new();

        // Multi-scale template matching
        for &scale in &self.config.scale_factors {
            let scaled_template = if (scale - 1.0).abs() < f64::EPSILON {
                processed_template.clone()
            } else {
                self.scale_template(&processed_template, scale)?
            };

            let matches = self.match_template_single_scale(
                &processed_image,
                &scaled_template,
                &template.image.size()?, // Original size for bbox
                &template.name,
            )?;
            all_matches.extend(matches);
        }

        // Apply NMS
        let result = all_matches
            .filter_by_confidence(self.config.threshold)
            .apply_nms(self.config.nms_threshold);

        let mut limited = result.as_slice().to_vec();
        limited.truncate(self.config.max_detections_per_template);

        Ok(BBoxCollection::from_vec(limited))
    }

    /// Preprocess image based on configuration
    fn preprocess(&self, image: &Mat) -> Result<Mat> {
        use PreprocessingMethod::*;

        let processed = match self.config.preprocessing {
            None => image.clone(),
            HistogramEqualization => self.apply_histogram_equalization(image)?,
            Laplacian => self.apply_laplacian(image)?,
            SobelMagnitude => self.apply_sobel_magnitude(image)?,
            Canny => self.apply_canny(image)?,
            CLAHE => self.apply_clahe(image)?,
        };

        Ok(processed)
    }

    /// Apply histogram equalization
    fn apply_histogram_equalization(&self, image: &Mat) -> Result<Mat> {
        let mut equalized = Mat::default();
        imgproc::equalize_hist(image, &mut equalized)
            .context("Histogram equalization failed")?;
        Ok(equalized)
    }

    /// Apply Laplacian gradient (robust to illumination)
    fn apply_laplacian(&self, image: &Mat) -> Result<Mat> {
        let mut laplacian = Mat::default();
        
        imgproc::laplacian(
            image,
            &mut laplacian,
            CV_64F,
            self.config.preprocessing_params.laplacian_ksize,
            1.0,
            0.0,
            core::BORDER_DEFAULT,
        ).context("Laplacian failed")?;

        // Convert to absolute values
        let mut abs_laplacian = Mat::default();
        core::convert_scale_abs(&laplacian, &mut abs_laplacian, 1.0, 0.0)?;

        Ok(abs_laplacian)
    }

    /// Apply Sobel gradient magnitude
    fn apply_sobel_magnitude(&self, image: &Mat) -> Result<Mat> {
        let mut grad_x = Mat::default();
        let mut grad_y = Mat::default();

        // Compute x and y gradients
        imgproc::sobel(image, &mut grad_x, CV_64F, 1, 0, 3, 1.0, 0.0, core::BORDER_DEFAULT)?;
        imgproc::sobel(image, &mut grad_y, CV_64F, 0, 1, 3, 1.0, 0.0, core::BORDER_DEFAULT)?;

        // Compute magnitude: sqrt(grad_x^2 + grad_y^2)
        let mut grad_x_sq = Mat::default();
        let mut grad_y_sq = Mat::default();
        core::pow(&grad_x, 2.0, &mut grad_x_sq)?;
        core::pow(&grad_y, 2.0, &mut grad_y_sq)?;

        let mut magnitude_sq = Mat::default();
        core::add(&grad_x_sq, &grad_y_sq, &mut magnitude_sq, &core::no_array(), -1)?;

        let mut magnitude = Mat::default();
        core::sqrt(&magnitude_sq, &mut magnitude)?;

        // Convert to 8-bit
        let mut magnitude_8u = Mat::default();
        core::convert_scale_abs(&magnitude, &mut magnitude_8u, 1.0, 0.0)?;

        Ok(magnitude_8u)
    }

    /// Apply Canny edge detection
    fn apply_canny(&self, image: &Mat) -> Result<Mat> {
        let mut edges = Mat::default();
        
        imgproc::canny(
            image,
            &mut edges,
            self.config.preprocessing_params.canny_low,
            self.config.preprocessing_params.canny_high,
            3,
            false,
        )?;

        Ok(edges)
    }

    /// Apply CLAHE (Contrast Limited Adaptive Histogram Equalization)
    fn apply_clahe(&self, image: &Mat) -> Result<Mat> {
        let mut clahe = imgproc::create_clahe(
            self.config.preprocessing_params.clahe_clip_limit,
            core::Size::new(
                self.config.preprocessing_params.clahe_tile_size.0,
                self.config.preprocessing_params.clahe_tile_size.1,
            ),
        )?;

        let mut equalized = Mat::default();
        clahe.apply(image, &mut equalized)?;

        Ok(equalized)
    }

    /// Core template matching at single scale
    fn match_template_single_scale(
        &self,
        image: &Mat,
        template: &Mat,
        original_template_size: &Size,
        template_name: &str,
    ) -> Result<BBoxCollection> {
        let method = self.config.matching_method.to_opencv();

        let mut result = Mat::default();
        imgproc::match_template(
            image,
            template,
            &mut result,
            method,
            &core::no_array(),
        ).context("Template matching failed")?;

        let mut matches = Vec::new();

        // Convert result to f64
        let mut result_f64 = Mat::default();
        result.convert_to(&mut result_f64, CV_64F, 1.0, 0.0)?;

        // Find matches based on method type
        let is_inverted = self.config.matching_method.is_inverted();

        for y in 0..result.rows() {
            for x in 0..result.cols() {
                let confidence: f64 = *result_f64.at_2d(y, x)?;

                let passes_threshold = if is_inverted {
                    // For SQDIFF methods, lower is better
                    confidence <= self.config.threshold
                } else {
                    // For correlation methods, higher is better
                    confidence >= self.config.threshold
                };

                if passes_threshold {
                    // Normalize confidence for inverted methods
                    let normalized_confidence = if is_inverted {
                        1.0 - confidence // Convert to "higher is better"
                    } else {
                        confidence
                    };

                    let bbox = BBox::new(
                        x,
                        y,
                        original_template_size.width,
                        original_template_size.height,
                        normalized_confidence,
                    ).with_class(template_name.to_string(), (255, 255, 255));

                    matches.push(bbox);
                }
            }
        }

        Ok(BBoxCollection::from_vec(matches))
    }

    /// Match multiple templates
    pub fn match_multiple(&self, image: &Mat, templates: &[Template]) -> Result<BBoxCollection> {
        let mut all_matches = BBoxCollection::new();

        #[cfg(feature = "parallel")]
        {
            use rayon::prelude::*;
            let matches: Result<Vec<_>> = templates
                .par_iter()
                .map(|template| self.match_single(image, template))
                .collect();
            
            for template_matches in matches? {
                all_matches.extend(template_matches);
            }
        }

        #[cfg(not(feature = "parallel"))]
        {
            for template in templates {
                let matches = self.match_single(image, template)?;
                all_matches.extend(matches);
            }
        }

        Ok(all_matches.apply_global_nms(self.config.nms_threshold))
    }

    /// Scale template
    fn scale_template(&self, template: &Mat, scale: f64) -> Result<Mat> {
        let original_size = template.size()?;
        let new_width = (original_size.width as f64 * scale) as i32;
        let new_height = (original_size.height as f64 * scale) as i32;
        
        let mut scaled = Mat::default();
        imgproc::resize(
            template,
            &mut scaled,
            Size::new(new_width, new_height),
            0.0,
            0.0,
            imgproc::INTER_LINEAR,
        )?;

        Ok(scaled)
    }

    /// Batch process multiple images
    pub fn batch_match<P: AsRef<std::path::Path>>(
        &self, 
        image_paths: &[P], 
        templates: &[Template]
    ) -> Result<Vec<BBoxCollection>> {
        let mut results = Vec::new();

        for path in image_paths {
            let image = ImageUtils::load_grayscale(path)?;
            let matches = self.match_multiple(&image, templates)?;
            results.push(matches);
        }

        Ok(results)
    }
}

impl TemplateMatchable for TemplateMatcher {
    fn match_template(&self, image: &Mat, template: &Mat, threshold: f64) -> Result<Vec<BBox>> {
        let temp_template = Template::new("temp".to_string(), template.clone());
        let mut temp_config = self.config.clone();
        temp_config.threshold = threshold;
        
        let temp_matcher = TemplateMatcher::new(temp_config);
        let matches = temp_matcher.match_single(image, &temp_template)?;
        
        Ok(matches.into_iter().collect())
    }
}

impl Default for TemplateMatcher {
    fn default() -> Self {
        Self::new(TemplateConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencv::core::{Mat, CV_8UC1};

    #[test]
    fn test_laplacian_preprocessing() -> Result<()> {
        let image = Mat::new_rows_cols_with_default(100, 100, CV_8UC1, core::Scalar::all(128.0))?;
        
        let config = TemplateConfig::gradient_matching();
        let matcher = TemplateMatcher::new(config);
        
        let processed = matcher.preprocess(&image)?;
        assert!(!processed.empty());
        
        Ok(())
    }

    #[test]
    fn test_sqdiff_matching() -> Result<()> {
        let image = Mat::new_rows_cols_with_default(100, 100, CV_8UC1, core::Scalar::all(128.0))?;
        let template_mat = Mat::new_rows_cols_with_default(20, 20, CV_8UC1, core::Scalar::all(128.0))?;
        let template = Template::new("test".to_string(), template_mat);
        
        let config = TemplateConfig::sqdiff_laplacian();
        let matcher = TemplateMatcher::new(config);
        
        let matches = matcher.match_single(&image, &template)?;
        assert!(!matches.is_empty());
        
        Ok(())
    }
}
