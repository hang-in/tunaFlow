---
title: Prompt Regression Eval Plan (Phase 6)
status: active
canonical: true
created_at: 2026-04-21
owner: architect
related:
  - docs/reference/harnessMaturityAudit_2026-04-16.md  # §2.2 Evaluation harness (3/10 → target 7/10)
  - evals/README.md
  - docs/reference/aistartkit-harness-evaluation_2026-04-21.md
---

# Phase 6 — Prompt Regression Eval Suite

> 프롬프트 / 모델 / ContextPack 변경이 과거 성공 케이스의 품질을 무너뜨리지 않는지
> 자동으로 감시하는 회귀 테스트.

## 목적

Harness Maturity Audit §2.2 에서 "Regression Evaluation Harness 3/10 (하)" 로 판정된 영역을 MVP 수준으로 끌어올린다. 루브릭만 있고 회귀 측정 수단이 없었던 상태 → 20개 정도의 golden 기준점 + LLM-as-a-Judge + CI 게이트.

## 설계 요약

### 3단계 구조

1. **Golden Dataset** (`evals/golden/*.jsonl`)
   - 과거 성공한 (prompt, output) 쌍 + `expected_behaviors`
   - 5 카테고리: plan-generation / dev-implementation / review-verdict / rt-verdict / branch-adopt
   - `extract-from-trace.mjs` 로 DB 에서 seed 후 사람이 `expected_behaviors` 수작업

2. **LLM-as-a-Judge**
   - Judge 프롬프트: `evals/judge/prompts/semantic-equivalence.md`
   - Judge 모델: Haiku (기본 `claude-haiku-4-5-20251001`) — Sonnet 대비 10배 저렴
   - 출력: 0.0-1.0 score + verdict + covered/missing/regressions/improvements

3. **CI 게이트** (`.github/workflows/eval-regression.yml`)
   - Trigger: workflow_dispatch / 관련 경로 PR / `eval:run` label
   - Warning-only 정책으로 2-4주 운영 → threshold 튜닝 후 hard gate 로 전환

### 파일 구조

```
evals/
├─ README.md
├─ golden/                    # 카테고리별 JSONL
├─ judge/prompts/semantic-equivalence.md
├─ scripts/
│  ├─ extract-from-trace.mjs  # DB → JSONL seed (6-1)
│  ├─ run-eval.mjs            # 전체 runner (6-3)
│  └─ report.mjs              # markdown diff (6-4)
└─ results/                   # 회차별 JSON 스냅샷
```

## 산출물

### 6-1 extract-from-trace.mjs ✅

`~/.tunaflow/db/tunaflow.db` 의 `messages` 테이블에서 persona 기준으로 성공 케이스 추출. 4가지 persona 카테고리 + branches.adopted 조인으로 branch-adopt 지원. user_prompt 페어링 실패 시 직전 assistant 메시지로 fallback (auto-invoked Reviewer/Synthesizer 커버).

실측 결과 (2026-04-21):
- plan-generation: 5
- dev-implementation: 5
- review-verdict: 3
- rt-verdict: 2 (DB 에 실 데이터 2개)
- branch-adopt: 0 (adopted_message_id 있는 엔트리 없음)

### 6-2 Golden Dataset Seed (WIP — 사람 작업)

수작업으로 진행:
1. `npm run eval:extract 10 > evals/golden/candidates.jsonl`
2. JSONL 을 카테고리별 파일로 분리 (`plan-generation-01.jsonl` …)
3. 각 항목에 `expected_behaviors` 3-5줄 수작업 (must-cover 행동)
4. `context_hint` 프로젝트 스택/상태 간단 1-2줄
5. 1항목당 5-10분 × 15-20개 = 2-3시간 작업

LLM 자동 생성 금지 — baseline 을 LLM 이 정의하면 regression 측정이 원리적으로 불가.

### 6-3 run-eval.mjs ✅

각 golden 항목에 대해:
1. 스크래치 project + conversation 생성
2. `POST /api/v1/conversations/{id}/send` + WS 구독
3. `agent:completed` 대기 (기본 180s 타임아웃)
4. 최신 assistant 메시지 fetch
5. Judge 에 system prompt + (CATEGORY/PROMPT/REFERENCE/CANDIDATE/EXPECTED_BEHAVIORS) 전달
6. Judge JSON 결과 파싱 → 결과 누적
7. 스냅샷 JSON 저장: `evals/results/<ISO>-<engine>-<model>.json`

환경 변수:
- `TUNAFLOW_BASE` / `TUNAFLOW_TOKEN`
- `EVAL_ENGINE` / `EVAL_MODEL`
- `JUDGE_ENGINE` / `JUDGE_MODEL`
- `EVAL_FILTER` (정규식; id 또는 category 매치)
- `EVAL_TIMEOUT_MS` (기본 180000)

### 6-4 report.mjs + CI workflow ✅

`report.mjs` — 최신 스냅샷 읽어 markdown 출력. 4단계 게이트 라벨:

| Pass Rate | 레이블 |
|-----------|--------|
| ≥85% | 🟢 PASS |
| 70-84% | 🟡 WARNING |
| 50-69% | 🟠 FAILING |
| <50% | 🔴 BLOCK |

카테고리별 브레이크다운 + per-item 스코어 테이블 포함. `--baseline` 모드에서 회차간 diff 제공.

`.github/workflows/eval-regression.yml` — 선택적 트리거:
- `workflow_dispatch` (항상)
- 프롬프트/ContextPack/eval 경로 변경 PR
- `eval:run` 라벨 수동

Warning-only 운영 (2-4주) → 데이터 축적 후 hard gate.

## 비용 추산

- Golden 20개 × (생성 Sonnet + Judge Haiku) = 40 LLM 호출
- 토큰: 생성 ~2k × 20 + Judge ~3k × 20 ≈ 100k 토큰
- 금액: $0.3-0.5 / run
- 실행: 주 1회 수동 + 관련 PR 시 → 월 $5-10 미만

## 운영 정책

### 초기 2-4주 (Calibration)

- 모든 결과는 warning-only (merge 블록 하지 않음)
- 매 실행 후 `missing` / `regressions` 수동 검토 → expected_behaviors 보정
- 오탐률 > 20% 면 threshold 0.7 → 0.65 조정 고려

### 이후 (Enforcement)

- Pass rate <50% 면 merge block (exit 1)
- 50-69% 는 failing check, 리뷰어 override 가능
- `expected_behaviors` 변경은 별도 PR 로 분리 (baseline 조작 방지)

## 확장 (v2)

- **Regression 추세 시각화**: `TracePanel` 에 eval 탭 추가, 최근 10회 pass율 sparkline
- **Failure bucket 자동 분류**: Judge 의 `regressions[]` 토픽 모델링 → 회귀 축 알림
- **Auto-seed 후보 큐**: plan done + review pass 케이스를 golden 후보 자동 큐잉, 사람 검수 후 승격

## 다음 행동

1. `npm run eval:extract 10 > /tmp/candidates.jsonl` 실행
2. JSONL 을 사람이 카테고리별 파일로 분리 + `expected_behaviors` 수작업 (2-3시간)
3. 첫 실행: `npm run eval:regression`
4. 결과 확인 후 threshold / prompt 조정
5. 3-5회 실행으로 기준점 확립 → CI workflow hard gate 전환
