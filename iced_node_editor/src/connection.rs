use std::sync::Mutex;

use iced::advanced::graphics::mesh::{Indexed, SolidVertex2D};
use iced::advanced::renderer;
use iced::advanced::widget::Tree;
use iced::{advanced::Widget, Length, Point, Size, Vector};

use crate::{
    mesh_renderer::MeshRenderer,
    node_element::{GraphNodeElement, ScalableWidget},
    styles::connection::StyleSheet,
    SocketRole,
};

pub struct Connection<Message, Theme>
where
    Theme: StyleSheet,
{
    link: Link,
    width: f32,
    number_of_segments: usize,
    style: Theme::Style,

    phantom_message: std::marker::PhantomData<Message>,
    spline: Mutex<Vec<Vector>>,
}

impl<Message, Theme> Connection<Message, Theme>
where
    Theme: StyleSheet,
{
    pub fn new(link: Link) -> Self {
        Connection {
            spline: Mutex::new(Vec::new()),
            link,
            width: 1.2,
            number_of_segments: 20,
            phantom_message: std::marker::PhantomData,
            style: Default::default(),
        }
    }

    pub fn between(first: Endpoint, second: Endpoint) -> Self {
        Self::new(Link::from_unordered(first, second))
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn number_of_segments(mut self, number_of_segments: usize) -> Self {
        self.number_of_segments = number_of_segments;
        self
    }

    pub fn style(mut self, style: Theme::Style) -> Self {
        self.style = style;
        self
    }
}

pub fn connection<Message, Theme>(from: Point, to: Point) -> Connection<Message, Theme>
where
    Theme: StyleSheet,
{
    Connection::between(Endpoint::Absolute(from), Endpoint::Absolute(to))
}

impl<Message, Theme, Renderer> ScalableWidget<Message, Renderer> for Connection<Message, Theme>
where
    Theme: StyleSheet,
    Renderer: renderer::Renderer,
{
    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &iced::advanced::layout::Limits,
        scale: f32,
        socket_state: &mut super::node_element::SocketLayoutState,
    ) -> iced::advanced::layout::Node {
        // Set the flag that we've started to process connections;
        // this will cause a panic if there are further nodes
        socket_state.done = true;

        let spline = generate_spline(
            self.link.start.resolve(scale, socket_state),
            1.0,
            self.link.end.resolve(scale, socket_state),
            self.number_of_segments,
            1.0_f32,
        );

        let spline_bounds = bounds_for_vectors(&spline);

        let spline = spline
            .iter()
            .map(|p| Vector::new(p.x - spline_bounds.x, p.y - spline_bounds.y))
            .collect();

        let node = iced::advanced::layout::Node::new(Size::new(
            (spline_bounds.width + self.width).ceil(),
            (spline_bounds.height + self.width).ceil(),
        ));

        let mut self_state = self.spline.lock().expect("Could not lock mutex");
        *self_state = spline;

        node.translate(Vector::new(spline_bounds.x, spline_bounds.y))
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Connection<Message, Theme>
where
    Theme: StyleSheet,
    Renderer: renderer::Renderer + MeshRenderer,
{
    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        todo!("This should never be called.")
    }

    fn draw(
        &self,
        _tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _renderer_style: &renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::mouse::Cursor,
        _viewport: &iced::Rectangle,
    ) {
        let bounds = layout.bounds();
        let style = theme.appearance(&self.style);

        let spline = self.spline.lock().unwrap();
        let (vertices, indices) = line_to_polygon(&spline, self.width / 2.0);

        let buffers = Indexed {
            vertices: vertices
                .iter()
                .map(|p| SolidVertex2D {
                    position: [p.x, p.y],
                    color: iced::advanced::graphics::color::pack(style.color.unwrap()),
                })
                .collect(),
            indices,
        };

        renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.draw_buffers(buffers);
        });
    }

    fn size(&self) -> Size<Length> {
        todo!("This should never be called.");
    }
}

impl<'a, Message, Theme, Renderer> From<Connection<Message, Theme>>
    for GraphNodeElement<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: StyleSheet + 'a,
    Renderer: renderer::Renderer + MeshRenderer + 'a,
{
    fn from(node: Connection<Message, Theme>) -> Self {
        Self::new(node)
    }
}

#[derive(Debug, Clone)]
pub struct Link {
    start: Endpoint,
    end: Endpoint,
}

impl Link {
    pub fn new(start: Endpoint, end: Endpoint) -> Self {
        // Assert correct order
        if let Endpoint::Socket(l_start) = start {
            assert_eq!(l_start.role, SocketRole::Out)
        }
        if let Endpoint::Socket(l_end) = end {
            assert_eq!(l_end.role, SocketRole::In)
        }

        Self { start, end }
    }

    pub fn from_unordered(e1: Endpoint, e2: Endpoint) -> Self {
        match e1 {
            Endpoint::Absolute(_) => match e2 {
                Endpoint::Absolute(_) => Self::new(e1, e2),
                Endpoint::Socket(l2) => match l2.role {
                    SocketRole::In => Self::new(e1, e2),
                    SocketRole::Out => Self::new(e2, e1),
                },
            },
            Endpoint::Socket(l1) => {
                if let Endpoint::Socket(l2) = e2 {
                    if l1.role == l2.role {
                        panic!("tried to order two logical endpoints with same role");
                    }
                }

                match l1.role {
                    SocketRole::In => Self::new(e2, e1),
                    SocketRole::Out => Self::new(e1, e2),
                }
            }
        }
    }

    pub fn start(&self) -> &Endpoint {
        &self.start
    }

    pub fn end(&self) -> &Endpoint {
        &self.end
    }

    pub fn unwrap_sockets(&self) -> (&LogicalEndpoint, &LogicalEndpoint) {
        if let Endpoint::Socket(l_start) = &self.start {
            if let Endpoint::Socket(l_end) = &self.end {
                return (l_start, l_end);
            }
        }

        panic!("tried to call unwrap_sockets() on a Link containing absolute endpoints");
    }
}

#[derive(Debug, Clone)]
pub enum Endpoint {
    Absolute(Point),
    Socket(LogicalEndpoint),
}

impl Endpoint {
    pub fn socket(node_index: usize, role: SocketRole, socket_index: usize) -> Self {
        Endpoint::Socket(LogicalEndpoint {
            node_index,
            role,
            socket_index,
        })
    }

    pub fn start(node_index: usize, socket_index: usize) -> Self {
        Self::socket(node_index, SocketRole::Out, socket_index)
    }

    pub fn end(node_index: usize, socket_index: usize) -> Self {
        Self::socket(node_index, SocketRole::In, socket_index)
    }

    fn resolve(&self, scale: f32, socket_state: &super::node_element::SocketLayoutState) -> Vector {
        match self {
            Endpoint::Absolute(point) => Vector::new(point.x * scale, point.y * scale),
            Endpoint::Socket(logical) => {
                let node_sockets = match logical.role {
                    SocketRole::In => &socket_state.inputs,
                    SocketRole::Out => &socket_state.outputs,
                };

                match node_sockets.get(logical.node_index) {
                    Some(sockets) => match sockets.get(logical.socket_index) {
                        Some(rect) => Vector::new(rect.center_x(), rect.center_y()),
                        None => panic!("socket index {} out of bounds for role {:?} of node {}; only found {} socket(s)", logical.socket_index, logical.role, logical.node_index, sockets.len())
                    }
                    None => panic!("node index {} out of bounds for role {:?}; only found {} node(s)", logical.node_index, logical.role, node_sockets.len())
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LogicalEndpoint {
    pub node_index: usize,
    pub role: SocketRole,
    pub socket_index: usize,
}

fn line_to_polygon(points: &[Vector], width: f32) -> (Vec<Vector>, Vec<u32>) {
    let mut result = Vec::new();
    let mut indices = Vec::new();

    let mut last = points[0];
    for point in points.iter().skip(1) {
        let dir = normalize_vector(*point - last);
        let normal = Vector::new(dir.y, -dir.x);

        result.push(last + normal * width);
        result.push(*point + normal * width);
        result.push(*point - normal * width);
        result.push(last - normal * width);

        let start = result.len() as u32 - 4;
        indices.push(start);
        indices.push(start + 1);
        indices.push(start + 2);

        indices.push(start);
        indices.push(start + 2);
        indices.push(start + 3);

        last = *point;
    }

    (result, indices)
}

fn normalize_vector(vector: Vector) -> Vector {
    let length = (vector.x * vector.x + vector.y * vector.y).sqrt();
    if length == 0.0 {
        Vector::new(0.0, 0.0)
    } else {
        Vector::new(vector.x / length, vector.y / length)
    }
}

fn dot_vector(vector: Vector, other: Vector) -> f32 {
    vector.x * other.x + vector.y * other.y
}

fn generate_spline(
    from: Vector,
    control_scale: f32,
    to: Vector,
    number_of_segments: usize,
    alpha: f32,
) -> Vec<Vector> {
    let mut spline = Vec::new();

    for i in 0..number_of_segments {
        let t = i as f32 / (number_of_segments - 1) as f32;
        let p = catmull_rom(
            Vector::new(from.x - control_scale, from.y),
            from,
            to,
            Vector::new(to.x + control_scale, to.y),
            t,
            alpha,
        );
        spline.push(p);
    }

    spline
}

// Code taken and adapted from https://en.wikipedia.org/wiki/Centripetal_Catmull%E2%80%93Rom_spline
fn get_t(t: f32, alpha: f32, p0: Vector, p1: Vector) -> f32 {
    let d = p1 - p0;
    let a = dot_vector(d, d);
    let b = a.powf(alpha * 0.5);
    b + t
}

fn catmull_rom(p0: Vector, p1: Vector, p2: Vector, p3: Vector, t: f32, alpha: f32) -> Vector {
    let t0 = 0.0;
    let t1 = get_t(t0, alpha, p0, p1);
    let t2 = get_t(t1, alpha, p1, p2);
    let t3 = get_t(t2, alpha, p2, p3);
    let t = t1 + (t2 - t1) * t;
    let a1 = p0 * ((t1 - t) / (t1 - t0)) + p1 * ((t - t0) / (t1 - t0));
    let a2 = p1 * ((t2 - t) / (t2 - t1)) + p2 * ((t - t1) / (t2 - t1));
    let a3 = p2 * ((t3 - t) / (t3 - t2)) + p3 * ((t - t2) / (t3 - t2));
    let b1 = a1 * ((t2 - t) / (t2 - t0)) + a2 * ((t - t0) / (t2 - t0));
    let b2 = a2 * ((t3 - t) / (t3 - t1)) + a3 * ((t - t1) / (t3 - t1));
    b1 * ((t2 - t) / (t2 - t1)) + b2 * ((t - t1) / (t2 - t1))
}

fn bounds_for_vectors(points: &[Vector]) -> iced::Rectangle {
    let mut min_x = points[0].x;
    let mut min_y = points[0].y;
    let mut max_x = points[0].x;
    let mut max_y = points[0].y;

    for point in points.iter().skip(1) {
        if point.x < min_x {
            min_x = point.x;
        }
        if point.y < min_y {
            min_y = point.y;
        }
        if point.x > max_x {
            max_x = point.x;
        }
        if point.y > max_y {
            max_y = point.y;
        }
    }

    iced::Rectangle {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}
