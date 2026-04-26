# LazyNet PRD (Product Requirements Document)

## 1. Overview

**Product Name:** LazyNet  
**Type:** Terminal User Interface (TUI) system tool  
**Primary Platform:** Linux (first-class), macOS (limited support)  

**Goal:**  
LazyNet provides a unified, extensible, and configurable terminal interface to inspect network-related devices and their relationships.

---

## 2. Target Users

- Systems researchers
- Datacenter operators
- Performance engineers
- Kernel / networking developers

---

## 3. Key Use Cases

### UC1: Inspect all network interfaces
- List all interfaces with IP, MTU, MAC, state
- Identify RDMA-capable interfaces

### UC2: RDMA debugging
- Map RDMA devices to netdevs
- Show IB vs RoCE

### UC3: DPDK debugging
- Check binding and NUMA alignment

### UC4: Topology inspection
- netdev ↔ PCI ↔ RDMA ↔ DPDK relationships

---

## 4. Product Principles

- Unified View
- Extensibility First
- Read-only by default
- Platform-aware

---

## 5. Functional Requirements

### Core Features
- Device Inventory aggregation
- Interface listing
- Detail view
- RDMA view
- DPDK view
- Search/filter
- JSON export

---

## 6. Non-Functional Requirements

- <200ms refresh
- <50MB memory
- No root required (optional for advanced collectors)

---

## 7. Architecture

Collectors → Facts → Inventory Graph → Views → TUI

---

## 8. Data Model

```rust
struct Inventory {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}
```

---

## 9. UI Design

Tabs:
- Interfaces
- RDMA
- DPDK
- PCI
- Raw

---

## 10. Configuration

File: ~/.lazynet/config.toml

---

## 11. MVP Scope

Included:
- Interface list
- RDMA detection
- PCI mapping
- Basic TUI
- JSON export

Excluded:
- Write operations
- Remote SSH

---

## 12. Future Extensions

- SSH mode
- eBPF integration
- Live monitoring
- SmartNIC / GPU support

---

## 13. Risks

- Data inconsistency
- RDMA detection complexity
- Platform gaps

---

## 14. Success Metrics

- Replace multiple CLI tools
- Fast debugging (<5s)

---

## 15. Tech Stack

- Rust
- ratatui + crossterm
- serde + toml
- tokio

---

## 16. Milestones

Phase 1: basic collectors + UI  
Phase 2: RDMA + graph  
Phase 3: DPDK + config  
Phase 4: macOS + polish  

---

## 17. Deliverables

- lazynet binary
- config file
- README
- JSON export
