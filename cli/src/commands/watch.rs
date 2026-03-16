use std::path::PathBuf;
use std::time::Duration;

use notify::{EventKind, RecursiveMode, Watcher};
use owo_colors::OwoColorize;
use tokio::sync::mpsc;

use crate::compose::ComposeFile;
use crate::context::build_context;
use crate::error::Result;
use crate::sync::files::extract_dockerfile_paths;

const DEBOUNCE_MS: u64 = 500;

pub async fn run(indices: Vec<usize>) -> Result<()> {
    let context = build_context().await?;

    println!(
        "{} Starting stacks and watching for changes...",
        "Watch".cyan().bold()
    );

    super::start::run(indices.clone()).await?;

    let paths_to_watch = collect_watch_paths(&context.compose_file);

    println!(
        "{} Watching {} path(s) for changes. Press Ctrl+C to stop.",
        "Watch".cyan().bold(),
        paths_to_watch.len()
    );

    for path in &paths_to_watch {
        println!("  {} {}", "→".dimmed(), path.display().to_string().dimmed());
    }

    let (fs_sender, mut fs_receiver) = mpsc::channel::<()>(16);

    let _watcher = create_file_watcher(&paths_to_watch, fs_sender)?;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!(
                    "\n{} Stopping all stacks...",
                    "Watch".cyan().bold()
                );
                let _ = super::stop::run(indices.clone()).await;
                return Ok(());
            }
            _ = wait_for_change(&mut fs_receiver) => {
                println!(
                    "\n{} File change detected, restarting stacks...",
                    "Watch".cyan().bold()
                );
                let _ = super::restart::run(indices.clone()).await;
            }
        }
    }
}

fn collect_watch_paths(compose_file: &ComposeFile) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    paths.push(compose_file.compose_path.clone());

    for dockerfile_path in extract_dockerfile_paths(compose_file) {
        if let Some(parent) = dockerfile_path.parent() {
            let parent_buf = parent.to_path_buf();
            if !paths.contains(&parent_buf) {
                paths.push(parent_buf);
            }
        }
    }

    paths
}

fn create_file_watcher(paths: &[PathBuf], sender: mpsc::Sender<()>) -> Result<impl Watcher> {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event
            && matches!(
                event.kind,
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
            )
        {
            let _ = sender.try_send(());
        }
    })
    .map_err(|error| {
        crate::error::RftError::Config(format!("failed to create watcher: {error}"))
    })?;

    for path in paths {
        if path.exists() {
            let mode = if path.is_dir() {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };
            if let Err(error) = watcher.watch(path, mode) {
                eprintln!(
                    "{}",
                    format!("warning: cannot watch {}: {error}", path.display()).yellow()
                );
            }
        }
    }

    Ok(watcher)
}

async fn wait_for_change(receiver: &mut mpsc::Receiver<()>) {
    receiver.recv().await;

    // Debounce: drain additional events within the window
    let deadline = tokio::time::Instant::now() + Duration::from_millis(DEBOUNCE_MS);
    loop {
        tokio::select! {
            _ = receiver.recv() => {}
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }
}
