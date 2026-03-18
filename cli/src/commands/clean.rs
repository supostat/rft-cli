use owo_colors::OwoColorize;
use tokio::task::JoinSet;

use crate::context::build_context;
use crate::error::{Result, RftError};
use crate::sanitize::compose_project_name;

pub async fn run() -> Result<()> {
    let context = build_context().await?;

    let non_main: Vec<_> = context.worktrees.iter().filter(|wt| !wt.is_main).collect();

    if non_main.is_empty() {
        println!("{}", "Nothing to clean.".dimmed());
        return Ok(());
    }

    println!("{}", "Stopping all worktree stacks...".bold());

    let mut stop_set = JoinSet::new();
    for worktree in &non_main {
        let project_name = compose_project_name(
            &context.repo_name,
            worktree.index,
            &worktree.project_label(&context.config.project_name_source),
        );
        let branch = worktree.branch.clone();
        let index = worktree.index;
        let worktree_path = worktree.path.clone();

        stop_set.spawn(async move {
            super::stop::stop_single_project(&project_name, &worktree_path, index, &branch).await
        });
    }

    while let Some(result) = stop_set.join_next().await {
        if let Ok(Err(error)) = result {
            eprintln!("{}", format!("warning: stop failed: {error}").yellow());
        }
    }

    println!("{}", "Removing worktrees...".bold());

    for worktree in &non_main {
        println!(
            "{} {}",
            "Removing".red().bold(),
            format!("[{}] {}", worktree.index, worktree.branch).bold()
        );

        let output = tokio::process::Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&worktree.path)
            .current_dir(&context.repo_root)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            eprintln!(
                "{}",
                format!(
                    "warning: failed to remove worktree {}: {stderr}",
                    worktree.branch
                )
                .yellow()
            );
        }
    }

    let prune_output = tokio::process::Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(&context.repo_root)
        .output()
        .await?;

    if !prune_output.status.success() {
        let stderr = String::from_utf8_lossy(&prune_output.stderr).to_string();
        eprintln!(
            "{}",
            format!("warning: git worktree prune failed: {stderr}").yellow()
        );
    }

    println!("{}", "Cleaning Docker resources...".bold());

    let pattern = format!("{}-rft-*", crate::sanitize::sanitize(&context.repo_name));
    remove_docker_resources_by_pattern(&pattern).await?;

    println!("{}", "Clean complete.".green().bold());

    Ok(())
}

async fn remove_docker_resources_by_pattern(pattern: &str) -> Result<()> {
    let containers_output = tokio::process::Command::new("docker")
        .args(["ps", "-a", "--filter", &format!("name={pattern}"), "-q"])
        .output()
        .await?;

    let container_ids = String::from_utf8_lossy(&containers_output.stdout)
        .trim()
        .to_string();

    if !container_ids.is_empty() {
        let ids: Vec<&str> = container_ids.lines().collect();
        let mut rm_args = vec!["rm", "-f"];
        rm_args.extend(ids);

        let output = tokio::process::Command::new("docker")
            .args(rm_args)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(RftError::CommandFailed {
                cmd: "docker rm -f".to_string(),
                stderr,
            });
        }
    }

    let networks_output = tokio::process::Command::new("docker")
        .args([
            "network",
            "ls",
            "--filter",
            &format!("name={pattern}"),
            "-q",
        ])
        .output()
        .await?;

    let network_ids = String::from_utf8_lossy(&networks_output.stdout)
        .trim()
        .to_string();

    if !network_ids.is_empty() {
        for network_id in network_ids.lines() {
            let _ = tokio::process::Command::new("docker")
                .args(["network", "rm", network_id])
                .output()
                .await;
        }
    }

    let volumes_output = tokio::process::Command::new("docker")
        .args(["volume", "ls", "--filter", &format!("name={pattern}"), "-q"])
        .output()
        .await?;

    let volume_ids = String::from_utf8_lossy(&volumes_output.stdout)
        .trim()
        .to_string();

    if !volume_ids.is_empty() {
        for volume_id in volume_ids.lines() {
            let _ = tokio::process::Command::new("docker")
                .args(["volume", "rm", "-f", volume_id])
                .output()
                .await;
        }
    }

    Ok(())
}
