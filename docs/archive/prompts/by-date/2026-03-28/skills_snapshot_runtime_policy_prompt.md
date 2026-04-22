# tunaFlow skills snapshot runtime policy 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28
- 카테고리: skills / runtime-policy / docs

```md
# tunaFlow skills runtime snapshot 운영 문서화

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
`~/.tunaflow/skills`가 일반 사용자 편집 공간이 아니라,
`_research/_skills`에서 발행되는 **runtime snapshot 전용 디렉터리**라는 점을
명확히 문서화하라.

중요:
- 코드 변경 금지
- 스크립트 동작 변경 금지
- 문서만 수정 또는 추가
- 핵심은 운영 규칙을 분명히 하는 것이다

현재 전제:
- source of truth: `/Users/d9ng/privateProject/_research/_skills`
- runtime snapshot target: `~/.tunaflow/skills`
- publisher: `scripts/publish-skills.sh`

문서에 반드시 포함할 내용:

1. `~/.tunaflow/skills`는 snapshot publish 결과물이다
2. 수동 편집/수동 파일 추가를 권장하지 않는다
3. publish 시 기존 runtime snapshot은 삭제 후 재생성될 수 있다
4. 로컬 커스텀 스킬을 두고 싶다면 별도 정책이 필요하다
5. 현재 tunaFlow는 runtime snapshot만 읽는다

권장 산출물:
- how-to 문서 또는 reference 문서 1개
- 필요하면 기존 skills 관련 문서에 링크 추가

출력 형식:
### A. Files Updated
### B. Runtime Snapshot Rules
### C. What Users Must Not Do
### D. Remaining Open Questions
```
