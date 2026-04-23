---
title: 베타 공개 전 코드베이스 전수감사 (2026-04-23)
status: ready-to-implement
priority: P0 (베타 공개 blocker)
created_at: 2026-04-23
related:
  - docs/plans/publicReadinessChecklistPlan.md
  - docs/plans/i18nPlan.md
  - docs/plans/insightStabilityPlan.md
---

# 베타 공개 전 코드베이스 전수감사 결과

6개 영역 병렬 감사 수행: (1) 보안·공개 준비 (2) Rust 코드 품질 (3) 프론트엔드 코드 품질 + i18n (4) 빌드·린트·의존성 (5) 테스트 스위트 (6) 문서·Plan 정합성.

## TL;DR

| 영역 | P0 | P1 | P2 | 양호 하이라이트 |
|---|---|---|---|---|
| 보안/공개 | 3 | 5 | 4 | API key 0건, LICENSE 정합, .gitignore 전반 OK |
| Rust 품질 | 0 | 4 | 2 | Mutex 재진입 0, unsafe 주석 있음, FK ON |
| FE 품질/i18n | 3 | 5 | 3 | 빈 testt.only 없음, cleanup 대부분 OK |
| 빌드/린트 | 1 | 3 | 3 | tsc 0 에러, vite 0 경고, 소스맵 미공개 |
| 테스트 | 2 | 2 | 4 | skip/ignore 0건, 322+485 pass, CI 19/20 |
| 문서 | 2 | 3 | 3 | broken 내부 링크 0, 아카이브 구조 정상 |
| **합계** | **11** | **22** | **19** | — |

**베타 공개 최소 조건**: P0 11건 중 **실제 blocker 6건 처리** (나머지 5건은 경고성). 약 1시간 작업 + evals 원인 조사 시간.

---

## 🔥 P0 — 베타 공개 blocker (6 must-fix)

### 1. `.tunaflow/outbox/*.md` tracked
4개 파일이 git 에 올라간 상태. 에이전트 대화 로그가 공개 repo 에 노출. 파일:
- `.tunaflow/outbox/1775854254733.md`
- `.tunaflow/outbox/1775854565931.md`
- `.tunaflow/outbox/1775854713667.md`
- `.tunaflow/outbox/1775854842763.md`

**수정**:
```bash
git rm --cached .tunaflow/outbox/*.md
echo ".tunaflow/" >> .gitignore
```

### 2. `package.json` / `Cargo.toml` 메타 필드 부재
- `package.json` — `license`, `author`, `repository` 없음
- `src-tauri/Cargo.toml` — `license`, `authors`, `repository`, `description` 없음
- npm/crates 공개 시 라이선스 자동탐지 실패 + 검색 노출 저하

**수정**:
```json
// package.json
"license": "Apache-2.0",
"author": "d9ng <d9ng@outlook.com>",
"repository": { "type": "git", "url": "https://github.com/<user>/tunaFlow" }
```
```toml
# src-tauri/Cargo.toml
license = "Apache-2.0"
authors = ["d9ng <d9ng@outlook.com>"]
repository = "https://github.com/<user>/tunaFlow"
description = "Multi-agent orchestration client for CLI coding agents"
```

### 3. `NOTICE` 파일 부재 + 제3자 attribution 누락
Apache 2.0 배포 의무 + MIT 호환 파생 저작물 고지.

**필수 attribution 대상**:
- `rawq` (MIT, github.com/auyelbekov/rawq) — code search sidecar, 로컬 patch 적용
- `codex` (Apache 2.0, OpenAI) — `_util/codex/` archive (publicReadiness Phase 1 에서 제거됐는지 재확인)
- `xterm.js` — PTY 터미널
- `react-markdown` / `remark-gfm` / `react-syntax-highlighter`
- `D2Coding` fonts (OFL, 8MB tracked)
- `lucide-react`, `i18next`, `tauri`, `zustand` 등 주요 의존성

**수정**: `NOTICE` 파일 생성 + README.md 의 "References & Acknowledgments" 섹션 실제 내용으로 채움.

### 4. `cargo check --all-targets` 실패
**파일**: `src-tauri/tests/db_integration.rs:68`
**증상**: `db::migrations::run(&conn)` 시그니처 mismatch. E0308 3건 + compile failure 1건. 통합 테스트 컴파일 불가 상태.

**수정**:
```rust
// tests/db_integration.rs:68
// 기존: db::migrations::run(&conn)
// 수정: db::migrations::run(&mut conn)
```
(또는 `migrations::run()` 시그니처를 `&Connection` 으로 되돌림 — 최근 변경 이력 확인 후 결정)

### 5. evals 최신 실행 결과 2/20 pass (10%)
**파일**: `evals/results/2026-04-22T06-48-16-631Z-claude-default.json`
`avg_score = 0.14`. `evals/README.md` 의 게이트 기준 (<50% 시 block) 아래.

**원인 후보**:
- golden JSONL 회귀 (schema drift)
- judge 프롬프트 변경 후 scoring 기준 이동
- claude CLI 버전 업그레이드로 응답 포맷 drift (insightStability Subtask 03 관련)
- ContextPack 변경 후 하위 응답 품질 하락

**조치**: evals 재실행 + 첫 3~5 failure 상세 분석. golden 또는 judge 중 어느 쪽이 원인인지 확정. **원인 조사 시간 불확실** — 베타 공개 전 블록할지, "evals 게이트 임시 완화 + 베타 후 원인 조사" 로 갈지 사용자 판단 필요.

### 6. `docs/plans/index.md` 에 `publicReadinessChecklistPlan` 미등재
README.md 의 "Beta" 배지가 이 plan 을 링크하고 있음에도 index 에 등재 안 됨. 외부 사용자가 공개 준비 현황 탐색 불가.

**수정**: index.md 의 "🟢 진행 예정 / 진행 중" 테이블에 한 줄 추가:
```md
| [publicReadinessChecklistPlan](./publicReadinessChecklistPlan.md) | **P0** — OSS 공개 준비 체크리스트 (Apache 2.0 LICENSE + hygiene 파일 + root cleanup + README 보강 + 크리티컬 점검) |
```

---

## ⚠️ P1 — 공개 전 권장 (1시간 내 처리 가능)

### 7. docs/ 하드코딩 개인 경로 40+건
`/Users/d9ng/...` 하드코딩이 베타 문서에 그대로 노출. 대상:
- `docs/how-to/rawq-setup.md:61`
- `docs/plans/insightStabilityPlan.md:172, 232`
- `docs/ideas/abtopAnalysisForTunaFlow.md:5-6`
- `docs/archive/prompts/one-time/*.md` 다수
- `docs/reference/aistartkit-harness-evaluation_2026-04-21.md:14, 30`

**수정**: `sd '/Users/d9ng/privateProject/tunaFlow' '<repo-root>' docs/**/*.md` 일괄 치환 (또는 sed 등가)

### 8. `evals/golden/review-verdict-mixed.jsonl` 프라이빗 경로 노출
다른 프로젝트 경로 포함: `/Users/d9ng/privateProject/seCall/...`, `/Users/d9ng/privateProject/tunaInsight/...`. 프라이빗 프로젝트명 노출.

**수정**: 경로 sanitize (`<project>/...` 로 placeholder 치환) 또는 익명화.

### 9. i18n 하드코딩 한국어 잔존 4건
A2 미완료 커버리지:
- `src/components/tunaflow/TerminalPanel.tsx:182, 194, 197` — "Terminal을 사용하려면 프로젝트를 선택하세요", "Claude 재시작", "재시작"
- `src/components/tunaflow/context-panel/IdentityView.tsx:86` — "프로젝트를 선택하세요."
- `src/components/tunaflow/settings/AgentsSection.tsx:321` — hint "한국어 요약"

A2-D 슬라이스 범주로 포함하여 처리.

### 10. PTY/background Promise cleanup 누락
- `src/components/tunaflow/RuntimeStatusBar.tsx:221, 225` — `listen("background_insight_progress")` unlisten 후 `.catch(() => {})` silent, listen 실패 시 cleanup 안 됨
- `src/components/tunaflow/TerminalPanel.tsx:115~128` — `listen<"pty:output">` useEffect 재실행 시 race condition

**수정**: catch 에 console.error / toast 추가, cleanupRef 패턴 재검토.

### 11. Rust LLM 응답 parsing OOB 위험 (Subtask 05 감)
- `src-tauri/src/agents/identity_analyzer.rs:192` — `unwrap_or(content.len() - body_start)` 자체가 OOB 위험
- `src-tauri/src/agents/gemini_sdk.rs:101-102` / `openai_sdk.rs:104-105` — SSE parsing slice 경계 재검증 필요
- `src-tauri/src/commands/roundtable_helpers/prompt.rs:207-222` — LLM 응답 parsing 에 `unwrap()` 6개

**수정**: `strip_prefix` / `checked_sub` / `get(..)` 류 안전 API 로 교체. `.ok_or_else()` + AppError 반환.

### 12. Silent catch 28건 (FE) + `.ok()` drop ~20곳 (Rust)
사용자에게 에러 피드백 없이 무시. `feedback_error_visibility` 기억에 따르면 "silent catch 금지, toast/console 로 표면화" 가 규칙인데 다수 위반. **전수 fix 는 베타 후 별도 plan**, 다만 hot path (workflow / branch / insight) 에 한해 우선 점검.

### 13. React / Vite / TypeScript major version lag
- react / react-dom 18 → **19**
- vite 5 → **8**
- typescript 5 → **6**
- @vitejs/plugin-react 4 → **6**

베타에 영향 없음. **베타 후 별도 업그레이드 plan** 으로 등록.

### 14. README 내부 용어 미정의
"Branch-adopt", "ContextPack", "rawq", "Roundtable(RT)" 가 첫 등장 시 정의 없이 사용. 외부 독자 진입 장벽.

**수정**: README 상단에 **Glossary** 섹션 추가 또는 첫 등장 시 짧은 설명 괄호 추가.

### 15. critical path 무테스트 (6곳)
- `src/stores/slices/threadSlice.ts` (RT/streaming core)
- `src/lib/insightOrchestration.ts`
- `src/lib/toolRequestHandler.ts`
- `src-tauri/src/agents/gemini.rs` (4-engine parity 주장과 모순)
- `src-tauri/src/agents/loader.rs`
- `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs`

베타 후 단위 테스트 보강 plan 필요. `promptRegressionEvalPlan` 에 병합 가능.

### 16~21: 나머지 P1 (간단 요약)
- `cargo-audit` 미설치 → 설치 + 실행 권장
- npm audit moderate 2건 (esbuild/vite, dev-server 한정, 위험 낮음)
- `CLAUDE.md §11` stale 항목 (라이트모드 / Project-per-window)
- `implementationStatus.md` Rust 테스트 수치 stale (232 → 485)
- `sessionHistory.md` 세션 번호 inconsistency (21, 35 비고)
- `designReviewGatePlan-task-*.md` 4개 subtask 파일 index.md 미등재

---

## 📝 P2 — 베타 후 처리

- Rust `println!/eprintln!` 245건 → `tracing` 로거 점진 치환
- FE `console.log` 79건 → build stripping 또는 logger 래퍼
- clippy pedantic 2,406 warning (default lint 깨끗) → 점진 정리
- 대용량 chunk 1.6MB (Tauri 데스크톱이라 영향 없음)
- TODO/FIXME 2건 (Insight 카테고리 키워드, 의도적)
- SECURITY 제보 `d9ng@outlook.com` 을 `security@tunaflow.dev` 로 전용 별칭 권장
- `public/fonts/D2Coding-*.ttf` 8MB+ tracked → NOTICE 에 포함 (P0 #3 의 일부)
- `src/test-utils/` 공용 mock/fixture 디렉토리 신설 (현재 중복 없음, 예방)
- `docs/archive/prompts/one-time/` 다수 개인 경로 → archive 라 공개해도 되지만 최소 sanitize

---

## ✅ 감사 통과 영역

- `tsc --noEmit` 0 에러 / 0 경고
- `vite build` 0 경고 + 소스맵 미공개 (prod safe)
- `cargo check` lib 자체는 0 에러 (tests 만 실패)
- npm audit high/critical 0건
- Vitest 322 pass / 0 fail / 0 skip
- Cargo test --lib 485 pass / 0 fail / 0 ignored
- CI 최근 20 run 19 success (flaky 0)
- LICENSE Apache 2.0 전문 + `Copyright 2026 d9ng, tunaflow.dev` 정확
- `.gitignore` 전반 완전 (`.tunaflow/` 만 누락)
- PRAGMA `foreign_keys = ON` 적용
- 마이그레이션 `add_column_if_missing` idempotent
- `evals/scripts/run-eval.mjs` cleanup opt-out 전환 완료
- Rust Mutex 재진입 위험 0건
- `unsafe` 블록 2개 (sqlite_vec FFI, 주석 명시)
- `evals/golden/` JSONL 5 카테고리 × 4 = 20개 README 약속 일치
- Rust `panic!()/todo!()/unimplemented!()` 모두 테스트 코드에 격리
- 하드코딩 API key / secret 0건 (grep 전수)
- 빈 테스트 body / `assert!(true)` 스모크 0건
- React anti-pattern 심각 사례 없음
- WebSocket / EventSource / addEventListener cleanup 대부분 구현

---

## Developer 핸드오프 프롬프트

> 새 세션에 아래 blob 을 통째로 붙여넣는다. 베타 공개 최종 정리 PR.

```
[작업] 베타 공개 전 P0 블로커 6건 수정 + 주요 P1 5건 처리

[SSOT] docs/plans/preBetaAuditPlan_2026-04-23.md 먼저 읽기

[P0 — 반드시]

1. `.tunaflow/outbox/` untrack:
   git rm --cached .tunaflow/outbox/*.md
   echo ".tunaflow/" >> .gitignore

2. package.json + src-tauri/Cargo.toml 메타 필드 추가:
   - license, author, repository (package.json)
   - license, authors, repository, description (Cargo.toml)
   - 값은 plan §"P0 #2" 예시 참고

3. NOTICE 파일 생성 + README "References & Acknowledgments" 섹션 실제 채움:
   - rawq (MIT), codex (Apache), xterm.js (MIT), react-markdown (MIT),
     D2Coding fonts (OFL), lucide-react (ISC), i18next (MIT), tauri (Apache),
     zustand (MIT) 최소 커버

4. tests/db_integration.rs:68 시그니처 수정:
   - cargo check --all-targets 통과 확인

5. docs/plans/index.md 에 publicReadinessChecklistPlan 등재
   (테이블 진행 중 섹션에 한 줄 추가)

6. evals 2/20 pass 원인 조사:
   - 최신 results 파일의 첫 3~5 failure 분석
   - golden 회귀 vs judge 변경 vs claude CLI drift 중 확정
   - 원인 복구 또는 별도 이슈 등록 후 게이트 임시 완화 결정 (사용자 확인 필요)

[P1 — 1시간 내]

7. docs/ 하드코딩 경로 sanitize:
   sd '/Users/d9ng/privateProject/tunaFlow' '<repo-root>' docs/**/*.md

8. evals/golden/review-verdict-mixed.jsonl 프라이빗 경로 마스킹

9. TerminalPanel / IdentityView / AgentsSection i18n 한국어 4건 이관
   (A2-D 슬라이스에 포함)

10. identity_analyzer.rs:192 / gemini_sdk:101-102 / openai_sdk:104-105 /
    roundtable_helpers/prompt.rs:207-222 안전 API 로 교체
    (strip_prefix, checked_sub, get(..), ok_or_else)

11. RuntimeStatusBar / TerminalPanel Promise cleanup 보강
    (.catch 에 console.error 또는 toast)

[검증]
- cargo check --all-targets: 에러 0
- npx tsc --noEmit: 0
- npx vitest run / cargo test --lib: 전량 pass
- evals 재실행: pass율 정상화 확인 (또는 게이트 임시 완화 결정)
- .tunaflow/outbox 에 신규 파일 생기지 않는지 확인 (gitignore 작동)

[커밋]
- chore(gitignore): untrack .tunaflow/outbox logs
- chore(meta): add license/author fields to package.json + Cargo.toml
- docs(notice): add NOTICE + README References section for third-party attribution
- fix(tests): correct migrations::run signature in db_integration.rs
- docs(index): register publicReadinessChecklistPlan
- fix(evals): <원인 복구 or 게이트 조정>
- chore(docs): sanitize hardcoded paths
- chore(evals): mask private project paths in golden data
- feat(i18n): migrate TerminalPanel/IdentityView/AgentsSection hardcoded Korean (A2-D)
- fix(rust): replace unsafe slicing/unwrap in LLM response parsing
- fix(fe): propagate errors in RuntimeStatusBar/TerminalPanel listen cleanup

각 커밋 본문에 Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
fix(beta): pre-release audit — 6 blockers + 5 critical P1 (license, NOTICE, gitignore, cargo check, evals)

[주의]
- git stash drop/clear 금지
- destructive ops 사용자 승인 후에만
- NOTICE 작성 시 라이선스 문구는 공식 template 사용 (추측 금지)
- evals 원인 조사에서 시간 초과 시 사용자에게 중간 보고, 독자 판단 금지
```

## Invariants

- **[INV-1]** 공개 repo 에는 `.tunaflow/` (사용자 로컬 workspace / outbox) 가 tracked 돼서는 안 된다. 이유: 에이전트 대화 로그 / 로컬 상태 파일의 외부 노출. **검증**: `git ls-files .tunaflow/` 결과 0.
- **[INV-2]** `NOTICE` 및 README References 섹션은 **실제 번들/의존하는 모든 MIT/Apache/OFL 라이선스 제3자 프로젝트** 를 열거. Apache 2.0 배포 조건. **검증**: package.json + Cargo.toml 의 의존성 중 Apache/MIT/OFL 라이선스를 가진 것이 NOTICE 에 포함됐는지 샘플 확인.
- **[INV-3]** 모든 manifest 파일 (package.json / Cargo.toml) 은 license / author / repository 필드 필수. **검증**: `jq .license package.json` / `grep -E "^(license|authors)" src-tauri/Cargo.toml`.
- **[INV-4]** evals 게이트 (README 기준 <50% pass 시 block) 가 실제로 CI 에서 강제되는지 확인. 현재는 재현 수치 (10%) 만 있고 block 여부 불명 — CI config 확인 필요.
- **[INV-5]** 베타 공개 후 `cargo check --all-targets` 는 항상 에러 0. 테스트 컴파일 깨지면 CI 게이트가 실질적으로 동작 안 함.

## Rationale

감사는 6개 영역 병렬 수행 (총 5~10분 소요) — 보안/공개, Rust 품질, FE 품질+i18n, 빌드/린트/의존성, 테스트 무결성, 문서/Plan 정합성.

**도출 근거**:
- P0 선정 기준 = "공개 후 외부 사용자가 즉시 접하는 문제" OR "법적/라이선스 리스크" OR "CI/빌드 본질 파손"
- P1 선정 기준 = "사용자 경험 저하 but 공개 자체는 가능" OR "베타 후 빠른 follow-up 필요"
- P2 선정 기준 = "체감 없음, 장기 품질"

**전체 상태 평가**: P0 11건 중 실제 blocker 는 6건 (나머지 5건은 경고성 — `cargo check` 에러가 tests 만이라 실제 배포엔 영향 없고, evals 는 내부 게이트). 각 1~30분 작업. **evals 원인 조사만 시간 불확실 — 원인 파악 후 게이트 임시 완화로 공개 가능**.

양호 영역이 상당히 많음. 특히:
- 테스트 스위트 skip/ignore 0건 = 사문화 없음
- tsc/vite 빌드 완전 clean
- API key 노출 0건
- Mutex 재진입 / silent panic 없음

AI 작성 코드베이스임에도 코드 품질은 양호한 수준. 주요 이슈는 "공개 준비 체크리스트 마지막 정리" 수준.

## 관련 문서

- `docs/plans/publicReadinessChecklistPlan.md` — 이미 진행된 공개 준비 (Phase 0~4 완료)
- `docs/plans/insightStabilityPlan.md` — 세션 40 머지 완료
- `docs/plans/i18nPlan.md` — A1+A2+A2-B+A2-C 머지, A2-D~G + A3 진행 중
- `docs/prompts/nextSessionHandoff_2026-04-24.md` — 다음 세션 SSOT
