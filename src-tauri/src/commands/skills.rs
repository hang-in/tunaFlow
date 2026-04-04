use serde::Serialize;
use std::fs;
use std::path::PathBuf;

use crate::errors::AppError;

/// Parsed skill definition loaded from `~/.tunaflow/skills/{name}/SKILL.md`
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillDef {
    pub name: String,
    pub description: String,
    pub content: String,
    pub vendor: Option<String>,
    pub source_path: Option<String>,
}

/// Snapshot-level metadata from `~/.tunaflow/skills/_snapshot.json`
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillsSnapshotInfo {
    pub published_at: Option<String>,
    pub total_skills: u64,
    pub source: Option<String>,
}

/// Skill base directory: `~/.tunaflow/skills/`
fn skills_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let home = std::env::var("USERPROFILE").ok();
    #[cfg(not(target_os = "windows"))]
    let home = std::env::var("HOME").ok();
    home.map(|h| PathBuf::from(h).join(".tunaflow").join("skills"))
}

/// Scan `~/.tunaflow/skills/*/SKILL.md` and return all valid skill definitions.
#[tauri::command]
pub fn list_skills() -> Result<Vec<SkillDef>, AppError> {
    let base = match skills_dir() {
        Some(d) if d.is_dir() => d,
        _ => return Ok(Vec::new()),
    };

    let mut skills = Vec::new();
    let Ok(entries) = fs::read_dir(&base) else {
        return Ok(Vec::new());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        let Ok(content) = fs::read_to_string(&skill_file) else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let (description, body) = parse_skill(&content);

        // Read _meta.json for vendor/source metadata
        let (vendor, source_path) = read_meta(&path);

        skills.push(SkillDef {
            name,
            description,
            content: body,
            vendor,
            source_path,
        });
    }

    // Scan external tool skill directories (chops pattern)
    let mut seen_names: std::collections::HashSet<String> = skills.iter().map(|s| s.name.clone()).collect();
    for (dir, tool_source) in external_skill_paths() {
        for skill in scan_skill_dir(&dir, &tool_source) {
            if !seen_names.contains(&skill.name) {
                seen_names.insert(skill.name.clone());
                skills.push(skill);
            }
        }
    }

    // Scan Claude CLI plugin skills
    for skill in scan_claude_plugin_skills() {
        if !seen_names.contains(&skill.name) {
            seen_names.insert(skill.name.clone());
            skills.push(skill);
        }
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(skills)
}

/// Load a single skill by name. Used by ContextPack assembly.
#[tauri::command]
pub fn get_skill(name: String) -> Result<SkillDef, AppError> {
    let base = skills_dir().ok_or_else(|| {
        AppError::NotFound("skills directory not found".into())
    })?;
    let skill_file = base.join(&name).join("SKILL.md");
    let content = fs::read_to_string(&skill_file).map_err(|e| {
        AppError::NotFound(format!("Skill '{}' not found: {}", name, e))
    })?;

    let (description, body) = parse_skill(&content);
    let skill_dir = base.join(&name);
    let (vendor, source_path) = read_meta(&skill_dir);
    Ok(SkillDef {
        name,
        description,
        content: body,
        vendor,
        source_path,
    })
}

/// Read `_meta.json` from a skill directory for vendor/source metadata.
fn read_meta(skill_dir: &std::path::Path) -> (Option<String>, Option<String>) {
    let meta_file = skill_dir.join("_meta.json");
    let Ok(text) = fs::read_to_string(&meta_file) else {
        return (None, None);
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (None, None);
    };
    let vendor = val.get("vendor").and_then(|v| v.as_str()).map(|s| s.to_string());
    let source_path = val.get("source_path").and_then(|v| v.as_str()).map(|s| s.to_string());
    (vendor, source_path)
}

/// Return snapshot-level metadata from `~/.tunaflow/skills/_snapshot.json`.
#[tauri::command]
pub fn get_skills_snapshot() -> Result<SkillsSnapshotInfo, AppError> {
    let base = skills_dir().ok_or_else(|| {
        AppError::NotFound("skills directory not found".into())
    })?;
    let snap_file = base.join("_snapshot.json");
    let text = fs::read_to_string(&snap_file).map_err(|e| {
        AppError::NotFound(format!("_snapshot.json not found: {}", e))
    })?;
    let val: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        AppError::Agent(format!("_snapshot.json parse error: {}", e))
    })?;
    Ok(SkillsSnapshotInfo {
        published_at: val.get("published_at").and_then(|v| v.as_str()).map(|s| s.to_string()),
        total_skills: val.get("total_skills").and_then(|v| v.as_u64()).unwrap_or(0),
        source: val.get("source").and_then(|v| v.as_str()).map(|s| s.to_string()),
    })
}

// ─── Multi-tool skill scanning (chops pattern, MIT license) ───────────────────

/// External tool skill paths — ported from chops (github.com/shpigford/chops, MIT).
/// Scans skill directories from Claude Code, Cursor, Codex, Windsurf, Copilot,
/// Amp, OpenCode, Aider, Pi, Antigravity, and global ~/.agents/skills/.
fn external_skill_paths() -> Vec<(String, String)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let xdg = std::env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{home}/.config"));

    // (path, tool_source) pairs
    let candidates = [
        (format!("{home}/.claude/skills"), "claude"),
        (format!("{home}/.cursor/skills"), "cursor"),
        (format!("{home}/.cursor/rules"), "cursor"),
        (format!("{home}/.codex/skills"), "codex"),
        (format!("{home}/.codeium/windsurf/memories"), "windsurf"),
        (format!("{home}/.windsurf/rules"), "windsurf"),
        (format!("{home}/.copilot/skills"), "copilot"),
        (format!("{xdg}/amp/skills"), "amp"),
        (format!("{xdg}/opencode/skills"), "opencode"),
        (format!("{home}/.pi/agent/skills"), "pi"),
        (format!("{home}/.agents/skills"), "global"),
        (format!("{home}/.gemini/antigravity/skills"), "antigravity"),
    ];

    candidates
        .into_iter()
        .filter(|(path, _)| std::path::Path::new(path).is_dir())
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect()
}

/// Filenames to ignore — tool config files, not skills (ported from chops).
const IGNORED_FILES: &[&str] = &[
    "README.md", "README", "CLAUDE.md", "AGENTS.md", "AGENTS.override.md",
    "global_rules.md", "SYSTEM.md", "APPEND_SYSTEM.md", "LICENSE.md", "LICENSE",
    "CHANGELOG.md",
];

/// Project-local skill paths to probe inside a project directory (ported from chops).
#[allow(dead_code)]
const PROJECT_PROBES: &[(&str, &str)] = &[
    (".claude/skills", "claude"),
    (".cursor/skills", "cursor"),
    (".cursor/rules", "cursor"),
    (".codex/skills", "codex"),
    (".windsurf/rules", "windsurf"),
    (".github", "copilot"),
    (".config/amp/skills", "amp"),
    (".opencode/skills", "opencode"),
];

/// Scan project-local skill directories.
#[allow(dead_code)]
fn project_local_skill_paths(project_path: &str) -> Vec<(String, String)> {
    let root = std::path::Path::new(project_path);
    PROJECT_PROBES
        .iter()
        .filter_map(|(subpath, tool)| {
            let p = root.join(subpath);
            if p.is_dir() { Some((p.to_string_lossy().to_string(), tool.to_string())) } else { None }
        })
        .collect()
}

/// Scan Claude CLI plugin skills from ~/.claude/plugins/installed_plugins.json.
fn scan_claude_plugin_skills() -> Vec<SkillDef> {
    let home = std::env::var("HOME").unwrap_or_default();
    let json_path = format!("{home}/.claude/plugins/installed_plugins.json");
    let Ok(text) = fs::read_to_string(&json_path) else { return Vec::new() };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) else { return Vec::new() };

    let mut skills = Vec::new();
    if let Some(plugins) = val.get("plugins").and_then(|v| v.as_object()) {
        for (_name, installations) in plugins {
            if let Some(arr) = installations.as_array() {
                for inst in arr {
                    if let Some(install_path) = inst.get("installPath").and_then(|v| v.as_str()) {
                        let skills_dir = format!("{install_path}/skills");
                        skills.extend(scan_skill_dir(&skills_dir, "claude-plugin"));
                    }
                }
            }
        }
    }
    skills
}

/// Scan a directory for skill files (SKILL.md, *.md, *.mdc).
/// Returns skills with tool_source metadata.
fn scan_skill_dir(dir: &str, tool_source: &str) -> Vec<SkillDef> {
    let path = std::path::Path::new(dir);
    let Ok(entries) = fs::read_dir(path) else { return Vec::new() };

    let mut skills = Vec::new();
    for entry in entries.flatten() {
        let epath = entry.path();

        if epath.is_dir() {
            // Directory with SKILL.md inside (standard pattern)
            let skill_file = epath.join("SKILL.md");
            if skill_file.is_file() {
                if let Ok(content) = fs::read_to_string(&skill_file) {
                    let name = epath.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let (description, body) = parse_skill(&content);
                    skills.push(SkillDef {
                        name: format!("{tool_source}-{name}"),
                        description,
                        content: body,
                        vendor: Some(tool_source.to_string()),
                        source_path: Some(skill_file.to_string_lossy().to_string()),
                    });
                }
            }
        } else if epath.is_file() {
            // Standalone .md or .mdc file
            let ext = epath.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "md" || ext == "mdc" {
                // Skip tool config files (ported from chops)
                let filename = epath.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if IGNORED_FILES.contains(&filename) { continue; }
                if let Ok(content) = fs::read_to_string(&epath) {
                    let stem = epath.file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let (description, body) = parse_skill(&content);
                    let name = if stem == "SKILL" {
                        // Parent dir name as skill name
                        epath.parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or(&stem)
                            .to_string()
                    } else {
                        stem
                    };
                    skills.push(SkillDef {
                        name: format!("{tool_source}-{name}"),
                        description,
                        content: body,
                        vendor: Some(tool_source.to_string()),
                        source_path: Some(epath.to_string_lossy().to_string()),
                    });
                }
            }
        }
    }
    skills
}

// ─── Project stack detection ──────────────────────────────────────────────────

/// Result of scanning project manifest files for tech stack keywords.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStackResult {
    pub keywords: Vec<String>,
    pub detected_files: Vec<String>,
}

/// Scan a project directory for manifest files (package.json, Cargo.toml, pyproject.toml)
/// and extract dependency names as tech-stack keywords.
#[tauri::command]
pub fn detect_project_stack(project_path: String) -> Result<ProjectStackResult, AppError> {
    let root = std::path::Path::new(&project_path);
    let mut keywords = std::collections::HashSet::new();
    let mut detected_files = Vec::new();

    // ── package.json ──
    let pkg_json = root.join("package.json");
    if pkg_json.is_file() {
        if let Ok(text) = fs::read_to_string(&pkg_json) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                detected_files.push("package.json".to_string());
                for section in ["dependencies", "devDependencies"] {
                    if let Some(obj) = val.get(section).and_then(|v| v.as_object()) {
                        for key in obj.keys() {
                            keywords.insert(key.to_lowercase());
                        }
                    }
                }
            }
        }
    }

    // ── Cargo.toml ──
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.is_file() {
        if let Ok(text) = fs::read_to_string(&cargo_toml) {
            if let Ok(val) = text.parse::<toml::Value>() {
                detected_files.push("Cargo.toml".to_string());
                for section in ["dependencies", "dev-dependencies"] {
                    if let Some(table) = val.get(section).and_then(|v| v.as_table()) {
                        for key in table.keys() {
                            keywords.insert(key.to_lowercase());
                        }
                    }
                }
            }
        }
    }

    // ── pyproject.toml ──
    let pyproject = root.join("pyproject.toml");
    if pyproject.is_file() {
        if let Ok(text) = fs::read_to_string(&pyproject) {
            if let Ok(val) = text.parse::<toml::Value>() {
                detected_files.push("pyproject.toml".to_string());
                // PEP 621: [project.dependencies] — array of requirement strings
                if let Some(deps) = val
                    .get("project")
                    .and_then(|p| p.get("dependencies"))
                    .and_then(|d| d.as_array())
                {
                    for dep in deps {
                        if let Some(s) = dep.as_str() {
                            // Extract package name before version specifier (>=, ==, <, ~=, etc.)
                            let name = s.split(&['>', '<', '=', '!', '~', ';', '[', ' '][..])
                                .next()
                                .unwrap_or(s);
                            if !name.is_empty() {
                                keywords.insert(name.to_lowercase());
                            }
                        }
                    }
                }
                // Poetry: [tool.poetry.dependencies] — table
                if let Some(table) = val
                    .get("tool")
                    .and_then(|t| t.get("poetry"))
                    .and_then(|p| p.get("dependencies"))
                    .and_then(|d| d.as_table())
                {
                    for key in table.keys() {
                        if key != "python" {
                            keywords.insert(key.to_lowercase());
                        }
                    }
                }
            }
        }
    }

    let mut sorted: Vec<String> = keywords.into_iter().collect();
    sorted.sort();

    Ok(ProjectStackResult {
        keywords: sorted,
        detected_files,
    })
}

// ─── Skill registry (skills.sh, ported from chops, MIT) ───────────────────────

/// Search result from skills.sh registry.
#[derive(Debug, Serialize, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrySkill {
    pub id: String,
    pub skill_id: String,
    pub name: String,
    pub installs: i64,
    pub source: String,
}

#[derive(serde::Deserialize)]
struct RegistrySearchResponse {
    skills: Vec<RegistrySkill>,
}

/// Search the skills.sh registry for skills matching a query.
#[tauri::command]
pub async fn search_skill_registry(query: String) -> Result<Vec<RegistrySkill>, AppError> {
    if query.len() < 2 {
        return Ok(Vec::new());
    }
    let encoded: String = query.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c.to_string() }
        else { format!("%{:02X}", c as u32) }
    }).collect();
    let url = format!("https://skills.sh/api/search?q={encoded}&limit=30");
    let resp = reqwest::get(&url).await
        .map_err(|e| AppError::Agent(format!("registry search failed: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError::Agent(format!("registry returned {}", resp.status())));
    }
    let body: RegistrySearchResponse = resp.json().await
        .map_err(|e| AppError::Agent(format!("registry parse failed: {}", e)))?;
    Ok(body.skills)
}

/// Install a skill from the registry — download SKILL.md from GitHub and save to ~/.tunaflow/skills/.
#[tauri::command]
pub async fn install_registry_skill(source: String, skill_name: String) -> Result<String, AppError> {
    // 1. Get default branch
    let repo_url = format!("https://api.github.com/repos/{source}");
    let client = reqwest::Client::new();
    let branch = match client.get(&repo_url)
        .header("User-Agent", "tunaFlow")
        .send().await
    {
        Ok(resp) if resp.status().is_success() => {
            let val: serde_json::Value = resp.json().await.unwrap_or_default();
            val.get("default_branch").and_then(|v| v.as_str()).unwrap_or("main").to_string()
        }
        _ => "main".to_string(),
    };

    // 2. Get tree to find SKILL.md paths
    let tree_url = format!("https://api.github.com/repos/{source}/git/trees/{branch}?recursive=1");
    let tree_resp = client.get(&tree_url)
        .header("User-Agent", "tunaFlow")
        .send().await
        .map_err(|e| AppError::Agent(format!("tree fetch failed: {}", e)))?;

    #[derive(serde::Deserialize)]
    struct TreeEntry { path: String, #[serde(rename = "type")] kind: String }
    #[derive(serde::Deserialize)]
    struct TreeResponse { tree: Vec<TreeEntry> }

    let skill_paths: Vec<String> = if tree_resp.status().is_success() {
        let tree: TreeResponse = tree_resp.json().await.unwrap_or(TreeResponse { tree: Vec::new() });
        tree.tree.into_iter()
            .filter(|e| e.kind == "blob" && e.path.ends_with("/SKILL.md"))
            .map(|e| e.path)
            .collect()
    } else {
        Vec::new()
    };

    // 3. Find matching SKILL.md by frontmatter name
    let sanitized = skill_name.to_lowercase()
        .replace(' ', "-")
        .chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.' || *c == '_')
        .collect::<String>()
        .trim_matches(|c| c == '.' || c == '-').to_string();

    if sanitized.is_empty() {
        return Err(AppError::Agent("invalid skill name".into()));
    }

    let mut content: Option<String> = None;
    for path in &skill_paths {
        let raw_url = format!("https://raw.githubusercontent.com/{source}/{branch}/{path}");
        if let Ok(resp) = client.get(&raw_url).header("User-Agent", "tunaFlow").send().await {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    // Check frontmatter name match
                    let (desc, _) = parse_skill(&text);
                    let _ = desc; // frontmatter parsed
                    // Match by directory name or skill_id
                    let dir_name = std::path::Path::new(path)
                        .parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    if dir_name == sanitized || dir_name == skill_name || path.contains(&sanitized) {
                        content = Some(text);
                        break;
                    }
                    // If only one SKILL.md, use it
                    if skill_paths.len() == 1 {
                        content = Some(text);
                        break;
                    }
                }
            }
        }
    }

    // Fallback: try direct path guess
    if content.is_none() {
        let guess_url = format!("https://raw.githubusercontent.com/{source}/{branch}/SKILL.md");
        if let Ok(resp) = client.get(&guess_url).header("User-Agent", "tunaFlow").send().await {
            if resp.status().is_success() {
                content = resp.text().await.ok();
            }
        }
    }

    let skill_content = content.ok_or_else(|| AppError::Agent("SKILL.md not found in repository".into()))?;

    // 4. Save to ~/.tunaflow/skills/{name}/SKILL.md
    let base = skills_dir().ok_or_else(|| AppError::NotFound("skills directory not found".into()))?;
    let skill_dir = base.join(&sanitized);
    fs::create_dir_all(&skill_dir).map_err(|e| AppError::Agent(format!("mkdir failed: {}", e)))?;
    let skill_file = skill_dir.join("SKILL.md");
    fs::write(&skill_file, &skill_content).map_err(|e| AppError::Agent(format!("write failed: {}", e)))?;

    // Write _meta.json
    let meta = serde_json::json!({
        "vendor": "registry",
        "source_path": source,
        "published_at": chrono::Utc::now().to_rfc3339(),
    });
    let _ = fs::write(skill_dir.join("_meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());

    Ok(sanitized)
}

/// Build a skill pack recommendation: detect tech stack → find local matches → search registry for gaps.
#[tauri::command]
pub async fn build_skill_pack(project_path: String) -> Result<SkillPackRecommendation, AppError> {
    // 1. Detect tech stack
    let stack = detect_project_stack(project_path)?;
    if stack.keywords.is_empty() {
        return Ok(SkillPackRecommendation { keywords: vec![], local: vec![], registry: vec![] });
    }

    // 2. Find local skills matching keywords
    let all_skills = list_skills()?;
    let local_names: std::collections::HashSet<String> = all_skills.iter().map(|s| s.name.clone()).collect();

    // Simple keyword matching against skill names/descriptions
    let top_keywords: Vec<String> = stack.keywords.iter().take(10).cloned().collect();
    let local_matches: Vec<String> = all_skills.iter()
        .filter(|s| {
            let lower_name = s.name.to_lowercase();
            let lower_desc = s.description.to_lowercase();
            top_keywords.iter().any(|kw| lower_name.contains(kw) || lower_desc.contains(kw))
        })
        .map(|s| s.name.clone())
        .collect();

    // 3. Search registry for keywords not covered by local skills
    let mut registry_results: Vec<RegistrySkill> = Vec::new();
    // Search top 3 keywords that don't have local matches
    let uncovered: Vec<&String> = top_keywords.iter()
        .filter(|kw| !local_matches.iter().any(|m| m.to_lowercase().contains(&kw.to_lowercase())))
        .take(3)
        .collect();

    for kw in uncovered {
        if let Ok(results) = search_skill_registry(kw.clone()).await {
            for r in results {
                if !local_names.contains(&r.name) && !registry_results.iter().any(|rr| rr.id == r.id) {
                    registry_results.push(r);
                }
            }
        }
    }
    // Limit registry results
    registry_results.truncate(10);

    Ok(SkillPackRecommendation {
        keywords: top_keywords,
        local: local_matches,
        registry: registry_results,
    })
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkillPackRecommendation {
    pub keywords: Vec<String>,
    pub local: Vec<String>,
    pub registry: Vec<RegistrySkill>,
}

/// Parse SKILL.md: extract `description:` from frontmatter, rest is content.
fn parse_skill(raw: &str) -> (String, String) {
    if !raw.starts_with("---") {
        return (String::new(), raw.trim().to_string());
    }

    let after_open = &raw[3..];
    let Some(close_pos) = after_open.find("\n---") else {
        return (String::new(), raw.trim().to_string());
    };

    let frontmatter = &after_open[..close_pos];
    let body_raw = &raw[3 + close_pos + 4..];
    let body = body_raw
        .strip_prefix('\n')
        .unwrap_or(body_raw)
        .to_string();

    let description = frontmatter
        .lines()
        .find_map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("description:") {
                Some(trimmed["description:".len()..].trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    (description, body)
}
