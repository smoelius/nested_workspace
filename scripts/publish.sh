#! /bin/bash

# set -x
set -euo pipefail

for X in nested_workspace; do
    pushd "$X"
    cargo publish "$@"
    popd
done
