use miette::Result;
use owo_colors::OwoColorize;

use crate::context::{build_context, filter_worktrees};
use crate::ports::{BASE_OFFSET, PortAllocation, PortMapping, allocate_worktree_ports};
use crate::sanitize::compose_project_name;

const PADDING: usize = 2;
const MIN_WIDTH: usize = 50;

pub async fn run() -> Result<()> {
    run_inner().await.map_err(miette::Report::new)
}

pub async fn run_inner() -> crate::error::Result<()> {
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
            &worktree.project_label(&context.config.project_name_source),
        );
        let ports =
            match allocate_worktree_ports(&context.port_mappings, worktree.index, base_offset) {
                Ok(ports) => ports,
                Err(error) => {
                    eprintln!(
                        "{}",
                        format!(
                            "warning: port allocation failed for worktree {}: {error}",
                            worktree.branch
                        )
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

    render_bordered(
        &rows,
        &context.port_mappings,
        &context.config.host,
        &context.repo_name,
    );
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

pub async fn list_as_text() -> crate::error::Result<String> {
    let context = build_context().await?;
    let non_main = filter_worktrees(&context.worktrees, &[]);

    if non_main.is_empty() {
        return Ok("No worktrees found (only main branch).".to_string());
    }

    let base_offset = context.config.port_offset.unwrap_or(BASE_OFFSET);
    let mut rows = Vec::new();

    for worktree in &non_main {
        let project_name = compose_project_name(
            &context.repo_name,
            worktree.index,
            &worktree.project_label(&context.config.project_name_source),
        );
        let ports = allocate_worktree_ports(&context.port_mappings, worktree.index, base_offset)
            .unwrap_or_default();

        let status = get_container_status(&project_name).await;

        rows.push(WorktreeRow {
            index: worktree.index,
            branch: worktree.branch.clone(),
            status,
            project_name,
            ports,
        });
    }

    Ok(render_table_as_text(&rows))
}

fn render_table_as_text(rows: &[WorktreeRow]) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "{:<4} {:<30} {:<10} {}",
        "#", "Branch", "Status", "Ports"
    ));
    lines.push("-".repeat(80));

    for row in rows {
        let ports_display = if row.ports.is_empty() {
            "-".to_string()
        } else {
            row.ports
                .iter()
                .map(|allocation| format!("{}={}", allocation.env_var, allocation.port))
                .collect::<Vec<_>>()
                .join(", ")
        };

        lines.push(format!(
            "{:<4} {:<30} {:<10} {}",
            row.index, row.branch, row.status, ports_display
        ));
    }

    lines.join("\n")
}

fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
        .max(MIN_WIDTH)
}

fn port_hyperlink(port: u16, host: &str) -> String {
    format!("\x1b]8;;http://{host}:{port}\x1b\\{port}\x1b]8;;\x1b\\")
}

fn format_port(allocation: &PortAllocation, host: &str) -> String {
    format!(
        "{}={}",
        allocation.env_var,
        port_hyperlink(allocation.port, host)
    )
}

fn visible_port_width(allocation: &PortAllocation) -> usize {
    allocation.env_var.len() + 1 + allocation.port.to_string().len()
}

fn status_styled(status: &ContainerStatus) -> (String, usize) {
    match status {
        ContainerStatus::Up => (format!("{}", "● up".green()), 4),
        ContainerStatus::Down => (format!("{}", "○ down".red()), 6),
        ContainerStatus::Partial => (format!("{}", "◐ partial".yellow()), 9),
        ContainerStatus::Unknown => (format!("{}", "○ unknown".dimmed()), 9),
    }
}

struct ColumnWidths {
    branch: usize,
    status: usize,
    ports: usize,
    total: usize,
}

fn calculate_columns(rows: &[WorktreeRow], max_width: usize) -> ColumnWidths {
    let status_width = 12; // "◐ partial" + padding
    let borders = 4; // │ between 3 cols + outer │

    let branch_natural = rows
        .iter()
        .map(|r| format!("[{}] {}", r.index, r.branch).len())
        .max()
        .unwrap_or(10)
        + PADDING * 2;

    let ports_natural = rows
        .iter()
        .flat_map(|r| r.ports.iter())
        .map(visible_port_width)
        .max()
        .unwrap_or(5)
        + PADDING * 2;

    let content_width = (branch_natural + status_width + ports_natural + borders).min(max_width);

    let min_ports = 28;
    let min_branch = 20; // "[1] feature/auth" + padding
    let max_branch = content_width.saturating_sub(borders + status_width + min_ports);
    let branch_width = branch_natural.min(max_branch).max(min_branch);
    let ports_width = content_width.saturating_sub(borders + branch_width + status_width);

    ColumnWidths {
        branch: branch_width,
        status: status_width,
        ports: ports_width,
        total: branch_width + status_width + ports_width + borders,
    }
}

fn print_border_row(cols: &ColumnWidths, left: &str, mid: &str, right: &str) {
    println!(
        "{}",
        format!(
            "{}{}{}{}{}{}{}",
            left,
            "─".repeat(cols.branch),
            mid,
            "─".repeat(cols.status),
            mid,
            "─".repeat(cols.ports),
            right,
        )
        .dimmed()
    );
}

fn print_cell_row(
    cols: &ColumnWidths,
    branch_content: &str,
    branch_visible_len: usize,
    status_content: &str,
    status_visible_len: usize,
    port_content: &str,
    port_visible_len: usize,
) {
    let b_pad = cols.branch.saturating_sub(branch_visible_len);
    let s_pad = cols.status.saturating_sub(status_visible_len);
    let p_pad = cols.ports.saturating_sub(port_visible_len);

    let port_padded = format!(" {}{}", port_content, " ".repeat(p_pad.saturating_sub(1)));
    println!(
        "{}{}{}{}{}{}{}{}{}",
        "│".dimmed(),
        branch_content,
        " ".repeat(b_pad),
        "│".dimmed(),
        status_content,
        " ".repeat(s_pad),
        "│".dimmed(),
        port_padded,
        "│".dimmed(),
    );
}

pub fn render_bordered(
    rows: &[WorktreeRow],
    port_mappings: &[PortMapping],
    host: &str,
    repo_name: &str,
) {
    let width = terminal_width();
    let cols = calculate_columns(rows, width);

    // Top border with title
    let title = format!(" rft \u{2022} {} ", repo_name); // • = U+2022
    let title_display_width = " rft . ".len() + repo_name.len() + 1; // • = 1 col
    let top_remaining = cols.total.saturating_sub(3 + title_display_width); // ╭─ + ╮
    println!(
        "{}{}{}{}",
        "╭─".dimmed(),
        title.bold(),
        "─".repeat(top_remaining).dimmed(),
        "╮".dimmed(),
    );

    // Header row
    let branch_header = format!(
        "  {:<w$}",
        "Branch",
        w = cols.branch.saturating_sub(PADDING)
    );
    let status_header = format!(" {:<w$}", "Status", w = cols.status.saturating_sub(1));
    let ports_header = format!(" {:<w$}", "Ports", w = cols.ports.saturating_sub(1));
    println!(
        "{}{}{}{}{}{}{}",
        "│".dimmed(),
        branch_header.bold(),
        "│".dimmed(),
        status_header.bold(),
        "│".dimmed(),
        ports_header.bold(),
        "│".dimmed(),
    );

    // Header separator
    print_border_row(&cols, "├", "┼", "┤");

    for (i, row) in rows.iter().enumerate() {
        let (styled_status, status_display_width) = status_styled(&row.status);
        let branch_text = format!("  [{}] {}", row.index, row.branch);
        let branch_visible_len = branch_text.len();

        // Truncate branch if needed
        let branch_display = if branch_visible_len > cols.branch {
            let max = cols.branch.saturating_sub(1);
            format!("{}…", &branch_text[..max.saturating_sub(1)])
                .bold()
                .to_string()
        } else {
            branch_text.bold().to_string()
        };
        let branch_vis = branch_visible_len.min(cols.branch);

        let status_text = format!(" {}", styled_status);
        let status_vis = status_display_width + 1;

        if row.ports.is_empty() {
            print_cell_row(
                &cols,
                &branch_display,
                branch_vis,
                &status_text,
                status_vis,
                "-",
                1,
            );
        } else {
            // First port on the same line as branch+status
            let first_port = format_port(&row.ports[0], host);
            let first_vis = visible_port_width(&row.ports[0]);
            print_cell_row(
                &cols,
                &branch_display,
                branch_vis,
                &status_text,
                status_vis,
                &first_port,
                first_vis,
            );

            // Remaining ports on separate lines
            for allocation in &row.ports[1..] {
                let port = format_port(allocation, host);
                let vis = visible_port_width(allocation);
                print_cell_row(&cols, "", 0, "", 0, &port, vis);
            }
        }

        if i < rows.len() - 1 {
            print_border_row(&cols, "├", "┼", "┤");
        }
    }

    // Bottom border
    println!(
        "{}{}{}{}{}{}{}",
        "╰".dimmed(),
        "─".repeat(cols.branch).dimmed(),
        "┴".dimmed(),
        "─".repeat(cols.status).dimmed(),
        "┴".dimmed(),
        "─".repeat(cols.ports).dimmed(),
        "╯".dimmed(),
    );

    if has_raw_port_warnings(port_mappings) {
        println!(
            "\n{}",
            "Warning: some ports use raw values without env vars. Use ${VAR:-default}:container format for port isolation."
                .yellow()
        );
    }
}

pub fn has_raw_port_warnings(port_mappings: &[PortMapping]) -> bool {
    port_mappings
        .iter()
        .any(|mapping| mapping.env_var.is_none())
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
