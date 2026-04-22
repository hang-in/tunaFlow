# tunaFlow Sidebar Workspace Hierarchy 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

현재 좌측 Sidebar는 `프로젝트 선택`과 `선택된 프로젝트의 채팅/브랜치 탐색`이 한 덩어리처럼 보여서
위계가 잘 드러나지 않는다.

이 문서는 아래 구조를 1차 목표로 한다.

```text
[📁 Workspace ▼]   <- 프로젝트 드롭다운
  + Add project    <- 드롭다운 내부 액션

CHATS              <- 선택된 프로젝트의 채팅 트리
  Main
    b1
    Test
    원탁회의...

ARTIFACTS
FILES
```

## 핵심 판단

1. `Project`는 탐색 트리 항목이 아니라 전역 컨텍스트 선택기다
2. `Chats / RT / Branches`는 선택된 프로젝트의 하위 트리다
3. `Add project`는 프로젝트 선택 UI 안에 있어야 자연스럽다

## 현재 문제

- 프로젝트 목록과 채팅 목록이 같은 레벨처럼 보임
- 사용자는 "지금 어느 프로젝트 안을 보고 있는지" 즉시 파악하기 어렵다
- 새 프로젝트 생성 액션이 탐색 영역 안에 섞여 있다

## 목표 구조

### 1. Workspace selector

- Sidebar 최상단에 `Workspace` 드롭다운
- 현재 선택 프로젝트 이름 표시
- 드롭다운 내부에 프로젝트 목록
- 드롭다운 하단에 `+ Add project`

### 2. Chats tree

- 드롭다운 아래에는 선택된 프로젝트의 채팅만 노출
- Main conversation 아래에 branch / RT를 트리로 정리
- 현재 선택 항목이 트리에서 명확히 강조돼야 함

### 3. Files / Artifacts

- 프로젝트 기준 보조 탐색 섹션
- Chats와 시각적으로 분리

## 구현 범위

- `Sidebar.tsx`
- `sidebar/ChatsSection.tsx`
- 필요 시 project selector용 새 작은 컴포넌트

## 비목표

- 전체 Sidebar 전면 재설계
- Skills 최종 위치 결정
- Artifacts 트리 고도화
- Files 패널 구조 변경

## 완료 기준

1. 프로젝트 선택이 드롭다운 하나로 분리됨
2. Add project가 드롭다운 안으로 들어감
3. Chats는 선택된 프로젝트의 하위 트리처럼 보임
4. 프로젝트와 채팅의 위계가 즉시 읽힘

