# tunaFlow 문서 파일명 규칙

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 목적: 문서 파일명을 짧고 일관되게 유지하면서도, 에이전트와 사람이 문서 성격을 쉽게 판단하게 하는 기준

## 핵심 판단

파일명은 모든 의미를 다 담으려 하면 길어지고 읽기 어려워진다.

따라서 tunaFlow는:

- 파일명 = 짧은 식별자
- 문서 제목 = 사람에게 보이는 긴 설명
- 메타 / index = 현재성 / 관계 / 읽기 순서

이 세 층으로 나눠야 한다.

## 기본 원칙

### 1. 파일명은 짧게

권장:

- 2~4개의 핵심 토큰
- 소문자/카멜/케밥 중 기존 관행 유지

비권장:

- 문장형 이름
- 같은 의미의 단어를 여러 번 반복
- 파일명만 보고 모든 설명이 가능해야 한다는 강박

### 2. 사람에게 보여줄 이름은 문서 내부 `title`

UI나 index나 문서 목록에서
가능하면 파일명보다 문서 내부 제목/요약을 노출하는 방향이 맞다.

즉:

- 파일명 = 저장소 식별자
- title = 사람이 읽는 이름

### 3. 약자/코드는 규칙이 있을 때만 쓴다

무규칙 약자는 금지.

이유:

- `rt`는 runtime / roundtable 둘 다 될 수 있다
- `ap`는 applied / agent profile 둘 다 가능하다

따라서 약자를 쓸 경우
표준 약자표를 먼저 둔다.

## 문서 타입별 파일명 규칙

### Reference

권장:

- 날짜 없는 안정 이름

예:

- `implementationStatus.md`
- `workingRulesForAgents.md`
- `documentVersioningPolicy_2026-03-30.md` 는 예외적 신규 기준 문서

원칙:

- 같은 주제의 current reference는 새 날짜 파일을 계속 만들지 않는다

### Plan

권장 패턴:

- `기능핵심 + Plan + 날짜`

예:

- `runtimeSettingsImplementationPlan_2026-03-30.md`
- `agentProfileChatInputBindingPlan_2026-03-29.md`

허용:

- 작업 단위가 독립적이면 날짜 파일 유지

### Prompt

권장 패턴:

- `기능핵심 + prompt`
- 날짜는 가능하면 폴더로 관리

예:

- `docs/prompts/2026-03-30/runtime_settings_implementation_prompt.md`
- `docs/prompts/2026-03-30/documentation_ia_governance_prompt.md`

원칙:

- prompt는 날짜 폴더 + 짧은 파일명 조합을 우선 사용

### How-to

권장:

- 날짜 없는 안정 이름

예:

- `skills-runtime-policy.md`
- `rawq-setup.md`

### Brainstorm / Review / External reference

권장 패턴:

- 성격 단어 + 주제 + 날짜

예:

- `chatUiVsTunaChatGapReview_2026-03-29.md`
- `tunaflow_references_2026-03-30.md`

원칙:

- reference처럼 보이지 않게 성격 단어를 명확히 넣는다

## 길이 규칙

정확한 글자 수 제한보다 아래를 권장한다.

- 핵심 토큰 3~5개 이내
- 같은 의미의 단어 반복 금지
- `plan`, `prompt`, `review`, `reference` 같은 타입 표시는 한 번만

예:

- 좋음: `runtimeSettingsImplementationPlan_2026-03-30.md`
- 과함: `runtimeSettingsActualImplementationDetailedPlan_2026-03-30.md`

## 약자 표준 (초안)

아직 적극 사용 권장 단계는 아니지만,
쓸 경우 아래만 허용한다.

| 약자 | 의미 |
|---|---|
| `rt` | roundtable |
| `ux` | user experience |
| `ia` | information architecture |
| `md` | markdown |

주의:

- `runtime`은 `rt`로 줄이지 않는다
- `agent profile`은 `ap`로 줄이지 않는다
- 헷갈릴 수 있으면 약자를 쓰지 않는다

## 예시 리라이트

### 현재형

- `agentProfileChatInputBindingPlan_2026-03-29.md`
- `appliedAgentConfigVisibilityPlan_2026-03-29.md`
- `knowledgeSourcesSettingsShellPlan_2026-03-30.md`

### 더 짧은 예시

- `agentProfileInputPlan_2026-03-29.md`
- `agentConfigVisibilityPlan_2026-03-29.md`
- `knowledgeSourcesShellPlan_2026-03-30.md`

## 최종 원칙

좋은 파일명은
“모든 의미를 파일명에 넣는 것”이 아니라,
“문서 성격을 빠르게 구분하고 index/title/metas와 함께 읽히는 것”이다.

