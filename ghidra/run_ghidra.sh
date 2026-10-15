#!/bin/bash
# usage: run_ghidra.sh NAME   (ROOT or overlay name)
source ~/.local/opt/u5env.sh
NAME=$1
D=~/Projects/Ultima5Revolutions/ghidra
mkdir -p $D/export/$NAME $D/logs
$GHIDRA_HOME/support/analyzeHeadless $D/proj U5_$NAME -import "$HOME/Projects/Ultima 5/ULTIMA.EXE" -processor "x86:LE:16:Real Mode" -loader MzLoader -overwrite \
  -scriptPath $D/scripts -preScript SetupOverlay.java $NAME -postScript ExportAll.java $D/export/$NAME > $D/logs/$NAME.log 2>&1
echo "$NAME exit=$? $(grep -c 'ERROR' $D/logs/$NAME.log) errors"
