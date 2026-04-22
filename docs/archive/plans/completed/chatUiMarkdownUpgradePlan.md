# tunaFlow 채팅 UI / Markdown 고도화 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 09:24 KST

## 목적

`tunaFlow`의 현재 채팅 화면은 작업 흐름과 멀티에이전트 기능은 강하지만, 메시지 렌더링과 Markdown 표현력은 아직 상대적으로 단순하다. 반면 `tunaChat`는 `react-markdown + remark-gfm` 기반의 풍부한 채팅 렌더링, 커스텀 Markdown 컴포넌트, 파일/문서 뷰를 갖고 있다.

이 문서는 `tunaChat`의 채팅 UI/Markdown 구조를 참고해, `tunaFlow`에 어떤 요소를 우선 도입하면 좋은지 정리한다.

## 실제 참고 소스

`tunaChat`에서 확인된 핵심 파일:

- `D:\privateProject\tunaChat\client\src\components\chat\MessageView.tsx`
- `D:\privateProject\tunaChat\client\src\components\chat\MarkdownComponents.tsx`
- `D:\privateProject\tunaChat\client\src\components\chat\FileViewer.tsx`
- `D:\privateProject\tunaChat\client\package.json`

확인된 사실:

- `react-markdown` 사용
- `remark-gfm` 사용
- 메시지 렌더링은 `MessageView.tsx`
- Markdown 세부 표현은 `MarkdownComponents.tsx`
- Markdown/문서 파일 뷰는 `FileViewer.tsx`

## tunaChat에서 참고할 핵심 포인트

### 1. Markdown 렌더링 스택

`tunaChat`는 아래 조합을 쓴다.

- `react-markdown`
- `remark-gfm`
- custom markdown components

이 조합으로 다음이 자연스럽게 된다.

- 코드 블록
- 인라인 코드
- 표
- 체크리스트
- 링크
- 문단/리스트 스타일

### 2. 메시지 렌더링과 Markdown 렌더링 분리

`MessageView.tsx`가 메시지 컨테이너 역할을 하고,
실제 Markdown 표현은 `markdownComponents`에 위임한다.

이 구조의 장점:

- 채팅 bubble UI와 Markdown 표현을 분리할 수 있음
- 이후 코드 블록/링크/파일 링크를 독립적으로 고도화 가능

### 3. 파일/문서 뷰어 확장성

`FileViewer.tsx`는 `.md`, `.mdx`, `.markdown` 등 문서 파일을 Markdown으로 보여준다.

이건 `tunaFlow`에서도:

- artifact preview
- memo preview
- docs/preview

와 연결 가능성이 있다.

## tunaFlow에 우선 도입할 가치가 큰 것

### 1순위: Assistant/User 메시지 Markdown 렌더링

가장 먼저 필요한 건 채팅 본문 자체를 Markdown으로 더 잘 보여주는 것이다.

도입 가치:

- 코드 블록 가독성 향상
- 표/리스트/체크박스 표현 개선
- plan, findings, artifact handoff, brief가 훨씬 읽기 쉬워짐

### 2순위: 커스텀 Markdown 컴포넌트

특히 아래를 별도 커스텀하는 것이 좋다.

- code block
- inline code
- links
- blockquote
- table

도입 가치:

- 에이전트 출력의 구조적 정보가 더 잘 보임
- 코드/경로/명령어 가독성이 높아짐

### 3순위: 파일/문서 preview

이건 후순위다.

먼저 채팅 메시지 렌더링부터 좋아져야 하고,
그 다음 artifact/memo/docs preview로 확장하는 것이 맞다.

## tunaFlow에 적용할 권장 방향

### Phase 1. 채팅 본문 Markdown 렌더링

대상:

- 현재 assistant/user 메시지 렌더링 컴포넌트
- 예: `MessageItem.tsx` 중심

도입:

- `react-markdown`
- `remark-gfm`

최소 적용:

- assistant 메시지 먼저
- 이후 필요하면 user 메시지도 동일 처리

### Phase 2. 커스텀 MarkdownComponents 도입

별도 파일 예시:

- `src/components/tunaflow/chat/MarkdownComponents.tsx`

커스텀 대상:

- `code`
- `pre`
- `a`
- `table`
- `blockquote`

### Phase 3. 코드 블록 polish

후속으로:

- 복사 버튼
- 언어 라벨
- 긴 코드 블록 스크롤

### Phase 4. artifact/memo/file preview 확장

이건 마지막 단계다.

## 주의점

### 1. 전부 한 번에 옮기지 말 것

`tunaChat`의 UI를 통째로 복붙하는 게 아니라,
구조와 역할만 참고해서 `tunaFlow`에 맞게 단계적으로 옮겨야 한다.

### 2. 멀티에이전트 UI와 충돌시키지 말 것

`tunaFlow`는 일반 채팅 앱이 아니라:

- branch
- roundtable
- findings
- artifacts
- follow-up

같은 구조를 갖고 있으므로, Markdown 도입이 이 메타데이터 UI를 침범하면 안 된다.

### 3. 먼저 메시지 본문부터

파일 뷰어나 큰 문서 렌더링보다,
현재 채팅 메시지 본문이 먼저다.

## 권장 구현 순서

1. `react-markdown + remark-gfm` 도입
2. assistant 메시지 Markdown 렌더링 적용
3. `MarkdownComponents.tsx` 분리
4. 코드 블록/UI polish
5. artifact/memo preview 확장

## 판단

`tunaFlow`는 기능적으로는 이미 강하지만, 채팅 UI와 Markdown 표현력은 더 좋아질 여지가 크다. `tunaChat`의 메시지 렌더링 구조를 참고해 `MessageItem` 중심으로 단계적으로 도입하는 것이 가장 비용 대비 효과가 크다.
