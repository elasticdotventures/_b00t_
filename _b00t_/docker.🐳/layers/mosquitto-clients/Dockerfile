# syntax=docker/dockerfile:1.7
# Layer: mosquitto-clients
# MQTT client tools (mosquitto_pub, mosquitto_sub)

FROM b00t-layer/debian-base:bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends mosquitto-clients && \
    rm -rf /var/lib/apt/lists/*

LABEL layer.name="mosquitto-clients" \
      layer.version="2.0" \
      layer.description="MQTT client tools"
