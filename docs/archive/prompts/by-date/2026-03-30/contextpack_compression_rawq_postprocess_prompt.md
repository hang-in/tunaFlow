# ContextPack Compression + rawq Post-Processing Prompt

> 실행 프롬프트 — `contextPackCompressionAndRawqPostprocessPlan_2026-03-30.md` 참조

## 지시

ContextPack의 rawq 후처리와 compression 품질을 개선하라. Phase 1 (rawq)부터 순서대로 진행.

### Phase 1: rawq post-processing

1. `SearchResult`에 `scope: Option<String>`, `confidence: f64` 필드 추가
2. `parse_json()`에서 scope, confidence를 SearchResult에 채움
3. `build_rawq_section()`에서 후처리:
   - 같은 file + ±5줄 이내 결과 병합 (높은 confidence 유지)
   - confidence < 0.4 결과 제거
   - confidence 내림차순 정렬
   - snippet을 300자로 확장
   - `(scope, 91%)` 형태로 메타 표시

### Phase 2: Compression 개선

1. `maybe_compress_section`에 `section_hint: Option<&str>` 파라미터 추가
2. hint에 따라 압축 목표 크기와 보존 우선순위를 Claude prompt에 포함
3. cross-session, context summary: 항목별 budget 균등 분배 후 초과 항목만 truncate, 전체 초과 시에만 Claude 압축

### 제약

- 새 파일 생성 불가
- rawq.rs, context_pack.rs, compression.rs만 수정
- Claude 압축 호출 빈도를 늘리지 않는 방향
- 기존 테스트 깨지지 않아야 함
