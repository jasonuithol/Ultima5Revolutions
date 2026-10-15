#!/bin/bash
source ~/.local/opt/u5env.sh
NAME=$1
D=~/Projects/Ultima5Revolutions/ghidra
mkdir -p $D/export/DRV_$NAME $D/logs
$GHIDRA_HOME/support/analyzeHeadless $D/proj U5_DRV_$NAME -import "$HOME/Projects/Ultima 5/$NAME.DRV" -processor "x86:LE:16:Real Mode" -loader BinaryLoader -loader-baseAddr 3000:0000 -overwrite \
  -scriptPath $D/scripts -preScript DrvSetup.java -postScript ExportAll.java $D/export/DRV_$NAME > $D/logs/DRV_$NAME.log 2>&1
echo "$NAME exit=$? $(grep -c 'ERROR' $D/logs/DRV_$NAME.log) errors"
