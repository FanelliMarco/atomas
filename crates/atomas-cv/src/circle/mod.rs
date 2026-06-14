pub mod detector;

pub use detector::{CircleDetector, DetectedCircle};

use opencv::core::Point;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Circle {
    pub center: (i32, i32),
    pub radius: i32,
    pub confidence: f64,
}

impl Circle {
    pub fn new(center: (i32, i32), radius: i32, confidence: f64) -> Self {
        Self {
            center,
            radius,
            confidence,
        }
    }

    pub fn center_point(&self) -> Point {
        Point::new(self.center.0, self.center.1)
    }

    pub fn distance_to(&self, other: &Circle) -> f64 {
        let dx = (self.center.0 - other.center.0) as f64;
        let dy = (self.center.1 - other.center.1) as f64;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn area(&self) -> f64 {
        std::f64::consts::PI * (self.radius as f64).powi(2)
    }
}

