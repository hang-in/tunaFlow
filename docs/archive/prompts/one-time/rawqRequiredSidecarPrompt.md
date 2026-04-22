# tunaFlow rawq 필수 sidecar 전환 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28

```md
# tunaFlow rawq 필수 sidecar 전환

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/rawqRequiredSidecarPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/how-to/rawq-setup.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/rawqAutomationPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/src-tauri/src/agents/rawq.rs`

가능하면 추가 참고:
- tunaDish의 rawq vendor/submodule 구조
- tunaDish의 sidecar bundle 방식

이번 작업의 전제는 하나다.

`rawq`는 선택 기능이 아니다.
`tunaFlow`가 코드 컨텍스트를 확보하기 위해 함께 동작해야 하는 **필수 런타임 의존성**이다.

따라서 이번 작업에서는 기존의 "rawq가 없으면 fallback" 전제를 버리고,
`tunaDish`처럼 rawq를 app-managed sidecar로 준비하는 방향으로 정리하라.

중요:
- 추측 금지
- 실제 코드 기준으로만 수정
- rawq 내부 검색 로직을 재구현하지 말 것
- 기존 adapter 구조는 최대한 유지
- 대규모 무관 리팩토링 금지
- 문서와 코드의 전제를 일치시킬 것

## 먼저 확인할 파일

- `/Users/d9ng/privateProject/tunaFlow/src-tauri/src/agents/rawq.rs`
- `/Users/d9ng/privateProject/tunaFlow/src-tauri/src/commands/projects.rs`
- `/Users/d9ng/privateProject/tunaFlow/src-tauri/tauri.conf.json`
- `/Users/d9ng/privateProject/tunaFlow/docs/how-to/rawq-setup.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/rawqIntegrationPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/rawqAutomationPlan.md`

## 이번 단계 목표

1. rawq를 필수 의존성으로 다루는 정책 확정
2. binary resolution 우선순위 정리
3. sidecar/vendor 도입에 필요한 실제 코드 변경 범위 확정
4. fallback 제거 또는 dev-only로 축소
5. 문서 정합성 복구

## 구현/정리 원칙

### 1. 기본 전략

권장 방향:
- 배포: sidecar bundle
- 개발: vendor/submodule build
- 런타임: 준비된 binary를 사용

기본 경로로 두지 말 것:
- 앱 시작 시 매번 Cargo build
- rawq 실패 시 silent fallback

### 2. binary resolution

권장 우선순위:
1. `RAWQ_BIN`
2. bundled sidecar path
3. 개발용 local/vendor build path
4. PATH (`dev` 보조 수단)

### 3. 상태 처리

rawq가 없으면:
- 조용히 degraded mode로 가지 말 것
- 명확한 에러/상태를 반환할 것
- 사용자가 어떤 준비가 필요한지 알 수 있어야 함

### 4. 이번 단계에서 할 수 있는 것

- 문서 수정
- 계획 문서 정리
- path resolution 정리
- sidecar bundle을 위한 설정/구조 추가
- 개발용 bootstrap 스크립트 추가

### 5. 이번 단계에서 하지 말 것

- rawq search 엔진 재구현
- 새로운 대형 UI 설계
- code-review-graph 통합
- updater 자동화

## 기대 산출물

최소한 아래 중 해당되는 것을 만들어라.

1. rawq 필수 sidecar 전략 문서
2. rawq 설치/운영 문서 업데이트
3. 필요한 경우 sidecar 경로 탐색 코드 수정
4. 필요한 경우 build/bootstrap script 초안
5. 필요한 경우 tauri sidecar 설정 초안

## 검증

작업 후 반드시 설명할 것:

1. 왜 기존 fallback 전제가 맞지 않는지
2. 왜 "앱 시작 시 매번 즉석 빌드"보다 "sidecar/vendor + 명시적 bootstrap"이 나은지
3. 런타임 binary resolution이 어떻게 바뀌는지
4. 개발 환경과 배포 환경이 각각 어떻게 rawq를 준비하는지
5. 남은 리스크가 무엇인지

## 출력 형식

### A. Decision
### B. Why This Direction
### C. Changes Made
### D. Runtime Resolution Order
### E. Dev vs Release Flow
### F. Remaining Risks

바로 수정까지 진행하라.
```
