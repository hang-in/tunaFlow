# Beta E2E Automation (Phase 5 5-6)

HTTP + WS 기반 자동 검증 스크립트. 전체 10개 시나리오 중 **6개를 커버**합니다 (시나리오 1, 3, 5-API, 6, 8). 나머지(2 Plan 사이클, 4 RT, 7 Settings UI, 9 스크롤, 10 재시작)는 UI 표면이 핵심이라 수동 검증으로 남깁니다 — `docs/reference/beta-e2e-checklist.md`.

## 준비

1. tunaFlow 가 실행 중이어야 합니다. `npm run tauri dev` 또는 배포 빌드.
2. Settings > Mobile 에서 API 토큰을 복사합니다.

```bash
export TUNAFLOW_BASE=http://127.0.0.1:8787   # 기본값
export TUNAFLOW_TOKEN=<복사한 토큰>
export E2E_ENGINE=claude                      # 기본값; codex/gemini/ollama 지원
export E2E_PROJECT_PATH=/tmp/tunaflow-e2e    # 옵션; 더미 경로
export VERBOSE=1                              # 옵션; HTTP/WS 로그
```

## 실행

### 전체

```bash
node scripts/beta-e2e/run-all.mjs
```

마지막에 pass/fail 요약 테이블이 출력됩니다. 하나라도 실패하면 exit code 1.

### 개별

```bash
node scripts/beta-e2e/01-project-and-message.mjs
node scripts/beta-e2e/03-branch-lifecycle.mjs
node scripts/beta-e2e/05-meta-inbox.mjs
node scripts/beta-e2e/06-insight-flow.mjs
node scripts/beta-e2e/08-ws-replay.mjs
```

## 커버리지

| # | 시나리오 | API | 파일 |
|---|---------|:---:|------|
| 1 | 프로젝트 생성 → 첫 응답 | 100% | `01-project-and-message.mjs` |
| 3 | Branch lifecycle (create/rename/archive/adopt/delete) | 100% | `03-branch-lifecycle.mjs` |
| 5 | Meta inbox list/read/dismiss | 80% | `05-meta-inbox.mjs` |
| 6 | Insight sessions/findings/status | 100% | `06-insight-flow.mjs` |
| 8 | WS 재연결 + `?since=<ms>` replay | 100% | `08-ws-replay.mjs` |

### 미커버 (수동 검증)

| # | 시나리오 | 이유 |
|---|---------|------|
| 2 | Plan 전체 사이클 | Architect/Dev/Reviewer LLM 호출이 필요, 실행 시간/비용/환경 의존 |
| 4 | RT 다중 참가자 | `POST /roundtables/run` 은 가능하지만 응답 품질과 verdict 집계 검증은 UI 필요 |
| 7 | Settings 프로필 | localStorage 계열 저장, HTTP API 미노출 |
| 9 | 긴 대화 스크롤 | 순수 UI 성능 |
| 10 | 재시작 세션 복구 | 앱 재시작 자체는 자동화 대상 아님 |

## 실패 시

1. Verbose 모드로 재실행: `VERBOSE=1 node scripts/beta-e2e/XX.mjs`
2. HTTP 상태 + 응답 body 가 stderr 에 출력됨
3. 앱의 `~/.tunaflow/crash-reports/` 확인
4. 실패 시나리오 + 로그를 이슈에 첨부

## 주의

- **실제 DB 에 쓰기를 수행합니다** — 테스트 프로젝트 인스턴스에서만 실행하세요
- 스크립트는 생성한 프로젝트/대화를 **자동 정리하지 않습니다**. 세션 후 Sidebar 에서 `[E2E-*]` 라벨 항목을 수동 삭제하세요
- 시나리오 1 은 실제 LLM 호출 → API 비용 발생 가능
