# RT 알고리즘 강화 아이디어

> Status: idea
> Created: 2026-04-01
> 출처: 6개 논문/프레임워크 분석 + AutoBe/Typia harness 패턴 교차 검토

---

## 1. 배경: tunaFlow RT 현재 구조

```
Sequential:  A → B → C  (각자 이전 응답 가시)
Deliberative: A | B | C  (병렬, 이전 라운드만 가시)
Blind mode:  topic만 전달, 다른 응답 비가시
Role caps:   proposer=1200, reviewer=900, verifier=800, synthesizer=1500 토큰
```

현재 RT는 **1회 라운드 실행 → 결과 표시 → 사용자 판단** 구조. 자동 수렴, 투표, 피드백 루프 없음.

---

## 2. 검토한 알고리즘/논문

### 2.1 Mixture-of-Agents (MoA) — Together AI, 2024

> 논문: [Mixture-of-Agents Enhances Large Language Model Capabilities](https://arxiv.org/abs/2406.04692)
> 코드: [github.com/togethercomputer/MoA](https://github.com/togethercomputer/MoA)

**핵심 아이디어**: 다층 에이전트 구조. Layer 1의 여러 에이전트 출력이 Layer 2 에이전트의 입력이 됨.

```
Layer 1: [Agent-A, Agent-B, Agent-C]  → 각각 독립 응답 생성 (Proposer)
Layer 2: [Aggregator]                  → Layer 1 출력 전부 받아 통합 (Synthesizer)
```

**핵심 발견 — Collaborativeness**: LLM은 다른 모델의 출력을 참조하면 **자기 단독보다 더 좋은 응답**을 생성한다. 약한 모델의 출력도 강한 모델의 성능을 향상시킴.

**성과**: AlpacaEval 2.0에서 65.1% (GPT-4 Omni 57.5% 대비 7.6%p 상승). 오픈소스 모델만으로 달성.

**tunaFlow 적용 포인트**:
- RT의 Sequential 모드가 이미 유사 (B가 A의 출력을 참조)
- **부족한 것**: Layer 2 Aggregator 역할이 없음. 현재는 사용자가 수동으로 결과를 종합
- **추가할 것**: 자동 Synthesizer 라운드 — 모든 참가자 응답을 받아 최종 통합 응답 생성

---

### 2.2 Multi-Agent Debate (MAD) — 비판적 분석

> 원문: [Improving Factuality and Reasoning through Multiagent Debate](https://arxiv.org/abs/2305.14325) (ICML 2024)
> 비판: [ICLR 2025 Blog — MAD Performance & Scaling](https://d2jud02ci9yv69.cloudfront.net/2025-04-28-mad-159/blog/mad/)

**핵심 아이디어**: 여러 에이전트가 서로의 답을 비판하며 수렴.

**ICLR 2025 비판적 발견**:
- MAD가 **Self-Consistency(다수결) 대비 일관되게 열등**
- 토큰 소비 대비 성능이 비효율적
- **치명적 문제**: 맞는 답을 틀린 답으로 바꾸는 비율이 높음 (과도한 수정)
- Multi-Persona 방식이 **가장 나쁜 성능** (제한된 토론이 효과 감소)

**핵심 교훈 — tunaFlow RT에 중요한 시사점**:

| MAD 문제 | tunaFlow 시사점 |
|---------|----------------|
| 전체 응답 단위로 토론 → 세부 오류 감지 실패 | **reasoning step 단위로 토론**해야 효과적 |
| 사전 학습된 답 선호도가 객관성 방해 | **blind mode**가 이 문제를 정확히 해결 |
| 다수결이 토론보다 효과적 | **투표 메커니즘 도입 가치 확인** |
| 이종 모델 조합이 가장 유망 | tunaFlow의 **4-engine 혼합**이 올바른 방향 |

---

### 2.3 Self-Consistency + 투표 알고리즘

> 논문: [Self-Consistency Improves Chain of Thought Reasoning](https://arxiv.org/abs/2203.11171) (Wang et al., 2023)
> 후속: [Ranked Voting based Self-Consistency](https://arxiv.org/abs/2505.10772) (2025)
> 후속: [Confidence Improves Self-Consistency](https://arxiv.org/pdf/2502.06233) (2025)

**핵심 아이디어**: 같은 질문에 N번 샘플링 → **다수결로 최종 답 선택**. 단순하지만 MAD보다 효과적.

**발전형**:
- **Ranked Voting**: 단순 다수결 대신 Borda count, Instant-runoff 등 투표 알고리즘 적용 → 정확도 향상
- **Confidence-weighted**: 각 답에 모델의 신뢰도 점수를 가중치로 적용 → 6.8배 적은 샘플로 동일 성능
- **Self-Verification**: 답 생성 후 같은 모델이 검증 단계 수행 → 정확도 추가 향상

**tunaFlow 적용 포인트**:
- RT 참가자 verdict를 **투표로 집계** (현재는 사용자가 수동 판단)
- Confidence score를 verdict에 포함하여 가중 투표
- 이종 모델(Claude + Gemini + Codex) 투표가 동종 모델 3회보다 다양성 확보에 유리

---

### 2.4 Self-Refine — 단일 모델 반복 개선

> 논문: [Self-Refine: Iterative Refinement with Self-Feedback](https://arxiv.org/abs/2303.17651) (NeurIPS 2023)

**핵심 아이디어**: Generate → Feedback → Refine 루프를 **같은 모델**로 반복.

```
Initial output → Self-feedback ("이 부분이 약하다") → Refined output → 반복
```

**성과**: 평균 ~20% 성능 향상 (7개 태스크). 추가 학습 불필요.

**tunaFlow 적용 포인트**:
- **Developer 구현 후 Self-Refine**: Developer가 자기 코드를 리뷰하는 1차 단계 추가
- **Architect plan-proposal 후 Self-Refine**: 제안 후 자체 비판 → 개선된 제안
- 비용: 동일 모델 2-3회 호출이므로 RT(다중 모델)보다 저렴

---

### 2.5 LLM-as-Judge + 구조화된 루브릭

> 서베이: [A Survey on LLM-as-a-Judge](https://arxiv.org/abs/2411.15594) (2024)
> 프레임워크: [Agent-as-a-Judge](https://arxiv.org/html/2508.02994v1) (2025)

**핵심 아이디어**: LLM이 다른 LLM의 출력을 **정형화된 루브릭**으로 평가.

**루브릭 유형**:
- **Short Scoring**: 1-5점 척도 + 소프트 라벨
- **Long Scoring**: 다차원 평가 (정확성, 완전성, 코드 품질 등 개별 점수)
- **Binary**: True/False 판정

**Agent-as-a-Judge (2025)**: 단순 텍스트 비교가 아니라 에이전트가 **도구를 사용해서** 검증 (테스트 실행, 파일 읽기 등).

**tunaFlow 적용 포인트**:
- Review RT의 verdict를 **구조화된 루브릭 기반 채점**으로 전환
- 각 평가 차원(plan 일치도, 코드 품질, 테스트 커버리지)에 개별 점수
- Agent-as-a-Judge: Reviewer가 `run_project_tests` 결과를 직접 참조하여 판정

---

### 2.6 Automated Quality Gate

> 논문: [Automated Self-Testing as a Quality Gate](https://arxiv.org/abs/2603.15676) (2025)

**핵심 아이디어**: LLM 애플리케이션의 릴리스 결정을 자동화하는 프레임워크.

**5-dimension 판정**:
1. Task success rate
2. Context preservation
3. P95 latency
4. Safety pass rate
5. Evidence coverage

**판정**: PROMOTE / HOLD / ROLLBACK (3-way)

**tunaFlow 적용 포인트**:
- Review verdict의 pass/fail/conditional을 **다차원 점수 기반**으로 강화
- 각 차원에 임계값 설정 → 자동 판정 가능
- tunaFlow의 현재 3-way verdict(pass/fail/conditional)와 구조적으로 일치

---

## 3. tunaFlow RT 강화 제안 — 우선순위별

> **우선순위 결정 근거** (Codex 리뷰 반영):
> "먼저 판정 형식을 고정하고, 그 다음 집계하고, 마지막에 통합하라."
> - 출력 계약(루브릭)이 없으면 투표 집계가 약하고, Synthesizer가 "그럴듯한 덮어쓰기"가 됨
> - 루브릭 → 투표 → Synthesizer 순서가 각 단계의 입력 품질을 보장

### P0: 구조화된 Verdict 루브릭 (기반 인프라 이미 존재)

**현재 문제**: verdict가 pass/fail + 자유형식 findings → 일관성 없음
**해결**: 다차원 루브릭 기반 채점

```markdown
<!-- tunaflow:review-verdict -->
verdict: conditional
scores:
  plan_coverage: 4/5      # subtask 구현 완전성
  code_quality: 3/5        # 버그, 보안, 성능
  test_coverage: 2/5       # 테스트 커버리지
  convention: 5/5          # 코딩 컨벤션 준수
total: 14/20
threshold: 16/20           # pass 기준
findings:
- [code_quality] SQL injection 위험: src/api/users.ts:42
- [test_coverage] 에러 케이스 테스트 누락
recommendations:
- parameterized query로 변경
- 400/404/500 응답 테스트 추가
<!-- /tunaflow:review-verdict -->
```

**구현 방법**:
- `planProposalParser.ts`에 scores 파서 추가
- `ReviewVerdictCard`에 차원별 점수 시각화 (바 차트 또는 레이더)
- Reviewer 프롬프트에 루브릭 차원 명시
- 총점 ≥ threshold → auto pass, 미만 → auto fail/conditional

**예상 효과**:
- 리뷰 일관성 향상 (자유형식 → 정형화)
- 자동 판정 가능 (사용자 개입 감소)
- AutoBe의 "스키마가 프롬프트를 대체" 원칙 적용

---

### P1: Self-Consistency 투표 메커니즘 (루브릭 의존)

**전제조건**: P0 루브릭이 있어야 점수 기반 집계가 의미 있음

**현재 문제**: 2명의 Reviewer verdict가 다르면 사용자가 수동 판단
**해결**: 구조화 점수 기반 투표 자동 집계

```
Reviewer-A: pass (score 18/20)
Reviewer-B: fail (score 12/20)

투표 결과:
- pass: 1표, fail: 1표
- 가중 평균 점수: 15/20
- threshold(16) 미달 → 최종: conditional
- 쟁점 차원: [test_coverage] A=4/5 vs B=1/5
```

**투표 규칙**:
1. 만장일치 pass → auto pass
2. 과반 fail → auto fail
3. 그 외 → conditional (사용자 판단)
4. 차원별 점수 분산이 큰 항목을 "쟁점"으로 하이라이트

**구현 방법**: `workflowOrchestration.ts`의 `processReviewVerdict` 확장. 프론트엔드에 투표 결과 시각화.

---

### P1: MoA Synthesizer 라운드 — Structured Reducer (투표 의존)

> Codex 리뷰 반영: Synthesizer는 "final answer generator"가 아니라 "structured reducer"여야 한다.
> 투표 결과가 있어야 "근거 있는 통합자"가 됨. 투표 없이 넣으면 dissent를 덮어쓸 위험.

**전제조건**: P0 루브릭 + P1 투표가 있어야 의미 있음

**현재 문제**: RT 완료 후 사용자가 수동으로 결과 종합
**해결**: 투표 집계 후 Synthesizer가 구조화된 통합 리포트 생성

```
Round 1: [Reviewer-A(Claude), Reviewer-B(Gemini)]  → 독립 리뷰 (루브릭 verdict)
Vote:    자동 집계 (점수 평균, 쟁점 차원 식별)
Round 2: [Synthesizer(Claude)]                      → 구조화 통합
```

**Synthesizer 출력 형식** (원 verdict 보존):

```markdown
## 합의점
- [plan_coverage] 모든 reviewer 4+/5 — subtask 완전 구현 합의
- [convention] 모든 reviewer 5/5 — 컨벤션 준수 합의

## 쟁점
- [test_coverage] Reviewer-A: 4/5 vs Reviewer-B: 2/5
  - A: "기본 케이스 커버" vs B: "에러 케이스 누락"

## Blind Verifier Dissent
- "SQL injection 위험이 code_quality 점수에 반영되지 않음"

## 최종 권고
verdict: conditional (쟁점 해소 필요)
해소 필요 항목: test_coverage, SQL injection
```

**핵심 제약**:
- 원문(reviewer별 verdict + findings) 전체를 **보존** — 덮어쓰지 않음
- blind verifier dissent를 **별도 블록으로 유지**
- 쟁점은 **양측 근거를 병렬 표시** — 한쪽으로 결론 내리지 않음
- 최종 verdict는 **투표 집계 결과와 일관** — synthesizer가 투표를 뒤집지 않음

**구현 방법**:
- `executor.rs`에 `auto_synthesize: bool` 옵션
- Round 1 완료 + 투표 집계 후 자동으로 Synthesizer participant 추가 실행
- Synthesizer 프롬프트에 투표 결과 + reviewer verdict 전문 포함
- 변경 범위: `executor.rs` ~30줄, `roundtable.rs` ~20줄

---

### P2: Self-Refine 사전 검증 단계

**현재 문제**: Developer 코드가 바로 Review RT로 → 기본적인 문제도 Reviewer가 발견해야 함
**해결**: Review RT 전에 Developer 자체 리뷰 1회

```
Developer 구현 완료
  → Self-Refine: "방금 작성한 코드를 plan 기준으로 자체 리뷰하세요"
  → 자체 수정 (있으면)
  → 수정된 코드로 Review RT 진입
```

**예상 효과**: Self-Refine 논문 기준 ~20% 성능 향상. Reviewer가 고수준 이슈에 집중 가능.

---

### P2: Agent-as-Judge — 도구 기반 검증

**현재 문제**: Reviewer가 코드를 텍스트로만 읽음. 실제 실행 결과 참조 제한적.
**해결**: Reviewer가 테스트 결과를 구조화된 형태로 직접 받음

```
Review RT 시작 전:
1. run_project_tests 실행
2. 결과를 Reviewer 프롬프트에 구조화 삽입:
   ## 테스트 결과
   - cargo test: 60/60 passed ✅
   - vitest: 78/78 passed ✅
   - 새로 추가된 테스트: 4/4 passed ✅
```

현재 `startReviewRT`에 `testOutput` 파라미터가 이미 있으므로, 자동 실행 + 주입만 연결하면 됨.

---

### P3: Adaptive Stopping — 라운드 자동 종료

**현재 문제**: RT는 항상 1라운드만 실행. 다라운드 시 사용자가 수동으로 followup.
**해결**: Beta-Binomial 안정성 감지로 합의 도달 시 자동 종료

> 참고: [Multi-Agent Debate with Adaptive Stability Detection](https://arxiv.org/html/2510.12697v1)

```
Round 1: 참가자 verdict 분산 높음 → 계속
Round 2: verdict 수렴 (모두 pass 또는 점수 차이 < 2) → 자동 종료
Round 3 (필요 시): 여전히 분산 → 사용자에게 판단 위임
```

**구현 복잡도**: 높음. 다라운드 RT 자동 실행 + 수렴 판정 로직 필요.

---

### P3: 이종 모델 조합 최적화

ICLR 2025 분석에서 **이종 모델 조합이 가장 유망**:
- GPT-4o-mini + Llama3.1-70b → MMLU 88.2% (단일 모델 최고 79%)
- tunaFlow의 4-engine(Claude + Gemini + Codex + OpenCode)이 이미 이 방향

**추가 최적화**:
- 역할별 최적 엔진 매핑 학습 (Architect=Opus, Developer=Sonnet, Reviewer=Gemini 등)
- RT에서 엔진 조합별 verdict 품질 추적 → 최적 조합 자동 추천

---

## 4. MAD 비판에서 얻은 설계 원칙

ICLR 2025 MAD 비판 논문에서 tunaFlow RT가 반드시 지켜야 할 원칙:

| # | 원칙 | 근거 | tunaFlow 적용 |
|---|------|------|--------------|
| 1 | **전체 응답 토론 금지** | 세부 오류 감지 실패 | subtask 단위로 리뷰 분리 |
| 2 | **blind 모드 활용** | 답 선호도 편향 방지 | 이미 구현됨 ✅ |
| 3 | **투표 > 자유 토론** | 다수결이 토론보다 효과적 | 투표 메커니즘 도입 (P1) |
| 4 | **이종 모델 혼합** | 동종 모델 복제보다 다양성이 중요 | 4-engine 지원 ✅ |
| 5 | **검증 가능한 구조화 출력** | 자연어 토론은 수렴 보장 없음 | 루브릭 기반 verdict (P0) |
| 6 | **과도한 수정 방지** | MAD가 맞는 답을 틀리게 바꿈 | Synthesizer = structured reducer, 원 verdict 보존 (P1) |

---

## 5. 구현 로드맵

> 순서 원칙: "먼저 판정 형식을 고정하고, 그 다음 집계하고, 마지막에 통합하라." (Codex 리뷰)

```
Step 1 — 출력 계약 확립 (기존 파서 인프라 활용):
├── P0: 구조화된 Verdict 루브릭 (~100줄 TS 파서 + UI)
│       verdict 마커/파서/처리 흐름이 이미 존재 → 점수 필드 확장
│
Step 2 — 집계 메커니즘 (루브릭 위에):
├── P1: 투표 메커니즘 (~50줄 TS)
│       구조화 점수가 있으므로 가중 평균/다수결 안정적
├── P1: MoA Synthesizer — structured reducer (~50줄 Rust)
│       투표 결과 + 원 verdict 입력 → 합의/쟁점/dissent 정리
│
Step 3 — 품질 게이트 연동:
├── P2: Agent-as-Judge 테스트 결과 자동 주입 (파라미터 이미 존재)
├── P2: Self-Refine 사전 검증 (~30줄 TS)
│
Step 4 — 고도화:
├── P3: Adaptive Stopping (다라운드 + 수렴 감지)
├── P3: 이종 모델 조합 최적화 (추적 + 추천)
```

---

## 6. 핵심 인사이트 요약

### AutoBe/Typia에서 가져올 것
- **"검증 가능하면 수렴한다"** → verdict를 구조화하면 자동 판정 가능
- **스키마 기반 제약** → 루브릭이 자연어 verdict를 대체
- **소형 모델 = 스키마 스트레스 테스터** → 루브릭 견고성 검증에 활용

### MoA에서 가져올 것
- **Collaborativeness** → Sequential RT + Synthesizer 라운드가 최적 구조
- **약한 모델도 강한 모델을 향상시킴** → 다양한 엔진 혼합의 근거
- **단, Synthesizer는 루브릭+투표 뒤에** → 출력 계약 없이 통합하면 "덮어쓰기"가 됨 (Codex 리뷰)

### MAD 비판에서 가져올 것
- **투표가 토론보다 낫다** → 자유 토론 대신 구조화 투표
- **과도한 수정 방지** → Synthesizer = structured reducer, dissent 보존
- **blind 모드 필수** → 이미 구현됨, 적극 활용

### Self-Refine에서 가져올 것
- **자체 피드백 루프** → Developer Self-Refine이 Review 품질 부담 감소
- **추가 학습 불필요** → 프롬프트만으로 구현 가능

### Quality Gate에서 가져올 것
- **다차원 판정** → 5가지 차원 점수 기반 자동 PROMOTE/HOLD/ROLLBACK
- **tunaFlow verdict와 구조 일치** → pass/fail/conditional ≈ PROMOTE/HOLD/ROLLBACK

---

## 참고 자료

### 논문
- [Mixture-of-Agents Enhances Large Language Model Capabilities](https://arxiv.org/abs/2406.04692) — Together AI, 2024
- [Improving Factuality and Reasoning through Multiagent Debate](https://arxiv.org/abs/2305.14325) — Du et al., ICML 2024
- [Multi-LLM-Agents Debate: Performance, Efficiency, and Scaling](https://d2jud02ci9yv69.cloudfront.net/2025-04-28-mad-159/blog/mad/) — ICLR 2025 Blog
- [Self-Refine: Iterative Refinement with Self-Feedback](https://arxiv.org/abs/2303.17651) — Madaan et al., NeurIPS 2023
- [A Survey on LLM-as-a-Judge](https://arxiv.org/abs/2411.15594) — 2024
- [Agent-as-a-Judge](https://arxiv.org/html/2508.02994v1) — 2025
- [Automated Self-Testing as a Quality Gate](https://arxiv.org/abs/2603.15676) — 2025
- [Multi-Agent Debate with Adaptive Stability Detection](https://arxiv.org/html/2510.12697v1) — 2025
- [Ranked Voting based Self-Consistency](https://arxiv.org/abs/2505.10772) — 2025
- [Confidence Improves Self-Consistency](https://arxiv.org/pdf/2502.06233) — 2025
- [Self-Consistency Improves Chain of Thought Reasoning](https://arxiv.org/abs/2203.11171) — Wang et al., 2023

### 연관 tunaFlow 문서
- `docs/ideas/smallModelStressTesterIdea.md` — 소형 모델 stress tester 검토
- `docs/ideas/guardrailImprovementIdeas.md` — 가드레일 개선
- `docs/plans/orchestratedWorkflowPipelinePlan.md` — 워크플로우 파이프라인 (archived)
- `docs/reference/multiAgentContextStrategy.md` — RT 컨텍스트 전략
