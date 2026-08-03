#!/bin/sh
set -eu

for runtime_directory in \
    /var/lib/trpg/admin \
    /var/lib/trpg/secret-catalog \
    /var/lib/trpg/audit \
    /var/lib/trpg/exports \
    /var/lib/trpg/home \
    /var/lib/trpg/backups \
    /var/lib/trpg/restore-safety-points \
    /var/lib/trpg/model-certification-requests \
    /var/lib/trpg/model-certification-status
do
    install -d -o trpg -g trpg -m 0700 "$runtime_directory"
done
export HOME=/var/lib/trpg/home

if [ "${TRPG_SECRET_MOUNT:-}" = /run/secrets ]; then
    private_secret_mount=/tmp/trpg-mounted-secrets
    install -d -o trpg -g trpg -m 0700 "$private_secret_mount"
    for mounted_secret in /run/secrets/*.v*; do
        [ -e "$mounted_secret" ] || continue
        if [ -L "$mounted_secret" ] || [ ! -f "$mounted_secret" ]; then
            printf 'service-secret-staging error=REGULAR_SECRET_FILE_REQUIRED\n' >&2
            exit 1
        fi
        secret_name=${mounted_secret##*/}
        case "$secret_name" in
            *[!A-Za-z0-9_.-]*)
                printf 'service-secret-staging error=INVALID_SECRET_FILE_NAME\n' >&2
                exit 1
                ;;
        esac
        install -o trpg -g trpg -m 0400 \
            "$mounted_secret" "$private_secret_mount/$secret_name"
    done
    export TRPG_SECRET_MOUNT="$private_secret_mount"
fi

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
