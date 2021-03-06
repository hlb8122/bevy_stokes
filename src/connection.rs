use std::{collections::VecDeque, net::SocketAddr};

use bevy::prelude::*;
use laminar::Packet;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A marker [`Component`] for the connection entity.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Default, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub struct ConnectionMarker;

/// A [`Component`] storing the peers [`SocketAddr`] within a connection.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub struct ConnectionAddress(pub SocketAddr);

/// A [`Component`] used to relate connections with their socket.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub struct SocketId(pub Entity);

/// A [`Component`] storing all packets received from a peer.
#[derive(Debug, Default, Clone, Component, PartialEq, Eq)]
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
