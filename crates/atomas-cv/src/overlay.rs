//! Milestone 1: Annotated Overlay Visualization
//!
//! This module draws visual overlays on game screenshots to show where
//! decisions will be executed (yellow ring for INSERT, red X for REMOVE).

use crate::action::{ActionCoordinates, Decision};
use crate::utils::ImageUtils;
use anyhow::Context;
use opencv::{
    core::{self, Mat, Point, Scalar},
    imgproc::{self, FONT_HERSHEY_SIMPLEX, LINE_AA},
};
use std::path::Path;

/// Colors for overlay markers (BGR format for OpenCV)
const COLOR_INSERT: Scalar = Scalar::new(0.0, 255.0, 255.0, 255.0); // Yellow
const COLOR_REMOVE: Scalar = Scalar::new(0.0, 0.0, 255.0, 255.0);   // Red
const COLOR_PLUS: Scalar = Scalar::new(0.0, 255.0, 0.0, 255.0);     // Green
const COLOR_MINUS: Scalar = Scalar::new(0.0, 165.0, 255.0, 255.0);  // Orange
const COLOR_TEXT_BG: Scalar = Scalar::new(0.0, 0.0, 0.0, 255.0);    // Black

/// Draw an action overlay on a screenshot
///
/// # Arguments
/// * `image_path` - Path to the input screenshot
/// * `decision` - The decision to visualize
/// * `coordinates` - The screen coordinates where the action will be performed
/// * `output_path` - Path to save the annotated image
///
/// # Returns
/// * `Ok(())` if the overlay was successfully drawn and saved
pub fn draw_action_overlay<P: AsRef<Path>>(
    image_path: P,
    decision: &Decision,
    coordinates: ActionCoordinates,
    output_path: P,
) -> anyhow::Result<()> {
    // Load the image
    let mut image = ImageUtils::load_color(&image_path)
        .with_context(|| format!("Failed to load image: {:?}", image_path.as_ref()))?;

    let position = Point::new(coordinates.x, coordinates.y);

    match decision {
        Decision::Insert { gap_index } => {
            draw_insert_marker(&mut image, position, *gap_index)?;
        }
        Decision::Remove { atom_index } => {
            draw_remove_marker(&mut image, position, *atom_index)?;
        }
        Decision::ConvertToPlus => {
            draw_plus_marker(&mut image, position)?;
        }
        Decision::MinusTake { target_index } => {
            draw_minus_marker(&mut image, position, *target_index)?;
        }
        Decision::NeutrinoCopy { target_index } => {
            draw_neutrino_marker(&mut image, position, *target_index)?;
        }
    }

    // Save the annotated image
    ImageUtils::save_image(&image, &output_path)
        .with_context(|| format!("Failed to save overlay: {:?}", output_path.as_ref()))?;

    println!(
        "âœ“ Overlay saved: {:?} at ({}, {})",
        output_path.as_ref(),
        coordinates.x,
        coordinates.y
    );

    Ok(())
}

/// Draw a yellow circle marker for INSERT decisions
fn draw_insert_marker(image: &mut Mat, position: Point, gap_index: usize) -> anyhow::Result<()> {
    let radius = 30;
    let thickness = 4;

    // Draw yellow circle
    imgproc::circle(image, position, radius, COLOR_INSERT, thickness, LINE_AA, 0)?;

    // Draw label above the circle
    let label = "INSERT HERE";
    draw_label(image, position, label, COLOR_INSERT, -radius - 10)?;

    println!(
        "INSERT marker: gap_index={} at ({}, {}) - Yellow circle drawn",
        gap_index, position.x, position.y
    );

    Ok(())
}

/// Draw a red X marker for REMOVE decisions
fn draw_remove_marker(image: &mut Mat, position: Point, atom_index: usize) -> anyhow::Result<()> {
    let size = 25;
    let thickness = 4;

    // Draw red X (two diagonal lines)
    imgproc::line(
        image,
        Point::new(position.x - size, position.y - size),
        Point::new(position.x + size, position.y + size),
        COLOR_REMOVE,
        thickness,
        LINE_AA,
        0,
    )?;

    imgproc::line(
        image,
        Point::new(position.x + size, position.y - size),
        Point::new(position.x - size, position.y + size),
        COLOR_REMOVE,
        thickness,
        LINE_AA,
        0,
    )?;

    // Draw label above the X
    let label = "REMOVE";
    draw_label(image, position, label, COLOR_REMOVE, -size - 15)?;

    println!(
        "REMOVE marker: atom_index={} at ({}, {}) - Red X drawn",
        atom_index, position.x, position.y
    );

    Ok(())
}

/// Draw a green double-circle marker for ConvertToPlus decisions
///
/// Converting taps the held centre atom, so we mark the centre with a
/// distinctive double-ring + "+" cross.
fn draw_plus_marker(image: &mut Mat, position: Point) -> anyhow::Result<()> {
    let thickness = 4;

    // Outer circle
    imgproc::circle(image, position, 34, COLOR_PLUS, thickness, LINE_AA, 0)?;
    // Inner circle
    imgproc::circle(image, position, 18, COLOR_PLUS, thickness, LINE_AA, 0)?;

    // Draw a small "+" cross in the center
    let arm = 10;
    imgproc::line(
        image,
        Point::new(position.x, position.y - arm),
        Point::new(position.x, position.y + arm),
        COLOR_PLUS,
        thickness,
        LINE_AA,
        0,
    )?;
    imgproc::line(
        image,
        Point::new(position.x - arm, position.y),
        Point::new(position.x + arm, position.y),
        COLOR_PLUS,
        thickness,
        LINE_AA,
        0,
    )?;

    let label = "CONVERT TO PLUS";
    draw_label(image, position, label, COLOR_PLUS, -34 - 10)?;

    println!(
        "CONVERT TO PLUS marker at ({}, {}) - Green double-circle drawn",
        position.x, position.y
    );

    Ok(())
}

/// Draw an orange crossed-circle marker for MinusTake decisions
///
/// Minus pulls ONE atom from the ring into the hand; we mark the tap target
/// with an orange circle + "-" bar so it's visually distinct from REMOVE.
fn draw_minus_marker(image: &mut Mat, position: Point, target_index: usize) -> anyhow::Result<()> {
    let radius = 30;
    let thickness = 4;

    // Draw orange circle around the target atom
    imgproc::circle(image, position, radius, COLOR_MINUS, thickness, LINE_AA, 0)?;

    // Draw a "-" bar through the center to signal "minus"
    let arm = 14;
    imgproc::line(
        image,
        Point::new(position.x - arm, position.y),
        Point::new(position.x + arm, position.y),
        COLOR_MINUS,
        thickness,
        LINE_AA,
        0,
    )?;

    let label = "MINUS TAKE";
    draw_label(image, position, label, COLOR_MINUS, -radius - 10)?;

    println!(
        "MINUS TAKE marker: target_index={} at ({}, {}) - Orange circle drawn",
        target_index, position.x, position.y
    );

    Ok(())
}

/// Draw a cyan circle marker for NeutrinoCopy decisions
fn draw_neutrino_marker(
    image: &mut Mat,
    position: Point,
    target_index: usize,
) -> anyhow::Result<()> {
    let radius = 30;
    let thickness = 4;
    let color = Scalar::new(255.0, 255.0, 0.0, 255.0); // Cyan (BGR)

    imgproc::circle(image, position, radius, color, thickness, LINE_AA, 0)?;
    imgproc::circle(image, position, 12, color, thickness, LINE_AA, 0)?;

    let label = "NEUTRINO COPY";
    draw_label(image, position, label, color, -radius - 10)?;

    println!(
        "NEUTRINO COPY marker: target_index={} at ({}, {}) - Cyan circle drawn",
        target_index, position.x, position.y
    );

    Ok(())
}

/// Draw a text label with background
fn draw_label(
    image: &mut Mat,
    position: Point,
    text: &str,
    color: Scalar,
    y_offset: i32,
) -> anyhow::Result<()> {
    let font_face = FONT_HERSHEY_SIMPLEX;
    let font_scale = 0.6;
    let thickness = 2;
    let mut baseline = 0;

    // Get text size
    let text_size = imgproc::get_text_size(text, font_face, font_scale, thickness, &mut baseline)?;

    // Calculate text position (centered horizontally)
    let text_x = position.x - text_size.width / 2;
    let text_y = position.y + y_offset;

    // Draw black background rectangle for text
    let padding = 5;
    let rect = core::Rect::new(
        text_x - padding,
        text_y - text_size.height - padding,
        text_size.width + 2 * padding,
        text_size.height + baseline + 2 * padding,
    );
    imgproc::rectangle(
        image,
        rect,
        COLOR_TEXT_BG,
        -1, // Filled
        LINE_AA,
        0,
    )?;

    // Draw text
    imgproc::put_text(
        image,
        text,
        Point::new(text_x, text_y),
        font_face,
        font_scale,
        color,
        thickness,
        LINE_AA,
        false,
    )?;

    Ok(())
}

/// Convenience function: Draw overlay directly from a detection result
///
/// This is a higher-level API that combines detection + decision + overlay
pub fn draw_decision_on_detection<P: AsRef<Path>>(
    image_path: P,
    detection_result: &crate::detection::DetectionResult,
    decision: &Decision,
    output_path: P,
) -> anyhow::Result<()> {
    // Map decision to coordinates
    let coordinates = crate::action::map_decision_to_coordinates(decision, detection_result)?;

    // Draw overlay
    draw_action_overlay(image_path, decision, coordinates, output_path)?;

    Ok(())
}



