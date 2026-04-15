---
title: Known Issues — 2026-04-15 세션 (베타 공개 전 체크리스트)
status: active
canonical: true
created_at: 2026-04-15
updated_at: 2026-04-15
owner: architect
related:
  - docs/reference/knownIssues_2026-04-12.md
  - docs/reference/indexingPipelineBug_P0_2026-04-15.md
  - docs/reference/retrievalContextPollutionFix_2026-04-15.md
  - docs/reference/geminiCriticReview_2026-04-15.md
  - docs/plans/betaReleaseReadinessPlan.md
---

# Known Issues — 2026-04-15 세션

이번 세션(persona fix / PTY 격리 / retrieval 오염 / 외부 LLM 리뷰) 진행 중 발견했거나 재확인된 이슈 모음. **베타 공개 전 처리 우선순위**로 정리.

---

## 🔴 P0 — 베타 공개 차단

### I1. 인덱싱 파이프라인 3중 버그 (v32 이후 embedding 복구 실패)

- **문서**: `docs/reference/indexingPipelineBug_P0_2026-04-15.md` (상세)
- **증상**: Vector 검색 사실상 기능 정지 — gemento 프로젝트 327 chunks 중 12개만 embedding, 문서 chunk 6877개 전부 NULL
- **원인**: v32 migration이 embedding을 NULL로 초기화만 하고, `already_indexed` 쿼리에 `AND embedding IS NOT NULL` 누락 → 영구 skip + document 자동 재인덱싱 트리거 부재
- **Fix**: 1+2 치명, 3+5 보조 (5개 후보 모두 문서 정리)
- **진행 상태**: secall PR 처리 후 브랜치 `fix/indexing-pipeline-recovery` 착수 예정

### I2. README "2-agent 교차 검증" 문구와 코드 불일치

- **문서**: `docs/reference/geminiCriticReview_2026-04-15.md` §5
- **증상**: `reviewWorkflow.ts:58`에 `"빌드/테스트 명령을 직접 실행하지 마세요"` 명시. 즉 Reviewer는 Developer의 텍스트 보고만 읽음. README의 "self-validation 한계 극복" 주장과 불일치
- **Fix 옵션**:
  - (A) README 문구만 수정 — "Reviewer=정적 리뷰, 테스트는 Developer 책임" 명확화 (10분)
  - (B) Tester 에이전트 별도 단계 추가 — RT 고도화 이후
  - (C) 랜덤 재검증 spot-check
- **베타 전 최소**: (A). (B)/(C)는 후속

---

## 🟡 P1 — 베타 전 권장 (공개 가능, 하지만 UX 찐빠)

### I3. 스트리밍 중 HTML 주석 마커 UX 플래시 🆕 (이번 세션 발견)

- **증상**: 에이전트 응답 스트리밍 중 `<!-- Plan ... -->` 같은 마커가 **텍스트로 잠깐 노출**됐다가 닫는 토큰 도착 후 사라짐
- **원인 추정** (미확인, 재현 후 확증 필요):
  - 스트리밍 chunk 단위로 토큰이 점진적으로 도착: `<!--` → `<!-- Plan` → ... → `<!-- Plan ... -->`
  - markdown 렌더러는 닫는 `-->` 도착 전까지 "HTML comment 아님"으로 판단 → 일반 텍스트로 표시
  - `-->` 도착 후 비로소 주석 인식 → 숨김
- **영향**: 기능 실패 아님. UI 플래시만. 그러나 **데모/스크린샷에서 눈에 띄면 인상 악화**
- **Fix 후보**:
  - (a) frontend streaming preview 단계에서 `<!--[^>]*$` 패턴(미완성 comment) 정규식으로 감지해 숨김 — 1~2시간 작업
  - (b) 마커를 zero-width 또는 code-fenced 패턴으로 변경 — 파이프라인 전체 변경, 중~고
- **추천**: (a). 렌더 직전 전처리로 해결
- **우선순위**: 베타 전 P1. plan 1개로 분리 가능
- **상태**: 이번 세션에서 실측 로그로 확인됨. 재현 조건 = 에이전트가 marker 포함한 응답 스트리밍

### I4. 마커 파싱 실패 시 사용자 경고 UI 부재

- **문서**: `docs/reference/geminiCriticReview_2026-04-15.md` §1
- **증상**: `extractReviewVerdict` / `extractImplPlan` 등이 파싱 실패해도 silent fallback. 사용자는 왜 워크플로우가 멈췄는지 모름
- **Fix**: 파싱 실패 시 toast + "verdict 판독 실패, 수동 확인 필요" 안내. 2~3시간
- **상태**: 이번 세션에서 실제 재현 없음. Gemini 리뷰에서 이론적으로 지적된 방어 개선

### I5. 위험 CLI 플래그 활성화 시 경고 배너 없음

- **문서**: `docs/reference/geminiCriticReview_2026-04-15.md` §4
- **증상**: `--full-auto` / `--dangerously-skip-permissions` 같은 CLI approval 우회 플래그 켜진 상태를 UI가 인지하지 못함
- **Fix**: PTY 세션 args 파싱 후 위험 플래그 감지 → 상단 경고 배너. 3~4시간
- **상태**: 베타 공개 안전성 관점. 선택적

---

## 🟢 P2 — 베타 이후 (영향 작음)

### I6. Retrieval 로그 preview 오도

- **원인 파일**: `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs:300`
- **증상**: FTS5가 매칭한 실제 메시지와 preview에 표시되는 메시지가 다를 수 있음 (`build_pair_chunk`이 user+assistant 쌍으로 확장하면서 preview는 user만 보여주는 경우). 사용자는 "뜬금없는 맥락이 retrieval에 들어왔다"고 오해
- **영향**: 기능은 정상 (ContextPack에는 전체 chunk 포함). 로그 가독성만
- **Fix**: hit_id의 content snippet을 preview로 사용하거나, 매칭된 단어 주변 ± N자 발췌. 1시간

### I7. Scratchpad 주제 혼재로 retrieval noise

- **증상**: 하나의 scratchpad 대화방에 여러 주제(포스트/codex/context-hub 등)가 섞여있으면, 한 주제 쿼리에 다른 주제 pair가 함께 retrieval됨. pair 단위 확장의 태생적 한계
- **Fix 후보**:
  - scratchpad 소속 pair에 `kind_bonus -= 0.1`
  - chunk 단위를 pair → 단일 메시지로 (맥락 손실 트레이드오프)
  - 주제 세그먼테이션(복잡)
- **상태**: Vector 복구(I1) 후 자연 보완 가능성 높음. 관찰 후 결정

### I8. `index_chunks_blocking` embed 실패 silent drop

- **원인 파일**: `src-tauri/src/commands/vector_search/index.rs:322-327`
- **증상**: bge-m3 embed 실패 시 로그조차 없이 drop. `index_conversation_chunks` (async 버전)엔 `eprintln!("[vector] embed failed ...")` 있음
- **Fix**: async 버전과 parity 맞추기. 1줄 추가
- **처리**: I1 Fix 3에 포함 예정

---

## 이번 세션에서 이미 해결된 것

참고용 정리.

| 커밋 | 해결 |
|------|------|
| `6cc607a` | personaLabel stale 회귀 (plan-completed auto-notify 경로) |
| `0e65295` | PTY send 경로에서 분리 — Enter-hang 증상 제거 |
| `1a98149` | Retrieval recency/self-exclusion/coverage ABCD |
| `adf8408` | 인덱싱 P0 버그 조사 (수정은 후속) |
| `c195599` | unused_mut 경고 제거 |

---

## 내 의견 (베타 공개 가능성)

**현재 상태**: 인덱싱 I1만 해결되면 **베타 가능** 수준.

**이유**:
- I1은 Vector 기능 정지 수준의 구조적 결함. 이걸 남겨두면 "ContextPack + 장기 메모리" 핵심 기능이 반쯤 죽은 채 공개하는 것 → 기만 리스크
- I2는 10분 작업. I1 작업 마무리할 때 같이 처리 가능
- I3~I5는 UX/방어 개선. 베타 1~2차에서 피드백 받으며 처리 가능
- I6~I8은 의미 없는 지연 요소

**권장 순서**:
1. secall PR 처리 (사용자 현재 작업)
2. `fix/indexing-pipeline-recovery` 브랜치 — I1 Fix 1+2 (P0 2개) + I8 (parity)
3. `chore/readme-reviewer-wording` 브랜치 — I2 (A안 문구만) + Gemini 리뷰 문서 업데이트
4. 베타 배포 (`v0.1.0-beta.1`)
5. 베타 피드백 + I3/I4/I5 플랜 작성

**I3(마커 UX 플래시)는 플랜으로 뺍니다** — 사용자가 말한 대로. 베타 공개 후 빠르게 수정 가능.

**I6/I7**은 I1 수정 후 실측하고 재평가. Vector가 살아나면 대부분 자연 해결될 가능성이 있음.

### 베타 차단 여부 최종 판단

| 이슈 | 차단? |
|------|-------|
| I1 인덱싱 P0 | **차단** — 해결 필수 |
| I2 README 문구 | **차단** — 기만 리스크, 하지만 10분 작업 |
| I3 마커 UX 플래시 | 차단 아님 — 플랜으로 관리 |
| I4 파싱 경고 UI | 차단 아님 |
| I5 위험 플래그 배너 | 차단 아님 |
| I6~I8 | 차단 아님 |

즉 **I1 + I2만 처리하면 베타 공개 가능** 판단.
