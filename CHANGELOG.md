# Changelog

All notable changes to tunaFlow are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [0.1.7-beta-4] - 2026-05-07

🚨 **Windows production 빌드 CSP IPC 차단 hotfix + bge-m3 메모리 최적화** —
v0.1.7-beta-3 의 capabilities fix 후에도 외부 사용자 (devbug,
[#264](https://github.com/hang-in/tunaFlow/issues/264)) 환경에서 회복 안 된
3rd layer root cause: `tauri.conf.json` 의 CSP `connect-src` 가 Tauri 2 의
IPC custom protocol (`http://ipc.localhost`) 을 명시 안 해 production 빌드
에서 모든 plugin 호출 차단 → 사용자 설정 / WindowControls listener 등이
*전부* fail. dev 모드는 vite localhost 라 postMessage 폴백 으로 우회되어
표면 안 됐던 회귀. 같이 묶어 [#271](https://github.com/hang-in/tunaFlow/issues/271)
의 bge-m3 메모리 최적화도 처리.

### Fixed

- **CSP IPC 프로토콜 + jsdelivr 폰트 허용** ([PR #272](https://github.com/hang-in/tunaFlow/pull/272)) —
  `tauri.conf.json:26` 의 csp `connect-src` 에 `ipc:` (macOS/Linux) +
  `http://ipc.localhost` (Windows production) 명시. plugin 호출 (`store|load`
  / `event|listen` 등) 정상 동작 회복. style-src/font-src 에
  `https://cdn.jsdelivr.net/gh/orioncactus/pretendard/` 경로 허용 (pretendard
  variable 폰트 로드 회복).
- **bge-m3 default pool size 1 — RSS 약 1.1GB 절감** ([PR #273](https://github.com/hang-in/tunaFlow/pull/273), issue [#271](https://github.com/hang-in/tunaFlow/issues/271)) —
  `agents/embedder.rs` 의 default pool 을 2 → 1 로 변경. 1.1GB 모델 weight 가
  in-process 로 두 번 로드되던 회귀 차단. `EMBED_SEMAPHORE = Semaphore(1)` 가
  동시 추론을 직렬화하므로 throughput 회귀 0. 8GB RAM 환경 / 다른 앱 jetsam
  간접 유발 위험 영역 개선.

### Notes

- **외부 사용자 회복 path**: v0.1.7-beta-3 까지의 fix (capabilities + drag
  region 격리 + native frame fallback) 도 모두 유효한 fix 였으나 *CSP 가
  IPC 자체를 차단* 한 상태에서는 효과 못 봄. v0.1.7-beta-4 부터 IPC 가 살아
  capabilities + WindowControls 가 비로소 정상 동작. devbug 환경 새 자산
  재설치 후 회복 확인 부탁드립니다.
- dev 모드 (`npm run tauri dev` via `http://localhost:1420`) 가 production
  과 다른 origin/CSP 적용 path 라 회귀 표면 차이가 진단 어려움. 향후 production
  smoke 자동화 axis 검토 영역.
- macOS / Linux 영향 0 — CSP 변경이 모든 OS 적용이지만 *기존 허용 영역
  (dos.zone / github avatars / dataURI 등) 모두 보존* + IPC 프로토콜은
  postMessage 폴백 으로 dev 환경에서 정상 동작했음.
- Gemini code review (PR #272): connect-src 의 tauri.localhost 중복 제거 +
  jsdelivr path 축소 권장 모두 수용.

## [0.1.7-beta-3] - 2026-05-07

🚨 **Windows 캡션바 root cause hotfix — Tauri 2 capabilities 권한 4건 추가** —
v0.1.7-beta-2 의 진단 보강 ([PR #268](https://github.com/hang-in/tunaFlow/pull/268)) 이
유효한 진단 path 였고 architect dev 환경 console 캡처로 결정타 확보:
*"window.close not allowed. Permissions: core:window:allow-close"* 에러가 모든
button click 에서 발생. PR #237 의 custom WindowControls 가 처음부터 동작한 적
없는 진짜 root cause 는 capabilities permission 누락이었음.

### Fixed

- **Tauri 2 capabilities 권한 4건 추가** ([PR #269](https://github.com/hang-in/tunaFlow/pull/269)) —
  `src-tauri/capabilities/default.json` 에 `core:window:allow-minimize` /
  `allow-toggle-maximize` / `allow-close` / `allow-is-maximized` 명시 부족이
  custom WindowControls button click 의 모든 호출을 *권한 부재* 로 차단하던
  회귀 fix. 닫기 / 최소화 / 최대화 / Maximize state 동기화 (`onResized` listener)
  모두 정상 동작.
- **Drag region cascade button click 가로채기 방지** ([PR #269](https://github.com/hang-in/tunaFlow/pull/269)) —
  `TitleBar` outer div 의 `data-tauri-drag-region` 제거 후 좌패딩 / 정보 row /
  중앙 spacer 3 sub-section 에만 attribute 유지. button 영역이 drag region
  descendant 에서 빠짐. WindowControls button 들에 `onMouseDown stopPropagation`
  이중 안전망.

### Added

- **`tauri features = ["devtools"]`** ([PR #269](https://github.com/hang-in/tunaFlow/pull/269)) —
  release 빌드에서도 `Ctrl+Shift+I` / `F12` 로 devtools 활성. 향후 회귀 발견
  시 외부 사용자가 직접 console 로그 캡처 가능 (베타 단계 진단 가치).
- **WindowControls click 실패 시 console.error 진단** ([PR #269](https://github.com/hang-in/tunaFlow/pull/269)) —
  권한 누락 또는 Tauri API 영역 회귀가 다시 발생하면 즉시 표면화.

### Notes

- **외부 사용자 보고 회복 path**: 이전 v0.1.7-beta-2 release 시 *"fix 됐다"* 안내
  드렸으나 진단 보강 단계만 들어간 상태였습니다. v0.1.7-beta-3 부터 권한 fix
  적용으로 실제 button 동작 회복. devbug 환경에서 새 자산 재설치 후 회복 부탁드립니다.
- **Backend `set_decorations(false)` 제거** — Windows native frame fallback 보존.
  capabilities/WindowControls 어느 한쪽이 깨져도 OS native control 로 사용자
  회복 path 확보 (이중 안전망). UI 중복 회피는 후속 PR 영역 (Gemini code review
  #2 영역).
- macOS / Linux 영향 0 — capabilities 추가는 cross-platform 안전, set_decorations
  제거는 cfg(windows) 분기 안에 있던 호출.

## [0.1.7-beta-2] - 2026-05-07

🩹 **Windows 캡션바 hotfix + platform detect 진단 보강** —
외부 사용자 (devbug, [#264](https://github.com/hang-in/tunaFlow/issues/264))
보고 회복 1차 작업. v0.1.5-beta Windows 자산부터 잠재한 *"native titleBar /
닫기·최소화·최대화 버튼 / 창 드래그·리사이즈 모두 부재"* 회귀 차단.
config 분리로 macOS-only 옵션 영향 차단 + frontend `WindowControls` 의
`isWindows` gate 신뢰성 향상 + 진단 console.warn 보강 (캡션바가 여전히
부재 시 devtools 로 root cause 확정 가능).

### Fixed

- **Tauri config platform-conditional 분리** ([PR #268](https://github.com/hang-in/tunaFlow/pull/268)) —
  `tauri.macos.conf.json` 신규로 macOS 전용 `titleBarStyle: "Overlay"` +
  `hiddenTitle: true` 분리. base `tauri.conf.json` 에서 두 키 제거 +
  `decorations: true` 명시 (Windows / Linux native chrome 의도 표현).
  Tauri 2 의 platform-conditional merge 로 macOS 동작 보존.

### Added

- **`detectPlatformDiagnostic()` snapshot helper** ([PR #268](https://github.com/hang-in/tunaFlow/pull/268)) —
  `lib/platform.ts` 가 `navigator.userAgentData.platform` 우선 + userAgent
  regex 폴백으로 OS detect. `TitleBar.tsx` 가 module-load 시 1회,
  `WindowControls.tsx` 가 mount 시 1회 console.warn 출력 — 사용자 devtools
  에서 (a) 미마운트 vs (b) 마운트는 됐으나 invisible 구분 가능.

### Notes

- **사용자 검증 단서**: v0.1.7-beta-2 재설치 후 캡션바 회복 안 되면
  Ctrl+Shift+I 로 devtools 열어 console 의 `[TitleBar] platform diag` /
  `[WindowControls] mounted` 로그 확인. `isWindows: false` 면 detection
  실패 (후속 PR axis), 둘 다 정상 출력 + 캡션바 부재면 z-index/styling
  axis.
- macOS / Linux 영향 0 (회귀 가드): macOS override 에 두 키 살아있고
  base 의 `decorations: true` 는 Linux default 와 동일.

## [0.1.7-beta] - 2026-05-07

🩹 **Roundtable 합의 영구화 + RT marker 격리 + Architect ContextPack 인계** —
외부 사용자 (devbug, [#263](https://github.com/hang-in/tunaFlow/issues/263))
보고 회복. RT 환각/오동작 3 영역 (라운드 간 합의 망각 / main conv 단일
dispatch 시 합의 무시 / Architect 가 RT 대화 내역 접근 못 함) 의 root cause
3중 복합 fix. *"사용자 RT 사용 포기"* 단계 → 정상 사용 회복 path 도입.

### Added

- **`roundtable_consensus` 테이블** (DB migration v50) — RT round 간 axis
  별 합의 항목을 영구 누적. 컬럼: id / conversation_id / round_index / axis /
  decision / participants(json) / confidence / created_at. INDEX:
  `idx_roundtable_consensus_conv_round`.
- **합의 추출 helper** — synthesizer 응답에서 `<!-- tunaflow:consensus -->`
  JSON fence (primary) 또는 `## Agreed axes` markdown bullet (fallback) 으로
  axis 추출 후 `roundtable_consensus` row 누적.
- **synthesizer prompt 의 *"## Consensus reached so far"* 섹션** — 라운드
  N+1 의 synthesizer + 참여자 prompt 에 라운드 1~N 의 누적 합의 명시 포함.
  같은 합의 재시도 환각 차단 (시나리오 B 회복 핵심 path).
- **`messages.rt_round_index` 컬럼** (DB migration v51, nullable) — RT round
  헤더 + 참여자 메시지 + synthesizer 헤더에 round_num 기록. 부분 INDEX
  `idx_messages_rt_round`.
- **`load_recent_messages_excluding_rt()` helper** — single agent dispatch
  시 ContextPack 의 `current_messages` 가 RT round transcript 를 *주제별
  컨텍스트* 로 prepend 하지 않음 (시나리오 A 회복 핵심 path).
- **`build_rt_consensus_section()` helper** — Architect dispatch / single
  agent dispatch 가 받는 ContextPack 에 *"## Roundtable Consensus"* 섹션
  명시 인계. 라운드별 axis / decision / participants 누적 list (시나리오
  C 회복 핵심 path). branch shadow conv 도 cover.

### Fixed

- **시나리오 A** (#263 보고 #1) — RT 진행 중 단일 에이전트 follow-up 질의
  시 *라운드 재실행 흉내* / *합의 부정* 환각 회복. ContextPack 이 RT round
  transcript 를 transcript 영역에서 제외하고 합의는 별 섹션으로 인계.
- **시나리오 B** (#263 보고 #2) — RT 5 라운드 이상 진행 시 *같은 합의
  재시도* / *3 fail 임계값 누적* / *사용자 fallback 영구 반복* 회복. 누적
  합의가 synthesizer + 참여자 prompt 에 명시 등장.
- **시나리오 C** (#263 보고 #3) — RT 종료 후 Architect dispatch 시 *마지막
  라운드만 정리* 회복. 라운드별 누적 합의 + 참여자 의견 명시 인계.

### Notes

- DB migration v50 + v51 자동 진행. 기존 사용자 데이터 보존 (기존
  `roundtable_brief` memo / messages 의 NULL `rt_round_index` 모두 그대로).
- INV-RTC-1~8 모두 보존: round 본체 알고리즘 / Voting + MoA Synthesizer
  본체 / conv 공유 정책 / 기존 ContextPack 섹션 / branchSessionPolicy
  INV-1~5 / migration destructive 금지 / RT 미사용 영향 0.
- Frontend 변경 0 (backend 영역). 사용자 가시 변화는 *RT 진행 동작 회복*
  으로 표면됨.

## [0.1.6-beta] - 2026-05-04

**Workflow architectural change** — Reviewer verdict 처리 책임이 Meta-agent inbox
에서 Architect main conv 로 이동. *"Of the agent, By the agent, For the agent"*
원칙에 따라 *plan 사이클 결정* 은 Architect 의 design 책임으로 통합되고, Meta
는 *Tier 2 brief 생성기 + 알림 inbox* 역할로 좁아짐 (전면 해체 아님).

### Changed

- **Reviewer verdict → Architect 직행** (PR #261) — Plan 리뷰 통과 / 5회 누적
  실패 시 Meta inbox 알림이 아닌 Architect main conv 로 prompt 자동 dispatch.
  - `review_passed` (pass) → Architect 가 *"plan 완료, 다음 우선순위 제안"*
    prompt 자동 수신
  - `doom_loop_escalated` (5회 fail 누적) → Architect 가 *plan 재설계* prompt
    자동 수신 (기존엔 사용자 명시 클릭 필요)
  - 신규 helper SSOT: `src/lib/workflow/architectDispatch.ts` —
    `dispatchArchitectNextPriority(plan)` / `dispatchArchitectRedesign(plan,
    verdict, opts)`. `ReviewVerdictCard.handleRedesign()` (사용자 클릭) 도 같은
    helper 재사용.
  - doom warn (3회 fail) 은 사용자 결정 영역 보존 — `plan_event_log` 에
    `doom_loop_warning` event 만 남고 Architect 자동 호출 없음
- **MetaNotificationKind 정리** (PR #262) — review-cycle 4종 (`review_passed` /
  `review_failed` / `doom_loop_warning` / `doom_loop_escalated`) deprecated.
  `tier2_brief` 신규 — Tier 2 분석 (Haiku/Flash 저비용 brief) 결과 전용.
  기존 inbox 의 deprecated kind row 는 fallback 라벨로 표시 (route navigation
  / dismiss 정상). 마이그레이션 불필요.

### Removed

- **askMeta UX 폐지** (PR #260) — Meta 알림 inbox 의 *"메타에게 물어보기"* 버튼
  / `askMetaAbout` callback / 관련 i18n 키 (`action_ask_meta`, `ask_about_*`)
  제거. inbox 항목은 *읽기 / dismiss / route 이동* 만 동작. 사용자가 직접 메타에게
  질문하는 흐름은 Meta floating chat 의 *채팅 탭 입력창* 으로 대체.

### Notes

- Tier 2 brief 분석 (`maybeTriggerMetaAnalysis`) 의 트리거 시점 (review_passed /
  review_failed) + 엔진 (Haiku/Flash) + 분석 결과 dispatch 자체는 보존 — kind
  만 `tier2_brief` 로 이동.
- identity-trigger / memory auto-trigger / Rust `meta_agent/` 모듈 / Meta
  conversation 자체 / `tool_request_failed` / `insight_detected` /
  `plan_promoted` / `architect_redesign_requested` / `generic` 알림 모두 보존.
- Plan SSOT: `docs/plans/reviewerVerdictDirectArchitectPlan_2026-05-04.md`
  (10 invariants, 7 task → 3 PR 분리). c-2 scope (Meta role 부분 축소).
- Test baseline: FE 401 → 422 (+21 — architectDispatch 단위 / verdict 분기 /
  askMeta 비존재 가드). Rust 614 변동 없음.
- Gemini code review medium feedback follow-up 동일 cycle 머지 (40bc1aa):
  `architectDispatch` 의 `useChatStore` static import → dynamic
  `await import("@/stores/chatStore")` 패턴 (다른 workflow 모듈과 일관성),
  `MetaFloatingChat.test.tsx` 의 source-level regex 가드 brittleness 한계 +
  의도 주석 보강.

## [0.1.5-beta] - 2026-05-03

🩹 **devbug 외부 사용자 보고 #254 / #255 hotfix release** — 두 영역 자가 회복 path 회복.

### Fixed

- **branch view chat input 회귀 fix** (PR #257, issue #255) — plan A 진행 중
  plan B revision 머지 시 plan A 의 dev branch 가 `archive_branch` 호출로
  `status='archived'` 로 변경 → `BranchThreadPanel.tsx` 의 `isReadOnly` 분기가
  archived 도 readonly 로 처리하여 chat input mount 차단 → 사용자가 메인
  창으로 우회. `!isReadOnly` 분기를 input 영역만 `status !== "adopted"` 로
  좁힘 — archived branch 에서도 chat input 노출, INV-1~5 (branch session =
  main session 공유) 정책 보존되어 send 도 main sdk session 으로 안전 전달.
  status badge / Adopt / Delete 등 다른 readonly 분기는 기존 동작 유지.
- **ARCHITECT_TEMPLATE result task 자동 inject 차단** (PR #256, issue #254
  영역 A) — Architect 가 plan 작성 시 마지막 task 에 "result.md 작성" 자동
  inject 시키는 패턴 차단. PR #211 ("Never read result.md") + PR #212
  (REVIEWER read 차단) 정책과 모순되어 reviewer verdict 가 result 포맷 위반으로
  반복 fail 하던 회귀 root cause. 본문에 `Result Report — DO NOT include as a
  subtask` 전용 섹션 + Critical Rules 라인 추가, unit test (`architect_template
  _blocks_result_task_inject`) 로 정책 lock-in.
- **ARCHITECT_TEMPLATE 본문 prompt 노이즈 정리** (PR #259) — Gemini code
  review (PR #256 follow-up) 반영. agent 가 매번 읽는 prompt 본문의
  관리용 `(PR #212 policy)` / `(issue #254)` 라벨 제거 — LLM 토큰 효율 개선.
  doc comment / unit test 메시지의 issue 참조는 유지 (개발자 추적 컨텍스트).

### Added

- **docs/agents/\*.md sentinel 보존 + migration** (PR #258, issue #254 영역
  B) — 외부 사용자가 architect.md 에 customize 추가해도 tunaFlow 재시작 시
  scaffold 가 덮어쓰던 회귀 fix. `<!-- BEGIN user-customize --> ~ <!-- END
  user-customize -->` sentinel 마커 도입:
  - sentinel 안 영역: scaffold 가 절대 손대지 않음 (사용자 customize 보존)
  - sentinel 밖 영역: tunaFlow 의 latest template 으로 자동 갱신
  - legacy file (sentinel 미보유) → `*.md.pre-sentinel` 백업 후 새 template +
    빈 sentinel 영역 적용. 백업 생성 fail 시 scaffold 갱신 자체 abort
    (사용자 데이터 보존 우선)
  `refresh_agent_doc_with_sentinel()` 함수 + 8 unit test (3 case: missing /
  sentinel-aware refresh / legacy migration with backup) 로 동작 lock-in.
  ARCHITECT/DEVELOPER/REVIEWER_TEMPLATE 끝에 빈 `## Custom Rules` + sentinel
  쌍 inline 추가.

### Notes

- §B (Known issues policy) 변경 없음 — community batch 정책 유지.
- v0.1.4-beta hardening (T9-a/b/T11/T12 + Mac/Windows hotfix + sdk transport
  flip) 위에 누적되는 hotfix release — 외부 사용자(devbug) 보고 #254/#255
  영역만 한정 변경, 다른 영역 회귀 0.

## [0.1.4-beta] - 2026-04-30

🚨 **긴급 패치** — claude CLI 2.1.121 (2026-04-28 자동 update) 의 `--sdk-url`
정책 변경으로 tunaFlow sdk-session 모드 영구 차단. 모든 사용자 환경에서 claude
응답이 30s timeout 으로 중단되는 회귀 발생. CLI `-p --resume` path 로 transport
전환 (Anthropic 공식 사용자 path).

### Fixed

- **macOS Edit submenu 추가** (PR #252) — Cmd+C / Cmd+V / Cmd+X / Cmd+A /
  Cmd+Z / Shift+Cmd+Z 등 standard clipboard shortcut 회복. PR #217 (Plan C
  T01, global Cmd+, shortcut + macOS menu) 의 menu.rs 가 App submenu
  (Settings/About/Quit) 만 등록 + Edit submenu 누락 회귀. Tauri 2 macOS
  정책상 Edit menu 미등록 시 WKWebView 의 standard shortcut 자체 처리도
  dead. `cfg(target_os = "macos")` 분기로 PredefinedMenuItem 6 항목
  (undo / redo / cut / copy / paste / select_all) 추가. Win/Linux 영향 0.
- **dev 모드 notification crash 차단** (PR #251) — Plan D (PR #220,
  native UNUserNotificationCenter bridge) 의 dev 빌드 한정 회귀.
  bundle 없는 binary (`target/debug/tuna-flow`) 에서 `currentNotificationCenter`
  호출 시 NSInternalInconsistencyException → SIGABRT. `cfg!(debug_assertions)`
  guard 로 dev 빌드 시 graceful skip. release `.app` (bundle 보유) 영향 0.
- **claude cli mode 의 ContextPack double history 차단** (PR #245~#248,
  사용자 환경 발견 2026-04-30) — transport flip (`-p --resume`) 후 cli mode
  가 `session_freshness "적용 제외"` 로 박혀 있어 *Claude session 자체
  history + tunaFlow ContextPack 의 conversational/structured/anchor 2 turns
  동시 inject* = double history → paid API 영역 차감 → 사용자 환경
  (org_level_disabled paygo) 에서 거부 회귀. 사용자 architectural insight
  ("어차피 DB 에 history 있으니 검색해서 가져옴") 기반 누적 fix:
  - **T9-a** (PR #245): cli mode session_freshness 적용 — 두 번째 send 부터
    minimal mode 자동 발동 (`is_session_continuation=true` → drop
    recent_context + compressed_memory).
  - **T9-b** (PR #246): cli fresh session 도 `compressed_memory` drop —
    첫 send 도 paid API trigger 회피.
  - **T11** (PR #247): cli fresh session 시 plan / plan_document / artifacts
    / findings / retrieval / cross_session 도 drop.
  - **T12** (PR #248): cli fresh session 시 current_messages /
    parent_messages drop — `prompt_assembly.rs:421-422` 의 anchor 2 turns
    "budget 초과여도 무조건 포함" 정책 우회.
  - 결과: cli fresh session prompt = platform + agent-role + skills +
    user_prompt (~15K chars) → paid API trigger 회피. agent 가 필요 시
    tool-request 마커 (`recent_turns:N` / `probe_message:ID` /
    `full_message:ID` 등 s38 부터 구현됨) 로 on-demand 검색.
  - SSOT: `docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md` §4
    Task 09~12.
- **stale resume_token 자동 회복** (PR #238~#242, T1~T8) —
  ([claudeTransportFlipHardeningPlan](docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md))
  한동안 미사용 conversation 의 resume_token 이 (a) 과거 sdk-url 시점
  session id 이거나 (b) Anthropic 측 TTL 만료 → `--resume <id>` 시도가
  "out of extra usage" 형태로 거부되던 문제. 사용자 액션 0 자동 회복.
  - **T1**: claude.rs `stream_run` 이 `rate_limit_event` line parse →
    `RunOutput.last_rate_limit` 로 노출.
  - **T2**: `stream_run` wrapper 가 result.is_error keyword 패턴 detect →
    `--resume` 제거 후 1회 retry. false positive 차단.
  - **T3**: retry 성공 시 `session_freshness::clear_delivered_key` → 다음
    send 부터 `is_session_continuation=false` → ContextPack revival 자동.
    frontend 에 `claude:fresh_fallback` 이벤트 emit.
  - **T4**: fresh_fallback toast + RuntimeStatusBar 의 rate_limit
    indicator. claude.ai/settings/usage 링크.
  - **T5**: DB migration v49 — 7일+ idle conversation 의 stale claude
    resume_token 일괄 NULL. idempotent.
  - **T6**: Conversation 우클릭 메뉴에 "Claude 세션 재시작" 항목 추가.
  - **T7**: claude API 에러 6 종 분류 (stale_resume_token / auth_failure /
    rate_limited / quota_exceeded / model_unavailable / unknown).
- **Community follow-up batch** (PR #211, #215~#222) — batmania52 외부
  사용자 보고 5 plan 일괄 처리:
  - PR #216: rawq vendor 자동 git clone fallback (build 진입 장벽 차단)
  - PR #215: onboarding "건너뛰기" 버튼 노출 회복
  - PR #217+#218: Cmd+, 글로벌 단축키 + macOS 메뉴 + recent projects DB v48
  - PR #219: docs panel scope toggle (P3-Lite, default='all')
  - PR #220: native UNUserNotificationCenter bridge (osascript 의존 제거)
  - PR #222: codex stderr piped (onboarding 진단 도구)
  - PR #211: result.md contamination — reviewer ContextPack 입력 격리
- **Reviewer 정책 위반 차단** (PR #211 + 후속) — Codex Reviewer 가
  `*-result.md` 를 자체 read tool 로 직접 열람 후 잘림 패턴을 verdict 근거로
  사용하던 정책 위반 패턴 확인. ContextPack 입력 차단 (PR #211, root cause)
  에 더해 REVIEWER_TEMPLATE 에 "Never read `*-result.md`" 규칙 명시 추가
  (이 plan). reportSync 의 truncation 도 UTF-8 boundary-safe 8k/2k 상한 +
  잘림 마커 + sentinel 기반 self-include guard 로 강화.
- **claude agent watchdog trailing kill 차단** — reader loop 정상 종료 후
  watchdog 30s sleep 누적이 이미 reap 된 PID 에 `kill -9` 송출하던 race.
  PID 재사용 시 엉뚱한 프로세스 kill 위험 0 으로 차단. RAII guard 패턴.
- **claude transport 영구 차단 회귀** (claude CLI 2.1.121 정책 변경):
  - claude 2.1.121 가 `--sdk-url` 의 host 를 `api.anthropic.com` 등 5 도메인만
    허용하도록 hardcoded whitelist 도입. tunaFlow 의 localhost WS 서버 차단
    → 모든 send 가 30s timeout. 사용자 가시 메시지 "claude did not connect within 30s"
  - **fix**: dispatch default 를 `-p --session-id`/`--resume` path 로 flip.
    claude internal session store 가 history 보관, 매 send 마다 fresh spawn
    (~2.5s) + cache hit 으로 빠른 reload (cache_read_input_tokens ~36k 확인)
  - manual 검증: Step 1 `--session-id "remember 42"` → "OK" / Step 2
    `--resume "what number?"` → "42" (stateful conversation 정상)
  - SSOT: `docs/plans/claudeResumeSessionTransitionPlan_2026-04-29.md`

### Changed

- **`resolve_claude_mode` default flip** — `cli` (resume-session) 가 default,
  `sdk-url` 은 `TUNAFLOW_USE_SDK_URL=1` 환경변수 명시 시만 활성화 (Anthropic 정책
  우회 path 발견 시 즉시 재활성화 가능). 기존 `TUNAFLOW_DISABLE_SDK_URL` env 는
  의미 반대 변경됨 — 사용자 환경에 set 됐다면 unset 또는 새 변수로 마이그레이션.
- **`restart_sdk_session` 명령 의미 확장** — sdk-url path 는 기존대로 process
  kill + RESUME_IDS clear + DB clear, cli (resume-session) path 는 DB resume_token
  NULL 처리 (다음 send 가 신규 session 으로 시작). engine / model 변경 시 같은
  명령으로 통일.

### Notes

- **sdk-session 코드 유지** (`src-tauri/src/agents/claude_sdk_session.rs`) —
  Anthropic 정책 우회 path 발견 시 `TUNAFLOW_USE_SDK_URL=1` 으로 즉시 재활성화.
  본 release 에서 deprecate 만, 코드는 그대로.
- **검증된 우회 path 후보** (모두 production 부적합): `/etc/hosts` + self-signed
  TLS (system-wide 침범), binary patch (ToS 회색), desktop app 빈틈 (cloud 사용),
  PTY 회귀 (parsing 불안), Anthropic 공식 RC 등록 (가능성 낮음).

### Windows-specific changes

- **첫 실행 동의 dialog + Settings 수동 설치 버튼** (PR #227 / #229 — T4/T5)
  — `chub` (`@aisuite/chub`) 와 `code-review-graph` 가 Windows 미설치 상태로
  unavailable 표기되던 회귀 차단. 첫 실행 시 consent dialog 노출, 사용자
  동의 시 npm/pip 으로 글로벌 설치 (timeout npm 60s / pip 120s, 활성 venv
  자동 활용). dismiss 시 graceful fallback + Settings → Runtime 카드의
  "npm/pip 으로 설치" 버튼 노출. silent global install 금지 (INV-DEP-A).
  SSOT: `docs/plans/windowsDependencyBootstrapPlan_2026-04-29.md`.
- **`context_hub` / `crg` `resolve_bin` Windows path 인식** (PR #221 / #223
  — T1/T2) — `%APPDATA%\npm\chub.cmd` 와 `<python>\Scripts\code-review-graph.exe`
  를 Windows native process `Command::new` 가 정상 spawn 하도록 cfg 분기 +
  PATH fallback 보강.
- **Windows 타이틀바 통합** (PR #237 — T-WT-1/2/3) — `decorations: false` +
  자체 `WindowControls.tsx` (Min / Maximize-Restore / Close 사각 46×32) +
  좌측 정렬 통일. 기존 *3 라인 헤더* (native title bar + TitleBar.tsx +
  콘텐츠 헤더) → *1 라인 통합*. mac 도 같은 좌측 정렬 적용 (시각 회귀 0).
  SSOT: `docs/plans/windowsTitlebarUnificationPlan_2026-04-29.md`.
- **claude watchdog `taskkill` 분기** (PR #231 — §D) — `Command::new("kill")`
  은 Unix-only 라 Windows 에서 idle_timeout (600s) 시 child `claude.exe` 가
  zombie 잔존 위험. `cfg(unix)` 는 `kill -9`, `cfg(windows)` 는
  `taskkill /F /PID` 분기.
- **`kill_orphan_sdk_processes` Windows no-op stub** (PR #235) — Unix-only
  `pgrep`/`ps` 가 Windows 에서 silently no-op 였음을 explicit 화. 실제
  orphan 처리는 `windowsOrphanProcessHardeningPlan` (P3, post-beta) 후속.
- **conventions `@import` path separator 정규화** (PR #213) — `Path::display()`
  Windows backslash 출력으로 Claude Code `@path` syntax 깨지던 회귀 fix.
- **`commands/files` 테스트 path-separator 정규화** (PR #226 — R-W-7
  hotfix) — `flatten_md_paths` test helper 정규화. escalate-1~4 + 동일 패턴
  3건 일괄 처리, production 영향 0.
- **DB project path stale fallback** (PR #234 — Track 3) — mac 동기화 DB 의
  `projects.path` 가 Windows 에서 invalid 일 때 file IO timeout hang 차단.
  startup load 시점 validate + UI fallback, DB row 보존.
- **claude SDK 세션 stderr surface** (PR #233 — Track 2 진단 도구) —
  `Stdio::null()` → `Stdio::piped()` + `[sdk-session-stderr]` 라인 forward.
  PR #222 (codex stderr surface) 동등 패턴.

### Internal / housekeeping (Windows)

- **Rust warning silence** (PR #230) — `unused_imports` 1건 + `dead_code` 2건
  (test-only `InvokeClaude::Empty/Stub` + `NotificationAuthStatus` stub
  variants). 동작 변경 0.
- **Plan / handoff docs**: `windowsDependencyBootstrapPlan` (#214/#236),
  `windowsCiPipelinePlan` (#224, mac+win cross-OS regression detection
  정책), `windowsTitlebarUnificationPlan` (#228/#236), 그리고 status 갱신
  (#232 `complete`).

### Known issues (Windows)

- **첫 실행 후 첫 메시지 ~30초 지연** — Microsoft Defender 의 first-scan
  영향으로 추정 (정적 분석 결과 가설 (b) 가장 유력). **1분 후 다시 보내면
  정상 동작**. Track 2 진단 도구 (PR #233) 가 다음 cold start 시 backend
  stderr 에 root cause 를 노출 → fix axis 는 v0.1.5 정식 release 또는 캡처
  후 별 PR. SSOT: `docs/plans/windowsBetaHardeningPlan_2026-04-26.md` §B.
- **Windows 11 snap layouts overlay 미표시** — Maximize 버튼 hover 시 Win11
  22H2+ 의 snap layouts 가 안 뜸. `decorations: false` 와 잠재 충돌 진단 후
  v0.1.5 에서 fix (T-WT-5 / Q-WT-3). SSOT: `windowsTitlebarUnificationPlan`.

## [0.1.3-beta] - 2026-04-26

Beta 사용자 보고 follow-up. 첫 외부 사용자 환경에서 두 건 보고 — rawq sidecar 가
앱 번들에서 영구 미인식 (Tauri 가 sidecar 번들 시 triple suffix strip 하는데
코드는 `rawq-{triple}` 이름만 검색) + 채팅/로그 single newline 이 한 줄로
collapse. 둘 다 v0.1.0~v0.1.2 사용자 모두 영향이라 hotfix.

### Fixed

- **rawq sidecar resolution** (#210) — `sidecar_strip_name()` + `resolve_diagnostics()`
  추가 (`src-tauri/src/agents/rawq.rs`). Tauri 가 번들 시 triple suffix 를 strip
  해서 `Contents/MacOS/rawq` 로 들어가는데 코드는 `rawq-aarch64-apple-darwin`
  으로만 검색하던 영구 mismatch. v0.1.0-beta 부터 모든 macOS 사용자에게 영향.
  drag-install 시 quarantine (`xattr`) 부착으로 sidecar 가 SIGKILL 되는 케이스도
  같이 정리. CI 의 `build-tauri-lite` 에 staged + built bundle 양쪽 verify step
  추가로 회귀 차단.
- **`get_rawq_status` unavailable 메시지** — 다음 단계 액션 (`xattr -cr` 후
  재시도, README 링크) 포함하도록 명료화.

### Added

- **`remark-breaks` 마크다운 플러그인** (#209) — 채팅/로그 paste 시 single
  newline 이 visible line break 으로 표시됨. CommonMark spec 상 paragraph 안
  single `\n` 은 공백으로 collapse 되는 게 정상이지만, 채팅·로그 컨텍스트엔
  부적합. `src/lib/markdownPlugins.ts` SSOT 모듈 신규 + 11 사용처 통일 +
  회귀 테스트 13건 (single newline → `<br>` / paragraph break / list / code
  block / table / strikethrough 보존).
- **INSTALL.md drag-install 안내** — `xattr -cr /Applications/tunaFlow.app`
  필요성 + 문제 해결 표 + smoke checklist 4 단계.

### Changed

- **README / README.ko Known Constraints** — "rawq is a bundled sidecar"
  명시 + drag-install quarantine 영향 보강. 시스템 PATH 의 `rawq` 는 영향 없음.

### Notes

- `docs/reference/rawqSidecarReleaseAudit_2026-04-26.md` — Layer A1 audit 결과
  (DMG mount + `xattr` + `file` 출력 인용). 진단 분기 근거 SSOT.
- 이번 fix 머지 + 신 release 까지 필요. 기존 v0.1.x 사용자가 `xattr -cr` 만
  실행해도 코드측 mismatch 가 별도라 rawq 인식 안 됨.

## [0.1.2-beta] - 2026-04-26

Windows build support + fragility audit hardening. First Windows release
(NSIS installer for x64). Followup audit on yesterday's UTF-8 panic cascade
yields atomic-transaction wraps for `delete_branch` / `update_plan_status` /
`delete_conversation`, plus production-path panic / unwrap audit confirming
zero remaining fragility in the same category.

### Added

- **Windows x64 build** via NSIS installer (`tunaFlow_*_x64-setup.exe`).
  CI matrix extended to `windows-latest` for `rawq` sidecar + Tauri Lite
  bundle. Same `v*.*.*` Release as macOS — single asset listing per release.
  Plan: `docs/plans/windowsBuildPlan_2026-04-24.md`.
- **`basename(path, fallback)` utility** (`src/lib/utils.ts`) — supports both
  `/` (Unix) and `\` (Windows) separators. Replaces 5 hardcoded
  `path.split("/").pop()` sites.
- **`scripts/build-rawq.ps1`** — PowerShell mirror of `build-rawq.sh` for
  Windows local sidecar builds.
- **`NoConsole` trait** (`src-tauri/src/no_console.rs`) — Windows
  `CREATE_NO_WINDOW` flag applied to all subprocess spawns. Stops the cmd
  window flicker that was happening on every CLI agent / git / model
  discovery call (50 spawn sites across 17 files patched).
- **Splash UI on app init** (`AppShell.tsx`) — spinner + stepwise loading
  text ("환경 설정 로드 중..." / "프로젝트 목록 로드 중..." / "엔진 / 모델
  감지 중..." / "프로젝트 열기: {name}..."). Replaces the blank sidebar-color
  box that left users wondering if the app had hung. `setLoaded(true)` moved
  to `finally` so `selectProject` failure no longer traps users on the splash.

### Changed

- **`bundle.targets`** narrowed from `"all"` to explicit list `["app", "dmg",
  "appimage", "deb", "rpm", "nsis"]` — MSI excluded. MSI rejects prerelease
  identifiers (`-beta`); NSIS has no such restriction. Beta-window decision;
  may revisit MSI when `-beta` is dropped.
- **`bundle.macOS.signingIdentity = "-"`** moved from CI `--config` override
  to permanent `tauri.conf.json` setting. Windows shell-escape of multiline
  `--config '{...}'` JSON kept breaking; permanent config sidesteps it.
- **CI workflow_dispatch behavior** — version falls back to `package.json`
  default (smoke-test mode), `tagName=''` so no draft release is generated.
  Tag-push path unchanged — release flow identical to v0.1.1-beta.
- **Tauri icons regenerated** via `npx tauri icon` — old `icon.ico` was
  actually a PNG with `.ico` extension, which Windows `RC.EXE` rejected. New
  ICO is proper multi-resolution Windows icon resource.
- **`INSTALL.md`** — Windows installation section + Gatekeeper / SmartScreen /
  antivirus guidance split into 3 axes. VirusTotal verification note added.
  Release body in `build.yml` mirrors the same 3-axis structure.

### Fixed

- **UTF-8 char boundary panic** (`identity_analyzer.rs:96`) — `i + 1` byte
  index split a multi-byte CJK character (`'지'` mid-bytes) → panic →
  `Lock poisoned` cascade across `bg-worker` / vector indexing until app
  restart. Replaced with `i + c.len_utf8()` and proper char-count tracking.
  Same fix applied to `project_onboarding.rs:203` (`&content[..3000]`).
- **`delete_branch`** (`branches.rs:387`) — 8 sequential DELETE/UPDATE
  statements wrapped in a single transaction. Mid-statement failure (FK
  constraint, lock contention) no longer leaves partial state with child
  branches deleted but parent intact.
- **`update_plan_status`** (`plans.rs:319`) — status / phase / branch-archive
  3 statements wrapped in a transaction. Removes the "status='done' but
  phase='active' stuck" partial-commit window.
- **`delete_conversation`** (`conversations.rs:127`) — 4 + N×5 + 1 statements
  (including shadow-branch conversations) wrapped in a transaction.

### Removed

- **MSI bundle target** (Windows) — see Changed.

### Notes

- Production unwrap / expect / panic / unreachable / todo / unimplemented
  audit: zero remaining in non-test paths after this release.
- `failure_lessons.rs:63 create_failure_lessons_batch` loop multi-execute
  is intentional partial-commit (failed lesson skipped, others kept) —
  out of scope.

## [0.1.1-beta] - 2026-04-25

First post-launch maintenance release. Triages public-beta community reports
(#175 / #176 / #178 / #180), recovers brand-session intent that drifted during
the s36 PTY → sdk-url WS transition, and lands a stack of plan-driven fixes for
multi-Developer collisions, brand cancel semantics, and layout cascading bugs.

### Added

- **Custom endpoint config UI for Ollama / LM Studio** (#175) — base URL override
  per engine, no more rebuild-to-switch.
- **Manual verification gate (B-19)** between impl-complete and review (#176) —
  optional fail-reason field with placeholder fallback.
- **rawq cancel channel** for in-flight index builds (#197 / audit #5).
- **`rebuild_rawq_index` command + Settings UI button** for stale-index recovery.
- **User intent SSOT surfacing** — Architect ContextPack now anchors on conversation
  intent extracted from raw turns (#199).
- **Brand inherits main CLI session** — `session_key_for(conv_id)` normalizes
  `branch:*` → root conversation; brand sends skip ContextPack to reuse main
  session continuity (#198).
- **Multi-Developer active-plan isolation** — brand-aware plan slot + ContextPack
  sender Developer ID (#204).
- **`flexboxConventions.md` SSOT** — `flex-col + flex-1` requires `min-h-0` on
  every parent; documented after #191 / #201 cascade chain.
- **CHANGELOG.md** — this file.

### Changed

- **CI self-trust trigger** — main-push trigger removed; only external PRs and
  release tags (`v*.*.*`) run CI. Cuts cognitive context fragmentation for solo
  dev. See `docs/plans/selfTrustCiTriggerOptimizationPlan_2026-04-25.md`.
- **install.sh** — fallback to `sudo` when `/usr/local/bin` is root-owned;
  `/releases` (not `/releases/latest`) for prerelease tag support; DMG matched by
  arch tag (`aarch64` / `x64`) instead of Rust triple.
- **Cargo / npm manifest metadata** — license / author / repository / description
  populated on both crates and root package.
- **README** — embed 6-minute demo video via GitHub user-attachments CDN; sync
  README.ko with English; correct 4-engine → 5-engine parity; refresh stale
  DB/test counters.
- **Cancel semantics on brand** — stream-abort token only; `restart_sdk_session`
  remains the explicit session-kill path (#202).

### Fixed

- **#178** — Claude `--dangerously-skip-permissions` flag added at all 3 call
  sites (`claude.rs:162`, `claude.rs:380`, `claude_sdk_session.rs:381`); fixes
  infinite hang on fs permission prompts.
- **#180** — rawq excludes build-artifact dirs (`target/**`, `node_modules/**`,
  `.venv/**`, `dist/**`, `build/**`, 14 patterns total) to prevent OOM.
- **#191** — `min-h-0` on main flex parent so long drawer content cannot stretch
  the viewport.
- **#201** — `min-h-0` cascade fix for ChatPanel plan→dev phase footer drift
  (3 nested flex children).
- **#188** — tool-steps finalize running status on stream completion; non-streaming
  UI fallback path.
- **#190** — onboarding Skip cancels the Rust analysis task instead of leaking;
  unified error-state buttons.
- **#193** — `startReviewRT` entry failure rollback + retry UX.
- **#194** — Codex / Gemini meta-agent analysis no longer biased to Claude's
  output format; `parse_output` accepts engine-native shapes.
- **#195** — plan generation atomic DB transaction with file-write rollback.
- **#196** — branch adopt wraps DB writes in a single transaction.
- **#186** — DB v47 migration: `agent_jobs.conversation_id` nullable for
  detached jobs.
- **C-2 / B-16** — tunaflow marker scrubbing consolidated across result / insight
  export paths.
- **brand cancel** — was no-op (or worse, killed main session) post-PR #198;
  now stream-abort only, session preserved (#202).

### Removed

- Stale `.tunaflow/outbox/*.md` artifacts from the polling-deprecated era
  (post-9295062 cleanup) + `.tunaflow/outbox/` added to `.gitignore` (#200).
- Unused experimental README ack entries (DINKIssTyle-Markdown-Browser).

### Docs

- `docs/reference/branchCancelAudit_2026-04-25.md` — audit feeding #202.
- `docs/reference/flexboxAuditResult_2026-04-25.md` — repo-wide `flex-1` survey.
- `docs/reference/multiDeveloperIsolationDecision_2026-04-25.md` — A+B option
  rationale.
- `docs/plans/selfTrustCiTriggerOptimizationPlan_2026-04-25.md` — CI trigger
  policy SSOT.
- `docs/plans/branchInheritsMainSessionPlan_2026-04-25.md` — Task A intent
  recovery + 4-layer fix.
- 7 additional plans in `docs/plans/` (today's user reports + sibling work).

## [0.1.0-beta] - 2026-04-23

Public beta launch. See README and `docs/reference/sessionHistory.md` for the
full backstory; this entry only marks the cut.

[0.1.4-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.3-beta...v0.1.4-beta
[0.1.3-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.2-beta...v0.1.3-beta
[0.1.2-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.1-beta...v0.1.2-beta
[0.1.1-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.0-beta...v0.1.1-beta
[0.1.0-beta]: https://github.com/hang-in/tunaFlow/releases/tag/v0.1.0-beta
