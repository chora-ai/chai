#!/bin/sh
# Sanitize ls output for agent consumption:
# 1. Remove the "total N" header from ls -l output (disk block allocation — not useful)
# 2. Strip symlink target arrows to avoid leaking absolute paths outside the sandbox
# A line like:
#   lrwxrwxrwx 1 ryan users 29 May 27 09:47 chai -> /home/ryan/Code/chora-ai/chai
# becomes:
#   lrwxrwxrwx 1 ryan users 29 May 27 09:47 chai
sed '/^total/d' | sed 's/ -> .*//'
