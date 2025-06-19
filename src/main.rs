use atomas_core::elements::Data;
use std::path::Path;

mod gamestate;
mod parser;

fn main() {
    // Try different possible paths for the elements file
    let possible_paths = [
        "assets/txt/elements.txt",
        "./assets/txt/elements.txt",
        "../assets/txt/elements.txt",
        "crates/atomas-core/../../../assets/txt/elements.txt",
    ];
    
    let mut elements_path = None;
    for path in &possible_paths {
        if Path::new(path).exists() {
            elements_path = Some(*path);
            break;
        }
    }
    
    let path = match elements_path {
        Some(p) => {
            println!("Found elements file at: {}", p);
            p
        }
        None => {
            eprintln!("Elements file not found. Tried paths:");
            for path in &possible_paths {
                eprintln!("  - {}", path);
            }
            eprintln!("Please ensure the elements.txt file exists in the assets/txt/ directory.");
            return;
        }
    };
    
    let data = Data::load(path);

    // Try different possible paths for the board image
    let board_possible_paths = [
        "assets/jpg/board.jpg",
        "./assets/jpg/board.jpg", 
        "../assets/jpg/board.jpg",
    ];
    
    let mut board_path = None;
    for path in &board_possible_paths {
        if Path::new(path).exists() {
            board_path = Some(*path);
            break;
        }
    }
    
    let board_image_path = match board_path {
        Some(p) => {
            println!("Found board image at: {}", p);
            p
        }
        None => {
            eprintln!("Board image not found. Tried paths:");
            for path in &board_possible_paths {
                eprintln!("  - {}", path);
            }
            eprintln!("Please ensure a board.jpg file exists in the assets/jpg/ directory.");
            return;
        }
    };
    
    match parser::detect_game_state(board_image_path, &data) {
        Ok(game_state) => {
            println!("Detected Game State: {:?}", game_state);
        }
        Err(e) => {
            eprintln!("Detection failed: {}", e);
        }
    }
}
