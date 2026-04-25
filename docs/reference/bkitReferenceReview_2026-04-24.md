---
title: bkit (popup-studio-ai) 레퍼런스 검토 — tunaFlow 관점 분석
canonical: false
status: review
created_at: 2026-04-24
author: Architect (Claude Opus 4.7)
related:
  - https://github.com/popup-studio-ai/bkit-claude-code
  - https://github.com/popup-studio-ai/bkit-codex
  - https://github.com/popup-studio-ai/bkit-gemini
---

# 개요

외부 분석가 (ChatGPT) 가 `popup-studio-ai/bkit` 계열 프로젝트를 tunaFlow 레퍼런스로 볼 가치가 있는지 검토 요청. 세 레포 (`bkit-claude-code`, `bkit-codex`, `bkit-gemini`) 를 `_util/` 에 shallow clone 하고 README / CUSTOMIZATION-GUIDE / bkit-system / skills/ 를 직접 읽고 작성한 분석.

본 문서는 **평가/비교 메모**. 실행 plan 은 후속 plan 파일로 분리.

# TL;DR

- **참고 가치 있음**, 단 "구조 차용" 아니라 "설계 패턴 3~4개 + 생태계 관계 명시" 로 제한.
- 외부 분석가 프롬프트가 두 프로젝트를 수평 비교 구도로 세팅했는데, **실물은 수직 보완 관계**. 재프레임 필요.
- 즉시 가치: Skill classification frontmatter, docs-code-sync CI, Context Engineering 용어 정리.
- 중기 가치: level-based preset, defense-in-depth hook 레이어, bkit state (`.bkit/`) 감지 연동.
- 피해야 할 것: bkit 의 엔지니어링 복잡도 (128 lib modules / Clean Architecture 4-Layer / 21 hook events) 이식, PDCA 용어를 tunaFlow 기본 워크플로우로 고정.

# 재프레임 — 수직 보완 관계

원 분석가는 bkit 과 tunaFlow 를 수평 비교표로 세팅하고 "겹치는 부분" 을 묻는 프레임을 사용. 실물은 그렇지 않다.

| 층위 | 위치 | 책임 |
|---|---|---|
| Claude Code / Codex / Gemini CLI | 엔진 | 실제 코드 실행 |
| **bkit** | **엔진 내부 plugin** | 각 CLI 를 PDCA 규율로 묶음 — 39 skills · 36 agents · 128 lib modules (~27K LOC) · 21 hook events 주입 |
| **tunaFlow** | **엔진 위 desktop orchestrator** | 여러 CLI 를 GUI 한 surface 에서 조율, ContextPack / Branch / RT / Memory |

즉 한 사용자가 "Claude CLI + bkit (CLI 내부 확장) + tunaFlow (멀티엔진 GUI)" 를 **모두 함께 쓸 수 있음**. 경쟁 관계가 아니라 층이 다른 이웃이다.

이 인식이 전제돼야 차용 판단도 바뀐다.

# bkit 실물 관찰

## 규모 (v2.1.10 기준)

- 39 Skills · 36 Agents · 128 Lib Modules · 21 Hook Events · 2 MCP Servers · 16 MCP Tools · 47 Scripts · 113 test files · 3,762 TC
- Clean Architecture 4-Layer (Domain / Application / Infrastructure / Presentation)
- Defense-in-Depth 4-Layer (CC runtime sandbox → PreToolUse hook → audit-logger → Token Ledger NDJSON)
- Invocation Contract L1~L5 (226 CI-gated assertions)
- CC 75 consecutive compatible releases 유지

## 세 레포 관계

- `bkit-claude-code` — 주력, 위의 모든 요소 포함
- `bkit-codex` — Codex 용 별도 이식. 간소. `AGENTS.md` + `packages/` 구조
- `bkit-gemini` — Gemini 용 별도 이식

즉 bkit 은 **"멀티엔진 통합" 이 아니라 "각 CLI 별 별도 플러그인"**. 멀티엔진 parity 관점에서는 tunaFlow 가 더 유리한 구조 (`build_normalized_prompt_with_budget()` 하나로 5엔진 커버).

## Context Engineering 철학

bkit README 는 자신을 **"Context Engineering 의 구현체"** 로 정의. 구체적으로는:

| Layer | Components |
|---|---|
| Domain Knowledge | 39 Skills |
| Behavioral Rules | 36 Agents |
| State Management | 128 Lib Modules (PDCA state machine 등) |

6-Layer Hook System 으로 각 레이어 간 context 주입.

tunaFlow 의 **ContextPack** 이 이미 동일한 문제를 풀지만, "Context Engineering" 이라는 용어로 설명된 적이 없다. 마케팅/문서적 가치만 있어도 차용할 만함.

# 차용 가치 평가

## 유용한 차용 요소

| bkit 요소 | tunaFlow 적용 | 적용 위치 | 우선순위 |
|---|---|---|---|
| Context Engineering 용어·도식 | ContextPack 을 "Context Engineering 구현체" 로 설명. README 개선 | README, docs/reference | P2 |
| Skill classification (Workflow/Capability/Hybrid + deprecation-risk + effort) | Skills snapshot frontmatter 에 동일 필드. 온보딩 메타에이전트 추천 활용 | `~/.tunaflow/skills/` 스키마 | P1 |
| Version invariant + docs-code-sync CI | DB 버전 / 테스트 수 / 엔진 수 등 문서 하드코딩 수치 자동 대조. I-1 작업 재발 방지 | `scripts/docs-check.mjs` 신규 | P1 |
| Defense-in-Depth 4-Layer | Issue #178 `--dangerously-skip-permissions` 보완. 외부 경로 write 시도 감지/차단 hook | `src-tauri/src/agents/` hook 레이어 | P2 |
| Level-based adaptation (Starter / Dynamic / Enterprise) | onboarding meta agent 가 프로젝트 규모 판정 → 자동 preset (ContextPack budget, RT 참여자 수 등) | `metaAgentOnboardingPlan` | P2 |
| **bkit state 인식 integration** | `.bkit/state/*.json`, `.bkit/runtime/token-ledger.json` 등을 ContextPack builder 가 자동 감지 → 섹션 주입. 두 도구를 **함께 쓰는 사용자에 대한 차별화 포인트** | `build_normalized_prompt_with_budget()` 소스 레이어 | P1 (차별화) |

## 이미 tunaFlow 가 강한 부분

| 영역 | bkit | tunaFlow | 판단 |
|---|---|---|---|
| 멀티엔진 parity | 각 CLI 용 별도 이식 (3 repo) | 단일 ContextPack 으로 5엔진 | tunaFlow 압도 |
| Branch / Adopt / Roundtable | 없음 (CLI session-bound) | 1급 시민 | tunaFlow 유일 |
| Review Loop + verdict marker | PDCA check phase (단일 agent) | RT 기반 병렬 Reviewer + verdict schema + rework queue | tunaFlow 더 정교 |
| Long-term memory | session memory 수준 | sqlite-vec + bge-m3 + cross-session link + compression | tunaFlow 광범위 |
| Graph RAG / 문서 인덱싱 | 제한적 | rawq + CRG + document graph | tunaFlow |
| Interactive PTY / sdk-url WS | CC plugin 이라 엔진 직접 호출만 | 세션 lifecycle 직접 관리 | tunaFlow |

# 피해야 할 것

1. **엔지니어링 복잡도 이식 금지** — 128 lib modules, 47 scripts, 3762 TC, 21 hook events, Clean Architecture 4-Layer, Guard Registry. 이 규모는 bkit 이 **"CC plugin 생태계 안에서 홀로 서기 위한 과잉 엔지니어링"**. tunaFlow 는 Rust + Tauri + 자체 DB 를 가진 데스크탑 앱이라 같은 복잡도 쌓을 이유 없다.
2. **PDCA 를 기본 워크플로우 DSL 로 강제 채택 금지** — tunaFlow 의 "Plan → Dev → Review → Done" 이 이미 명확. PDCA 는 외부 설명용 용어로만 병기 가능.
3. **agent / skill 이름 복사 금지** — `cto-lead`, `pdca-eval-plan`, `bkend-expert` 등. 식별력 저하.
4. **9-phase development-pipeline skill 강제 주입 금지** — bkit 도 optional 로 두고 있음. tunaFlow 가 굳이 구현할 필요 낮음.

# 실행 제안 (우선순위 순)

## P1 (근 1~2 스프린트)

1. **Skills frontmatter 확장** — `classification`, `effort`, `deprecation-risk` 필드 도입. 온보딩 메타에이전트가 분류 기반 추천.
2. **docs-code-sync CI 스크립트** — DB 버전 · 테스트 수 · 엔진 수 · skill 수 자동 대조. 2026-04-24 I-1 작업 (stale 수치 정리) 같은 drift 재발 방지.
3. **bkit state 인식 integration** — `.bkit/` 감지 시 ContextPack 섹션 자동 주입. 두 도구를 함께 쓰는 사용자에게 즉각적 시너지. tunaFlow 포지션 차별화.

## P2 (베타 피드백 이후)

4. **Context Engineering 도식 README 반영** — bkit 수준의 시각화. "3 Layer: engines + ContextPack + memory/retrieval" 도식.
5. **Level-based onboarding preset** — Starter / Dynamic / Enterprise. 각각 RT 참여자 기본값 / ContextPack budget / memory trigger threshold 다름.
6. **Defense-in-Depth hook 레이어** — `src-tauri/src/agents/` 에 PreToolUse 상응 훅. 프로젝트 밖 경로 쓰기 감지/차단.

## P3 (장기)

7. **bkit 과 상호 언급 README** — 두 프로젝트가 서로의 README 에 "호환 생태계" 섹션. 커뮤니티 신뢰 효과.

# 최종 추천

**"2.5"** — 원 분석가가 제시한 선택지 중 "2. 문서/템플릿 수준 차용" 과 "3. 일부 workflow 기능 흡수" 사이.

- 문서/템플릿 즉시 반영: Context Engineering 용어, Skill classification frontmatter, docs-code-sync CI
- 워크플로우 일부 흡수: level-based preset, defense-in-depth (베타 피드백 후)
- **설계 철학 이식 금지**: bkit 의 복잡도는 bkit 만의 전장 조건

**+ 추가 포인트**: 원 분석가가 놓친 축이 있다. **bkit 과의 연동 API** — tunaFlow 의 고유 포지션 ("멀티엔진 orchestrator") 이 잘 살려면 "CLI 생태계 확장팩들 (bkit, 향후 나올 다른 플러그인) 을 자동 인식" 이 차별화 포인트가 될 수 있다.

# 라이선스

bkit 세 레포 모두 **Apache License 2.0**. Copyright POPUP STUDIO PTE. LTD. (2024-2026, 싱가포르). tunaFlow 도 Apache 2.0 이라 라이선스 충돌 없음.

| 차용 형태 | 라이선스 의무 |
|---|---|
| 개념/패턴 (Context Engineering, PDCA, Skill classification 등 아이디어) | 없음 — 저작권 대상 아님 |
| 파일 단위 복사 (SKILL.md, hooks.json 템플릿 등) | Apache 2.0 §4 — LICENSE/NOTICE 포함 + 원저작자 attribution + 변경 사항 명시 |
| "bkit compatible" 배지/로고 사용 | 허가 필요 (상표 조항, NOTICE 에 명시) |
| "works with bkit" 같은 서술적 언급 | nominative fair use 로 대부분 OK, 로고는 피할 것 |

현재 §차용 가치 평가 의 P1~P3 항목은 **전부 개념 차용** 범주라 라이선스 의무 없음. 향후 파일 단위 복사가 필요하면 그때 NOTICE/attribution 챙기면 됨.

# 부록

## 분석가 프롬프트의 방법론적 메모

원 분석가 (ChatGPT) 프롬프트는 다음 특성을 보였다:

1. **프레임 선설정**: "표로 비교하기", "6 섹션 정해놓기", "산출물 형식 지정" — 응답의 자유도를 좁힘.
2. **본인 가설 노출**: "제 개인 가설은 2 또는 3" 을 프롬프트 내부에 명시 → 답변자가 그 방향으로 수렴하도록 유도.
3. **실물 분석 결여**: 세 레포를 직접 읽지 않고 README 요약 + 공식 키워드 (PDCA, Context Engineering) 수준에서 프레임을 빌드.

교훈: **외부 분석 요청 프롬프트를 받을 때 프레임부터 검증**. 실물 확인 후 프레임이 틀렸으면 먼저 재프레임.

## 참고 자료 경로

- `/Users/d9ng/privateProject/tunaFlow/_research/_util/bkit-claude-code/` (39 skills + 36 agents + 128 lib)
- `/Users/d9ng/privateProject/tunaFlow/_research/_util/bkit-codex/` (Codex 이식, 간소)
- `/Users/d9ng/privateProject/tunaFlow/_research/_util/bkit-gemini/` (Gemini 이식)

`_util/` 은 gitignore 됨. 본 문서 이후 불필요하면 제거해도 됨.

## 관련 이슈 / 기록

- Issue #178 (`--dangerously-skip-permissions`) — Defense-in-Depth 관점에서 후속 개선 가능
- 세션 s40 (2026-04-24 베타 공개 첫날 batch) — I-1 stale 수치 정리. docs-code-sync CI 가 있었으면 자동 탐지 가능했을 것.
- `docs/plans/metaAgentOnboardingPlan_2026-04-16.md` — Level-based preset 적용 시 이 plan 확장.
