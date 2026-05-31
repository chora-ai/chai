#!/bin/sh
# Sanitize ls output: strip symlink target arrows to avoid leaking
# absolute paths outside the sandbox to the agent. A line like:
#   lrwxrwxrwx 1 ryan users 29 May 27 09:47 chai -> /home/ryan/Code/chora-ai/chai
# becomes:
#   lrwxrwxrwx 1 ryan users 29 May 27 09:47 chai
sed 's/ -> .*//'
