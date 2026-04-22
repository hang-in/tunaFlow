# tunaFlow Context Budget Scaling 베타 실험 준비

적용 스킬:
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build\react-best-practices`
  - 이유: 현재 ContextPack guardrail은 보수적으로 작게 잡혀 있어, 장기적으로 엔진별/모드별 budget을 더 유연하게 다룰 준비가 필요함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build\composition-patterns`
  - 이유: total prompt limit, section cap, context mode, traceability를 한 번에 보지 않으면 budget 상향 실험이 엉킬 수 있음
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build\frontend-design`
  - 이유: 목표는 단순히 많이 넣는 것이 아니라, 실제 답변 품질과 체감 UX가 좋아지는지 검증 가능한 구조를 만드는 것임

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\contextBudgetScalingPlan.md`
- `D:\privateProject\tunaFlow\docs\plans\contextPackTraceabilityPlan.md`
- `D:\privateProject\tunaFlow\src-tauri\src\guardrail.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents_helpers\context_pack.rs`

현재 상태:
- `MAX_TOTAL_PROMPT = 60_000 chars`
- 프로젝트 내부 추정으로는 약 `15k tokens` 전후
- 이는 모델 최대 한계가 아니라 앱 내부의 보수적 guardrail임
- 1M 컨텍스트 시대 기준으로는 충분히 작을 수 있음
- 하지만 지금 당장 전면 확대보다, 베타에서 단계적으로 올리며 측정하는 것이 안전함

이번 작업 목표는:
**ContextPack budget을 나중에 베타에서 단계적으로 올릴 수 있도록, 현재 guardrail 구조와 traceability/측정 지점을 정리하고 실험 준비 상태를 만드는 것**이다.

중요:
- 실제 코드 기준으로만 작업
- 이번 단계는 “실험 준비”가 목표지, 전면 상향 적용이 목표가 아님
- 가능하면 feature flag 또는 실험적 설정 경로를 우선 고려
- 모든 응답과 보고는 한국어로만 작성하라

---

## 목표

최소한 아래를 만족하라.

1. 현재 total/section budget 구조를 코드 기준으로 명확히 정리
2. 이후 베타에서 `60k → 120k` 같은 단계적 상향 실험을 할 수 있는 준비 상태를 만듦
3. 엔진별/모드별 budget 분리 가능성도 고려
4. budget 상향 시 어떤 데이터가 잘리고, 어떤 품질 변화가 있는지 측정 가능한 방향을 제시

---

## 먼저 확인할 파일

- `D:\privateProject\tunaFlow\src-tauri\src\guardrail.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents_helpers\context_pack.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents_helpers\trace_log.rs`
- 필요 시 trace 관련 UI 파일

---

## 구현 요구사항

### 1. 현재 예산 구조 재확인
다음을 실제 코드 기준으로 다시 확인하라.

- total prompt cap
- section별 cap
- Lite / Standard / Full 차이
- truncate가 어느 순서로 일어나는지

### 2. 베타 상향 준비
이번 단계에서 가능한 범위에서 아래를 준비하라.

- 상향 실험 시 손댈 상수 위치 명확화
- 가능하면 feature flag / config 기반 전환 지점 마련
- 즉시 적용이 아니라 “나중에 실험하기 쉬운 구조”가 목표

### 3. traceability 연계 검토
budget 상향 전에 아래를 알 수 있어야 한다.

- 실제 prompt length
- sections included
- truncation 여부 또는 어느 섹션이 잘렸는지

이번 단계에서 전부 구현 못 해도,
최소한 연결 설계를 분명히 하라.

### 4. 엔진별/모드별 분리 가능성 검토
현재 공통 상한 하나로 운영 중이라면,
나중에 아래 분리를 할 수 있을지 검토하라.

- engine-aware budget
- mode-aware budget

중요:
- 이번 단계에서 전면 구현은 금지
- 구조적으로 가능한지 정리하는 수준이면 충분

### 5. 범위 제한
이번 단계에서는 하지 말 것:
- 즉시 전 사용자 대상 budget 상향
- 1M 수준까지 한 번에 확대
- docs 작업 같이 하기

---

## 검증

작업 후 반드시 아래를 설명하라.

1. 현재 ContextPack budget 구조가 실제 코드에서 어떻게 되어 있는지
2. 왜 바로 크게 올리지 않고 베타 상향 실험이 맞는지
3. 나중에 어떤 방식으로 단계적 상향을 테스트하면 되는지
4. 엔진별/모드별 분리 가능성이 어떤지
5. traceability와 어떻게 연결되는지
6. 타입체크/빌드가 필요한 변경인지 여부
7. 남은 리스크

---

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. Context Budget Flow
### D. Verification
### E. Remaining Risks

바로 코드 확인과 필요한 최소 준비 작업까지 진행하라.
