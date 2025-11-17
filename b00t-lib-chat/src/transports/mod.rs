//! Transport implementations for various messaging protocols.
//!
//! Provides concrete implementations of the IpcTransport trait for:
//! - NATS (cloud-native, high-scale)
//! - MQTT (IoT, edge, lightweight)

pub mod mqtt_transport;
pub mod nats_transport;

pub use mqtt_transport::MqttTransport;
pub use nats_transport::NatsTransport;
