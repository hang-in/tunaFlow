# tunaFlow skills runtime snapshot 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28
- 카테고리: skills / runtime-snapshot / all-vendors

```md
# tunaFlow 공용 스킬 runtime snapshot 발행

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:

공용 스킬 원본 저장소인
`/Users/d9ng/privateProject/_research/_skills`
를 직접 링크하거나 다중 루트로 바로 읽게 하지 말고,
`~/.tunaflow/skills`에 **runtime snapshot 복사본**을 발행하는 흐름을 만들라.

중요 전제:
- 현재 tunaFlow는 `~/.tunaflow/skills/*/SKILL.md`만 읽는다
- 지금은 loader 구조를 바꾸는 작업이 아니다
- 링크/심볼릭 링크는 사용하지 말 것
- 특정 vendor만 고르지 말고 **모든 vendor를 고려**할 것
- 다만 실제 snapshot에는 `SKILL.md`가 있는 항목만 runtime skill로 복사해도 된다
- `README.md`, `AGENTS.md`, `metadata.json` 같은 파일은 참고 자산으로 함께 복사 가능

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/2026-03-28_skills_runtime_snapshot_plan.md`
- `/Users/d9ng/privateProject/tunaFlow/src-tauri/src/commands/skills.rs`
- `/Users/d9ng/privateProject/tunaFlow/src-tauri/src/commands/agents_helpers/context_pack.rs`

source of truth:
- `/Users/d9ng/privateProject/_research/_skills`

runtime target:
- `~/.tunaflow/skills`

## 먼저 확인할 것

1. `_research/_skills` 아래 vendor 목록
2. 각 vendor 아래에서 실제 runtime skill로 발행 가능한 폴더
3. tunaFlow가 현재 기대하는 최소 구조가 무엇인지

## 이번 단계에서 할 일

1. runtime snapshot publish 스크립트 추가
2. `_research/_skills` 전체 vendor를 스캔하는 규칙 정의
3. `SKILL.md`가 있는 항목을 `~/.tunaflow/skills`로 복사
4. 가능하면 `_meta.json` 같은 source metadata 파일을 각 skill 폴더에 생성
5. 운영 문서 추가
6. 검증 절차 추가

## 권장 publish 규칙

### source scan

- `_research/_skills/skills-*`
- vendor별 하위 디렉토리를 순회
- `SKILL.md`가 있는 폴더를 runtime skill 후보로 간주

### runtime naming

skill 이름 충돌을 피하기 위해 vendor prefix를 붙이는 방향을 우선 고려하라.

예:
- `anthropic-template`
- `microsoft-tests`
- `openai-...`

### copy policy

- 기존 runtime snapshot은 publish 시 덮어쓰거나 재생성
- 원본은 절대 수정하지 말 것
- 링크 금지

### metadata

가능하면 각 runtime skill 폴더에 아래 정보를 남겨라.

- source vendor
- source path
- published_at

## 비목표

- skill loader 다중 루트 지원
- skill registry UI
- vendor 선택 UI
- 앱 번들 내부 스킬 포함

## 검증

작업 후 반드시 확인할 것:

1. `~/.tunaflow/skills`가 실제로 생성되는가
2. `_research/_skills` 전체 vendor를 고려하는가
3. `SKILL.md`가 있는 항목만 runtime skill로 정리되는가
4. tunaFlow의 `list_skills()`와 현재 구조가 충돌하지 않는가
5. 운영 문서가 source-of-truth와 runtime snapshot을 명확히 구분하는가

## 출력 형식

### A. Decision
### B. Source Scan Rules
### C. Runtime Snapshot Layout
### D. Files Changed
### E. Verification
### F. Deferred Work

바로 구현까지 진행하라.
```
