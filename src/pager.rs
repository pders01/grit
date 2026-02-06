use std::process::Command;

/// Detect the user's preferred pager.
/// Checks GIT_PAGER -> git config core.pager -> PAGER -> "less"
pub fn detect_pager() -> String {
    if let Ok(pager) = std::env::var("GIT_PAGER") {
        if !pager.is_empty() {
            return pager;
        }
    }

    if let Ok(output) = Command::new("git").args(["config", "core.pager"]).output() {
        if output.status.success() {
            let pager = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !pager.is_empty() {
                return pager;
            }
        }
    }

    if let Ok(pager) = std::env::var("PAGER") {
        if !pager.is_empty() {
            return pager;
        }
    }

    "less".to_string()
}

/// Pipe content to the pager's stdin, the same way git does it.
/// This works with all pagers (less, delta, bat, etc.) since they all
/// read from stdin when used as a pager.
pub fn open_pager(content: &str, pager_cmd: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    // Delta only pages when content exceeds the viewport height by default.
    // Force --paging=always so short diffs don't flash and vanish when we
    // restore the TUI.
    let cmd = ensure_paging_always(pager_cmd);

    let mut child = Command::new("sh")
        .args(["-c", &cmd])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        // Write all content then drop to close the pipe (signals EOF)
        let _ = stdin.write_all(content.as_bytes());
    }

    child.wait()?;
    Ok(())
}

/// If the pager command invokes delta without an explicit --paging flag,
/// append `--paging=always` so it always spawns its internal pager.
fn ensure_paging_always(pager_cmd: &str) -> String {
    // Check if any whitespace-separated token is "delta" (bare command or trailing path component).
    let has_delta = pager_cmd
        .split_whitespace()
        .any(|tok| tok == "delta" || tok.ends_with("/delta"));

    if has_delta && !pager_cmd.contains("--paging") {
        format!("{} --paging=always", pager_cmd)
    } else {
        pager_cmd.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_delta_gets_paging_always() {
        assert_eq!(ensure_paging_always("delta"), "delta --paging=always");
    }

    #[test]
    fn delta_with_args_gets_paging_always() {
        assert_eq!(
            ensure_paging_always("delta --dark --side-by-side"),
            "delta --dark --side-by-side --paging=always"
        );
    }

    #[test]
    fn delta_with_explicit_paging_unchanged() {
        let cmd = "delta --paging=never";
        assert_eq!(ensure_paging_always(cmd), cmd);
    }

    #[test]
    fn absolute_path_delta_gets_paging_always() {
        assert_eq!(
            ensure_paging_always("/usr/local/bin/delta"),
            "/usr/local/bin/delta --paging=always"
        );
    }

    #[test]
    fn less_unchanged() {
        assert_eq!(ensure_paging_always("less"), "less");
    }

    #[test]
    fn bat_unchanged() {
        assert_eq!(
            ensure_paging_always("bat --style=plain"),
            "bat --style=plain"
        );
    }

    #[test]
    fn delta_in_unrelated_word_unchanged() {
        // "deltaforce" shouldn't match
        assert_eq!(ensure_paging_always("deltaforce"), "deltaforce");
    }
}
