use std::time::{Duration, Instant};

use bevy::prelude::*;
use laminar::Packet;

/// A marker [`Component`] for the socket entity.
#[derive(Debug, Default, Clone, Copy, Component, PartialEq, Eq, Hash)]
pub struct SocketMarker;

#[derive(Debug, Component)]
pub(crate) struct Socket(pub(crate) laminar::Socket);

/// A [`Component`] representing the minimum interval between socket polls.
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
