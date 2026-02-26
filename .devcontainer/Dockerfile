FROM mcr.microsoft.com/devcontainers/rust:2.0.8-bookworm

RUN apt-get update

# install wasm pack, flamegraph, and nasm
RUN \
  apt-get install -y linux-perf nasm mtools dosfstools xxd \
  && rustup target add wasm32-unknown-unknown && cargo install wasm-pack \
  && cargo install flamegraph \
  && rm -rf /usr/local/cargo/registry

# install node
USER vscode
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.4/install.sh | bash
USER root
