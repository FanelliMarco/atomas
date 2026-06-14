FROM ghcr.io/rust-gpu/rust-cuda-ubuntu24-cuda12:latest

ENV DEBIAN_FRONTEND=noninteractive

# ============================================================================
# BASE SYSTEM PACKAGES
# ============================================================================
RUN apt-get update && apt-get install -y \
    build-essential \
    gcc \
    cmake \
    pkg-config \
    dirmngr \
    ca-certificates \
    software-properties-common \
    apt-transport-https \
    dkms \
    curl \
    wget \
    unzip \
    xz-utils \
    git \
    tmux \
    libssl-dev \
    zlib1g-dev \
    libtinfo-dev \
    libxml2-dev \
    libopencv-dev \
    llvm-dev \
    libclang-dev \
    clang \
    libsdl2-dev \
    libsdl2-gfx-dev \
    luarocks \
    lua5.1 \
    liblua5.1-dev \
    lib32stdc++6 \
    lib32z1 \
    libc6-dev-i386 \
    qemu-kvm \
    bridge-utils \
    cpu-checker \
    fd-find \
    ripgrep \
    gzip \
    gh \
    sudo \
    socat \
    dos2unix \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# ============================================================================
# NEOVIM
# ============================================================================
RUN add-apt-repository ppa:neovim-ppa/unstable -y && \
    apt-get update && \
    apt-get install -y neovim && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# ============================================================================
# JAVA
# ============================================================================
ARG JAVA_VERSION=21
RUN apt-get update && \
    apt-get install -y openjdk-${JAVA_VERSION}-jdk && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

ENV JAVA_HOME="/usr/lib/jvm/java-${JAVA_VERSION}-openjdk-amd64"

# ============================================================================
# NODE.JS (via NVM)
# ============================================================================
ENV NVM_DIR="/root/.nvm"
ENV NODE_VERSION="20.11.0"

RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.3/install.sh | bash && \
    . "$NVM_DIR/nvm.sh" && \
    nvm install ${NODE_VERSION} && \
    nvm alias default ${NODE_VERSION} && \
    nvm use default && \
    npm install --global tabby-agent

ENV NODE_PATH="$NVM_DIR/versions/node/v${NODE_VERSION}/lib/node_modules"
ENV PATH="$NVM_DIR/versions/node/v${NODE_VERSION}/bin:$PATH"

# ============================================================================
# RR DEBUGGER
# ============================================================================
RUN cd /tmp && \
    wget https://github.com/rr-debugger/rr/releases/download/5.9.0/rr-5.9.0-Linux-$(uname -m).deb && \
    dpkg -i rr-5.9.0-Linux-$(uname -m).deb || apt-get install -f -y && \
    rm -f rr-5.9.0-Linux-$(uname -m).deb

# ============================================================================
# Z3 THEOREM PROVER
# ============================================================================
RUN apt-get update && apt-get install -y z3 && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

# ============================================================================
# TAILSCALE
# ============================================================================
RUN curl -fsSL https://tailscale.com/install.sh | sh

# ============================================================================
# LLVM 7.0.1
# ============================================================================
RUN curl -O https://releases.llvm.org/7.0.1/clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04.tar.xz && \
    tar -xf clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04.tar.xz && \
    rm clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04.tar.xz && \
    mv clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04 /root/llvm

# ============================================================================
# RUST
# ============================================================================
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y && \
    echo 'source $HOME/.cargo/env' >> ~/.bashrc

# ============================================================================
# ENVIRONMENT VARIABLES
# ============================================================================
ENV LLVM_CONFIG=/root/llvm/bin/llvm-config
ENV CUDA_ROOT=/usr/local/cuda
ENV CUDA_PATH=$CUDA_ROOT
ENV LLVM_LINK_STATIC=1
ENV RUST_LOG=info
ENV PATH="$CUDA_ROOT/nvvm/lib64:/root/.cargo/bin:$PATH"

RUN echo $CUDA_ROOT/lib64 >> /etc/ld.so.conf && \
    echo $CUDA_ROOT/compat >> /etc/ld.so.conf && \
    echo $CUDA_ROOT/nvvm/lib64 >> /etc/ld.so.conf && \
    ldconfig

# ============================================================================
# ANDROID SDK SETUP
# ============================================================================
WORKDIR /opt

# Copy and install Android SDK
COPY docker/scripts/install-sdk.sh /opt/
RUN dos2unix /opt/install-sdk.sh && \
    chmod +x /opt/install-sdk.sh && \
    /opt/install-sdk.sh

# Copy all scripts and fix line endings
COPY docker/scripts/*.sh /opt/
RUN dos2unix /opt/*.sh && \
    chmod +x /opt/*.sh

# ============================================================================
# APPLICATION SETUP
# ============================================================================
WORKDIR /atomas

COPY . /atomas/

RUN bash -c 'source /root/.cargo/env && cargo build --release'

# ============================================================================
# RUNTIME
# ============================================================================
EXPOSE 5037 5554-5585

ENTRYPOINT ["/bin/bash", "/opt/entrypoint.sh"]
