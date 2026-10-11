# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

# The Linux M2 lifecycle assembles these exact source-built binaries, current
# schemas and the unchanged web build in its owned context. No runtime base or
# generic shell is added; every Compose entry selects a real service binary.
FROM scratch AS m2
ARG M2_UID=65532
ARG M2_GID=65532
ARG M2_PROJECT
ARG M2_SOURCE
LABEL trpg.m2.project=${M2_PROJECT} trpg.m2.source=${M2_SOURCE}
COPY --chown=${M2_UID}:${M2_GID} rootfs/ /
USER ${M2_UID}:${M2_GID}
WORKDIR /app
ENTRYPOINT ["/app/platformd"]

FROM docker.io/library/golang:1.26.5-alpine3.23@sha256:622e56dbc11a8cfe87cafa2331e9a201877271cbff918af53d3be315f3da88cc AS build

ARG BINARY
WORKDIR /src

COPY go.mod go.sum ./
RUN go mod download

COPY cmd ./cmd
COPY internal ./internal
COPY pkg ./pkg

RUN test -n "${BINARY}" && \
    CGO_ENABLED=0 GOOS=linux go build -trimpath \
      -ldflags="-s -w -buildid=" \
      -o /out/service "./cmd/${BINARY}"


# The M1 harness supplies freshly source-built static binaries in its own
# temporary context. This test target never pulls a runtime base image.
FROM scratch AS m1-fixture
COPY rootfs/ /
USER 65532:65532
WORKDIR /app
ENTRYPOINT ["/app/platformd"]
CMD ["--help"]


FROM scratch

COPY --from=build --chown=65532:65532 /out/service /app/service

USER 65532:65532
WORKDIR /app
ENTRYPOINT ["/app/service"]
CMD ["--help"]
