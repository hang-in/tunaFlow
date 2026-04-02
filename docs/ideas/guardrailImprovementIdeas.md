# 가드레일 개선 아이디어

> Status: idea
> Created: 2026-04-01
> 출처: Claude Code 소스코드 안전 아키텍처 분석 (wikidocs.net/338204)

---

## 배경

Claude Code는 6-layer 안전 파이프라인을 구현하고 있다:

1. **Input Validation** — Zod 스키마로 시맨틱 검증
2. **PreToolUse Hooks** — 사용자 정의 훅으로 실행 전 차단
3. **Permission Rules** — allow/deny/ask 규칙 평가
4. **AI Classifier** — 2단계 위험도 평가 (fast + thinking)
5. **Tool-Specific Checks** — 도메인별 안전 로직
6. **Execution Isolation** — 샌드박스 환경

tunaFlow 현재 상태와 대조하여 적용 가치 있는 개선점을 정리한다.

---

## 현재 tunaFlow 가드레일 현황

| 영역 | 현재 구현 | 수준 |
|------|----------|------|
| 입력 검증 | `serde::Deserialize` 타입 검증만 | 최소 |
| 권한 분리 | 없음 — 모든 에이전트 동일 권한 | 없음 |
| 셸 명령 보호 | `run_project_tests`만 존재, 일반 셸 미지원 | 해당 없음 |
| 동시성 안전 | thread-local queue 직렬 실행 | 충분 |
| 예산 제어 | `guardrail.rs` 하드코딩 상수 + `context_budget_cap` | 기본 |
| 에러 복구 | `agent:error` 이벤트 → UI 표시만, 재시도 없음 | 최소 |
| 위임 안전 | RT participant `blind`/`role` 있으나 권한 제한 아님 | 최소 |
| 데이터 영속 | DB = SSOT, 이벤트 유실 시 `list_messages`로 복구 | 충분 |
| 격리 | 프로젝트 경로 기반, `scaffold_project_dir` 범위 제한 | 기본 |

---

## 아이디어 1: 워크플로우 역할별 권한 분리

### Claude Code 참고

Claude Code의 Permission Modes:
- **Default**: 읽기 자동 승인, 위험 작업은 사용자 확인
- **Auto**: AI classifier가 결정, 위험 작업은 여전히 프롬프트
- **Plan**: 읽기 전용 도구만 (분석 단계)
- **Bypass**: 전부 자동 승인 (개발 전용)

### tunaFlow 적용 방안

워크플로우 파이프라인의 각 역할에 권한 수준을 부여:

| 역할 | 권한 수준 | 허용 범위 |
|------|----------|----------|
| Architect | Plan 모드 | 분석 + plan-proposal 생성만. 코드 수정 금지 |
| Developer | Auto 모드 | 승인된 plan 범위 내 코드 수정. plan 외 변경 경고 |
| Reviewer | ReadOnly 모드 | 읽기 + verdict 생성만. 코드/plan 수정 금지 |

**구현 레벨 옵션:**

1. **Soft (현재 가능)**: `docs/agents/*.md` 템플릿에 행동 제약을 상세화. 에이전트가 자발적으로 준수. 무시 가능.
2. **Medium**: CLI 플래그 전달. Claude는 `--permission-mode plan`, Codex/Gemini는 프롬프트에 제약 강화.
3. **Hard**: ContextPack에 `## Restrictions` 섹션 주입 + 응답 후처리에서 금지 패턴 감지 (코드 블록 포함 여부 등).

### 위험도

- Soft: 위험 낮음 (현재와 동일)
- Medium: 위험 낮음 (CLI 플래그 추가만)
- Hard: 위험 중간 (후처리 로직 추가 필요, false positive 가능)

---

## 아이디어 2: 동적 예산 배분

### Claude Code 참고

- 세션 단위 누적 토큰 추적
- 예산 임계값 초과 시 실행 중단 (`maxBudgetUsd`)
- 모델별 input/output/cache 토큰 분리 계산

### tunaFlow 현재 문제

`guardrail.rs`의 섹션별 상수가 고정값:

```
MAX_SKILLS_SECTION    = 8,000    ← 스킬 1개(500자)에도 8,000 예약
MAX_RAWQ_SECTION      = 4,000    ← rawq 결과 0개여도 4,000 예약
합계                   36,000    (60,000 중 60% 사전 할당)
```

빈 섹션이 예산을 차지하고, 내용이 풍부한 섹션이 truncation 당함.

### tunaFlow 적용 방안

**Phase 1: 동적 배분 함수** (~80줄 Rust)

```rust
fn allocate_budget(
    total: usize,
    sections: &[(name, content_len, weight, min_chars, max_chars)]
) -> Vec<(name, allocated)>
```

- 각 섹션에 `min_chars` 보장 + 가중치 비례 잔여 배분
- 빈 섹션(content_len=0)은 `min_chars`도 반납
- max cap 유지 → 한 섹션이 전체 독점 방지

**Phase 2: 대화 단위 비용 cap**

- `conversations.total_cost_usd` 필드 활용
- 설정에서 대화당/프로젝트당 비용 상한 설정
- 상한 초과 시 경고 + 전송 차단 옵션

### 기존 계획 참조

- `docs/plans/contextPackAlgorithmImprovementsPlan.md` — P2로 이미 계획됨 (line 160-276)
- `docs/reference/externalToolAnalysisAndRefactoringDirection.md` — 중간 심각도 지적 (line 167)

---

## 아이디어 3: 에러 복구 + 재시도

### Claude Code 참고

- `413 Prompt Too Long` → 자동 context 축소 후 재시도
- `Max Output Tokens` → 8K→64K 단계적 확대, 3회 재시도
- 복구 가능한 에러는 버퍼링 후 자동 처리

### tunaFlow 현재 문제

에이전트 실패 시:
1. `agent:error` 이벤트 발생
2. `finalize_engine_run()`에서 메시지 status='error' 저장
3. UI에 에러 표시
4. **끝** — 재시도, context 축소, fallback 없음

### tunaFlow 적용 방안

**Phase 1: Context 초과 자동 축소 + 재시도**

```rust
// send_common.rs — prepare_engine_run 내부
let (ep, sys_ctx, meta) = assemble_prompt(&data, identity_frag);
if meta.length > MAX_TOTAL_PROMPT {
    // context_mode를 한 단계 낮추어 재조립
    data.context_mode_override = Some(downgrade_mode(&meta.mode));
    let (ep2, sys_ctx2, meta2) = assemble_prompt(&data, identity_frag);
    // 축소된 버전 사용
}
```

**Phase 2: 에이전트 타임아웃 1회 재시도**

```rust
// finalize_engine_run 또는 background thread에서
match result {
    Err(AppError::Agent(msg)) if msg.contains("timeout") && retry_count == 0 => {
        // 1회 재시도
    }
    Err(ref e) => { /* 기존 에러 처리 */ }
}
```

**Phase 3: UI "재시도" 버튼**

- 에러 메시지 옆에 "Retry" 버튼
- 클릭 시 동일 prompt + context로 재전송
- 마지막 user 메시지의 content를 재사용

### 위험도

- Phase 1: 낮음 (조립 단계에서만 동작, 외부 호출 없음)
- Phase 2: 중간 (비용 2배 가능성 → cost cap과 연동 필요)
- Phase 3: 낮음 (UI 변경만)

---

## 우선순위 제안

| 순위 | 아이디어 | 효과 | 난이도 |
|------|---------|------|--------|
| P1 | 동적 예산 배분 (아이디어 2, Phase 1) | context 품질 직접 향상 | ~80줄 Rust |
| P2 | 에러 복구 — context 축소 재시도 (아이디어 3, Phase 1) | 실패율 감소 | ~30줄 Rust |
| P2 | UI 재시도 버튼 (아이디어 3, Phase 3) | UX 개선 | ~50줄 TSX |
| P3 | 역할별 권한 분리 Soft (아이디어 1) | 안전성 | 템플릿 수정만 |
| P3 | 대화 비용 cap (아이디어 2, Phase 2) | 비용 제어 | DB + Settings UI |
| 후순위 | 역할별 권한 Hard (아이디어 1) | 강제력 | 후처리 로직 필요 |

---

## 참고 자료

- Claude Code 소스코드 안전 아키텍처 분석: https://wikidocs.net/338204
- tunaFlow 가드레일 현황: `src-tauri/src/guardrail.rs`
- ContextPack 알고리즘 개선 계획: `docs/plans/contextPackAlgorithmImprovementsPlan.md`
- 외부 도구 분석 리팩토링 방향: `docs/reference/externalToolAnalysisAndRefactoringDirection.md`
