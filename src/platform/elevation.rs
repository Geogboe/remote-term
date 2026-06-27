#[cfg(windows)]
use anyhow::Context;
use anyhow::ensure;

pub fn ensure_session_allowed(allow_elevated: bool) -> anyhow::Result<()> {
    ensure_session_allowed_for(is_elevated()?, allow_elevated)
}

fn ensure_session_allowed_for(elevated: bool, allow_elevated: bool) -> anyhow::Result<()> {
    ensure!(
        !elevated || allow_elevated,
        "refusing to start a terminal session as root/administrator; rerun with \
         --allow-elevated only if the elevated child session is intentional"
    );
    Ok(())
}

#[cfg(windows)]
fn is_elevated() -> anyhow::Result<bool> {
    check_elevation::is_elevated().context("failed to inspect the Windows process token")
}

#[cfg(unix)]
fn is_elevated() -> anyhow::Result<bool> {
    // SAFETY: geteuid has no arguments and no preconditions.
    Ok(unsafe { libc::geteuid() == 0 })
}

#[cfg(not(any(windows, unix)))]
fn is_elevated() -> anyhow::Result<bool> {
    anyhow::bail!("elevation detection is unsupported on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevated_session_requires_explicit_bypass() {
        assert!(ensure_session_allowed_for(true, false).is_err());
        assert!(ensure_session_allowed_for(true, true).is_ok());
        assert!(ensure_session_allowed_for(false, false).is_ok());
    }
}
