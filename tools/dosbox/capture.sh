#!/bin/bash
# usage: capture.sh <seconds-to-wait> <out.png> [dosbox extra -c commands...]
WAIT=${1:-8}; OUT=${2:-shot.png}
export DISPLAY=:99
Xvfb :99 -screen 0 800x600x24 >/dev/null 2>&1 &
XPID=$!
sleep 1
dosbox -conf ~/Projects/Ultima5Revolutions/tools/dosbox/headless.conf -noconsole >/dev/null 2>&1 &
DPID=$!
sleep $WAIT
import -display :99 -window root "$OUT"
kill $DPID $XPID 2>/dev/null
wait 2>/dev/null
echo "captured $OUT"
