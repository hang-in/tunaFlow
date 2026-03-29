import { useState } from "react";
import { cn } from "@/lib/utils";
import { X, Bot, UserCircle, Zap, Cpu } from "lucide-react";
import { SkillsPanel } from "./context-panel/SkillsPanel";

type SettingsSection = "agents" | "personas" | "skills" | "runtime";

const SECTIONS: { id: SettingsSection; label: string; icon: React.ReactNode }[] = [
  { id: "agents", label: "Agents", icon: <Bot className="w-4 h-4" /> },
  { id: "personas", label: "Personas", icon: <UserCircle className="w-4 h-4" /> },
  { id: "skills", label: "Skills", icon: <Zap className="w-4 h-4" /> },
  { id: "runtime", label: "Runtime", icon: <Cpu className="w-4 h-4" /> },
];

interface SettingsPanelProps {
  onClose: () => void;
}

export function SettingsPanel({ onClose }: SettingsPanelProps) {
  const [activeSection, setActiveSection] = useState<SettingsSection>("skills");

  return (
    <div className="fixed inset-0 z-[80] flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/30" onClick={onClose} />

      {/* Settings window */}
      <div className="relative bg-sidebar border border-border/40 rounded-xl shadow-2xl w-[80vw] max-w-[900px] h-[70vh] max-h-[600px] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center px-5 h-12 shrink-0">
          <span className="text-[14px] font-[550] text-foreground flex-1">Settings</span>
          <button
            onClick={onClose}
            className="p-1.5 rounded-md hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body: nav + content */}
        <div className="flex flex-1 min-h-0">
          {/* Left nav */}
          <nav className="w-[180px] shrink-0 px-3 py-2 space-y-0.5">
            {SECTIONS.map((section) => (
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
                {section.label}
              </button>
            ))}
          </nav>

          {/* Right content */}
          <div className="flex-1 min-w-0 border-l border-border/30 overflow-y-auto">
            <div className="p-5">
              {activeSection === "agents" && (
                <PlaceholderSection
                  title="Agent Profiles"
                  description="에이전트 프로필을 구성합니다. 각 프로필은 엔진, 모델, 페르소나, 기본 스킬 세트를 하나의 실행 단위로 묶습니다."
                  items={["Architect Claude — claude / sonnet / architect persona", "Reviewer Codex — codex / gpt-4.1 / reviewer persona", "Tester Gemini — gemini / 2.5-pro / tester persona"]}
                />
              )}

              {activeSection === "personas" && (
                <PlaceholderSection
                  title="Personas"
                  description="에이전트의 역할과 행동 스타일을 정의합니다. 페르소나는 Agent Profile에서 선택하여 사용합니다."
                  items={["architect — 설계 중심, 구조적 판단", "reviewer — 코드 리뷰, 버그/리스크 발견", "tester — 테스트 관점, 엣지 케이스 탐색", "concise — 간결한 응답 스타일"]}
                />
              )}

              {activeSection === "skills" && (
                <div>
                  <h2 className="text-[14px] font-[550] text-foreground mb-1">Skills</h2>
                  <p className="text-[12px] text-muted-foreground mb-4">에이전트에게 적용할 스킬을 관리합니다. 활성화된 스킬은 모든 에이전트 요청에 포함됩니다.</p>
                  <SkillsPanel />
                </div>
              )}

              {activeSection === "runtime" && (
                <PlaceholderSection
                  title="Runtime"
                  description="런타임 환경을 설정합니다."
                  items={["rawq — 코드 검색 엔진 상태 및 인덱싱 관리", "Context Budget — 컨텍스트 윈도우 크기 제한 (현재 60k chars)", "Model Catalog — 엔진별 사용 가능한 모델 목록 관리", "Daemon — 백그라운드 서비스 상태"]}
                />
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function PlaceholderSection({ title, description, items }: { title: string; description: string; items: string[] }) {
  return (
    <div>
      <h2 className="text-[14px] font-[550] text-foreground mb-1">{title}</h2>
      <p className="text-[12px] text-muted-foreground mb-4">{description}</p>
      <div className="space-y-2">
        {items.map((item, i) => (
          <div key={i} className="flex items-center gap-3 px-4 py-3 rounded-lg border border-border/30 bg-background/50">
            <div className="w-2 h-2 rounded-full bg-muted-foreground/20" />
            <span className="text-[13px] text-muted-foreground/60">{item}</span>
          </div>
        ))}
      </div>
      <p className="text-[11px] text-muted-foreground/30 mt-4 italic">Coming soon</p>
    </div>
  );
}
