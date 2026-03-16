use owo_colors::OwoColorize;
use tokio::task::JoinSet;

use crate::context::{build_context, filter_worktrees};
use crate::error::{Result, RftError};
use crate::sanitize::compose_project_name;

pub async fn run(indices: Vec<usize>) -> Result<()> {
    let context = build_context().await?;
    let targets = filter_worktrees(&context.worktrees, &indices);

    if targets.is_empty() {
        println!("{}", "No matching worktrees found.".dimmed());
        return Ok(());
    }

    let mut join_set = JoinSet::new();

    for worktree in targets {
        let project_name =
            compose_project_name(&context.repo_name, worktree.index, &worktree.branch);
        let branch = worktree.branch.clone();
        let index = worktree.index;
        let worktree_path = worktree.path.clone();

        join_set.spawn(async move {
            stop_single_project(&project_name, &worktree_path, index, &branch).await
        });
    }

    let mut errors = Vec::new();

    loop {
        tokio::select! {
            result = join_set.join_next() => {
                match result {
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(error))) => errors.push(error),
                    Some(Err(join_error)) => {
                        errors.push(RftError::TaskPanicked(format!("{join_error}")));
                    }
                    None => break,
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\n{}", "Interrupted! Waiting for docker compose down to finish...".yellow().bold());
                // Don't abort stop tasks — let them finish to avoid zombie containers
                while let Some(result) = join_set.join_next().await {
                    if let Ok(Err(error)) = result {
                        errors.push(error);
                    }
                }
                break;
            }
        }
    }

    if !errors.is_empty() {
        for error in &errors {
            eprintln!("{}", format!("error: {error}").red());
        }
        let count = errors.len();
        return Err(RftError::Multiple { count });
    }

    Ok(())
}

pub async fn stop_single_project(
    project_name: &str,
    worktree_path: &std::path::Path,
    index: usize,
    branch: &str,
) -> Result<()> {
    println!(
        "{} {}",
        "Stopping".yellow().bold(),
        format!("[{index}] {branch} ({project_name})").bold()
    );

    let output = tokio::process::Command::new("docker")
        .args(["compose", "-p", project_name, "down"])
        .current_dir(worktree_path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("no configuration file")
            || stderr.contains("no such service")
            || stderr.contains("not found")
        {
            println!(
                "{} {}",
                "Already stopped".dimmed(),
                format!("[{index}] {branch}").dimmed()
            );
            return Ok(());
        }
        return Err(RftError::CommandFailed {
            cmd: format!("docker compose -p {project_name} down"),
            stderr,
        });
    }

    println!(
        "{} {}",
        "Stopped".yellow().bold(),
        format!("[{index}] {branch}").bold()
    );

    Ok(())
}
