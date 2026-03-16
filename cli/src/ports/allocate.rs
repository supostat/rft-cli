use std::collections::HashSet;

use crate::error::{Result, RftError};
use crate::ports::extract::{PortMapping, suggest_env_var};

pub const BASE_OFFSET: u32 = 20000;

#[derive(Debug, Clone)]
pub struct PortAllocation {
    pub service_name: String,
    pub env_var: String,
    pub port: u16,
    pub container_port: u16,
}

pub fn allocate_port(
    default_port: u16,
    worktree_index: usize,
    base_offset: u32,
) -> Result<u16> {
    let candidate = base_offset + default_port as u32 + worktree_index as u32;

    if candidate <= 65535 {
        return Ok(candidate as u16);
    }

    let fallback = default_port as u32 + 100 * worktree_index as u32;

    if !(1024..=65535).contains(&fallback) {
        return Err(RftError::PortOutOfRange { port: fallback });
    }

    Ok(fallback as u16)
}

pub fn allocate_worktree_ports(
    mappings: &[PortMapping],
    worktree_index: usize,
    base_offset: u32,
) -> Result<Vec<PortAllocation>> {
    let eligible: Vec<_> = mappings.iter().filter(|m| m.env_var.is_some()).collect();

    let mut allocated_ports = HashSet::with_capacity(eligible.len());
    let mut allocations = Vec::with_capacity(eligible.len());

    for mapping in eligible {
        let port = allocate_port(mapping.default_port, worktree_index, base_offset)?;

        if !allocated_ports.insert(port) {
            return Err(RftError::PortCollision { port });
        }

        let env_var = mapping
            .env_var
            .clone()
            .unwrap_or_else(|| suggest_env_var(&mapping.service_name, mapping.default_port));

        allocations.push(PortAllocation {
            service_name: mapping.service_name.clone(),
            env_var,
            port,
            container_port: mapping.container_port,
        });
    }

    Ok(allocations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mapping(
        service_name: &str,
        env_var: Option<&str>,
        default_port: u16,
        container_port: u16,
    ) -> PortMapping {
        PortMapping {
            service_name: service_name.to_owned(),
            env_var: env_var.map(str::to_owned),
            default_port,
            container_port,
            raw: format!("{default_port}:{container_port}"),
        }
    }

    #[test]
    fn allocate_basic_formula() {
        let port = allocate_port(3000, 1, BASE_OFFSET).unwrap();
        assert_eq!(port, 23001);
    }

    #[test]
    fn allocate_with_zero_index() {
        let port = allocate_port(8080, 0, BASE_OFFSET).unwrap();
        assert_eq!(port, 28080);
    }

    #[test]
    fn allocate_overflow_uses_fallback() {
        let port = allocate_port(50000, 1, 20000).unwrap();
        // 20000 + 50000 + 1 = 70001 > 65535, fallback: 50000 + 100 = 50100
        assert_eq!(port, 50100);
    }

    #[test]
    fn allocate_fallback_also_overflows() {
        let result = allocate_port(65000, 10, 20000);
        // 20000 + 65000 + 10 > 65535, fallback: 65000 + 1000 = 66000 > 65535
        assert!(result.is_err());
    }

    #[test]
    fn allocate_fallback_below_min_port() {
        let result = allocate_port(500, 0, 70000);
        // 70000 + 500 + 0 > 65535, fallback: 500 + 0 = 500 < 1024
        assert!(result.is_err());
    }

    #[test]
    fn allocate_worktree_ports_skips_none_env_var() {
        let mappings = [
            make_mapping("frontend", Some("FE_PORT"), 3000, 3000),
            make_mapping("redis", None, 6379, 6379),
        ];

        let allocations = allocate_worktree_ports(&mappings, 1, BASE_OFFSET).unwrap();
        assert_eq!(allocations.len(), 1);
        assert_eq!(allocations[0].service_name, "frontend");
        assert_eq!(allocations[0].env_var, "FE_PORT");
        assert_eq!(allocations[0].port, 23001);
    }

    #[test]
    fn allocate_worktree_ports_preserves_env_var_name() {
        let mappings = [make_mapping("api", Some("API_PORT"), 8080, 80)];

        let allocations = allocate_worktree_ports(&mappings, 2, BASE_OFFSET).unwrap();
        assert_eq!(allocations[0].env_var, "API_PORT");
        assert_eq!(allocations[0].port, 28082);
        assert_eq!(allocations[0].container_port, 80);
    }

    #[test]
    fn detect_port_collision() {
        let mappings = [
            make_mapping("svc-a", Some("A_PORT"), 3000, 3000),
            make_mapping("svc-b", Some("B_PORT"), 3000, 3000),
        ];

        let result = allocate_worktree_ports(&mappings, 1, BASE_OFFSET);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(
            matches!(error, RftError::PortCollision { port: 23001 }),
            "expected PortCollision(23001), got: {error:?}"
        );
    }

    #[test]
    fn no_collision_with_different_ports() {
        let mappings = [
            make_mapping("frontend", Some("FE_PORT"), 3000, 3000),
            make_mapping("api", Some("API_PORT"), 8080, 80),
        ];

        let allocations = allocate_worktree_ports(&mappings, 1, BASE_OFFSET).unwrap();
        assert_eq!(allocations.len(), 2);
        assert_eq!(allocations[0].port, 23001);
        assert_eq!(allocations[1].port, 28081);
    }
}
