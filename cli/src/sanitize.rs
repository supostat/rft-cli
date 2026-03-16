/// Lowercase, replace non-alphanumeric with `-`, trim leading/trailing dashes, collapse consecutive dashes.
pub fn sanitize(input: &str) -> String {
    let lowered = input.to_lowercase();

    let replaced: String = lowered
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();

    collapse_and_trim_dashes(&replaced)
}

/// Generate Docker Compose project name: `{repo}-rft-{index}-{branch}`.
pub fn compose_project_name(repo: &str, index: usize, branch: &str) -> String {
    let sanitized_repo = sanitize(repo);
    let sanitized_branch = sanitize(branch);
    format!("{sanitized_repo}-rft-{index}-{sanitized_branch}")
}

fn collapse_and_trim_dashes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut previous_was_dash = false;

    for character in input.chars() {
        if character == '-' {
            if !previous_was_dash {
                result.push('-');
            }
            previous_was_dash = true;
        } else {
            result.push(character);
            previous_was_dash = false;
        }
    }

    result.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_simple_string() {
        assert_eq!(sanitize("hello-world"), "hello-world");
    }

    #[test]
    fn converts_to_lowercase() {
        assert_eq!(sanitize("Hello-World"), "hello-world");
    }

    #[test]
    fn replaces_special_characters() {
        assert_eq!(sanitize("feature/add_auth"), "feature-add-auth");
    }

    #[test]
    fn collapses_consecutive_dashes() {
        assert_eq!(sanitize("a---b"), "a-b");
    }

    #[test]
    fn trims_leading_and_trailing_dashes() {
        assert_eq!(sanitize("--hello--"), "hello");
    }

    #[test]
    fn handles_dots_and_at_signs() {
        assert_eq!(sanitize("user@host.com"), "user-host-com");
    }

    #[test]
    fn handles_empty_string() {
        assert_eq!(sanitize(""), "");
    }

    #[test]
    fn handles_only_special_characters() {
        assert_eq!(sanitize("@#$%"), "");
    }

    #[test]
    fn generates_compose_project_name() {
        let name = compose_project_name("my-repo", 2, "feature/auth");
        assert_eq!(name, "my-repo-rft-2-feature-auth");
    }

    #[test]
    fn compose_name_sanitizes_all_parts() {
        let name = compose_project_name("My_Repo", 1, "fix/ISSUE-123");
        assert_eq!(name, "my-repo-rft-1-fix-issue-123");
    }

    #[test]
    fn compose_name_with_complex_branch() {
        let name = compose_project_name("app", 0, "refs/heads/feature/foo@bar");
        assert_eq!(name, "app-rft-0-refs-heads-feature-foo-bar");
    }
}
