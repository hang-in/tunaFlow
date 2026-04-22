# tunaFlow 패널 / 드로어 UX 재설계 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 20:18 KST

## 목적

현재 `tunaFlow`의 화면 구조는 다음과 같다.

- 좌측 `Sidebar`
- 중앙 `ChatPanel`
- 중앙 우측 고정폭 `BranchThreadPanel`
- 우측 고정폭 `ContextPanel`

이 구조는 기능이 늘어날수록 다음 문제가 생긴다.

1. 좌우 패널이 고정폭이라 사용자의 작업 맥락에 맞게 조절되지 않는다
2. thread / RT panel이 같은 레벨의 분할 패널처럼 붙어 있어 "한 단계 위 레이어" 느낌이 약하다
3. 우측 패널에 plan / reviews / tests / artifacts / trace까지 들어오면 과적재될 가능성이 높다
4. 현재 UI 기능 정보가 산만하게 분산돼 있어, 사용자가 무엇을 먼저 봐야 하는지 직관이 약하다

이 문서는 패널 리사이즈, thread/RT drawer, 우측 workspace panel, 그리고 UI 정리 절차를 함께 제안한다.

## 현재 구조 확인

실제 코드 기준:

- `src/components/tunaflow/AppShell.tsx`
  - `Sidebar | ChatPanel + BranchThreadPanel | ContextPanel`
- `src/components/tunaflow/BranchThreadPanel.tsx`
  - 현재 `w-[480px]` 고정폭
  - 메인 영역 내부에서 일반 flex item처럼 렌더
- `src/components/tunaflow/ContextPanel.tsx`
  - 현재 `w-64` 고정폭
  - `Branches / Assets` 탭 구조

즉 현재는 "3분할 + 중앙 보조 패널"에 가깝다.

## UX 원칙

### 1. 좌우 패널은 리사이즈 가능해야 한다

다른 IDE처럼 사용자가 작업 상황에 맞게 폭을 조절할 수 있어야 한다.

권장:

- `Sidebar`
  - 최소: `220px`
  - 기본: `260px ~ 300px`
  - 최대: `360px`
- `Workspace Panel`(기존 ContextPanel)
  - 최소: `280px`
  - 기본: `320px ~ 400px`
  - 최대: `520px`

### 2. Thread / RT는 상위 레이어 drawer여야 한다

thread/RT는 일반 분할 패널보다 **overlay drawer**에 가까워야 한다.

권장 형태:

- `ContextPanel`의 왼쪽 경계에서 펼쳐지는 drawer
- 중앙 채팅 위에 올라오는 상위 레이어
- 닫히면 중앙 채팅이 본체로 복귀
- 열릴 때 부모 메시지 preview를 상단 고정

즉 현재의 `BranchThreadPanel`을 고정폭 flex item으로 두지 말고,
`thread drawer`로 재구성하는 것이 맞다.

### 3. Thread / RT drawer는 최대 폭 제한이 있어야 한다

사용자가 drawer를 크게 키우더라도 메인 채팅 본체를 완전히 먹어버리면 안 된다.

권장 제한:

- 최소 폭: `360px`
- 기본 폭: `420px ~ 520px`
- 최대 폭:
  - 중앙 채팅 가용 영역의 `90%`
  - 또는 왼쪽 Sidebar에서 최소 `10%` 이상은 항상 보이게 유지

실무적으로는:

- `maxWidth = centerAreaWidth * 0.9`
- 또는 `mainVisibleWidth >= 120px ~ 160px` 보장

정도로 잡는 것이 좋다.

### 4. 중앙 상단 탭은 채팅 객체만 가진다

Harness 관점에서도 탭은 대화 객체 전환용이어야 한다.

권장:

- `Architect`
- `Developer Branch`
- `RT Branch`
- 필요 시 `Reviewer Thread`

### 5. 우측 패널은 workspace panel이어야 한다

우측 패널은 단순 정보 적층 영역이 아니라,
현재 작업 단계에 따라 전환되는 작업 패널이어야 한다.

권장 모드:

- `Plan`
- `Reviews`
- `Tests`
- `Artifacts`
- `Trace`

중요:

- 한 번에 하나의 주 모드만 크게 보여준다
- 나머지는 badge/count/summary만 노출한다

## Slack / Discord 감각으로 풀기

직접 제품 도움말/커뮤니티 흐름을 참고했을 때, 중요한 공통점은 다음과 같다.

### Slack 감각

- 메인 채널/대화를 유지한 채 thread를 옆에서 본다
- 필요하면 더 크게 보는 별도 보기로 확장한다
- 원본 메시지 맥락을 잃지 않는 것이 중요하다

### Discord 감각

- thread는 원 채널과 분리되지만, 완전히 독립된 화면보다 "보조 레이어"에 가깝다
- 원본 메시지와 thread 사이의 관계가 분명해야 한다

따라서 `tunaFlow`에서는:

- thread/RT를 메인 대화와 연결된 drawer로
- 부모 메시지 preview를 상단 anchor로
- 필요 시 `Open Full`을 유지

하는 방식이 가장 잘 맞는다.

## 권장 정보 구조

### 중앙

- 메인 채팅
- 채팅 객체 탭
- 입력창

### 우측 workspace panel

- 현재 작업 단계에 맞는 단일 모드
- plan 승인 / review 판정 / test 확인 / artifact 탐색 / trace 확인

### drawer

- branch thread
- RT branch
- 향후 reviewer thread

즉 세 층으로 나눈다.

1. 대화 중심 중앙
2. 작업 중심 우측 panel
3. 보조 대화 중심 drawer

## UI 산만함 문제에 대한 접근

현재 `tunaFlow`는 기능은 많아졌지만, 아래 문제가 있다.

- 같은 정보가 여러 위치에 약하게 반복됨
- 현재 단계에서 가장 중요한 액션이 무엇인지 약함
- owner / plan / branch / rawq / review 같은 메타가 채팅 정보와 경쟁함

따라서 정리는 단순 "예쁘게"가 아니라,
**정보 구조와 우선순위 재정의** 문제로 접근해야 한다.

## Claude와 자료를 주고받으며 정리하는 권장 방식

### 1. 먼저 현황을 artifact로 만든다

Claude에게 바로 "UI 정리해줘"라고 던지지 말고,
아래 자료를 먼저 구조화해서 주는 편이 좋다.

권장 준비물:

- 현재 화면 스크린샷 3~5장
  - 기본 메인 채팅
  - thread 열린 상태
  - RT 상태
  - 우측 패널 asset/plan 상태
- 현재 컴포넌트 맵
  - `Sidebar`
  - `ChatPanel`
  - `BranchThreadPanel`
  - `ContextPanel`
  - 주요 하위 패널
- 불편 목록
  - "무엇이 안 보이는지"
  - "무엇이 너무 경쟁하는지"
  - "어디서 맥락이 끊기는지"

### 2. 문제를 미학이 아니라 작업 흐름으로 설명한다

예:

- "사용자가 branch를 열었을 때 메인 채팅과 branch의 관계가 잘 안 보인다"
- "우측 패널에 기능이 많아질수록 현재 액션이 묻힌다"
- "thread가 중앙 분할처럼 보여 보조 대화 레이어 느낌이 약하다"

즉 "예쁘게 바꿔줘"보다
"작업 흐름상 무엇이 모호한지"를 전달해야 한다.

### 3. Claude에는 한 번에 하나의 UX 질문만 던진다

권장 순서:

1. 패널 레이아웃과 크기 정책
2. thread/RT drawer UX
3. workspace panel 정보 구조
4. 각 모드별 우선 액션

이 순서가 좋다.

### 4. 결과는 반드시 구조화해서 받는다

Claude에게 받을 산출물 형식:

- 문제 진단
- 제안 구조
- 왜 그런지
- 버려야 할 요소
- MVP 우선순위

가능하면 wireframe 수준 텍스트까지 요구하면 좋다.

### 5. 마지막엔 다시 tunaFlow 코드 기준으로 환원한다

Claude가 제안한 UI가 좋아 보여도,
실제 적용은 반드시 아래 기준으로 재검토해야 한다.

- 현재 `AppShell.tsx`에서 무리 없이 옮길 수 있는가
- 현재 store/state 구조와 맞는가
- RT / branch / plan / review 흐름을 깨지 않는가

즉 Claude는 구조 제안자이고,
최종 적용안은 코드베이스 기준으로 다시 줄여야 한다.

## 권장 구현 순서

### Phase 1. 패널 리사이즈

- Sidebar resize
- Workspace panel resize
- width 상태 저장

### Phase 2. BranchThreadPanel → Drawer 전환

- fixed flex panel 제거
- overlay drawer로 이동
- 최소/기본/최대 폭 제한
- resize handle 추가

### Phase 3. RT도 같은 drawer 규칙 공유

- RT branch와 일반 thread의 표시 규칙 통일

### Phase 4. ContextPanel → Workspace Panel 재구성

- `Plan / Reviews / Tests / Artifacts / Trace` 모드 전환
- 단일 모드 중심 표시

### Phase 5. 정보 구조 정리

- badge/count
- 현재 단계 중심 액션
- 중복 메타 제거

## 테스트 포인트

### 레이아웃

- 최소/최대 폭 제한이 정상 동작하는지
- 화면 크기 축소 시 패널이 과도하게 겹치지 않는지
- drawer가 중앙 채팅을 완전히 먹지 않는지

### 상태 유지

- width 저장/복원
- thread 열기/닫기 후 상태 유지

### 작업 흐름

- 메인 채팅 중 thread 열기
- RT branch 열기
- 우측 패널 모드 전환
- 계획 승인 / review / tests 흐름에서 정보 접근성이 개선되는지

## 현재 판정

지금 필요한 것은 단순한 스타일 개선이 아니라,

- 패널 구조 재배치
- drawer 레이어 도입
- workspace panel 정보 구조 정리

다.  

따라서 구현도 "BranchThreadPanel width 조정" 같은 작은 수정 하나로 끝내기보다,
패널 시스템과 정보 구조를 함께 다루는 것이 맞다.
