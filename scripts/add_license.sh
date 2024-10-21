#!/bin/bash
#
# Checks if the source code contains required license and adds it if necessary.
# Returns 1 if there was a missing license, 0 otherwise.
YEAR="$(date '+%Y')"
ret=0
set -e -x

# Look for files without headers.
while read -r file; do
    echo "$file was missing header"
    sed -i "$file" -f - <<EOF
1i\
// SPDX-License-Identifier: Apache-2.0, MIT'
EOF
    ret=1
done < <(git grep -IL "^// SPDX-License-Identifier:" -- '*.rs')

while read -r file; do
    if grep -q "^// Copyright \([0-9]\+-\)\?${YEAR} Filecoin Core Devs" "$file"; then
        continue
    fi
    ret=1
    if grep -q "^// Copyright [0-9]\+\(-[0-9]\+\)\? Filecoin Core Devs" "$file"; then
        # Update the copyright line if available.
        script="s|^// Copyright \([0-9]\+\)\(-[0-9]\+\)\? Filecoin Core Devs|// Copyright \1-${YEAR} Filecoin Core Devs|"
    else
        # Otherwise, add the new one _AFTER_ the SDPX header.
        script="/^\/\/ SPDX-License-Identifier: /a\
// Copyright ${YEAR} Filecoin Core Devs
"
    fi
    sed -i "$file" -e "$script"
done < <(git diff --diff-filter=DM --name-only --merge-base master -- '*.rs')

exit $ret
