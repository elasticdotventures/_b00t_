---
DataFabricPipeline write fanout MUST go through b00t-ipc Transport bus (not tokio: :join!). NATS subject pattern: b00t.data_fabric.{namespace}.upsert — grafeo and zvec each subscribe independently. Read path stays direct (no request-reply bus overhead for in-process stores). pipeline.rs: with_bus(T: Transport) + start_workers() to arm subscribers.

---
NATS core has NO persistence — publishing before subscribers exist silently drops data. DataFabricPipeline guards this with AtomicBool workers_started: bus_upsert() returns Err until start_workers() is called. If JetStream durability is needed, switch transport to async_nats::jetstream.
