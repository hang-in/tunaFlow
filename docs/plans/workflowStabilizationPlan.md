# 워크플로우 안정화 실행 계획

> Status: active
> Created: 2026-04-02
> 선행 조건: 워크플로우 파이프라인 V2 기본 흐름 완료
> 후속: SDK 전환 (docs/ideas/sdkIntegrationIdea.md)

---

## 목표

워크플로우 파이프라인의 정상 경로를 안정화하여 **사용자가 수동 개입 없이** Plan → Dev → Review → Decision 전체 사이클을 완주할 수 있게 한다.

---

## Phase 1: 프롬프트 양식 적용

**문서**: `docs/reference/workflowPromptTemplates.md` (8개 양식 정의 완료)
**목표**: 모든 에이전트 전달 프롬프트를 구조화된 양식으로 통일

### 1-1: Rework 전달 양식 (가독성 가장 심각)

**파일**: `src/components/tunaflow/context-panel/DevProgressView.tsx`
**현재**: findings 전문을 한 덩어리로 전달
**변경**: 체크리스트 형식 + 파일 경로 + 완료 조건

```
┌─ Rework #1 ──────────────────────────┐
│ 수정 항목 (2건):                      │
│ □ 1. {finding 요약}                   │
│   파일: {경로}                        │
│ □ 2. {finding 요약}                   │
│   파일: {경로}                        │
│ 완료 조건: impl-complete 포함         │
└──────────────────────────────────────┘
```

### 1-2: Review / Re-review 양식

**파일**: `src/components/tunaflow/context-panel/DevProgressView.tsx` (handleStartReview)
**현재**: 파일 경로 + verdict 요청
**변경**: 검증 문서 목록 + 이전 findings 체크리스트 (re-review)

### 1-3: Dev 시작 양식

**파일**: `src/lib/workflowOrchestration.ts` (approveAndStartImplementation)
**현재**: 파일 목록 + 규칙 3줄
**변경**: 양식 적용 (이미 경량화 완료, 구조만 정리)

### 1-4: Subtask 수정/대화/작성 양식

**파일**: `src/components/tunaflow/context-panel/SubtaskReviewView.tsx`
**현재**: 경량화 완료 (파일 경로 + 의견)
**변경**: 양식 적용 (구조 정리만)

### 1-5: Plan 승격 문서 작성 양식

**파일**: `src/components/tunaflow/chat/PlanProposalCard.tsx` (handlePromote)
**현재**: 파일 목록 + 포함 내용 + subtask 목록
**변경**: 양식 적용

### 1-6: Plan 문서 반영 양식

**파일**: `src/components/tunaflow/context-panel/SubtaskReviewView.tsx` (handleSyncToMainPlan)
**현재**: 파일 경로 + 반영 지시
**변경**: 양식 적용

### 검증

- 각 양식 적용 후 실제 프롬프트가 구조화되었는지 `tauri dev`에서 확인
- 에이전트가 양식을 이해하고 체크리스트대로 처리하는지 확인

---

## Phase 2: 에이전트 CLI 권한 승인 UI

**배경**: Claude CLI `--permission-mode bypassPermissions`로 전체 허용 중. 사용자가 앱 내에서 권한을 제어할 수 없음.

### 2-1: 프로젝트별 허용 명령 설정

**파일**: Settings > Runtime 또는 프로젝트 설정
**구현**:
- 프로젝트 `.claude/settings.local.json`에 permissions 관리
- UI에서 허용할 명령 패턴 추가/제거 (`Bash(npm install*)`, `Bash(cargo build*)` 등)
- 기본 프리셋: "개발용" (npm/cargo/git 허용), "제한적" (읽기만)

### 2-2: 런타임 권한 요청 표시

**구현**:
- 에이전트가 권한 요청 시 앱에서 toast/modal 표시
- 승인/거부 → CLI에 전달
- 난이도 높음 — CLI와의 양방향 통신 필요

### 판단

2-1은 설정 UI로 비교적 간단. 2-2는 SDK 전환 후에 자연스럽게 해결 (tool call의 `requires_approval`).
**2-1만 우선 구현**, 2-2는 SDK 전환 시.

---

## Phase 3: 에러 경로 처리 (최소한)

**배경**: 에이전트 hang (2시간), UTF-8 panic, 마커 미감지 등 발생.

### 3-1: Idle timeout

**파일**: `src-tauri/src/agents/claude.rs`, `gemini.rs` 등
**구현**:
- 에이전트 프로세스에서 마지막 출력 후 N분(예: 10분) 경과 시 타임아웃
- 타임아웃 시 프로세스 kill + `agent:error` 이벤트 발행
- 사용자에게 toast 알림

### 3-2: Streaming 상태 복구

**파일**: `src-tauri/src/commands/agents.rs`, 또는 앱 시작 시
**구현**:
- 앱 시작 시 `status='streaming'`인 메시지를 `status='error'`로 전환
- stale job 정리 (`agent_jobs.status='running'` + 오래된 것)

### 3-3: fire-and-forget 최소 warn

**범위**: `catch { /* silent */ }` 패턴 전체
**구현**: `catch { console.warn(...) }` 또는 Rust `eprintln!` 추가

### 판단

3-1이 가장 중요 (hang 방지). 3-2는 간단. 3-3은 코드 전체 스캔 필요.
SDK 전환 시 idle timeout + stall detection이 네이티브로 제공되므로 깊게 만들지 않음.

---

## 실행 순서

| 순서 | 작업 | 예상 | 비고 |
|------|------|------|------|
| 1 | Phase 1-1: Rework 양식 | 빠름 | 가독성 가장 심각 |
| 2 | Phase 1-2: Review 양식 | 빠름 | |
| 3 | Phase 1-3~6: 나머지 양식 | 빠름 | 구조 정리만 |
| 4 | Phase 3-1: Idle timeout | 중간 | hang 방지 |
| 5 | Phase 3-2: Streaming 복구 | 빠름 | 앱 시작 시 |
| 6 | Phase 2-1: 허용 명령 설정 | 중간 | Settings UI |
| 7 | Phase 3-3: warn 추가 | 낮음 | 전체 스캔 |

---

## 완료 기준

- [ ] 모든 프롬프트가 구조화된 양식으로 전달됨
- [ ] 에이전트 hang 시 10분 후 자동 타임아웃 + 알림
- [ ] 앱 재시작 시 stale streaming 자동 정리
- [ ] 프로젝트별 CLI 허용 명령 설정 가능

## 후속

완료 후 → SDK 전환 (docs/ideas/sdkIntegrationIdea.md) Phase 1 (Gemini SDK) 착수
