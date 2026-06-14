//! Main automation loop controller
//!
//! Orchestrates: Capture â†’ Detect â†’ Decide â†’ Execute â†’ Repeat

use super::ActionExecutor;
use super::ScreenshotSource;
use anyhow::{Context, Result};
use atomas_core::elements::Data;
use atomas_cv::{
    Decision, DetectionConfig, DetectionResult, GameStateDetector, map_decision_to_coordinates,
};

/// Trait for solvers that can choose moves
pub trait DecisionSolver {
    fn choose_move(&mut self, detection: &DetectionResult) -> Result<Decision>;
}

// Implement for SimpleSolver
impl DecisionSolver for super::SimpleSolver {
    fn choose_move(&mut self, detection: &DetectionResult) -> Result<Decision> {
        // Call the inherent method on SimpleSolver directly, not the trait method.
        super::SimpleSolver::choose_move(self, detection)
    }
}

// Implement for ExpectimaxSolver
impl DecisionSolver for super::ExpectimaxSolver {
    fn choose_move(&mut self, detection: &DetectionResult) -> Result<Decision> {
        // Call the inherent method on ExpectimaxSolver directly, not the trait method.
        super::ExpectimaxSolver::choose_move(self, detection)
    }
}

/// Statistics for the automation loop
#[derive(Debug, Default, Clone)]
pub struct LoopStats {
    pub total_moves: usize,
    pub successful_moves: usize,
    pub failed_moves: usize,
    pub detection_failures: usize,
}

impl LoopStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_moves == 0 {
            0.0
        } else {
            (self.successful_moves as f64 / self.total_moves as f64) * 100.0
        }
    }
}

/// Main automation loop (generic over solver type)
pub struct AutomationLoop<S: DecisionSolver> {
    screenshot_source: ScreenshotSource,
    solver: S,
    executor: ActionExecutor,
    detector: GameStateDetector,
    elements_data: Data<'static>,
    max_moves: usize,
    stats: LoopStats,
}

impl<S: DecisionSolver> AutomationLoop<S> {
    /// Create a new automation loop
    pub fn new(
        screenshot_source: ScreenshotSource,
        solver: S,
        executor: ActionExecutor,
        max_moves: usize,
    ) -> Result<Self> {
        // Load element data
        let elements_path = format!("{}/assets/txt/elements.txt", env!("CARGO_MANIFEST_DIR"));
        let elements_data = Data::load(&elements_path);

        // Create detector with accurate color matching config
        let mut config = DetectionConfig::accurate_color_matching();
        config.output_dir = format!("{}/assets/png/outputs", env!("CARGO_MANIFEST_DIR")).into();
        let detector =
            GameStateDetector::new(config).context("Failed to create game state detector")?;

        Ok(Self {
            screenshot_source,
            solver,
            executor,
            detector,
            elements_data,
            max_moves,
            stats: LoopStats::default(),
        })
    }

    /// Run the automation loop for the configured number of moves
    pub fn run(&mut self) -> Result<()> {
        log::info!("==========================================================");
        log::info!("MILESTONE 2+3: Automation Loop with Solver");
        log::info!("Max moves: {}", self.max_moves);
        log::info!("==========================================================\n");

        // Validate screenshot source before starting
        self.screenshot_source
            .is_available()
            .context("Screenshot source not available")?;

        for move_number in 1..=self.max_moves {
            log::info!("----------------------------------------------------------");
            match self.execute_move(move_number) {
                Ok(_) => {
                    self.stats.successful_moves += 1;
                    log::info!("[Move {}/{}] âœ“ Complete\n", move_number, self.max_moves);
                }
                Err(e) => {
                    self.stats.failed_moves += 1;
                    log::error!(
                        "[Move {}/{}] âœ— Failed: {}\n",
                        move_number,
                        self.max_moves,
                        e
                    );
                }
            }
            self.stats.total_moves += 1;
        }

        self.print_summary();

        Ok(())
    }

    /// Execute a single move
    fn execute_move(&mut self, move_number: usize) -> Result<()> {
        // Steps 1+2: capture + detect until STABLE. Stability = two
        // consecutive detections (>=350ms apart) that agree on the board:
        // same sorted ring values and same player atom. A frame captured
        // mid-fusion-animation can look internally plausible, but it cannot
        // reproduce itself across two captures; a settled board can. (An
        // earlier heuristic — "ring shrank too much" — deadlocked after big
        // legitimate fusion chains, which really do consume 5+ atoms.)
        const STABILITY_ATTEMPTS: usize = 8;
        const STABILITY_WAIT_MS: u64 = 350;

        let summarize = |r: &DetectionResult<'_>| -> (Vec<i16>, Option<i16>) {
            let mut ring: Vec<i16> = r
                .ring_elements
                .iter()
                .map(|(e, _)| e.element_type.to_numeric())
                .collect();
            ring.sort_unstable();
            let player = r
                .player_atom
                .as_ref()
                .map(|(p, _)| p.element_type.to_numeric());
            (ring, player)
        };

        let mut prev_summary: Option<(Vec<i16>, Option<i16>)> = None;
        let mut detection_result = None;
        for attempt in 0..STABILITY_ATTEMPTS {
            let screenshot_path = self
                .screenshot_source
                .capture(move_number)
                .context("Failed to capture screenshot")?;

            log::info!(
                "[Move {}/{}] Detecting game state{}...",
                move_number,
                self.max_moves,
                if attempt > 0 {
                    format!(" (stability check {}/{})", attempt, STABILITY_ATTEMPTS - 1)
                } else {
                    String::new()
                }
            );
            let result = self
                .detector
                .detect_from_file(&screenshot_path, &self.elements_data)
                .context("Failed to detect game state")?;

            let summary = summarize(&result);
            log::info!(
                "  Detected: {} ring atoms, {} player atom",
                result.ring_elements.len(),
                if result.player_atom.is_some() { "1" } else { "0" }
            );

            let plausible = !summary.0.is_empty() && summary.1.is_some();
            if plausible {
                if prev_summary.as_ref() == Some(&summary) {
                    log::info!("  Stable: two consecutive captures agree");
                    detection_result = Some(result);
                    break;
                }
                prev_summary = Some(summary);
            } else {
                // Empty/playerless frames can't anchor agreement.
                prev_summary = None;
            }

            std::thread::sleep(std::time::Duration::from_millis(STABILITY_WAIT_MS));
        }

        let Some(detection_result) = detection_result else {
            self.stats.detection_failures += 1;
            anyhow::bail!(
                "No stable detection after {} captures - skipping move",
                STABILITY_ATTEMPTS
            );
        };

        // Step 3: Choose move using solver
        log::info!("[Move {}/{}] Choosing move...", move_number, self.max_moves);
        let decision = self
            .solver
            .choose_move(&detection_result)
            .context("Failed to choose move")?;

        // Step 4: Map decision to coordinates
        log::info!(
            "[Move {}/{}] Mapping to coordinates...",
            move_number,
            self.max_moves
        );
        let coordinates = map_decision_to_coordinates(&decision, &detection_result)
            .context("Failed to map decision to coordinates")?;

        log::info!("  Coordinate: ({}, {})", coordinates.x, coordinates.y);

        // Step 5: Execute tap (dry-run prints command)
        self.executor
            .execute_tap(coordinates, move_number)
            .context("Failed to execute tap")?;

        Ok(())
    }

    /// Print final summary
    fn print_summary(&self) {
        log::info!("==========================================================");
        log::info!("AUTOMATION LOOP SUMMARY");
        log::info!("==========================================================");
        log::info!("Total moves attempted: {}", self.stats.total_moves);
        log::info!("Successful: {}", self.stats.successful_moves);
        log::info!("Failed: {}", self.stats.failed_moves);
        log::info!("Detection failures: {}", self.stats.detection_failures);
        log::info!("Success rate: {:.1}%", self.stats.success_rate());
        log::info!("==========================================================\n");
    }

    /// Get current statistics
    pub fn stats(&self) -> LoopStats {
        self.stats.clone()
    }
}



