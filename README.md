# LazyNet v0.1

A terminal UI for inspecting network devices, RDMA adapters, and PCIe hardware — with live counter recording and delta analysis.

## Counter Recording

> **The killer feature.** Inspired by video editing in/out points — press `[` to mark the start, do whatever you need (run a benchmark, send traffic, trigger a workload), then press `]` to mark the end.

LazyNet instantly shows you:

- **Δ bytes / packets** — exactly how much traffic moved during the window
- **Throughput rate** — RX/TX in bps/Kbps/Mbps/Gbps calculated over the precise elapsed time
- **Δ error counters** — any drops, errors, or missed frames that occurred, highlighted in red
- **RDMA deltas** — port RCV/XMT data, RoCE retransmits, CNP/ECN congestion events, out-of-buffer counts

Press `Esc` to exit delta view and return to live counters. Press `[` again to start a new recording.

This makes it trivial to answer questions like:
- "How much bandwidth did that `iperf3` run actually use?"
- "Did any PFC pause frames fire during that congestion event?"
- "Were there RNR NAK retries during my RDMA benchmark?"

## Features

- **Interfaces tab** — all network interfaces with state, MAC, MTU, and live traffic counters
- **RDMA tab** — RDMA ports with transport type, firmware version, GUID, netdev mapping, PFC config, port counters, and RoCE/congestion counters (ECN/CNP, retransmits, buffer errors)
- **PCI tab** — all PCI devices with vendor/device/class, driver, PCIe link speed and width (with degraded detection), NUMA node, IOMMU group
- **Raw tab** — live, stable JSON inventory dump for debugging, with line and page scrolling
- **Counter recording** — press `[` to start, `]` to finish; shows delta counts and throughput rates
- **Search** — `/` to filter any tab by name, vendor, or description
- **Graph navigation** — `←`/`→` jumps between connected Interface, RDMA, and PCI devices
- **Navigation** — `↑`/`↓` to select items; detail panel updates live

## Installation

```bash
cargo build --release
# optional
cargo install --path .
```

## Usage

```bash
lazynet              # launch TUI
lazynet --export     # print inventory as JSON
lazynet --config /path/to/config.toml
```

## Keybindings

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch tabs |
| `1`–`4` | Jump to tab |
| `↑` / `↓` | Navigate list / scroll Raw by line |
| `PageUp` / `PageDown` | Scroll Raw by page |
| `←` / `→` | Jump to connected Interface/RDMA/PCI device |
| `/` | Search |
| `Esc` | Clear search / exit delta view |
| `[` | Start counter recording |
| `]` | Finish recording, show delta |
| `h` / `F1` | Toggle help |
| `q` | Quit |

## Configuration

`~/.lazynet/config.toml` is created automatically on first run.

```toml
[ui]
refresh_interval_ms = 1000
default_tab = "interfaces"
show_raw_tab = true

[collectors]
enable_network = true
enable_pci = true
enable_rdma = true
enable_dpdk = false

[export]
pretty_json = true
include_metadata = true
```

## Data Sources

| Data | Source |
|------|--------|
| Network interfaces | `ip addr`, `/sys/class/net/*/statistics/` |
| PCI devices | `lspci -vmm`, `/sys/bus/pci/devices/*/` |
| RDMA devices | `/sys/class/infiniband/`, `ibv_devinfo` |
| RDMA→netdev mapping | `rdma link` |
| PFC config | `dcb pfc show` |
| PFC / port counters | `ethtool -S`, `/sys/class/infiniband/*/ports/*/counters/` |
| RoCE hw counters | `/sys/class/infiniband/*/ports/*/hw_counters/` |

## Requirements

- Linux (primary platform)
- `lspci` for PCI data
- `ibv_devinfo`, `rdma`, `dcb`, `ethtool` for full RDMA/PFC data (gracefully degraded if absent)

## License

MIT
