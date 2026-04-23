# Prompt Regression Eval Suite (Phase 6)

> 프롬프트 / 모델 / ContextPack 변경이 과거 성공 케이스의 품질을 **무너뜨리지 않는지** 자동으로 감시하는 회귀 테스트.

## 설계 요약

1. **Golden Dataset** (`golden/*.jsonl`) — 과거 성공한 input/output 쌍 + 필수 행동 리스트. 사람이 한 번 작성, 이후 자동 실행
2. **LLM-as-a-Judge** (`judge/prompts/`) — 새 응답과 reference 를 세만틱 동등성 기준으로 채점 (값싼 Haiku 모델 사용)
3. **CI 게이트** (`.github/workflows/eval-regression.yml`) — 프롬프트/엔진 파일 변경 또는 `eval:run` 라벨 시 자동 실행

상세 설계: `docs/plans/promptRegressionEvalPlan.md` (예정)

## 디렉터리

```
evals/
├─ golden/                # 카테고리별 JSONL (5 카테고리 × 4 = 20개)
│  ├─ plan-generation-*.jsonl
│  ├─ dev-implementation-*.jsonl
│  ├─ review-verdict-*.jsonl
│  ├─ rt-verdict-*.jsonl
│  └─ branch-adopt-*.jsonl
├─ judge/
│  └─ prompts/semantic-equivalence.md
├─ scripts/
│  ├─ extract-from-trace.mjs    # DB 에서 seed 추출
│  ├─ run-eval.mjs              # 전체 runner
│  └─ report.mjs                # 결과 diff → markdown
└─ results/                     # 회차별 JSON 스냅샷
```

## 실행

```bash
# seed (한 번만)
node evals/scripts/extract-from-trace.mjs --limit 50 > evals/golden/candidates.jsonl

# 수동 실행
export TUNAFLOW_TOKEN=<Settings > Mobile>
export EVAL_ENGINE=claude
export JUDGE_ENGINE=claude
export JUDGE_MODEL=claude-haiku-4-5-20251001
node evals/scripts/run-eval.mjs

# 결과 리포트
node evals/scripts/report.mjs
```

## 게이트 정책

| Pass율 | 처리 |
|--------|------|
| ≥ 85% | 그린 |
| 70-85% | warning comment, 머지 가능 |
| 50-70% | failing check, 리뷰어 override |
| < 50% | block |

초기 2~4주는 전부 warning 운영 → 오탐률 측정 → threshold 조정.

## JSONL 스키마

각 항목 1줄:

```json
{
  "id": "plan-generation-01",
  "category": "plan-generation",
  "prompt": "사용자 요청 원문",
  "context_hint": "프로젝트 스택/상태",
  "engine_ref": "claude",
  "model_ref": "claude-sonnet-4-6",
  "reference_output": "<과거 성공 응답>",
  "expected_behaviors": [
    "must-cover 행동 1",
    "must-cover 행동 2"
  ],
  "rubric_threshold": 0.7,
  "created_at": 1713567890000,
  "source_trace_id": "trace-abc123"
}
```

## Running the eval

```bash
# 전체 golden 셋 실행 (기본: 결과 확인 후 scratch 프로젝트 자동 정리)
node evals/scripts/run-eval.mjs

# 디버깅 — 실패 run 을 보관한 채 진행 (opt-out)
node evals/scripts/run-eval.mjs --no-cleanup
# 또는:
EVAL_CLEANUP=0 node evals/scripts/run-eval.mjs
```

기본 cleanup 은 **ON**. `[eval] <label>` 스크래치 프로젝트가 `projects` 테이블에 누적되지 않도록 baseline. 특정 실패 run 의 artifacts 를 사후 검증하려면 `--no-cleanup` / `EVAL_CLEANUP=0` 으로 opt-out.

## Golden seed 빌드 (secall + 타 프로젝트 기반)

실제 사용자 DB 에서 카테고리별 4개 × 5 = 20개 시나리오를 추출해 JSONL 생성:

```bash
node evals/scripts/build-golden-seed.mjs
```

카테고리별 쿼리:
- `plan-generation` — Architect 페르소나 + `<!-- tunaflow:plan-proposal -->` 마커 포함
- `dev-implementation` — Implementer/Coder 페르소나 + `🔧 구현 시작` 프롬프트
- `review-verdict` — Reviewer 페르소나 + 긴 verdict 응답
- `rt-verdict` — RT branch 의 마지막 assistant 메시지 (고유 conversation)
- `branch-adopt` — `status='adopted'` 브랜치의 마지막 assistant 메시지

`expected_behaviors` 는 초기에 카테고리 공통 rubric 5개. 실제 eval 돌려보며 per-entry 로 튜닝.
