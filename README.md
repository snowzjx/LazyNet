# LazyNet

A Terminal User Interface (TUI) system tool for network device inspection and debugging.

## Overview

LazyNet provides a unified, extensible, and configurable terminal interface to inspect network-related devices and their relationships. It's designed for systems researchers, datacenter operators, performance engineers, and kernel/networking developers.

## Features

- **Unified Device View**: Inspect network interfaces, PCI devices, and RDMA devices in one place
- **Relationship Mapping**: Understand connections between netdevs, PCI devices, and RDMA devices
- **Fast Performance**: <200ms refresh time, <50MB memory usage
- **No Root Required**: Works with user privileges (some advanced features may require root)
- **JSON Export**: Export inventory data for automation and analysis
- **Configurable**: Customize behavior via `~/.lazynet/config.toml`

## Installation

```bash
# Clone the repository
git clone <repository-url>
cd LazyNet

# Build the project
cargo build --release

# Install (optional)
cargo install --path .
```

## Usage

### Basic Usage

```bash
# Launch the TUI
lazynet

# Export inventory to JSON
lazynet --export

# Use custom config file
lazynet --config /path/to/config.toml
```

### TUI Navigation

- **Tab/Shift+Tab**: Switch between tabs
- **1-4**: Jump directly to tabs (Interfaces, RDMA, PCI, Raw)
- **h/F1**: Toggle help
- **q**: Quit
- **/**: Search (TODO)
- **Esc**: Clear search/Close help

### Tabs

1. **Interfaces**: Network interfaces with IP, MTU, MAC, and state information
2. **RDMA**: RDMA devices with transport type (InfiniBand/RoCE) and netdev mappings
3. **PCI**: PCI devices with descriptions and network device connections
4. **Raw**: Raw JSON inventory data for debugging

## Configuration

LazyNet uses a TOML configuration file located at `~/.lazynet/config.toml`. A default configuration is created automatically on first run.

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

## Platform Support

- **Linux**: First-class support with full feature set
- **macOS**: Limited support (network interfaces and PCI devices)

## Architecture

```
Collectors → Facts → Inventory Graph → Views → TUI
```

- **Collectors**: Gather data from system interfaces (`ip`, `lspci`, `ibstat`, sysfs)
- **Inventory Graph**: Unified data model with nodes (devices) and edges (relationships)
- **Views**: Tab-specific rendering of inventory data
- **TUI**: Terminal interface built with ratatui

## Data Model

The core data model consists of:

- **Inventory**: Container for all nodes and edges
- **Node**: Represents a device (network interface, PCI device, RDMA device)
- **Edge**: Represents a relationship between devices

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Adding New Collectors

1. Implement the `Collector` trait
2. Add your collector to the `collectors` module
3. Integrate it into the `App::new()` method

## Roadmap

### Phase 1 (MVP) ✅
- [x] Basic collectors (network, PCI, RDMA)
- [x] Core TUI with tab navigation
- [x] JSON export
- [x] Configuration system

### Phase 2 (Planned)
- [ ] Search and filtering
- [ ] DPDK device support
- [ ] Live monitoring mode
- [ ] Performance optimizations

### Phase 3 (Future)
- [ ] SSH remote mode
- [ ] eBPF integration
- [ ] SmartNIC/GPU support
- [ ] Historical data tracking

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see LICENSE file for details.

## Troubleshooting

### Common Issues

1. **No RDMA devices found**: Ensure RDMA drivers are loaded and devices are present
2. **Permission denied**: Some collectors may require elevated privileges for full functionality
3. **Command not found**: Ensure required system tools (`ip`, `lspci`) are installed

### Debug Mode

Use the Raw tab to inspect the complete inventory data structure for debugging.