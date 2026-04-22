# 세션 18 핸드오프 프롬프트

> 아래 내용을 새 세션의 첫 메시지로 사용하세요.

---

tunaFlow 세션 18 시작.

## 프로젝트 개요

tunaFlow는 **다중 에이전트 오케스트레이션 클라이언트(AOC)**. Tauri 2 + React + TypeScript + Rust + SQLite.
프로젝트 단위로 Claude/Codex/Gemini 에이전트를 실행하며, Roundtable 토론, Branch 분기, Plan/Artifact 관리, ContextPack 맥락 조립 등을 지원한다.

핵심 철학: "에이전트가 편해야 결과가 좋다" — 에이전트 능력의 근본적 확장을 위한 설계.

## 현재 상태

- **Branch**: `feature/pty-interactive` (main에 미머지, origin에 push 완료)
- **세션 16~17**: PTY 인터랙티브 모드 구현 — CLI의 `-p` 모드(stateless)에서 PTY(stateful) 모드로 전환
- **총 66커밋** on feature/pty-interactive (43파일, +4987줄)

## PTY 아키텍처 (세션 16~17에서 구현)

### 핵심 정책 (docs/ideas/ptySessionPolicy.md)
- **채팅 = 세션 1:1**: 각 Conversation이 하나의 CLI 세션(resume_token)에 대응
- **세션 재사용**: 앱 재시작 시 `--resume <sessionId>`로 기존 세션 이어감
- **JSONL 식별**: resume_token = JSONL 파일명, DB에 영속
- **RT는 `-p` 유지**: 1턴 발언, stateful 불필요, ContextPack으로 맥락 주입
- **Branch**: 현재 `-p` 유지 (PTY 부모 공유는 보류 — shadow conv 분리 이슈)
- **워크플로우 에이전트(Developer/Reviewer)**: `-p` 모드 유지 (정밀 맥락 주입 > 연속성)
- **`-p` fallback**: PTY 실패 시 자동 전환

### 구현 완료 (Phase 1~5)
1. **Phase 1 안정화**: /clear, -p fallback, 자동 재시작, Settings PTY/CLI 토글
2. **Phase 2 ContextPack**: 새 세션 첫 메시지 전체 주입 + 기존 세션 CLAUDE.md delta 갱신
3. **Phase 3 멀티엔진**: Codex/Gemini 파서(pty_poll_codex, pty_poll_gemini) + 엔진 일반화
4. **Phase 4 워크플로우**: 마커 감지 + HMR cleanup
5. **Phase 5 UI**: TerminalPanel → RuntimeStatusBar 아이콘 토글

### 핵심 파일
| 파일 | 역할 |
|------|------|
| `src-tauri/src/commands/pty.rs` | PTY spawn/write/kill + JSONL 파싱 (Claude/Codex/Gemini) + ContextPack 빌드 + CLAUDE.md 갱신 |
| `src/stores/ptyStore.ts` | PTY 세션 상태 (sessions, jsonlPath, completion detection) |
| `src/stores/slices/conversationSlice.ts` | spawnPtyForConversation (엔진별 spawn + resume) |
| `src/stores/slices/runtimeSlice.ts` | sendViaPty (200ms JSONL polling + tool steps + ContextPack) |
| `src/components/tunaflow/message/ToolStepsView.tsx` | tool steps UI (스트리밍 + collapsed + output 토글) |
| `src/components/tunaflow/RuntimeStatusBar.tsx` | TerminalPanel 토글 아이콘 |
| `docs/ideas/ptySessionPolicy.md` | 전체 정책 문서 |

### 알려진 이슈 / 미완료
- **Branch PTY 부모 공유**: shadow conv에 응답 저장해야 하는데, PTY 세션은 부모에 연결 → 복잡. 보류.
- **Codex app-server**: `codex app-server` 실험적 HTTP/WS 모드 — PTY/JSONL 대안 검토 필요 (memory: project_codex_app_server.md)
- **Codex/Gemini PTY 실사용 테스트**: 파서는 구현했지만 실제 PTY 모드로 Codex/Gemini 구동 테스트 미진행
- **TERM=dumb 제거 사이드이펙트**: thinking 내용 보존을 위해 제거했는데, TUI 출력이 PTY에 나옴. TerminalPanel에서 보이므로 문제는 아니지만 확인 필요
- **CLAUDE.md 동적 갱신**: lite 모드로 갱신 중인데, 이게 프로젝트의 기존 CLAUDE.md를 덮어쓰지 않는지 확인 필요 (## tunaFlow Context 섹션만 교체하도록 구현)

## 다음 우선순위

1. **실사용 검증** — PTY 모드로 실제 프로젝트(seCall, gemento) 워크플로우 풀사이클 테스트
2. **Codex app-server 프로토콜 분석** — `codex app-server` 실행, TS 바인딩 생성, 프로토콜 파악
3. **Branch PTY 공유** — shadow conv 분리 문제 해결 후 구현
4. **main 머지 준비** — feature/pty-interactive 안정화 확인 후 main에 머지

## 참고 문서
- CLAUDE.md §5: 세션별 완료 항목 + 알려진 이슈
- memory/project_session_2026-04-11_s17.md: 세션 17 상세
- memory/feedback_pty_parser_per_engine.md: 엔진별 파서 정책
- memory/project_pty_value.md: PTY의 제품 가치
- memory/feedback_no_sdk.md: CLI-first 아키텍처 원칙
- memory/project_codex_app_server.md: Codex app-server 레퍼런스
- docs/ideas/ptySessionPolicy.md: PTY 전체 정책
- docs/ideas/ptyFullIntegrationPlan.md: Phase 1~5 로드맵
