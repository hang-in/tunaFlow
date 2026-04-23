import { useEffect, useState } from "react";
import { useTranslation, Trans } from "react-i18next";
import { ExternalLink, Keyboard, Lightbulb, AlertTriangle, FileWarning } from "lucide-react";
import { listRecentCrashReports, type CrashReportSummary } from "@/lib/crashReporter";

const SHORTCUT_KEYS = [
  { keys: "Cmd+K", descKey: "help.shortcut_desc.cmd_k" },
  { keys: "Cmd+Enter", descKey: "help.shortcut_desc.cmd_enter" },
  { keys: "Shift+Enter", descKey: "help.shortcut_desc.shift_enter" },
  { keys: "Esc", descKey: "help.shortcut_desc.esc" },
  { keys: "Tab", descKey: "help.shortcut_desc.tab" },
] as const;

const FEATURE_KEYS = ["pdr", "branch", "contextpack", "insight", "pty"] as const;
const TROUBLESHOOTING_KEYS = ["no_response", "insight_reset", "cpu_high", "macos_gatekeeper"] as const;

export function HelpSection() {
  const { t } = useTranslation("settings");
  const [reports, setReports] = useState<CrashReportSummary[]>([]);
  useEffect(() => {
    listRecentCrashReports(5).then(setReports);
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1">{t("help.heading")}</h2>
        <p className="text-[12px] text-muted-foreground">
          {t("help.description")}
        </p>
      </div>

      {reports.length > 0 && (
        <section>
          <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
            <FileWarning className="w-4 h-4 text-amber-400" />
            {t("help.crash_reports.title", { count: reports.length })}
          </h3>
          <div className="rounded-lg border border-amber-400/20 bg-amber-400/5 p-3 space-y-1.5 text-[12px]">
            {reports.map((r) => (
              <div key={r.file} className="flex items-center justify-between gap-2">
                <code className="text-muted-foreground truncate" title={r.file}>
                  {r.file.split("/").pop()}
                </code>
                <span className="text-muted-foreground shrink-0">
                  {(r.size / 1024).toFixed(1)} KB
                </span>
              </div>
            ))}
            <div className="pt-2 text-muted-foreground">
              <Trans
                i18nKey="help.crash_reports.location"
                ns="settings"
                components={{ code: <code /> }}
              />
            </div>
          </div>
        </section>
      )}

      <section>
        <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
          <Keyboard className="w-4 h-4 text-muted-foreground" />
          {t("help.shortcuts_title")}
        </h3>
        <div className="rounded-lg border border-border/40 divide-y divide-border/40">
          {SHORTCUT_KEYS.map(({ keys, descKey }) => (
            <div key={keys} className="flex items-center justify-between px-3 py-2 text-[12px]">
              <kbd className="font-mono text-[11px] bg-muted/50 px-2 py-0.5 rounded border border-border/40">
                {keys}
              </kbd>
              <span className="text-muted-foreground">{t(descKey)}</span>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
          <Lightbulb className="w-4 h-4 text-muted-foreground" />
          {t("help.features_title")}
        </h3>
        <div className="space-y-3 text-[12px]">
          {FEATURE_KEYS.map((key) => (
            <div key={key}>
              <div className="text-foreground font-[500] mb-0.5">{t(`help.features.${key}.title`)}</div>
              <div className="text-muted-foreground leading-relaxed">{t(`help.features.${key}.desc`)}</div>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
          <AlertTriangle className="w-4 h-4 text-muted-foreground" />
          {t("help.troubleshooting_title")}
        </h3>
        <div className="space-y-3 text-[12px]">
          {TROUBLESHOOTING_KEYS.map((key) => (
            <div key={key}>
              <div className="text-foreground font-[500] mb-0.5">{t(`help.troubleshooting.${key}.title`)}</div>
              <div className="text-muted-foreground leading-relaxed whitespace-pre-line">{t(`help.troubleshooting.${key}.desc`)}</div>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h3 className="text-[13px] font-[500] text-foreground mb-2">{t("help.external_title")}</h3>
        <div className="space-y-1.5 text-[12px]">
          <LinkItem href="https://github.com/hang-in/tunaFlow" label={t("help.external.github")} />
          <LinkItem href="https://github.com/hang-in/tunaFlow/issues" label={t("help.external.issues")} />
          <LinkItem href="mailto:d9ng@outlook.com" label={t("help.external.email")} />
        </div>
      </section>
    </div>
  );
}

function LinkItem({ href, label }: { href: string; label: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="flex items-center gap-1.5 text-primary hover:underline"
    >
      <ExternalLink className="w-3.5 h-3.5" />
      {label}
    </a>
  );
}
