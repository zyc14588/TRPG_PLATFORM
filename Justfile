# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

projectctl := "go run ./cmd/projectctl"

bootstrap:
    {{projectctl}} bootstrap

generate:
    {{projectctl}} docs generate

check:
    {{projectctl}} check

build:
    {{projectctl}} build

test:
    {{projectctl}} test

ci:
    {{projectctl}} ci

docs-check:
    {{projectctl}} docs check

license-check:
    {{projectctl}} license check

codex-plan:
    {{projectctl}} codex plan

codex-route mode="PLAN":
    {{projectctl}} codex route --mode {{mode}}

codex-check:
    {{projectctl}} codex check

accept-m0:
    {{projectctl}} accept --milestone M0
