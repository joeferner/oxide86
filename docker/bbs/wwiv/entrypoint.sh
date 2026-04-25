#!/bin/bash
set -e

if [ ! -f /opt/wwiv/.initialized ]; then
    echo "First run: installing WWIV..."
    mkdir -p /opt/wwiv
    tar -xzf /tmp/wwiv.tar.gz -C /opt/wwiv
    cd /opt/wwiv && ./install.sh --force --yes
    touch /opt/wwiv/.initialized
fi

getent group wwiv &>/dev/null || groupadd -g 1000 wwiv
id -u wwiv &>/dev/null || useradd -u 1000 -g 1000 -m -d /opt/wwiv -s /bin/bash wwiv

exec su -s /bin/bash wwiv -c "cd /opt/wwiv && exec ./wwivd --v"
