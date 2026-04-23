import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import QRCode from "react-qr-code";
import { Smartphone, RefreshCw, Copy, Check } from "lucide-react";

interface ConnectionInfo {
  url: string;
  token: string;
}

export function MobileSection() {
  const { t } = useTranslation("settings");
  const [info, setInfo] = useState<ConnectionInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState<"url" | "token" | null>(null);

  async function load() {
    setLoading(true);
    try {
      const result = await invoke<ConnectionInfo>("get_api_connection_info");
      setInfo(result);
    } catch (e) {
      console.error("Failed to load connection info:", e);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { load(); }, []);

  async function copy(type: "url" | "token", text: string) {
    await navigator.clipboard.writeText(text);
    setCopied(type);
    setTimeout(() => setCopied(null), 1500);
  }

  const qrPayload = info ? JSON.stringify({ url: info.url, token: info.token }) : "";

  return (
    <div>
      <h2 className="text-[14px] font-[550] text-foreground mb-1">{t("mobile.heading")}</h2>
      <p className="text-[12px] text-muted-foreground mb-5">
        {t("mobile.description")}
      </p>

      {loading && (
        <div className="flex items-center gap-2 text-[12px] text-muted-foreground">
          <RefreshCw className="w-3.5 h-3.5 animate-spin" />
          {t("mobile.loading")}
        </div>
      )}

      {info && (
        <div className="flex gap-6">
          {/* QR Code */}
          <div className="shrink-0">
            <div className="bg-white p-3 rounded-lg inline-block">
              <QRCode value={qrPayload} size={160} />
            </div>
            <p className="text-[11px] text-muted-foreground mt-2 text-center">
              {t("mobile.scan_hint")}
            </p>
          </div>

          {/* Manual info */}
          <div className="flex-1 space-y-4">
            <div>
              <label className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
                {t("mobile.server_url")}
              </label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 text-[12px] bg-accent/50 rounded-md px-3 py-1.5 font-mono text-foreground break-all">
                  {info.url}
                </code>
                <button
                  onClick={() => copy("url", info.url)}
                  className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                >
                  {copied === "url" ? <Check className="w-3.5 h-3.5 text-green-500" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
              </div>
            </div>

            <div>
              <label className="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
                {t("mobile.api_token")}
              </label>
              <div className="flex items-center gap-2 mt-1">
                <code className="flex-1 text-[12px] bg-accent/50 rounded-md px-3 py-1.5 font-mono text-foreground truncate">
                  {info.token}
                </code>
                <button
                  onClick={() => copy("token", info.token)}
                  className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
                >
                  {copied === "token" ? <Check className="w-3.5 h-3.5 text-green-500" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
              </div>
            </div>

            <div className="rounded-lg bg-accent/30 p-3">
              <div className="flex items-start gap-2">
                <Smartphone className="w-4 h-4 text-muted-foreground mt-0.5 shrink-0" />
                <div className="text-[12px] text-muted-foreground space-y-1">
                  <p>{t("mobile.steps.step1")}</p>
                  <p>
                    {t("mobile.steps.step2_prefix")}
                    <strong className="text-foreground">{t("mobile.steps.step2_strong")}</strong>
                    {t("mobile.steps.step2_suffix")}
                  </p>
                  <p>{t("mobile.steps.step3")}</p>
                </div>
              </div>
            </div>

            <button
              onClick={load}
              className="flex items-center gap-1.5 text-[12px] text-muted-foreground hover:text-foreground transition-colors"
            >
              <RefreshCw className="w-3 h-3" />
              {t("mobile.refresh")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
