FROM ghcr.io/kicad/kicad:nightly-full

# Install additional dependencies if required
RUN apt-get update && apt-get install -y \
    git \
    curl \
    just

# Clone additional repositories or set up configuration files as needed
RUN git clone https://github.com/example/example-plugin.git /opt/example-plugin

# Set up environment variables, paths, etc.
ENV PATH="/opt/example-plugin/bin:${PATH}"
