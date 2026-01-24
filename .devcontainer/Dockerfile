FROM mcr.microsoft.com/devcontainers/rust:2.0.6-bookworm

RUN apt-get update

# install wasm pack and flamegraph
RUN \
  apt-get install -y linux-perf \
  && rustup target add wasm32-unknown-unknown && cargo install wasm-pack \
  && cargo install flamegraph \
  && rm -rf /usr/local/cargo/registry

# install node
USER vscode
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
USER root
