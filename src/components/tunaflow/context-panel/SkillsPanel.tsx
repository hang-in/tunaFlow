import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Zap, ChevronDown, ChevronRight, Search, X, Sparkles, Check, Download, Globe, Loader2 } from "lucide-react";
import type { SkillsSnapshotInfo } from "@/types";
import { useSkillFiltering, getVendor } from "./useSkillFiltering";

function skillLabel(name: string): string {
  const idx = name.indexOf("-");
  return idx > 0 ? name.slice(idx + 1) : name;
}

const VENDOR_COLORS: Record<string, string> = {
  anthropic: "bg-agent-claude/20 text-agent-claude",
  microsoft: "bg-blue-500/15 text-blue-400",
  openai: "bg-agent-codex/20 text-agent-codex",
  vercel: "bg-foreground/10 text-foreground/70",
  supabase: "bg-emerald-500/15 text-emerald-400",
  remotion: "bg-purple-500/15 text-purple-400",
};

// ─── Presets (CLAUDE.md §15) ────────────────────────────────────────────────

interface Preset {
  label: string;
  skills: string[];
}

const PRESETS: Preset[] = [
  { label: "Frontend", skills: ["anthropic-frontend-design", "microsoft-zustand-store-ts"] },
  { label: "Review", skills: ["microsoft-frontend-design-review", "anthropic-webapp-testing"] },
  { label: "OpenAI", skills: ["openai-openai-docs"] },
  { label: "Claude", skills: ["anthropic-claude-api"] },
  { label: "MCP", skills: ["anthropic-mcp-builder"] },
];

interface SkillPackRec {
  keywords: string[];
  local: string[];
  registry: RegistrySkill[];
}

function ProjectSkillPack() {
  const { t } = useTranslation("skills");
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const acceptRecommendedSkills = useChatStore((s) => s.acceptRecommendedSkills);
  const loadSkills = useChatStore((s) => s.loadSkills);
  const [pack, setPack] = useState<SkillPackRec | null>(null);
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [dismissed, setDismissed] = useState(false);

  const buildPack = async () => {
    setLoading(true);
    try {
      const project = await invoke<{ path?: string }>("get_project", { key: selectedProjectKey });
      if (!project.path) { setLoading(false); return; }
      const result = await invoke<SkillPackRec>("build_skill_pack", { projectPath: project.path });
      setPack(result);
      setSelected(new Set(result.local));
    } catch (e) { console.error("[skill-pack]", e); }
    setLoading(false);
  };

  const installAndApply = async () => {
    if (!pack) return;
    // Install registry skills first
    for (const rs of pack.registry) {
      if (selected.has(rs.name)) {
        setInstalling(rs.name);
        try {
          await invoke("install_registry_skill", { source: rs.source, skillName: rs.skillId || rs.name });
        } catch (e) { console.error("[install]", e); }
      }
    }
    setInstalling(null);
    // Reload skills list then activate all selected
    await loadSkills();
    acceptRecommendedSkills([...selected]);
    setDismissed(true);
  };

  const toggleItem = (name: string) => {
    setSelected((prev) => { const n = new Set(prev); if (n.has(name)) n.delete(name); else n.add(name); return n; });
  };

  if (dismissed) return null;

  return (
    <div className="rounded-md border border-primary/25 bg-primary/5 px-2.5 py-2 space-y-1.5">
      <div className="flex items-center gap-1.5">
        <Sparkles className="w-3.5 h-3.5 text-primary/70 shrink-0" />
        <span className="text-[10px] font-semibold text-foreground/80">{t("pack.title")}</span>
        {!pack && (
          <button
            onClick={buildPack}
            disabled={loading || !selectedProjectKey}
            className="ml-auto text-[8px] px-1.5 py-0.5 rounded bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-30 transition-colors"
          >
            {loading ? <Loader2 className="w-3 h-3 animate-spin" /> : t("pack.analyze_button")}
          </button>
        )}
      </div>

      {pack && (
        <>
          {pack.keywords.length > 0 && (
            <p className="text-[8px] text-muted-foreground/40">{t("pack.keywords_detected", { keywords: pack.keywords.slice(0, 8).join(", ") })}</p>
          )}

          {/* Local skills */}
          {pack.local.length > 0 && (
            <div className="space-y-0.5">
              <p className="text-[8px] text-muted-foreground/50 font-medium">{t("pack.local_header")}</p>
              <div className="flex flex-wrap gap-1">
                {pack.local.map((name) => {
                  const label = name.indexOf("-") > 0 ? name.slice(name.indexOf("-") + 1) : name;
                  const checked = selected.has(name);
                  return (
                    <button key={name} onClick={() => toggleItem(name)}
                      className={cn("flex items-center gap-0.5 text-[8px] px-1.5 py-0.5 rounded-full border transition-colors",
                        checked ? "border-primary/40 bg-primary/15 text-primary" : "border-border/30 text-muted-foreground/40 line-through"
                      )}>
                      {checked && <Check className="w-2 h-2" />}{label}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Registry skills */}
          {pack.registry.length > 0 && (
            <div className="space-y-0.5">
              <p className="text-[8px] text-muted-foreground/50 font-medium">{t("pack.registry_header")}</p>
              <div className="flex flex-wrap gap-1">
                {pack.registry.map((rs) => {
                  const checked = selected.has(rs.name);
                  return (
                    <button key={rs.id} onClick={() => toggleItem(rs.name)}
                      className={cn("flex items-center gap-0.5 text-[8px] px-1.5 py-0.5 rounded-full border transition-colors",
                        checked ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-400" : "border-border/30 text-muted-foreground/40 line-through"
                      )}>
                      {checked && <Download className="w-2 h-2" />}
                      {installing === rs.name && <Loader2 className="w-2 h-2 animate-spin" />}
                      {rs.name}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {(pack.local.length > 0 || pack.registry.length > 0) && (
            <div className="flex items-center gap-2 pt-1">
              <button onClick={installAndApply} disabled={selected.size === 0 || !!installing}
                className="text-[9px] font-semibold px-2 py-0.5 rounded bg-primary/20 text-primary hover:bg-primary/30 disabled:opacity-30 transition-colors">
                {installing ? t("pack.installing_status") : t("pack.apply_button", { count: selected.size })}
              </button>
              <button onClick={() => setDismissed(true)}
                className="text-[9px] text-muted-foreground/40 hover:text-muted-foreground/60 transition-colors">
                {t("pack.dismiss_button")}
              </button>
            </div>
          )}

          {pack.local.length === 0 && pack.registry.length === 0 && (
            <p className="text-[8px] text-muted-foreground/40">{t("pack.no_match")}</p>
          )}
        </>
      )}
    </div>
  );
}

interface RegistrySkill {
  id: string;
  skillId: string;
  name: string;
  installs: number;
  source: string;
}

function SkillRegistrySearch({ onInstalled }: { onInstalled: () => void }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<RegistrySkill[]>([]);
  const [searching, setSearching] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  const doSearch = async () => {
    if (query.length < 2) return;
    setSearching(true);
    try {
      const r = await invoke<RegistrySkill[]>("search_skill_registry", { query });
      setResults(r);
    } catch (e) {
      console.error("[registry]", e);
      setResults([]);
    }
    setSearching(false);
  };

  const doInstall = async (skill: RegistrySkill) => {
    setInstalling(skill.id);
    try {
      await invoke<string>("install_registry_skill", {
        source: skill.source,
        skillName: skill.skillId || skill.name,
      });
      onInstalled();
    } catch (e) {
      console.error("[registry install]", e);
    }
    setInstalling(null);
  };

  return (
    <div className="border-t border-border/20 pt-2">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 w-full text-left px-1 py-0.5 rounded hover:bg-accent/30 transition-colors"
      >
        {open ? <ChevronDown className="w-3 h-3 text-muted-foreground/50" /> : <ChevronRight className="w-3 h-3 text-muted-foreground/50" />}
        <Globe className="w-3 h-3 text-muted-foreground/50" />
        <span className="text-[10px] font-semibold text-muted-foreground/50 uppercase tracking-wider">Registry</span>
      </button>

      {open && (
        <div className="mt-1.5 px-1 space-y-1.5">
          <div className="flex gap-1">
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && doSearch()}
              placeholder="Search skills.sh..."
              className="flex-1 h-6 px-2 text-[10px] bg-muted/30 border border-border/20 rounded text-foreground placeholder:text-muted-foreground/30 focus:outline-none focus:border-border/50"
            />
            <button
              onClick={doSearch}
              disabled={searching || query.length < 2}
              className="px-2 h-6 text-[9px] font-medium rounded bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-30 transition-colors"
            >
              {searching ? <Loader2 className="w-3 h-3 animate-spin" /> : <Search className="w-3 h-3" />}
            </button>
          </div>

          {results.length > 0 && (
            <div className="space-y-0.5 max-h-[200px] overflow-y-auto">
              {results.map((skill) => (
                <div key={skill.id} className="flex items-center gap-2 px-1.5 py-1 rounded hover:bg-accent/40 transition-colors">
                  <div className="flex-1 min-w-0">
                    <p className="text-[10px] font-medium text-foreground truncate">{skill.name}</p>
                    <p className="text-[8px] text-muted-foreground/40 truncate">{skill.source} · {skill.installs > 1000 ? `${(skill.installs / 1000).toFixed(0)}K` : skill.installs} installs</p>
                  </div>
                  <button
                    onClick={() => doInstall(skill)}
                    disabled={installing === skill.id}
                    className="p-1 text-primary/50 hover:text-primary transition-colors disabled:opacity-30"
                    title="Install"
                  >
                    {installing === skill.id ? <Loader2 className="w-3 h-3 animate-spin" /> : <Download className="w-3 h-3" />}
                  </button>
                </div>
              ))}
            </div>
          )}

          {!searching && results.length === 0 && query.length >= 2 && (
            <p className="text-[9px] text-muted-foreground/40 px-1">No results</p>
          )}
        </div>
      )}
    </div>
  );
}

export function SkillsPanel() {
  const skills = useChatStore((s) => s.skills);
  const activeSkills = useChatStore((s) => s.activeSkills);
  const toggleSkill = useChatStore((s) => s.toggleSkill);
  const loadSkills = useChatStore((s) => s.loadSkills);
  const [collapsedVendors, setCollapsedVendors] = useState<Set<string>>(new Set());
  const [snapshot, setSnapshot] = useState<SkillsSnapshotInfo | null>(null);
  const [search, setSearch] = useState("");
  const [vendorFilter, setVendorFilter] = useState<string | null>(null);

  useEffect(() => {
    loadSkills();
    invoke<SkillsSnapshotInfo>("get_skills_snapshot").then(setSnapshot).catch((e) => console.warn("[skills-snapshot]", e));
  }, []);

  const { allVendors, filtered, grouped, sortedVendors } = useSkillFiltering(skills, search, vendorFilter);

  const toggleVendor = (vendor: string) => {
    setCollapsedVendors((prev) => {
      const next = new Set(prev);
      if (next.has(vendor)) next.delete(vendor);
      else next.add(vendor);
      return next;
    });
  };

  const isPresetActive = (preset: Preset) =>
    preset.skills.length > 0 && preset.skills.every((s) => activeSkills.includes(s));

  const applyPreset = (preset: Preset) => {
    if (isPresetActive(preset)) {
      // Re-click: deactivate all preset skills
      for (const name of preset.skills) {
        if (activeSkills.includes(name)) toggleSkill(name);
      }
    } else {
      // Deactivate non-preset, activate preset
      const available = new Set(skills.map((s) => s.name));
      for (const name of activeSkills) {
        if (!preset.skills.includes(name)) toggleSkill(name);
      }
      for (const name of preset.skills) {
        if (available.has(name) && !activeSkills.includes(name)) toggleSkill(name);
      }
    }
  };

  if (skills.length === 0) {
    return <p className="text-xs text-muted-foreground px-2">No skills found. Add SKILL.md files to ~/.tunaflow/skills/.</p>;
  }

  return (
    <div className="space-y-2">
      {/* ─── Project Skill Pack ─── */}
      <ProjectSkillPack />

      {/* ─── Presets ─── */}
      <div className="flex flex-wrap gap-1 px-1">
        {PRESETS.map((preset) => {
          const active = isPresetActive(preset);
          return (
            <button
              key={preset.label}
              onClick={() => applyPreset(preset)}
              title={preset.skills.join(", ")}
              className={cn(
                "text-[8px] px-1.5 py-0.5 rounded-full border transition-colors",
                active
                  ? "border-primary/50 bg-primary/15 text-primary font-semibold"
                  : "border-border/30 text-muted-foreground/60 hover:border-border/60 hover:text-muted-foreground"
              )}
            >
              {preset.label}
            </button>
          );
        })}
      </div>

      {/* ─── Search + Vendor filter ─── */}
      <div className="px-1 space-y-1.5">
        {/* Search */}
        <div className="relative">
          <Search className="absolute left-1.5 top-1/2 -translate-y-1/2 w-3 h-3 text-muted-foreground/40" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search skills..."
            className="w-full h-6 pl-5 pr-6 text-[10px] bg-muted/30 border border-border/20 rounded text-foreground placeholder:text-muted-foreground/30 focus:outline-none focus:border-border/50"
          />
          {search && (
            <button onClick={() => setSearch("")} className="absolute right-1.5 top-1/2 -translate-y-1/2">
              <X className="w-3 h-3 text-muted-foreground/40 hover:text-muted-foreground" />
            </button>
          )}
        </div>

        {/* Vendor filter pills */}
        <div className="flex flex-wrap gap-0.5">
          {allVendors.map((v) => {
            const isSelected = vendorFilter === v;
            const colorClass = VENDOR_COLORS[v] || "bg-muted text-muted-foreground";
            return (
              <button
                key={v}
                onClick={() => setVendorFilter(isSelected ? null : v)}
                className={cn(
                  "text-[7px] px-1 py-px rounded transition-colors",
                  isSelected ? colorClass : "text-muted-foreground/30 hover:text-muted-foreground/50"
                )}
              >
                {v}
              </button>
            );
          })}
        </div>
      </div>

      {/* ─── Results count (when filtering) ─── */}
      {(search || vendorFilter) && (
        <p className="text-[9px] text-muted-foreground/40 px-1">
          {filtered.length} / {skills.length} skills
        </p>
      )}

      {/* ─── Grouped skill list ─── */}
      {sortedVendors.map((vendor) => {
        const vendorSkills = grouped.get(vendor)!;
        const activeCount = vendorSkills.filter((s) => activeSkills.includes(s.name)).length;
        const isCollapsed = collapsedVendors.has(vendor);
        const colorClass = VENDOR_COLORS[vendor] || "bg-muted text-muted-foreground";

        return (
          <div key={vendor}>
            <button
              onClick={() => toggleVendor(vendor)}
              className="flex items-center gap-1.5 w-full text-left px-1 py-0.5 rounded hover:bg-accent/50 transition-colors"
            >
              {isCollapsed
                ? <ChevronRight className="w-3 h-3 text-muted-foreground/50" />
                : <ChevronDown className="w-3 h-3 text-muted-foreground/50" />
              }
              <span className={cn("text-[8px] font-medium px-1 rounded", colorClass)}>
                {vendor}
              </span>
              <span className="text-[9px] text-muted-foreground/40 flex-1">
                {vendorSkills.length}
              </span>
              {activeCount > 0 && (
                <span className="text-[8px] bg-primary/10 text-primary/70 px-1 rounded">
                  {activeCount}
                </span>
              )}
            </button>

            {!isCollapsed && (
              <div className="ml-3 mt-0.5 space-y-0.5">
                {vendorSkills.map((skill) => {
                  const isActive = activeSkills.includes(skill.name);
                  return (
                    <div
                      key={skill.name}
                      className="flex items-center gap-2 px-1.5 py-1 rounded hover:bg-accent/40 transition-colors group"
                      title={skill.sourcePath || skill.name}
                    >
                      <Zap className={cn("w-3 h-3 shrink-0", isActive ? "text-primary" : "text-muted-foreground/30")} />
                      <div className="flex-1 min-w-0">
                        <p className="text-[11px] font-medium text-foreground truncate">{skillLabel(skill.name)}</p>
                        {skill.description && (
                          <p className="text-[9px] text-muted-foreground/60 truncate">{skill.description}</p>
                        )}
                      </div>
                      <button
                        onClick={() => toggleSkill(skill.name)}
                        className={cn("relative w-7 h-3.5 rounded-full transition-colors shrink-0 overflow-hidden", isActive ? "bg-primary" : "bg-muted")}
                      >
                        <span className={cn("absolute top-0.5 w-2.5 h-2.5 rounded-full bg-white transition-all shadow-sm", isActive ? "left-[16px]" : "left-0.5")} />
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}

      {/* ─── Registry search ─── */}
      <SkillRegistrySearch onInstalled={() => loadSkills()} />

      {/* ─── Snapshot metadata footer ─── */}
      {snapshot && (
        <div className="mt-3 pt-2 border-t border-border/20 px-1">
          <p className="text-[9px] text-muted-foreground/40">
            {snapshot.totalSkills} skills
            {snapshot.publishedAt && (
              <> · {snapshot.publishedAt.slice(0, 10)}</>
            )}
          </p>
        </div>
      )}
    </div>
  );
}
