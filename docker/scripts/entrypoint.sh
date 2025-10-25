#!/bin/bash
set -e

source /root/.bashrc
source /root/.cargo/env
[ -f "$NVM_DIR/nvm.sh" ] && source "$NVM_DIR/nvm.sh"

echo "Starting emulator..."
/opt/start-emulator.sh > /var/log/emulator.log 2>&1 &
echo $! > /var/run/emulator.pid

echo "Emulator PID: $(cat /var/run/emulator.pid)"
echo "Logs: tail -f /var/log/emulator.log"

tail -f /dev/null
