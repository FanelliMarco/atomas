//! Screenshot capture module
//!
//! Handles loading screenshots from disk (dry-run mode)
//! or capturing from ADB device (Phase 2)

use super::adb;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Source of game screenshots
#[derive(Debug, Clone)]
pub enum ScreenshotSource {
    /// Load from a file on disk (dry-run mode)
    File(PathBuf),
    /// Capture from ADB device
    Adb {
        device_serial: String,
        output_path: PathBuf,
    },
}

impl ScreenshotSource {
    /// Create a file-based source
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        Self::File(path.as_ref().to_path_buf())
    }

    /// Create an ADB-based source
    pub fn from_adb<P: AsRef<Path>>(device_serial: String, output_path: P) -> Self {
        Self::Adb {
            device_serial,
            output_path: output_path.as_ref().to_path_buf(),
        }
    }

    /// Check if the screenshot source is available
    pub fn is_available(&self) -> Result<()> {
        match self {
            Self::File(path) => {
                if !path.exists() {
                    anyhow::bail!("Screenshot file not found: {:?}", path);
                }
                Ok(())
            }
            Self::Adb { device_serial, .. } => {
                // Check ADB available and device connected
                adb::check_adb_available().context("ADB not available")?;

                // Verify device exists
                let devices = adb::get_devices().context("Failed to get device list")?;
                if !devices.iter().any(|d| d.serial == *device_serial) {
                    anyhow::bail!("Device '{}' not connected", device_serial);
                }

                Ok(())
            }
        }
    }

    /// Capture a screenshot
    ///
    /// In dry-run mode, this just validates the file exists
    /// In ADB mode, this executes `adb exec-out screencap -p`
    pub fn capture(&self, move_number: usize) -> Result<PathBuf> {
        match self {
            Self::File(path) => {
                log::info!("[Move {}/N] Capturing screenshot...", move_number);
                log::info!("  Source: {:?} (dry-run mode)", path);

                if !path.exists() {
                    anyhow::bail!("Screenshot file not found: {:?}", path);
                }

                Ok(path.clone())
            }
            Self::Adb {
                device_serial,
                output_path,
            } => {
                log::info!(
                    "[Move {}/N] Capturing screenshot from device...",
                    move_number
                );
                log::info!("  Device: {}", device_serial);
                log::info!("  Output: {:?}", output_path);

                let path = adb::capture_screenshot(device_serial, output_path)
                    .context("Failed to capture screenshot via ADB")?;

                log::info!("  ✓ Screenshot captured");

                Ok(path)
            }
        }
    }
}

impl Default for ScreenshotSource {
    fn default() -> Self {
        // Default to sample screenshot from Milestone 1
        Self::from_file("assets/jpg/board.jpg")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_source() {
        let source = ScreenshotSource::from_file("assets/jpg/board.jpg");
        assert!(matches!(source, ScreenshotSource::File(_)));
    }
}
