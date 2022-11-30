#!/bin/bash
#
# Checks if the source code contains required license and adds it if necessary.
# Returns 1 if there was a missing license, 0 otherwise.
COPYRIGHT_TXT=$(dirname $0)/copyright.txt

# Any year is fine. We can update the year as a single PR in all files that have it up to last year.
PAT_PL=".*// Copyright 2021-202\d Protocol Labs.*"
# Files that were taken from ChainSafe and not modified can be left as-is.
PAT_CS=".*// Copyright 2019-2022 ChainSafe Systems.*"
PAT_SPDX="/*// SPDX-License-Identifier: Apache-2.0, MIT.*"

# Look at enough lines so that we can include multiple copyright holders.
LINES=4

ret=0

# Look for files without headers.
for file in $(git grep --cached -Il '' -- '*.rs'); do
  header=$(head -$LINES "$file")
	if ! echo "$header" | grep -q -P "$PAT_SPDX"; then
		echo "$file was missing header"
		cat $COPYRIGHT_TXT "$file" > temp
		mv temp "$file"
		ret=1
	fi
done

# Look for changes that don't have the new copyright holder.
for file in $(git diff --diff-filter=d --name-only master -- '*.rs'); do
  header=$(head -$LINES "$file")
	if ! echo "$header" | grep -q -P "$PAT_PL"; then
		echo "$file was missing Protocol Labs"
		head -1 $COPYRIGHT_TXT > temp
		cat "$file" >> temp
		mv temp "$file"
		ret=1
	fi
done

exit $ret
