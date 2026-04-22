# LiteRT-LM 통합 방향 검토

> Status: idea
> Created: 2026-04-22
> Trigger: Gemini의 "LiteRT-LM 기반 하네스/엔진 레이어 고도화 전략" 제안 검토 요청
> 참고: https://github.com/google-ai-edge/LiteRT-LM

---

## 1. 배경

Google `LiteRT-LM` (구 TFLite-LM) — WebGPU/Wasm 환경에서 LLM 온디바이스 추론에 최적화된 런타임. 주로 Gemma 계열과 소형 LLM 대상.

Gemini 제안 요지:
1. 5번째 로컬 엔진으로 추가 (Ollama/LMStudio 외)
2. Agent-as-Judge / ContextPack 전처리 이관
3. Regression Eval 자동화 + On-device Debugger

---

## 2. 결론 요약

- **데스크톱 앱의 5번째 엔진으로 추가는 권하지 않음** — Ollama/LMStudio와 기능 중복, 4-engine parity 유지 비용만 증가.
- **tunaflow-mobile (웹/모바일 클라이언트)** 에서는 fit — 여기가 진짜 용도.
- **보조 레이어 (분류/요약/reranking)** 로는 조건부 OK. **Judge 로는 권하지 않음** — 품질 리스크.

---

## 3. 영역별 평가

### 3.1 로컬 엔진 레이어 확장 — 데스크톱 X / 모바일 O

**데스크톱에서 권장 X**:
- 현재 `stream_run` 은 **CLI subprocess stdout 스트리밍** 가정. LiteRT-LM 은 in-process (WASM/native binding) → 완전 다른 어댑터 필요.
- 4-engine parity 의 `build_normalized_prompt_with_budget()` 경로도 엔진 수만큼 검증 대상 증가.
- CLI-first 방침 (`feedback_no_sdk`) 과 충돌. Ollama 가 이미 로컬 CPU/GPU 추론 + OpenAI-호환 API 제공.

**모바일(tunaflow-mobile, s30) 에서 권장 O**:
- Ollama 설치 불가 환경. 브라우저 WebGPU 로 offline 가능한 유일한 선택지.
- guest/offline 엔진으로만 한정 탐색.

### 3.2 하네스 강화 — 조건부

**a) Agent-as-Judge 이관 — 권장 X**
- 5차원 루브릭 judging 은 nuance 요구 → Gemma-2B/3B 급 모델로는 Claude/GPT-4o judge 와 calibration gap 발생.
- regression eval suite 의 판정 안정성 훼손 가능.
- 비용 절감 < judge flakiness 복구 비용.

**b) ContextPack 전처리 — 조건부 O**

다음 **작은 I/O + 명확한 정답** 작업만 이관 후보:

| 작업 | 오답 영향 | 권장도 |
|---|---|---|
| compression (긴 메시지 요약) | ContextPack 커짐, 실행 실패 아님 | O |
| retrieval reranking (top-k 재정렬) | 품질 소폭 저하, 회복 가능 | O |
| intent classification (handoff target) | fallback path 존재 | O |
| 5차원 rubric scoring | eval 판정 왜곡 | X |
| tool result verdict | workflow 방향 왜곡 | X |

원칙: 오답이 나도 **실행 실패가 아닌** 작업만.

### 3.3 Regression Eval / Self-healing

**a) Regression suite 판정자로 사용 — 권장 X**
- Phase 6 (PR #108) 에서 LLM-as-Judge (Claude/GPT-4o) 구조 완성. 여기서 judge 를 소형 모델로 교체하면 **평가 자체가 흔들림** → harness 품질 3→7 목표 달성 불가.
- LiteRT-LM 자리: **dev 루프 inner loop** — PR 직후 로컬 quick sanity check 용 (flaky 허용). CI gate 는 기존 judge 유지.

**b) Self-healing (On-device Debugger) — 유망하나 필수성 낮음**
- 이미 Failure Learning 테이블에 실패 패턴 축적 중.
- 초기 실패 코퍼스 부족 단계에서는 규칙 기반 (파일/함수 매칭) 이 더 정확.
- 코퍼스 축적 후 재검토.

---

## 4. 권장 action

1. **데스크톱 엔진 통합 플랜은 보류**. 인지 부담 > 실익.
2. **tunaflow-mobile 에서 WebGPU offline 엔진 PoC** 로 scope 한정.
3. **ContextPack 보조 레이어 (compression/reranker) 단일 지점부터 A/B** — 품질 회귀 없으면 확대.
4. **Judge / regression eval 에는 도입 X**. Phase 6 구조 유지.

---

## 5. 검증 필요 항목

- LiteRT-LM Rust FFI 성숙도 (현재 ABI stable 인지)
- Gemma-2B/3B quantized 모델의 5차원 루브릭 정확도 측정 데이터
- 모바일 브라우저 WebGPU 지원 범위 (iOS Safari 18+ 필요)
- Gemma 라이선스가 tunaFlow 배포 조건과 양립 가능한지 (Gemma Terms)

---

## 6. 관련 문서

- `docs/ideas/onboardingMetaAgentIdea.md`
- `docs/ideas/mobileArchitectureIdea.md`
- `docs/archive/plans/completed/naturalLanguageHandoffPlan.md`
- Failure Learning (세션 14, project_session_2026-04-06_s14)
- 세션 30 (tunaflow-mobile scaffold)
