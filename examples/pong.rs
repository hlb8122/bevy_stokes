use std::{net::SocketAddr, time::Duration};

use bevy::{log::LogPlugin, prelude::*};
use bevy_stokes::*;
use laminar::Packet;

const PONG_ADDR: &str = "127.0.0.1:8001";

fn setup(mut commands: Commands) {
    let addr: SocketAddr = PONG_ADDR.parse().unwrap();
    let socket_bundle = bind(addr, Duration::from_millis(10)).unwrap();
    commands.spawn_bundle(socket_bundle);
}

fn pong(
    mut socket_query: Query<&mut SendQueue, With<SocketMarker>>,
    mut connection_query: Query<
        (&SocketId, &ConnectionAddress, &mut ReceiveQueue),
        With<ConnectionMarker>,
    >,
) {
    if let Ok((socket_id, conn_addr, mut queue)) = connection_query.get_single_mut() {
        for ping in queue.drain() {
            info!("received ping");

            let mut packet_queue = socket_query.get_mut(socket_id.0).unwrap();
            let pong = Packet::reliable_unordered(conn_addr.0, ping.payload().to_vec());
            packet_queue.send(pong);
            info!("returned pong");
        }
    }
}

pub fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugin(LogPlugin::default())
        .add_plugin(NetworkPlugin::always())
        .add_startup_system(setup)
        .add_system(pong)
        .run()
}
