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
