FROM rust:1.96-bookworm@sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663 AS builder

WORKDIR /workspace
COPY . .
RUN cargo build --locked --release -p api-server
RUN cargo build --locked --release -p realtime-server
RUN cargo build --locked --release -p agent-worker
RUN cargo build --locked --release -p admin-server
RUN cargo build --locked --release -p migration-runner

FROM pgvector/pgvector@sha256:1d533553fefe4f12e5d80c7b80622ba0c382abb5758856f52983d8789179f0fb AS postgres-tools

FROM node:24-alpine@sha256:a0b9bf06e4e6193cf7a0f58816cc935ff8c2a908f81e6f1a95432d679c54fbfd AS web-builder

WORKDIR /workspace/apps/web
COPY apps/web .
RUN node scripts/build.mjs

FROM nginx:1.27-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10 AS web-runtime

COPY --from=web-builder /workspace/apps/web/dist /usr/share/nginx/html

FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl libssl3 postgresql-client util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 trpg \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin trpg

COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server
COPY --from=builder /workspace/target/release/realtime-server /usr/local/bin/realtime-server
COPY --from=builder /workspace/target/release/agent-worker /usr/local/bin/agent-worker
COPY --from=builder /workspace/target/release/admin-server /usr/local/bin/admin-server
COPY --from=builder /workspace/target/release/migration-runner /usr/local/bin/migration-runner
COPY --from=postgres-tools /usr/lib/postgresql/16/bin/pg_dump /usr/local/libexec/trpg/pg_dump
COPY --from=postgres-tools /usr/lib/postgresql/16/bin/pg_restore /usr/local/libexec/trpg/pg_restore
COPY --from=postgres-tools /usr/lib/postgresql/16/bin/psql /usr/local/libexec/trpg/psql
COPY config/container/trpg-entrypoint.sh /usr/local/bin/trpg-entrypoint
COPY config/plugins/registry.json /etc/trpg/plugins/registry.json

RUN chmod 0555 /usr/local/bin/api-server \
        /usr/local/bin/realtime-server \
        /usr/local/bin/agent-worker \
        /usr/local/bin/admin-server \
        /usr/local/bin/migration-runner \
        /usr/local/bin/trpg-entrypoint \
        /usr/local/libexec/trpg/pg_dump \
        /usr/local/libexec/trpg/pg_restore \
        /usr/local/libexec/trpg/psql \
    && chmod 0444 /etc/trpg/plugins/registry.json

ENTRYPOINT ["/usr/local/bin/trpg-entrypoint"]
