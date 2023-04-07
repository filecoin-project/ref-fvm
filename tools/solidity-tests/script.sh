#!/bin/bash

# BUNDLE varilabl emust be set
if [ -z "$BUNDLE" ]; then
    echo "builtin-actors bundle not specified; please set the BUNDLE variable"
    exit 1
fi

# fvm-bench tool path; if not user provided try to use the release from current path
fvm_bench=${FVM_BENCH:-../../target/release/fvm-bench}
if [ ! -e "$fvm_bench" ]; then
   echo "fvm-bench executable $fvm_bench does not exist"
   exit 1
fi

# Set the output directory to "./contracts-output" and create if it doesn't exist
output_dir="./contracts-output"
if [ ! -d "$output_dir" ]; then
  mkdir "$output_dir"
fi

# Clear directory of previous compiler output
rm "$output_dir"/*

# Recursively find all files in the ./tests directory that end with ".sol"
# Use solcjs to compile each and generate binary+abi output
# Place all output in $output_dir
echo "Compiling..."
# If we passed in part of a file name, we only compile that file
# ... and only the files we compile get run as tests.
if [ -z "$1" ]; then
  find ./tests -name "*.sol" -exec sh -c "echo Compiling {}; solcjs --optimize --bin --abi {} --output-dir ${output_dir}" \;
else
  find ./tests -name "*$1*.sol" -exec sh -c "echo Compiling {}; solcjs --optimize --bin --abi {} --output-dir ${output_dir}" \;
fi

echo "Testing contracts..."
echo " "

# Find all files in the "./contracts-output" directory that end with ".bin"
for bin_file in "$output_dir"/*.bin; do
  # Skip calling any libraries we've added to the libraries dir
  if [[ $bin_file == "$output_dir/libraries_"* ]]; then
    # echo "Skipping library: $bin_file"
    continue
  fi

  # if [[ $bin_file != *"TestRecursiveCall.bin" ]]; then
  #   continue
  # fi

  # Run fvm-bench on the compiled file
  # Call the `testEntry()` function, and send no other calldata
  output=$("$fvm_bench" -b "$BUNDLE" "$bin_file" c0406226 0000000000000000000000000000000000000000000000000000000000000000)

  # echo "$output"

  # echo "Parsing output for $bin_file:"
  gas_used=$(echo "$output" | grep "Gas Used:")
  # echo "Output:"
  # echo "=========="
  # echo "$gas_used"
  # echo "=========="

  if [ $? -ne 0 ]; then
    exit 1
  fi

  # Parse the output to retrieve the returndata from the "Result" line
  returndata=$(echo "$output" | grep "Result:" | awk '{print $2}')
  # echo "Raw returndata:"
  # echo "=========="
  # echo "$returndata"
  # echo "=========="

  # Use forge-cast to abi-decode the returndata and echo the result
  # Note: right now, you need to manually change the return params
  #       here if you change testEntry() to return something new
  decoded=$(cast --abi-decode "run()(string[])" "0x$returndata")
  echo "Test results for $bin_file:"
  echo "=========="
  echo "$gas_used"
  echo $decoded | jq -r ".[]"
  echo "=========="
  echo " "
done
