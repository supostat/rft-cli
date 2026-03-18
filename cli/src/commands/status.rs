use crate::commands::list::{ContainerStatus, get_container_status};
use crate::context::{build_context, filter_worktrees};
use crate::error::Result;
use crate::sanitize::compose_project_name;

pub async fn run() -> Result<()> {
    let context = build_context().await?;
    let non_main = filter_worktrees(&context.worktrees, &[]);

    if non_main.is_empty() {
        println!("rft: no worktrees");
        return Ok(());
    }

    let total = non_main.len();
    let mut running_count = 0usize;

    for worktree in &non_main {
        let project_name = compose_project_name(
            &context.repo_name,
            worktree.index,
            &worktree.project_label(&context.config.project_name_source),
        );
        let status = get_container_status(&project_name).await;

        if status == ContainerStatus::Up || status == ContainerStatus::Partial {
            running_count += 1;
        }
    }

    let summary = match running_count {
        0 => "rft: all down".to_string(),
        count if count == total => format!("rft: all {total} up"),
        count => format!("rft: {count}/{total} up"),
    };

    println!("{summary}");

    Ok(())
}
