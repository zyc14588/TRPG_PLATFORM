<!-- SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 -->

# M0 development Compose

`deploy/compose.yaml` builds and starts the `platformd`, `workerd`, and `lua-runner` process shells. Each container is read-only, drops Linux capabilities, runs as a non-root user, and is checked through the deterministic `health` command.

This is not a production deployment. M0 deliberately contains no reverse proxy, public port, database, object storage, migration, game package, session runtime, or AI provider.

The supported entrypoint will be:

```text
just bootstrap
just build
just test
```

Direct Compose inspection is read-only:

```text
docker compose -f deploy/compose.yaml config
```
