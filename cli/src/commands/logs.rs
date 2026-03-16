use owo_colors::OwoColorize;

use crate::context::build_context;
use crate::error::Result;
use crate::git::get_worktree_by_index;
use crate::sanitize::compose_project_name;

pub async fn run(index: usize, service: Option<String>, no_follow: bool) -> Result<()> {
    let context = build_context().await?;
    let worktree = get_worktree_by_index(&context.repo_root, index).await?;
    let project_name = compose_project_name(&context.repo_name, worktree.index, &worktree.branch);

    let mut args = vec![
        "compose".to_string(),
        "-p".to_string(),
        project_name.clone(),
        "logs".to_string(),
        "--tail".to_string(),
        "100".to_string(),
    ];

    if !no_follow {
        args.push("--follow".to_string());
    }

    if let Some(ref service_name) = service {
        args.push(service_name.clone());
    }

    println!(
        "{} {}",
        "Logs".cyan().bold(),
        format!(
            "[{}] {} ({})",
            worktree.index, worktree.branch, project_name
        )
        .bold()
    );

    let status = tokio::process::Command::new("docker")
        .args(&args)
        .current_dir(&worktree.path)
        .status()
        .await?;

    if !status.success() {
        return Err(crate::error::RftError::CommandFailed {
            cmd: format!("docker {}", args.join(" ")),
            stderr: "exited with non-zero status".to_string(),
        });
    }

    Ok(())
}
