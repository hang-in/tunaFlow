---
title: Beta E2E Scenario Checklist (Phase 5 Finding 5-1)
updated_at: 2026-04-20
canonical: true
status: active
owner: tunaFlow-core
---

# Beta E2E Scenario Checklist

베타 공개 직전 **필수 수동 검증** 10개 시나리오. 모두 pass 해야 release notes 를 확정하고 announcement 를 낼 수 있다.

## 검증 원칙

- **실제 프로젝트에서 실행** — test project(tunaInsight) 는 보조, 본 검증은 실제 개발 중인 코드 대상
- **엔진별 반복** — Claude / Codex / Gemini 중 최소 2개 엔진에서 각 시나리오 실행
- **실패 시 즉시 정지** — bypass 금지. 원인 파악 → hotfix → 재검증
- **로그 보존** — 실패 케이스는 `~/.tunaflow/crash-reports/` + 콘솔 로그 캡처

---

## 1. 프로젝트 첫 생성 → Main 대화 → 첫 응답

**목표**: 초기 사용자가 앱을 처음 켰을 때 막히는 지점이 없는지 확인.

체크 항목:
- [ ] 프로젝트 없는 상태에서 `ProjectStartup` 렌더
- [ ] 새 프로젝트 생성 (경로 선택 → 스택 감지)
- [ ] ProjectOnboardingModal 에이전트 감지 목록 표시
- [ ] "메인 대화" 선택 → 첫 메시지 전송
- [ ] 에이전트 응답 스트리밍 정상 (streaming 중단 없음)
- [ ] 응답 완료 후 메시지 DB 영속화 (앱 재시작 시 남아있음)

**실패 시 확인**: `conversations` 테이블, `messages` 테이블, `bootstrap::db::init_db` 로그

---

## 2. Plan → Architect → Subtask 검토 → Dev → Review → Done

**목표**: 핵심 3-role 워크플로우 전체 사이클.

체크 항목:
- [ ] Architect 호출 → Plan proposal 생성 (plan-proposal 마커 파싱 성공)
- [ ] Plan 승인 → `plans` 테이블 row 생성
- [ ] Architect 에게 drafting 문서 작성 요청 → 문서 생성 확인
- [ ] "Subtask 검토" 버튼 활성화 (문서 작성 완료 후에만)
- [ ] Subtask 검토 Branch 생성 → 승인
- [ ] Developer 호출 → 구현 Branch (linkedPlan.implementationBranchId)
- [ ] 구현 완료 → Review Branch 자동 생성 / Reviewer 호출
- [ ] Review verdict = pass → Plan 상태 "done"
- [ ] Review verdict = fail → findings → rev.N+1 Plan 제안

**실패 시 확인**: PlansPanel 상태, `plans.status`, `branches.mode`, role_guidance 파이프라인

---

## 3. Branch 생성 → adopt / archive

**목표**: 대화 분기 핵심 기능.

체크 항목:
- [ ] 메시지 checkpoint 에서 Branch 생성 → 드로어 열림
- [ ] Branch 안에서 대화 진행 → 메시지 shadow conversation 에 쌓임
- [ ] `adopt` → 부모 conversation 에 요약 메시지 삽입
- [ ] `archive` → Branch 가 "archived" 상태로 표시 + 드로어 닫힘
- [ ] 드로어 pin/unpin 토글
- [ ] Branch 라벨 rename (slug 충돌 방지 확인)
- [ ] Branch 삭제 (cascade: messages, branches)

**실패 시 확인**: `branches.status`, `adoptBranch` 로직, streamingUtils shadow conv ID 매핑

---

## 4. RT (Roundtable) 다중 참가자 → verdict 집계

**목표**: RT 전용 페르소나 + verdict aggregation.

체크 항목:
- [ ] Branch 에서 "RT 모드로 전환" → 참가자 추가 (Claude + Codex + Gemini)
- [ ] Sequential 모드: 순차 발언 → 각 에이전트 응답 영속화
- [ ] Deliberative 모드: 동시 발언 → 메시지 타임스탬프 순서 정합
- [ ] 각 참가자에게 role_guidance 주입 확인 (RT 전용 페르소나)
- [ ] verdict 수집 → aggregation 결과 표시
- [ ] RT 종료 → adopt 가능

**실패 시 확인**: `rt_rounds`, `rt_verdicts` 테이블, RT sequential orchestrator

---

## 5. Meta inbox 알림 수신 → askMeta

**목표**: 메타 에이전트 플로우팅 채팅.

체크 항목:
- [ ] Meta 알림 발생 이벤트 (예: Plan done) → MetaFloatingChat 배지 +1
- [ ] Bot 버튼 클릭 → inbox 탭에 알림 목록
- [ ] 알림 클릭 → 연관 Plan/Conversation 으로 네비게이션
- [ ] "chat" 탭 전환 → Meta conversation 에서 질문 전송
- [ ] Meta 에이전트 응답 스트리밍 정상
- [ ] localStorage 알림 persist 확인 (앱 재시작 후 미읽음 유지)

**실패 시 확인**: `meta-notifications-v1` localStorage, `getOrCreateMetaConversation`, `tunaflow:meta-task` event

---

## 6. Insight 분석 실행 → findings → 상태 업데이트

**목표**: Insight 탭 재설계 후 전체 사이클.

체크 항목:
- [ ] Insight 탭 진입 → 카테고리 6개 표시 (안정성/테스트/아키텍처/성능/보안/기술부채)
- [ ] "전체 선택" → 분석 실행
- [ ] 진행 로그 실시간 표시 (Insight slice 에 progressLines 누적)
- [ ] 분석 중 다른 탭 이동 → 돌아와도 진행 상태 유지 (unmount 내구성)
- [ ] findings 결과 표시 → fix_difficulty 표시
- [ ] finding 상태 업데이트 (open → resolved)
- [ ] Plan done 트리거 → 관련 finding 자동 resolved

**실패 시 확인**: `insight_findings` 테이블, `insightSlice`, `count_open_insight_findings`

---

## 7. Settings 에서 agent 프로필 생성 → 사용

**목표**: Personas / Agents / Profile 연계.

체크 항목:
- [ ] Settings > Agents → 사용 가능한 CLI 자동 감지
- [ ] Settings > Personas → 새 persona 생성 (role + engine + model 조합)
- [ ] Settings > Profile → 사용자 프로필 작성 → ContextPack 에 주입 확인
- [ ] 메인 대화에서 persona 선택 → 해당 engine/model 로 요청
- [ ] Skills 섹션 → vendor snapshot 표시, 선택 가능
- [ ] Runtime 섹션 → rawq daemon 상태 확인
- [ ] Help 섹션 → 단축키/기능/문제해결/크래시 리포트 배지

**실패 시 확인**: `personas`, `user_profile` 설정, resolveModel 로직

---

## 8. 모바일 client 연결 → HTTP 소비 → WS 구독 → 재연결 복구

**목표**: HTTP API + WebSocket event replay.

체크 항목:
- [ ] Settings > Mobile → API 토큰 생성 / cloudflared tunnel URL 복사
- [ ] 모바일 브라우저에서 접속 → conversations 목록 로딩
- [ ] 대화 선택 → messages 렌더
- [ ] 메시지 전송 → WS 로 실시간 수신
- [ ] WS 끊김 시뮬레이션 (Wi-Fi 토글) → `?since=<ms>` 로 재연결 + 놓친 이벤트 replay
- [ ] 긴 응답 스트리밍 중 재연결 → 이후 chunk 모두 수신
- [ ] 모바일에서 보낸 메시지 → 데스크톱에서도 동일 session 으로 이어짐 (session_id 오염 없음)

**실패 시 확인**: `ws_event_log` 테이블, `auth_middleware` skip list, `http_api::events::broadcast_event`

---

## 9. 긴 대화 (1000+ 메시지) 스크롤 + 검색

**목표**: Virtuoso 가상 스크롤 + retrieval.

체크 항목:
- [ ] 메시지 1000개 이상 conversation 열기 (기존 대화 또는 seed)
- [ ] 스크롤 60fps 유지 (Chrome DevTools Performance 녹화)
- [ ] 상단/하단 경계 메시지 정상 렌더 (overdraw 누락 없음)
- [ ] 검색 (Cmd+K → 메시지 검색) → hit 위치로 점프
- [ ] 스트리밍 중 스크롤해도 auto-scroll lock 유지 (끝에서만 따라감)
- [ ] vizMarkers (tool-request 등) 토글 → 즉시 반영

**실패 시 확인**: MessageList Virtuoso, `_staleConversations`, streamingUtils 중복 방지

---

## 10. 앱 재시작 → 세션 이어가기

**목표**: 재시작 후 상태 복구.

체크 항목:
- [ ] 마지막 열었던 프로젝트 자동 선택 (`lastProjectKey`)
- [ ] 마지막 대화 자동 선택 (`lastConversationId`)
- [ ] 사이드바 너비/드로어 너비/테마 복구
- [ ] PTY 세션: 재시작 시 이전 세션 resume 또는 깨끗이 종료 (고아 PTY 없음)
- [ ] 미완료 run: `cleanup_stale_jobs` 가 정리 → UI 에 "실행중" 잔존 없음
- [ ] Claude resume_token: 다음 메시지 시 `~/.tunaflow/api-token` 재주입 확인
- [ ] 크래시 리포트 배지: 이전 세션에 panic 있었다면 Help 섹션에 표시

**실패 시 확인**: `AppShell` init flow, `runningThreadIds` 리셋, `pty_kill_all` HMR cleanup

---

## 종합 Pass 조건

- [ ] 10개 시나리오 모두 pass (블로커 없음)
- [ ] 각 시나리오마다 최소 2개 엔진에서 검증
- [ ] 발견된 P0/P1 버그는 모두 hotfix 머지 후 재검증
- [ ] 크래시 리포트 디렉터리에 이 세션 발생 panic 0 건

## Fail 허용 기준

- P2/P3 (사용성 불편, 시각적 사소한 문제): known issues 에 등록하고 pass
- RT 중간 스트리밍 미지원: CLAUDE.md 기등록 이슈, pass
- JSONL 완료 감지 실패(P1): 기등록, pass (단, 발생 빈도가 이전보다 높아지면 fail)
