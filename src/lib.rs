#![deny(
    missing_docs,
    missing_debug_implementations,
    single_use_lifetimes,
    unreachable_pub
)]
#![forbid(unsafe_code)]

//! A [bevy](https://github.com/bevyengine/bevy/) plugin providing a thin and ergonomic wrapper
//! around [laminar](https://github.com/TimonPost/laminar).
//!
//! Both sockets and their connections are represented by entities and distinguished by
//! [`SocketMarker`] and [`ConnectionMarker`] respectively.
//!
//! To send packets one should use [`SendQueue`] on the socket entity. Conversely, to receive
//! packets one should use [`ReceiveQueue`] on the connection entity.
//!
//! Connection entities will be spawned automatically by a socket entity when a message has been
//! received. In addition to [`ReceiveQueue`] and [`ConnectionMarker`] they will include
//! [`SocketId`] and [`ConnectionAddress`].
//!
//! Socket entities must be spawned by the user and [`Bundle`]s describing them are yielded from
//! the [`bind`] and [`bind_with_config`] functions. In addition to [`ReceiveQueue`] and
//! [`SocketMarker`] they will include [`PollInterval`].

mod connection;
mod socket;

use std::{
    collections::{hash_map::Entry, HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bevy::prelude::*;

pub use connection::*;
use laminar::SocketEvent;
pub use laminar::{Config, Packet};
pub use socket::*;

fn flush_send(mut query: Query<(&mut Socket, &mut SendQueue)>) {
    for (mut socket, mut queue) in query.iter_mut() {
        for packet in queue.0.drain(..) {
            if let Err(error) = socket.0.send(packet) {
                error!(message = "failed to send", %error);
            }
        }
    }
}

/// Represents the current state of a connection.
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    /// Connection has been sent and received over.
    Connected,
    /// Connection has received a message.
    Pending,
    /// Connection has been disconnected.
    Disconnected,
}

struct Action {
    state: Option<ConnectionState>,
    packets: VecDeque<Packet>,
}

#[derive(Bundle)]
struct ConnectionBundle {
    marker: ConnectionMarker,
    socket_id: SocketId,
    address: ConnectionAddress,
    queue: ReceiveQueue,
    state: ConnectionState,
}

fn drain_recv(
    mut socket_query: Query<(Entity, &mut Socket, Option<&ConnectionBuilder>), With<SocketMarker>>,
    mut connection_query: Query<
        (
            Entity,
            &SocketId,
            &ConnectionAddress,
            &mut ReceiveQueue,
            &mut ConnectionState,
        ),
        With<ConnectionMarker>,
    >,

    mut commands: Commands,
) {
    for (socket_id, mut socket, builder_opt) in socket_query.iter_mut() {
        let mut actions: HashMap<SocketAddr, Action> = HashMap::new();

        while let Some(event) = socket.0.recv() {
            match event {
                SocketEvent::Connect(connect_address) => {
                    trace!(message = "connect event", address = %connect_address);

                    actions
                        .entry(connect_address)
                        .and_modify(|action| action.state = Some(ConnectionState::Connected))
                        .or_insert(Action {
                            state: Some(ConnectionState::Connected),
                            packets: VecDeque::new(),
                        });
                }
                SocketEvent::Disconnect(disconnect_address) => {
                    trace!(message = "disconnect event", address = %disconnect_address);

                    actions
                        .entry(disconnect_address)
                        .and_modify(|action| action.state = Some(ConnectionState::Disconnected))
                        .or_insert(Action {
                            state: Some(ConnectionState::Disconnected),
                            packets: VecDeque::new(),
                        });
                }
                SocketEvent::Packet(packet) => {
                    let packet_addr = packet.addr();

                    trace!(message = "packet event", address = %packet_addr);

                    match actions.entry(packet_addr) {
                        Entry::Occupied(mut action) => {
                            action.get_mut().packets.push_back(packet);
                        }
                        Entry::Vacant(empty) => {
                            empty.insert(Action {
                                state: None,
                                packets: [packet].into(),
                            });
                        }
                    }
                }
                SocketEvent::Timeout(timeout_address) => {
                    trace!(message = "timeout event", address = %timeout_address);
                }
            }
        }

        for (connection_addr, action) in actions.into_iter() {
            let result = connection_query
                .iter_mut()
                .find(|(_, id, addr, _, _)| id.0 == socket_id && addr.0 == connection_addr);

            if let Some((_, _, _, mut queue, mut state)) = result {
                queue.0.extend(action.packets);
                if let Some(new_state) = action.state {
                    *state = new_state;
                }
            } else {
                trace!(message = "spawning connection", address = %connection_addr);

                let mut entity_commands = commands.spawn_bundle(ConnectionBundle {
                    marker: ConnectionMarker,
                    socket_id: SocketId(socket_id),
                    address: ConnectionAddress(connection_addr),
                    queue: ReceiveQueue(action.packets),
                    state: action.state.unwrap_or(ConnectionState::Pending),
                });
                if let Some(builder) = builder_opt {
                    builder.0(connection_addr, &mut entity_commands)
                }
            }
        }
    }
}

/// Binds to a UDP socket, with provided [`Config`] and `poll_interval`, returning a [`Bundle`].
///
/// The `poll_interval` sets the elapsed time before polls.
///
/// The returned [`Bundle`] must be spawned in order to use the socket. It will include
/// [`PollInterval`], [`SocketMarker`], and [`SendQueue`].
#[must_use = "The returned Bundle must be spawned to use the socket"]
pub fn bind_with_config<A>(
    addresses: A,
    poll_interval: Duration,
    config: Config,
) -> Result<impl Bundle, laminar::ErrorKind>
where
    A: ToSocketAddrs,
{
    let socket = laminar::Socket::bind_with_config(addresses, config)?;

    Ok(SocketBundle {
        marker: SocketMarker,
        socket: Socket(socket),
        last_poll: LastPoll(None),
        poll_interval: PollInterval(poll_interval),
        send_queue: SendQueue::default(),
    })
}

/// Binds to a UDP socket, with default [`Config`] and provided `poll_interval`, returning a
/// [`Bundle`].
///
/// See [`bind_with_config`] for more details.
pub fn bind<A>(addresses: A, poll_interval: Duration) -> Result<impl Bundle, laminar::ErrorKind>
where
    A: ToSocketAddrs,
{
    bind_with_config(addresses, poll_interval, Config::default())
}

/// A [`Plugin`] encapsulating the networking systems.
pub struct NetworkPlugin {
    system_set_f: Box<dyn Fn() -> SystemSet + Send + Sync + 'static>,
}

impl Debug for NetworkPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkPlugin").finish_non_exhaustive()
    }
}

impl NetworkPlugin {
    /// The plugin will always run.
    pub fn always() -> Self {
        Self {
            system_set_f: Box::new(SystemSet::new),
        }
    }

    /// The plugin will run only in a specified state.
    pub fn on_state<State>(state: State) -> Self
    where
        State: Send + Sync + 'static,
        State: Clone + Eq + Hash + Debug,
    {
        Self {
            system_set_f: Box::new(move || SystemSet::on_update(state.clone())),
        }
    }
}

/// Labels enumerating the different network systems.
///
/// The order is `Poll` < `Recv` < `Send`, which means that anything sent will be performed next
/// tick.
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub enum NetworkSystemLabels {
    /// Labels the system polling the underlying socket.
    Poll,
    /// Labels the system draining the packets from the socket.
    Recv,
    /// Labels the system draining the sending packets.
    Send,
}

impl SystemLabel for NetworkSystemLabels {
    fn dyn_clone(&self) -> Box<dyn SystemLabel> {
        Box::new(*self)
    }
}

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        let polling_set = (self.system_set_f)()
            .label(NetworkSystemLabels::Poll)
            .with_system(socket_poll);
        let recv_set = (self.system_set_f)()
            .label(NetworkSystemLabels::Recv)
            .after(NetworkSystemLabels::Poll)
            .with_system(drain_recv);
        let send_set = (self.system_set_f)()
            .label(NetworkSystemLabels::Send)
            .after(NetworkSystemLabels::Recv)
            .with_system(flush_send);

        app.add_system_set(polling_set)
            .add_system_set(send_set)
            .add_system_set(recv_set);
    }
}
