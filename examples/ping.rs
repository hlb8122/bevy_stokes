use std::{net::SocketAddr, time::Duration};

use bevy::{core::FixedTimestep, log::LogPlugin, prelude::*};
use bevy_stokes::*;
use laminar::Packet;

const PING_ADDR: &str = "127.0.0.1:8000";
const PONG_ADDR: &str = "127.0.0.1:8001";

fn setup(mut commands: Commands) {
    let addr: SocketAddr = PING_ADDR.parse().unwrap();
    let socket_bundle = bind(addr, Duration::from_millis(10)).unwrap();
    commands.spawn_bundle(socket_bundle);
}

fn ping(mut socket_query: Query<&mut SendQueue, With<SocketMarker>>) {
    let mut packet_queue = socket_query.single_mut();
    let ping = Packet::reliable_unordered(PONG_ADDR.parse().unwrap(), b"DEADBEEF".to_vec());
    packet_queue.send(ping);
    info!("sent ping");
}

fn pong(mut connection_query: Query<&mut ReceiveQueue, With<ConnectionMarker>>) {
    if let Ok(mut queue) = connection_query.get_single_mut() {
        for _ in queue.drain() {
            info!("received pong");
        }
    }
}

pub fn main() {
    let ping_interval = SystemSet::new()
        .with_run_criteria(FixedTimestep::step(1.0))
        .with_system(ping);

    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugin(LogPlugin::default())
        .add_plugin(NetworkPlugin::always())
        .add_startup_system(setup)
        .add_system_set(ping_interval)
        .add_system(pong)
        .run()
}
