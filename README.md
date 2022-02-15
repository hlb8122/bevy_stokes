# bevy_stokes

A [bevy](https://github.com/bevyengine/bevy/) plugin providing a thin and ergonomic wrapper around 
[laminar](https://github.com/TimonPost/laminar).

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Cargo](https://img.shields.io/crates/v/bevy_stokes.svg)](https://crates.io/crates/bevy_stokes)
[![Documentation](https://docs.rs/bevy_stokes/badge.svg)](
https://docs.rs/bevy_stokes)

## Example


### Ping

```rust
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
```

### Pong

```rust
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
```