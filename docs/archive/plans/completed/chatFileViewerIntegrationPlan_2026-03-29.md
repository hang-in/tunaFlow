# tunaFlow Chat FileViewer Integration Plan

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 제안

## 목표

채팅 메시지 안 파일 경로와 문서 참조를 클릭 가능한 UI로 만들고,
간단한 파일 preview를 제공한다.

## 배경

코드 대화에서는 아래 패턴이 자주 나온다.

- `src/foo/bar.ts`
- `/absolute/path/file.md`
- `README.md:12`

현재 `tunaFlow`는 이 텍스트를 그냥 보여주기만 한다.
`tunaChat`는 FileViewer로 연결해 협업 흐름을 더 짧게 만든다.

## 구현 범위

### Phase 1. 경로 감지

- inline code 안 파일 경로 감지
- 일반 텍스트 안 파일 경로 감지
- file path href 처리

### Phase 2. FileViewer

- markdown/text/code 파일 preview
- path copy
- content copy
- 현재 프로젝트 path 기준 상대경로 resolve

### Phase 3. 채팅 연결

- `ChatPanel` 또는 상위 레벨에 viewer mount
- markdown components에서 viewer open

## 주의

- 보안상 외부 링크와 로컬 파일 링크를 구분해야 한다
- 프로젝트 외부 경로 허용 범위는 실제 Tauri command 기준으로 맞춘다
- 대형 IDE 수준 편집 기능은 이번 범위가 아니다

## 검증

1. 메시지 내 파일 경로 클릭 시 preview가 열림
2. markdown 파일은 markdown으로, 일반 파일은 plain text/code로 표시
3. 상대경로가 현재 프로젝트 기준으로 resolve됨

