#!/usr/bin/env bash

tmux kill-session
tmux new-session -d 'cargo run listen'
tmux split-window -h 'cargo run speak; zsh'
tmux -2 attach-session -d
