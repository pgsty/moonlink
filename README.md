# moonlink

[![Website: mooncake.pgsty.com](https://img.shields.io/badge/Website-mooncake.pgsty.com-slategray?style=flat&logo=cilium&logoColor=white)](https://mooncake.pgsty.com/)
[![Docs: moonlink](https://img.shields.io/badge/Docs-moonlink-slategray?style=flat)](https://mooncake.pgsty.com/docs/intro/moonlink/)
[![Version: v0.3.0](https://img.shields.io/badge/Version-v0.3.0-slategray?style=flat)](https://github.com/pgsty/moonlink/releases)
[![License: BSL-1.1 + Exception](https://img.shields.io/badge/License-BSL--1.1%20%2B%20Exception-slategray?style=flat)](https://github.com/pgsty/moonlink/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/pgsty/moonlink?style=flat&logo=github&logoColor=black&color=slategray)](https://star-history.com/#pgsty/moonlink&Date)

**`moonlink`** is an Iceberg-native Rust ingestion engine that mirrors PostgreSQL tables into Apache Iceberg tables via logical replication with sub-second freshness, powering [`pg_mooncake`](https://github.com/pgsty/pg_mooncake) and [`duckdb_mooncake`](https://github.com/pgsty/duckdb_mooncake).

[Website](https://mooncake.pgsty.com/) | [Documentation](https://mooncake.pgsty.com/docs/intro/moonlink/) | [pg_mooncake](https://github.com/pgsty/pg_mooncake) | [duckdb_mooncake](https://github.com/pgsty/duckdb_mooncake) | [Blog](https://mooncake.pgsty.com/blog/) | [PGEXT](https://pgext.cloud/ext/pg_mooncake) | [Releases](https://github.com/pgsty/moonlink/releases) | [Issues](https://github.com/pgsty/moonlink/issues) | [Upstream](https://github.com/Mooncake-Labs/moonlink)

---

## Mooncake Suite

`moonlink` is the real-time storage and ingestion engine of the **Mooncake** family maintained by [PGSTY](https://pgsty.com):

- **[`pg_mooncake`](https://github.com/pgsty/pg_mooncake)** ([Docs](https://mooncake.pgsty.com/docs/intro/) / [Meta](https://pgext.cloud/ext/pg_mooncake)): PostgreSQL 15–19 extension (`v0.3.1`, MIT) providing `mooncake.create_table()`, logical replication workers, and transparent query routing via `pg_duckdb` `1.2.1`.
- **[`moonlink`](https://github.com/pgsty/moonlink)** ([Docs](https://mooncake.pgsty.com/docs/intro/moonlink/)): Rust real-time Iceberg v3 ingestion engine (`v0.3.0`, BSL-1.1 with `pg_mooncake` Linking & Distribution Exception; client crates MIT).
- **[`duckdb_mooncake`](https://github.com/pgsty/duckdb_mooncake)** ([Docs](https://mooncake.pgsty.com/docs/intro/duckdb-mooncake/)): DuckDB `v2.0.1` storage & catalog scanner extension (`v0.0.3`, MIT) merging in-memory Arrow IPC buffers, Roaring deletion vectors, and flushed Parquet files with LSN snapshot consistency.

---

## Overview

Moonlink is the engine behind [`pg_mooncake`](https://github.com/pgsty/pg_mooncake): it mirrors Postgres tables into Iceberg (columnstore) tables via logical replication, **without** separate maintenance and compaction jobs.

It runs inside a Postgres background worker started by pg_mooncake and is controlled over a unix socket (`moonlink_rpc`). Moonlink buffers, caches, and indexes data so Iceberg tables stay read-optimized.

```

             ┌──────────moonlink───────────┐                         
             │  ┌───────────────────────┐  │  ┌───────Iceberg───────┐
             │  │                       │  │  │      obj. store     │
             │  │┌ ─ ─ ─ ─ ┐ ┌ ─ ─ ─ ─ ┐│  │  │┌───────┐ ┌─────────┐│
             │  │                       │  │  ││       │ │         ││
Postgres ───►│  ││  index  │ │  cache  ││  ├──►│ index │ │ parquet ││
             │  │                       │  │  ││       │ │         ││
             │  │└ ─ ─ ─ ─ ┘ └ ─ ─ ─ ─ ┘│  │  │└───────┘ └─────────┘│
             │  │                  nvme │  │  │                     │
             │  └───────────────────────┘  │  └─────────────────────┘
             └─────────────────────────────┘                         
```

## Version v0.3.0

Version `v0.3.0` syncs upstream [`Mooncake-Labs/moonlink` `v0.3-dev`](https://github.com/Mooncake-Labs/moonlink/tree/v0.3-dev) (`dcf999a9`), pinned by [`pg_mooncake v0.3.1`](https://github.com/pgsty/pg_mooncake/releases/tag/v0.3.1) and [`duckdb_mooncake v0.0.3`](https://github.com/pgsty/duckdb_mooncake/releases/tag/v0.0.3):

- **Lean Embedded Footprint & Clean Advisories**: Removes unused Delta Lake, DataFusion, REST/Glue catalog, Kafka, OpenTelemetry, GCS, and standalone WAL code, upgrades Arrow/Parquet to `56`, and clears all open RustSec vulnerability advisories.
- **Hardened CDC, Recovery & Storage**: Guarantees initial copy durability before `create_table` returns, enforces per-table errored state isolation, adds atomic Iceberg metadata writes, disk-full backoff, bounded memory caches, Iceberg file garbage collection (`MOONLINK_ICEBERG_GC`), partition leaf `REPLICA IDENTITY FULL`, and unrelated-WAL forced snapshot completion.
- **Wire-Compatible RPC**: Preserves the exact `moonlink_rpc` and `moonlink_table_metadata` wire format guarded by `tests/wire_golden.rs`.

## Why Moonlink?

Traditional ingestion tools write data and metadata files per update into Iceberg. That's fine for slow-changing data, but on real-time streams it causes:

- **Tiny data files** — frequent commits create thousands of small Parquet files  
- **Metadata explosion** — equality-deletes compound this problem

which leads to:
- **Slow read performance** — query planning overhead scales with file count
- **Manual maintenance** — periodic Spark jobs for compaction/cleanup

Moonlink minimizes write amplification and metadata churn by buffering incoming data, building indexes and caches on NVMe, and committing read-optimized files and deletion vectors to Iceberg.

**Inserts** are buffered and flushed as size-tuned Parquet

```
          ┌───moonlink───┐  ┌────iceberg───┐
          │┌─ ─ ─ ─ ─ ─ ┐│  │┌─ ─ ─ ─ ─ ─ ┐│
raw insert│              │  │              │
────────► ││   Arrow    │├─►││   Parquet  ││
          │              │  │              │
          │└─ ─ ─ ─ ─ ─ ┘│  │└─ ─ ─ ─ ─ ─ ┘│
          └──────────────┘  └──────────────┘
```

**Deletes** are mapped to deletion vectors using an index  built on row positions

```
           ┌───moonlink───┐   ┌────iceberg───┐
           │┌─ ─ ─── ─ ─ ┐│   │┌─ ─ ─ ─ ─ ─ ┐│
raw deletes│              │   │              │
  ────────►││   index    │├──►││  deletion  ││
           │              │   │   vectors    │
           │└─ ─ ─── ─ ─ ┘│   │└─ ─ ─ ─ ─ ─ ┘│
           └──────────────┘   └──────────────┘
```

## Write Path

**PostgreSQL CDC**: tables are initially copied, then kept in sync via logical replication with millisecond-level latency. The source Postgres needs `wal_level = logical`.

Mirroring a table sets `REPLICA IDENTITY FULL` on the source table, so updates carry full old rows, including unchanged TOASTed columns.

## Read Path

Moonlink commits data as **Iceberg v3 tables with deletion vectors** (filesystem catalog), which can be queried from any Iceberg-compatible engine.

For sub-second freshness, readers ask moonlink for the current table state over the unix socket:

1. **Postgres**: with the [`pg_mooncake`](https://github.com/pgsty/pg_mooncake) extension.
2. **DuckDB**: with the [`duckdb_mooncake`](https://github.com/pgsty/duckdb_mooncake) extension (also used by pg_mooncake).

## Embedding

pg_mooncake starts moonlink from its background worker:

```rust
moonlink_service::start_with_config(moonlink_service::ServiceConfig {
    base_path: "pg_mooncake".to_owned(),
})
.await
```

The service listens on `{base_path}/moonlink.sock` and stores tables, caches and its SQLite metadata under `base_path`. Logs go to stdout (the Postgres server log).

The RPC wire format (`src/moonlink_rpc`, and the `ScanTableBegin` payload in `src/moonlink_table_metadata`) is shared with prebuilt duckdb_mooncake binaries that pin `moonlink_rpc` by git, so it is append-only. `tests/wire_golden.rs` in both crates holds byte-level goldens; CI runs them in the `wire_format` job.

## Development

```bash
cargo build
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --lib --workspace              # unit tests
# Wire-format goldens live under tests/ (not covered by --lib).
cargo test -p moonlink_rpc -p moonlink_table_metadata --tests
cargo test -p moonlink --test scan_payload_order

# Postgres-backed tests need a Postgres with wal_level=logical.
# They share one replication slot, so run them with cargo test (one process), not nextest.
export DATABASE_URL=postgresql://<user>:<password>@localhost:5432/postgres
cargo test -p moonlink_connectors --lib --features connector-pg
cargo test -p moonlink_backend --test '*'
cargo test -p moonlink_service --lib --features postgres-integration
```

## Waiting for replication

A query at an LSN (and `mooncake.create_snapshot`) first waits until replication reaches that LSN. The Postgres backend cannot be cancelled while it waits, so the wait is bounded: after 30 seconds the query fails with a retryable error that names the requested and replicated LSNs. If your replication is often slower than that, raise the bound by setting `MOONLINK_LSN_WAIT_TIMEOUT_SECS` (whole seconds, at most one day) in the environment Postgres is started with.

## Durability and Recovery

> **Before upgrading from pg_mooncake 0.2 (Moonlink with its own WAL), snapshot every mirror, or re-create every mirror after the upgrade.** That version acknowledged changes to Postgres once they were in its `_wal` directory. The new version does not read that directory, and Postgres does not send those changes again, so a mirror silently misses every change that had not reached Iceberg yet. Before upgrading:
> 1. Stop writes to the mirrored tables.
> 2. Still on the old version, run `CALL mooncake.create_snapshot('<mirror>');` for every mirror.
> 3. Upgrade.
>
> If you upgraded without these steps, drop and re-create every mirror. Until the old WAL directory, `$PGDATA/pg_mooncake/_wal`, is deleted, Moonlink logs a warning at startup naming the mirrors that have files in it. Delete it once every mirror is snapshotted or re-created; dropping a mirror no longer removes its part of it.

Moonlink acknowledges the Postgres replication slot only up to changes that every mirror has persisted in Iceberg. After a restart or a dropped replication connection, Postgres re-sends everything after that point, and Moonlink skips what it had already applied.

**WAL retention on the source.** Postgres keeps WAL until every mirror has persisted the changes in it:
- A mirror that is being written persists at least every 5 minutes or so (a forced snapshot every 300 s), sooner when enough rows accumulate.
- A mirror with no new changes, including an empty one, does not hold WAL back.
- A mirror in the errored state does not hold WAL back either, even when it is the only mirror of its database. It no longer follows its source; drop and re-create it, which copies the table afresh. Moonlink logs a warning naming the errored mirrors about once a minute until then.
- After a restart, a mirror that cannot be restored yet (for example, its source is unreachable) holds its database's slot, so that it can replay what it missed; meanwhile the slots of databases whose mirrors are all errored do not advance either. Moonlink retries it and logs a warning naming it about once a minute. After an hour it is errored and its slot is released.

Watch the retained WAL with:
```sql
SELECT slot_name, pg_size_pretty(pg_wal_lsn_diff(pg_current_wal_lsn(), confirmed_flush_lsn))
FROM pg_replication_slots WHERE slot_name LIKE 'moonlink_slot%';
```
To cap it, set `max_slot_wal_keep_size` on the source. If Moonlink then falls that far behind, Postgres invalidates the slot and every mirror has to be re-created.

**After a crash**, if a mirror looks wrong, drop it and re-create it (`CALL mooncake.create_table(...)` again).

Data files are made durable before the Iceberg commit that references them, but deletion vector and index puffin files are not fsynced into their directory, so a power loss right after a commit can leave a mirror that fails to load and must be re-created.

On the first start after upgrading, every existing mirror rebuilds its lookup indices from its data files (earlier versions hashed rows with a seed that changed on every restart, so updates and deletes of already-flushed rows could fail after a restart). This is a one-time read of each table's key columns, logged as a warning per table. If a table's indices cannot be rebuilt, that mirror is marked out of sync and must be dropped and re-created; other mirrors are not affected.

This version also deletes Iceberg files that neither the current version nor the one before it references (old snapshots, metadata files, manifests, deletion vectors, files replaced by compaction), which earlier versions kept forever. A replaced data or deletion-vector file is kept until no running scan can still read it. On the first start after upgrading, each table's leftover files are removed when the table loads. Set `MOONLINK_ICEBERG_GC=dry-run` to only log what would be deleted (each file once, as a count and size per commit; paths at debug level), or `MOONLINK_ICEBERG_GC=off` to keep every file. Dry-run is meant for a short look: it keeps listing and remembering every superseded file, so its cost grows the longer it runs; use `off` to keep files for good.

If the disk fills up, mirrors stop advancing instead of failing: flushes and Iceberg commits are retried (at most every minute) until space is available, and files of failed attempts are removed. Changes that wait for disk space are held in memory, so a mirror is marked out of sync once a flush cannot be written for 30 minutes, or sooner, once more than 512 MiB of changes are waiting, counted over all mirrors together (busy mirrors on a full disk would otherwise grow the Postgres background worker until it is killed for memory); then the mirror holding the most of them is marked out of sync, and the others keep waiting. A mirror that is marked out of sync or dropped releases its waiting changes at once. Postgres keeps the unconfirmed changes in `pg_wal` meanwhile.

## What mirrors do not follow

A mirror that can no longer follow its source table enters the errored state: queries on it fail with a message that names the reason, every other mirror keeps replicating, and the remedy is always to drop the mirror and then create it again (`CALL mooncake.create_table(...)`); creating a new mirror of the same table while the errored one still exists is refused. An errored mirror stays errored across restarts.

- **Schema changes.** Any change to the source table's columns (add, drop, rename, change type, drop and re-add, add a generated column) or setting its replica identity to anything but `FULL` errors the mirror, also when made while moonlink is stopped.
- **Not detected:** `ALTER TABLE ... ALTER COLUMN ... TYPE <same type> USING <expression>` rewrites every row without replicating the change, and switching a partition to `UNLOGGED` and back to `LOGGED` within one transaction or within about 5 seconds (for example to load it faster) loses the rows written while it was unlogged. Re-create the mirror after either.
- **`TRUNCATE`** of the source table errors the mirror.
- **Inheritance.** A table that gains an inheritance child (`CREATE TABLE ... INHERITS`) errors its mirror: the child's rows are part of the parent in Postgres queries but are not replicated with it.
- **Rolled-back savepoints in large transactions.** In a transaction large enough for Postgres to stream it before it commits (more changes than `logical_decoding_work_mem`, 64MB by default), `ROLLBACK TO SAVEPOINT` (or a PL/pgSQL exception handler that catches an error after changes) errors the mirrors of the tables the rolled-back part changed, once the transaction commits. Released savepoints, and savepoints in smaller transactions, replicate normally.
- **Partitioned tables** need a primary key. A new partition is followed if it is still empty when moonlink first sees it, so create partitions ahead of use. A partition that already holds rows when it is added (attached, or created and filled at once), and detaching (even to attach it again) or dropping any partition (including routine retention), error the mirror. An `UPDATE` or `DELETE` in a new partition within about 5 seconds of its creation can error the mirror too. Restoring the source from a dump or upgrading it with `pg_upgrade` rewrites the partition catalog and errors these mirrors; re-create them afterwards. Postgres does not replicate changes of unlogged partitions: creating one, or setting a partition `UNLOGGED`, errors the mirror once moonlink sees it (it checks about every 5 seconds), and a table with one is refused. A partitioned table and one of its partitions cannot both be mirrored, since Postgres then publishes the partition's changes only as the partitioned table's: the second `create_table` is refused, and a mirrored table attached as a partition of a mirrored table errors its own mirror (not detected: attaching it and detaching it again within about 5 seconds, or while moonlink is stopped, loses the changes made while it was attached). A mirror whose table leaves the publication `moonlink_pub` (for example through `ALTER PUBLICATION ... DROP TABLE`) is errored too.
- **Refused at `create_table`:** tables with generated columns, inheritance parents, partitioned tables without a primary key or with an unlogged partition, and column types moonlink cannot store exactly (for example `interval`, `timetz`, multi-dimensional arrays, `numeric` with a precision above 38 or a scale outside 0..precision, and domains over such types); the error names the column and its type. Domains over other types are stored as their base type. Enums are stored as their labels, so in the mirror they sort and compare as text, not in the enum's declared order; renaming an enum value (`ALTER TYPE ... RENAME VALUE`) errors the mirror once moonlink sees it (it checks about every 5 seconds), adding one does not. A value renamed and renamed back between two checks is not detected.
- **Values a mirror cannot store** (for example `numeric` `NaN` or `Infinity`, more fractional digits than an unconstrained `numeric` keeps, `infinity` and `-infinity` dates and timestamps, years beyond 262142) error the mirror instead of being stored differently.
- **After upgrading to this version,** mirrors of tables without a primary key that have `json`, `jsonb`, `char(n)`, `"char"`, unconstrained `numeric`, array or composite columns are errored at the first start, because they were stored differently before. Re-create them.

A source that cannot be reached when moonlink starts (restarting, refusing connections) is not an error: its mirrors report that they are not available yet and are restored automatically once it is back. Postgres retains WAL for that database meanwhile. If it is still unreachable an hour after moonlink started, its mirrors enter the errored state (drop and re-create them), and the slot is no longer held for them.

---

## Documentation

Full documentation is available at **[mooncake.pgsty.com/docs/intro/moonlink](https://mooncake.pgsty.com/docs/intro/moonlink/)**:

- [Moonlink Ingestion Engine Overview](https://mooncake.pgsty.com/docs/intro/moonlink/)
- [Mooncake Architecture](https://mooncake.pgsty.com/docs/intro/architecture/)
- [duckdb_mooncake Scanner (`v0.0.3`)](https://mooncake.pgsty.com/docs/intro/duckdb-mooncake/)
- [Iceberg & Object Storage Reference](https://mooncake.pgsty.com/docs/reference/iceberg/)
- [Licensing & BSL-1.1 Exception Details](https://mooncake.pgsty.com/docs/about/license/)

---

## License

- **Core Engine Crates** (`moonlink`, `moonlink_backend`, `moonlink_connectors`, `moonlink_metadata_store`, `moonlink_service`): Licensed under the **Business Source License 1.1 ([`BSL-1.1`](LICENSE))** (Change Date: `2029-06-03`, Change License: `Apache-2.0`).
  - **`PG_MOONCAKE LINKING & DISTRIBUTION EXCEPTION`**: Explicitly grants the right to statically or dynamically link `moonlink` object code with [`pg_mooncake`](https://github.com/pgsty/pg_mooncake) and distribute the combined executable under the MIT License applicable to `pg_mooncake`.
- **Client & Wire-Protocol Crates** (`moonlink_rpc`, `moonlink_proto`, `moonlink_error`, `moonlink_table_metadata`): Licensed under the **MIT License**.

- Copyright (c) 2025 Mooncake Labs, Inc.
- Copyright (c) 2026 PGSTY PTE. LTD.
- Maintained by [Ruohang Feng](mailto:rh@vonng.com) @ [PGSTY](https://pgsty.com)
