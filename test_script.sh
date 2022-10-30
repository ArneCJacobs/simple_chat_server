#!/usr/bin/env bash
SESSION_NAME="CLIENT_SERVER_TEST"
if test -z "$TMUX";
then
  # if not currently in a tmux session, create one create new windows
  tmux kill-session -t $SESSION_NAME
  tmux new-session -s $SESSION_NAME -d 'cargo run listen' \; split-window -d -h "cargo run speak; zsh" \; attach
else
  # if in a tmux session
  LAST=$(tmux display-message -p '#I')
  if tmux select-window -t $SESSION_NAME
  then
    # if a widow is already created, re-run commands
    tmux select-pane -t 1 \; respawn-pane -k \; select-pane -t 2 \; respawn-pane -k
  else
    # if a window is not created, create setup and run commands
    tmux new-window -n $SESSION_NAME \; split-window -t $SESSION_NAME -h 
    tmux set-option -wt $SESSION_NAME remain-on-exit on
    tmux select-pane -t 1 \; respawn-pane -k 'cargo run listen' \; select-pane -t 2 \; respawn-pane -k 'cargo run speak'
  fi
  tmux select-window -t "$LAST"
fi
