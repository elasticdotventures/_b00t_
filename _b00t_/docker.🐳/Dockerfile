# _b00t_ Development Environment Container
ARG GH_TOKEN
FROM ubuntu:24.04
ENV GH_TOKEN=$GH_TOKEN

# Set non-interactive frontend for apt
ENV DEBIAN_FRONTEND=noninteractive

# Set labels
LABEL org.opencontainers.image.source=https://github.com/elasticdotventures/dotfiles
LABEL org.opencontainers.image.description="_b00t_ initialization framework with comprehensive developer tooling"
LABEL org.opencontainers.image.licenses=MIT

# Set working directory
WORKDIR /root

# Install initial dependencies
RUN apt-get update && apt-get install -y \
    git \
    curl \
    wget \
    unzip \
    ca-certificates \
    software-properties-common \
    sudo \
    apt-utils \
    gnupg \
    lsb-release \
    build-essential \
    pkg-config \
    cmake \
    stow \
    && rm -rf /var/lib/apt/lists/*

# Create necessary directories
RUN mkdir -p /root/.local/bin /root/.kube

# Copy the entire repository into the image
COPY . /root/.dotfiles/

# Change to dotfiles directory
WORKDIR /root/.dotfiles

# Install Just command runner
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin

# Copy and run setup.sh to initialize the environment
# This is a long-running script. Any changes to it will invalidate the cache from this point forward.
RUN chmod +x setup.sh && ./setup.sh

# Run the just install command
RUN just install

# Set up shell configuration
RUN echo 'eval "$(starship init bash)"' >> /root/.bashrc \
    && echo 'eval "$(direnv hook bash)"' >> /root/.bashrc \
    && echo 'alias tf=tofu' >> /root/.bashrc \
    && echo 'alias itree="rg --files | tree --fromfile"' >> /root/.bashrc

# Stow the dotfiles
RUN stow -d /root/.dotfiles -t /root bash

# Ensure the .bashrc is sourced on container start
RUN echo "source ~/.bashrc" >> /root/.profile

# Set the default working directory
WORKDIR /workspace

# Set the entrypoint to bash
ENTRYPOINT ["/bin/bash"]
CMD ["-l"]
