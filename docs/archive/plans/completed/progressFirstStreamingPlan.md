# tunaFlow Progress-First Streaming 설계

작성 목적:
- `tunaFlow`의 현재 스트리밍 UX를 `tunaChat`의 실제 동작 방식에 맞춰 재구성하기 위한 설계 문서다.
- 목표는 "최종 답변 본문을 스트리밍하는 구조"가 아니라, "툴 사용/진행 로그를 먼저 스트리밍하고 완료 후 최종 답변을 렌더하는 구조"로 전환하는 것이다.

## 현재 문제

현재 `tunaFlow`는 Claude 스트리밍에서 실제 답변 본문을 청크 단위로 갱신한다.

- `claude:chunk` 이벤트가 assistant 본문에 바로 반영됨
- `MessageItem`은 스트리밍 중에도 Markdown 렌더를 태움
- 결과적으로 청크마다 본문 Markdown이 다시 계산됨

이 구조는 다음 문제를 만든다.

- 스트리밍 체감이 느림
- ReactMarkdown/remark-gfm 비용이 청크마다 반복됨
- 스트리밍 중간 상태와 최종 답변의 의미가 섞임

## tunaChat 레퍼런스에서 확인된 구조

`tunaChat`는 아래 방식으로 동작한다.

### 실행 중

- sidecar의 `action` 이벤트를 받음
- 이 값을 `progressContent`에 누적
- UI에서는 `ProgressBlock`으로 마지막 몇 줄만 롤링 표시

즉 실행 중에는 "툴 사용/진행 로그"만 보여준다.

### 완료 후

- `completed` 이벤트에서 최종 `answer`를 한 번만 넣음
- 같은 메시지에서
  - 위에는 `progressContent`의 마지막 3줄 축약본
  - 아래에는 최종 답변 Markdown
  을 함께 보여준다

즉 UX는:
- 1단계: 툴 사용 로그 스트리밍
- 2단계: 툴 로그 3줄 요약 + 최종 답변

## 목표 UX

`tunaFlow`도 아래와 같이 바꾼다.

### 스트리밍 중

- assistant 메시지는 `progress-first`
- 툴 사용/생각/단계 로그만 스트리밍
- 최종 답변 본문은 아직 렌더하지 않음

### 완료 후

- 최종 답변을 한 번에 넣음
- 같은 메시지 안에서
  - progress 축약본 3줄
  - 최종 답변 Markdown
  을 같이 보여줌

## 데이터 모델 방향

현재 `Message`에는 이미 `progressContent` 필드가 있다.
이걸 본래 용도로 일관되게 쓴다.

권장 의미:

- `content`
  - 완료 후 최종 답변 본문
- `progressContent`
  - 실행 중 누적된 툴/진행 로그
- `status`
  - `streaming` / `done` / `error`

중요:
- 스트리밍 중에는 `content`를 최종 답변 텍스트처럼 키우지 않는다
- `progressContent`만 갱신한다

## 이벤트 모델 제안

### 실행 중 이벤트

- `started`
- `action`
- `note`
- 필요 시 `tool`

이 이벤트는 모두 `progressContent`에 반영한다.

### 완료 이벤트

- `completed`
  - `answer`
  - usage
  - resume token

완료 시에만 최종 답변을 `content`에 기록한다.

## 프론트 렌더링 원칙

### streaming 상태

- `ProgressBlock`만 렌더
- Markdown 본문 렌더 금지

### done 상태 + progressContent 존재

- 위: `ProgressBlock(isDone=true)`로 마지막 3줄만 축약
- 아래: 최종 답변 Markdown 렌더

### done 상태 + progressContent 없음

- 일반 Markdown 메시지 렌더

## 구현 단계

### Phase 1. 렌더 경로 분리

목표:
- 스트리밍 중에는 Markdown 본문 렌더를 하지 않음

포함:
- `MessageItem` 또는 별도 컴포넌트에서 `ProgressBlock` 도입
- `streaming` 상태일 때 plain progress 표시
- 완료 후 최종 답변 Markdown 렌더

완료 기준:
- 스트리밍 체감이 빨라짐
- Markdown은 완료 후에만 무겁게 렌더

### Phase 2. 진행 로그 source 정리

목표:
- Claude 경로에서 실제 툴 사용/진행 로그를 `progressContent`에 넣을 수 있게 함

포함:
- `claude:chunk`를 최종 답변 청크가 아니라 진행 로그 이벤트로 전환하거나
- 별도 `claude:action`/`claude:progress` 이벤트 추가

완료 기준:
- 스트리밍 중 툴 사용이 보임

### Phase 3. 전체 엔진 확장

목표:
- Codex / Gemini / OpenCode도 같은 패턴으로 맞춤

완료 기준:
- 엔진별 스트리밍 UX 일관화

## 주의점

1. 현재 `RoundtableView`는 `progressContent`를 prompt source JSON 용도로도 일부 사용한다.
   - 장기적으로는 의미 분리가 필요하다.
2. 지금 단계에서 가장 큰 체감 개선은 "렌더 분리"다.
   - backend 이벤트를 완전히 다시 설계하지 않아도 1차 개선 가능
3. `tunaChat`처럼 보이게 만들려면
   - 중간 진행 로그
   - 완료 후 3줄 축약
   - 최종 답변 분리
   이 셋이 모두 필요하다

## 구현 후 확인할 것

1. 스트리밍 중 Markdown 본문 렌더가 사라졌는지
2. 툴/진행 로그가 롤링 표시되는지
3. 완료 후 progress 3줄 + 최종 답변이 함께 보이는지
4. 기존 follow-up / branch / memo 액션과 충돌 없는지

## 최종 판단

`tunaFlow`는 `tunaChat`처럼 "progress-first streaming" 구조로 바꾸는 것이 맞다.
이 방식은 빠르고, 툴 사용을 잘 보여주며, 최종 답변 Markdown 렌더 비용을 완료 시점으로 미룰 수 있다.
