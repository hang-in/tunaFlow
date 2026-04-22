# tunaFlow rawq 도입 계획 (초기 계획 — 아카이브)

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 08:26 KST
- **상태: 완료 (Phase 1-2 구현 후 sidecar 전환으로 대체)**
- **현재 기준 문서: [`rawqRequiredSidecarPlan.md`](./rawqRequiredSidecarPlan.md)**

> **주의**: 이 문서는 rawq 최초 도입 당시의 계획이다.
> 당시 전제는 "rawq가 없으면 graceful fallback"이었으나,
> 이후 rawq를 필수 런타임 의존성으로 전환하면서 이 전제는 폐기되었다.
> 현재 rawq 운영 방침은 `rawqRequiredSidecarPlan.md`를 따른다.

## 목적

`tunaFlow`의 현재 코드 검색은 이름만 rawq이고, 실제로는 요청 시점에 프로젝트 파일을 순회하는 최소 키워드 검색이다. 이 문서는 `tunaDish`에서 이미 사용 중인 rawq CLI 기반 검색/인덱싱 구조를 참고해, `tunaFlow`에서도 실제 rawq를 사용할 수 있도록 단계별 도입 방향을 정리한다.

## 현재 상태 확인

실제 코드 기준:

- `src-tauri/src/agents/rawq.rs`
  - 주석에 `Minimal rawq: keyword-based code file search`라고 명시되어 있다.
  - persistent index, daemon, semantic search, map 기능이 없다.
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
  - `build_rawq_section()`이 `rawq::search(path, prompt, RAWQ_MAX_RESULTS)`를 호출한다.
  - 현재 결과는 `## Code context` 텍스트 블록으로만 주입된다.
- `tunaDish`
  - `docs/prompts/integration/rawq_integration.md`
  - `docs/prompts/feature/rawq-agent-enhancement.md`
  - `docs/prompts/feature/rawq-scoped-indexing.md`
  - 위 문서들과 `vendor/rawq`, 브리지 구조를 보면 CLI 브리지, 인덱스 상태 확인, map, 자동 인덱싱, graceful fallback이 이미 정리되어 있다.

결론:

- `tunaFlow`에는 rawq 섹션이 있긴 하지만, 아직 `tunaDish` 수준의 rawq 도입은 아니다.
- 현재 상태는 "간이 코드 검색"이며, 실제 rawq CLI 통합은 미도입 상태다.

## 목표 상태 (초기 계획 당시)

`tunaFlow`에서 rawq를 다음 수준으로 사용한다.

1. rawq CLI 사용 가능 여부를 감지한다. — **완료**
2. 프로젝트별 인덱스 상태를 확인할 수 있다. — **완료**
3. Claude 경로의 `ContextPack`에서 실제 rawq 검색 결과를 사용한다. — **완료**
4. ~~rawq가 없거나 실패하면 현재의 최소 키워드 검색으로 안전하게 fallback 한다.~~ — **폐기**: rawq는 필수 의존성으로 전환됨. fallback 없음.
5. 이후 단계에서 code map, 수동 검색, 상태 표시까지 확장할 수 있다.

## 설계 원칙

- 기존 `ContextPack` 조립 구조를 유지한다.
- rawq는 필수 런타임 의존성이다. sidecar bundle 또는 명시적 bootstrap으로 준비한다.
- rawq 미존재 시 silent fallback 하지 않고 명확한 에러/상태를 반환한다.
- 초기 단계에서는 `Claude` 경로 우선 적용으로 충분하다.
- 프로젝트 전체를 무조건 인덱싱하지 말고, 현재 활성 프로젝트 경로만 대상으로 한다.

> **참고**: 이 계획은 Phase 1-2 구현 완료 시점의 문서이다. rawq의 필수 sidecar 전환은 `rawqRequiredSidecarPlan.md`를 참조.

## 단계별 계획

### Phase 1. CLI 브리지 + graceful fallback — **완료**

> 초기 계획 당시 전제: rawq가 없으면 기존 최소 검색으로 fallback.
> 현재 전제: rawq는 sidecar로 번들되며, fallback 없이 명시적 에러 반환.

구현 완료 사항:

- `agents/rawq.rs`: rawq CLI wrapper (search, index status/build)
- binary resolution 4단계 (RAWQ_BIN → sidecar → local build → PATH)
- `rawq search --json` + 5초 타임아웃
- JSON 파싱 실패 시 `RawqError` 반환 (silent fallback 아님)

### Phase 2. 프로젝트별 인덱스 상태/빌드

목표:

- 활성 프로젝트 기준으로 `rawq index status`
- 필요 시 `rawq index build`
- 인덱싱은 현재 프로젝트만 수행

구현 포인트:

- Tauri command 추가 예:
  - `getRawqStatus`
  - `buildRawqIndex`
- rawq 상태를 `ContextPanel` 또는 작은 상태 UI로 표시
- 인덱스 빌드는 수동 트리거 우선

완료 기준:

- 사용자가 현재 프로젝트 rawq 상태를 볼 수 있다.
- 수동 인덱스 빌드가 가능하다.

### Phase 3. Code Map / 수동 검색 UX

목표:

- `rawq map`
- 수동 `code search`
- 검색 결과를 `ContextPanel`에서 확인

구현 포인트:

- `tunaDish`의 ContextPanel 구조 참고
- 결과를 별도 탭 또는 기존 context 영역에 표시
- 자동 주입과 수동 탐색을 분리

완료 기준:

- 사용자가 rawq 결과를 직접 조회할 수 있다.
- 구조 파악용 code map을 볼 수 있다.

### Phase 4. 협업 기능과 연결

목표:

- Follow-up / plan / artifact 흐름과 rawq를 느슨하게 연결

예:

- follow-up 시 source + rawq 검색 결과를 함께 넘김
- 특정 subtask 문맥에서 rawq 검색어 보정

이 단계는 후순위다.

## tunaDish에서 직접 참고할 문서

- `D:\privateProject\tunaDish\docs\prompts\integration\rawq_integration.md`
- `D:\privateProject\tunaDish\docs\prompts\feature\rawq-agent-enhancement.md`
- `D:\privateProject\tunaDish\docs\prompts\feature\rawq-scoped-indexing.md`

## tunaFlow에서 우선 수정될 가능성이 큰 위치

- `src-tauri/src/agents/rawq.rs`
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents.rs`
- 향후:
  - `src/components/tunaflow/context-panel/*`
  - `src/lib/api/*`

## 추천 순서 (초기 계획 당시)

1. ~~Phase 1만 먼저 구현~~ — 완료
2. ~~실제 검색 품질 확인~~ — 완료
3. ~~그 다음 Phase 2로 상태/인덱싱 추가~~ — 완료
4. 마지막에 map/UI 확장 — 후순위

## 현재 상태 (2026-03-28)

이 계획의 Phase 1-2는 구현 완료되었다.
이후 rawq를 필수 런타임 의존성으로 격상하면서, 운영 방침은 `rawqRequiredSidecarPlan.md`로 이관되었다.
Phase 3-4(code map, 수동 검색, 협업 연결)는 후순위로 유지된다.
