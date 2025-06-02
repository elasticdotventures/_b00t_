# _b00t_ Development Environment Container
FROM ubuntu:24.04

# Set non-interactive frontend for apt
ENV DEBIAN_FRONTEND=noninteractive

# Set labels
LABEL org.opencontainers.image.source=https://github.com/elasticdotventures/dotfiles
LABEL org.opencontainers.image.description="_b00t_ initialization framework with comprehensive developer tooling"
LABEL org.opencontainers.image.licenses=MIT

# Set working directory
WORKDIR /root

# Install initial dependencies needed to clone the repository and run setup.sh
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
    bat \
    tree \
    fzf \
    ripgrep \
    jq \
    stow \
    inotify-tools \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Create necessary directories
RUN mkdir -p /root/.local/bin /root/.kube

# Clone the dotfiles repository
RUN git clone https://github.com/elasticdotventures/dotfiles /root/.dotfiles --depth 1

# Change to dotfiles directory
WORKDIR /root/.dotfiles

# Install Rust with non-interactive mode
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install OpenTofu (Terraform alternative)
RUN curl --proto '=https' --tlsv1.2 -fsSL 'https://packages.opentofu.org/install/repositories/opentofu/tofu/script.deb.sh?any=true' | bash \
    && apt-get update && apt-get install -y tofu && rm -rf /var/lib/apt/lists/*

# Install AWS CLI
RUN curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip" \
    && unzip awscliv2.zip \
    && ./aws/install \
    && rm -rf aws awscliv2.zip

# Install Google Cloud SDK
RUN echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | tee -a /etc/apt/sources.list.d/google-cloud-sdk.list \
    && curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | apt-key --keyring /usr/share/keyrings/cloud.google.gpg add - \
    && apt-get update && apt-get install -y google-cloud-cli \
    && rm -rf /var/lib/apt/lists/*

# Install other tools from cargo
RUN cargo install starship --locked \
    && cargo install dotenvy --bin dotenvy --features cli \
    && cargo install datafusion-cli

# Install Just command runner
RUN curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to /usr/local/bin

# Install eget and move it to the right location
RUN curl -sSfL https://raw.githubusercontent.com/zyedidia/eget/master/install.sh | bash \
    && cp eget /usr/local/bin/ \
    && chmod +x /usr/local/bin/eget

# Install wasm-to-oci
RUN wget https://github.com/engineerd/wasm-to-oci/releases/download/v0.1.2/linux-amd64-wasm-to-oci \
    && mv linux-amd64-wasm-to-oci wasm-to-oci \
    && chmod +x wasm-to-oci \
    && cp wasm-to-oci /usr/local/bin/

# Install Direnv
RUN curl -sfL https://direnv.net/install.sh | bash

# Install Pixi
RUN curl -fsSL https://pixi.sh/install.sh | bash

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