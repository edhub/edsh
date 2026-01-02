# Project: edsh (Iroh-based SSH Proxy)

## Overview
`edsh` is a peer-to-peer (P2P) SSH proxy built on top of the `iroh` network stack. It allows users to establish SSH connections through NATs and firewalls without complex port forwarding or VPNs, using `iroh`'s QUIC-based connectivity and NAT-traversal capabilities.

## Architecture
The project is a single Rust crate that provides a unified CLI tool:

- **edsh**: The main binary with subcommands for different roles.
  - **Client (`edsh connect`)**: Designed to be used as an SSH `ProxyCommand`. It bridges standard I/O to an `iroh` stream.
  - **Server (`edsh server`)**: A daemon that listens for incoming `iroh` connections and forwards them to a local SSH server (typically port 22).

## Workflow
1. **Server**: Runs `edsh server`, generating/loading a `SecretKey` and listening on a specific ALPN.
2. **Client**: Configured via `~/.ssh/config` using `ProxyCommand edsh connect <EndpointID>`.
3. **Transport**: Data is encapsulated in QUIC streams via `iroh`, providing end-to-end encryption and P2P hole punching.

## Tech Stack
- **Language**: Rust
- **Networking**: [iroh](https://iroh.computer/) (QUIC, DERP, Magicsock)
- **Async Runtime**: tokio
- **CLI**: clap
