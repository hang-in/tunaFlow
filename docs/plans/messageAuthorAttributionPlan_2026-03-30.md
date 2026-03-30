# Message Author Attribution Plan

상태: 제안
작성: 2026-03-30

## 문제

`Agent Identity Framing`으로 현재 응답 중인 에이전트의 자기소개 규칙은 보강됐지만, 멀티에이전트 대화에서는 여전히 아래 혼동이 남는다.

- 현재 응답 중인 에이전트가 과거 2~3개의 답변도 모두 자신의 이전 답변이라고 오해
- 과거 메시지의 실제 작성자와 현재 speaker를 구분하지 못함
- 사용자가 `방금 전 3개의 대답은 누가 한 거야?`라고 물으면, 현재 응답 주체가 과거 메시지의 작성자까지 자기 자신으로 흡수

이 문제는 self-identification과 별개로 **message attribution 규칙 부재**에 가깝다.

## 목표

에이전트가 과거 메시지 작성자를 설명할 때 아래를 분리하도록 만든다.

1. `현재 응답 중인 speaker`
2. `과거 메시지의 실제 작성자`
3. `현재 speaker는 과거 메시지를 참조/검토 중이라는 관계`

## 핵심 원칙

### 1. 현재 speaker와 과거 author는 다를 수 있다

- 현재 응답 중인 에이전트는 `지금 말하는 주체`
- 과거 메시지 작성자는 message meta 기준의 별도 주체

### 2. conversation continuity는 author ownership이 아니다

- `이 대화를 이어간다`는 규칙은 과거 메시지를 참고한다는 뜻이지
- 과거 메시지의 작성권까지 흡수한다는 뜻이 아니다

### 3. 작성자 설명은 message meta 우선

- 가능하면 DB/message meta의 profile label 기준으로 설명
- 추측해서 `다 제 답변입니다`라고 단정하지 않는다

## 권장 응답 규칙

### 사용자가 현재 speaker를 물을 때

- `현재 응답 중인 에이전트는 {current_profile}입니다.`

### 사용자가 과거 메시지 작성자를 물을 때

- `방금 전 3개의 답변 작성자는 각각 {author_1}, {author_2}, {author_3}입니다.`
- `저는 지금 그 답변들을 검토/참조하고 있는 {current_profile}입니다.`

### 금지 형식

- `방금 전 답변도 모두 제 이전 답변입니다`
- `같은 세션이므로 다 제가 한 답변입니다`
- author metadata를 무시하고 현재 speaker 중심으로 덮어쓰는 표현

## 구현 범위

1. identity/prompt 규칙에 speaker vs author 분리를 추가
2. message author를 참조할 수 있는 경로가 있으면 그 메타를 우선 사용
3. author 정보가 불분명하면 추측하지 않고 한계를 말하게 한다

## 비목표

- 전체 대화 모델 재설계
- message schema 개편
- 새로운 author DB 컬럼 추가

## 성공 기준

- `방금 전 3개의 대답은 누가 한거야?` 같은 질문에 현재 speaker와 과거 author를 분리해서 답한다
- 현재 speaker가 과거 메시지의 소유권을 잘못 주장하지 않는다
- author 메타가 있으면 그것을 우선 사용한다

## 후속

필요 시 다음 단계로 이어질 수 있다.

1. message meta를 prompt에 더 명시적으로 포함
2. multi-agent conversation attribution 표시 강화
