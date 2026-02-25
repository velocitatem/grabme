//! Synthetic test pattern generation

use image::{ImageBuffer, Rgb, RgbImage};
use imageproc::drawing::draw_filled_circle_mut;

/// Create tracking pattern with colored markers at known positions
pub fn create_tracking_pattern(width: u32, height: u32) -> RgbImage {
    let mut img = ImageBuffer::from_pixel(width, height, Rgb([40, 40, 50]));

    // Draw corner markers (red circles)
    let marker_radius = 20;
    let markers = [
        (marker_radius + 10, marker_radius + 10),
        (width - marker_radius - 10, marker_radius + 10),
        (width - marker_radius - 10, height - marker_radius - 10),
        (marker_radius + 10, height - marker_radius - 10),
    ];

    for (x, y) in &markers {
        draw_filled_circle_mut(
            &mut img,
            (*x as i32, *y as i32),
            marker_radius as i32,
            Rgb([255, 50, 50]),
        );
    }

    // Center marker (green)
    draw_filled_circle_mut(
        &mut img,
        ((width / 2) as i32, (height / 2) as i32),
        30,
        Rgb([50, 255, 50]),
    );

    // Edge markers (blue)
    let mid_x = width / 2;
    let mid_y = height / 2;
    let edge_markers = [
        (mid_x, marker_radius + 10),
        (width - marker_radius - 10, mid_y),
        (mid_x, height - marker_radius - 10),
        (marker_radius + 10, mid_y),
    ];

    for (x, y) in &edge_markers {
        draw_filled_circle_mut(&mut img, (*x as i32, *y as i32), 15i32, Rgb([50, 50, 255]));
    }

    img
}

/// Create grid pattern with coordinates for spatial verification
pub fn create_grid_pattern(width: u32, height: u32) -> RgbImage {
    let mut img = ImageBuffer::from_pixel(width, height, Rgb([30, 30, 35]));

    let grid_spacing = 100;
    let line_color = Rgb([80, 80, 90]);

    // Vertical lines
    for x in (0..width).step_by(grid_spacing as usize) {
        for y in 0..height {
            img.put_pixel(x, y, line_color);
        }
    }

    // Horizontal lines
    for y in (0..height).step_by(grid_spacing as usize) {
        for x in 0..width {
            img.put_pixel(x, y, line_color);
        }
    }

    // Major axes (brighter)
    let axis_color = Rgb([150, 150, 160]);
    let mid_x = width / 2;
    let mid_y = height / 2;

    for y in 0..height {
        img.put_pixel(mid_x, y, axis_color);
    }
    for x in 0..width {
        img.put_pixel(x, mid_y, axis_color);
    }

    // Quadrant markers
    let quadrants = [
        (width / 4, height / 4),
        (3 * width / 4, height / 4),
        (3 * width / 4, 3 * height / 4),
        (width / 4, 3 * height / 4),
    ];

    for (x, y) in &quadrants {
        draw_filled_circle_mut(&mut img, (*x as i32, *y as i32), 8i32, Rgb([200, 150, 50]));
    }

    img
}

/// Create high-frequency pattern for quality verification
pub fn create_quality_pattern(width: u32, height: u32) -> RgbImage {
    let mut img = ImageBuffer::from_pixel(width, height, Rgb([50, 50, 50]));

    // Checkerboard pattern in top-left
    let checker_size = 20u32;
    for y in (0..height / 3).step_by(checker_size as usize) {
        for x in (0..width / 3).step_by(checker_size as usize) {
            let color = if (x / checker_size + y / checker_size) % 2 == 0 {
                Rgb([200, 200, 200])
            } else {
                Rgb([50, 50, 50])
            };

            for dy in 0..checker_size {
                for dx in 0..checker_size {
                    if x + dx < width && y + dy < height {
                        img.put_pixel(x + dx, y + dy, color);
                    }
                }
            }
        }
    }

    // Gradient in top-right
    for y in 0..height / 3 {
        for x in 2 * width / 3..width {
            let intensity = ((x - 2 * width / 3) * 255 / (width / 3)) as u8;
            img.put_pixel(x, y, Rgb([intensity, intensity, intensity]));
        }
    }

    // Color bars in bottom half
    let bar_width = width / 6;
    let colors = [
        Rgb([255, 0, 0]),   // Red
        Rgb([0, 255, 0]),   // Green
        Rgb([0, 0, 255]),   // Blue
        Rgb([255, 255, 0]), // Yellow
        Rgb([255, 0, 255]), // Magenta
        Rgb([0, 255, 255]), // Cyan
    ];

    for (i, color) in colors.iter().enumerate() {
        let x_start = (i as u32) * bar_width;
        let x_end = x_start + bar_width;
        for y in height / 2..height {
            for x in x_start..x_end.min(width) {
                img.put_pixel(x, y, *color);
            }
        }
    }

    img
}

/// Get test points for cursor movement (in pixel coordinates)
pub fn get_test_points(width: u32, height: u32) -> Vec<(u32, u32)> {
    vec![
        // Corners
        (50, 50),
        (width - 50, 50),
        (width - 50, height - 50),
        (50, height - 50),
        // Center
        (width / 2, height / 2),
        // Edges
        (width / 2, 50),
        (width - 50, height / 2),
        (width / 2, height - 50),
        (50, height / 2),
        // Quadrants
        (width / 4, height / 4),
        (3 * width / 4, height / 4),
        (3 * width / 4, 3 * height / 4),
        (width / 4, 3 * height / 4),
        // Return to center
        (width / 2, height / 2),
    ]
}
