# tunaFlow 메시지 쌍 삭제 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 21:50 KST

## 목적

`tunaFlow`의 일반 대화는 기본적으로

- 사용자 질문 1개
- 에이전트 응답 1개

의 쌍으로 읽히는 경우가 많다.

따라서 개별 메시지 하나만 삭제하는 것보다,
**사용자 질문을 기준으로 user+assistant 쌍을 함께 삭제하는 UX**가 더 자연스럽다.

이 문서는:

1. 왜 메시지 쌍 삭제가 필요한지
2. 어떤 범위에서 안전하게 적용할지
3. 삭제 시 컨텍스트팩/파생 데이터에 어떤 영향이 있는지
4. 단계별 구현 방향

을 정리한다.

## 현재 상태

현재 메시지 command:

- `list_messages`
- `create_user_message`
- `append_assistant_message`
- `update_message_status`

위 command는 존재하지만, **메시지 삭제 command는 아직 없다.**

즉 현재는 conversation 전체 삭제는 가능하지만,
특정 질문/응답 쌍만 제거하는 기능은 없다.

## 왜 쌍 삭제가 맞는가

일반 채팅에서:

- 질문만 남고 답변이 없어지거나
- 답변만 남고 질문이 없어지면

맥락이 쉽게 붕괴된다.

특히 `tunaFlow`는 branch / handoff / context inheritance / plan follow-up이 있어
메시지가 단독 조작되면 사용자가 흐름을 이해하기 어려워질 수 있다.

따라서 1차 UX는:

- **질문을 삭제하면 직후의 assistant 응답도 함께 삭제**
- 또는 **assistant 응답에서 삭제를 눌러도 해당 질문+응답 쌍을 함께 삭제**

가 적절하다.

## 컨텍스트팩 영향

### 메시지 기반 맥락

메시지 row를 실제 DB에서 삭제하면,
이후 컨텍스트팩에서는 해당 내용이 빠진다.

이유:

- 최근 메시지
- 부모 대화 recent turns
- context summary
- cross-session rows

가 모두 `messages` 테이블 조회 기반이기 때문이다.

즉 메시지 삭제는 **이후 실행의 메시지 기반 컨텍스트를 실제로 줄인다.**

### 파생 데이터

다만 아래는 자동으로 사라지지 않는다.

- memos
- artifacts
- branch checkpoint 참조
- adopt summary
- roundtable brief

즉 메시지 쌍 삭제는:

- **메시지 컨텍스트 삭제**
이지,
- **모든 흔적 삭제**
는 아니다.

## 1차 적용 범위

안전한 1차 범위:

1. 일반 chat conversation
2. 상태가 완료된 user + assistant 메시지 쌍
3. 인접한 한 쌍만 삭제

권장 규칙:

- 기준 메시지가 `user`이면:
  - 바로 뒤의 첫 `assistant` done/error 메시지까지 함께 삭제
- 기준 메시지가 `assistant`이면:
  - 바로 앞의 `user` 메시지와 함께 삭제

이렇게 하면 일반 대화 UX에서는 일관성이 높다.

## 1차에서 보류할 것

다음은 처음부터 같이 하지 않는 것이 좋다.

- RT conversation의 다중 participant 응답 삭제
- branch shadow conversation의 복잡한 연쇄 삭제
- memo/artifact cascade delete
- checkpoint/branch 참조 무결성 검사 자동화
- soft delete/history 복원

## 권장 구현 방식

### backend

새 command 예:

- `delete_message_pair(message_id: String)`

역할:

1. 기준 메시지 조회
2. 같은 conversation 안에서 짝 메시지 찾기
3. user/assistant 두 row를 삭제
4. 삭제된 message id 목록 반환 가능

권장:

- transaction 사용
- 짝을 못 찾으면 기준 메시지만 지우지 말고 에러 또는 no-op 판단 명확화

### frontend

적용 위치 후보:

- `MessageItem` action menu

권장 UX:

- 삭제 액션 라벨을 `Delete pair` 또는 `질문/응답 삭제`
- hover action 또는 context menu
- 확인 dialog 한 번

삭제 후:

- 현재 conversation 메시지 목록 reload
- 필요한 경우 memos/artifacts는 그대로 둠

## 향후 확장

### Phase 2

- memo/artifact dangling reference 안내
- branch checkpoint 참조 메시지 삭제 차단 또는 경고
- RT 대화용 삭제 규칙 별도 설계

### Phase 3

- soft delete
- undo
- deleted message placeholder

## 기대 효과

이 기능이 들어가면:

1. 잘못된 질문/응답 한 턴을 정리하기 쉬워짐
2. 이후 컨텍스트팩에서도 해당 대화가 빠짐
3. 대화 히스토리 품질을 사용자가 직접 정리할 수 있음

