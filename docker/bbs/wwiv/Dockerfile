FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    wget \
    unzip \
    expect \
    libncurses5 \
    libncursesw5 \
    libstdc++6 \
    libc6 \
    zip \
    sudo \
    dos2unix \
    screen \
    vim \
    systemd \
    lrzsz \
    && rm -rf /var/lib/apt/lists/*

RUN wget -q https://github.com/wwivbbs/wwiv/releases/download/wwiv_5.8.1/wwiv-linux-ubuntu2004-5.8.1.3688.tar.gz \
    -O /tmp/wwiv.tar.gz

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENV WWIV_DIR=/opt/wwiv
ENV WWIV_USER=wwiv

EXPOSE 2323

ENTRYPOINT ["/entrypoint.sh"]
