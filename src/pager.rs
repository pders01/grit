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

    let mut child = Command::new("sh")
        .args(["-c", pager_cmd])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        // Write all content then drop to close the pipe (signals EOF)
        let _ = stdin.write_all(content.as_bytes());
    }

    child.wait()?;
    Ok(())
}
