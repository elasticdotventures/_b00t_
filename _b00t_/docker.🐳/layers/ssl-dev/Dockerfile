# syntax=docker/dockerfile:1.7
# Layer: ssl-dev
# OpenSSL development libraries for building

FROM b00t-layer/debian-base:bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends libssl-dev && \
    rm -rf /var/lib/apt/lists/*

LABEL layer.name="ssl-dev" \
      layer.version="bookworm" \
      layer.description="OpenSSL development libraries"
