#!/usr/bin/env bash

set -e

# Merged traces file.
JSON_FILE=$1
PNG_FILE=$2
PLT_FILE=$(dirname $0)/$(basename $0 .sh).plt
DAT_FILE=$(dirname $PNG_FILE)/$(basename $PNG_FILE .png).dat
TITLE=$(basename $JSON_FILE .jsonline)

mkdir -p $(dirname $PNG_FILE)
rm -rf $DAT_FILE

SERIES=$(cat $JSON_FILE | jq -r ".label" | sort | uniq)

for SERIE in $SERIES; do
  cat $JSON_FILE \
    | jq -r "select(.label == \"${SERIE}\") | [.variables[0], .elapsed_nanos, .compute_gas] | @tsv" \
    >> $DAT_FILE
  # Series separator for gnuplot
  echo $'\n' >> $DAT_FILE
done

gnuplot \
  -e "filein='$DAT_FILE'" \
  -e "fileout='$PNG_FILE'" \
  -e "title='$(echo $TITLE | tr _ - )'" \
  -e "series='$(echo $SERIES | tr _ - )'" \
  $PLT_FILE

rm $DAT_FILE
