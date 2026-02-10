//! Pointer heatmap utilities for Auto-Director inspection.

use grabme_project_model::event::InputEvent;

/// Grid configuration for heatmap generation.
#[derive(Debug, Clone, Copy)]
pub struct HeatmapConfig {
    pub cols: usize,
    pub rows: usize,
    /// Optional temporal weighting. 0.0 means equal weight.
    pub temporal_decay: f64,
}

impl Default for HeatmapConfig {
    fn default() -> Self {
        Self {
            cols: 32,
            rows: 18,
            temporal_decay: 0.0,
        }
    }
}

/// A normalized heatmap grid.
#[derive(Debug, Clone)]
pub struct HeatmapGrid {
    pub cols: usize,
    pub rows: usize,
    pub cells: Vec<f64>,
    pub max_density: f64,
}

impl HeatmapGrid {
    pub fn from_events(events: &[InputEvent], config: HeatmapConfig) -> Self {
        let cols = config.cols.max(1);
        let rows = config.rows.max(1);
        let mut cells = vec![0.0; cols * rows];

        if events.is_empty() {
            return Self {
                cols,
                rows,
                cells,
                max_density: 0.0,
            };
        }

        let first_t = events.first().map(|e| e.timestamp_ns).unwrap_or(0);
        let last_t = events.last().map(|e| e.timestamp_ns).unwrap_or(first_t);
        let span = (last_t.saturating_sub(first_t)).max(1) as f64;

        for event in events {
            let Some((x, y)) = event.pointer_position() else {
                continue;
            };

            let px = x.clamp(0.0, 0.999_999);
            let py = y.clamp(0.0, 0.999_999);
            let cx = (px * cols as f64).floor() as usize;
            let cy = (py * rows as f64).floor() as usize;

            let t = (event.timestamp_ns.saturating_sub(first_t) as f64) / span;
            let temporal_weight = if config.temporal_decay <= 0.0 {
                1.0
            } else {
                (config.temporal_decay * t).exp()
            };

            let idx = cy * cols + cx;
            cells[idx] += temporal_weight;
        }

        let max_density = cells.iter().copied().fold(0.0_f64, f64::max);
        Self {
            cols,
            rows,
            cells,
            max_density,
        }
    }

    pub fn cell(&self, col: usize, row: usize) -> Option<f64> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        Some(self.cells[row * self.cols + col])
    }

    pub fn normalized_cell(&self, col: usize, row: usize) -> Option<f64> {
        let value = self.cell(col, row)?;
        if self.max_density <= 0.0 {
            return Some(0.0);
        }
        Some(value / self.max_density)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heatmap_tracks_hotspot() {
        let events = vec![
            InputEvent::pointer(0, 0.1, 0.1),
            InputEvent::pointer(16_000_000, 0.1, 0.1),
            InputEvent::pointer(32_000_000, 0.12, 0.1),
            InputEvent::pointer(48_000_000, 0.9, 0.9),
        ];

        let heatmap = HeatmapGrid::from_events(
            &events,
            HeatmapConfig {
                cols: 10,
                rows: 10,
                temporal_decay: 0.0,
            },
        );

        let top_left = heatmap.cell(1, 1).unwrap();
        let bottom_right = heatmap.cell(9, 9).unwrap();
        assert!(top_left > bottom_right);
        assert!(heatmap.max_density >= top_left);
    }

    #[test]
    fn heatmap_empty_events_is_zeroed() {
        let heatmap = HeatmapGrid::from_events(&[], HeatmapConfig::default());
        assert_eq!(heatmap.max_density, 0.0);
        assert!(heatmap
            .cells
            .iter()
            .all(|v| (*v - 0.0).abs() < f64::EPSILON));
    }
}
