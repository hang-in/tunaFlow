import { useState, useEffect } from "react";
import { getSetting, setSetting } from "@/lib/appStore";

interface TerminalSettings {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
}

const DEFAULTS: TerminalSettings = {
  fontFamily: "'JetBrains Mono', 'Consolas', monospace",
  fontSize: 12,
  lineHeight: 1.3,
};

const FONT_OPTIONS = [
  "'JetBrains Mono', 'Consolas', monospace",
  "'Fira Code', monospace",
  "'Source Code Pro', monospace",
  "'SF Mono', 'Monaco', monospace",
  "monospace",
];

export function TerminalSection() {
  const [settings, setSettings] = useState<TerminalSettings>(DEFAULTS);

  useEffect(() => {
    getSetting<TerminalSettings>("terminalSettings", DEFAULTS).then(setSettings);
  }, []);

  const update = (patch: Partial<TerminalSettings>) => {
    const next = { ...settings, ...patch };
    setSettings(next);
    setSetting("terminalSettings", next);
  };

  return (
    <div className="space-y-3">
      <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">Terminal</h3>

      {/* Font Family */}
      <div className="flex items-center justify-between">
        <label className="text-xs text-muted-foreground">Font</label>
        <select
          value={settings.fontFamily}
          onChange={(e) => update({ fontFamily: e.target.value })}
          className="text-xs bg-background border border-border/40 rounded px-2 py-1 text-foreground"
        >
          {FONT_OPTIONS.map((f) => (
            <option key={f} value={f}>{f.split("'")[1] || f}</option>
          ))}
        </select>
      </div>

      {/* Font Size */}
      <div className="flex items-center justify-between">
        <label className="text-xs text-muted-foreground">Font Size</label>
        <div className="flex items-center gap-2">
          <input
            type="range" min={8} max={18} step={1}
            value={settings.fontSize}
            onChange={(e) => update({ fontSize: Number(e.target.value) })}
            className="w-20 h-1 accent-primary"
          />
          <span className="text-xs text-muted-foreground w-6 text-right">{settings.fontSize}</span>
        </div>
      </div>

      {/* Line Height */}
      <div className="flex items-center justify-between">
        <label className="text-xs text-muted-foreground">Line Height</label>
        <div className="flex items-center gap-2">
          <input
            type="range" min={1.0} max={2.0} step={0.1}
            value={settings.lineHeight}
            onChange={(e) => update({ lineHeight: Number(e.target.value) })}
            className="w-20 h-1 accent-primary"
          />
          <span className="text-xs text-muted-foreground w-6 text-right">{settings.lineHeight.toFixed(1)}</span>
        </div>
      </div>

      <p className="text-[9px] text-muted-foreground/40">
        Changes apply on next terminal open. Font must be installed on your system.
      </p>
    </div>
  );
}
