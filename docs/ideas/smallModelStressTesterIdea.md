# 소형 모델 Stress Tester — RT 투입 검토

> Status: idea
> Created: 2026-04-01
> 출처: AutoBe (github.com/wrtnlabs/autobe), Typia (github.com/samchon/typia)
> 원문: dev.to/samchon/qwen-meetup-function-calling-harness-from-675-to-100-3830

---

## 1. AutoBe/Typia 패턴 요약

### 핵심 발견

**소형 모델(3B active)이 설계 취약점을 정직하게 노출한다.**

- Qwen3.5 3B active → 6.75% 성공률 → 스키마의 모호성을 즉시 노출
- 대형 모델은 모호한 스키마도 "정확히 추측" → 숨은 결함을 가림
- Typia harness(파싱 + 검증 + 피드백 루프) 적용 → **모든 모델이 99.8-100%** 달성

### AutoBe 아키텍처

5-phase 워터폴 파이프라인 (40+ 전문 에이전트):

```
Analyze → Database → Interface → Test → Realize
```

각 phase에서 LLM 출력을 **구조화된 AST**로 제약하고, **컴파일러로 검증**:
- Database: 7개 타입만 허용 (`boolean | int | double | string | uri | uuid | datetime`)
- API: OpenAPI v3.2 스펙으로 검증
- Test: 30+ IExpression 변형의 재귀 union type
- **스키마에 없는 것은 물리적으로 출력 불가능**

### Typia Harness 패턴

```
LLM → parse() → validate() → stringify() → [feedback] → LLM (루프)
```

4-layer 처리:
1. **Parse**: 깨진 JSON 복구 + 타입 강제 변환
2. **Validate**: 스키마 위반 감지
3. **Feedback**: 인라인 에러 코멘트 (`// ❌ [path, expected]`)
4. **Correct**: LLM이 피드백 받아 해당 필드만 수정

### 소형 모델의 전략적 가치

| 모델 | Active 파라미터 | 성공률 | 역할 |
|------|---------------|--------|------|
| qwen3-30b-a3b | 3B | ~10% | 스키마 모호성 즉시 노출 |
| qwen3-next-80b-a3b | 3B | ~20% | 미묘한 nested 타입 불일치 발견 |
| qwen3.5-27b | 27B | 100% | 정상 운영 |
| qwen3.5-397b-a17b | 17B | 100% | 정상 운영 |

**"3B 모델이 깨뜨릴 수 없으면, 어떤 모델도 깨뜨릴 수 없다"**

---

## 2. tunaFlow RT 현재 구조

### 실행 모드

- **Sequential**: 참가자 순차 실행, 이전 참가자 응답 가시
- **Deliberative**: 참가자 병렬 실행, 현재 라운드 응답 비가시

### 참가자 구조

```typescript
interface RoundtableParticipant {
  name: string;
  engine?: string;    // "claude" | "codex" | "gemini" | "opencode"
  model?: string;
  blind?: boolean;    // true면 다른 참가자 응답 비가시
  role?: string;      // "proposer" | "reviewer" | "verifier" | "synthesizer"
  maxTokens?: number; // 역할별 기본값 있음 (verifier=800, reviewer=900 등)
}
```

### 엔진 패턴

모든 엔진이 동일 인터페이스:
```rust
pub fn run(input: RunInput) -> Result<RunOutput, AppError>
// RunInput: prompt, model, system_prompt, project_path
// RunOutput: content, cost_usd, input_tokens, output_tokens
```

새 엔진 추가: `agents/` 모듈 + `executor.rs` match arm 추가만으로 가능. RT 핵심 로직 변경 불필요.

---

## 3. tunaFlow RT에 qwen 9b 투입 — 현재 의미가 제한적인 이유

### 이유 1: RT 출력은 자연어, 구조화된 스키마가 아니다

AutoBe/Typia의 핵심은 **JSON 스키마 + 컴파일러 검증 + 피드백 루프**. LLM 출력이 deterministic 구조에 맞는지 기계적으로 검증 가능.

tunaFlow RT의 출력:
- `plan-proposal` — 마크다운 + HTML 마커 (자유 형식)
- `review-verdict` — pass/fail + findings (자연어)
- 구현 보고서 — 자유 형식

**deterministic 검증기가 없으므로**, 소형 모델 실패가 "구조적 결함 발견"인지 "모델 능력 부족"인지 구분 불가.

### 이유 2: RT의 가치는 다양한 관점, 스키마 테스트가 아님

RT에서 Reviewer-A(Claude) + Reviewer-B(Gemini)를 두는 이유: **서로 다른 추론 패턴으로 다른 결함 발견**.

qwen 9b가 발견하는 것은 "결함"이 아니라 "이 프롬프트를 이해하지 못했다"일 가능성이 높음. AutoBe에서 소형 모델이 가치있는 이유는 실패가 **스키마의 모호성**을 가리키기 때문. tunaFlow에서는 실패가 **프롬프트의 난이도**만 가리킴.

### 이유 3: 런타임 비용-품질 트레이드오프

AutoBe는 소형 모델을 **R&D 단계** (스키마 검증)에 사용. tunaFlow RT는 **런타임**.

9b가 의미있는 리뷰를 하려면: plan + impl summary + test results = 수천 토큰 컨텍스트 필요. 9b의 context window와 추론 능력으로는 이 양의 정보를 소화하고 유의미한 verdict를 내기 어려움.

---

## 4. 의미있을 수 있는 조건

### 조건: 마커를 JSON 스키마 기반으로 강화

현재:
```markdown
<!-- tunaflow:review-verdict -->
verdict: pass
findings:
- 구현이 plan과 일치합니다
<!-- /tunaflow:review-verdict -->
```

미래 (Typia 패턴 적용 시):
```json
{
  "verdict": "pass" | "fail" | "conditional",
  "findings": [{ "severity": "critical" | "major" | "minor", "file": "string", "line": "number", "description": "string" }],
  "recommendations": [{ "action": "string", "priority": "number" }]
}
```

이 경우:
- Typia식 파서 + 검증기 + 피드백 루프 적용 가능
- qwen 9b가 이 스키마를 깨뜨리면 → 스키마가 더 명확해야 한다는 신호
- 모든 모델이 동일 harness를 통과하면 → 구조적 견고성 보장

---

## 5. 투입 위치와 형태 — 적합성 평가

| 투입 위치 | 형태 | 가치 | 이유 |
|----------|------|------|------|
| **R&D: 마커 파서 테스트** | qwen 9b 출력으로 파서 견고성 검증 | **높음** | 파서가 다양한 형식 변형을 처리하는지 확인 |
| **R&D: 프롬프트 품질 벤치마크** | 워크플로우 프롬프트를 9b에 줘서 이해도 측정 | **중간** | 프롬프트 명확성 개선 지표 |
| **런타임: RT blind verifier** | RT 3번째 참가자, blind 모드 | **낮음** | verdict 품질이 리뷰에 부족 |
| **런타임: RT 일반 reviewer** | RT 참가자 | **매우 낮음** | 노이즈만 증가, 유의미한 발견 어려움 |

---

## 6. 실행 가능한 활용 방안

### Phase 1: R&D 도구 — 마커 파서 Fixture (즉시 가능)

`planProposalParser.test.ts`에 qwen 9b 실제 출력 fixture 추가:

```typescript
// qwen 9b가 생성한 plan-proposal 변형들
const QWEN_FIXTURES = [
  // 마커 태그 불완전 (닫는 태그 누락)
  "<!-- tunaflow:plan-proposal -->\n## Plan: Test\n### Subtasks\n1. 작업",
  // 마크다운 구조 불일치 (### 대신 **)
  "<!-- tunaflow:plan-proposal -->\n**Plan Proposal: Test**\n...\n<!-- /tunaflow:plan-proposal -->",
  // JSON 혼합 (구조화 시도했으나 마크다운과 혼합)
  "<!-- tunaflow:plan-proposal -->\n{\"title\": \"Test\", \"subtasks\": [...]}\n<!-- /tunaflow:plan-proposal -->",
];

for (const fixture of QWEN_FIXTURES) {
  test(`parser handles qwen variant: ${fixture.slice(0, 40)}...`, () => {
    // 파서가 crash하지 않고 graceful하게 처리하는지 검증
    expect(() => splitPlanProposals(fixture)).not.toThrow();
  });
}
```

### Phase 2: R&D 도구 — 프롬프트 명확성 벤치마크 (단기)

워크플로우 프롬프트(Developer pre-report, Review prompt)를 ollama qwen 9b에 실행:

```bash
# 수동 벤치마크
ollama run qwen3.5:9b "당신은 Developer입니다. 아래 Plan을 구현해야 합니다..."
```

9b가 `<!-- tunaflow:impl-plan -->` 형식을 지키는지 확인. 실패 시 프롬프트 개선.

**성공 기준**: 9b가 마커 형식의 골격이라도 출력하면 프롬프트가 충분히 명확한 것.

### Phase 3: 구조화 출력 강화 후 — Harness 패턴 도입 (장기)

마커를 JSON 스키마로 강화한 후:

1. `typia` 또는 `zod` 기반 검증기로 에이전트 출력 validate
2. 실패 시 인라인 피드백 → 에이전트 재시도 (자동 수정 루프)
3. qwen 9b를 스키마 스트레스 테스터로 정식 투입
4. "9b가 3회 재시도 내 통과" = 스키마 견고성 증명

---

## 7. 새 엔진 추가 기술 경로 (ollama 통합)

현재 RT 아키텍처에서 ollama 추가는 기술적으로 간단:

```
필요한 변경:
1. src-tauri/src/agents/ollama.rs        (~100줄, subprocess spawn + 출력 파싱)
2. executor.rs match arm 1줄 추가        ("ollama" => ollama::run(input))
3. model_discovery.rs ollama 추가         (ollama ls --json 파싱)
4. types/index.ts engine 타입 확장        ("ollama" 추가)
5. lib.rs command 등록                    (start_ollama_run)
```

RT 핵심 로직(prompt builder, blind mode, output cap, persistence) 변경 없음.

---

## 8. 핵심 인사이트 정리

| AutoBe/Typia 인사이트 | tunaFlow 적용 여부 | 이유 |
|----------------------|-------------------|------|
| 소형 모델이 스키마 결함 노출 | **조건부** | 현재 자연어 마커 → JSON 스키마 전환 후에만 유효 |
| Harness(파싱+검증+피드백 루프) | **적용 가치 높음** | 마커 파서 견고성 + 자동 수정 루프 |
| "검증 가능하면 수렴한다" | **핵심 원칙** | tunaFlow 워크플로우 전체에 적용 가능 |
| 스키마가 프롬프트를 대체 | **장기 방향** | 자연어 마커 → 구조화 스키마 전환 시 |
| R&D 단계에서 소형 모델 활용 | **즉시 가능** | 파서 fixture + 프롬프트 벤치마크 |

### 최종 결론

**현재 tunaFlow RT에 qwen 9b를 런타임 참가자로 투입하는 것은 의미가 낮다.** 자연어 기반 마커에는 deterministic 검증기가 없어 소형 모델 실패에서 구조적 인사이트를 추출할 수 없다.

**R&D 도구로는 즉시 가치가 있다.** 파서 견고성 테스트와 프롬프트 명확성 벤치마크에 활용 가능.

**장기적으로 마커를 JSON 스키마로 강화하면**, AutoBe/Typia의 "소형 모델 = 스키마 스트레스 테스터" 패턴이 tunaFlow에서도 완전히 유효해진다. 이 전환이 이루어지면 qwen 9b (또는 더 작은 모델)를 RT R&D 검증 에이전트로 정식 투입할 수 있다.

---

## 참고 자료

- AutoBe: https://github.com/wrtnlabs/autobe
- Typia: https://github.com/samchon/typia
- 원문: https://dev.to/samchon/qwen-meetup-function-calling-harness-from-675-to-100-3830
- tunaFlow RT 구조: `src-tauri/src/commands/roundtable.rs`, `roundtable_helpers/executor.rs`
- tunaFlow 마커 파서: `src/lib/planProposalParser.ts`
