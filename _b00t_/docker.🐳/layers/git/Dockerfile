# syntax=docker/dockerfile:1.7
# Layer: git
# Git version control system

FROM b00t-layer/debian-base:bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends git && \
    rm -rf /var/lib/apt/lists/*

LABEL layer.name="git" \
      layer.version="2.43" \
      layer.description="Git version control system"
