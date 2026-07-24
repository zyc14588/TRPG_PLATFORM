#!/bin/sh
set -eu

for runtime_directory in \
    /var/lib/trpg/secret-catalog \
    /var/lib/trpg/audit \
    /var/lib/trpg/exports
do
    install -d -o trpg -g trpg -m 0700 "$runtime_directory"
done

exec setpriv \
    --reuid=10001 \
    --regid=10001 \
    --init-groups \
    --no-new-privs \
    -- "$@"
