#!/bin/sh
set -eu

for runtime_directory in \
    /var/lib/trpg/secret-catalog \
    /var/lib/trpg/audit \
    /var/lib/trpg/exports
do
    install -d -o trpg -g trpg -m 0700 "$runtime_directory"
done

if [ -r /run/secrets/minio_tls_ca_certificate ]; then
    cat /etc/ssl/certs/ca-certificates.crt \
        /run/secrets/minio_tls_ca_certificate \
        > /tmp/trpg-ca-bundle.crt
    chmod 0444 /tmp/trpg-ca-bundle.crt
    export SSL_CERT_FILE=/tmp/trpg-ca-bundle.crt
fi

exec setpriv \
    --reuid=10001 \
    --regid=10001 \
    --init-groups \
    --no-new-privs \
    -- "$@"
