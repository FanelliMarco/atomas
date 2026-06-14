//! Action execution module
//!
//! Handles executing tap actions on the game
//! Phase 1: Dry-run mode (prints commands)
//! Phase 2: Real ADB tap execution

use super::adb;
use anyhow::Result;
use atomas_cv::ActionCoordinates;
use std::time::Duration;

/// Execution mode
#[derive(Debug, Clone)]
pub enum ExecutionMode {
    /// Dry-run: print commands without executing
    DryRun,
    /// Real ADB execution
    Adb {
        device_serial: String,
        delay_ms: u64,
    },
}

/// Action executor
pub struct ActionExecutor {
    mode: ExecutionMode,
}

impl ActionExecutor {
    /// Create a new executor with the given mode
    pub fn new(mode: ExecutionMode) -> Self {
        Self { mode }
    }

    /// Execute a tap at the given coordinates
    ///
    /// In dry-run mode, this prints the command that would be executed
    /// In ADB mode, this executes `adb shell input tap x y`
    pub fn execute_tap(&self, coordinates: ActionCoordinates, move_number: usize) -> Result<()> {
        match &self.mode {
            ExecutionMode::DryRun => self.dry_run_tap(coordinates, move_number),
            ExecutionMode::Adb {
                device_serial,
                delay_ms,
            } => self.adb_tap(device_serial, coordinates, move_number, *delay_ms),
        }
    }

    /// Dry-run mode: print the tap command without executing
    fn dry_run_tap(&self, coordinates: ActionCoordinates, move_number: usize) -> Result<()> {
        log::info!("[Move {}/N] Executing tap (DRY-RUN)...", move_number);
        log::info!(
            "  Would execute: adb shell input tap {} {}",
            coordinates.x,
            coordinates.y
        );
        log::info!("  Status: Skipped (dry-run mode)");

        Ok(())
    }

    /// ADB mode: execute real tap on device
    fn adb_tap(
        &self,
        device_serial: &str,
        coordinates: ActionCoordinates,
        move_number: usize,
        delay_ms: u64,
    ) -> Result<()> {
        log::info!("[Move {}/N] Executing tap (ADB)...", move_number);
        log::info!(
            "  Command: adb shell input tap {} {}",
            coordinates.x,
            coordinates.y
        );

        adb::execute_tap(device_serial, coordinates.x, coordinates.y)?;

        log::info!("  ✓ Tap executed");

        // Wait delay before next action
        if delay_ms > 0 {
            log::debug!("  Waiting {} ms...", delay_ms);
            std::thread::sleep(Duration::from_millis(delay_ms));
        }

        Ok(())
    }
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new(ExecutionMode::DryRun)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dry_run_executor() {
        let executor = ActionExecutor::new(ExecutionMode::DryRun);
        let coords = ActionCoordinates::new(100, 200);
        assert!(executor.execute_tap(coords, 1).is_ok());
    }
}
