use std::net::TcpListener;

use crate::ports::PortAllocation;

#[derive(Debug, PartialEq)]
pub struct PortConflict {
    pub port: u16,
    pub service_name: String,
    pub env_var: String,
}

/// Check whether a single port is available for binding.
pub fn check_port_available(port: u16) -> std::io::Result<bool> {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_listener) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => Ok(false),
        Err(error) => Err(error),
    }
}

/// Check all allocated ports and return conflicts for those already in use.
pub fn check_ports(allocations: &[PortAllocation]) -> Vec<PortConflict> {
    allocations
        .iter()
        .filter_map(|allocation| {
            match check_port_available(allocation.port) {
                Ok(true) => None,
                _ => Some(PortConflict {
                    port: allocation.port,
                    service_name: allocation.service_name.clone(),
                    env_var: allocation.env_var.clone(),
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_port_returns_true() {
        // Port 0 lets OS pick a free port; use it to find one that's free
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let free_port = listener.local_addr().unwrap().port();
        drop(listener);

        assert!(check_port_available(free_port).unwrap());
    }

    #[test]
    fn occupied_port_returns_false() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied_port = listener.local_addr().unwrap().port();
        // Keep listener alive so port stays occupied
        assert!(!check_port_available(occupied_port).unwrap());
        drop(listener);
    }

    #[test]
    fn check_ports_detects_conflict() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied_port = listener.local_addr().unwrap().port();

        let allocations = vec![PortAllocation {
            service_name: "web".to_string(),
            env_var: "WEB_PORT".to_string(),
            port: occupied_port,
            container_port: 3000,
        }];

        let conflicts = check_ports(&allocations);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].port, occupied_port);
        assert_eq!(conflicts[0].service_name, "web");
        assert_eq!(conflicts[0].env_var, "WEB_PORT");

        drop(listener);
    }

    #[test]
    fn check_ports_returns_empty_when_all_available() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let free_port = listener.local_addr().unwrap().port();
        drop(listener);

        let allocations = vec![PortAllocation {
            service_name: "api".to_string(),
            env_var: "API_PORT".to_string(),
            port: free_port,
            container_port: 8080,
        }];

        let conflicts = check_ports(&allocations);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn check_ports_mixed_available_and_occupied() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let occupied_port = listener.local_addr().unwrap().port();

        let free_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let free_port = free_listener.local_addr().unwrap().port();
        drop(free_listener);

        let allocations = vec![
            PortAllocation {
                service_name: "web".to_string(),
                env_var: "WEB_PORT".to_string(),
                port: occupied_port,
                container_port: 3000,
            },
            PortAllocation {
                service_name: "api".to_string(),
                env_var: "API_PORT".to_string(),
                port: free_port,
                container_port: 8080,
            },
        ];

        let conflicts = check_ports(&allocations);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].service_name, "web");

        drop(listener);
    }
}
