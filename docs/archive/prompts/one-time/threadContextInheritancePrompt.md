# tunaFlow Thread / RT Context Inheritance 구현 프롬프트

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\threadContextInheritancePlan.md`

이번 작업 목표는:
thread와 RT가 같은 프로젝트 안에서 더 자연스럽게 이어지도록,
**기존 ContextPack 위에 parent/local context inheritance 레이어를 추가**하는 것이다.

중요:
- 실제 코드 기준으로만 작업
- 기존 ContextPack 구조를 최대한 재사용할 것
- 전체 대화 히스토리를 통째로 넣지 말 것
- 이번 단계는 Phase 1까지만
- 모든 응답과 보고는 한국어로만 작성하라

---

# 목표

최소한 아래를 만족하라.

1. 일반 thread 시작 시 parent anchor message가 기본 포함
2. RT 시작 시 parent anchor message 또는 explicit source가 기본 포함
3. 최근 local turns 2~3개가 최소 상속된다
4. 기존 project/context/rawq 흐름은 유지

---

# 먼저 확인할 파일

### 백엔드
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\branches.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\roundtable.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents_helpers\context_pack.rs`

### 프론트
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- `D:\privateProject\tunaFlow\src\components\tunaflow\ChatPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\BranchThreadPanel.tsx`

---

# 구현 요구사항

## 1. inheritance source 정의

이번 단계에서 우선 지원할 source:

- parent anchor message
- explicit selected source (artifact / plan / message)
- recent local turns 2~3개

중요:
- explicit source가 있으면 anchor보다 우선
- anchor는 recent turns보다 우선

## 2. 일반 thread

일반 thread를 열거나 thread 메시지를 보낼 때,
기존 project context와 함께 아래를 포함하라.

- parent message 본문
- 최근 local turns 2~3개

## 3. RT

RT를 시작하거나 follow-up 할 때,
기존 project context와 함께 아래를 포함하라.

- explicit source 또는 parent message
- 간단한 recent turns 1~2개
- 왜 이 RT가 열렸는지에 대한 짧은 instruction

## 4. 전체 대화 히스토리 금지

중요:
- parent conversation 전체 메시지를 통째로 넣지 말 것
- inheritance는 압축된 최소 local context만

## 5. 구현 방식

권장:

- `context_pack.rs` 쪽에 inheritance section helper 추가
- RT / thread send 경로에서 해당 section을 조립

중요:
- 새 거대한 context system을 만들지 말고 기존 조립 구조에 최소 추가

---

# 하지 말 것

- 전체 히스토리 자동 상속
- 새 메모리 시스템 전면 도입
- sidecar
- docs 작업 같이 하기

---

# 검증

작업 후 반드시 아래를 설명하라.

1. thread와 RT 각각 어떤 local context를 상속하는지
2. explicit source / anchor / recent turns 우선순위를 어떻게 정했는지
3. project context와 충돌 없이 어떻게 합쳤는지
4. prompt가 과도하게 커지지 않도록 어떤 제한을 둔 것인지
5. 타입체크/빌드/가능한 검증 결과
6. 남은 리스크

---

# 출력 형식

### A. Changes Made
### B. Files Modified
### C. Thread / RT Inheritance Flow
### D. Verification
### E. Remaining Risks

바로 실제 코드 수정까지 진행하라.
