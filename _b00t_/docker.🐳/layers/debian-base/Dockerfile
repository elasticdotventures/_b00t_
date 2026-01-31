# syntax=docker/dockerfile:1.7
# Layer: debian-base
# Minimal Debian bookworm base system

FROM debian:bookworm-slim

# Clean apt cache to minimize layer size
RUN rm -rf /var/lib/apt/lists/*

# Set non-interactive frontend
ENV DEBIAN_FRONTEND=noninteractive

LABEL layer.name="debian-base" \
      layer.version="bookworm" \
      layer.description="Minimal Debian bookworm base system"
