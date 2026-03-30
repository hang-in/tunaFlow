# ContextPack Compression + rawq Post-Processing Plan

> 작성: 2026-03-30
> 선행: 4-engine context metadata parity, context visibility UI polish

## 목적

ContextPack의 내용 품질을 개선한다. 두 축:
1. **Compression 품질 개선** — 긴 section이 truncate fallback으로 가기 전에 compression 결과를 더 쓸모있게 만든다
2. **rawq post-processing** — top-K 결과를 그대로 넣지 않고 dedup/confidence 보강/snippet 확장을 수행한다

범위는 tunaFlow 내부에서 바로 가치가 나는 최적화로 고정. entroly나 외부 retrieval 변경은 포함하지 않는다.

## 현재 상태

### Compression (`compression.rs`)
- `maybe_compress_section(section, limit)`: limit 초과 시 Claude로 <600자 요약 → 실패 시 truncate fallback
- 문제점:
  - 요약 목표가 600자 고정 — section 유형/중요도와 무관
  - Claude 실패 시 무조건 truncate (중간 단계 없음)
  - cross-session, context summary 같은 다중 항목 섹션은 항목별 압축이 아닌 전체 압축 → 정보 손실 큼

### rawq (`rawq.rs` + `context_pack.rs`)
- `search(path, query, 5)` → 상위 5개 → 120자 snippet → `## Code context (rawq)` 섹션
- 문제점:
  - dedup 없음 — 같은 파일의 인접 라인이 중복 등장 가능
  - confidence 정보가 log에만 기록, prompt에 미포함 → 모델이 신뢰도 판단 불가
  - snippet이 120자로 잘림 — 함수 시그니처만 남고 본문 없음
  - scope(Struct.method) 정보가 SearchResult에 미포함

## 변경 사항

### Phase 1: rawq post-processing 개선

#### Step 1.1: SearchResult에 scope, confidence 추가

`agents/rawq.rs`:
```rust
pub struct SearchResult {
    pub file: String,
    pub line: usize,
    pub snippet: String,
    pub scope: Option<String>,      // NEW
    pub confidence: f64,             // NEW
}
```

`parse_json()`에서 이미 읽고 있는 scope, confidence를 SearchResult에 포함.

#### Step 1.2: dedup + confidence 기반 재정렬

`context_pack.rs`의 `build_rawq_section()` 내에서 후처리:
1. **File dedup**: 같은 file+line 범위(±5줄 이내)의 결과를 병합, 더 높은 confidence 유지
2. **Confidence 정렬**: 병합 후 confidence 내림차순 정렬
3. **Low-confidence 필터**: confidence < 0.4이면 제외 (rawq의 0.3 threshold보다 약간 엄격)

#### Step 1.3: snippet 확장 + confidence 표시

rawq 결과 형식 개선:
```
## Code context (rawq)

`src/agents/rawq.rs` L344 (search, 91%):
pub fn search(project_path: &str, query: &str, limit: usize) -> Result<Vec<SearchResult>, RawqError> {
    let bin = resolve_rawq_bin()?;
    let mut cmd = Command::new(&bin);
    ...

`src/commands/model_discovery.rs` L197 (get_models_for_engine, 78%):
let discovered = match engine { ... };
```
- scope 이름을 `(scope, confidence%)` 형태로 표시
- snippet을 120자 → 300자로 확장 (rawq 4000자 budget 내에서 5개×300자 = 1500자 여유 있음)

### Phase 2: Compression 품질 개선

#### Step 2.1: section 유형별 압축 목표 조정

`compression.rs`의 `maybe_compress_section`을 `maybe_compress_section_with_hint`로 확장:

| section 유형 | 압축 목표 | 보존 우선순위 |
|---|---|---|
| context (대화) | 800자 | 최근 발화, 결정 사항 |
| cross-session | 600자 | 세션 구분, 핵심 결론 |
| findings | 400자 | 발견 사항, 권고 |
| plan | 원본 유지 | (보통 짧음) |
| rawq | truncate만 | (Claude 호출 불필요) |

Claude 요약 시 hint를 prompt에 포함:
```
Compress this {section_type} section to under {target} characters.
Preserve: {priority_items}.
```

#### Step 2.2: 다중 항목 섹션 항목별 압축

cross-session과 context summary는 각 항목(세션/메시지)이 독립적이므로, 전체를 한 번에 압축하지 않고:
1. 각 항목의 길이 확인
2. budget을 항목 수로 균등 분배
3. budget 초과 항목만 개별 truncate (Claude 호출 없이)
4. 전체가 여전히 limit 초과 시에만 Claude 압축

이 방식은 Claude 호출 횟수를 줄이면서 정보 보존율을 높인다.

## 수정 파일

| 파일 | 변경 |
|---|---|
| `src-tauri/src/agents/rawq.rs` | SearchResult에 scope, confidence 필드 추가, parse_json 수정 |
| `src-tauri/src/commands/agents_helpers/context_pack.rs` | build_rawq_section 후처리 (dedup, re-rank, snippet 확장, confidence 표시) |
| `src-tauri/src/commands/agents_helpers/compression.rs` | section 유형별 압축 목표, 다중 항목 균등 분배 |

## 검증

1. `cargo check && cargo test --lib`
2. rawq가 있는 프로젝트에서 코드 관련 질문 → trace에서 rawq section 확인
3. 긴 대화(10+ 메시지)에서 context section compression 확인
4. cross-session 3개+ 활성 → 항목별 분배 vs 전체 압축 비교

## 범위 밖

- entroly/외부 retrieval 시스템 도입
- context budget slider UI
- dynamic budget re-allocation (빈 섹션 → 다른 섹션에 재배분)
- rawq 임베딩 모델 변경
