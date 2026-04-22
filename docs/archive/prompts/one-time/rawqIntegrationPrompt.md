# tunaFlow rawq 실제 도입 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 08:26 KST

```md
# tunaFlow rawq 실제 도입 (Phase 1 우선)

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\rawqIntegrationPlan.md`
- `D:\privateProject\tunaDish\docs\prompts\integration\rawq_integration.md`
- `D:\privateProject\tunaDish\docs\prompts\feature\rawq-agent-enhancement.md`
- `D:\privateProject\tunaDish\docs\prompts\feature\rawq-scoped-indexing.md`

이번 작업 목표는 하나다.

현재 `tunaFlow`의 rawq는 실제 rawq CLI 통합이 아니라 최소 키워드 파일 검색이다.
이번 단계에서는 `tunaDish`를 참고해 **실제 rawq CLI를 사용할 수 있게 하되**, 실패 시 현재 검색으로 fallback 하도록 구현하라.

중요:
- 추측 금지
- 실제 코드 기준으로만 작업
- 기존 구조 유지
- 대규모 리팩토링 금지
- rawq 미설치 환경에서도 앱이 깨지면 안 됨
- 이번 단계는 Phase 1만 하고 멈출 것

## 먼저 확인할 파일

- `D:\privateProject\tunaFlow\src-tauri\src\agents\rawq.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents_helpers\context_pack.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\guardrail.rs`

참고:
- `D:\privateProject\tunaDish\docs\prompts\integration\rawq_integration.md`
- `D:\privateProject\tunaDish\docs\prompts\feature\rawq-agent-enhancement.md`

## 구현 목표

1. rawq CLI 사용 가능 여부 감지
2. 가능하면 `rawq search ... --json` 사용
3. 실패하거나 미설치면 기존 `agents/rawq.rs` 최소 검색으로 fallback
4. 기존 `build_rawq_section()` 출력 형식은 최대한 유지

## 구현 요구사항

### 1. rawq availability helper

예:
- `is_rawq_available()`
- `run_rawq_search(...)`

허용 방향:
- `rawq --version` 확인
- 또는 `rawq search` 직접 실행 후 실패 감지

중요:
- 매 호출마다 불필요하게 무거운 검사 반복은 피하라
- 하지만 이번 단계에서 캐시는 꼭 필요하지 않다

### 2. CLI search

가능하면 아래와 유사하게 호출:

- `rawq search "<query>" "<path>" --json --top 5`

필요하면:
- `--lang`
- `--exclude`

는 실제 코드 기준으로 최소만 사용하라.

JSON 결과에서 최소한 아래를 추출:
- file/path
- line range 또는 line
- snippet/content 일부

현재 `build_rawq_section()`이 기대하는 형식으로 변환하라.

### 3. fallback

다음 경우에는 반드시 현재 최소 검색으로 fallback:

- rawq 미설치
- 타임아웃
- JSON 파싱 실패
- CLI exit code 실패

중요:
- 본 기능 흐름이 rawq 때문에 깨지면 안 된다
- fallback 경로는 현재 동작과 동일해야 한다

### 4. 범위 제한

이번 단계에서는 하지 말 것:
- rawq index build/status command 추가
- rawq map UI 추가
- ContextPanel 새 탭 추가
- daemon/status UX 추가
- docs 대규모 수정

## 검증

작업 후 반드시 아래를 해라.

1. 현재 rawq가 왜 "실제 rawq 통합"이 아니었는지 설명
2. 어떤 helper를 추가/변경했는지 설명
3. CLI 성공 시 경로와 fallback 경로를 설명
4. rawq 미설치 환경에서 어떻게 graceful fallback 되는지 설명
5. `cargo check` 수행
6. 가능하면 간단한 수동 확인 절차 제시

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. rawq CLI Flow
### D. Fallback Flow
### E. Verification
### F. Remaining Risks

바로 코드 수정까지 진행하라.
```
