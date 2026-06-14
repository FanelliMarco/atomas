#!/bin/bash
set -e

echo "Starting emulator in tmux session..."

# Start tmux with a persistent shell, then run emulator
tmux new-session -d -s emulator bash

# Send the emulator start command to the tmux session
tmux send-keys -t emulator "source /root/.bashrc && /opt/start-emulator.sh" C-m

echo "Emulator started in tmux session 'emulator'"
echo "Attach: tmux attach -t emulator"

tail -f /dev/null
