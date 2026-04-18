---
title: Retrieval 골든셋 회귀 평가
status: active
created_at: 2026-04-18
---

# Retrieval Golden-Set Evaluation

ContextPack 의 **retrieval 품질**을 정량적으로 측정하는 회귀 스위트. Conventions Sync Phase 2, type filter 같은 retrieval 파이프라인 변경의 **before/after** 를 수치로 비교하기 위해 만들었다.

## 언제 돌리는가

- ContextPack / retrieval 로직 변경 PR 의 마지막 확인
- 베타 공개 직전의 기준선(baseline) 기록
- 신규 모델/엔진 추가 후 파라미터 튜닝 직후

`/src/components/tunaflow/context-panel/QualityDashboard.tsx` 는 **실시간 관측**, 이 스위트는 **결정적 회귀**. 용도 다르므로 같이 보지 않는다.

## 데이터셋 만들기

1. `docs/eval/golden-set.json` 을 본인 환경에 맞게 채운다.
2. 실제 대화에서 다음 패턴들로 최소 20~30 개 엔트리 큐레이션:
   - **과거 대화 레퍼런스** — "이전에 refactoring 얘기할 때 뭐 결정했지?" → 실제 결정 메시지 ID
   - **파일 언급** — "src/foo.ts 에서 뭐 수정했지?" → 해당 파일 논의 메시지 ID
   - **cross-session 주제** — "다른 프로젝트에서 비슷한 거 어떻게 했지?" → 유사 프로젝트 메시지 ID
   - **현재 작업 상태** — "지금 어디까지 했지?" → 최근 plan/implementation 메시지 ID
3. `expected_message_ids` 는 사람이 "이 대답을 생성하려면 적어도 이 메시지들이 context 에 있어야 한다" 고 판단한 **message ID**.

### 엔트리 스키마

```json
{
  "id": "q1",
  "question": "사용자 질문",
  "context": { "conversation_id": "실제 conversation ID" },
  "expected_message_ids": ["실제 message ID 1", "..."],
  "notes": "선택 — 사람 메모"
}
```

## 실행

```bash
cd src-tauri

cargo run --release --bin eval_retrieval -- \
    --db ~/.tunaflow/db/tunaflow.db \
    --set ../docs/eval/golden-set.json \
    --k 5
```

### 옵션
- `--db <path>` — DB 파일 경로 (read-only 로 open)
- `--set <path>` — 골든셋 JSON
- `--k <n>` — top-K 검색 (기본 5)
- `--project-key <key>` — project_key 명시 (생략 시 첫 엔트리의 conversation 으로 추론)

### 출력
`id`, `retrieved`, `expected`, `hits`, `recall`, `precision` TSV + 마지막에 aggregate:

```
recall_at_k=0.820
precision_at_k=0.640
total_hits=41
total_expected=50
total_retrieved=64
```

## Before/after 비교 워크플로우

1. 변경 전 main 에서 실행 → `baseline.txt` 저장
2. 변경 적용 → 다시 실행 → `candidate.txt`
3. `recall_at_k` 가 **유의미하게 떨어졌으면** (대략 -5% 이상) 변경을 재검토

## 한계 (정직)

- `expected_message_ids` 는 사람이 판단한 "해당 답변에 필요할 것" 의 proxy.
  실제 모델이 그 메시지 없이도 답을 잘 만드는 경우가 많다 → recall 숫자가
  절대적 품질 지표는 아님
- `RetrievedChunk` 이 message id 를 직접 담지 않아 현재는 `(conversation_id, timestamp)`
  JOIN 으로 역추적. timestamp 충돌이 드물긴 하지만 이론적으로 ambiguous 할 수 있음
- 골든셋이 **사용자 환경의 DB** 에 종속 — 팀 공유하려면 sanitized fixture DB 를
  별도로 만들어야 함 (follow-up)

## 후속 개선

- [ ] `RetrievedChunk` 에 `root_message_id` 필드 노출 (지금은 workaround 로 역추적)
- [ ] Project scope — 현재는 특정 conversation 기반. 프로젝트 전체 기준
      sampling 모드 추가
- [ ] LLM-as-judge 경로 — retrieval 이 답변에 실제로 기여했는지 2차 검증
- [ ] Fixture DB — 팀/CI 공유 가능한 고정 데이터셋
