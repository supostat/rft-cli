use comfy_table::{Table, ContentArrangement, presets::UTF8_FULL_CONDENSED};
use miette::Result;
use owo_colors::OwoColorize;

use crate::context::{RftContext, build_context, filter_worktrees};
use crate::ports::{PortMapping, PortAllocation, allocate_worktree_ports, BASE_OFFSET};
use crate::sanitize::compose_project_name;

pub async fn run() -> Result<()> {
    let context = build_context().await?;
    let non_main = filter_worktrees(&context.worktrees, &[]);

    if non_main.is_empty() {
        println!("{}", "No worktrees found (only main branch).".dimmed());
        return Ok(());
    }

    let base_offset = context.config.port_offset.unwrap_or(BASE_OFFSET);
    let mut rows = Vec::new();

    for worktree in &non_main {
        let project_name = compose_project_name(
            &context.repo_name,
            worktree.index,
            &worktree.branch,
        );
        let ports = match allocate_worktree_ports(
            &context.port_mappings,
            worktree.index,
            base_offset,
        ) {
            Ok(ports) => ports,
            Err(error) => {
                eprintln!(
                    "{}",
                    format!("warning: port allocation failed for worktree {}: {error}", worktree.branch)
                        .yellow()
                );
                Vec::new()
            }
        };

        let status = get_container_status(&project_name).await;

        rows.push(WorktreeRow {
            index: worktree.index,
            branch: worktree.branch.clone(),
            status,
            project_name,
            ports,
        });
    }

    render_table(&rows, &context.port_mappings);
    Ok(())
}

#[derive(Debug)]
pub struct WorktreeRow {
    pub index: usize,
    pub branch: String,
    pub status: ContainerStatus,
    pub project_name: String,
    pub ports: Vec<PortAllocation>,
}

#[derive(Debug, PartialEq)]
pub enum ContainerStatus {
    Up,
    Down,
    Partial,
    Unknown,
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerStatus::Up => write!(f, "up"),
            ContainerStatus::Down => write!(f, "down"),
            ContainerStatus::Partial => write!(f, "partial"),
            ContainerStatus::Unknown => write!(f, "unknown"),
        }
    }
}

pub fn render_table(rows: &[WorktreeRow], port_mappings: &[PortMapping]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["#", "Branch", "Status", "Ports"]);

    for row in rows {
        let status_display = match row.status {
            ContainerStatus::Up => "up".green().to_string(),
            ContainerStatus::Down => "down".red().to_string(),
            ContainerStatus::Partial => "partial".yellow().to_string(),
            ContainerStatus::Unknown => "unknown".dimmed().to_string(),
        };

        let ports_display = if row.ports.is_empty() {
            "-".dimmed().to_string()
        } else {
            row.ports
                .iter()
                .map(|allocation| format!("{}={}", allocation.env_var, allocation.port))
                .collect::<Vec<_>>()
                .join(", ")
        };

        table.add_row(vec![
            row.index.to_string(),
            row.branch.clone(),
            status_display,
            ports_display,
        ]);
    }

    println!("{table}");

    if has_raw_port_warnings(port_mappings) {
        println!(
            "\n{}",
            "Warning: some ports use raw values without env vars. Use ${VAR:-default}:container format for port isolation."
                .yellow()
        );
    }
}

pub fn has_raw_port_warnings(port_mappings: &[PortMapping]) -> bool {
    port_mappings.iter().any(|mapping| mapping.env_var.is_none())
}

pub async fn get_container_status(project_name: &str) -> ContainerStatus {
    let output = tokio::process::Command::new("docker")
        .args(["compose", "-p", project_name, "ps", "--format", "json"])
        .output()
        .await;

    let output = match output {
        Ok(output) => output,
        Err(_) => return ContainerStatus::Unknown,
    };

    if !output.status.success() {
        return ContainerStatus::Down;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();

    if stdout.is_empty() {
        return ContainerStatus::Down;
    }

    let mut total = 0usize;
    let mut running = 0usize;

    for line in stdout.lines() {
        let parsed: std::result::Result<serde_json::Value, _> = serde_json::from_str(line);
        if let Ok(value) = parsed {
            total += 1;
            if value.get("State").and_then(|s| s.as_str()) == Some("running") {
                running += 1;
            }
        }
    }

    match (total, running) {
        (0, _) => ContainerStatus::Down,
        (t, r) if t == r => ContainerStatus::Up,
        (_, 0) => ContainerStatus::Down,
        _ => ContainerStatus::Partial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::PortMapping;

    #[test]
    fn detects_raw_port_warnings() {
        let mappings = vec![PortMapping {
            service_name: "web".to_string(),
            env_var: None,
            default_port: 3000,
            container_port: 3000,
            raw: "3000:3000".to_string(),
        }];
        assert!(has_raw_port_warnings(&mappings));
    }

    #[test]
    fn no_warnings_when_all_ports_have_env_vars() {
        let mappings = vec![PortMapping {
            service_name: "web".to_string(),
            env_var: Some("WEB_PORT".to_string()),
            default_port: 3000,
            container_port: 3000,
            raw: "${WEB_PORT:-3000}:3000".to_string(),
        }];
        assert!(!has_raw_port_warnings(&mappings));
    }

    #[test]
    fn container_status_display() {
        assert_eq!(ContainerStatus::Up.to_string(), "up");
        assert_eq!(ContainerStatus::Down.to_string(), "down");
        assert_eq!(ContainerStatus::Partial.to_string(), "partial");
        assert_eq!(ContainerStatus::Unknown.to_string(), "unknown");
    }
}
