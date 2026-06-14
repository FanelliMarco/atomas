//! Milestone 2 Test: Automation Loop
//!
//! Phase 1: Dry-run mode (no emulator required)
//! Phase 2: ADB integration
//! Milestone 3 Stage 3: Solver integration

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

mod automation;
mod solver_integration;

use automation::executor::ExecutionMode;
use automation::{
    ActionExecutor, AutomationLoop, ExpectimaxSolver, ScreenshotSource, SimpleSolver,
    SimpleStrategy, SolverStrategy, adb,
};

/// Decision solver selection
enum SolverChoice {
    Simple(SimpleSolver),
    Expectimax(ExpectimaxSolver),
}

impl SolverChoice {
    fn choose_move(&mut self, detection: &atomas_cv::DetectionResult) -> Result<atomas_cv::Decision> {
        match self {
            Self::Simple(s) => s.choose_move(detection),
            Self::Expectimax(s) => s.choose_move(detection),
        }
    }
}

/// Milestone 2+3: Automation Loop for Atomas with Solver Integration
#[derive(Parser, Debug)]
#[command(name = "milestone2")]
#[command(about = "Automation loop: Capture → Detect → Decide → Execute → Repeat")]
#[command(version = "0.3.0")]
struct Args {
    /// Enable ADB mode (requires emulator/device)
    #[arg(long)]
    adb: bool,

    /// Run in dry-run mode (no ADB required) [default if --adb not specified]
    #[arg(long)]
    dry_run: bool,

    /// Path to screenshot for dry-run mode
    #[arg(long, default_value = "assets/jpg/board.jpg")]
    screenshot: PathBuf,

    /// Device serial (optional, uses first device if not specified)
    #[arg(long)]
    device: Option<String>,

    /// Delay in milliseconds between moves (ADB mode only)
    #[arg(long, default_value_t = 1000)]
    delay_ms: u64,

    /// Screenshot capture path for ADB mode
    #[arg(long, default_value = "assets/png/runtime/current_screen.png")]
    screenshot_path: PathBuf,

    /// Number of moves to execute
    #[arg(long, default_value_t = 10)]
    moves: usize,

    /// Solver type: simple (default), expectimax, expectimax-fast, expectimax-thorough
    #[arg(long, default_value = "simple")]
    solver: String,

    /// Simple solver strategy: random, prefer-insert, prefer-remove (only used with --solver simple)
    #[arg(long, default_value = "prefer-insert")]
    strategy: String,

    /// Expectimax solver depth (only used with --solver expectimax)
    #[arg(long)]
    solver_depth: Option<usize>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .format_timestamp(None)
        .init();

    // Validate arguments
    if args.moves == 0 {
        anyhow::bail!("Number of moves must be greater than 0");
    }

    // Determine mode (default to dry-run if neither specified)
    let use_adb = args.adb; // Explicit ADB mode only

    if !args.adb && !args.dry_run {
        log::info!("No mode specified, defaulting to --dry-run");
    }

    // Setup solver
    let solver_choice = setup_solver(&args)?;

    // Setup components based on mode
    let (screenshot_source, execution_mode) = if use_adb {
        // ADB Mode
        log::info!("=== ADB Mode ===");

        // Check ADB availability
        adb::check_adb_available().context(
            "ADB not available. Please install Android SDK Platform Tools and add to PATH.",
        )?;

        // Select device
        let device_serial =
            adb::select_device(args.device.as_deref()).context("Failed to select device")?;

        log::info!("Using device: {}", device_serial);
        log::info!("Screenshot path: {:?}", args.screenshot_path);
        log::info!("Delay between moves: {} ms", args.delay_ms);

        let source = ScreenshotSource::from_adb(device_serial.clone(), args.screenshot_path);
        let executor = ExecutionMode::Adb {
            device_serial,
            delay_ms: args.delay_ms,
        };

        (source, executor)
    } else {
        // Dry-run Mode
        log::info!("=== Dry-Run Mode ===");
        log::info!("Screenshot: {:?}", args.screenshot);

        let source = ScreenshotSource::from_file(&args.screenshot);
        let executor = ExecutionMode::DryRun;

        (source, executor)
    };

    // Validate source is available
    screenshot_source
        .is_available()
        .context("Screenshot source not available")?;

    let executor = ActionExecutor::new(execution_mode);

    // Create and run automation loop with appropriate solver
    let result = match solver_choice {
        SolverChoice::Simple(solver) => {
            let mut automation_loop =
                AutomationLoop::new(screenshot_source, solver, executor, args.moves)?;
            automation_loop.run()?;
            automation_loop.stats()
        }
        SolverChoice::Expectimax(solver) => {
            let mut automation_loop =
                AutomationLoop::new(screenshot_source, solver, executor, args.moves)?;
            automation_loop.run()?;
            automation_loop.stats()
        }
    };

    // Check results
    if result.failed_moves > 0 {
        log::warn!("Some moves failed. Check logs above for details.");
        log::warn!(
            "Success rate: {:.1}% ({}/{} successful)",
            result.success_rate(),
            result.successful_moves,
            result.total_moves
        );
    } else {
        log::info!("✓ All moves completed successfully!");
    }

    Ok(())
}

fn setup_solver(args: &Args) -> Result<SolverChoice> {
    let solver_type = args.solver.to_lowercase();

    if solver_type == "simple" {
        // Parse strategy for simple solver
        let strategy = SimpleStrategy::from_str(&args.strategy).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid strategy: '{}'. Valid options: random, prefer-insert, prefer-remove",
                args.strategy
            )
        })?;

        let solver = SimpleSolver::new(strategy);
        Ok(SolverChoice::Simple(solver))
    } else {
        // Parse expectimax solver type
        let strategy = SolverStrategy::from_str(&solver_type).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid solver: '{}'. Valid options: simple, expectimax, expectimax-fast, expectimax-thorough",
                args.solver
            )
        })?;

        let solver = if let Some(depth) = args.solver_depth {
            if depth == 0 || depth > 5 {
                anyhow::bail!("Solver depth must be between 1 and 5");
            }
            ExpectimaxSolver::with_depth(depth)
        } else {
            ExpectimaxSolver::new(strategy)
        };

        Ok(SolverChoice::Expectimax(solver))
    }
}
