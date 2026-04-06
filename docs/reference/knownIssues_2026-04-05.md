# Known Issues & Improvements — 2026-04-06 (세션 13 종료 시점)

---

## P1: 개선 필요

### Claude CLI 동시 실행 충돌
- **현상**: 같은 프로젝트에서 브랜치+메인 양쪽에 Claude 사용 시 프로세스 레벨 간섭
- **원인**: Claude CLI lock 파일 충돌
- **우회**: 한쪽을 다른 엔진(Gemini, Codex 등)으로 실행

### implComplete 마커 조기 활성화
- **현상**: 에이전트 실행 중에 implComplete 마커가 감지되면 리뷰 시작 버튼이 조기 활성화
- **해결**: runningThreadIds 가드 추가 (세션 13 커밋)

---

## P2: 후순위

### Artifacts 탭 Plan별 그룹핑 미구현
- artifacts 테이블에 plan_id 없음
- Plan별 artifact 분류 UI 필요

### 스마트 scaffold 기존 CLAUDE.md 갱신 안내
- **현상**: refresh_project_stack_info가 §1을 갱신하지만 사용자에게 알림 없음
- **수정 방향**: toast 알림 "프로젝트 스택 정보가 업데이트되었습니다" 추가

### microcompact 적용 확장
- prune_tool_results가 compression pre-pass + recent context에 적용됨
- 추가: rawq 결과의 `## Relevant code` 섹션 감지 패턴 보강
- 추가: CRG graph impact 결과의 다양한 헤더 형식 대응

### 컨텍스트 메뉴 확장
- 빈 영역 우클릭 메뉴 (ChatAreaContextMenu) 미적용
- 코드블록 우클릭 메뉴 미구현

---

## 참고: 세션 13에서 해결된 이슈

| 이슈 | 해결 |
|------|------|
| Review verdict 자동 감지 | autoDetectReviewVerdict + useSubtaskProgress review phase 폴링 |
| Reviewer 파일 접근 불가 (Codex) | PLATFORM_TIER0 규칙 수정 + task 파일 ContextPack 주입 |
| Verdict 스캔 첫 번째만 반환 | 마지막 verdict 우선 (3곳) |
| Doom loop 카운터 미리셋 | doom_loop_escalated + architect_redesign_requested 이후 리셋 (4곳) |
| Conditional verdict가 failure로 카운트 | review_conditional 이벤트 분리 |
| 크로스 프로젝트 스트리밍 오염 | isActiveThread() 가드 (5곳) |
| Rework 프롬프트 실패 카운트 과다 | doom loop 리셋 경계 적용 |
| processReviewVerdict 중복 호출 | 같은 verdict 타입 중복 방지 가드 |
| Conditional → Pass 사용자 override 차단 | verdict 타입별 가드로 수정 |
| Architect 워크플로우 무시 | PLATFORM_TIER0 + ContextPack plan phase 주입 |
| plan-proposal 파서 멀티라인 details 미지원 | parseNumberedList 멀티라인 지원 |
| DraftingActions details 없으면 버튼 숨김 | 항상 표시 (경고와 함께) |
| 사이드바 브랜치 → 탭 미전환 | Plan 탭 + 워크플로우 스테이지 자동 전환 |
| CSP 비활성화 | tauri.conf.json CSP 정책 설정 |
| 빈 catch 35개 | console.debug/warn 로깅 추가 |
| Non-null assertion 11개 | convId 별칭 + optional chaining |
| Mutex poison 위험 | parking_lot::Mutex 전환 |
| 한국어 토큰 과소 추정 | estimate_tokens() CJK 보정 |
| AppError 문자열 평탄화 | { code, message } JSON 구조화 |
| Plan 상태 변경 UI 부재 | 우클릭 컨텍스트 메뉴 + 배지 확대 |
| Plan 전체 보기 없음 | "All" 스테이지 탭 |
| 승격 프롬프트 PLATFORM_TIER0 미동기화 | Verification + Scope boundary 추가 |
| testOutput Reviewer 미전달 | PlanCard에서 run_project_tests → startReviewRT 배선 |
| Reviewer MCP 언급 | "MCP 미사용, 로컬 파일 직접 읽기" 규칙 추가 |
| Developer 에러 무시 패턴 | PLATFORM_TIER0에 에러 처리 규칙 추가 |

---

## 참고: 세션 12에서 해결된 이슈

| 이슈 | 해결 |
|------|------|
| Dev↔Review 과다 순환 | 3-role 프롬프트 전면 수정 + 에이전트 템플릿 동기화 |
| EngineSelector ollama 크래시 | ENGINE_LIST + fallback 방어 |
| 테스트 반복 실행 (탭 전환) | testResultCache 모듈 레벨 + cancelled 가드 제거 |
| subtask 완료 표시 누락 | impl-complete = 전부 done + 0-based idx 수정 |
| slug 충돌 (한국어 Plan) | DB plan.slug + collision detection (v26) |
| abandoned Plan 표시 | status filter 추가 |
| workflow stage 칩 색상 | 선택된 것만 highlight |
| 드로어 애니메이션 밀림 | translateX 100% → 24px |
| hover toolbar 깜빡임 | Radix data-state=open 활용 |
