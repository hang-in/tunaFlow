# tunaFlow Settings Shell IA 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

메인 워크플로(Chat/Plan/Artifacts/Review/Test)에서 분리되어야 하는
관리 영역을 수용할 Settings 셸을 도입한다.

## 왜 지금 필요한가

1. **Skills**가 사이드바에 임시로 배치되어 있지만, 최종 위치는 Settings
2. **Personas**와 **Agent Profiles** 개념이 도입 예정이지만 배치할 곳이 없음
3. **Runtime 설정**(rawq, daemon, context budget 등)이 코드에 흩어져 있음
4. 메인 워크플로 UI가 정리되면서 관리 영역의 진입점이 필요해짐

## 초기 섹션 구조

```
Settings
  ├─ Agents        ← Agent Profile 관리 (engine + model + persona + default skills)
  ├─ Personas      ← 역할/스타일 정의 (architect, reviewer, tester 등)
  ├─ Skills        ← 기존 SkillsPanel 이식, vendor 그룹핑, active 관리
  └─ Runtime       ← rawq, daemon, context budget, model catalog
```

### 확장 예정 (이번 범위 아님)
- Models — 엔진별 모델 카탈로그 관리
- Projects — 프로젝트 설정 (path, default engine, workspace root)
- Keybindings — 단축키 설정
- Appearance — 테마, 폰트 크기

## MVP 범위

이번 단계는 **Settings Shell**만 구현:

1. 진입 버튼: 사이드바 하단 좌측에 설정 아이콘 (⚙️)
2. Settings 화면: 전체 화면 오버레이 또는 메인 패널 교체
3. 좌측: 섹션 네비게이션 (Agents / Personas / Skills / Runtime)
4. 우측: 본문 영역 — 각 섹션의 placeholder 또는 기존 컴포넌트 이식

### Skills 섹션
- 기존 `SkillsPanel` 컴포넌트를 Settings > Skills에 이식
- 사이드바에서 Skills 섹션 제거

### 나머지 섹션
- Agents: placeholder ("Agent Profiles will be configured here")
- Personas: placeholder
- Runtime: placeholder (향후 rawq 설정, context budget 등)

## 비목표

- Agent Profile CRUD 구현
- Persona 편집 기능
- 실제 Runtime 설정 반영
- Settings 데이터 persistence (DB/파일 저장)

## UI 구조

```
┌─────────┬──────────────────────────────┐
│ Sidebar │ [Chat] [Plan] [Art] [Rev]    │
│         │                              │
│         │ (main workspace)             │
│         │                              │
│ ⚙️      │                              │  ← 사이드바 하단 아이콘
└─────────┴──────────────────────────────┘

⚙️ 클릭 시:
┌─────────────────────────────────────────┐
│ Settings                          [✕]  │
│ ┌──────────┬──────────────────────────┐ │
│ │ Agents   │                          │ │
│ │ Personas │  (선택된 섹션 내용)       │ │
│ │ Skills ● │                          │ │
│ │ Runtime  │                          │ │
│ └──────────┴──────────────────────────┘ │
└─────────────────────────────────────────┘
```

## 완료 기준

1. 사이드바 하단에 설정 아이콘이 있음
2. 클릭 시 Settings 화면이 열림
3. 4개 섹션 네비게이션이 동작함
4. Skills 섹션에 기존 SkillsPanel이 이식되어 있음
5. 나머지 섹션에 placeholder가 표시됨
6. 닫기 버튼으로 워크스페이스로 복귀
