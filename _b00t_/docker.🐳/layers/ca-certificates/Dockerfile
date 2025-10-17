# syntax=docker/dockerfile:1.7
# Layer: ca-certificates
# CA certificates for HTTPS connections

FROM b00t-layer/debian-base:bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

LABEL layer.name="ca-certificates" \
      layer.version="bookworm" \
      layer.description="CA certificates for HTTPS"
