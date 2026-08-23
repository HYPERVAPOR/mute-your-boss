# Toolchain Version Lock

This document records the required toolchain versions for building Mute-your-boss.

## Required Tools

| Tool | Version | Where Locked |
|------|---------|--------------|
| Rust | `1.92.0` | `rust-toolchain.toml` |
| Go | `1.26.1` | `gateway/go.mod` (`toolchain` directive) |
| protoc | `27.3` | `Makefile` (`PROTOC_VERSION`) |
| protoc-gen-go | `v1.34.2` | `Makefile` (`PROTOC_GEN_GO_VERSION`) |
| protoc-gen-go-grpc | `v1.5.1` | `Makefile` (`PROTOC_GEN_GO_GRPC_VERSION`) |

## Dependency Locks

### Rust

- `Cargo.lock` is committed to the repository and locks all Rust crate versions.
- Run `cargo build` to use the locked versions.

### Go

- `gateway/go.sum` is committed and locks all Go module hashes.
- Run `go mod download` or `go build` to use the locked versions.

## Installing Local Tools

If `protoc` or the Go protoc plugins are not installed globally, run:

```bash
make install-tools
```

This downloads `protoc` to `tools/protoc/` and installs the Go plugins to `$(go env GOPATH)/bin`.

The `tools/` directory is ignored by Git; each developer installs tools locally using the pinned versions above.

## Changing Tool Versions

1. Update the version in the lock file (`rust-toolchain.toml`, `gateway/go.mod`, or `Makefile`).
2. Run `make install-tools` to refresh local tooling.
3. Run `make build` to verify everything still compiles.
4. Commit the updated lock files (`Cargo.lock`, `gateway/go.sum`, etc.).
