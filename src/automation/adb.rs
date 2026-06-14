//! ADB (Android Debug Bridge) wrapper
//!
//! Provides functions for interacting with Android emulator/device:
//! - Device detection and validation
//! - Screenshot capture
//! - Touch input (tap execution)

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// ADB device information
#[derive(Debug, Clone)]
pub struct AdbDevice {
    pub serial: String,
    pub status: String,
}

/// Check if ADB is available on the system
pub fn check_adb_available() -> Result<()> {
    let output = Command::new("adb")
        .arg("version")
        .output()
        .context("Failed to execute 'adb version'. Is ADB installed and in PATH?")?;

    if !output.status.success() {
        anyhow::bail!("ADB command failed. Please install Android SDK Platform Tools.");
    }

    let version = String::from_utf8_lossy(&output.stdout);
    log::info!(
        "ADB available: {}",
        version.lines().next().unwrap_or("unknown")
    );

    Ok(())
}

/// Get list of connected devices
pub fn get_devices() -> Result<Vec<AdbDevice>> {
    let output = Command::new("adb")
        .arg("devices")
        .output()
        .context("Failed to execute 'adb devices'")?;

    if !output.status.success() {
        anyhow::bail!("ADB devices command failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in stdout.lines().skip(1) {
        // Skip "List of devices attached" header
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            devices.push(AdbDevice {
                serial: parts[0].to_string(),
                status: parts[1].to_string(),
            });
        }
    }

    Ok(devices)
}

/// Select device to use
///
/// If device_serial is provided, validates it exists
/// If None, uses the first available device
/// Fails if no devices are connected
pub fn select_device(device_serial: Option<&str>) -> Result<String> {
    let devices = get_devices().context("Failed to get device list")?;

    if devices.is_empty() {
        anyhow::bail!(
            "No devices connected. Please start an emulator or connect a device.\n\
             Check with: adb devices"
        );
    }

    let selected = if let Some(serial) = device_serial {
        // Validate requested device exists
        devices.iter().find(|d| d.serial == serial).ok_or_else(|| {
            anyhow::anyhow!(
                "Device '{}' not found. Available devices: {:?}",
                serial,
                devices.iter().map(|d| &d.serial).collect::<Vec<_>>()
            )
        })?
    } else {
        // Use first device
        &devices[0]
    };

    log::info!("Selected device: {} ({})", selected.serial, selected.status);

    if selected.status != "device" {
        log::warn!(
            "Device status is '{}', expected 'device'. May not be ready.",
            selected.status
        );
    }

    Ok(selected.serial.clone())
}

/// Capture screenshot from device
///
/// Executes: adb exec-out screencap -p > output_path
/// Uses -s to specify device if multiple devices connected
pub fn capture_screenshot<P: AsRef<Path>>(device_serial: &str, output_path: P) -> Result<PathBuf> {
    let output_path = output_path.as_ref();

    // Create parent directory if needed
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create screenshot directory")?;
    }

    log::debug!(
        "Capturing screenshot from device {} to {:?}",
        device_serial,
        output_path
    );

    // Execute: adb -s <device> exec-out screencap -p
    let output = Command::new("adb")
        .args(&["-s", device_serial, "exec-out", "screencap", "-p"])
        .output()
        .context("Failed to execute adb screencap command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Screenshot capture failed: {}", stderr);
    }

    // Write PNG data to file
    std::fs::write(output_path, &output.stdout).context("Failed to write screenshot to file")?;

    log::debug!("Screenshot saved: {:?}", output_path);

    Ok(output_path.to_path_buf())
}

/// Execute tap at coordinates
///
/// Executes: adb shell input tap x y
pub fn execute_tap(device_serial: &str, x: i32, y: i32) -> Result<()> {
    log::debug!("Executing tap on device {}: ({}, {})", device_serial, x, y);

    let output = Command::new("adb")
        .args(&[
            "-s",
            device_serial,
            "shell",
            "input",
            "tap",
            &x.to_string(),
            &y.to_string(),
        ])
        .output()
        .context("Failed to execute adb tap command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Tap execution failed: {}", stderr);
    }

    log::debug!("Tap executed successfully");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires ADB available
    fn test_check_adb() {
        let result = check_adb_available();
        println!("ADB check: {:?}", result);
    }

    #[test]
    #[ignore] // Requires device connected
    fn test_get_devices() {
        let devices = get_devices().unwrap();
        println!("Devices: {:?}", devices);
        assert!(!devices.is_empty());
    }
}
