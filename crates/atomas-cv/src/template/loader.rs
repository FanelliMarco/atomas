//! Template loading utilities

use super::Template;
use crate::utils::image::ImageUtils;
use crate::Result;
use anyhow::Context;
use std::fs;
use std::path::{Path, PathBuf};

/// Template loader with multiple search strategies
pub struct TemplateLoader {
    template_dirs: Vec<PathBuf>,
    supported_extensions: Vec<String>,
}

impl TemplateLoader {
    /// Create new template loader
    pub fn new() -> Self {
        Self {
            template_dirs: Vec::new(),
            supported_extensions: vec![
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "bmp".to_string(),
            ],
        }
    }

    /// Add template directory
    pub fn add_template_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.template_dirs.push(dir.as_ref().to_path_buf());
        self
    }

    /// Add supported extension
    pub fn add_extension(mut self, ext: String) -> Self {
        self.supported_extensions.push(ext);
        self
    }

    /// Load template by name with multiple search strategies
    pub fn load_template(&self, name: &str) -> Result<Option<Template>> {
        let candidates = self.generate_template_candidates(name);
        
        for candidate in candidates {
            if let Some(path) = self.find_template_file(&candidate)? {
                let image = ImageUtils::load_grayscale(&path)
                    .with_context(|| format!("Failed to load template: {:?}", path))?;
                
                return Ok(Some(Template::new(name.to_string(), image)
                    .with_metadata("path".to_string(), path.to_string_lossy().to_string())
                    .with_metadata("original_name".to_string(), candidate)));
            }
        }

        Ok(None)
    }

    /// Load all templates from directories
    pub fn load_all_templates(&self) -> Result<Vec<Template>> {
        let mut templates = Vec::new();

        for dir in &self.template_dirs {
            if !dir.exists() {
                continue;
            }

            let entries = fs::read_dir(dir)
                .with_context(|| format!("Failed to read directory: {:?}", dir))?;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                
                if let Some(extension) = path.extension() {
                    let ext = extension.to_string_lossy().to_lowercase();
                    if self.supported_extensions.contains(&ext) {
                        if let Some(stem) = path.file_stem() {
                            let name = stem.to_string_lossy().to_string();
                            let image = ImageUtils::load_grayscale(&path)?;
                            
                            templates.push(Template::new(name, image)
                                .with_metadata("path".to_string(), path.to_string_lossy().to_string()));
                        }
                    }
                }
            }
        }

        Ok(templates)
    }

    /// Generate template name candidates
    fn generate_template_candidates(&self, name: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        
        for ext in &self.supported_extensions {
            // Exact name
            candidates.push(format!("{}.{}", name, ext));
            candidates.push(format!("{}.{}", name.to_lowercase(), ext));
            
            // With underscore prefix
            candidates.push(format!("_{}.{}", name, ext));
            candidates.push(format!("_{}.{}", name.to_lowercase(), ext));
            
            // Various case transformations
            candidates.push(format!("{}.{}", name.to_uppercase(), ext));
        }

        candidates
    }

    /// Find template file in directories
    fn find_template_file(&self, candidate: &str) -> Result<Option<PathBuf>> {
        for dir in &self.template_dirs {
            let path = dir.join(candidate);
            if path.exists() {
                return Ok(Some(path));
            }

            // Case-insensitive search
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name();
                    let file_name_str = file_name.to_string_lossy();
                    
                    if file_name_str.to_lowercase() == candidate.to_lowercase() {
                        return Ok(Some(entry.path()));
                    }
                }
            }
        }

        Ok(None)
    }
}

impl Default for TemplateLoader {
    fn default() -> Self {
        Self::new()
    }
}
