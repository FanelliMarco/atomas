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

/// Detect game state using gradient-based matching (robust to lighting)
pub fn detect_game_state<'a>(
    input_image_path: &str,
    data: &'a Data,
) -> Result<GameState<'a>> {
    // Use configuration optimized for uneven lighting
    let mut config = DetectionConfig::for_uneven_lighting();
    config.template_dirs = vec!["assets/png".into()];
    config.output_dir = "assets/png/outputs".into();
    
    // Try different configurations if first fails
    let configs_to_try = vec![
        ("Laplacian + CCoeff", config.clone()),
        ("Laplacian + SqDiff", DetectionConfig::with_sqdiff_laplacian()),
        ("Edge-based", DetectionConfig::with_edge_matching()),
    ];
    
    for (name, mut cfg) in configs_to_try {
        println!("Trying configuration: {}", name);
        cfg.template_dirs = vec!["assets/png".into()];
        cfg.output_dir = "assets/png/outputs".into();
        
        let detector = GameStateDetector::new(cfg)?;
        let detection_result = detector.detect_from_file(input_image_path, data)?;
        
        // Check if we got reasonable results
        if detection_result.ring_elements.len() >= 3 {
            return build_game_state(detection_result, data);
        }
        
        println!("  -> Only {} detections, trying next method", 
                 detection_result.ring_elements.len());
    }
    
    // Fallback: use last detection even if not ideal
    println!("Using best available detection");
    let detector = GameStateDetector::new(config)?;
    let detection_result = detector.detect_from_file(input_image_path, data)?;
    
    build_game_state(detection_result, data)
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
    
    println!("Detection completed:");
    println!("  - Ring elements: {}", detection_result.confidence_stats.ring_detections);
    println!("  - Player atom: {}", detection_result.confidence_stats.player_detections > 0);
    println!("  - Avg confidence: {:.3}", detection_result.confidence_stats.avg_confidence);
    println!("  - Time: {}ms", detection_result.confidence_stats.processing_time_ms);
    
    Ok(game_state)
}
