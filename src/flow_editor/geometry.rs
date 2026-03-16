use iced::{Point, Rectangle, Transformation, Vector};

#[derive(Debug, Clone, Copy)]
pub struct Viewport2D {
    pub pan: Vector,
    pub zoom: f32,
}

impl Default for Viewport2D {
    fn default() -> Self {
        Self {
            pan: Vector::new(0.0, 0.0),
            zoom: 1.0,
        }
    }
}

impl Viewport2D {
    pub fn transformation(&self, bounds: Rectangle) -> Transformation {
        Transformation::translate(bounds.x + self.pan.x, bounds.y + self.pan.y)
            * Transformation::scale(self.zoom)
            * Transformation::translate(-bounds.x, -bounds.y)
    }

    pub fn scene_to_screen(&self, point: Point, bounds: Rectangle) -> Point {
        Point::new(
            bounds.x + self.pan.x + point.x * self.zoom,
            bounds.y + self.pan.y + point.y * self.zoom,
        )
    }

    pub fn screen_to_scene(&self, point: Point, bounds: Rectangle) -> Point {
        Point::new(
            (point.x - bounds.x - self.pan.x) / self.zoom,
            (point.y - bounds.y - self.pan.y) / self.zoom,
        )
    }

    pub fn scene_rect_to_screen(&self, rect: Rectangle, bounds: Rectangle) -> Rectangle {
        let p = self.scene_to_screen(Point::new(rect.x, rect.y), bounds);
        Rectangle {
            x: p.x,
            y: p.y,
            width: rect.width * self.zoom,
            height: rect.height * self.zoom,
        }
    }
}
