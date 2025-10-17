# syntax=docker/dockerfile:1.7
# Layer: build-essential
# GCC, make, and essential build tools

FROM b00t-layer/debian-base:bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config && \
    rm -rf /var/lib/apt/lists/*

LABEL layer.name="build-essential" \
      layer.version="bookworm" \
      layer.description="GCC, make, and essential build tools"
