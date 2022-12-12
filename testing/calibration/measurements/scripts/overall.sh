#!/usr/bin/env bash

set -e

# Merged traces file.
OBS_DIR=$1
PNG_FILE=$2
PLT_FILE=$(dirname $0)/$(basename $0 .sh).plt
DAT_FILE=$(dirname $PNG_FILE)/$(basename $PNG_FILE .png).dat

mkdir -p $(dirname $PNG_FILE)
rm -rf $DAT_FILE

JSON_FILES=$(find $OBS_DIR -type f \( -name "*.jsonline" \))
SERIES=""

for JSON_FILE in $JSON_FILES; do
  SERIE=$(basename $JSON_FILE .jsonline)
  SERIES="$SERIES $SERIE"
  cat $JSON_FILE \
    | jq -r "[.elapsed_nanos, .compute_gas] | @tsv" \
    >> $DAT_FILE
  # Series separator for gnuplot
  echo $'\n' >> $DAT_FILE
done

gnuplot \
  -e "filein='$DAT_FILE'" \
  -e "fileout='$PNG_FILE'" \
  -e "series='$(echo $SERIES | tr _ - )'" \
  $PLT_FILE

rm $DAT_FILE
