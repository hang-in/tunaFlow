import { useEffect, useState } from "react";
import { ExternalLink, Keyboard, Lightbulb, AlertTriangle, FileWarning } from "lucide-react";
import { listRecentCrashReports, type CrashReportSummary } from "@/lib/crashReporter";

export function HelpSection() {
  const [reports, setReports] = useState<CrashReportSummary[]>([]);
  useEffect(() => {
    listRecentCrashReports(5).then(setReports);
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1">Help</h2>
        <p className="text-[12px] text-muted-foreground">
          tunaFlow 주요 기능, 키보드 단축키, 문제 해결 가이드.
        </p>
      </div>

      {reports.length > 0 && (
        <section>
          <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
            <FileWarning className="w-4 h-4 text-amber-400" />
            최근 크래시 리포트 ({reports.length})
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
              파일 위치: <code>~/.tunaflow/crash-reports/</code> · 이슈 신고 시 첨부해주세요.
            </div>
          </div>
        </section>
      )}

      <section>
        <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
          <Keyboard className="w-4 h-4 text-muted-foreground" />
          키보드 단축키
        </h3>
        <div className="rounded-lg border border-border/40 divide-y divide-border/40">
          {SHORTCUTS.map(([keys, desc]) => (
            <div key={keys} className="flex items-center justify-between px-3 py-2 text-[12px]">
              <kbd className="font-mono text-[11px] bg-muted/50 px-2 py-0.5 rounded border border-border/40">
                {keys}
              </kbd>
              <span className="text-muted-foreground">{desc}</span>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
          <Lightbulb className="w-4 h-4 text-muted-foreground" />
          주요 기능
        </h3>
        <div className="space-y-3 text-[12px]">
          {FEATURES.map((f) => (
            <div key={f.title}>
              <div className="text-foreground font-[500] mb-0.5">{f.title}</div>
              <div className="text-muted-foreground leading-relaxed">{f.desc}</div>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h3 className="flex items-center gap-2 text-[13px] font-[500] text-foreground mb-2">
          <AlertTriangle className="w-4 h-4 text-muted-foreground" />
          문제 해결
        </h3>
        <div className="space-y-3 text-[12px]">
          {TROUBLESHOOTING.map((t) => (
            <div key={t.title}>
              <div className="text-foreground font-[500] mb-0.5">{t.title}</div>
              <div className="text-muted-foreground leading-relaxed whitespace-pre-line">{t.desc}</div>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h3 className="text-[13px] font-[500] text-foreground mb-2">외부 자료</h3>
        <div className="space-y-1.5 text-[12px]">
          <LinkItem href="https://github.com/hang-in/tunaFlow" label="GitHub 저장소" />
          <LinkItem href="https://github.com/hang-in/tunaFlow/issues" label="이슈 / 버그 신고" />
          <LinkItem href="mailto:d9ng@outlook.com" label="이메일 문의: d9ng@outlook.com" />
        </div>
      </section>
    </div>
  );
}

const SHORTCUTS: [string, string][] = [
  ["Cmd+K", "명령 팔레트 열기"],
  ["Cmd+Enter", "현재 입력 전송"],
  ["Shift+Enter", "입력창 줄바꿈"],
  ["Esc", "드로어/모달 닫기"],
  ["Tab", "포커스 순환 이동"],
];

const FEATURES: { title: string; desc: string }[] = [
  {
    title: "Plan → Develop → Review",
    desc: "Architect 가 Plan 을 설계하면 Developer 가 구현하고 Reviewer 가 검증합니다. 실패하면 findings 를 기반으로 rev.N+1 Plan 을 자동 제안합니다.",
  },
  {
    title: "Branch / Roundtable",
    desc: "대화 중간에서 Branch 로 분기해 독립 실험을 하고 결과만 요약 삽입(adopt)할 수 있습니다. RT 는 여러 엔진이 순차/동시로 토론하는 Branch 확장 모드입니다.",
  },
  {
    title: "ContextPack",
    desc: "매 요청마다 프로젝트 문서, 기억, 도구 결과를 엔진 공통 포맷으로 조립합니다. Lite/Standard/Full 자동 전환으로 토큰을 절약합니다.",
  },
  {
    title: "Insight",
    desc: "안정성·테스트·아키텍처·성능·보안·기술부채 6개 카테고리로 프로젝트를 분석합니다. Quick Wins 는 에이전트가 자동 수정할 수 있습니다.",
  },
  {
    title: "PTY Terminal",
    desc: "CLI 에이전트와 `-p` 플래그 없이 인터랙티브 세션을 유지합니다. 파일 수정, 명령 실행 등 전체 도구 사용이 가능합니다.",
  },
];

const TROUBLESHOOTING: { title: string; desc: string }[] = [
  {
    title: "에이전트가 응답하지 않아요",
    desc: "Settings > Agents 에서 해당 CLI 가 감지되는지 먼저 확인하세요. Claude/Codex/Gemini 는 각자 한 번 로그인이 필요합니다. 로그인 후에도 응답이 없으면 상단 RuntimeStatusBar 를 눌러 프로세스 상태를 확인하세요.",
  },
  {
    title: "Insight 분석이 시작하자마자 초기화됩니다",
    desc: "Insight 실행 중 다른 탭으로 이동해도 상태는 Zustand 에 저장됩니다. 계속 초기화가 발생하면 Settings > Runtime 에서 rawq daemon 상태를 확인해주세요.",
  },
  {
    title: "CPU 사용률이 높습니다",
    desc: "bge-m3 임베딩 인덱싱 중일 수 있습니다. Settings > Runtime 에서 `증분 인덱싱 전용` 모드로 전환하면 대규모 재인덱싱을 피할 수 있습니다.",
  },
  {
    title: "macOS 에서 앱이 열리지 않습니다",
    desc: "ad-hoc 서명 빌드이므로 Gatekeeper 가 차단할 수 있습니다.\n터미널에서 `xattr -cr /Applications/tunaFlow.app` 실행 후 다시 열어보세요.",
  },
];

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
