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

codex-plan milestone:
    {{projectctl}} codex plan --milestone {{milestone}}

codex-route mode milestone batch="":
    {{projectctl}} codex route --mode {{mode}} --milestone {{milestone}} --batch '{{batch}}'

codex-check:
    {{projectctl}} codex check

accept-m0:
    {{projectctl}} accept --milestone M0
