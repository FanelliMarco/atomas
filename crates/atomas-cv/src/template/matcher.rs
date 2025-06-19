//! Template matching implementation using OpenCV

use super::{Template, TemplateConfig};
use crate::bbox::{BBox, BBoxCollection};
use crate::traits::TemplateMatchable;
use crate::utils::ImageUtils;
use crate::Result;
use anyhow::Context;
use opencv::{
    core::{self, Mat, Size},
    imgproc::{self, TM_CCOEFF_NORMED, TM_CCORR_NORMED},
    prelude::*,
};

/// OpenCV-based template matcher with opencv-match integration
pub struct TemplateMatcher {
    config: TemplateConfig,
}

impl TemplateMatcher {
    /// Create new template matcher
    pub fn new(config: TemplateConfig) -> Self {
        Self { config }
    }

    /// Match single template against image with multi-scale support
    pub fn match_single(&self, image: &Mat, template: &Template) -> Result<BBoxCollection> {
        let mut all_matches = BBoxCollection::new();

        // Multi-scale template matching if configured
        for &scale in &self.config.scale_factors {
            let scaled_template = if (scale - 1.0).abs() < f64::EPSILON {
                template.image.clone()
            } else {
                self.scale_template(&template.image, scale)?
            };

            let matches = self.match_template_single_scale(image, &scaled_template, &template.name)?;
            all_matches.extend(matches);
        }

        // Apply NMS to remove overlapping detections
        let result = all_matches
            .filter_by_confidence(self.config.threshold)
            .apply_nms(self.config.nms_threshold);

        // Limit number of detections per template
        let mut limited = result.as_slice().to_vec();
        limited.truncate(self.config.max_detections_per_template);

        Ok(BBoxCollection::from_vec(limited))
    }

    /// Match multiple templates with optional parallel processing
    pub fn match_multiple(&self, image: &Mat, templates: &[Template]) -> Result<BBoxCollection> {
        let mut all_matches = BBoxCollection::new();

        // Use rayon for parallel template matching if available
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

        // Apply global NMS across all templates
        Ok(all_matches.apply_global_nms(self.config.nms_threshold))
    }

    /// Advanced template matching with opencv-match features
    pub fn match_with_nms(&self, image: &Mat, template: &Template) -> Result<BBoxCollection> {
        // Perform basic template matching
        let matches = self.match_single(image, template)?;

        // Apply additional NMS if needed
        Ok(matches.apply_nms(self.config.nms_threshold))
    }

    /// Scale template image using OpenCV
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

    /// Core template matching at single scale
    fn match_template_single_scale(
        &self,
        image: &Mat,
        template: &Mat,
        template_name: &str,
    ) -> Result<BBoxCollection> {
        let method = if self.config.use_normalized_correlation {
            TM_CCOEFF_NORMED
        } else {
            TM_CCORR_NORMED
        };

        let mut result = Mat::default();
        imgproc::match_template(
            image,
            template,
            &mut result,
            method,
            &core::no_array(),
        ).context("Template matching failed")?;

        let template_size = template.size()?;
        let mut matches = Vec::new();

        // Convert result to f64 for easier processing
        let mut result_f64 = Mat::default();
        result.convert_to(&mut result_f64, core::CV_64F, 1.0, 0.0)?;

        // Find all matches above threshold
        for y in 0..result.rows() {
            for x in 0..result.cols() {
                let confidence: f64 = *result_f64.at_2d(y, x)?;

                if confidence >= self.config.threshold {
                    let bbox = BBox::new(
                        x,
                        y,
                        template_size.width,
                        template_size.height,
                        confidence,
                    ).with_class(template_name.to_string(), (255, 255, 255));

                    matches.push(bbox);
                }
            }
        }

        Ok(BBoxCollection::from_vec(matches))
    }

    /// Batch process multiple images with templates
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
    fn test_template_matching() -> Result<()> {
        // Create dummy image and template
        let image = Mat::new_rows_cols_with_default(100, 100, CV_8UC1, opencv::core::Scalar::all(128.0))?;
        let template_mat = Mat::new_rows_cols_with_default(20, 20, CV_8UC1, opencv::core::Scalar::all(128.0))?;
        let template = Template::new("test".to_string(), template_mat);
        
        let matcher = TemplateMatcher::default();
        let matches = matcher.match_single(&image, &template)?;
        
        // Should find at least one match since template matches image
        assert!(!matches.is_empty());
        Ok(())
    }
}
