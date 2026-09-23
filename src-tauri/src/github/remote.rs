/// Extracts `(owner, repo)` from a GitHub remote URL, supporting the forms
/// git actually produces: `https://github.com/owner/repo.git`,
/// `git@github.com:owner/repo.git`, and `ssh://git@github.com/owner/repo`.
/// Returns `None` for anything not hosted on github.com (e.g. GitLab, a
/// bare local path) — callers treat that as "no PR integration here".
pub fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let after_host = trimmed
        .strip_prefix("git@github.com:")
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
        .or_else(|| trimmed.strip_prefix("https://github.com/"))
        .or_else(|| trimmed.strip_prefix("http://github.com/"))?;

    let mut parts = after_host.splitn(2, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_github_remote_forms() {
        assert_eq!(
            parse_github_remote("https://github.com/facebook/react.git"),
            Some(("facebook".to_string(), "react".to_string()))
        );
        assert_eq!(
            parse_github_remote("https://github.com/facebook/react"),
            Some(("facebook".to_string(), "react".to_string()))
        );
        assert_eq!(
            parse_github_remote("git@github.com:facebook/react.git"),
            Some(("facebook".to_string(), "react".to_string()))
        );
        assert_eq!(
            parse_github_remote("ssh://git@github.com/facebook/react.git"),
            Some(("facebook".to_string(), "react".to_string()))
        );
        assert_eq!(
            parse_github_remote("https://github.com/facebook/react/"),
            Some(("facebook".to_string(), "react".to_string()))
        );
    }

    #[test]
    fn rejects_non_github_or_malformed_urls() {
        assert_eq!(parse_github_remote("https://gitlab.com/owner/repo.git"), None);
        assert_eq!(parse_github_remote("C:\\repos\\local-repo"), None);
        assert_eq!(parse_github_remote("https://github.com/only-owner"), None);
    }
}
