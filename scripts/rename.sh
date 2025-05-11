#! /bin/bash

# set -x
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "$0: expect two arguments: 'from' and 'to'" >&2
    exit 1
fi

FROM="$1"
TO="$2"

for X in *; do
    Y="$(echo "$X" | sed "s/$FROM/$TO/")"
    mv "$X" "$Y" || true
done
