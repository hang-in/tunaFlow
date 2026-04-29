import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { getSetting, setSetting } from "@/lib/appStore";

/** Settings 키 상수 — DocsSection 과 공유. */
export const DOCS_PANEL_SCOPE_KEY = "docsPanel.scope";
export type DocsPanelScope = "all" | "tunaflow";
export const DOCS_PANEL_SCOPE_DEFAULT: DocsPanelScope = "all";

/** 외부 사용자(특히 모노레포) 의 docs 가시성 향상을 위해 default = 'all'.
 *  기존 사용자는 settings.json 에 키가 없으면 자동으로 'all' fallback. */
export function DocsScopeSection() {
  const { t } = useTranslation("settings");
  const [scope, setScope] = useState<DocsPanelScope>(DOCS_PANEL_SCOPE_DEFAULT);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let alive = true;
    getSetting<DocsPanelScope>(DOCS_PANEL_SCOPE_KEY, DOCS_PANEL_SCOPE_DEFAULT).then((v) => {
      if (alive) {
        // 잘못된 값(이전 실험으로 'custom' 등) 들어와도 default 로 폴백.
        const safe: DocsPanelScope = v === "tunaflow" ? "tunaflow" : "all";
        setScope(safe);
        setLoaded(true);
      }
    });
    return () => { alive = false; };
  }, []);

  const choose = async (next: DocsPanelScope) => {
    if (next === scope) return;
    setScope(next);
    await setSetting(DOCS_PANEL_SCOPE_KEY, next);
    // DocsSection 이 invoke 시점에 새 scope 를 다시 읽도록 이벤트 한번 흘림.
    window.dispatchEvent(new CustomEvent("tf:docs-scope-changed", { detail: { scope: next } }));
  };

  if (!loaded) return null;

  const OPTIONS = [
    { id: "all" as const, labelKey: "docs_scope.option.all_label" as const, descKey: "docs_scope.option.all_desc" as const },
    { id: "tunaflow" as const, labelKey: "docs_scope.option.tunaflow_label" as const, descKey: "docs_scope.option.tunaflow_desc" as const },
  ];

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1">{t("docs_scope.heading")}</h2>
        <p className="text-[12px] text-muted-foreground mb-4">{t("docs_scope.description")}</p>
      </div>

      <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-2">
        {OPTIONS.map((opt) => {
          const active = scope === opt.id;
          return (
            <button
              key={opt.id}
              type="button"
              onClick={() => choose(opt.id)}
              className={cn(
                "w-full text-left rounded-md border p-3 transition-colors",
                active
                  ? "border-primary/40 bg-primary/5"
                  : "border-border/20 hover:border-border/40 hover:bg-accent/20",
              )}
            >
              <div className="flex items-center gap-2">
                <span
                  className={cn(
                    "w-3.5 h-3.5 rounded-full border flex items-center justify-center shrink-0",
                    active ? "border-primary" : "border-border/50",
                  )}
                  aria-hidden
                >
                  {active && <span className="w-1.5 h-1.5 rounded-full bg-primary" />}
                </span>
                <span className="text-[13px] font-medium text-foreground">{t(opt.labelKey)}</span>
              </div>
              <p className="text-[11px] text-muted-foreground/70 mt-1 pl-5.5" style={{ paddingLeft: "22px" }}>
                {t(opt.descKey)}
              </p>
            </button>
          );
        })}
      </div>
    </div>
  );
}
