use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::ports::PortAllocation;

const BLOCK_START: &str = "# --- rft port overrides ---";
const BLOCK_END: &str = "# --- end rft ---";

pub async fn copy_base_env(repo_root: &Path, worktree_path: &Path) -> Result<PathBuf> {
    let target = worktree_path.join(".env");

    let env_source = repo_root.join(".env");
    if env_source.is_file() {
        tokio::fs::copy(&env_source, &target).await?;
        return Ok(target);
    }

    let example_source = repo_root.join(".env.example");
    if example_source.is_file() {
        tokio::fs::copy(&example_source, &target).await?;
        return Ok(target);
    }

    tokio::fs::write(&target, "").await?;
    Ok(target)
}

pub async fn inject_port_overrides(
    env_path: &Path,
    allocations: &[PortAllocation],
    env_overrides: &HashMap<String, String>,
) -> Result<()> {
    let content = tokio::fs::read_to_string(env_path).await?;
    let clean = strip_override_block(&content);
    let block = build_override_block(allocations, env_overrides);

    let mut final_content = clean.trim_end().to_string();
    if !final_content.is_empty() {
        final_content.push('\n');
    }
    final_content.push_str(&block);
    final_content.push('\n');

    tokio::fs::write(env_path, final_content).await?;
    Ok(())
}

pub fn strip_override_block(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut inside_block = false;

    for line in content.lines() {
        if line.trim() == BLOCK_START {
            inside_block = true;
            continue;
        }
        if line.trim() == BLOCK_END {
            inside_block = false;
            continue;
        }
        if !inside_block {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

pub fn build_override_block(
    allocations: &[PortAllocation],
    env_overrides: &HashMap<String, String>,
) -> String {
    let mut lines = Vec::new();
    lines.push(BLOCK_START.to_string());

    for allocation in allocations {
        lines.push(format!("{}={}", allocation.env_var, allocation.port));
    }

    let resolved_overrides = resolve_template_vars(allocations, env_overrides);
    let mut override_keys: Vec<&String> = resolved_overrides.keys().collect();
    override_keys.sort();
    for key in override_keys {
        lines.push(format!("{}={}", key, resolved_overrides[key]));
    }

    lines.push(BLOCK_END.to_string());
    lines.join("\n")
}

fn resolve_template_vars(
    allocations: &[PortAllocation],
    env_overrides: &HashMap<String, String>,
) -> HashMap<String, String> {
    let port_lookup: HashMap<&str, u16> = allocations
        .iter()
        .map(|allocation| (allocation.env_var.as_str(), allocation.port))
        .collect();

    env_overrides
        .iter()
        .map(|(key, template)| {
            let resolved = resolve_single_template(template, &port_lookup);
            (key.clone(), resolved)
        })
        .collect()
}

fn resolve_single_template(template: &str, port_lookup: &HashMap<&str, u16>) -> String {
    let mut result = template.to_string();
    for (var_name, port_value) in port_lookup {
        let placeholder = format!("${{{var_name}}}");
        result = result.replace(&placeholder, &port_value.to_string());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_allocation(env_var: &str, port: u16) -> PortAllocation {
        PortAllocation {
            service_name: "svc".to_string(),
            env_var: env_var.to_string(),
            port,
            container_port: 80,
        }
    }

    #[test]
    fn strip_removes_override_block() {
        let content = "\
DB_HOST=localhost
# --- rft port overrides ---
WEB_PORT=23001
# --- end rft ---
EXTRA=value
";
        let stripped = strip_override_block(content);
        assert!(stripped.contains("DB_HOST=localhost"));
        assert!(stripped.contains("EXTRA=value"));
        assert!(!stripped.contains("WEB_PORT=23001"));
        assert!(!stripped.contains("rft port overrides"));
    }

    #[test]
    fn strip_is_idempotent() {
        let content = "DB_HOST=localhost\n";
        let stripped_once = strip_override_block(content);
        let stripped_twice = strip_override_block(&stripped_once);
        assert_eq!(stripped_once, stripped_twice);
    }

    #[test]
    fn strip_handles_empty_content() {
        let stripped = strip_override_block("");
        assert_eq!(stripped, "");
    }

    #[test]
    fn build_block_with_allocations() {
        let allocations = vec![
            make_allocation("WEB_PORT", 23001),
            make_allocation("API_PORT", 28081),
        ];
        let block = build_override_block(&allocations, &HashMap::new());

        assert!(block.starts_with(BLOCK_START));
        assert!(block.ends_with(BLOCK_END));
        assert!(block.contains("WEB_PORT=23001"));
        assert!(block.contains("API_PORT=28081"));
    }

    #[test]
    fn build_block_with_env_overrides() {
        let allocations = vec![make_allocation("WEB_PORT", 23001)];
        let mut overrides = HashMap::new();
        overrides.insert(
            "APP_URL".to_string(),
            "http://localhost:${WEB_PORT}".to_string(),
        );

        let block = build_override_block(&allocations, &overrides);

        assert!(block.contains("WEB_PORT=23001"));
        assert!(block.contains("APP_URL=http://localhost:23001"));
    }

    #[test]
    fn template_substitution_replaces_multiple_vars() {
        let allocations = vec![
            make_allocation("WEB_PORT", 23001),
            make_allocation("API_PORT", 28081),
        ];
        let mut overrides = HashMap::new();
        overrides.insert(
            "PROXY".to_string(),
            "web=${WEB_PORT},api=${API_PORT}".to_string(),
        );

        let block = build_override_block(&allocations, &overrides);
        assert!(block.contains("PROXY=web=23001,api=28081"));
    }

    #[test]
    fn template_without_matching_var_stays_as_is() {
        let allocations = vec![make_allocation("WEB_PORT", 23001)];
        let mut overrides = HashMap::new();
        overrides.insert(
            "URL".to_string(),
            "http://localhost:${UNKNOWN_PORT}".to_string(),
        );

        let block = build_override_block(&allocations, &overrides);
        assert!(block.contains("URL=http://localhost:${UNKNOWN_PORT}"));
    }

    #[tokio::test]
    async fn copy_base_env_uses_dotenv() {
        let repo = tempfile::tempdir().unwrap();
        let worktree = tempfile::tempdir().unwrap();

        tokio::fs::write(repo.path().join(".env"), "DB=postgres").await.unwrap();

        let target = copy_base_env(repo.path(), worktree.path()).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&target).await.unwrap(),
            "DB=postgres"
        );
    }

    #[tokio::test]
    async fn copy_base_env_falls_back_to_example() {
        let repo = tempfile::tempdir().unwrap();
        let worktree = tempfile::tempdir().unwrap();

        tokio::fs::write(repo.path().join(".env.example"), "DB=example")
            .await
            .unwrap();

        let target = copy_base_env(repo.path(), worktree.path()).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&target).await.unwrap(),
            "DB=example"
        );
    }

    #[tokio::test]
    async fn copy_base_env_creates_empty_when_no_source() {
        let repo = tempfile::tempdir().unwrap();
        let worktree = tempfile::tempdir().unwrap();

        let target = copy_base_env(repo.path(), worktree.path()).await.unwrap();
        assert_eq!(tokio::fs::read_to_string(&target).await.unwrap(), "");
    }

    #[tokio::test]
    async fn inject_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let env_path = dir.path().join(".env");
        tokio::fs::write(&env_path, "EXISTING=value").await.unwrap();

        let allocations = vec![make_allocation("WEB_PORT", 23001)];

        inject_port_overrides(&env_path, &allocations, &HashMap::new())
            .await
            .unwrap();
        let first_write = tokio::fs::read_to_string(&env_path).await.unwrap();

        inject_port_overrides(&env_path, &allocations, &HashMap::new())
            .await
            .unwrap();
        let second_write = tokio::fs::read_to_string(&env_path).await.unwrap();

        assert_eq!(first_write, second_write);
    }
}
