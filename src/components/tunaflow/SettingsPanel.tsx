import { useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { X, Bot, UserCircle, Zap, Cpu, Terminal, Smartphone, User, HelpCircle, Globe, Brain, FileText, Bell } from "lucide-react";
import { SkillsPanel } from "./context-panel/SkillsPanel";
import { AgentsSection } from "./settings/AgentsSection";
import { PersonasSection } from "./settings/PersonasSection";
import { RuntimeSection } from "./settings/RuntimeSection";
import { TerminalSection } from "./settings/TerminalSection";
import { MobileSection } from "./settings/MobileSection";
import { NotificationsSection } from "./settings/NotificationsSection";
import { ProfileSection } from "./settings/ProfileSection";
import { HelpSection } from "./settings/HelpSection";
import { WorldviewSettings } from "./settings/WorldviewSettings";
import { IdentityAnalysisSettings } from "./settings/IdentityAnalysisSettings";
import { DocsScopeSection } from "./settings/DocsScopeSection";

type SettingsSection = "profile" | "worldview" | "identity" | "agents" | "personas" | "skills" | "runtime" | "terminal" | "mobile" | "docs_scope" | "notifications" | "help";

/** Section labels come from `settings.section.*` i18n keys. The order is visual
 *  layout, not config — 새 섹션 추가 시 JSON 키 + 이 배열 양쪽에 등록. */
const SECTION_CONFIG: { id: SettingsSection; icon: React.ReactNode }[] = [
  { id: "profile", icon: <User className="w-4 h-4" /> },
  { id: "worldview", icon: <Globe className="w-4 h-4" /> },
  { id: "identity", icon: <Brain className="w-4 h-4" /> },
  { id: "agents", icon: <Bot className="w-4 h-4" /> },
  { id: "personas", icon: <UserCircle className="w-4 h-4" /> },
  { id: "skills", icon: <Zap className="w-4 h-4" /> },
  { id: "runtime", icon: <Cpu className="w-4 h-4" /> },
  { id: "terminal", icon: <Terminal className="w-4 h-4" /> },
  { id: "mobile", icon: <Smartphone className="w-4 h-4" /> },
  { id: "docs_scope", icon: <FileText className="w-4 h-4" /> },
  { id: "notifications", icon: <Bell className="w-4 h-4" /> },
  { id: "help", icon: <HelpCircle className="w-4 h-4" /> },
];

interface SettingsPanelProps {
  onClose: () => void;
  initialSection?: string;
}

export function SettingsPanel({ onClose, initialSection }: SettingsPanelProps) {
  const valid: SettingsSection[] = ["profile", "worldview", "identity", "agents", "personas", "skills", "runtime", "terminal", "mobile", "docs_scope", "notifications", "help"];
  const initial = (initialSection && valid.includes(initialSection as SettingsSection))
    ? (initialSection as SettingsSection)
    : "profile";
  const [activeSection, setActiveSection] = useState<SettingsSection>(initial);
  const { t } = useTranslation("settings");

  return (
    <div className="fixed inset-0 z-[80] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/30" onClick={onClose} />

      <div className="relative bg-sidebar border border-border/40 rounded-xl shadow-2xl w-[80vw] max-w-[900px] h-[70vh] max-h-[600px] overflow-hidden flex flex-col">
        <div className="flex items-center px-5 h-12 shrink-0">
          <span className="text-[14px] font-[550] text-foreground flex-1">{t("title")}</span>
          <button onClick={onClose} className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex flex-1 min-h-0">
          <nav className="w-[180px] shrink-0 px-3 py-2 space-y-0.5">
            {SECTION_CONFIG.map((section) => (
              <button
                key={section.id}
                onClick={() => setActiveSection(section.id)}
                className={cn(
                  "w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-[13px] font-medium transition-colors text-left",
                  activeSection === section.id
                    ? "bg-accent text-foreground"
                    : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
                )}
              >
                {section.icon}
                {t(`section.${section.id}`)}
              </button>
            ))}
          </nav>

          <div className="flex-1 min-w-0 border-l border-border/30 overflow-y-auto">
            <div className="p-5">
              {activeSection === "profile" && <ProfileSection />}
              {activeSection === "worldview" && <WorldviewSettings />}
              {activeSection === "identity" && <IdentityAnalysisSettings />}
              {activeSection === "agents" && <AgentsSection />}
              {activeSection === "personas" && <PersonasSection />}
              {activeSection === "skills" && (
                <div>
                  <h2 className="text-[14px] font-[550] text-foreground mb-1">{t("section.skills")}</h2>
                  <p className="text-[12px] text-muted-foreground mb-4">{t("skills_description")}</p>
                  <SkillsPanel />
                </div>
              )}
              {activeSection === "runtime" && <RuntimeSection />}
              {activeSection === "terminal" && <TerminalSection />}
              {activeSection === "mobile" && <MobileSection />}
              {activeSection === "docs_scope" && <DocsScopeSection />}
              {activeSection === "notifications" && <NotificationsSection />}
              {activeSection === "help" && <HelpSection />}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
