//! Game state parser using atomas-cv

use atomas_core::{
    elements::Data,
    ring::{CircularList, AdjMatrix},
};
use atomas_cv::{
    detection::{DetectionConfig, GameStateDetector},
    Result,
};
use crate::gamestate::GameState;

/// Detect game state using the new CV system
pub fn detect_game_state<'a>(
    input_image_path: &str,
    data: &'a Data,
) -> Result<GameState<'a>> {
    // Create detection configuration
    let mut config = DetectionConfig::default();
    config.template_dirs = vec!["assets/png".into()];
    config.output_dir = "assets/png/outputs".into();

    // Low threshold to 0.5
    config.template_config.threshold = 0.5;
    
    // Create detector
    let detector = GameStateDetector::new(config)?;
    
    // Perform detection
    let detection_result = detector.detect_from_file(input_image_path, data)?;
    
    // Convert detection result to GameState
    let mut ring = CircularList::new();
    let mut player_atom = data.elements[0].clone(); // Default fallback
    
    // Add ring elements in order
    for (element, _bbox) in detection_result.ring_elements {
        ring.insert(element, ring.len());
    }
    
    // Set player atom if detected
    if let Some((element, _bbox)) = detection_result.player_atom {
        player_atom = element;
    }
    
    // Create game state
    let mut game_state = GameState {
        ring,
        player_atom,
        max_value: 1,
        score: 0,
        adj_matrix: AdjMatrix::new(12),
    };
    
    // Update adjacency matrix
    game_state.update_adjacency();
    
    println!("Detection completed:");
    println!("  - Ring elements: {}", detection_result.confidence_stats.ring_detections);
    println!("  - Player atom detected: {}", detection_result.confidence_stats.player_detections > 0);
    println!("  - Average confidence: {:.3}", detection_result.confidence_stats.avg_confidence);
    println!("  - Processing time: {}ms", detection_result.confidence_stats.processing_time_ms);
    
    Ok(game_state)
}
