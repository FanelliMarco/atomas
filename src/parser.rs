use atomas_core::{elements::Data, ring::{AdjMatrix, CircularList}};
use atomas_cv::{detection::{DetectionConfig, GameStateDetector}, Result};

use crate::gamestate::GameState;

pub fn detect_game_state<'a>(
    input_image_path: &str,
    data: &'a Data,
) -> Result<GameState<'a>> {
    let configs: Vec<(&str, DetectionConfig)> = vec![
        ("Accurate Color Matching (Normalized + HSV)", DetectionConfig::accurate_color_matching()),
        ("High Sensitivity",                          DetectionConfig::high_sensitivity()),
        ("High Contrast",                             DetectionConfig::high_contrast()),
        ("Default",                                   DetectionConfig::default()),
        ("Low Sensitivity",                           DetectionConfig::low_sensitivity()),
    ];

    for (name, mut cfg) in configs {
        println!("\n=== Trying configuration: {} ===", name);
        cfg.output_dir = "assets/png/outputs".into();

        let detector = GameStateDetector::new(cfg)?;
        let result = detector.detect_from_file(input_image_path, data)?;

        println!(
            "Detected: {} circles, {} ring elements",
            result.confidence_stats.total_detections, result.confidence_stats.ring_detections
        );

        if result.ring_elements.len() >= 6 && result.confidence_stats.total_detections >= 7 {
            println!("\n✓ Configuration '{}' succeeded!", name);
            return build_game_state(result, data);
        }

        println!("  -> Need >= 7 circles and >= 6 ring elements");
    }

    println!("\nFallback: using Accurate Color Matching");
    let mut cfg = DetectionConfig::accurate_color_matching();
    cfg.output_dir = "assets/png/outputs".into();
    let detector = GameStateDetector::new(cfg)?;
    let result = detector.detect_from_file(input_image_path, data)?;
    build_game_state(result, data)
}

fn build_game_state<'a>(
    detection_result: atomas_cv::detection::DetectionResult<'a>,
    data: &'a Data,
) -> Result<GameState<'a>> {
    let mut ring = CircularList::new();
    let mut player_atom = data.elements[0].clone();

    for (element, _bbox) in detection_result.ring_elements {
        ring.insert(element, ring.len());
    }

    if let Some((element, _bbox)) = detection_result.player_atom {
        player_atom = element;
    }

    let mut game_state = GameState {
        ring,
        player_atom,
        max_value: 1,
        score: 0,
        adj_matrix: AdjMatrix::new(12),
    };

    game_state.update_adjacency();

    println!("\n=== Detection Summary ===");
    println!("  Total circles:     {}", detection_result.confidence_stats.total_detections);
    println!("  Ring elements:     {}", detection_result.confidence_stats.ring_detections);
    println!("  Player atom found: {}", detection_result.confidence_stats.player_detections > 0);
    println!("  Processing time:   {}ms", detection_result.confidence_stats.processing_time_ms);
    println!("==========================\n");

    Ok(game_state)
}

