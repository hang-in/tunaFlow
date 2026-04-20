//! PATH / shell environment inheritance for GUI-launched app bundles.

/// Inherit the user's shell PATH + common install locations.
///
/// macOS .app bundles launched from Finder/Launchpad get a minimal PATH
/// (`/usr/bin:/bin:/usr/sbin:/sbin`) and miss user-installed CLI agents such as
/// `claude`, `codex`, `gemini`. Earlier attempt used `-l` (login) only which
/// does not source `.zshrc`, so nvm/asdf-initialized PATH entries were missed.
/// This version: (1) tries login+interactive, then login-only, (2) always
/// appends well-known install dirs, (3) expands `~/.nvm/versions/node/*/bin`.
pub fn inherit_shell_path() {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());

        // (1) Harvest PATH from shell. -l -i sources both .zprofile and .zshrc.
        let mut shell_path = String::new();
        for args in [
            &["-l", "-i", "-c", "echo -n $PATH"][..],
            &["-l", "-c", "echo -n $PATH"][..],
        ] {
            if let Ok(out) = std::process::Command::new(&shell).args(args).output() {
                if out.status.success() {
                    if let Ok(p) = String::from_utf8(out.stdout) {
                        let trimmed = p.trim();
                        if !trimmed.is_empty() {
                            shell_path = trimmed.to_string();
                            break;
                        }
                    }
                }
            }
        }

        // (2) Start from the shell PATH (or current PATH as fallback) and extend.
        let current = std::env::var("PATH").unwrap_or_default();
        let base = if shell_path.is_empty() { current } else { shell_path };
        let mut parts: Vec<String> = base
            .split(':')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let push_if_dir = |parts: &mut Vec<String>, p: String| {
            if std::path::Path::new(&p).is_dir() && !parts.iter().any(|x| x == &p) {
                parts.push(p);
            }
        };
        for extra in [
            "/opt/homebrew/bin".to_string(),
            "/opt/homebrew/sbin".to_string(),
            "/usr/local/bin".to_string(),
            "/usr/local/sbin".to_string(),
            format!("{}/.npm-global/bin", home),
            format!("{}/.local/bin", home),
            format!("{}/.cargo/bin", home),
            format!("{}/.bun/bin", home),
            format!("{}/.deno/bin", home),
        ] {
            push_if_dir(&mut parts, extra);
        }

        // (3) nvm: enumerate every installed node version's bin.
        let nvm_dir = format!("{}/.nvm/versions/node", home);
        if let Ok(entries) = std::fs::read_dir(&nvm_dir) {
            for ent in entries.flatten() {
                let bin = ent.path().join("bin");
                if let Some(s) = bin.to_str() {
                    push_if_dir(&mut parts, s.to_string());
                }
            }
        }

        let joined = parts.join(":");
        eprintln!("[bootstrap/env] PATH set ({} entries)", parts.len());
        // Optional verbose dump; keep at info level so user can diagnose.
        for p in &parts {
            eprintln!("  - {}", p);
        }
        std::env::set_var("PATH", joined);
    }
}
