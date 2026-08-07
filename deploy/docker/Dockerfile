# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

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

FROM docker.io/library/alpine:3.23.3@sha256:25109184c71bdad752c8312a8623239686a9a2071e8825f20acb8f2198c3f659

RUN addgroup -S trpg && adduser -S -D -H -G trpg trpg
COPY --from=build --chown=trpg:trpg /out/service /app/service

USER trpg:trpg
WORKDIR /app
ENTRYPOINT ["/app/service"]
CMD ["--help"]
