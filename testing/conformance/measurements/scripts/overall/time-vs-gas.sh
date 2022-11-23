#!/usr/bin/env bash

set -e

# Overall traces file.
TRACES=$1
OUT_DIR=$2

mkdir -p $OUT_DIR

DAT_FILE=$OUT_DIR/$(basename $TRACES .jsonline).dat
PNG_FILE=$OUT_DIR/$(basename $0 .sh).png
PLT_FILE=$(dirname $0)/$(basename $0 .sh).plt

cat $TRACES \
  | jq -r "[.elapsed_nanos, .gas_burned] | @tsv" \
  > $DAT_FILE

gnuplot \
  -e "filein='$DAT_FILE'" \
  -e "fileout='$PNG_FILE'" \
  $PLT_FILE

rm $DAT_FILE
