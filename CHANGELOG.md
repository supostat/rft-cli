# Changelog

All notable changes to this project will be documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

## [0.6.0] — 2026-03-18

### Added
- `project_name_source` config: `"directory"` (default) or `"branch"` for Docker project naming
- `host` config option for clickable port links (default: `localhost`), env `RFT_HOST`
- `.rftrc.local.toml` — personal config (gitignored), merges over shared `.rftrc.toml`
- `rft init` adds `.rftrc.local.toml` to `.gitignore` automatically

### Changed
- ratatui-style 3-column table for `rft list` with rounded corners, title border, dim borders
- Table width fits content, not terminal width
- OSC 8 clickable port links (cmd+click opens browser)
- Status icons: `● up`, `○ down`, `◐ partial`
- Start/stop output: `▶`/`■`/`✓` icons, no Docker project name
- Default `project_name_source` is `"directory"` — stable Docker volumes across branch switches

### Fixed
- Self-copy truncation when source == destination (bare-repo single worktree)
- Top-right corner alignment in bordered table

## [0.5.3] — 2026-03-18

### Changed
- Bordered layout for `rft list` with box-drawing characters
- Adaptive port wrapping based on terminal width
- Branch names truncated with `…` when terminal is narrow

## [0.5.2] — 2026-03-17

### Changed
- Card layout for `rft list` (replaces comfy-table)
- Ports grouped 3 per line with OSC 8 clickable links (cmd+click opens browser)
- Removed `comfy-table` dependency

## [0.5.1] — 2026-03-17

### Fixed
- Table rendering broken by OSC 8 hyperlinks (comfy-table incompatible with escape sequences)

## [0.5.0] — 2026-03-17

### Added
- `.rftrc.local.toml` — personal config (gitignored), merges over shared `.rftrc.toml`
- `host` config option (default: `localhost`), env `RFT_HOST`
- Clickable port numbers in `rft list` (OSC 8 hyperlinks, cmd+click opens browser)
- `rft init` adds `.rftrc.local.toml` to `.gitignore` automatically

### Changed
- Ports displayed vertically (one per line) instead of comma-separated
- Status indicators: `● up`, `○ down`, `◐ partial` instead of plain text
- Start/stop output: `▶`/`■`/`✓` icons, no Docker project name

## [0.4.2] — 2026-03-17

### Fixed
- File truncation when syncing compose file to the same worktree (source == destination)

## [0.4.1] — 2026-03-17

### Added
- Bare-repo worktree support — stable project naming from any worktree via `git-common-dir`
- Three repo patterns: normal clone, bare+`.bare` trick, plain bare clone
- Run rft from bare repo root (auto-fallback to main worktree)
- `NoMainWorktree` error with actionable hint
- Plain bare clone documented in getting-started

### Changed
- `get_repo_root`/`get_repo_name` replaced by `resolve_repo_identity`
- Docker project names stable regardless of which worktree rft runs from

## [0.4.0] — 2026-03-16

### Added
- `rft init` — analyze compose file, suggest port fixes, generate `.rftrc.toml`
- 8 integration tests (version, list, init, status, completions, dry-run)
- Troubleshooting page on documentation website (9 common issues)
- Command pages for init and watch on website
- All 12 commands linked in website command index
- README badges (crates.io, CI, license)

### Changed
- `LazyLock<Regex>` for port extraction (compiled once, not per call)
- `RftError::Multiple` for aggregate start/stop errors (was `Config`)
- Re-export `suggest_env_var` from ports module

### Removed
- Dead code `get_non_main_worktrees`

## [0.3.0] — 2026-03-16

### Added
- `rft start --dry-run` — show what would happen without executing (Executor abstraction)
- `rft watch` — start stacks and auto-restart on compose/Dockerfile changes (notify + 500ms debounce)
- Graceful shutdown on Ctrl+C for `start` (cleanup partially started stacks) and `stop` (wait for docker compose down to finish)
- CHANGELOG.md

## [0.2.0] — 2026-03-16

### Added
- Bare repo support — `git worktree list` skips bare entries, works with `git clone --bare` + worktree workflow
- `main_branch` config option — override default main/master detection (for repos using develop/trunk)
- Path traversal validation in file sync and promote (`validate_path_within`)
- Getting started guide with bare repo walkthrough
- 4 new command doc pages: restart, status, completions, mcp
- `meta.json` for sidebar ordering in documentation website

### Fixed
- Port conflict detection moved before parallel start (was inside each task — race condition)
- `expect()` in `spawn_blocking` replaced with `RftError::TaskPanicked`
- `get_worktree_by_index` rejects main worktree (index 0)
- Main branch detected by name (main/master), not by position in worktree list
- Aggregated error messages for parallel start/stop failures
- `--dry-run` flag removed from start CLI (was accepted but ignored)
- Shell prompt example uses `precmd()` instead of static `export PS1`
- GitHub URL consistency across Cargo.toml, website, README
- MCP docs removed "Stream logs" from use cases (no `rft_logs` tool)
- `is_excluded` in promote checks basename for nested paths

### Changed
- `RftError::Interrupted` variant for Ctrl+C (was `Config`)
- `RftError::PromoteConflict` variant for promote conflicts (was `Config`)

## [0.1.0] — 2026-03-16

### Added
- CLI with 10 commands: list, start, stop, restart, clean, logs, promote, status, completions, mcp
- Docker Compose port isolation for git worktrees
- Deterministic port allocation: `20000 + default_port + worktree_index`
- Parallel start/stop via `tokio::JoinSet`
- File sync: compose, Dockerfiles, extra files, `.env` with port override block
- Promote: `git checkout` for committed files, file copy for uncommitted/untracked
- MCP server with 6 tools via `rmcp` stdio transport
- Port conflict detection via `TcpListener::bind`
- Shell completions (bash, zsh, fish, powershell)
- One-line status for shell prompt (`rft status`)
- Configuration: `.rftrc.toml`, `.rftrc.json`, `package.json#rft`, env vars
- CI/CD: test on 3 OS, release builds for linux/macos/windows
- Lefthook: pre-commit (fmt + clippy), pre-push (test)
- Fumadocs documentation website (22 pages)
- Example Docker Compose project

[Unreleased]: https://github.com/supostat/rft-cli/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/supostat/rft-cli/compare/v0.5.3...v0.6.0
[0.5.3]: https://github.com/supostat/rft-cli/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/supostat/rft-cli/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/supostat/rft-cli/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/supostat/rft-cli/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/supostat/rft-cli/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/supostat/rft-cli/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/supostat/rft-cli/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/supostat/rft-cli/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/supostat/rft-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/supostat/rft-cli/releases/tag/v0.1.0
