use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};

use crate::elements::element::Element;
use crate::elements::id::Id;
use crate::elements::types::ElementType;

#[derive(Debug)]
pub struct Data<'a> {
    pub elements: Vec<Element<'a>>,
}

impl Data<'static> {
    pub fn load(path: &str) -> Data<'static> {
        Self::try_load(path).unwrap_or_else(|e| {
            eprintln!("Failed to load elements from '{}':", path);
            eprintln!("Error: {}", e);
            eprintln!("\nMake sure:");
            eprintln!("  1. The file exists at the specified path");
            eprintln!("  2. The file format is correct (Symbol\\-Name\\-R,G,B)");
            eprintln!("  3. You have read permissions");
            std::process::exit(1);
        })
    }

    fn try_load(path: &str) -> Result<Data<'static>> {
        let file = File::open(path).with_context(|| format!("Failed to open file: {}", path))?;
        let reader = BufReader::new(file);

        let mut elements = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line
                .with_context(|| format!("Failed to read line {} from {}", line_num + 1, path))?;

            if line.trim().is_empty() {
                continue; // Skip empty lines
            }

            let parts: Vec<&str> = line.split(r"\-").map(|s| s.trim()).collect();

            if parts.len() < 3 {
                eprintln!("Warning: Invalid line format at line {}: '{}' (expected 3 parts separated by \\-)", line_num + 1, line);
                continue;
            }

            let parts: Vec<String> = parts.iter().map(|s| s.to_string()).collect();

            let id = parts[0].clone();
            let name = parts[1].clone();
            let rgb_parts: Vec<&str> = parts[2].split(',').map(|s| s.trim()).collect();

            if rgb_parts.len() != 3 {
                eprintln!(
                    "Warning: Invalid color format at line {}: '{}' (expected R,G,B)",
                    line_num + 1,
                    parts[2]
                );
                continue;
            }

            let red = rgb_parts[0].parse::<u8>().with_context(|| {
                format!(
                    "Invalid red value at line {}: '{}'",
                    line_num + 1,
                    rgb_parts[0]
                )
            })?;
            let green = rgb_parts[1].parse::<u8>().with_context(|| {
                format!(
                    "Invalid green value at line {}: '{}'",
                    line_num + 1,
                    rgb_parts[1]
                )
            })?;
            let blue = rgb_parts[2].parse::<u8>().with_context(|| {
                format!(
                    "Invalid blue value at line {}: '{}'",
                    line_num + 1,
                    rgb_parts[2]
                )
            })?;

            let element = Element {
                id: Id::from_chars(id.chars().collect::<Vec<char>>().as_slice()),
                element_type: ElementType::Periodic(1), // Default to periodic for now
                name: Box::leak(name.into_boxed_str()),
                rgb: (red, green, blue),
            };
            elements.push(element);
        }

        println!(
            "Successfully loaded {} elements from {}",
            elements.len(),
            path
        );
        Ok(Data { elements })
    }
}
