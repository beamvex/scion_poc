# scion_router_proto

SCION router prototype in Rust.

This is a **control-plane oriented** prototype with a very small dataplane to validate plumbing.

- Dataplane: UDP listener that forwards packets based on a routing table.
- Control plane: HTTP API to add/delete routes.

## Run

```bash
cargo run -- --http-listen 127.0.0.1:3000 --data-listen 127.0.0.1:4000
```

Environment:

- `RUST_LOG=info` (or `debug`) for logs.

## Control-plane API

- `GET /health`
- `GET /routes`
- `POST /routes/:dst` with JSON body `{ "next_hop": "127.0.0.1:5000" }`
- `DELETE /routes/:dst`

- `GET /ifaces`
- `POST /ifaces/:ifid` with JSON body `{ "next_hop": "127.0.0.1:5000" }`
- `DELETE /ifaces/:ifid`

Example:

```bash
curl -X POST http://127.0.0.1:3000/routes/ia1-ff00_0_110 \
  -H 'content-type: application/json' \
  -d '{"next_hop":"127.0.0.1:5000"}'

curl -X POST http://127.0.0.1:3000/ifaces/121 \
  -H 'content-type: application/json' \
  -d '{"next_hop":"127.0.0.1:5000"}'

curl http://127.0.0.1:3000/routes
```

## Dataplane

The UDP listener expects **raw SCION packets** (bytes). For now, the dataplane only parses the SCION common+address header far enough to extract the **destination ISD-AS** and does a route lookup based on that.

Forwarding uses **PathType=SCION (1)** hop fields:

- It extracts the **current hop field** (via PathMeta `CurrHF`) and reads `ConsEgress`.
- It looks up the underlay next hop via `/ifaces/:ifid`.
- It increments `CurrHF` in PathMeta and forwards the modified packet.

Routes are keyed by destination IA string in the form:

```text
<isd>-<as0>:<as1>:<as2>
```

Example route key:

```text
1-ff00:0:110
```

## Notes / Next steps

- Extend parsing beyond the destination IA and implement hop-field/path processing.
- Add proper underlay adjacency (per-interface sockets), metrics, and policy.
- Add route distribution / path selection logic (SCIOND/daemon interop, beaconing) as needed for your prototype.
