pub fn is_wsl() -> bool {
    if !cfg!(target_os = "linux") {
        return false;
    }

    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .or_else(|_| std::fs::read_to_string("/proc/version"))
        .map(|content| content.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

pub fn lan_guidance(port: u16) -> String {
    format!(
        "Detected WSL. Windows may reach this at http://localhost:{port}/t/<token>.\n\
         For phone access, use WSL mirrored networking when available, or expose the port from Windows with portproxy/firewall rules."
    )
}
