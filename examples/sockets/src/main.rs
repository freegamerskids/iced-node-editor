use iced::widget::{container, text};
use iced::{Color, Element, Length, Padding, Point, Sandbox, Settings};
use iced_node_editor::{
    graph_container, node, Connection, Endpoint, GraphNodeElement, Link, LogicalEndpoint, Matrix,
    Socket, SocketRole, SocketSide,
};
use std::collections::HashMap;

pub fn main() -> iced::Result {
    // To resize the the resulting canvas for web: https://github.com/iced-rs/iced/issues/1265
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().unwrap();
        let (width, height) = (
            (window.inner_width().unwrap().as_f64().unwrap()) as u32,
            (window.inner_height().unwrap().as_f64().unwrap()) as u32,
        );

        Example::run(Settings {
            window: iced::window::Settings {
                size: (width, height),
                ..Default::default()
            },
            ..Default::default()
        })?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    Example::run(Settings {
        window: iced::window::Settings {
            size: (800, 600),
            ..Default::default()
        },
        ..Default::default()
    })?;

    Ok(())
}

struct NodeState {
    position: Point,
    text: String,
    sockets: (Vec<SocketType>, Vec<SocketType>),
}

// Define some types that sockets may have.
// The library does not perform any sort of type checking; it is entirely up to user code to verify
// that sockets with correct types are connected to each other. In this example, we just use the
// types to provide sockets with two different appearances (that behave identically).
enum SocketType {
    BlueSquare,
    RedCircle,
}

struct Example {
    matrix: Matrix,
    nodes: Vec<NodeState>,

    // Adjacency map of connections: the key corresponds to the node and socket index of the
    // connection **target** — the one on the right of the connection, the *input* socket at
    // which this connection ends. This is the better representation, because disconnections
    // originate from input sockets, and so we can easily look up the connections ending at
    // a certain input socket.
    //
    // For this example, we also make the restriction that only one connection may end in a
    // specific input socket, so it is doubly beneficial because we do not need a `Vec`
    // in the value type.
    connections: HashMap<(usize, usize), (usize, usize)>,

    // Our own representation of the “dangling connection” — the connection that follows the user's
    // mouse pointer in the process of connecting two sockets with each other.
    // It is divided into two parts:
    //  - the `dangling_source` represents the endpoint from which the dangling connection
    //    originates *logically*. It is used to provide correct functionality when connecting nodes.
    //  - the `dangling_connection` is essentially purely aesthetic; it is just an additional
    //    connection that is drawn such that the user gets some feedback on what they are doing.
    dangling_source: Option<LogicalEndpoint>,
    dangling_connection: Option<Link>,
}

#[derive(Debug, Clone)]
enum Message {
    ScaleChanged(f32, f32, f32),
    TranslationChanged(f32, f32),
    MoveNode(usize, f32, f32),
    Connect(Link),
    Disconnect(LogicalEndpoint, Point),
    Dangling(Option<(LogicalEndpoint, Link)>),
}

impl Sandbox for Example {
    type Message = Message;

    fn new() -> Self {
        let mut connections = HashMap::new();
        connections.insert((2, 0), (1, 1)); // Output socket #1 of node #1 to input socket #0 of node #2
        connections.insert((1, 0), (0, 1)); // Output socket #1 of node #0 to input socket #0 of node #1

        Example {
            matrix: Matrix::identity(),
            nodes: vec![
                // Node #0
                NodeState {
                    position: Point::new(0.0, 0.0),
                    text: String::from("Iced"),
                    sockets: (vec![], vec![SocketType::BlueSquare, SocketType::RedCircle]),
                },
                // Node #1
                NodeState {
                    position: Point::new(250.0, 250.0),
                    text: String::from("Node"),
                    sockets: (
                        vec![SocketType::RedCircle],
                        vec![SocketType::RedCircle, SocketType::BlueSquare],
                    ),
                },
                // Node #2
                NodeState {
                    position: Point::new(500.0, 250.0),
                    text: String::from("Editor"),
                    sockets: (vec![SocketType::BlueSquare, SocketType::RedCircle], vec![]),
                },
            ],
            connections,
            dangling_source: None,
            dangling_connection: None,
        }
    }

    fn title(&self) -> String {
        String::from("Iced Node Editor - Sockets Example")
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::ScaleChanged(x, y, scale) => {
                self.matrix = self
                    .matrix
                    .translate(-x, -y)
                    .scale(if scale > 0.0 { 1.2 } else { 1.0 / 1.2 })
                    .translate(x, y);
            }
            Message::TranslationChanged(x, y) => self.matrix = self.matrix.translate(x, y),
            Message::MoveNode(i, x, y) => {
                self.nodes[i].position = Point::new(
                    self.nodes[i].position.x + x / self.matrix.get_scale(),
                    self.nodes[i].position.y + y / self.matrix.get_scale(),
                );
            }
            Message::Connect(link) => {
                // The call to `unwrap_sockets` will panic if the `link` contains absolute
                // endpoints. But the `Connect` message is guaranteed to only contain `Link`s with
                // both endpoints being sockets.
                let (start, end) = link.unwrap_sockets();

                // Insert the new connection. The hash map design ensures that this will delete any
                // potentially previously present connections ending in the same node.
                self.connections.insert(
                    (end.node_index, end.socket_index),
                    (start.node_index, start.socket_index),
                );
            }
            Message::Disconnect(endpoint, new_dangling_end_position) => {
                // Remove the connection that ends in the socket, if it exists
                if let Some((start_node_index, start_socket_index)) = self
                    .connections
                    .remove(&(endpoint.node_index, endpoint.socket_index))
                {
                    // If there was a connection, turn it into a dangling one, such that the user
                    // may connect it to some other socket instead. First, set the source of the
                    // new dangling connection
                    let new_dangling_source = LogicalEndpoint {
                        node_index: start_node_index,
                        role: SocketRole::Out,
                        socket_index: start_socket_index,
                    };
                    self.dangling_source = Some(new_dangling_source);

                    // Construct a link for the dangling connection.
                    //
                    // This is not necessary just for correct behaviour.
                    // The node editor would emit the `Dangling` event with the full `Link`
                    // as soon as the mouse is moved anyway.
                    // However, if we do not do this, no dangling connection will be drawn until
                    // the mouse is moved. To be able to avoid this slight jank, the library
                    // provides us with a destination point to construct a new dangling connection
                    // for ourselves.
                    self.dangling_connection = Some(Link::from_unordered(
                        Endpoint::Socket(new_dangling_source),
                        Endpoint::Absolute(new_dangling_end_position),
                    ));
                }
            }
            Message::Dangling(Some((source, link))) => {
                // The dangling connection is updated, perhaps because the user moved their mouse
                self.dangling_source = Some(source);
                self.dangling_connection = Some(link);
            }
            Message::Dangling(None) => {
                // The dangling connection is cleared, e.g. when releasing the left mouse button
                // while not hovering over a valid target socket
                self.dangling_source = None;
                self.dangling_connection = None;
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let mut graph_content: Vec<GraphNodeElement<Message, _>> = vec![];

        // Convert our own node representations into widgets
        for (i, n) in self.nodes.iter().enumerate() {
            // Create sockets from our lists of `SocketType`s
            let (in_sockets, out_sockets) = &n.sockets;
            let mut node_sockets = vec![];
            for (role, sockets) in [(SocketRole::In, in_sockets), (SocketRole::Out, out_sockets)] {
                for socket_type in sockets {
                    // Call our own utility function to create the socket
                    let new_socket = make_socket(role, socket_type);
                    node_sockets.push(new_socket);
                }
            }

            graph_content.push(
                node(text(&n.text))
                    .padding(Padding::from(10.0))
                    .sockets(node_sockets)
                    .center_x()
                    .center_y()
                    .on_translate(move |p| Message::MoveNode(i, p.0, p.1))
                    .width(Length::Fixed(200.0))
                    .height(Length::Fixed(75.0))
                    .position(n.position)
                    .into(),
            );
        }

        // Convert our own `HashMap` representation of connections into the one used by the library.
        // Here it is important that this happens *after* the nodes have been added.
        // The socket layouting logic needs to process first the nodes, then the connections,
        // to have the information necessary to correctly position connection endpoints.
        for ((end_node_index, end_socket_index), (start_node_index, start_socket_index)) in
            self.connections.iter()
        {
            graph_content.push(
                Connection::between(
                    Endpoint::Socket(LogicalEndpoint {
                        node_index: *start_node_index,
                        role: SocketRole::Out,
                        socket_index: *start_socket_index,
                    }),
                    Endpoint::Socket(LogicalEndpoint {
                        node_index: *end_node_index,
                        role: SocketRole::In,
                        socket_index: *end_socket_index,
                    }),
                )
                .into(),
            );
        }

        // Append the dangling connection, if one exists
        if let Some(link) = &self.dangling_connection {
            graph_content.push(Connection::new(link.clone()).into())
        }

        container(
            graph_container(graph_content)
                .dangling_source(self.dangling_source)
                .on_translate(|p| Message::TranslationChanged(p.0, p.1))
                .on_scale(Message::ScaleChanged)
                .on_connect(Message::Connect)
                .on_disconnect(Message::Disconnect)
                .on_dangling(Message::Dangling)
                .width(Length::Fill)
                .height(Length::Fill)
                .matrix(self.matrix),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

fn make_socket<'a, Message, Renderer>(
    role: SocketRole,
    socket_type: &SocketType,
) -> Socket<'a, Message, Renderer>
where
    Renderer: iced::advanced::text::Renderer + 'a,
    Renderer::Theme: text::StyleSheet,
{
    // With this, we determine that the input sockets should be on the left side of a node
    // and the output sockets on the right side. The opposite would be possible as well,
    // as would a more complex arrangement where some input and output sockets are on the same side.
    let blob_side = match role {
        SocketRole::In => SocketSide::Left,
        SocketRole::Out => SocketSide::Right,
    };

    // In principle, we could also decouple the alignment of the socket content
    // (which is the element that is displayed within the node at the same height as the blob)
    // from the position of the blob, such that for example, a socket's blob appears on
    // the left side, but its label on the right side.
    // Here, we go with the obvious assignment
    let content_alignment = match role {
        SocketRole::In => iced::alignment::Horizontal::Left,
        SocketRole::Out => iced::alignment::Horizontal::Right,
    };

    const BLOB_RADIUS: f32 = 5.0;

    // The style of the blob is not determined by a style sheet, but by properties of the `Socket`
    // itself.
    let (blob_border_radius, blob_color, label) = match socket_type {
        SocketType::BlueSquare => (0.0, Color::from_rgb(0.0, 0.1, 0.8), "Blue square"),
        SocketType::RedCircle => (BLOB_RADIUS, Color::from_rgb(0.8, 0.1, 0.0), "Red circle"),
    };

    Socket {
        role,
        blob_side,
        content_alignment,

        blob_radius: BLOB_RADIUS,
        blob_border_radius,
        blob_color,
        content: text(label).into(), // Arbitrary widgets can be used here.

        min_height: 0.0,
        max_height: f32::INFINITY,
        blob_border_color: None, // If `None`, the one from the style sheet will be used.
    }
}
