use atomas_core::elements::Data;
use std::path::Path;

mod gamestate;
mod parser;

fn main() {
    let elements_path = format!("{}/assets/txt/elements.txt", env!("CARGO_MANIFEST_DIR"));

    let data = Data::load(&elements_path);

    let board_path = format!("{}/assets/jpg/board.jpg", env!("CARGO_MANIFEST_DIR"));

    match parser::detect_game_state(&board_path, &data) {
        Ok(game_state) => {
            println!("Detected Game State: {:?}", game_state);
        }
        Err(e) => {
            eprintln!("Detection failed: {}", e);
        }
    }
}
