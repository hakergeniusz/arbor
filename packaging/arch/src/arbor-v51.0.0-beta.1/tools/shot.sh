#!/bin/bash
# shot.sh <name> [crop WxH+X+Y] — capture Xvfb :98 screen (optionally cropped)
# Crop arg uses ImageMagick syntax WxH+X+Y and is translated to ffmpeg w:h:x:y.
# Images go to /tmp/ai-agent/shots/<name>.png
set -euo pipefail
source /tmp/ai-agent/baobab-env.sh
NAME=$1; shift || true
mkdir -p /tmp/ai-agent/shots
OUT=/tmp/ai-agent/shots/$NAME.png
if [ "$#" -ge 1 ]; then
  CROP=$(echo "$1" | sed -E 's/^([0-9]+)x([0-9]+)\+([0-9]+)\+([0-9]+)$/\1:\2:\3:\4/')
  ffmpeg -y -hide_banner -loglevel error -f x11grab -video_size 1440x900 -i :98 -frames:v 1 -vf "crop=$CROP" -update 1 "$OUT"
else
  ffmpeg -y -hide_banner -loglevel error -f x11grab -video_size 1440x900 -i :98 -frames:v 1 -update 1 "$OUT"
fi
echo "$OUT"
