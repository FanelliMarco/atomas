//! Non-maximum suppression utilities

use crate::bbox::BBoxCollection;

/// Non-maximum suppression utility functions
pub struct NonMaxSuppressionUtils;

impl NonMaxSuppressionUtils {
    /// Apply standard NMS to a collection of bounding boxes
    pub fn apply_nms(boxes: BBoxCollection, threshold: f64) -> BBoxCollection {
        boxes.apply_nms(threshold)
    }

    /// Apply class-aware NMS
    pub fn apply_class_nms(boxes: BBoxCollection, threshold: f64) -> BBoxCollection {
        boxes.apply_class_nms(threshold)
    }

    /// Apply global NMS across all classes
    pub fn apply_global_nms(boxes: BBoxCollection, threshold: f64) -> BBoxCollection {
        boxes.apply_global_nms(threshold)
    }
}
