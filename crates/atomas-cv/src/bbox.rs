//! Bounding box operations and non-maximum suppression
//! 
//! Core abstraction for representing and manipulating detection results.

use opencv::core::{Point, Rect, Scalar};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a bounding box detection with associated metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub confidence: f64,
    pub class_id: String,
    pub color: (u8, u8, u8),
    pub metadata: HashMap<String, String>,
}

impl BBox {
    /// Create a new bounding box
    pub fn new(x: i32, y: i32, width: i32, height: i32, confidence: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            confidence,
            class_id: String::new(),
            color: (255, 255, 255),
            metadata: HashMap::new(),
        }
    }

    /// Create from OpenCV Rect
    pub fn from_rect(rect: Rect, confidence: f64) -> Self {
        Self::new(rect.x, rect.y, rect.width, rect.height, confidence)
    }

    /// Convert to OpenCV Rect
    pub fn to_rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// Calculate area of the bounding box
    pub fn area(&self) -> f64 {
        (self.width * self.height) as f64
    }

    /// Calculate center point
    pub fn center(&self) -> Point {
        Point::new(
            self.x + self.width / 2,
            self.y + self.height / 2,
        )
    }

    /// Calculate intersection over union (IoU) with another box
    pub fn iou(&self, other: &BBox) -> f64 {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        if x2 <= x1 || y2 <= y1 {
            return 0.0;
        }

        let intersection = ((x2 - x1) * (y2 - y1)) as f64;
        let union = self.area() + other.area() - intersection;

        intersection / union
    }

    /// Check if this box overlaps with another
    pub fn overlaps(&self, other: &BBox, threshold: f64) -> bool {
        self.iou(other) > threshold
    }

    /// Set class information
    pub fn with_class(mut self, class_id: String, color: (u8, u8, u8)) -> Self {
        self.class_id = class_id;
        self.color = color;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Get OpenCV color scalar (BGR format)
    pub fn get_bgr_scalar(&self) -> Scalar {
        Scalar::new(
            self.color.2 as f64, // B
            self.color.1 as f64, // G
            self.color.0 as f64, // R
            255.0,
        )
    }
}

/// Collection of bounding boxes with batch operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BBoxCollection {
    boxes: Vec<BBox>,
}

impl BBoxCollection {
    /// Create new empty collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from vector of boxes
    pub fn from_vec(boxes: Vec<BBox>) -> Self {
        Self { boxes }
    }

    /// Add a box to the collection
    pub fn push(&mut self, bbox: BBox) {
        self.boxes.push(bbox);
    }

    /// Extend with another collection
    pub fn extend(&mut self, other: BBoxCollection) {
        self.boxes.extend(other.boxes);
    }

    /// Get boxes as slice
    pub fn as_slice(&self) -> &[BBox] {
        &self.boxes
    }

    /// Get mutable boxes
    pub fn as_mut_slice(&mut self) -> &mut [BBox] {
        &mut self.boxes
    }

    /// Get number of boxes
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// Sort by confidence (descending)
    pub fn sort_by_confidence(&mut self) {
        self.boxes.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    }

    /// Filter by confidence threshold
    pub fn filter_by_confidence(mut self, threshold: f64) -> Self {
        self.boxes.retain(|bbox| bbox.confidence >= threshold);
        self
    }

    /// Filter by class
    pub fn filter_by_class(mut self, class_id: &str) -> Self {
        self.boxes.retain(|bbox| bbox.class_id == class_id);
        self
    }

    /// Apply non-maximum suppression
    pub fn apply_nms(mut self, threshold: f64) -> Self {
        if self.boxes.is_empty() {
            return self;
        }

        // Sort by confidence
        self.sort_by_confidence();

        let mut keep = Vec::new();
        let mut suppressed = vec![false; self.boxes.len()];

        for i in 0..self.boxes.len() {
            if suppressed[i] {
                continue;
            }

            keep.push(self.boxes[i].clone());

            // Suppress overlapping boxes
            for j in (i + 1)..self.boxes.len() {
                if !suppressed[j] && self.boxes[i].overlaps(&self.boxes[j], threshold) {
                    suppressed[j] = true;
                }
            }
        }

        Self::from_vec(keep)
    }

    /// Apply class-aware NMS (NMS within each class)
    pub fn apply_class_nms(self, threshold: f64) -> Self {
        let mut class_groups: HashMap<String, Vec<BBox>> = HashMap::new();

        // Group by class
        for bbox in self.boxes {
            class_groups.entry(bbox.class_id.clone()).or_default().push(bbox);
        }

        // Apply NMS to each class separately
        let mut result = Vec::new();
        for (_, boxes) in class_groups {
            let collection = BBoxCollection::from_vec(boxes);
            result.extend(collection.apply_nms(threshold).boxes);
        }

        Self::from_vec(result)
    }

    /// Apply global NMS across all classes
    pub fn apply_global_nms(self, threshold: f64) -> Self {
        self.apply_nms(threshold)
    }

    /// Get boxes grouped by class
    pub fn group_by_class(&self) -> HashMap<String, Vec<&BBox>> {
        let mut groups: HashMap<String, Vec<&BBox>> = HashMap::new();
        
        for bbox in &self.boxes {
            groups.entry(bbox.class_id.clone()).or_default().push(bbox);
        }

        groups
    }

    /// Get statistics
    pub fn stats(&self) -> BBoxStats {
        let mut class_counts: HashMap<String, usize> = HashMap::new();
        let mut total_confidence = 0.0;
        let mut max_confidence: f64 = 0.0; // Fix: specify type explicitly
        let mut min_confidence = f64::INFINITY;

        for bbox in &self.boxes {
            *class_counts.entry(bbox.class_id.clone()).or_insert(0) += 1;
            total_confidence += bbox.confidence;
            max_confidence = max_confidence.max(bbox.confidence);
            min_confidence = min_confidence.min(bbox.confidence);
        }

        let avg_confidence = if self.boxes.is_empty() {
            0.0
        } else {
            total_confidence / self.boxes.len() as f64
        };

        BBoxStats {
            total_boxes: self.boxes.len(),
            class_counts,
            avg_confidence,
            max_confidence,
            min_confidence: if min_confidence == f64::INFINITY { 0.0 } else { min_confidence },
        }
    }

    /// Convert to iterator
    pub fn iter(&self) -> std::slice::Iter<BBox> {
        self.boxes.iter()
    }

    /// Convert to mutable iterator
    pub fn iter_mut(&mut self) -> std::slice::IterMut<BBox> {
        self.boxes.iter_mut()
    }
}

impl IntoIterator for BBoxCollection {
    type Item = BBox;
    type IntoIter = std::vec::IntoIter<BBox>;

    fn into_iter(self) -> Self::IntoIter {
        self.boxes.into_iter()
    }
}

impl FromIterator<BBox> for BBoxCollection {
    fn from_iter<T: IntoIterator<Item = BBox>>(iter: T) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

/// Statistics about a collection of bounding boxes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BBoxStats {
    pub total_boxes: usize,
    pub class_counts: HashMap<String, usize>,
    pub avg_confidence: f64,
    pub max_confidence: f64,
    pub min_confidence: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox_iou() {
        let box1 = BBox::new(0, 0, 10, 10, 0.9);
        let box2 = BBox::new(5, 5, 10, 10, 0.8);
        
        let iou = box1.iou(&box2);
        assert!(iou > 0.0 && iou < 1.0);
    }

    #[test]
    fn test_nms() {
        let mut collection = BBoxCollection::new();
        collection.push(BBox::new(0, 0, 10, 10, 0.9).with_class("A".to_string(), (255, 0, 0)));
        collection.push(BBox::new(2, 2, 10, 10, 0.8).with_class("A".to_string(), (255, 0, 0)));
        collection.push(BBox::new(20, 20, 10, 10, 0.7).with_class("B".to_string(), (0, 255, 0)));

        let result = collection.apply_nms(0.5);
        assert_eq!(result.len(), 2); // Should keep highest confidence from overlapping A's and the B
    }
}
