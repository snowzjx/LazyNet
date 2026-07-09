use crate::data::{IfaceCounters, Inventory, Node, NodeType};
use anyhow::Result;
use std::process::Command;

pub mod network;
pub mod pci;
pub mod rdma;

pub trait Collector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()>;
}

pub struct NetworkCollector;

impl NetworkCollector {
    pub fn new() -> Self {
        Self
    }

    #[cfg(target_os = "macos")]
    fn parse_ifconfig_output(&self, output: &str) -> Vec<Node> {
        let mut nodes = Vec::new();
        let mut current_interface: Option<String> = None;
        let mut current_flags: Option<String> = None;
        let mut current_mtu: Option<String> = None;

        for line in output.lines() {
            if !line.starts_with('\t') && !line.starts_with(' ') && line.contains(':') {
                // Save previous interface if exists
                if let Some(interface_name) = current_interface.take() {
                    let mut node = Node::new(
                        format!("netdev:{}", interface_name),
                        NodeType::NetworkInterface,
                    )
                    .with_property("name", &interface_name)
                    .with_property("type", "network_interface");

                    if let Some(flags) = current_flags.take() {
                        node = node.with_property("flags", &flags);
                        let state = if flags.contains("UP") { "up" } else { "down" };
                        node = node.with_property("state", state);
                    }

                    if let Some(mtu) = current_mtu.take() {
                        node = node.with_property("mtu", &mtu);
                    }

                    nodes.push(node);
                }

                // Parse new interface line
                if let Some(colon_pos) = line.find(':') {
                    current_interface = Some(line[..colon_pos].to_string());

                    // Parse flags
                    if let Some(flags_start) = line.find('<') {
                        if let Some(flags_end) = line.find('>') {
                            current_flags = Some(line[flags_start + 1..flags_end].to_string());
                        }
                    }

                    // Parse MTU
                    if let Some(mtu_pos) = line.find("mtu ") {
                        let mtu_part = &line[mtu_pos + 4..];
                        if let Some(mtu_end) = mtu_part.find(' ') {
                            current_mtu = Some(mtu_part[..mtu_end].to_string());
                        } else {
                            current_mtu = Some(mtu_part.trim().to_string());
                        }
                    }
                }
            }
        }

        // Don't forget the last interface
        if let Some(interface_name) = current_interface {
            let mut node = Node::new(
                format!("netdev:{}", interface_name),
                NodeType::NetworkInterface,
            )
            .with_property("name", &interface_name)
            .with_property("type", "network_interface");

            if let Some(flags) = current_flags {
                node = node.with_property("flags", &flags);
                let state = if flags.contains("UP") { "up" } else { "down" };
                node = node.with_property("state", state);
            }

            if let Some(mtu) = current_mtu {
                node = node.with_property("mtu", &mtu);
            }

            nodes.push(node);
        }

        nodes
    }

    #[cfg(target_os = "linux")]
    fn parse_ip_output(&self, output: &str) -> Vec<Node> {
        let mut nodes = Vec::new();

        for line in output.lines() {
            if line.trim().is_empty() || line.starts_with(' ') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let interface_name = parts[1].trim_end_matches(':');
            let mut node = Node::new(
                format!("netdev:{}", interface_name),
                NodeType::NetworkInterface,
            )
            .with_property("name", interface_name)
            .with_property("type", "network_interface");

            // Parse flags and state
            if let Some(flags_start) = line.find('<') {
                if let Some(flags_end) = line.find('>') {
                    let flags = &line[flags_start + 1..flags_end];
                    node = node.with_property("flags", flags);

                    let state = if flags.contains("UP") { "up" } else { "down" };
                    node = node.with_property("state", state);
                }
            }

            // Parse MTU
            if let Some(mtu_pos) = line.find("mtu ") {
                let mtu_part = &line[mtu_pos + 4..];
                if let Some(mtu_end) = mtu_part.find(' ') {
                    let mtu = &mtu_part[..mtu_end];
                    node = node.with_property("mtu", mtu);
                } else {
                    node = node.with_property("mtu", mtu_part.trim());
                }
            }

            nodes.push(node);
        }

        nodes
    }

    async fn get_mac_addresses(&self) -> Result<std::collections::HashMap<String, String>> {
        let mut mac_map = std::collections::HashMap::new();

        #[cfg(target_os = "linux")]
        {
            let output = Command::new("ip").args(["link", "show"]).output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_interface = None;

            for line in stdout.lines() {
                if !line.starts_with(' ') {
                    // New interface line
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        current_interface = Some(parts[1].trim_end_matches(':').to_string());
                    }
                } else if line.contains("link/ether") {
                    // MAC address line
                    if let Some(interface) = &current_interface {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 {
                            mac_map.insert(interface.clone(), parts[1].to_string());
                        }
                    }
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            let output = Command::new("ifconfig").output()?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_interface = None;

            for line in stdout.lines() {
                if !line.starts_with('\t') && !line.starts_with(' ') && line.contains(':') {
                    // New interface line
                    if let Some(colon_pos) = line.find(':') {
                        current_interface = Some(line[..colon_pos].to_string());
                    }
                } else if line.contains("ether ") {
                    // MAC address line
                    if let Some(interface) = &current_interface {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if let Some(ether_pos) = parts.iter().position(|&x| x == "ether") {
                            if ether_pos + 1 < parts.len() {
                                mac_map.insert(interface.clone(), parts[ether_pos + 1].to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(mac_map)
    }
}

impl Collector for NetworkCollector {
    async fn collect(&self, inventory: &mut Inventory) -> Result<()> {
        #[cfg(target_os = "linux")]
        let output = Command::new("ip").args(["addr", "show"]).output()?;

        #[cfg(target_os = "macos")]
        let output = Command::new("ifconfig").output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        #[cfg(target_os = "linux")]
        let mut nodes = self.parse_ip_output(&stdout);

        #[cfg(target_os = "macos")]
        let mut nodes = self.parse_ifconfig_output(&stdout);

        // Get MAC addresses
        let mac_addresses = self.get_mac_addresses().await?;

        // Add MAC addresses to nodes
        for node in &mut nodes {
            if let Some(name) = node.get_property("name") {
                if let Some(mac) = mac_addresses.get(name) {
                    node.properties.insert("mac".to_string(), mac.clone());
                }
            }
        }

        for node in nodes {
            if let Some(name) = node.get_property("name") {
                inventory
                    .iface_counters
                    .insert(name.clone(), read_iface_counters(name));
            }
            inventory.add_node(node);
        }

        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn read_stat(dev: &str, stat: &str) -> u64 {
    std::fs::read_to_string(format!("/sys/class/net/{}/statistics/{}", dev, stat))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
pub fn read_iface_counters(dev: &str) -> IfaceCounters {
    IfaceCounters {
        rx_bytes: read_stat(dev, "rx_bytes"),
        tx_bytes: read_stat(dev, "tx_bytes"),
        rx_packets: read_stat(dev, "rx_packets"),
        tx_packets: read_stat(dev, "tx_packets"),
        rx_errors: read_stat(dev, "rx_errors"),
        tx_errors: read_stat(dev, "tx_errors"),
        rx_dropped: read_stat(dev, "rx_dropped"),
        tx_dropped: read_stat(dev, "tx_dropped"),
        rx_missed: read_stat(dev, "rx_missed_errors"),
        collisions: read_stat(dev, "collisions"),
    }
}

#[cfg(target_os = "macos")]
pub fn read_iface_counters(dev: &str) -> IfaceCounters {
    use std::ptr;

    let mut mib = [libc::CTL_NET, libc::PF_ROUTE, 0, 0, libc::NET_RT_IFLIST2, 0];
    let mut len: libc::size_t = 0;

    let size_result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            ptr::null_mut(),
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    if size_result != 0 || len == 0 {
        return IfaceCounters::default();
    }

    let mut buf = vec![0_u8; len as usize];
    let read_result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            buf.as_mut_ptr() as *mut libc::c_void,
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    if read_result != 0 {
        return IfaceCounters::default();
    }

    let mut cursor = buf.as_ptr();
    let end = unsafe { buf.as_ptr().add(len as usize) };

    while cursor < end {
        let remaining = unsafe { end.offset_from(cursor) as usize };
        if remaining < std::mem::size_of::<libc::if_msghdr2>() {
            break;
        }

        let msg = cursor as *const libc::if_msghdr2;
        let msg_len = unsafe { ptr::addr_of!((*msg).ifm_msglen).read_unaligned() as usize };
        if msg_len == 0 || msg_len > remaining {
            break;
        }

        let msg_type = unsafe { ptr::addr_of!((*msg).ifm_type).read_unaligned() as libc::c_int };
        if msg_type == libc::RTM_IFINFO2 {
            let index = unsafe { ptr::addr_of!((*msg).ifm_index).read_unaligned() };
            if iface_name(index as libc::c_uint).as_deref() == Some(dev) {
                let data = unsafe { ptr::addr_of!((*msg).ifm_data) };
                let tx_dropped =
                    unsafe { ptr::addr_of!((*msg).ifm_snd_drops).read_unaligned() }.max(0) as u64;

                return IfaceCounters {
                    rx_bytes: unsafe { ptr::addr_of!((*data).ifi_ibytes).read_unaligned() },
                    tx_bytes: unsafe { ptr::addr_of!((*data).ifi_obytes).read_unaligned() },
                    rx_packets: unsafe { ptr::addr_of!((*data).ifi_ipackets).read_unaligned() },
                    tx_packets: unsafe { ptr::addr_of!((*data).ifi_opackets).read_unaligned() },
                    rx_errors: unsafe { ptr::addr_of!((*data).ifi_ierrors).read_unaligned() },
                    tx_errors: unsafe { ptr::addr_of!((*data).ifi_oerrors).read_unaligned() },
                    rx_dropped: unsafe { ptr::addr_of!((*data).ifi_iqdrops).read_unaligned() },
                    tx_dropped,
                    rx_missed: 0,
                    collisions: unsafe { ptr::addr_of!((*data).ifi_collisions).read_unaligned() },
                };
            }
        }

        cursor = unsafe { cursor.add(msg_len) };
    }

    IfaceCounters::default()
}

#[cfg(target_os = "macos")]
fn iface_name(index: libc::c_uint) -> Option<String> {
    use std::ffi::CStr;

    let mut name = [0 as libc::c_char; libc::IF_NAMESIZE as usize];
    let ptr = unsafe { libc::if_indextoname(index, name.as_mut_ptr()) };
    if ptr.is_null() {
        return None;
    }

    Some(
        unsafe { CStr::from_ptr(name.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn read_iface_counters(_dev: &str) -> IfaceCounters {
    IfaceCounters::default()
}
