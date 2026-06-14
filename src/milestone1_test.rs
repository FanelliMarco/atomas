//! Milestone 1 Test: Decision to Screen Action + Annotated Overlay
//!
//! This program demonstrates:
//! 1. Loading a screenshot and detecting the game state
//! 2. Creating INSERT and REMOVE decisions
//! 3. Mapping decisions to screen coordinates
//! 4. Drawing annotated overlays (yellow ring / red X)

use atomas_core::elements::Data;
use atomas_cv::{
    action::{Decision, map_decision_to_coordinates},
    detection::{DetectionConfig, GameStateDetector},
    overlay::draw_decision_on_detection,
};

fn main() -> anyhow::Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("MILESTONE 1: Decision → Screen Action Mapping");
    println!("{}\n", "=".repeat(60));

    // Load element data
    let elements_path = format!("{}/assets/txt/elements.txt", env!("CARGO_MANIFEST_DIR"));
    let data = Data::load(&elements_path);

    // Input screenshot
    let board_path = format!("{}/assets/jpg/board.jpg", env!("CARGO_MANIFEST_DIR"));
    println!("Input screenshot: {}", board_path);

    // Detect game state
    println!("\n--- Step 1: Detecting game state ---");
    let mut config = DetectionConfig::accurate_color_matching();
    config.output_dir = format!("{}/assets/png/outputs", env!("CARGO_MANIFEST_DIR")).into();

    let detector = GameStateDetector::new(config)?;
    let detection_result = detector.detect_from_file(&board_path, &data)?;

    println!(
        "Detected: {} ring elements, {} total circles",
        detection_result.ring_elements.len(),
        detection_result.confidence_stats.total_detections
    );

    if detection_result.ring_elements.is_empty() {
        anyhow::bail!("No ring elements detected - cannot proceed with Milestone 1");
    }

    // Test Case 1: INSERT decision
    println!("\n--- Step 2: Testing INSERT decision ---");
    let insert_decision = Decision::Insert { gap_index: 3 };
    let insert_coords = map_decision_to_coordinates(&insert_decision, &detection_result)?;
    println!(
        "INSERT at gap_index=3 → coordinates: ({}, {})",
        insert_coords.x, insert_coords.y
    );

    let insert_output = format!(
        "{}/assets/png/outputs/action_overlay_insert.png",
        env!("CARGO_MANIFEST_DIR")
    );
    draw_decision_on_detection(&board_path, &detection_result, &insert_decision, &insert_output)?;
    println!("✓ INSERT overlay saved: {}", insert_output);

    // Test Case 2: REMOVE decision
    println!("\n--- Step 3: Testing REMOVE decision ---");
    let remove_decision = Decision::Remove { atom_index: 5 };
    let remove_coords = map_decision_to_coordinates(&remove_decision, &detection_result)?;
    println!(
        "REMOVE at atom_index=5 → coordinates: ({}, {})",
        remove_coords.x, remove_coords.y
    );

    let remove_output = format!(
        "{}/assets/png/outputs/action_overlay_remove.png",
        env!("CARGO_MANIFEST_DIR")
    );
    draw_decision_on_detection(&board_path, &detection_result, &remove_decision, &remove_output)?;
    println!("✓ REMOVE overlay saved: {}", remove_output);

    // Test Case 3: INSERT at different gap (wrap-around test)
    println!("\n--- Step 4: Testing INSERT at gap 8 (wrap-around) ---");
    let insert_decision2 = Decision::Insert { gap_index: 8 };
    let insert_coords2 = map_decision_to_coordinates(&insert_decision2, &detection_result)?;
    println!(
        "INSERT at gap_index=8 → coordinates: ({}, {})",
        insert_coords2.x, insert_coords2.y
    );

    let insert_output2 = format!(
        "{}/assets/png/outputs/action_overlay_insert_gap8.png",
        env!("CARGO_MANIFEST_DIR")
    );
    draw_decision_on_detection(&board_path, &detection_result, &insert_decision2, &insert_output2)?;
    println!("✓ INSERT overlay saved: {}", insert_output2);

    // Summary
    println!("\n{}", "=".repeat(60));
    println!("✓ Milestone 1 Complete!");
    println!("{}", "=".repeat(60));
    println!("\nGenerated overlays:");
    println!("  1. INSERT (yellow):  {}", insert_output);
    println!("  2. REMOVE (red X):   {}", remove_output);
    println!("  3. INSERT (gap 8):   {}", insert_output2);
    println!("\nCoordinates used:");
    println!("  INSERT gap 3:   ({}, {})", insert_coords.x, insert_coords.y);
    println!("  REMOVE atom 5:  ({}, {})", remove_coords.x, remove_coords.y);
    println!("  INSERT gap 8:   ({}, {})", insert_coords2.x, insert_coords2.y);

    Ok(())
}
