use iced::advanced::{renderer, widget, Clipboard, Layout, Shell, Widget};
use iced::{
    alignment, event, mouse, Alignment, Background, Color, Element, Event, Length, Padding, Pixels,
    Point, Rectangle, Size, Vector,
};

use crate::{
    node_element::{GraphNodeElement, ScalableWidget},
    styles::node::StyleSheet,
};

pub struct Node<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
    Renderer::Theme: StyleSheet,
{
    width: Length,
    height: Length,
    max_width: f32,
    max_height: f32,
    padding: Padding,
    style: <Renderer::Theme as StyleSheet>::Style,
    content: Element<'a, Message, Renderer>,
    sockets: Vec<Socket<'a, Message, Renderer>>,
    socket_spacing: f32,
    position: Point,
    horizontal_alignment: alignment::Horizontal,
    vertical_alignment: alignment::Vertical,
    on_translate: Option<Box<dyn Fn((f32, f32)) -> Message + 'a>>,
}

pub struct Socket<'a, Message, Renderer> {
    pub role: SocketRole,

    pub min_height: f32,
    pub max_height: f32,

    pub blob_side: SocketSide,
    pub blob_radius: f32,
    pub blob_border_radius: f32,
    pub blob_color: Color,
    pub blob_border_color: Option<Color>,

    pub content: Element<'a, Message, Renderer>,
    pub content_alignment: alignment::Horizontal,
}

impl<'a, Message, Renderer> Socket<'a, Message, Renderer> {
    pub fn blob_rect(&self, node_left: f32, node_width: f32, center_y: f32) -> Rectangle {
        let x = match self.blob_side {
            SocketSide::Left => node_left,
            SocketSide::Right => node_left + node_width,
        };
        Rectangle::new(
            Point::new(x - self.blob_radius, center_y - self.blob_radius),
            Size::new(self.blob_radius * 2.0, self.blob_radius * 2.0),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketRole {
    In,
    Out,
}

#[derive(Debug)]
struct NodeState {
    drag_start_position: Option<Point>,
}

impl<'a, Message, Renderer> Node<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
    Renderer::Theme: StyleSheet,
{
    pub fn new<T>(content: T) -> Self
    where
        T: Into<Element<'a, Message, Renderer>>,
    {
        Node {
            width: Length::Shrink,
            height: Length::Shrink,
            max_width: f32::MAX,
            max_height: f32::MAX,
            padding: Padding::ZERO,
            style: Default::default(),
            content: content.into(),
            sockets: vec![],
            socket_spacing: 0.0,
            position: Point::new(0.0, 0.0),
            horizontal_alignment: alignment::Horizontal::Left,
            vertical_alignment: alignment::Vertical::Top,
            on_translate: None,
        }
    }

    pub fn on_translate<F>(mut self, f: F) -> Self
    where
        F: 'a + Fn((f32, f32)) -> Message,
    {
        self.on_translate = Some(Box::new(f));
        self
    }

    pub fn position(mut self, position: Point) -> Self {
        self.position = position;
        self
    }

    pub fn width(mut self, width: Length) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Length) -> Self {
        self.height = height;
        self
    }

    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn max_width(mut self, max_width: f32) -> Self {
        self.max_width = max_width;
        self
    }

    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn style(mut self, style: impl Into<<Renderer::Theme as StyleSheet>::Style>) -> Self {
        self.style = style.into();
        self
    }

    pub fn align_x(mut self, alignment: alignment::Horizontal) -> Self {
        self.horizontal_alignment = alignment;
        self
    }

    pub fn align_y(mut self, alignment: alignment::Vertical) -> Self {
        self.vertical_alignment = alignment;
        self
    }

    pub fn center_x(mut self) -> Self {
        self.horizontal_alignment = alignment::Horizontal::Center;
        self
    }

    pub fn center_y(mut self) -> Self {
        self.vertical_alignment = alignment::Vertical::Center;
        self
    }

    pub fn sockets(mut self, sockets: Vec<Socket<'a, Message, Renderer>>) -> Self {
        self.sockets = sockets;
        self
    }

    pub fn socket_spacing(mut self, socket_spacing: impl Into<Pixels>) -> Self {
        self.socket_spacing = socket_spacing.into().0;
        self
    }
}

pub fn node<'a, Message, Renderer>(
    content: impl Into<Element<'a, Message, Renderer>>,
) -> Node<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
    Renderer::Theme: StyleSheet,
{
    Node::new(content)
}

impl<'a, Message, Renderer> ScalableWidget<Message, Renderer> for Node<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
    Renderer::Theme: StyleSheet,
{
    fn layout(
        &self,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
        scale: f32,
        socket_state: &mut super::node_element::SocketLayoutState,
    ) -> iced::advanced::layout::Node {
        if socket_state.done {
            panic!("the graph content must consist of nodes, then connections; it is not allowed to have (more) nodes after the connections");
        }

        let limits = limits
            .loose()
            .max_width(self.max_width)
            .max_height(self.max_height)
            .width(self.width)
            .height(self.height);

        let mut content = self
            .content
            .as_widget()
            .layout(renderer, &limits.pad(self.padding).loose());

        let content_intrinsic_size = content.size();
        let padding = self.padding.fit(content_intrinsic_size, limits.max());

        let content_frame_size = limits.resolve(content.size());

        let content_available_width =
            content_frame_size.width * scale - padding.left - padding.right;
        let content_available_height =
            content_frame_size.height * scale - padding.top - padding.bottom;
        let content_available_size = Size::new(content_available_width, content_available_height);

        content.move_to(Point::new(padding.left, padding.top));
        content.align(
            Alignment::from(self.horizontal_alignment),
            Alignment::from(self.vertical_alignment),
            content_available_size,
        );

        let mut children = vec![content];

        let mut in_sockets: Vec<Rectangle> = vec![];
        let mut out_sockets: Vec<Rectangle> = vec![];

        let mut socket_top: f32 = content_available_size.height;
        for socket in self.sockets.iter() {
            socket_top += self.socket_spacing * scale;

            let socket_content_available_width =
                content_frame_size.width - padding.left - padding.right;

            let socket_limits = iced::advanced::layout::Limits::new(
                Size {
                    width: 0.0,
                    height: socket.min_height,
                },
                Size {
                    width: socket_content_available_width,
                    height: socket.max_height,
                },
            );

            let mut socket_content = socket.content.as_widget().layout(renderer, &socket_limits);

            let socket_content_size_scaled = Size::new(
                socket_content.size().width * scale,
                socket_content.size().height * scale,
            );
            let socket_area_size_scaled = Size::new(
                content_available_size.width,
                socket_content_size_scaled.height,
            );
            socket_content.align(
                Alignment::from(socket.content_alignment),
                Alignment::Center,
                socket_area_size_scaled,
            );

            let mut socket_node = iced::advanced::layout::Node::with_children(
                socket_area_size_scaled,
                vec![socket_content],
            );
            socket_node.move_to(Point::new(self.padding.left, padding.top + socket_top));
            children.push(socket_node);

            let blob_rect = socket.blob_rect(
                0.0,
                content_frame_size.width * scale,
                padding.top + socket_top + socket_area_size_scaled.height / 2.0,
            ) + (Vector::new(self.position.x, self.position.y) * scale);
            match socket.role {
                SocketRole::In => in_sockets.push(blob_rect),
                SocketRole::Out => out_sockets.push(blob_rect),
            }

            socket_top += socket_content_size_scaled.height;
        }

        socket_state.inputs.push(in_sockets);
        socket_state.outputs.push(out_sockets);

        let total_size = Size::new(
            content_frame_size.width * scale,
            padding.top + socket_top + padding.bottom,
        );
        let node = iced::advanced::layout::Node::with_children(total_size, children);

        node.translate(Vector::new(self.position.x, self.position.y) * scale)
    }
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for Node<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
    Renderer::Theme: StyleSheet,
{
    fn children(&self) -> Vec<widget::Tree> {
        let mut res = vec![widget::Tree::new(&self.content)];
        for socket in &self.sockets {
            res.push(widget::Tree::new(&socket.content));
        }
        res
    }

    fn diff(&self, tree: &mut widget::Tree) {
        let mut new_children: Vec<&dyn Widget<Message, Renderer>> = vec![self.content.as_widget()];
        for socket in &self.sockets {
            new_children.push(socket.content.as_widget());
        }
        tree.diff_children(new_children.as_slice())
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<widget::tree::State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(NodeState {
            drag_start_position: None,
        })
    }

    fn layout(
        &self,
        _renderer: &Renderer,
        _limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        todo!("This should never be called.")
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &<Renderer as iced::advanced::Renderer>::Theme,
        renderer_style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let style = theme.appearance(&self.style);
        let bounds = layout.bounds();

        if style.background.is_some() || style.border_width > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border_radius: style.border_radius.into(),
                    border_width: style.border_width,
                    border_color: style.border_color,
                },
                style
                    .background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        let mut children_iter = layout.children();
        let content_layout = children_iter
            .next()
            .expect("there should be a layout node for the graph node content");

        // Only draw node content if it would be sufficiently big
        if layout.bounds().width > content_layout.bounds().width
            && layout.bounds().height > content_layout.bounds().height
        {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &renderer::Style {
                    text_color: style.text_color.unwrap_or(renderer_style.text_color),
                },
                content_layout,
                cursor,
                viewport,
            );
        }

        for (socket_index, socket_layout) in children_iter.enumerate() {
            let socket = &self.sockets[socket_index];

            let child_layout = socket_layout
                .children()
                .next()
                .expect("the socket layout node should have one child");

            // Only draw socket content if it would be sufficiently big
            if socket_layout.bounds().width > child_layout.bounds().width
                && (socket_layout.bounds().height * 2.0) > child_layout.bounds().height
            {
                socket.content.as_widget().draw(
                    &tree.children[socket_index + 1],
                    renderer,
                    theme,
                    &renderer::Style {
                        text_color: style.text_color.unwrap_or(renderer_style.text_color),
                    },
                    child_layout,
                    cursor,
                    viewport,
                );
            }

            // Draw blob
            let blob_rect =
                socket.blob_rect(bounds.x, bounds.width, socket_layout.bounds().center_y());
            renderer.fill_quad(
                renderer::Quad {
                    bounds: blob_rect,
                    border_radius: socket.blob_border_radius.into(),
                    border_width: style.border_width,
                    border_color: socket.blob_border_color.unwrap_or(style.border_color),
                },
                Background::Color(socket.blob_color),
            );
        }
    }

    fn on_event(
        &mut self,
        tree: &mut widget::Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle<f32>,
    ) -> event::Status {
        let mut status = event::Status::Ignored;
        let state = tree.state.downcast_mut::<NodeState>();

        if let Some(cursor_position) = cursor.position() {
            if let Some(start) = state.drag_start_position {
                match event {
                    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                        state.drag_start_position = None;
                    }
                    Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                        let delta = cursor_position - start;
                        state.drag_start_position = Some(cursor_position);
                        if let Some(f) = &self.on_translate {
                            let message = f((delta.x, delta.y));
                            shell.publish(message);
                        }
                        status = event::Status::Captured;
                    }
                    _ => {}
                }
            } else {
                let mut layout_children_iter = layout.children();
                let content_layout = layout_children_iter
                    .next()
                    .expect("there should be a layout node for the graph node content");

                status = self.content.as_widget_mut().on_event(
                    &mut tree.children[0],
                    event.clone(),
                    content_layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );

                for (socket_index, socket_layout) in layout_children_iter.enumerate() {
                    if status == event::Status::Captured {
                        break;
                    }

                    status = self.sockets[socket_index].content.as_widget_mut().on_event(
                        &mut tree.children[socket_index + 1],
                        event.clone(),
                        socket_layout,
                        cursor,
                        renderer,
                        clipboard,
                        shell,
                        viewport,
                    );
                }
            }
        }

        if let Some(cursor_position) = cursor.position() {
            if status == event::Status::Ignored && layout.bounds().contains(cursor_position) {
                if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
                    state.drag_start_position = Some(cursor_position);
                    status = event::Status::Captured;
                }
            }
        }

        status
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn width(&self) -> Length {
        self.width
    }

    fn height(&self) -> Length {
        self.height
    }
}

impl<'a, Message, Renderer> From<Node<'a, Message, Renderer>>
    for GraphNodeElement<'a, Message, Renderer>
where
    Message: 'a,
    Renderer: renderer::Renderer + 'a,
    Renderer::Theme: StyleSheet,
{
    fn from(node: Node<'a, Message, Renderer>) -> Self {
        Self::new(node)
    }
}
