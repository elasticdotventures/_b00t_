# syntax=docker/dockerfile:1.7
# Layer: ssl-runtime
# OpenSSL runtime libraries

FROM b00t-layer/debian-base:bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends libssl3 && \
    rm -rf /var/lib/apt/lists/*

LABEL layer.name="ssl-runtime" \
      layer.version="bookworm" \
      layer.description="OpenSSL runtime libraries"
