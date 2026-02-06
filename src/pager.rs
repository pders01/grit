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

/// Write content to a temp file and open it in the pager.
/// The pager gets the file path as an argument so it can mmap/read directly -
/// no IPC piping overhead.
pub fn open_pager(content: &str, pager_cmd: &str) -> std::io::Result<()> {
    let tmp = std::env::temp_dir().join(format!("grit-diff-{}.diff", std::process::id()));
    std::fs::write(&tmp, content)?;

    let status = Command::new("sh")
        .args(["-c", &format!("{} {}", pager_cmd, tmp.display())])
        .status();

    let _ = std::fs::remove_file(&tmp);
    status?;
    Ok(())
}
