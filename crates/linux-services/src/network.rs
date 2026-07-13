//! Network information (TCLIS SS4 "Network"; Apis SS9 Network Service).
//! Interface listing uses `/sys/class/net` (a kernel interface, always
//! present) plus `ip addr` for addresses -- no new dependency needed.

use common::AppError;

#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub addresses: Vec<String>,
    pub is_up: bool,
}

pub fn list_interfaces() -> common::AppResult<Vec<NetworkInterface>> {
    let names: Vec<String> = std::fs::read_dir("/sys/class/net")
        .map_err(|e| AppError::Module {
            module: "linux-services::network",
            message: format!("reading /sys/class/net: {e}"),
        })?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();

    // Interface names come from sysfs unconditionally; addresses need
    // `ip`, which -- unlike /sys/class/net -- isn't guaranteed to be
    // installed (this sandbox didn't have it until iproute2 was added).
    // Missing it degrades to interface names with no addresses rather
    // than failing the whole call.
    let addr_text = std::process::Command::new("ip")
        .args(["-o", "addr", "show"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();

    let mut interfaces: Vec<NetworkInterface> = names
        .into_iter()
        .map(|name| {
            let is_up = std::fs::read_to_string(format!("/sys/class/net/{name}/operstate"))
                .map(|s| s.trim() == "up")
                .unwrap_or(false);

            let addresses = addr_text
                .lines()
                .filter(|line| {
                    line.split_whitespace()
                        .nth(1)
                        .map(|n| n.trim_end_matches(':') == name)
                        .unwrap_or(false)
                })
                .filter_map(|line| line.split_whitespace().nth(3).map(str::to_string))
                .collect();

            NetworkInterface {
                name,
                addresses,
                is_up,
            }
        })
        .collect();

    interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(interfaces)
}

/// `true` if any interface has a `wireless` subdirectory in sysfs --
/// i.e. actual Wi-Fi hardware is present. Containers and servers
/// (including this sandbox) honestly report `false` here; a laptop
/// would report `true`.
pub fn has_wifi_hardware() -> bool {
    std::fs::read_dir("/sys/class/net")
        .map(|rd| rd.flatten().any(|e| e.path().join("wireless").exists()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_lists_the_loopback_interface() {
        let interfaces = list_interfaces().unwrap();
        assert!(interfaces.iter().any(|i| i.name == "lo"));
    }

    #[test]
    fn loopback_has_the_localhost_address() {
        let interfaces = list_interfaces().unwrap();
        let lo = interfaces.iter().find(|i| i.name == "lo").unwrap();
        assert!(lo.addresses.iter().any(|a| a.starts_with("127.0.0.1")));
    }

    #[test]
    fn this_sandbox_honestly_has_no_wifi_hardware() {
        assert!(!has_wifi_hardware());
    }
}
