use std::{
    fmt::Debug,
    net::SocketAddr,
    time::{Duration, Instant},
};

use bevy::{ecs::system::EntityCommands, prelude::*};
use laminar::Packet;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A marker [`Component`] for the socket entity.
#[cfg_attr(features = "serde", Serialize, Deserialize)]
#[derive(Debug, Default, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub struct SocketMarker;

#[derive(Debug, Component)]
pub(crate) struct Socket(pub(crate) laminar::Socket);

/// A [`Component`] representing the minimum interval between socket polls.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Default, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub struct PollInterval(pub Duration);

#[derive(Debug, Default, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub(crate) struct LastPoll(pub(crate) Option<Instant>);

/// A [`Component`] storing all packets to be sent to a peer.
#[derive(Debug, Default, Clone, Component, PartialEq, Eq)]
pub struct SendQueue(pub(crate) Vec<Packet>);

impl SendQueue {
    /// Sends a [`Packet`] to a peer.
    pub fn send(&mut self, packet: Packet) {
        self.0.push(packet)
    }
}

/// A [`Component`] whose presence on a socket entity causes a modification to new connections.
#[derive(Component)]
pub struct ConnectionBuilder(
    pub(crate) Box<dyn Fn(SocketAddr, &mut EntityCommands) + Send + Sync + 'static>,
);

impl ConnectionBuilder {
    /// Creates a new [`ConnectionBuilder`] from a closure. This closure is run against the
    /// [`ConnectionAddress`] and [`EntityCommands`] of new connections.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(SocketAddr, &mut EntityCommands) + Send + Sync + 'static,
    {
        Self(Box::new(f))
    }

    /// Creates a new [`ConnectionBuilder`] which adjoins a component onto new connections.
    pub fn adjoin_component<C>(component: C) -> Self
    where
        C: Component + Clone,
    {
        Self(Box::new(move |_, commands| {
            commands.insert(component.clone());
        }))
    }
}

impl Debug for ConnectionBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MarkNewConnection")
            .field(&format_args!("_"))
            .finish()
    }
}

#[derive(Bundle)]
pub(crate) struct SocketBundle {
    pub(crate) marker: SocketMarker,
    pub(crate) socket: Socket,
    pub(crate) last_poll: LastPoll,
    pub(crate) poll_interval: PollInterval,
    pub(crate) send_queue: SendQueue,
}

pub(crate) fn socket_poll(
    time: Res<Time>,
    mut query: Query<(&mut Socket, &mut LastPoll, &PollInterval)>,
) {
    // Fetch current instant
    let now = if let Some(some) = time.last_update() {
        some
    } else {
        return;
    };

    for (mut socket, mut last_poll, poll_interval) in query.iter_mut() {
        // Only poll if interval is exceeded

        if let LastPoll(Some(instant)) = last_poll.as_mut() {
            if *instant + poll_interval.0 < now {
                *instant = now;
            } else {
                // Do not poll if interval has not completed
                continue;
            }
        } else {
            *last_poll = LastPoll(Some(now));
        }

        socket.0.manual_poll(now);
    }
}
