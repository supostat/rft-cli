use regex::Regex;

use crate::compose::ComposeService;

#[derive(Debug, Clone)]
pub struct PortMapping {
    pub service_name: String,
    pub env_var: Option<String>,
    pub default_port: u16,
    pub container_port: u16,
    pub raw: String,
}

pub fn extract_port_mappings(services: &[ComposeService]) -> Vec<PortMapping> {
    let env_var_pattern =
        Regex::new(r"^\$\{([A-Z_][A-Z0-9_]*):-(\d+)\}$").expect("valid regex");

    services
        .iter()
        .flat_map(|service| {
            service.ports.iter().filter_map(|raw| {
                parse_port_string(raw, &service.name, &env_var_pattern)
            })
        })
        .collect()
}

pub fn suggest_env_var(service_name: &str, port: u16) -> String {
    let normalized = service_name
        .replace('-', "_")
        .replace('.', "_")
        .to_uppercase();

    format!("{normalized}_PORT_{port}")
}

fn parse_port_string(
    raw: &str,
    service_name: &str,
    env_var_pattern: &Regex,
) -> Option<PortMapping> {
    let stripped = strip_protocol(raw);
    let (host_part, container_port_str) = split_host_container(&stripped);

    let container_port = container_port_str.parse::<u16>().ok()?;
    let (env_var, default_port) = parse_host_part(&host_part, env_var_pattern)?;

    Some(PortMapping {
        service_name: service_name.to_owned(),
        env_var,
        default_port,
        container_port,
        raw: raw.to_owned(),
    })
}

fn strip_protocol(port_str: &str) -> String {
    port_str
        .strip_suffix("/tcp")
        .or_else(|| port_str.strip_suffix("/udp"))
        .or_else(|| port_str.strip_suffix("/sctp"))
        .unwrap_or(port_str)
        .to_owned()
}

fn split_host_container(port_str: &str) -> (String, String) {
    // Split on ':' but respect '${...}' blocks that contain ':'
    let segments = split_respecting_braces(port_str);

    match segments.len() {
        1 => (segments[0].clone(), segments[0].clone()),
        2 => (segments[0].clone(), segments[1].clone()),
        3 => (segments[1].clone(), segments[2].clone()),
        _ => (port_str.to_owned(), port_str.to_owned()),
    }
}

fn split_respecting_braces(input: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut brace_depth: u32 = 0;

    for ch in input.chars() {
        match ch {
            '{' => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(ch);
            }
            ':' if brace_depth == 0 => {
                segments.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }

    segments.push(current);
    segments
}

fn parse_host_part(host_part: &str, env_var_pattern: &Regex) -> Option<(Option<String>, u16)> {
    if let Some(captures) = env_var_pattern.captures(host_part) {
        let var_name = captures[1].to_owned();
        let default_port = captures[2].parse::<u16>().ok()?;
        return Some((Some(var_name), default_port));
    }

    let port = host_part.parse::<u16>().ok()?;
    Some((None, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_service(name: &str, ports: &[&str]) -> ComposeService {
        ComposeService {
            name: name.to_owned(),
            ports: ports.iter().map(|s| s.to_string()).collect(),
            build: None,
            env_file: Vec::new(),
        }
    }

    #[test]
    fn parse_env_var_with_default() {
        let services = [make_service("frontend", &["${FRONTEND_PORT:-3000}:3000"])];
        let mappings = extract_port_mappings(&services);

        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].env_var.as_deref(), Some("FRONTEND_PORT"));
        assert_eq!(mappings[0].default_port, 3000);
        assert_eq!(mappings[0].container_port, 3000);
    }

    #[test]
    fn parse_numeric_host_container() {
        let services = [make_service("api", &["8080:80"])];
        let mappings = extract_port_mappings(&services);

        assert_eq!(mappings.len(), 1);
        assert!(mappings[0].env_var.is_none());
        assert_eq!(mappings[0].default_port, 8080);
        assert_eq!(mappings[0].container_port, 80);
    }

    #[test]
    fn parse_single_port() {
        let services = [make_service("redis", &["6379"])];
        let mappings = extract_port_mappings(&services);

        assert_eq!(mappings.len(), 1);
        assert!(mappings[0].env_var.is_none());
        assert_eq!(mappings[0].default_port, 6379);
        assert_eq!(mappings[0].container_port, 6379);
    }

    #[test]
    fn strip_tcp_protocol_suffix() {
        let services = [make_service("db", &["5432:5432/tcp"])];
        let mappings = extract_port_mappings(&services);

        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].default_port, 5432);
        assert_eq!(mappings[0].container_port, 5432);
    }

    #[test]
    fn strip_udp_protocol_suffix() {
        let services = [make_service("dns", &["5353:53/udp"])];
        let mappings = extract_port_mappings(&services);

        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].default_port, 5353);
        assert_eq!(mappings[0].container_port, 53);
    }

    #[test]
    fn handle_ip_prefix() {
        let services = [make_service("web", &["127.0.0.1:8080:80"])];
        let mappings = extract_port_mappings(&services);

        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].default_port, 8080);
        assert_eq!(mappings[0].container_port, 80);
    }

    #[test]
    fn multiple_services_and_ports() {
        let services = [
            make_service("frontend", &["${FE_PORT:-3000}:3000"]),
            make_service("api", &["8080:80", "9090:9090"]),
        ];
        let mappings = extract_port_mappings(&services);

        assert_eq!(mappings.len(), 3);
        assert_eq!(mappings[0].service_name, "frontend");
        assert_eq!(mappings[1].service_name, "api");
        assert_eq!(mappings[2].service_name, "api");
    }

    #[test]
    fn service_with_no_ports() {
        let services = [make_service("worker", &[])];
        let mappings = extract_port_mappings(&services);

        assert!(mappings.is_empty());
    }

    #[test]
    fn suggest_env_var_simple() {
        assert_eq!(suggest_env_var("frontend", 3000), "FRONTEND_PORT_3000");
    }

    #[test]
    fn suggest_env_var_with_dashes() {
        assert_eq!(suggest_env_var("my-api", 8080), "MY_API_PORT_8080");
    }

    #[test]
    fn suggest_env_var_with_dots() {
        assert_eq!(suggest_env_var("db.primary", 5432), "DB_PRIMARY_PORT_5432");
    }
}
