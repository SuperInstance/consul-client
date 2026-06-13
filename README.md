# Consul Client — Service Discovery and Distributed KV Store

**A Consul client** is an HTTP client for HashiCorp Consul — a service mesh and distributed key-value store used for service discovery, configuration, and health checking in microservice architectures. This crate provides typed methods for service registration, health-based lookup, and KV operations against the Consul HTTP API (v1).

## Why It Matters

Consul is one of the three pillars of HashiCorp's service infrastructure stack (Consul + Vault + Nomad), widely deployed in production at organizations running mixed workloads on Kubernetes, VMs, and bare metal. It solves three critical problems: (1) **service discovery** — "where is service X running right now?", (2) **health checking** — "is service X actually healthy, not just registered?", and (3) **distributed configuration** — KV storage with strong consistency via Raft consensus. A typed Rust client eliminates boilerplate HTTP plumbing and provides compile-time guarantees about request/response shapes.

## How It Works

### Architecture

```
┌──────────────┐     HTTP/JSON      ┌──────────────┐
│ ConsulClient │ ──────────────────► │  Consul Agent │
│  (this crate)│ ◄────────────────── │  (port 8500)  │
└──────────────┘                     └──────┬───────┘
                                            │ Raft gossip
                                     ┌──────▼───────┐
                                     │ Consul Cluster│
                                     │  (3–5 nodes)  │
                                     └──────────────┘
```

### Service Discovery

Services register with the local Consul agent:

```
PUT /v1/agent/service/register
{ "ServiceID": "web-1", "ServiceName": "web", "ServiceAddress": "10.0.0.5", "ServicePort": 8080 }
```

Clients query healthy instances:

```
GET /v1/health/service/web → [ { "Service": { ... } }, ... ]
```

Only passing instances are returned (Consul excludes failing health checks).

### Key-Value Store

Consul KV is a hierarchical key-value store with Raft-based strong consistency:

| Operation | Method | Path |
|---|---|---|
| Read | `GET` | `/v1/kv/{key}` |
| Write | `PUT` | `/v1/kv/{key}` |
| Delete | `DELETE` | `/v1/kv/{key}` |

Each key has a `ModifyIndex` — a monotonically increasing revision number that enables watch patterns and compare-and-set (CAS) operations.

### Authentication

Optional ACL tokens are sent via the `X-Consul-Token` header for multi-tenant Consul deployments.

**Complexity**: All operations are `O(1)` network round-trips. The client uses `reqwest::Client` with connection pooling, so amortized cost per call is a single HTTP request over a keep-alive connection.

## Quick Start

```rust
use consul_client::{ConsulClient, ConsulConfig, ServiceEntry};

let config = ConsulConfig {
    address: "http://127.0.0.1:8500".into(),
    token: None,
    timeout: std::time::Duration::from_secs(5),
    datacenter: Some("dc1".into()),
};

let client = ConsulClient::new(config)?;

// Register a service
client.register_service(&ServiceEntry {
    service_id: "web-1".into(),
    service_name: "webserver".into(),
    address: "10.0.0.5".into(),
    port: 8080,
    tags: vec!["http".into()],
}).await?;

// Discover healthy instances
let instances = client.get_service("webserver").await?;
for svc in &instances {
    println!("{} at {}:{}", svc.service_id, svc.address, svc.port);
}

// KV operations
client.kv_put("config/timeout", "30").await?;
let pair = client.kv_get("config/timeout").await?;
println!("{} = {}", pair.Key, pair.Value.unwrap_or_default());
```

## API

| Method | Description |
|---|---|
| `ConsulClient::new(config)` | Create client with connection pooling + optional ACL token. |
| `register_service(entry)` | Register a service with the local agent. |
| `deregister_service(id)` | Remove a service registration. |
| `get_service(name)` | Query healthy instances → `Vec<ServiceEntry>`. |
| `kv_get(key)` | Read a key → `KvPair` (with `ModifyIndex`). |
| `kv_put(key, value)` | Write a key → `bool`. |
| `kv_delete(key)` | Delete a key → `bool`. |
| `health()` | Check if Consul has an elected leader. |

## Architecture Notes

Consul integration serves the γ (generation/coordination) side of γ + η = C in SuperInstance. It provides the service discovery fabric that lets fleet instances find each other dynamically, and the distributed KV store for coordination state. See [SuperInstance Architecture](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md).

## References

1. HashiCorp Consul Documentation. <https://developer.hashicorp.com/consul/docs>
2. Ongaro, D. & Ousterhout, J. (2014). *In Search of an Understandable Consensus Algorithm (Raft)*. USENIX ATC. — Consul's consensus protocol.
3. HashiCorp. *Consul HTTP API*. <https://developer.hashicorp.com/consul/api-docs>

## License

MIT
