use std::path::Path;
use std::process::Command;

fn rft_binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rft"))
}

fn setup_test_repo(dir: &Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .expect("git config email");

    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .expect("git config name");

    std::fs::write(
        dir.join("docker-compose.yml"),
        r#"services:
  web:
    image: nginx
    ports:
      - "${WEB_PORT:-3000}:80"
  api:
    image: nginx
    ports:
      - "${API_PORT:-8080}:80"
"#,
    )
    .expect("write compose");

    std::fs::write(dir.join(".env"), "WEB_PORT=3000\nAPI_PORT=8080\n").expect("write env");

    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add");

    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir)
        .output()
        .expect("git commit");
}

fn create_worktree(repo: &Path, name: &str, branch: &str) {
    let worktree_path = repo.parent().unwrap().join(name);
    Command::new("git")
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            "-b",
            branch,
        ])
        .current_dir(repo)
        .output()
        .expect("git worktree add");
}

#[test]
fn version_flag() {
    let output = rft_binary().arg("--version").output().expect("run rft");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rft"));
    assert!(output.status.success());
}

#[test]
fn list_shows_worktrees() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    setup_test_repo(&repo);
    create_worktree(&repo, "wt-feature", "feature/test");

    let output = rft_binary()
        .arg("list")
        .current_dir(&repo)
        .output()
        .expect("run rft list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("feature/test"),
        "expected worktree branch in output: {stdout}"
    );
    assert!(
        stdout.contains("WEB_PORT") || stdout.contains("23001"),
        "expected port info in output: {stdout}"
    );
}

#[test]
fn list_no_worktrees_shows_message() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    setup_test_repo(&repo);

    let output = rft_binary()
        .arg("list")
        .current_dir(&repo)
        .output()
        .expect("run rft list");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No worktrees") || stdout.contains("only main"),
        "expected no-worktrees message: {stdout}"
    );
}

#[test]
fn init_detects_compose() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    setup_test_repo(&repo);

    let output = rft_binary()
        .arg("init")
        .current_dir(&repo)
        .output()
        .expect("run rft init");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("docker-compose.yml"),
        "should detect compose file: {stdout}"
    );
    assert!(
        stdout.contains("WEB_PORT") || stdout.contains("managed"),
        "should show port info: {stdout}"
    );
}

#[test]
fn init_without_compose_shows_error() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "t@t.com"])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "T"])
        .current_dir(&repo)
        .output()
        .unwrap();
    std::fs::write(repo.join("README.md"), "test").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo)
        .output()
        .unwrap();

    let output = rft_binary()
        .arg("init")
        .current_dir(&repo)
        .output()
        .expect("run rft init");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No compose file"),
        "should report missing compose: {stderr}"
    );
}

#[test]
fn status_outside_repo() {
    let dir = tempfile::tempdir().unwrap();

    let output = rft_binary()
        .arg("status")
        .current_dir(dir.path())
        .output()
        .expect("run rft status");

    assert!(!output.status.success());
}

#[test]
fn completions_generates_output() {
    let output = rft_binary()
        .args(["completions", "bash"])
        .output()
        .expect("run rft completions");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("rft"),
        "should contain rft in completions: {stdout}"
    );
    assert!(output.status.success());
}

#[test]
fn dry_run_does_not_start_docker() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    setup_test_repo(&repo);
    create_worktree(&repo, "wt-dry", "feature/dry");

    let output = rft_binary()
        .args(["start", "--dry-run"])
        .current_dir(&repo)
        .output()
        .expect("run rft start --dry-run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("copy") || stdout.contains("exec") || stdout.contains("docker"),
        "dry-run should show planned actions: {stdout}"
    );
}
