# tunaFlow Mobile — Claude.ai/design 재설계 프롬프트

> 타깃 도구: `claude.ai/design/` (2026-04 런칭)
> 이전 시도: `mobileDesignPrompt.md` (Stitch/v0 기반, 결과 UX가 도메인 모델을 반영 못해 폐기)
> 범위: 별도 프로젝트 `/Users/d9ng/privateProject/tunaflow-mobile/` 에서 새 구현 기반이 될 시안 산출

아래 본문을 복사해서 `claude.ai/design` 에 그대로 붙여넣으세요. 본문은 한국어 + 영어 도메인 식별자 혼용으로, tunaFlow 문서 관례를 따릅니다.

---

## 1. 제품의 본질

tunaFlow는 **다중 에이전트 오케스트레이션 클라이언트(AOC)** 입니다. 하나의 프로젝트 안에서 Claude / Codex / Gemini / OpenCode 같은 CLI 기반 코딩 에이전트 여러 개를 동시에 운용하며, 사람이 방향을 결정하고 에이전트가 실행합니다. 이 앱의 슬로건은 "**Of the agent, By the agent, For the agent**" — 에이전트가 불필요한 토큰 낭비 없이 정확한 맥락에서 작업할 수 있도록 돕는 것이 모든 UI 결정의 기준입니다.

이미 Tauri 기반 데스크톱 앱은 존재하며, **이번 작업은 그 모바일 버전**입니다. 모바일은 데스크톱을 1:1 복제하지 않습니다. 이동 중·회의 중·자리에 없을 때 사용자가 **진행 현황을 파악하고, 결정을 내리고, 짧은 지시를 남기는** 데 최적화된 별도 클라이언트입니다.

## 2. 타깃 사용자와 사용 시나리오

사용자는 프로젝트 오너(대개 개발자·기술 리더)입니다. 데스크톱에서 긴 세션을 돌려놓고 자리를 떠난 뒤, 다음 중 하나를 모바일에서 처리합니다.

- 긴 작업이 진행 중인지, 중단됐는지, 사람의 판단을 기다리는지 한눈에 확인
- Architect가 제안한 Plan을 읽고 **승인 / 거부 / 수정 요청**
- Reviewer가 낸 verdict를 보고 **rework 지시 또는 done 처리**
- Meta 에이전트가 보낸 알림(예: "doom loop 감지", "Review passed")을 읽고 필요시 본 화면으로 점프
- 떠오른 아이디어를 짧게 메시지로 던지기 (긴 코딩은 데스크톱으로)

**모바일은 결정과 모니터링 도구**입니다. 코드 에디터가 아닙니다.

## 3. 반드시 이해해야 할 도메인 모델

아래 개념들은 UI 전반에서 일관되게 등장해야 합니다. 이름을 바꾸거나 "Chat"처럼 일반화하지 마세요.

| 개념 | 의미 |
|---|---|
| **Project** | 최상위 컨테이너. 하나의 코드베이스/제품에 해당. 모든 데이터는 프로젝트 소속 |
| **Conversation** | 프로젝트 안의 대화 단위. 메시지 스트림을 가짐 |
| **Branch** | 대화를 특정 지점에서 분기한 독립 공간. git branch 유사. parent conversation과 관계를 유지하고, "adopt"로 결과를 부모로 병합 가능 |
| **Roundtable (RT)** | Branch의 **협업 모드**. 여러 에이전트가 `sequential`(순차) 또는 `deliberative`(병렬) 로 토론. 각 participant는 engine + model + role을 가짐 |
| **Plan** | 구조화된 작업 계획. phase: `approval` → `implementation` → `review` → `done`. 각 phase마다 담당 role이 다름 |
| **Subtask** | Plan의 하위 단위. 의존성(depends_on)과 병렬 그룹을 가질 수 있음 |
| **Artifact** | 대화에서 생성된 산출물 (brief / decision / review / test / plan_proposal 등). 타입별 색상 구분 |
| **Memo** | 사용자가 메시지에서 뽑아낸 짧은 노트 |
| **Insight** | 프로젝트 수준의 코드·품질 분석 리포트. findings + quadrant 분류 |
| **Skill** | 활성화 가능한 역량 번들 (예: frontend-design, webapp-testing) |
| **Meta** | 프로젝트 전반을 관찰하는 보조 에이전트. inbox 알림과 상담 채팅 제공. **제안만, 쓰기 금지** |
| **ContextPack** | 매 요청마다 자동 조립되는 normalized 컨텍스트. 모드: `Lite` / `Standard` / `Full` |
| **Engine** | `claude` / `codex` / `gemini` / `opencode` / `ollama` / `lmstudio` |
| **Model** | Engine별 구체 모델 (예: `claude-opus-4-7`, `gpt-5`, `gemini-2.5-pro`) |
| **Persona** | 시스템 프롬프트 fragment. 같은 engine으로 여러 persona 운용 가능 |
| **Role** | 에이전트의 역할: `Architect` / `Developer` / `Reviewer` / `Synthesizer` |

## 4. 화면 목록 (첫 릴리스 범위)

| # | 화면 | 목적 |
|---|---|---|
| 1 | **Project Home** | 프로젝트 전환, 진행 중 작업 요약, 최근 대화, 미읽 Meta 알림 |
| 2 | **Conversation List** | 선택된 프로젝트의 대화 목록 (+ roundtables, scratchpad 섹션) |
| 3 | **Chat** | 메시지 스트림, 메타 정보 표시, 입력창, persona/engine 전환 |
| 4 | **Message Actions Sheet** | 메시지 long-press 시: Branch 만들기 / Memo 저장 / 복사 / 공유 |
| 5 | **Plans** | Phase별 필터(Plan Check / Dev / Review / Done) + Plan 카드 리스트 |
| 6 | **Plan Detail** | subtask 트리, 진척도, approval/rework 버튼, 담당 role/engine |
| 7 | **Branch / Roundtable View** | 부모 관계 표시, participant별 round, adopt/archive |
| 8 | **Meta Inbox & Chat** | 알림 리스트 + "메타에게 물어보기" 채팅 |
| 9 | **Insight** | findings quadrant, 리포트 요약 (2차) |
| 10 | **Settings** | Profile / Agents / Personas / Skills / Runtime (핵심만) |

**우선순위**: 1~8이 MVP. 9~10은 이후 확장.

## 5. 핵심 플로우

### 5.1 Plan Lifecycle (앱의 중심 축)

```
Chat에서 Architect에게 요청
  ↓
Architect가 Plan Proposal 생성 (Artifact로 저장)
  ↓ 사용자 결정: 승인 / 거부 / 수정 요청
Plans > Approval
  ↓ 승인 시
implementation branch 자동 생성
  ↓
Developer가 코드 작업 (branch 안에서)
  ↓ 완료 신호
Reviewer가 review branch에서 검토 → verdict
  ↓ 사용자 결정: done / rework
Done 또는 Implementation으로 돌아감
```

모바일에서 이 흐름의 **모든 결정 지점**은 1~2 탭으로 도달 가능해야 합니다.

### 5.2 Branch/RT 흐름

Chat 화면의 메시지에서 "Branch 만들기" → 분기 지점 선택 → 새 Branch 진입 (풀스크린 스택). Branch 안에서 자유롭게 실험. 필요하면 `adopt` 로 결과 요약을 parent로 병합, 또는 `archive` 로 보관. RT는 Branch에서 참가자 선택 UI를 열어 전환.

### 5.3 Meta 알림 흐름

실행 중 발생한 이벤트(review passed, doom loop 경고, architect_redesign_requested 등)가 Meta Inbox에 쌓임. 사용자가 알림 탭 → 해당 Plan/Conversation 으로 점프. 옵션으로 "메타에게 물어보기" → 메타 에이전트와 대화.

## 6. 화면 어디에나 보여야 할 메타 정보

- **메시지 단위**: persona / engine / model / duration / token-or-cost (접힘 가능, 기본 접힌 상태)
- **Plan 카드**: phase 배지 / 진척률 / Architect·Developer·Reviewer 각 engine·model / 최근 이벤트 타임스탬프
- **앱 상단**: 실행 중인 job 수 · 현재 Context Mode(Lite/Std/Full) · 미읽 Meta 알림 수

"누가, 어떤 엔진·모델로, 어느 phase에서 말하는가"가 **항상 추적 가능**해야 합니다. 이것이 이전 시도가 놓친 핵심입니다.

## 7. 인터랙션 패턴 (모바일 네이티브)

- **하단 탭 4개**: Home / Plans / Meta / Settings. Chat은 Home과 Plans에서 각각 진입
- **풀스크린 스택**: Branch/RT 진입 시 데스크톱의 드로어를 모바일 스택 네비게이션으로 번역 (뒤로 제스처로 parent 복귀)
- **바텀 시트**: engine·model·persona 전환, 메시지 액션, RT 참가자 선택
- **큰 Primary 버튼**: 승인 / 거부 / Rework / RT 시작 등 결정 버튼은 화면 하단 고정 영역에 크게 배치
- **Long-press**: 메시지·Plan 카드·Branch 리스트 등의 컨텍스트 메뉴 진입
- **Pull-to-refresh**: 각 리스트 화면에서 최신 상태 동기화
- **FAB**: 각 주요 탭에 새로 만들기 버튼 (New Conversation / New Plan / Ask Meta)

## 8. 비주얼 / 디자인 언어

- **색 체계**: oklch 기반 다크/라이트 듀얼 모드 필수. 기본 다크, 토글로 라이트
- **Agent 색상**: claude / codex / gemini / opencode / ollama / lmstudio 각각 구분 가능한 accent. 메시지 버블 테두리, avatar, role 배지에 사용
- **Status 색상**: approved(green) / rejected(red) / draft(yellow) / archived(gray) / running(blue pulse)
- **Phase 색상**: approval / implementation / review / done — 진행 방향을 시사하는 톤 그라디언트
- **타이포**: 시스템 sans-serif. 메시지 본문은 읽기 편한 prose 스타일(줄간격 여유, 코드블록은 monospace + 스크롤 가능)
- **아이콘**: Lucide 기준 (GitBranch / Users / Bot / Loader2 / CheckCircle2 / XCircle / Inbox / Sparkles / Settings)
- **터치 타깃**: 최소 44pt, 리스트 항목 간 여백 충분
- **정보 밀도**: 데스크톱보다 한 화면당 정보 절반 수준으로. 세부는 탭해서 펼치기

## 9. 명시적 비목표 (하지 말 것)

- 데스크톱의 리사이저블 다중 패널을 흉내내지 말 것. 모바일은 단일 초점 화면
- 사이드바 고정, 드로어 핀 같은 모드 금지
- PTY 터미널, 전체 Trace span 트리, 풀 diff 뷰는 **요약 카드 + "데스크톱에서 열기"** 로 대체
- 데스크톱에 있는 모든 설정을 모바일에 담지 말 것. 이동 중 건드릴 만한 것만 (엔진 선택, 페르소나 전환, 테마, 프로필)
- "Chat 앱" 껍질로 포장하지 말 것. 이 앱은 에이전트 오케스트레이션 콘솔이지 메신저가 아님
- 일반적인 AI 챗봇 UI (ChatGPT 스타일 단일 스레드) 로 단순화 금지

## 10. 이전 시도에서 빠졌던 것들 (반드시 포함)

이전에 생성된 모바일 초안은 다음을 누락해 도메인 모델을 반영하지 못했습니다. 이번엔 모두 포함하세요.

- **Role 기반 워크플로우 시각화**: 지금 누가(Architect/Dev/Reviewer) 뭘 하고 있는가
- **Engine/Model 선택 UI**: 사용자가 persona/engine/model을 선택·전환 가능해야 함
- **Plan phase의 담당자 + 다음 액션**: "phase: review, reviewer: gpt-5, 대기 중" 같은 명시적 상태
- **Branch의 부모 관계**: Branch는 "다른 대화"가 아니라 "분기된 실험 공간". parent 링크/adopt/archive 표시
- **Meta Inbox**: 알림 리스트 + 본 화면으로 점프
- **Insight 진입점**: Findings quadrant 카드 (Critical / Medium / Low × Quality / Architecture / Performance)
- **ContextPack 모드 표시**: 현재 Lite/Std/Full이 뭔지, 탭하면 설명

## 11. 첫 산출물로 받고 싶은 것

첫 번째 생성에서는 아래 **5개 화면**을 우선 받고 싶습니다. 각 화면은 다크·라이트 둘 다.

1. **Project Home** — 프로젝트 선택 + 진행 중 요약 + Meta 알림 배지
2. **Chat** — 메시지 스트림(메타 정보 포함) + 입력창(persona/engine chip bar)
3. **Plan Detail** — subtask 트리 + phase 배지 + 결정 버튼(승인/rework)
4. **Branch/RT View** — parent 네비게이션 + participant 라운드 + adopt/archive
5. **Meta Inbox** — 알림 리스트 + "메타에게 묻기" 진입점

각 화면의 상호작용(탭, long-press, 시트) 이 실제로 가능한 프로토타입이면 좋고, 정적 시안이면 어디에서 어디로 이동하는지 주석 표기를 요청합니다.
