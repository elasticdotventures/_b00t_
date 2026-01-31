//! OpenTelemetry metrics for b00t-chat transports.
//!
//! Provides comprehensive observability for agent communication:
//! - Message throughput (sent/received counts)
//! - Latency distributions
//! - Error rates and types
//! - Connection health
//! - Transport-specific metrics

use opentelemetry::{
    global,
    metrics::{Counter, Histogram, Meter, UpDownCounter},
    KeyValue,
};
use std::sync::OnceLock;
use std::time::Instant;

/// Global metrics instance (initialized once).
static METRICS: OnceLock<ChatMetrics> = OnceLock::new();

/// Metrics for b00t-chat transport operations.
#[derive(Clone)]
pub struct ChatMetrics {
    _meter: Meter,

    // Message counters
    messages_sent: Counter<u64>,
    messages_received: Counter<u64>,
    messages_failed: Counter<u64>,

    // Latency metrics
    send_latency: Histogram<f64>,
    recv_latency: Histogram<f64>,

    // Connection metrics
    connections_active: UpDownCounter<i64>,
    connection_errors: Counter<u64>,

    // Transport-specific
    transport_operations: Counter<u64>,
}

impl ChatMetrics {
    /// Initialize global metrics instance.
    pub fn init() -> &'static ChatMetrics {
        METRICS.get_or_init(|| {
            let meter = global::meter("b00t-chat");

            ChatMetrics {
                _meter: meter.clone(),
                messages_sent: meter
                    .u64_counter("b00t.messages.sent")
                    .with_description("Total messages sent")
                    .build(),
                messages_received: meter
                    .u64_counter("b00t.messages.received")
                    .with_description("Total messages received")
                    .build(),
                messages_failed: meter
                    .u64_counter("b00t.messages.failed")
                    .with_description("Total messages that failed to send")
                    .build(),
                send_latency: meter
                    .f64_histogram("b00t.send.latency")
                    .with_description("Message send latency in milliseconds")
                    .with_unit("ms")
                    .build(),
                recv_latency: meter
                    .f64_histogram("b00t.recv.latency")
                    .with_description("Message receive latency in milliseconds")
                    .with_unit("ms")
                    .build(),
                connections_active: meter
                    .i64_up_down_counter("b00t.connections.active")
                    .with_description("Number of active connections")
                    .build(),
                connection_errors: meter
                    .u64_counter("b00t.connections.errors")
                    .with_description("Connection error count")
                    .build(),
                transport_operations: meter
                    .u64_counter("b00t.transport.operations")
                    .with_description("Transport-specific operations")
                    .build(),
            }
        })
    }

    /// Get global metrics instance (initializes if needed).
    pub fn global() -> &'static ChatMetrics {
        Self::init()
    }

    /// Record a message sent.
    pub fn record_message_sent(&self, transport: &str, channel: &str) {
        self.messages_sent.add(
            1,
            &[
                KeyValue::new("transport", transport.to_string()),
                KeyValue::new("channel", channel.to_string()),
            ],
        );
    }

    /// Record a message received.
    pub fn record_message_received(&self, transport: &str, channel: &str) {
        self.messages_received.add(
            1,
            &[
                KeyValue::new("transport", transport.to_string()),
                KeyValue::new("channel", channel.to_string()),
            ],
        );
    }

    /// Record a message send failure.
    pub fn record_message_failed(&self, transport: &str, error_type: &str) {
        self.messages_failed.add(
            1,
            &[
                KeyValue::new("transport", transport.to_string()),
                KeyValue::new("error", error_type.to_string()),
            ],
        );
    }

    /// Record send latency.
    pub fn record_send_latency(&self, transport: &str, duration_ms: f64) {
        self.send_latency.record(
            duration_ms,
            &[KeyValue::new("transport", transport.to_string())],
        );
    }

    /// Record receive latency.
    pub fn record_recv_latency(&self, transport: &str, duration_ms: f64) {
        self.recv_latency.record(
            duration_ms,
            &[KeyValue::new("transport", transport.to_string())],
        );
    }

    /// Record connection established.
    pub fn record_connection_opened(&self, transport: &str) {
        self.connections_active
            .add(1, &[KeyValue::new("transport", transport.to_string())]);
    }

    /// Record connection closed.
    pub fn record_connection_closed(&self, transport: &str) {
        self.connections_active
            .add(-1, &[KeyValue::new("transport", transport.to_string())]);
    }

    /// Record connection error.
    pub fn record_connection_error(&self, transport: &str, error_type: &str) {
        self.connection_errors.add(
            1,
            &[
                KeyValue::new("transport", transport.to_string()),
                KeyValue::new("error", error_type.to_string()),
            ],
        );
    }

    /// Record transport-specific operation.
    pub fn record_transport_operation(&self, transport: &str, operation: &str) {
        self.transport_operations.add(
            1,
            &[
                KeyValue::new("transport", transport.to_string()),
                KeyValue::new("operation", operation.to_string()),
            ],
        );
    }
}

/// Latency measurement helper.
pub struct LatencyTimer {
    start: Instant,
    transport: String,
    operation: &'static str,
}

impl LatencyTimer {
    /// Start measuring latency for a send operation.
    pub fn send(transport: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            transport: transport.into(),
            operation: "send",
        }
    }

    /// Start measuring latency for a receive operation.
    pub fn recv(transport: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            transport: transport.into(),
            operation: "recv",
        }
    }

    /// Stop the timer and record the latency.
    pub fn stop(self) {
        let duration_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        let metrics = ChatMetrics::global();

        match self.operation {
            "send" => metrics.record_send_latency(&self.transport, duration_ms),
            "recv" => metrics.record_recv_latency(&self.transport, duration_ms),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let metrics = ChatMetrics::global();
        metrics.record_message_sent("test", "mission.alpha");
        metrics.record_message_received("test", "mission.alpha");
    }

    #[test]
    fn test_latency_timer() {
        let timer = LatencyTimer::send("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.stop();
    }
}
