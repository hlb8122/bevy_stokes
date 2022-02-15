#![deny(missing_docs)]

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

#[inline]
fn find_connection<'a>(
    socket_id: Entity,
    connection_address: SocketAddr,
    connection_query: &'a mut Query<
        (Entity, &SocketId, &ConnectionAddress, &mut ReceiveQueue),
        With<ConnectionMarker>,
    >,
) -> Option<(Entity, Mut<'a, ReceiveQueue>)> {
    connection_query
        .iter_mut()
        .find(|(_, id, addr, _)| id.0 == socket_id && addr.0 == connection_address)
        .map(|(id, _, _, queue)| (id, queue))
}

fn flush_send(mut query: Query<(&mut Socket, &mut SendQueue)>) {
    for (mut socket, mut queue) in query.iter_mut() {
        for packet in queue.0.drain(..) {
            if let Err(error) = socket.0.send(packet) {
                error!(message = "failed to send", %error);
            }
        }
    }
}

fn drain_recv(
    mut socket_query: Query<(Entity, &mut Socket), With<SocketMarker>>,
    mut connection_query: Query<
        (Entity, &SocketId, &ConnectionAddress, &mut ReceiveQueue),
        With<ConnectionMarker>,
    >,

    mut commands: Commands,
) {
    for (socket_id, mut socket) in socket_query.iter_mut() {
        // Use these to avoid duplicate spawns/despawns
        let mut spawned = Vec::new();
        let mut despawned = Vec::new();

        while let Some(event) = socket.0.recv() {
            match event {
                SocketEvent::Connect(connect_address) => {
                    trace!(message = "connect event", address = %connect_address);

                    let conn_opt =
                        find_connection(socket_id, connect_address, &mut connection_query);

                    if conn_opt.is_none() && !spawned.contains(&connect_address) {
                        spawned.push(connect_address);
                        spawn_connection(socket_id, connect_address, None, &mut commands);
                    }
                }
                SocketEvent::Disconnect(disconnect_address) => {
                    trace!(message = "disconnect event", address = %disconnect_address);

                    let connection_opt =
                        find_connection(socket_id, disconnect_address, &mut connection_query);

                    if let Some((id, _)) = connection_opt {
                        if !despawned.contains(&disconnect_address) {
                            despawned.push(disconnect_address);
                            commands.entity(id).despawn();
                        }
                    }
                }
                SocketEvent::Packet(packet) => {
                    let packet_addr = packet.addr();

                    trace!(message = "packet event", address = %packet_addr);

                    let connection_opt =
                        find_connection(socket_id, packet_addr, &mut connection_query);

                    if let Some((_, mut message_queue)) = connection_opt {
                        message_queue.0.push_front(packet);
                    } else if !spawned.contains(&packet_addr) {
                        spawned.push(packet_addr);
                        spawn_connection(socket_id, packet_addr, Some(packet), &mut commands);
                    }
                }
                SocketEvent::Timeout(timeout_address) => {
                    trace!(message = "timeout event", address = %timeout_address);

                    let connection_opt =
                        find_connection(socket_id, timeout_address, &mut connection_query);

                    if let Some((id, _)) = connection_opt {
                        if !despawned.contains(&timeout_address) {
                            despawned.push(timeout_address);
                            commands.entity(id).despawn();
                        }
                    }
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
    system_set_new: Box<dyn Fn() -> SystemSet + Send + Sync + 'static>,
}

impl NetworkPlugin {
    /// The plugin will always run.
    pub fn always() -> Self {
        Self {
            system_set_new: Box::new(SystemSet::new),
        }
    }

    /// The plugin will run only in a specified state.
    pub fn on_state<State>(state: State) -> Self
    where
        State: Send + Sync + 'static,
        State: Clone + Eq + Hash + Debug,
    {
        Self {
            system_set_new: Box::new(move || SystemSet::on_update(state.clone())),
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
        let polling_set = (self.system_set_new)()
            .label(NetworkSystemLabels::Poll)
            .with_system(socket_poll);
        let recv_set = (self.system_set_new)()
            .label(NetworkSystemLabels::Recv)
            .after(NetworkSystemLabels::Poll)
            .with_system(drain_recv);
        let send_set = (self.system_set_new)()
            .label(NetworkSystemLabels::Send)
            .after(NetworkSystemLabels::Recv)
            .with_system(flush_send);

        app.add_system_set(polling_set)
            .add_system_set(send_set)
            .add_system_set(recv_set);
    }
}
