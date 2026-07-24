FROM rust:1.96-bookworm AS builder

WORKDIR /workspace
COPY . .
RUN cargo build --locked --release \
    -p api-server \
    -p realtime-server \
    -p agent-worker \
    -p admin-server \
    -p migration-runner

FROM node:24-alpine AS web-builder

WORKDIR /workspace/apps/web
COPY apps/web .
RUN node scripts/build.mjs

FROM nginx:1.27-alpine AS web-runtime

COPY --from=web-builder /workspace/apps/web/dist /usr/share/nginx/html

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl libssl3 util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 trpg \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin trpg

COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server
COPY --from=builder /workspace/target/release/realtime-server /usr/local/bin/realtime-server
COPY --from=builder /workspace/target/release/agent-worker /usr/local/bin/agent-worker
COPY --from=builder /workspace/target/release/admin-server /usr/local/bin/admin-server
COPY --from=builder /workspace/target/release/migration-runner /usr/local/bin/migration-runner
COPY config/container/trpg-entrypoint.sh /usr/local/bin/trpg-entrypoint
COPY config/plugins/registry.json /etc/trpg/plugins/registry.json

RUN chmod 0555 /usr/local/bin/api-server \
        /usr/local/bin/realtime-server \
        /usr/local/bin/agent-worker \
        /usr/local/bin/admin-server \
        /usr/local/bin/migration-runner \
        /usr/local/bin/trpg-entrypoint \
    && chmod 0444 /etc/trpg/plugins/registry.json

ENTRYPOINT ["/usr/local/bin/trpg-entrypoint"]
