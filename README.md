# moonlink

[![Website: mooncake.pgsty.com](https://img.shields.io/badge/Website-mooncake.pgsty.com-slategray?style=flat&logo=cilium&logoColor=white)](https://mooncake.pgsty.com/)
[![Docs: moonlink](https://img.shields.io/badge/Docs-moonlink-slategray?style=flat)](https://mooncake.pgsty.com/docs/intro/moonlink/)
[![Version: v0.0.1](https://img.shields.io/badge/Version-v0.0.1-slategray?style=flat)](https://github.com/pgsty/moonlink/releases)
[![License: BSL-1.1 + Exception](https://img.shields.io/badge/License-BSL--1.1%20%2B%20Exception-slategray?style=flat)](https://github.com/pgsty/moonlink/blob/main/LICENSE)
[![GitHub Stars](https://img.shields.io/github/stars/pgsty/moonlink?style=flat&logo=github&logoColor=black&color=slategray)](https://star-history.com/#pgsty/moonlink&Date)

**`moonlink`** is an Iceberg-native Rust ingestion engine that brings streaming inserts, updates, and deletes to Apache Iceberg tables with sub-second freshness, powering [`pg_mooncake`](https://github.com/pgsty/pg_mooncake) and [`duckdb_mooncake`](https://github.com/pgsty/duckdb_mooncake).

[Website](https://mooncake.pgsty.com/) | [Documentation](https://mooncake.pgsty.com/docs/intro/moonlink/) | [pg_mooncake](https://github.com/pgsty/pg_mooncake) | [duckdb_mooncake](https://github.com/pgsty/duckdb_mooncake) | [Blog](https://mooncake.pgsty.com/blog/) | [PGEXT](https://pgext.cloud/ext/pg_mooncake) | [Releases](https://github.com/pgsty/moonlink/releases) | [Issues](https://github.com/pgsty/moonlink/issues) | [Upstream](https://github.com/Mooncake-Labs/moonlink)

---

## Mooncake Suite

`moonlink` is the real-time storage and ingestion engine of the **Mooncake** family maintained by [PGSTY](https://pgsty.com):

- **[`pg_mooncake`](https://github.com/pgsty/pg_mooncake)** ([Docs](https://mooncake.pgsty.com/docs/intro/) / [Meta](https://pgext.cloud/ext/pg_mooncake)): PostgreSQL extension (`v0.3.0`, MIT) providing `mooncake.create_table()`, logical replication workers, and transparent query routing via `pg_duckdb` `1.2.0`.
- **[`moonlink`](https://github.com/pgsty/moonlink)** ([Docs](https://mooncake.pgsty.com/docs/intro/moonlink/)): Rust real-time Iceberg v3 ingestion engine (`v0.0.1`, BSL-1.1 with `pg_mooncake` Linking & Distribution Exception; client crates MIT).
- **[`duckdb_mooncake`](https://github.com/pgsty/duckdb_mooncake)** ([Docs](https://mooncake.pgsty.com/docs/intro/duckdb-mooncake/)): DuckDB `v2.0.0` storage & catalog scanner extension (`v0.0.2`, MIT) merging in-memory Arrow IPC buffers, Roaring deletion vectors, and flushed Parquet files with LSN snapshot consistency.

---

## Overview

Moonlink is an Iceberg-native ingestion engine bringing streaming inserts and upserts to your lakehouse.

Ingest Postgres CDC, event streams (Kafka), and OTEL into Iceberg **without** complex maintenance and compaction. 

Moonlink buffers, caches, and indexes data so Iceberg tables stay read-optimized.

```

             ┌──────────moonlink───────────┐                         
             │  ┌───────────────────────┐  │  ┌───────Iceberg───────┐
             │  │                       │  │  │      obj. store     │
Postgres ───►│  │┌ ─ ─ ─ ─ ┐ ┌ ─ ─ ─ ─ ┐│  │  │┌───────┐ ┌─────────┐│
             │  │                       │  │  ││       │ │         ││
Kafka    ───►│  ││  index  │ │  cache  ││  ├──►│ index │ │ parquet ││
             │  │                       │  │  ││       │ │         ││
Events   ───►│  │└ ─ ─ ─ ─ ┘ └ ─ ─ ─ ─ ┘│  │  │└───────┘ └─────────┘│
             │  │                  nvme │  │  │                     │
             │  └───────────────────────┘  │  └─────────────────────┘
             └─────────────────────────────┘                         
```

## Version v0.0.1

Version `v0.0.1` is the first PGSTY release, pinned by [`pg_mooncake v0.3.0`](https://github.com/pgsty/pg_mooncake/releases/tag/v0.3.0) and [`duckdb_mooncake v0.0.2`](https://github.com/pgsty/duckdb_mooncake/releases/tag/v0.0.2):

- **Non-Blocking Snapshot LSN Horizon**: Fixes `notify_lsn()` so `create_scan_snapshot` wakes immediately when the database's WAL flush LSN advances on unmirrored tables, eliminating the 30-second timeout stall on idle mirrored tables.
- **Partitioned Table Replication**: Sets `REPLICA IDENTITY FULL` on existing leaf partitions before creating the logical replication snapshot so partitioned updates and deletes replicate cleanly.
- **Security Hardening**: Upgrades `lru` to `0.18.2` (`RUSTSEC-2026-0253`) and refreshes lockfile dependencies (`bytes`, `openssl`, `quinn-proto`, `tar`, `wasmtime`) while keeping Apache Arrow/Parquet `55`, DataFusion `49`, OpenDAL `0.54`, and `deltalake` `0.28` storage APIs compatible.

See the [Moonlink Architecture Guide](https://mooncake.pgsty.com/docs/intro/moonlink/) and [pg_mooncake v0.3.0 Release Notes](https://mooncake.pgsty.com/blog/release/pg-mooncake-0.3.0/) for details.

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

## Write Paths

Moonlink supports multiple input sources for ingest:

1. **PostgreSQL CDC** — ingest via logical replication with millisecond-level latency  
2. **REST API** — simple HTTP endpoint for direct event ingestion  
3. **Kafka** — sink support coming soon  
4. **OTEL** — sink support on the roadmap  

## Read Path

Moonlink commits data as **Iceberg v3 tables with deletion vectors**. These tables can be queried from any Iceberg-compatible engine.

**Engines**
1. **DuckDB**   
2. **Apache Spark**
3. **Postgres** with `pg_duckdb` or  `pg_mooncake`

**Catalogs**
1. **AWS Glue** — coming soon  
2. **Unity Catalog** — coming soon  

---

### Real-Time Reads (<s freshness)

For workloads requiring sub-second visibility into new data, Moonlink supports real-time querying:

1. **DuckDB** — with the [`duckdb_mooncake`](https://github.com/pgsty/duckdb_mooncake) extension.
2. **Postgres** — with the [`pg_mooncake`](https://github.com/pgsty/pg_mooncake) extension.
3. **DataFusion** — with [`moonlink_datafusion`](https://github.com/pgsty/moonlink/tree/main/src/moonlink_datafusion).

 
## Quick Start

### 1. Clone & Build

Clone the repository and build the service binary:

```bash
git clone https://github.com/pgsty/moonlink.git
cd moonlink
cargo build --release --bin moonlink_service
```

### 2. Start the Moonlink Service

Start the Moonlink service, which will store data in the `./data` directory:

```bash
./target/release/moonlink_service ./data
```

### 3. Verify Service Health

Check that the service is running properly:

```bash
curl http://localhost:3030/health
```

### 4. Create a Table

Create a table with a defined schema. Here's an example creating a `users` table:

```bash
curl -X POST http://localhost:3030/tables/users \
  -H "Content-Type: application/json" \
  -d '{
    "database": "my_database",
    "table": "users",
    "schema": [
      {"name": "id", "data_type": "int32", "nullable": false},
      {"name": "name", "data_type": "string", "nullable": false},
      {"name": "email", "data_type": "string", "nullable": true},
      {"name": "age", "data_type": "int32", "nullable": true},
      {"name": "created_at", "data_type": "date32", "nullable": true}
    ],
    "table_config": {"mooncake": {"append_only": true}}
  }'
```

### 5. Insert Data

Insert data into the created table:

```bash
curl -X POST http://localhost:3030/ingest/users \
  -H "Content-Type: application/json" \
  -d '{
    "operation": "insert",
    "request_mode": "async",
    "data": {
      "id": 1,
      "name": "Alice Johnson",
      "email": "alice@example.com",
      "age": 30,
      "created_at": "2024-01-01"
    }
  }'
```

---

## Documentation

Full documentation is available at **[mooncake.pgsty.com/docs/intro/moonlink](https://mooncake.pgsty.com/docs/intro/moonlink/)**:

- [Moonlink Ingestion Engine Overview](https://mooncake.pgsty.com/docs/intro/moonlink/)
- [Mooncake Architecture](https://mooncake.pgsty.com/docs/intro/architecture/)
- [duckdb_mooncake Scanner (`v0.0.2`)](https://mooncake.pgsty.com/docs/intro/duckdb-mooncake/)
- [Iceberg & Object Storage Reference](https://mooncake.pgsty.com/docs/reference/iceberg/)
- [Licensing & BSL-1.1 Exception Details](https://mooncake.pgsty.com/docs/about/license/)

---

## License

- **Core Engine Crates** (`moonlink`, `moonlink_backend`, `moonlink_connectors`, `moonlink_datafusion`, `moonlink_metadata_store`, `moonlink_service`): Licensed under the **Business Source License 1.1 ([`BSL-1.1`](LICENSE))** (Change Date: `2029-06-03`, Change License: `Apache-2.0`).
  - **`PG_MOONCAKE LINKING & DISTRIBUTION EXCEPTION`**: Explicitly grants the right to statically or dynamically link `moonlink` object code with [`pg_mooncake`](https://github.com/pgsty/pg_mooncake) and distribute the combined executable under the MIT License applicable to `pg_mooncake`.
- **Client & Wire-Protocol Crates** (`moonlink_rpc`, `moonlink_proto`, `moonlink_error`, `moonlink_table_metadata`): Licensed under the **MIT License**.

- Copyright (c) 2025 Mooncake Labs, Inc.
- Copyright (c) 2026 PGSTY PTE. LTD.
- Maintained by [Ruohang Feng](mailto:rh@vonng.com) @ [PGSTY](https://pgsty.com)