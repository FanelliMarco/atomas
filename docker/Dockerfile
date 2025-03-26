FROM nvidia/cuda:12.8.1-cudnn-devel-ubuntu22.04

ENV DEBIAN_FRONTEND=noninteractive

# Update packages and install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    xz-utils \
    pkg-config \
    libssl-dev \
    zlib1g-dev \
    libtinfo-dev \
    libxml2-dev \
    git \
    cmake \
    neovim \
    libopencv-dev && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*

# Install Rust using rustup
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y && \
    echo 'source $HOME/.cargo/env' >> ~/.bashrc && \
    . ~/.bashrc

# Install prebuilt LLVM
RUN curl -O https://releases.llvm.org/7.0.1/clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04.tar.xz && \
    tar -xf clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04.tar.xz && \
    rm clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04.tar.xz && \
    mv clang+llvm-7.0.1-x86_64-linux-gnu-ubuntu-18.04 /root/llvm

# Set environment variables
ENV LLVM_CONFIG=/root/llvm/bin/llvm-config
ENV CUDA_ROOT=/usr/local/cuda
ENV CUDA_PATH=$CUDA_ROOT
ENV LLVM_LINK_STATIC=1
ENV RUST_LOG=info
ENV PATH=$CUDA_ROOT/nvvm/lib64:/root/.cargo/bin:$PATH

# Configure shared library paths
RUN echo $CUDA_ROOT/lib64 >> /etc/ld.so.conf && \
    echo $CUDA_ROOT/compat >> /etc/ld.so.conf && \
    echo $CUDA_ROOT/nvvm/lib64 >> /etc/ld.so.conf && \
    ldconfig

# Create app directory
WORKDIR /atomas

# Copy the project directory one level up into the container
COPY ../ /atomas/

# Build the Rust project in release mode
RUN ~/.cargo/bin/cargo build --release

# Set the runtime command to run the compiled binary
CMD ["./target/release/atomas"]

