#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub fn child_command(command: &[String]) -> anyhow::Result<Command> {
    anyhow::ensure!(!command.is_empty(), "command is required");

    let resolved = resolve(command)?;
    let mut cmd = Command::new(resolved.program);
    cmd.args(resolved.args);
    Ok(cmd)
}

pub fn resolve(command: &[String]) -> anyhow::Result<ResolvedCommand> {
    anyhow::ensure!(!command.is_empty(), "command is required");

    #[cfg(windows)]
    {
        resolve_windows(command)
    }

    #[cfg(not(windows))]
    {
        Ok(ResolvedCommand {
            program: PathBuf::from(&command[0]),
            args: command[1..].to_vec(),
        })
    }
}

#[cfg(windows)]
fn resolve_windows(command: &[String]) -> anyhow::Result<ResolvedCommand> {
    let Some(program) = find_windows_command(&command[0]) else {
        anyhow::bail!("program not found");
    };

    let extension = program
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "ps1" => {
            let mut args = vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                program.to_string_lossy().to_string(),
            ];
            args.extend_from_slice(&command[1..]);
            Ok(ResolvedCommand {
                program: PathBuf::from("pwsh"),
                args,
            })
        }
        "cmd" | "bat" => {
            let mut args = vec!["/C".to_string(), program.to_string_lossy().to_string()];
            args.extend_from_slice(&command[1..]);
            Ok(ResolvedCommand {
                program: PathBuf::from("cmd"),
                args,
            })
        }
        _ => Ok(ResolvedCommand {
            program,
            args: command[1..].to_vec(),
        }),
    }
}

#[cfg(windows)]
fn find_windows_command(program: &str) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.components().count() > 1 {
        return find_windows_candidate(path);
    }

    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .find_map(|dir| find_windows_candidate(&dir.join(program)))
}

#[cfg(windows)]
fn find_windows_candidate(base: &Path) -> Option<PathBuf> {
    if base.extension().is_some() && base.is_file() {
        return Some(base.to_path_buf());
    }

    for extension in windows_extensions() {
        let candidate = if base.extension().is_some() {
            base.with_extension(extension.trim_start_matches('.'))
        } else {
            let file_name = base.file_name()?.to_string_lossy();
            base.with_file_name(format!("{file_name}{extension}"))
        };
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    if base.is_file() {
        return Some(base.to_path_buf());
    }

    None
}

#[cfg(windows)]
fn windows_extensions() -> Vec<String> {
    let mut extensions = vec![
        ".exe".to_string(),
        ".com".to_string(),
        ".cmd".to_string(),
        ".bat".to_string(),
        ".ps1".to_string(),
    ];

    if let Some(pathext) = std::env::var_os("PATHEXT") {
        for extension in pathext.to_string_lossy().split(';') {
            if extension.is_empty() {
                continue;
            }
            let normalized = extension.to_ascii_lowercase();
            if !extensions.iter().any(|existing| existing == &normalized) {
                extensions.push(normalized);
            }
        }
    }

    extensions
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("rterm-command-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn ps1_scripts_are_invoked_through_pwsh() {
        let dir = test_dir("ps1");
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("tool.ps1");
        std::fs::write(&script, "Write-Output ok").unwrap();

        let resolved =
            resolve_windows(&[script.to_string_lossy().to_string(), "arg".to_string()]).unwrap();

        assert_eq!(resolved.program, PathBuf::from("pwsh"));
        assert!(resolved.args.contains(&"-File".to_string()));
        assert!(
            resolved
                .args
                .contains(&script.to_string_lossy().to_string())
        );
        assert_eq!(resolved.args.last().unwrap(), "arg");

        let _ = std::fs::remove_file(script);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn cmd_scripts_are_invoked_through_cmd() {
        let dir = test_dir("cmd");
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("tool.cmd");
        std::fs::write(&script, "@echo off").unwrap();

        let resolved =
            resolve_windows(&[script.to_string_lossy().to_string(), "arg".to_string()]).unwrap();

        assert_eq!(resolved.program, PathBuf::from("cmd"));
        assert_eq!(resolved.args[0], "/C");
        assert_eq!(resolved.args[1], script.to_string_lossy());
        assert_eq!(resolved.args[2], "arg");

        let _ = std::fs::remove_file(script);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn extension_wrappers_are_preferred_over_extensionless_shell_shims() {
        let dir = test_dir("extension-wrapper");
        std::fs::create_dir_all(&dir).unwrap();
        let shell_shim = dir.join("tool");
        let cmd_shim = dir.join("tool.cmd");
        std::fs::write(&shell_shim, "#!/bin/sh").unwrap();
        std::fs::write(&cmd_shim, "@echo off").unwrap();

        let resolved = resolve_windows(&[dir.join("tool").to_string_lossy().to_string()]).unwrap();

        assert_eq!(resolved.program, PathBuf::from("cmd"));
        assert_eq!(resolved.args[1], cmd_shim.to_string_lossy());

        let _ = std::fs::remove_file(shell_shim);
        let _ = std::fs::remove_file(cmd_shim);
        let _ = std::fs::remove_dir(dir);
    }
}
