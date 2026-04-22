# tunaFlow skills UI visibility 계획 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28
- 카테고리: skills / ui / planning

```md
# tunaFlow Skills UI 가시화 계획 정리

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
현재 tunaFlow는 `~/.tunaflow/skills`를 읽고 active skills를 토글할 수 있지만,
사용자가 아래를 충분히 볼 수는 없다.

- 어떤 skill이 현재 활성화되어 있는지
- 그 skill이 어느 source/vendor에서 왔는지
- 현재 runtime snapshot이 언제 발행되었는지

이번 단계에서는 구현이 아니라 **계획 문서**를 작성하라.

중요:
- 코드 수정 금지
- 문서만 작성
- 기존 구조(`SkillsPanel`, `activeSkills`, `~/.tunaflow/skills`, `_meta.json`, `_snapshot.json`)를 기준으로 계획할 것

먼저 확인할 파일:
- `src/components/tunaflow/context-panel/SkillsPanel.tsx`
- `src/stores/slices/assetSlice.ts`
- `src-tauri/src/commands/skills.rs`
- `docs/how-to/skills-runtime-policy.md`
- `CLAUDE.md`

계획 문서에 반드시 포함할 내용:

1. 현재 상태
- 지금 UI에서 보이는 것
- 지금 UI에서 안 보이는 것

2. 목표 UX
- active skill 뱃지/카운트
- skill별 vendor/source 표시
- snapshot published_at 표시
- 현재 conversation에 어떤 skill이 적용 중인지 가시화

3. 데이터 소스
- `SkillDef`
- `_meta.json`
- `_snapshot.json`
- 필요한 경우 backend 확장점

4. 단계별 계획
- Phase 1: 현재 UI 내 최소 가시화
- Phase 2: source/vendor 메타데이터 표시
- Phase 3: snapshot 상태/발행 시각 표시
- Phase 4: 향후 registry/collections와 연결

5. 비목표
- 지금 당장 다중 루트 스킬 로더 구현
- 외부 registry 도입
- 원격 업데이트 기능

출력 형식:
### A. Current Gaps
### B. UX Goals
### C. Data Sources
### D. Phased Plan
### E. Deferred Work
```
