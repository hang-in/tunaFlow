# 외부 도구 분석 및 tunaFlow 리팩토링 방향

- 작성: 2026-03-30
- 최종 수정: 2026-03-30 (실제 코드 대조 검토 반영)
- 목적: `_research/_util/` 레포 분석 결과 + tunaFlow 개선 방향 통합 정리
- 관련 문서: `docs/plans/contextPackAlgorithmImprovementsPlan.md`

---

## 1. 분석 대상 레포 종합

### 1.1 평가 기준

모든 레포를 동일 기준으로 분석:
- README 주장 vs 실제 구현 일치 여부
- 코드 품질 (수작성 vs LLM 생성, 에러 핸들링, 테스트)
- tunaFlow ContextPack 파이프라인에 대한 실질적 기여 가능성

### 1.2 종합 비교표

| 레포 | 언어 | 코드량 | 종합 점수 | README 정직도 | 실사용 가능 | tunaFlow 레퍼런스 가치 |
|------|------|--------|----------|-------------|-----------|---------------------|
| **opendev** | Rust | ~35K줄 (21 crate) | 8/10 | 7/10 | 8/10 | ⭐⭐⭐⭐ |
| **tunaFlow** | Rust+TS | ~22K줄 | 6.5/10 | 9/10 | 7/10 | — (본인) |
| **claw-compactor** | Python | 30K줄 | 5/10 | 3/10 | 5/10 | ⭐⭐ |
| **entroly** | Rust+Python | 36K줄 | 4.5/10 | 4/10 | 3/10 | ⭐⭐ |
| **code-review-graph** | Python | 10K줄 | 미분석(상세) | — | — | ⭐⭐⭐ |
| **context-hub** | JavaScript | 7K줄 | 실사용 중 | — | 7/10 | ⭐⭐⭐ |
| **chops** | Swift | 11K줄 | 프로덕션 | — | 8/10 | ⭐ |

나머지 (agentscope, takopi, takopi_swarm, takopi-discord, takopi-slack)는 ContextPack 퍼포먼스와 무관하여 제외.

---

## 2. 개별 레포 분석 요약

### 2.1 entroly (github.com/juyterman1000/entroly)

**정체:** LLM 토큰 컨텍스트 최적화 엔진 (Rust core 22K줄 + Python 14K줄)

**핵심 발견:**
- 22일간 1인 개발, 36K줄 — LLM 대량 생성이 거의 확실
- 알고리즘 수학은 올바름 (KKT 이분탐색, Shannon/Renyi 엔트로피, PRISM spectral gradient, Nash-KKT)
- GitHub 스타 87, 이슈 0 — 실사용자 부재
- 수치 주장(78% 토큰 절감, <10ms) 검증 불가 — 벤치마크 결과 데이터 없음
- SAST 규칙 "55개" → 실제 54개 (소폭 과장)
- Pitman-Yor 프로세스 → 실제로는 단순화된 CRP (알고리즘 이름 과장)

**tunaFlow에 참고할 알고리즘:**
- `entropy.rs` L26-49: Shannon 바이트 엔트로피 — rawq 결과 정보 밀도 스코어링
- `skeleton.rs`: 코드 시그니처 추출 — rawq 다해상도 출력
- `hierarchical.rs` L54-78: 3레벨 표현 (스켈레톤맵/의존클러스터/풀콘텐츠) — 개념 참고

**채택하지 않을 것:**
- KKT 냅색 솔버 — 섹션 7개에 과잉
- PRISM RL 학습 — 피드백 루프 인프라 없음
- Nash-KKT 멀티에이전트 예산 — 공유 예산 개념 불필요
- 전체를 의존성으로 추가 — 검증 안 된 LLM 생성 코드

### 2.2 claw-compactor (github.com/open-compress/claw-compactor)

**정체:** 14-stage Fusion Pipeline LLM 토큰 압축 엔진 (Python 30K줄)

**핵심 발견:**
- GitHub 스타 2,126, 이슈 0 — **스타가 인위적일 가능성 높음**
- 파이프라인 아키텍처(불변 데이터 흐름 + gate-before-compress)는 진짜 잘 설계됨
- LLMLingua-2 비교 벤치마크(ROUGE-L 0.653/0.723) — 코드에서 검증 불가, 출처 불명
- 벤치마크 자기 모순: RuleCompressor 11.8% 압축(안전), Engram 87.8% 압축(ROUGE-L 0.028 = 콘텐츠 파괴)
- "SimHash" → 실제로는 Jaccard similarity (알고리즘 이름 오류)
- Nexus ML 모델 → 랜덤 가중치(미학습)
- "16 languages" → content_detector에 10개, neurosyntax tree-sitter에 8개

**tunaFlow에 참고할 알고리즘:**
- `semantic_dedup.py` L91-177: 3-word shingle Jaccard — cross-session 중복 제거
- `tokenizer_optimizer.py` L40-78: 마크다운 포맷 경량화 (bold/italic 제거, 공백 정규화)
- `structural_collapse.py` L42-60: import 블록 접기 — rawq 코드 스니펫 압축
- `quantum_lock.py`: 시스템 프롬프트 동적 값 분리 — KV-cache 최적화 아이디어
- `FusionStage` + `FusionPipeline` 아키텍처 — 불변 데이터 흐름 패턴

**채택하지 않을 것:**
- Nexus ML 토큰 분류 — 미학습 모델
- Ionizer JSON 샘플링 — ContextPack에 대규모 JSON 없음
- Photon 이미지 압축 — tunaFlow는 텍스트 전용
- Engram LLM 요약 — compression.rs의 Claude 호출과 동일, 제거 대상
- 전체를 의존성으로 추가 — 마케팅 레이어 + 미완성 ML 딸려옴

### 2.3 opendev (github.com/opendev-to/opendev)

**정체:** 오픈소스 자율 코딩 에이전트 (Rust 35K줄, 21 crate)

**핵심 발견:**
- **이 대화에서 분석한 모든 레포 중 코드 품질 최고**
- 21 crate 모듈화, ReAct 루프 페이즈 추출, trait 기반 설계
- Compound AI: 워크플로우별 모델 바인딩 (Normal/Thinking/Compact/Critique/VLM) 실제 구현
- Doom loop 감지: 슬라이딩 윈도우 + 1-3길이 사이클 탐지 + 3단계 에스컬레이션 (Redirect→Notify→ForceStop)
- 단계적 컨텍스트 압축: 70/80/85/90/99% 5단계 점진적 대응
- 테스트 비율 37%, 핵심 로직 커버, trivial 아님
- 벤치마크 방법론 투명 (hyperfine, `/usr/bin/time -l`)
- arxiv 논문 주장과 코드 일치

**tunaFlow에 참고할 패턴:**
- `doom_loop.rs`: 라운드테이블 반복 응답 감지 — 직접 포팅 가치 ⭐⭐⭐⭐
- `compaction/`: 5단계 점진적 컨텍스트 압축 — ContextPack 개선 레퍼런스 ⭐⭐⭐
- `BaseAgent` trait: 엔진 추상화 패턴 — agents.rs 중복 제거 ⭐⭐⭐⭐
- `skills/discovery.rs`: 스킬 자동 발견 + 지연 로딩 — 스킬 시스템 개선 ⭐⭐⭐
- `react_loop/phases/`: 실행 로직 페이즈 추출 — 테스트 가능성 향상 ⭐⭐⭐

**tunaFlow와의 근본적 차이:**
- OpenDev = "에이전트 자체" (직접 LLM API 호출, 도구 실행)
- tunaFlow = "에이전트 오케스트레이터" (CLI 에이전트에 위임)
- OpenDev의 아키텍처를 복사하는 게 아니라, 엔지니어링 품질 패턴을 tunaFlow 모델 안에서 적용해야 함

### 2.4 code-review-graph

**정체:** Tree-sitter 기반 코드 지식 그래프 (Python 10K줄, 18 언어 지원)

**핵심 가치:**
- 구조적 의존성 추적 (호출자, import, 테스트 관계)
- rawq(시맨틱 검색)와 보완 관계 — rawq가 "비슷한 코드", CRG가 "관련된 코드"
- blast radius 100% recall — 변경 영향 분석
- 22 MCP 도구 + 5 MCP 프롬프트

**적용 시점:** P1-P4 개선 이후, rawq 결과의 구조적 확장이 필요해질 때.

### 2.5 context-hub

**정체:** 에이전트용 문서/스킬 MCP 서버 (JavaScript 7K줄)

**핵심 가치:**
- BM25 풀텍스트 + 퍼지 검색
- 600+ 패키지 문서 라이브러리
- 에이전트 어노테이션 + 피드백 루프
- 이미 tunaFlow 통합 계획 존재 (`contextHubSidecarIntegrationPlan`)

### 2.6 chops

**정체:** macOS 네이티브 스킬 관리 앱 (Swift 11K줄)

**핵심 가치:**
- 8개 도구(Claude Code, Cursor, Windsurf 등) 스킬 디렉토리 통합 스캔
- symlink 해제 중복 감지
- FSEvents 실시간 변경 감지
- frontmatter 파싱 (SKILL.md, .mdc)

**tunaFlow 적용:** chops 자체를 의존성으로 넣는 건 아님 (Swift↔Rust 언어 불일치). 스킬 스캔 로직(~150줄)을 Rust로 포팅하여 `skills.rs`에 통합. 스킬이 100개+ 규모가 되면 필요.

---

## 3. tunaFlow 현재 구조적 문제

### 3.1 Rust 백엔드 (10,878줄) — B+

**강점:**
- 에이전트 어댑터 (claude.rs, codex.rs 등)가 프로페셔널
- 방어적 프로그래밍 (UTF-8 char boundary safe truncation 일관 적용)
- 의존성 미니멀 (tauri, rusqlite, serde, uuid, tempfile, thiserror)
- OTel 스타일 트레이싱

**구조적 문제:**

| 문제 | 위치 | 심각도 |
|------|------|--------|
| `agents.rs` 8개 send 함수가 90% 동일 | L79-1152 | **높음** |
| DB lock contention — 컨텍스트 로딩 중 write lock 보유 | agents.rs L86-166 | 중간 |
| ContextPack 섹션이 DB 직접 호출 — 단위 테스트 불가 | context_pack.rs 전체 | 중간 |
| compression.rs Claude 호출 — LLM 비용 + 1-3초 지연 | compression.rs L14-33 | 중간 |
| guardrail.rs 예산 하드코딩 | guardrail.rs L9-20 | 중간 |
| doom loop 감지 없음 | — | 중간 |

### 3.2 React 프론트엔드 (10,787줄) — C+

**구조적 문제:**

| 문제 | 위치 | 심각도 |
|------|------|--------|
| runtimeSlice.ts send 함수 4벌 + branchSlice.ts thread 버전 4벌 = 8벌 중복 | runtimeSlice L88-344, branchSlice L292-419 | **높음** |
| SettingsPanel.tsx 904줄 — 분할 필요 | SettingsPanel.tsx | 중간 |
| 테스트 8개 전부 smoke — 실질적 커버리지 0% | tests/ | 중간 |
| deprecated `isRunning` 미정리 | types.ts L67 | 낮음 |
| 이벤트 리스너 cleanup 취약 | projectSlice.ts L99-112 | 낮음 |

---

## 4. 리팩토링 방향

### 4.1 원칙

1. **OpenDev의 아키텍처를 복사하지 않는다.** tunaFlow는 오케스트레이터 IDE이지 자율 에이전트가 아니다.
2. **OpenDev의 엔지니어링 품질 패턴을 적용한다.** trait 추상화, 페이즈 추출, doom loop, staged compaction.
3. **외부 의존성 0.** entroly, claw-compactor, opendev의 알고리즘을 참고하되 코드는 직접 작성.
4. **순감소 리팩토링.** 코드를 추가하는 것보다 중복을 제거하는 것이 우선.

### 4.1.1 실제 코드 대조 검토에서 확인된 보정 사항

초기 분석 후 tunaFlow 실제 코드와 OpenDev 구현을 대조한 검토에서 3가지 보정이 필요했다:

1. **Doom loop → multi-round RT 선행 필요.** tunaFlow RT는 현재 `rounds` 파라미터를 무시하고 1회만 실행한다. doom loop 감지가 의미 있으려면 multi-round RT 구현이 선행돼야 한다. 단일 라운드 내 참가자 간 반복 감지만 현재 가능. → 우선순위 P1에서 **P2로 하향**.

2. **OpenDev compaction ≠ tunaFlow compression.** OpenDev의 staged compaction은 ReAct 루프 내 도구 출력 masking/pruning (실행 중 컨텍스트 관리). tunaFlow compression은 대화 장기 기억 요약 (ContextPack 어셈블리 시). 적용 범위가 다르므로 직접 포팅이 아니라 tunaFlow ContextPack 섹션에 맞는 규칙을 설계해야 한다.

3. **스킬 자동 매칭보다 선택적 주입이 급하다.** 스킬 6개 활성화하면 skills 섹션만 8,000자. 매칭을 잘해봐야 context budget 초과하면 무의미. 스킬 전체 주입이 아닌 관련 섹션만 발췌하는 것이 선행 과제.

### 4.2 Phase 1: Engine trait 추상화 — 중복 제거

**문제:** agents.rs 8개 함수가 90% 동일
**레퍼런스:** OpenDev `BaseAgent` trait

```
변경 전:
  agents.rs (1,152줄)
    send_with_claude()           ~150줄
    stream_with_claude()         ~200줄
    send_with_codex()            ~100줄
    start_codex_stream()         ~100줄
    send_with_gemini()           ~100줄
    stream_with_gemini()         ~150줄
    send_with_opencode()         ~100줄
    start_opencode_stream()      ~100줄

변경 후:
  agents/engine.rs (신규, ~80줄)
    Engine trait { run(), stream_run(), supports_streaming() }
    EngineInput, EngineOutput 구조체

  agents.rs (~300줄)
    execute_with_engine() — 공통 흐름 1개
    stream_with_engine() — 스트리밍 공통 흐름 1개

예상: ~200줄 신규, ~850줄 삭제
```

프론트엔드도 동일 패턴 적용:
```
변경 전:
  runtimeSlice.ts: sendMessage, sendWithCodex, sendWithGemini, sendWithOpencode (각 ~80줄)
  branchSlice.ts: 동일 4개 thread 버전

변경 후:
  runtimeSlice.ts: createSendFn(engine, command) 팩토리 → 각 엔진은 1줄 호출
  branchSlice.ts: createThreadSendFn() 동일 패턴

예상: ~150줄 절약
```

### 4.3 Phase 2: ContextPack 모듈 분리 + 알고리즘 개선

**문제:** ContextPack 로직이 DB, 에이전트, 압축에 걸쳐 분산. 단위 테스트 불가.
**레퍼런스:** OpenDev `opendev-context` crate, entroly/claw 알고리즘

```
변경 전:
  commands/agents_helpers/context_pack.rs   (750줄, DB 직접 호출)
  commands/agents_helpers/compression.rs    (104줄, Claude 서브프로세스)
  commands/agents_helpers/send_common.rs    (510줄, 혼합)
  guardrail.rs                              (90줄, 하드코딩)

변경 후:
  context/
    mod.rs              — ContextPack 조립 엔트리포인트
    sections.rs         — 개별 섹션 빌더 (순수 함수, DB 의존 제거)
    budget.rs           — 우선순위 비례 동적 예산 배분 (P2)
    reduction.rs        — 규칙 기반 텍스트 축소 (P1)
                           - 반복 턴 접기 (Jaccard shingle, claw 참고)
                           - 마크다운 경량화 (claw tokenizer_optimizer 참고)
                           - import 블록 접기 (claw structural_collapse 참고)
    compression.rs      — Claude fallback (reduction 실패 시만)
    guardrail.rs        — 상수 + 트렁케이션 유틸
```

포함되는 알고리즘 개선 (contextPackAlgorithmImprovementsPlan.md 참조):

| ID | 내용 | 레퍼런스 | 구현량 |
|----|------|---------|--------|
| P1 | 규칙 기반 텍스트 축소 (Claude 호출 제거) | claw structural_collapse, tokenizer_optimizer, semantic_dedup | ~120줄 |
| P2 | 섹션 우선순위 동적 예산 배분 | entroly knapsack (개념만) | ~80줄 |
| P3 | rawq 정보 밀도 스코어링 | entroly entropy.rs | ~45줄 |
| P4 | rawq 다해상도 출력 | entroly skeleton.rs, hierarchical.rs | ~60줄 |
| P5 | cross-session 중복 제거 | claw semantic_dedup.py | ~30줄 |

### 4.4 Phase 3: 단계적 컨텍스트 관리 (Phase 2에 통합)

> **보정:** Phase 4로 분리했으나 Phase 2 reduction.rs 구현에 포함하는 것이 자연스럽다.

**문제:** 섹션 초과 시 "바로 Claude 호출 or truncate" 2단계뿐
**레퍼런스:** OpenDev compaction 5단계 점진적 대응 (개념 참고. 적용 범위는 다름 — §4.1.1 참조)

```
현재:  초과 → Claude 호출 → 실패 시 truncate

개선 (reduction.rs 단계적 파이프라인):
  1단계: strip_markdown_formatting()        비용 0, 지연 0
  2단계: fold_similar_turns()               비용 0, 지연 0
  3단계: collapse_imports()                 비용 0, 지연 0
  4단계: compress_context_with_claude()     비용 있음, 1-3초
  5단계: truncate_section()                 최후 수단
```

추가 적용 가능한 OpenDev 패턴 (Stage 2-3 수준):
- 오래된 assistant 메시지를 `[ref: 메시지 요약]`으로 마스킹 — LLM 호출 없이 context 절감
- 긴 assistant 응답의 코드 블록 → 시그니처만, 에러 → 마지막 3줄만
- compressed memory에 artifact 참조 자동 삽입

### 4.5 Phase 3: Doom loop 감지 (multi-round RT 이후)

> **보정:** 초기 P1에서 P3으로 하향. tunaFlow RT가 multi-round를 지원한 뒤에 의미 있음.
> 현재 구조에서는 단일 라운드 내 참가자 간 응답 유사도 체크만 가능.

**문제:** 라운드테이블에서 에이전트 반복 응답 방지 수단 없음
**선행 조건:** RT multi-round 구현 (현재 `rounds` 파라미터 무시됨)
**레퍼런스:** OpenDev `doom_loop.rs` (206줄)

```
신규:
  context/doom_loop.rs (~80-120줄)
    DoomLoopDetector 구조체
    - VecDeque(20) 슬라이딩 윈도우
    - "참가자 응답 내용 해시"로 fingerprint (OpenDev는 도구 호출 해시 — 구조가 다름)
    - 1~3 길이 사이클 탐지, 3회 반복 = 임계값
    - 2단계 에스컬레이션: Redirect(경고 주입) → ForceStop(라운드 종료)
      (OpenDev의 3단계 중 Notify는 tunaFlow RT에서 불필요하므로 생략)

적용 위치:
  roundtable_helpers/executor.rs — execute_sequential/execute_parallel 내 라운드 간 체크
  ForceStop 시 해당 라운드 종료 + 사유를 roundtable_brief에 기록
```

### 4.6 Phase 4: 스킬 선택적 주입 + 자동 매칭

> **보정:** 자동 매칭보다 선택적 주입이 선행. 스킬 6개 활성화 시 8,000자 소모가 실제 병목.

**문제 1 (급함):** 스킬 전체 내용을 주입하면 context budget 잠식
**문제 2 (추후):** 스킬은 사용자 수동 토글만 가능

```
Phase 4a — 스킬 선택적 주입 (~50줄):
  build_skills_section()에서 스킬 전문 대신 프롬프트와 관련된 섹션만 발췌
  - 스킬 내용을 ## 헤더 기준으로 분할
  - 프롬프트 키워드와 헤더/내용 매칭
  - 매칭된 섹션만 주입, 나머지는 "[스킬 이름: N개 추가 섹션 생략]" 참조

Phase 4b — 자동 매칭 (~40줄):
  skills.rs에 auto_select_skills() 추가
  - 프롬프트 키워드 ↔ 스킬 name/description 매칭

Phase 4c — 멀티도구 스캔 (~150줄, chops 로직 포팅):
  skills.rs에 scan_all_tool_skills() 추가
  - ~/.claude/skills/, ~/.cursor/skills/ 등 8개 도구 통합 스캔
  - symlink 해제 중복 감지
  - 스킬 100개+ 규모일 때 활성화

레퍼런스:
  - OpenDev skills/discovery.rs — priority chain (project > user > builtin)
  - OpenDev — 세션 중복 방지 (이미 주입된 스킬은 요약 참조로 대체)
  - chops SkillScanner.swift — 멀티도구 디렉토리 스캔
```

### 4.7 Phase 5 (추후): code-review-graph 통합

**문제:** rawq가 "auth.py"를 찾아도 관련 파일(auth_config.py, test_auth.py) 포함 안 됨
**레퍼런스:** code-review-graph (Tree-sitter 지식 그래프)

rawq 결과에 CRG의 1-hop 의존성(호출자, import, 테스트)을 확장하는 sidecar 통합. Phase 1-3 효과 측정 후 검토.

---

## 5. 리팩토링 우선순위 및 예상 효과 (보정 후)

| 순서 | Phase | 구현량 | 코드 증감 | 핵심 효과 | 선행 조건 |
|------|-------|--------|----------|----------|----------|
| **P1** | Engine trait 추상화 | ~200줄 신규 | **-850줄** | agents.rs 중복 제거, 유지보수성 | 없음 |
| **P1** | ContextPack 모듈 분리 + 규칙 기반 축소 + 단계적 압축 | ~435줄 신규 | ~±0 (이동) | Claude 호출 제거/축소, 동적 예산, 테스트 가능 | 없음 |
| **P2** | 스킬 선택적 주입 (4a) | ~50줄 신규 | +50줄 | 스킬 budget 잠식 해결 | 없음 |
| **P2** | rawq 정보 밀도 + 다해상도 (P3-P4) | ~105줄 신규 | +105줄 | rawq 결과 품질 + 커버리지 | 없음 |
| **P3** | Doom loop 감지 | ~100줄 신규 | +100줄 | RT 반복 응답 방지 | **RT multi-round** |
| **P4** | 스킬 자동 매칭 (4b) + 멀티도구 스캔 (4c) | ~190줄 신규 | +190줄 | 에이전트 응답 품질 향상 | 스킬 100개+ |
| **P5** | CRG 통합 | 미정 | 미정 | rawq 결과 구조적 확장 | P1-P3 효과 측정 |
| **합계 (P1-P2)** | | **~790줄 신규** | **순감소 ~695줄** | | |

---

## 6. 하지 않을 것 (명시적 제외)

| 항목 | 이유 |
|------|------|
| tunaFlow에서 직접 LLM API 호출 | Claude Code/Codex가 이미 하는 일. 재구현은 열화 복제 |
| 21개 crate로 분리 | Tauri 앱은 단일 바이너리. 모듈 분리면 충분 |
| 내장 도구 시스템 구축 | CLI 에이전트의 Read/Write/Bash 대체 이유 없음 |
| ReAct 루프 직접 구현 | 에이전트 CLI가 이미 실행 중 |
| entroly 전체를 의존성으로 추가 | 검증 안 된 LLM 생성 코드 |
| claw-compactor 전체를 의존성으로 추가 | 마케팅 레이어 + 미완성 ML |
| opendev를 의존성으로 추가 | 아키텍처 카테고리가 다름 (자율 에이전트 vs 오케스트레이터) |
| KKT 냅색 솔버 | 섹션 7개에 연속 최적화는 과잉 |
| PRISM RL 가중치 학습 | 피드백 루프 인프라 없음 |
| Nexus ML 토큰 분류 | 미학습 모델 |

---

## 7. 의존성 정책

- 외부 레포를 Cargo/pip/npm 의존성으로 추가하지 않는다
- 알고리즘 아이디어만 참고하고, Rust/TypeScript로 직접 구현한다
- 각 함수는 독립적으로 테스트 가능하게 작성한다
- 기존 동작을 깨뜨리지 않도록 모든 개선은 opt-in 또는 fallback을 유지한다
- 레퍼런스 출처를 코드 주석에 명시한다 (예: `// Ref: opendev doom_loop.rs`)
