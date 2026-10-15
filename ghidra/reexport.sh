#!/bin/bash
source ~/.local/opt/u5env.sh
NAME=$1
D=~/Projects/Ultima5Revolutions/ghidra
$GHIDRA_HOME/support/analyzeHeadless $D/proj U5_$NAME -process ULTIMA.EXE -noanalysis -scriptPath $D/scripts -postScript ExportAll.java $D/export/$NAME > $D/logs/reexport_$NAME.log 2>&1
echo "$NAME exit=$?"
