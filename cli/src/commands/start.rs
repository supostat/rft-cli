use std::collections::HashMap;
use std::path::PathBuf;

use owo_colors::OwoColorize;
use tokio::task::JoinSet;

use crate::commands::list;
use crate::compose::ComposeFile;
use crate::context::{build_context, filter_worktrees};
use crate::error::{Result, RftError};
use crate::git::WorktreeInfo;
use crate::ports::check::check_ports;
use crate::ports::{BASE_OFFSET, PortMapping, allocate_worktree_ports};
use crate::sanitize::compose_project_name;
use crate::sync::{env, files};

struct WorktreeStartParams {
    repo_root: PathBuf,
    repo_name: String,
    worktree: WorktreeInfo,
    compose_file: ComposeFile,
    port_mappings: Vec<PortMapping>,
    extra_sync: Vec<String>,
    env_overrides: HashMap<String, String>,
    base_offset: u32,
}

pub async fn run(indices: Vec<usize>) -> Result<()> {
    let context = build_context().await?;
    let targets = filter_worktrees(&context.worktrees, &indices);

    if targets.is_empty() {
        println!("{}", "No matching worktrees found.".dimmed());
        return Ok(());
    }

    let base_offset = context.config.port_offset.unwrap_or(BASE_OFFSET);

    let mut all_allocations = Vec::new();
    for worktree in &targets {
        if let Ok(allocations) =
            allocate_worktree_ports(&context.port_mappings, worktree.index, base_offset)
        {
            all_allocations.extend(allocations);
        }
    }

    let conflicts = check_ports(&all_allocations);
    if !conflicts.is_empty() {
        for conflict in &conflicts {
            eprintln!(
                "{}",
                format!(
                    "warning: port {} ({}, {}) is already in use",
                    conflict.port, conflict.service_name, conflict.env_var
                )
                .yellow()
            );
        }
    }

    let mut join_set = JoinSet::new();

    for worktree in targets {
        let params = WorktreeStartParams {
            repo_root: context.repo_root.clone(),
            repo_name: context.repo_name.clone(),
            worktree: worktree.clone(),
            compose_file: context.compose_file.clone(),
            port_mappings: context.port_mappings.clone(),
            extra_sync: context.config.sync.clone(),
            env_overrides: context.config.env_overrides.clone(),
            base_offset,
        };

        join_set.spawn(async move { start_single_worktree(params).await });
    }

    let mut errors = Vec::new();

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(error),
            Err(join_error) => {
                errors.push(RftError::TaskPanicked(format!("{join_error}")));
            }
        }
    }

    if !errors.is_empty() {
        for error in &errors {
            eprintln!("{}", format!("error: {error}").red());
        }
        let count = errors.len();
        return Err(RftError::Config(format!(
            "{count} worktree(s) failed to start"
        )));
    }

    println!();
    list::run_inner().await?;

    Ok(())
}

async fn start_single_worktree(params: WorktreeStartParams) -> Result<()> {
    let project_name = compose_project_name(
        &params.repo_name,
        params.worktree.index,
        &params.worktree.branch,
    );

    println!(
        "{} {}",
        "Starting".green().bold(),
        format!(
            "[{}] {} ({})",
            params.worktree.index, params.worktree.branch, project_name
        )
        .bold()
    );

    let allocations = allocate_worktree_ports(
        &params.port_mappings,
        params.worktree.index,
        params.base_offset,
    )?;

    files::sync_worktree_files(
        &params.repo_root,
        &params.worktree.path,
        &params.compose_file,
        &params.extra_sync,
    )
    .await?;

    let env_path = env::copy_base_env(&params.repo_root, &params.worktree.path).await?;
    env::inject_port_overrides(&env_path, &allocations, &params.env_overrides).await?;

    let output = tokio::process::Command::new("docker")
        .args(["compose", "-p", &project_name, "up", "-d", "--build"])
        .current_dir(&params.worktree.path)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(RftError::CommandFailed {
            cmd: format!("docker compose -p {project_name} up -d --build"),
            stderr,
        });
    }

    println!(
        "{} {}",
        "Started".green().bold(),
        format!("[{}] {}", params.worktree.index, params.worktree.branch).bold()
    );

    Ok(())
}
