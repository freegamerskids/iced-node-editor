use iced::advanced::graphics::mesh::{Indexed, SolidVertex2D, Renderer};
use iced::{Point, Rectangle, Size, Transformation};

pub trait MeshRenderer {
    fn draw_buffers(&mut self, buffers: Indexed<SolidVertex2D>);
}

impl MeshRenderer for iced::Renderer {
    fn draw_buffers(&mut self, buffers: Indexed<SolidVertex2D>) {
        let min = buffers
            .vertices
            .iter()
            .fold(Point::new(f32::MAX, f32::MAX), |min, v| {
                Point::new(min.x.min(v.position[0]), min.y.min(v.position[1]))
            });

        let max = buffers
            .vertices
            .iter()
            .fold(Point::new(f32::MIN, f32::MIN), |max, v| {
                Point::new(max.x.max(v.position[0]), max.y.max(v.position[1]))
            });

        let size = Size::new(max.x - min.x, max.y - min.y);
        let transformation = Transformation::IDENTITY;
        let clip_bounds = Rectangle::new(Point {x: max.x - min.y, y: max.y - min.y }, size);

        if size.width >= 1.0 && size.height >= 1.0 {
            self.draw_mesh(iced::advanced::graphics::Mesh::Solid { buffers, transformation, clip_bounds });
        }
    }
}
