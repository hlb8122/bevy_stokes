use std::{collections::VecDeque, net::SocketAddr};

use bevy::prelude::*;
use laminar::Packet;

/// A marker [`Component`] for the connection entity.
#[derive(Component)]
pub struct ConnectionMarker;

/// A [`Component`] storing the peers [`SocketAddr`] within a connection.
#[derive(Component)]
pub struct ConnectionAddress(pub SocketAddr);

/// A [`Component`] used to relate connections with their socket.
#[derive(Component)]
pub struct SocketId(pub Entity);

/// A [`Component`] storing all packets received from a peer.
#[derive(Component)]
pub struct ReceiveQueue(pub(crate) VecDeque<Packet>);

impl ReceiveQueue {
    /// Returns the number of packets.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the queue has a length of 0.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterates over the stored packets.
    pub fn iter(&self) -> impl Iterator<Item = &Packet> {
        self.0.iter()
    }

    /// Iterates over the stored packets while consuming them.
    pub fn drain(&mut self) -> impl Iterator<Item = Packet> + '_ {
        self.0.drain(..)
    }
}

#[derive(Bundle)]
pub(crate) struct ConnectionBundle {
    pub marker: ConnectionMarker,
    pub socket_id: SocketId,
    pub address: ConnectionAddress,
    pub queue: ReceiveQueue,
}

pub(crate) fn spawn_connection(
    socket_id: Entity,
    address: SocketAddr,
    first_message: Option<Packet>,
    commands: &mut Commands,
) {
    trace!(message = "spawning connection", %address);

    let queue = first_message
        .map(|packet| VecDeque::from([packet]))
        .unwrap_or_default();

    commands.spawn_bundle(ConnectionBundle {
        marker: ConnectionMarker,
        socket_id: SocketId(socket_id),
        address: ConnectionAddress(address),
        queue: ReceiveQueue(queue),
    });
}