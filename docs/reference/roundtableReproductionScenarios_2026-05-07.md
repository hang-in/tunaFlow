---
title: Roundtable (RT) 재현 시나리오 — devbug 보고 #263
created_at: 2026-05-07
canonical: true
status: active
issue_source: GitHub #263 — devbug (2026-05-06)
related:
  - docs/plans/roundtableConsensusPersistencePlan_2026-05-07.md  # 본 시나리오를 참조하는 plan
  - src-tauri/src/commands/roundtable.rs  # RT round 본체
  - src-tauri/src/commands/roundtable_helpers/sequential.rs  # round prompt 조립
  - src-tauri/src/commands/roundtable_helpers/prompt.rs  # build_round_prompt_with_identity (23~74)
  - src-tauri/src/commands/agents_helpers/context_pack/db_queries.rs  # build_findings_section (173~215) / build_rt_inheritance_section (364~398)
---

# Roundtable (RT) 재현 시나리오 — devbug 보고 #263

본 문서는 외부 사용자 (devbug, GitHub #263) 가 보고한 *RT 환각 / 오동작 3 영역* 의 **재현 절차 명세** 다. plan (`roundtableConsensusPersistencePlan_2026-05-07.md`) 의 §0.2 시나리오 정확화가 본 문서를 참조한다.

목적:
- 사용자가 따라할 수 있는 *결정적* 재현 path (환경 차이로 인한 noise 최소)
- 기대 동작 vs 실제 동작 매트릭스 (보고 #1/#2/#3 각각)
- 구현 진행 시 회귀 가드용 e2e 시나리오로 직접 옮겨갈 형태

본 시나리오는 **사용자 환경에서의 실 검증** 을 전제로 한다. Architect 세션 (Tauri GUI 직접 실행 불가) 은 작성/분석만 담당.

## 0. 공통 환경 / 사전 조건

- tunaFlow v0.1.6-beta (혹은 그 이후)
- 프로젝트 1개 생성됨, ContextPack 정상 조립 (다른 영역 회귀 없음)
- 엔진 3종 (claude / codex / gemini) 모두 onboarding 완료 — RT 참여자 다양성 확보
- 시작 시 main conv 의 messages 비어있거나, 적어도 RT 진입 직전 메시지 인덱스 기록
- (선택) `~/.tunaflow/logs/` 의 trace event 동시 관찰 — round 진행과 prompt 조립 영역 검증 시

각 시나리오는 *독립적 plan / 독립적 RT* 로 진행. 한 시나리오의 잔여 state 가 다음 시나리오에 영향 주지 않게 each plan 새로 생성.

## 1. 시나리오 A — RT 진행 중 단일 에이전트 질의 시 합의 무시 (보고 #1)

### 사용자 보고 (원문 인용)

> "라운드 진행 중에 synthesizer의 응답에 대해 간단한 추가 질문을 하고 싶을 때 채팅창에서 에이전트들 선택 해제해서 대화를 시도했는데 전혀 딴 소리를 했습니다. 질의 내용에 대해서 보다는 기존 라운드를 혼자서 다시 실행하는 느낌이었습니다. 합의된 내용에 대해 합의되지 않은 것으로 착각하기도 했습니다."

### 재현 절차

1. **Plan 생성** — 임의 주제 ("ContextPack 압축 전략 설계" 또는 "rawq 인덱스 갱신 정책 개선" 등 *3 라운드 안에 합의 도출 가능한* 주제)
2. **RT 시작** — 드로어 진입 → Roundtable 모드 → Sequential 또는 Deliberative 선택 → 참여자 3명 (claude / codex / gemini) 모두 활성화
3. **2 라운드 진행** — 각 참여자 1회 응답 + synthesizer 1회 brief. synthesizer brief 에 *"X 영역에 대해 합의됨"* 같은 명시적 합의 텍스트 포함되는지 확인
4. **합의 메시지 record** — 2 라운드 종료 시점에 synthesizer 가 제시한 합의 항목 캡처 (스크린샷 또는 텍스트 복사)
5. **단일 에이전트 질의** — 채팅창 좌하단 (또는 RT 드로어 외부 main conv 입력창) 에서 *모든 에이전트 선택 해제* 후 단일 에이전트 (예: claude) 만 활성화 → *"방금 합의한 X 항목에 대해 한 가지만 추가로 명확히 해줘"* 같은 follow-up 질의
6. **응답 관찰** — 단일 에이전트가:
   - (a) 합의 항목을 인지하고 그 기반에서 follow-up 답변 → 정상
   - (b) RT 라운드 자체를 다시 실행하는 듯한 응답 (다른 참여자 의견 가정 / 새 관점 전개) → **회귀 일치**
   - (c) 합의 항목을 *"그건 합의되지 않은 것 같다"* 처럼 부정 → **회귀 일치 (강한 형태)**

### 기대 동작 vs 실제 동작

| 항목 | 기대 | 실제 (보고) |
|---|---|---|
| 단일 에이전트가 RT 합의 인지 | ✅ ContextPack 에 RT artifact 포함 | ❌ 합의 무시 / 부정 |
| 응답 톤 | follow-up answer | 라운드 재실행 흉내 |
| RT marker 영향 | 단일 에이전트 prompt 에 RT 진행 신호 미포함 | ?? RT marker 가 *새 round 시작* 신호로 오인 가능성 |

### 검증 fact 후보

- main conv 의 messages 테이블 SELECT — 단일 질의 직전의 RT 메시지가 같은 conversation_id 에 persona 별로 어떻게 누적되는지
- ContextPack assembly trace — 단일 에이전트 dispatch 시 prompt 본문에 RT brief 가 포함되는지 (없으면 보고 #1 root cause 일치)
- `~/.tunaflow/logs/` 의 prompt build 로그 (있으면)

## 2. 시나리오 B — 라운드 길어지면 합의 망각, 매 라운드 같은 합의 재시도 (보고 #2)

### 사용자 보고 (원문 인용)

> "라운드가 길어지면 합의 했던 것을 잊는지 다시 합의를 시도하며, 해당 내용이 purposer에게 전달이 안 되는건지 지속적으로 해당 합의를 다시 시도합니다. 합의했다고 대여섯번을 전달했으나 꾸준히 해당 내용에 대한 합의를 시도했습니다. 3번 fail 하면 사용자 선택으로 넘기기 때문에, 매번 사용자 선택이 필요하다고 하며, 매번 진행한 것이라고 전달해도 다음 라운드에서 다시 합의되지 않았으므로 fail -> 사용자 선택으로 넘어갑니다. 이후부터 더이상 라운드 테이블을 사용하지 않게 됐습니다."

### 재현 절차

1. **Plan 생성** — 5 라운드 이상 길어질 만한 주제 ("multi-engine ContextPack parity + edge case 5종 처리" 등)
2. **RT 시작** — Sequential 모드, 참여자 3명
3. **첫 합의 도달** — 1~2 라운드 안에 *어느 한 axis* 에 대해 합의 도출. synthesizer brief 의 합의 항목 캡처
4. **계속 진행** — 라운드 3, 4, 5 ... 동일 RT 안에서 추가 axis 합의 시도. 매 라운드 종료 시 synthesizer brief 관찰
5. **합의 메모리 회귀 관찰** — 라운드 4 이상 진행 시 *이전 라운드에서 합의됐던 axis* 가 synthesizer brief 에 다시 *"합의 시도 중"* / *"미합의"* 로 표시되는지
6. **3 fail 임계값 트리거** — 같은 합의가 3 라운드 이상 *"fail"* 로 누적되어 사용자 fallback dialog 가 뜨는지 (이미 합의됐는데도)
7. **사용자 fallback 응답** — *"이미 합의된 항목"* 으로 사용자 답변 → 다음 라운드에서 같은 합의 다시 fail 트리거되는지

### 기대 동작 vs 실제 동작

| 항목 | 기대 | 실제 (보고) |
|---|---|---|
| 라운드 N+1 의 prompt | 라운드 1~N 합의 항목이 *"이미 합의됨, 재논의 불필요"* 로 명시 | ❌ 이전 라운드 응답만 transcript 로 포함, 합의 메타 별도 영역 없음 |
| synthesizer 판단 | 누적 합의 위에서 새 axis 만 평가 | ❌ 매 라운드 fresh state, 같은 합의 재시도 |
| 3 fail 임계값 | 새 axis 기준 fail 만 카운트 | ❌ 이미 합의된 axis 의 *"미합의"* 환각도 fail 로 카운트 |
| 사용자 fallback 영구성 | 사용자 *"이미 합의됨"* 답변이 다음 라운드 prompt 에 반영 | ❌ 다음 라운드 reset, 같은 fallback 반복 |

### 검증 fact 후보

- `roundtable.rs:64~166 run_synthesizer_after_round()` — synthesizer 가 받는 input 에 `round_responses` (현 라운드) 만 있고 `consensus_history` 같은 누적 합의 없는지
- `roundtable_helpers/prompt.rs:23~74 build_round_prompt_with_identity()` — *"Prior round responses"* 섹션은 있는데 *"Consensus reached so far"* 섹션 부재인지
- `roundtable_helpers/persist.rs save_shared_brief()` — 합의 항목이 별도 분리된 schema (consensus_id, axis, status="agreed") 로 persist 되는지 vs 단순 brief 텍스트 1건만 저장되는지
- DB messages 테이블 — synthesizer 응답이 plain text 인지 structured 한지 (consensus 항목 별도 컬럼 없으면 fresh-state 가설 확증)

## 3. 시나리오 C — Architect 가 RT 대화 내역 접근 못 함 (보고 #3)

### 사용자 보고 (원문 인용)

> "어느 정도 합의된 내용을 정리해서 실제 설계안->플랜 작성까지 진행하고 싶었는데 설계자는 라운드 테이블 대화 내역에 접근할 수 없었으며, 라운드 내부에서 설계자에게 전달할 합의 내용, 설계 정리된 걸 달라고 하면 극히 일부 내용(마지막 라운드 정도)만 정리해서 줍니다. 라운드 테이블의 용도를 제가 착각한 것인지, 아니면 사용성이 떨어지는 상태인건지 모르겠습니다."

### 재현 절차

1. **Plan 생성** — 시나리오 B 와 동일 주제 또는 더 단순한 주제 (3 라운드 안에 합의 도출)
2. **RT 정상 진행 + 합의 도달** — 시나리오 A/B 결과 무시하고 *논리상 합의됐다고 사용자가 인정* 한 시점까지 진행
3. **RT 종료** — Adopt / Done / Close 등 RT 종료 액션 (UI 표면에서 어떻게 종료되는지도 검증 항목)
4. **Architect dispatch** — RT 종료 직후 main conv 또는 새 conv 에서 Architect 에게 *"위 RT 합의 내용 기반으로 plan 초안 작성해줘"* 요청
5. **Architect 응답 관찰** — 응답에 다음 항목이 포함되는지:
   - (a) RT topic 명시
   - (b) 참여자 (claude / codex / gemini) 별 핵심 의견 요약
   - (c) 합의 항목 누적 list
   - (d) plan 초안 (subtasks / invariants 등)
6. **부재 항목 기록** — (a)~(d) 중 부재 항목 = ContextPack RT 인계 갭

### 기대 동작 vs 실제 동작

| 항목 | 기대 | 실제 (보고) |
|---|---|---|
| Architect ContextPack 의 RT 영역 | RT topic + 합의 항목 + 라운드별 핵심 응답 누적 주입 | ❌ "마지막 라운드 정도" 만 = 마지막 synthesizer brief 1건만 |
| RT artifact 영구화 | DB 에 consensus / decisions / round_summary 별도 row | ❌ shared_brief 1 row + transcript (raw messages) |
| Architect 입력 prompt 본문 | RT 결과 명시적 섹션 (Roundtable Consensus / Decisions) | ❌ findings_section 또는 일반 history 에 묻힘 |

### 검증 fact 후보

- `agents_helpers/context_pack/db_queries.rs:173~215 build_findings_section()` — `roundtable_brief` memo 최신 3개만 로드 → 누적 합의 부재
- `agents_helpers/context_pack/db_queries.rs:364~398 build_rt_inheritance_section()` — 함수 내용이 *parent context 만* 다루고 RT 결과 영역 없는지 (서브에이전트 분석 결과)
- Architect dispatch 시 ContextPack assembly trace — RT 영역 섹션이 prompt 본문에 등장하는지

## 4. 환각 vs 정상 분리 매트릭스

각 시나리오에서 *어디까지가 보고된 회귀, 어디부터가 환경/입력 잡음* 인지 구분:

| 관찰 | 보고 일치 | 환경 잡음 가능성 |
|---|---|---|
| 단일 질의가 새 라운드처럼 동작 | A-1 root cause | 사용자 prompt 자체가 *라운드 형식* 일 때는 자연스러움 |
| 라운드 4 이후 같은 합의 재시도 | B-1 root cause | 주제 자체가 *재합의 가능* 한 axis 일 가능성 (복잡 axis) |
| Architect 응답이 마지막 라운드만 정리 | C-1 root cause | RT 종료 시점 timing 차이 (RT 미종료 상태에서 dispatch) |
| RT 종료 후 plan 자동 생성 안 됨 | UI 차원 영역 | RT → plan 자동 트리거 미존재 vs 사용자 클릭 부재 |

## 5. 시나리오 활용 — 검증 단계로 옮겨질 형태

본 문서의 각 시나리오는 plan §3 의 *Verification* 단계로 1:1 옮겨진다:

- **시나리오 A** → Plan B Task NN 의 e2e 검증: *RT 진행 중 단일 에이전트 dispatch 시 ContextPack 에 RT artifact 포함되는지*
- **시나리오 B** → Plan B Task NN 의 e2e 검증: *5 라운드 진행 시 라운드 3 prompt 에 라운드 1~2 합의 명시 포함되는지*
- **시나리오 C** → Plan B Task NN 의 e2e 검증: *RT 종료 후 Architect dispatch 시 응답에 RT 누적 합의 등장하는지*

회귀 가드:
- fix 후 같은 시나리오 다시 실행 → *기대 동작* 칸과 일치
- 부분 fix (한 영역만) 시 다른 시나리오는 그대로 fail 유지 → 영역 격리 확인

## 6. 사용자 회신 timing

devbug 외부 사용자에게 진행 상황 안내:

- 본 시나리오 문서 + plan 머지 후: *"보고하신 3 영역 RT 회귀의 root cause 가설 + 재현 시나리오 정리 완료. 본 plan 의 fix 진행 후 v0.1.X-beta 에서 자가 회복 path 회복 안내드림"*
- 시나리오 검증 결과 (사용자 환경에서 재현되는지) 사용자 회신 받으면 plan §0.3 가설 확정/탈락 확정
- 시나리오 자체에 *"환경 차이로 재현 안 되는 항목"* 발견 시 별 추가 issue 분기

## 7. Fix 후 회복 결과 (PR-1 + PR-2 + PR-3 머지)

| 시나리오 | Fix 진입점 | 회복 메커니즘 | 회귀 가드 unit test |
|---|---|---|---|
| **A** (단일 질의 시 합의 무시) | PR-2 Task 03 | `messages.rt_round_index` 컬럼 + `load_recent_messages_excluding_rt()` 가 single agent dispatch 시 RT round transcript 를 *주제별 컨텍스트* 영역에서 제외. 합의는 PR-2 Task 04 의 `build_rt_consensus_section()` 으로 별 섹션 인계 | `single_agent_dispatch_skips_rt_transcript`, `legacy_loader_includes_rt_messages`, `architect_context_pack_includes_consensus_section` |
| **B** (라운드 길어지면 합의 망각) | PR-1 Task 02 | `roundtable_consensus` 테이블 영구화 + synthesizer 가 `<!-- tunaflow:consensus -->` JSON fence 또는 `## Agreed axes` markdown 으로 합의 추출 → `save_consensus()` row 누적 → 라운드 N+1 prompt 의 *"## Consensus reached so far"* 섹션이 라운드 1~N 합의 명시 포함 | `consensus_persisted_across_rounds`, `consensus_isolated_per_conversation`, `next_round_prompt_includes_prior_consensus`, `extract_consensus_from_json_marker`, `extract_consensus_from_markdown_fallback` |
| **C** (Architect 가 RT 대화 내역 접근 못 함) | PR-2 Task 04 | `build_rt_consensus_section()` 이 `roundtable_consensus` 테이블 조회 → ContextPack assembly 의 findings 다음에 *"## Roundtable Consensus"* 섹션 push. branch shadow conv 도 부모 conv 검색에 cover | `architect_context_pack_includes_consensus_section`, `consensus_includes_branch_shadow_conversations`, `rt_consensus_section_assembled_into_prompt_and_meta` |

회귀 가드 e2e (사용자 환경 검증으로 위임):
- 시나리오 A 의 *실 동작 환각 표면* (단일 질의가 라운드 재실행 흉내) 가 fix 후 *"단일 에이전트가 합의 인지 + follow-up answer"* 로 회복되는지
- 시나리오 B 의 *5 라운드 이상 진행 시* 같은 합의 재시도 환각이 사라지는지
- 시나리오 C 의 *Architect 응답에 라운드 1~N 누적 합의 + 참여자 의견 등장* 하는지

위 3 영역은 mcp 환경 차단으로 Architect 직접 e2e 캡처 불가 — v0.1.7-beta release 후 사용자 (devbug) 환경 e2e 검증으로 최종 확인 (Plan §6 / 핸드오프 §5).

## 8. 변경 이력

- 2026-05-07 초안 작성 (devbug #263 보고 직후, plan 작성 동시 시점)
- 2026-05-07 PR-1 (#265) + PR-2 (#266) + PR-3 머지 후 §7 Fix 후 회복 결과 추가
