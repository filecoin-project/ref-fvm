#!/usr/bin/env bash

set -e

# Merged traces file.
TRACES=$1
OUT_DIR=$2
TITLE=$3
CHARGE=$4

mkdir -p $OUT_DIR

PLT_FILE=$(dirname $0)/$(basename $0 .sh).plt

DAT_FILE=$OUT_DIR/${CHARGE}.dat
JSON_FILE=$OUT_DIR/${CHARGE}.jsonline
PNG_FILE=$OUT_DIR/${CHARGE}.${TITLE}.png

# Ignoring .storage_gas for now.
cat $TRACES \
  | jq -c "select(.name == \"${CHARGE}\" and .elapsed_nanos != null)" \
  > $JSON_FILE

ELAPSED_CUTOFF=$(cat $JSON_FILE | jq --slurp "map(.elapsed_nanos) | sort | .[length * 0.95 | floor]")

cat $JSON_FILE \
  | jq -r "select(.elapsed_nanos <= $ELAPSED_CUTOFF) | [.elapsed_nanos, .compute_gas] | @tsv" \
  > $DAT_FILE

gnuplot \
  -e "filein='$DAT_FILE'" \
  -e "fileout='$PNG_FILE'" \
  -e "charge='$(echo $CHARGE | tr _ - )'" \
  -e "max_elapsed=$ELAPSED_CUTOFF" \
  $PLT_FILE

rm $DAT_FILE
rm $JSON_FILE
