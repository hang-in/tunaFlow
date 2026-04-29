import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { Loader2, CheckCircle2, AlertCircle } from "lucide-react";
import { getSetting, setSetting } from "@/lib/appStore";

// Setting key gates whether the dialog ever shows. Pre-set to true to skip
// (debug toggle for startup-race diagnosis — Q-4 / W-9 of bootstrap plan).
export const FIRST_RUN_SETTING_KEY = "first_run_dependency_check_done";

interface DependencyStatus {
  name: string;
  available: boolean;
  installerCommand: string;
  requires: string;
  version: string | null;
}

interface InstallResult {
  name: string;
  success: boolean;
  message: string;
  manualCommand: string | null;
}

export function FirstRunDependencyDialog() {
  const { t } = useTranslation("dialog");
  const [open, setOpen] = useState(false);
  const [missing, setMissing] = useState<DependencyStatus[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState<Set<string>>(new Set());
  const [results, setResults] = useState<Record<string, InstallResult>>({});

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const done = await getSetting<boolean>(FIRST_RUN_SETTING_KEY, false);
      if (done || cancelled) return;
      try {
        const list = await invoke<DependencyStatus[]>("list_dependencies");
        if (cancelled) return;
        const missing = list.filter((d) => !d.available);
        if (missing.length === 0) {
          await setSetting(FIRST_RUN_SETTING_KEY, true);
          return;
        }
        setMissing(missing);
        setSelected(new Set(missing.map((d) => d.name)));
        setOpen(true);
      } catch (e) {
        console.error("[first-run-deps] list_dependencies failed", e);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const u = listen<InstallResult>("dependency:install_result", (e) => {
      const r = e.payload;
      setResults((prev) => ({ ...prev, [r.name]: r }));
      setInstalling((prev) => {
        if (!prev.has(r.name)) return prev;
        const next = new Set(prev);
        next.delete(r.name);
        return next;
      });
    });
    return () => { u.then((un) => un()).catch(() => {}); };
  }, []);

  const finish = async () => {
    await setSetting(FIRST_RUN_SETTING_KEY, true);
    setOpen(false);
  };

  const handleInstall = async () => {
    const targets = missing.filter((d) => selected.has(d.name));
    if (targets.length === 0) return;
    setInstalling(new Set(targets.map((d) => d.name)));
    await Promise.all(targets.map((d) =>
      invoke<InstallResult>("install_dependency", { name: d.name }).catch((e) => {
        const r: InstallResult = {
          name: d.name,
          success: false,
          message: String(e),
          manualCommand: d.installerCommand,
        };
        setResults((prev) => ({ ...prev, [d.name]: r }));
        setInstalling((prev) => {
          if (!prev.has(d.name)) return prev;
          const next = new Set(prev);
          next.delete(d.name);
          return next;
        });
      })
    ));
  };

  const toggle = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const targetCount = missing.filter((d) => selected.has(d.name)).length;
  const completedCount = missing.filter(
    (d) => selected.has(d.name) && results[d.name],
  ).length;
  const allAttempted = installing.size === 0 && targetCount > 0 && completedCount === targetCount;

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[110] flex items-center justify-center" data-testid="first-run-deps-dialog">
      <div className="absolute inset-0 bg-black/50" />
      <div className="relative z-10 w-[520px] max-h-[85vh] bg-background border border-border rounded-xl shadow-2xl flex flex-col overflow-hidden">
        <div className="px-6 pt-5 pb-3 border-b border-border">
          <h2 className="text-sm font-semibold text-foreground">{t("dependency_install.title")}</h2>
          <p className="text-[11px] text-muted-foreground mt-0.5">{t("dependency_install.lead")}</p>
        </div>

        <div className="px-6 py-4 space-y-3 flex-1 overflow-y-auto min-h-0">
          {missing.map((d) => {
            const r = results[d.name];
            const isInstalling = installing.has(d.name);
            const locked = isInstalling || !!r;
            return (
              <div key={d.name} className="rounded-md border border-border/60 p-3" data-testid={`dep-card-${d.name}`}>
                <label className="flex items-start gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={selected.has(d.name)}
                    onChange={() => toggle(d.name)}
                    disabled={locked}
                    className="w-3.5 h-3.5 mt-0.5"
                    aria-label={d.name}
                  />
                  <div className="flex-1 space-y-0.5">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-medium text-foreground">{d.name}</span>
                      <span className="text-[10px] text-muted-foreground">{d.requires}</span>
                    </div>
                    <code className="text-[10px] text-muted-foreground/70 block break-all">{d.installerCommand}</code>
                    {isInstalling && (
                      <div className="flex items-center gap-1.5 text-[11px] text-primary mt-1">
                        <Loader2 className="w-3 h-3 animate-spin" />
                        {t("dependency_install.installing")}
                      </div>
                    )}
                    {r?.success && (
                      <div className="flex items-center gap-1.5 text-[11px] text-green-500 mt-1">
                        <CheckCircle2 className="w-3 h-3" />
                        {r.message}
                      </div>
                    )}
                    {r && !r.success && (
                      <div className="flex items-start gap-1.5 text-[11px] text-destructive mt-1">
                        <AlertCircle className="w-3 h-3 mt-0.5" />
                        <div>
                          <div>{r.message}</div>
                          {r.manualCommand && (
                            <div className="mt-1 text-muted-foreground">
                              {t("dependency_install.manual_hint")}{" "}
                              <code className="text-foreground/70">{r.manualCommand}</code>
                            </div>
                          )}
                        </div>
                      </div>
                    )}
                  </div>
                </label>
              </div>
            );
          })}
          <p className="text-[10px] text-muted-foreground/60">{t("dependency_install.venv_note")}</p>
        </div>

        <div className="px-6 py-4 border-t border-border flex justify-between">
          <button
            onClick={finish}
            disabled={installing.size > 0}
            className="text-[11px] text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40"
          >
            {t("dependency_install.skip")}
          </button>
          {allAttempted ? (
            <button
              onClick={finish}
              className="px-4 py-1.5 rounded-lg bg-primary text-primary-foreground text-[11px] font-medium hover:bg-primary/90 transition-colors"
            >
              {t("dependency_install.close")}
            </button>
          ) : (
            <button
              onClick={handleInstall}
              disabled={installing.size > 0 || targetCount === 0}
              className="px-4 py-1.5 rounded-lg bg-primary text-primary-foreground text-[11px] font-medium hover:bg-primary/90 transition-colors disabled:opacity-40"
            >
              {t("dependency_install.install")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
