# 채팅 가독성 개선 — 메시지 렌더링 + 워크플로우 프롬프트

> Status: idea
> Created: 2026-04-03

---

## 1. 현재 문제

### 1.1 메시지 렌더링 가독성

에이전트가 내놓는 응답(코드 + 테이블 + 리스트 혼합)이 읽기 어렵다.

**폰트가 전반적으로 작다**

| 요소 | 현재 | 일반적 기준 |
|------|------|-----------|
| 메시지 본문 | `text-[13px]` | 14-16px |
| 코드블록 | `text-[12px]` | 13-14px |
| 인라인 코드 | `text-[12px]` | 본문과 동일 |
| 메타데이터 (모델, 시간) | `text-[10px]` | 11-12px |
| 브랜치 뱃지 | `text-[9px]` | 10-11px |

**텍스트 대비가 낮다**

| 요소 | 현재 opacity | 문제 |
|------|-------------|------|
| 본문 | `text-foreground/90` | 10% 손실. 미미하지만 누적 시 피로 |
| 링크 | `text-primary/80` | 어디가 링크인지 구분 약함 |
| 인용문 | `text-muted-foreground/70` | 상당히 흐림. 내용 읽기 어려움 |
| 인라인 코드 배경 | `bg-accent/40` | 본문과 거의 구분 안 됨 |
| 코드블록 테두리 | `border-border/20` | 본문과 코드블록 경계 불명확 |

**메시지 간 간격이 좁다**

| 상태 | 패딩 | 체감 |
|------|------|------|
| 기본 메시지 | `py-1.5` (6px) | 약간 빽빽함 |
| 그룹 메시지 | `py-0.5` (2px) | 메시지가 붙어 보임 |
| Virtuoso row gap | 없음 | 메시지 사이 구분선/공간 없음 |

**prose-sm이 기술 컨텐츠에 부적합**

Tailwind Typography의 `prose-sm`은 블로그/문서용으로 설계됨. 에이전트 응답의 특징:
- 코드블록이 자주 나옴 → 코드 전후 간격 필요
- 테이블이 자주 나옴 → 테이블 전후 간격 필요
- 제목(##, ###)이 자주 나옴 → 제목 위 간격 부족하면 구조가 안 보임
- 리스트 항목이 길고 코드를 포함 → 리스트 간 간격 부족

현재 `prose-sm`의 기본값:
```
p margin: 1.1428em 0    (약 16px at 14px base)
h2 margin-top: 1.7142em (약 24px)
h3 margin-top: 1.5em    (약 21px)
code block margin: 1.7142em 0
```

`prose-sm`이라서 모든 값이 줄어듦 → 기술 컨텐츠에서 구조가 뭉개짐.

---

### 1.2 워크플로우 프롬프트 렌더링

워크플로우에서 자동 생성되는 사용자 메시지가 ASCII 박스로 구성되어 있는데, 마크다운 렌더링에서 깨진다.

**현재 프롬프트 형식** (10+ 곳에서 사용):

```typescript
// workflowOrchestration.ts:189-201
const prompt = [
  `┌─ 구현 시작 ──────────────────────────┐`,
  `│`,
  `│ Plan: "${plan.title}"`,
  `│`,
  `│ 작업 지시서:`,
  ...taskItems,    // `│ □ docs/plans/slug-task-01.md`
  `│`,
  `│ 규칙:`,
  `│ 1. 각 task 파일을 읽고 순서대로 구현`,
  `│ 2. 각 완료 시 <!-- subtask-done:N -->`,
  `│ 3. 전체 완료 시 <!-- impl-complete -->`,
  `└──────────────────────────────────────┘`,
].join("\n");
```

**ASCII 박스가 깨지는 이유**:

1. **마크다운 줄바꿈 규칙**: `\n` 하나는 공백으로 처리됨. 줄바꿈하려면 `\n\n` 또는 줄 끝 공백 2개 필요
2. **가변폭 폰트**: Inter 폰트에서 `┌──┐` 박스 너비가 어긋남
3. **사용자 메시지는 조건부 마크다운**: `hasMarkdownSignal()` 감지 시에만 마크다운 렌더링. ASCII 박스는 마크다운 신호로 감지되지 않을 수 있음

**영향 받는 파일**:

| 파일 | 프롬프트 | 용도 |
|------|---------|------|
| `workflowOrchestration.ts:189` | `┌─ 구현 시작` | Developer 실행 프롬프트 |
| `PlanProposalCard.tsx:132` | `┌─ 문서 작성 요청` | Plan 승격 후 문서 작성 |
| `SubtaskReviewView.tsx:101` | `┌─ Plan 문서 반영` | Plan 문서 변경 반영 |
| `SubtaskReviewView.tsx:142` | `┌─ Subtask 수정 요청` | Subtask 수정 |
| `SubtaskReviewView.tsx:177` | `┌─ Subtask 논의` | Subtask 토론 |
| `SubtaskReviewView.tsx:212` | `┌─ 작업 지시서 작성` | Task 지시서 생성 |
| `DevProgressView.tsx:183` | `┌─ 리뷰 요청` | Review RT 시작 |
| `DevProgressView.tsx:338` | `┌─ Rework` | Rework 재시작 |

---

## 2. 개선 방향

### 2.1 메시지 렌더링: 최소 변경으로 최대 효과

변경이 적은 순서대로, 각각 독립 적용 가능.

#### A. 폰트 크기 상향

```
메시지 본문:   text-[13px] → text-sm (14px)
코드블록:      text-[12px] → text-[13px]
인라인 코드:   text-[12px] → text-[13px]
메타데이터:    text-[10px] → text-[11px]
```

변경 파일: `MessageItem.tsx`, `MarkdownComponents.tsx`, `MessageMeta.tsx`

#### B. 텍스트 대비 개선

```
본문:          text-foreground/90 → text-foreground
링크:          text-primary/80 → text-primary
인용문:        text-muted-foreground/70 → text-muted-foreground/80
인라인 코드:   bg-accent/40 → bg-accent/60
코드블록 테두리: border-border/20 → border-border/30
```

변경 파일: `MessageItem.tsx`, `MarkdownComponents.tsx`

#### C. 메시지 간격 조정

```
기본 메시지:   py-1.5 → py-2 (8px)
그룹 메시지:   py-0.5 → py-1 (4px)
```

변경 파일: `MessageItem.tsx`

#### D. prose 커스터마이징

`prose-sm` → `prose` (기본 크기)로 변경하거나, 커스텀 prose 오버라이드:

```css
/* index.css에 추가 */
.prose-chat h2 { margin-top: 1.5em; margin-bottom: 0.5em; }
.prose-chat h3 { margin-top: 1.25em; margin-bottom: 0.4em; }
.prose-chat pre { margin-top: 1em; margin-bottom: 1em; }
.prose-chat table { margin-top: 0.75em; margin-bottom: 0.75em; }
.prose-chat ul, .prose-chat ol { margin-top: 0.5em; margin-bottom: 0.5em; }
.prose-chat li { margin-top: 0.15em; margin-bottom: 0.15em; }
```

변경 파일: `index.css`, `MessageItem.tsx` (`prose-sm` → `prose prose-chat`)

---

### 2.2 워크플로우 프롬프트: ASCII 박스 → 마크다운 구조

#### 방안 1: 마크다운으로 변환 (권장)

ASCII 박스를 마크다운 구조(heading + list + blockquote)로 변환:

**Before** (현재):
```
┌─ 구현 시작 ──────────────────────────┐
│
│ Plan: "분석 진행 상태 시각화 개선"
│
│ 작업 지시서:
│ □ docs/plans/slug-task-01.md
│ □ docs/plans/slug-task-02.md
│
│ 규칙:
│ 1. 각 task 파일을 읽고 순서대로 구현
│ 2. 각 완료 시 <!-- subtask-done:N -->
│ 3. 전체 완료 시 <!-- impl-complete -->
└──────────────────────────────────────┘
```

**After** (마크다운):
```markdown
### 🔧 구현 시작

**Plan**: "분석 진행 상태 시각화 개선"

**작업 지시서**:
- `docs/plans/slug-task-01.md`
- `docs/plans/slug-task-02.md`

**규칙**:
1. 각 task 파일을 읽고 순서대로 구현
2. 각 완료 시 `<!-- subtask-done:N -->`
3. 전체 완료 시 `<!-- impl-complete -->`
```

장점:
- 마크다운 렌더링에서 자연스러움
- 가변폭 폰트에서도 정상
- 코드 경로가 인라인 코드로 렌더링 → 클릭 가능 (FileViewer 연동)
- react-markdown이 이미 처리하므로 추가 컴포넌트 불필요

단점:
- 워크플로우 프롬프트임이 시각적으로 덜 구분됨 (일반 메시지와 혼동 가능)

#### 방안 2: 전용 카드 컴포넌트

워크플로우 프롬프트를 감지해서 별도 UI 카드로 렌더링:

```typescript
// 사용자 메시지에 워크플로우 마커가 있으면 카드로 렌더링
function isWorkflowPrompt(content: string): string | null {
  const match = content.match(/^### (🔧|📋|🔍|🔄) (.+)/);
  return match ? match[2] : null;
}
```

```
┌────────────────────────────────────────┐
│ 🔧 구현 시작                           │
│                                        │
│ Plan: 분석 진행 상태 시각화 개선         │
│                                        │
│ 작업 지시서                             │
│  ▸ slug-task-01.md                     │
│  ▸ slug-task-02.md                     │
│                                        │
│ 규칙                                    │
│  1. 각 task 파일을 읽고 순서대로 구현    │
│  2. 각 완료 시 <!-- subtask-done:N -->  │
│  3. 전체 완료 시 <!-- impl-complete --> │
└────────────────────────────────────────┘
```

장점:
- 워크플로우 프롬프트가 일반 메시지와 명확히 구분됨
- 구조화된 레이아웃 (파일 경로 클릭, 체크박스 등)

단점:
- 전용 컴포넌트 구현 필요
- 마커 규칙 정의 + 유지보수 부담
- 새 워크플로우 프롬프트 추가 시마다 카드 업데이트 필요할 수 있음

#### 권장: 방안 1 먼저, 필요 시 방안 2

방안 1(마크다운 변환)은 프롬프트 문자열만 수정하면 됨. 10개 파일의 `.join("\n")` 블록을 마크다운 형식으로 교체. 렌더링 코드 변경 없음.

방안 2는 방안 1을 적용한 후에도 "워크플로우 메시지 구분이 안 된다"는 불만이 있을 때 도입.

---

## 3. 워크플로우 프롬프트 마크다운 변환 규칙

모든 워크플로우 프롬프트를 통일된 마크다운 포맷으로:

### 제목 접두사 규칙

| 워크플로우 단계 | 접두사 | 예시 |
|---------------|--------|------|
| Plan 문서 작성 | `📋` | `### 📋 문서 작성 요청` |
| 구현 시작 | `🔧` | `### 🔧 구현 시작` |
| 작업 지시서 작성 | `📝` | `### 📝 작업 지시서 작성` |
| Subtask 수정 | `✏️` | `### ✏️ Subtask 수정 요청` |
| 리뷰 요청 | `🔍` | `### 🔍 리뷰 요청` |
| Rework | `🔄` | `### 🔄 Rework` |
| Plan 반영 | `📌` | `### 📌 Plan 문서 반영` |

### 공통 구조

```markdown
### {이모지} {제목}

**Plan**: "{plan.title}"

{본문 (마크다운 리스트/볼드/인라인코드 자유 사용)}
```

### 변환 예시: PlanProposalCard 문서 작성 요청

**Before**:
```typescript
`┌─ 문서 작성 요청 ────────────────────┐`,
`│`,
`│ Plan: "${plan.title}"`,
`│`,
`│ 작성할 문서:`,
...docItems,
`│`,
`│ 각 작업 지시서 포함 내용:`,
`│ • 대상 파일 및 경로`,
`│ • 구현 접근법 (단계별)`,
`│ • 의존성 (패키지, 다른 subtask)`,
`│ • 리스크 및 주의사항`,
`│ • 완료 기준`,
`│`,
`│ 완료 조건: 모든 문서 작성 후 알려주세요`,
`└──────────────────────────────────────┘`,
```

**After**:
```typescript
`### 📋 문서 작성 요청`,
``,
`**Plan**: "${plan.title}"`,
``,
`**작성할 문서**:`,
...docItems.map(d => `- ${d}`),
``,
`**각 작업 지시서 포함 내용**:`,
`- 대상 파일 및 경로`,
`- 구현 접근법 (단계별)`,
`- 의존성 (패키지, 다른 subtask)`,
`- 리스크 및 주의사항`,
`- 완료 기준`,
``,
`> 완료 조건: 모든 문서 작성 후 알려주세요`,
```

---

## 4. 구현 계획

### Phase 1: 워크플로우 프롬프트 마크다운 변환 (빠름)

10개 ASCII 박스 프롬프트를 마크다운으로 변환. 렌더링 코드 변경 없음.

| 파일 | 프롬프트 수 |
|------|-----------|
| `workflowOrchestration.ts` | 1 (구현 시작) |
| `PlanProposalCard.tsx` | 1 (문서 작성 요청) |
| `SubtaskReviewView.tsx` | 4 (Plan 반영, Subtask 수정, Subtask 논의, 작업 지시서) |
| `DevProgressView.tsx` | 2 (리뷰 요청, Rework) |

### Phase 2: 메시지 렌더링 가독성 (A-D 순서대로)

| 단계 | 변경 | 영향 파일 | 리스크 |
|------|------|----------|--------|
| A. 폰트 크기 | 4개 값 조정 | 3파일 | 낮음 (시각적 변경만) |
| B. 대비 개선 | 5개 opacity 조정 | 2파일 | 낮음 |
| C. 간격 조정 | 2개 padding 조정 | 1파일 | 낮음 (Virtuoso 재계산 필요할 수 있음) |
| D. prose 커스텀 | CSS 추가 + 클래스 변경 | 2파일 | 중간 (기존 레이아웃 영향 확인 필요) |

### Phase 3: 워크플로우 전용 카드 (선택, 필요 시)

Phase 1-2 적용 후에도 워크플로우 메시지 구분이 부족하면 전용 카드 컴포넌트 도입.

---

## 5. hasMarkdownSignal() 확인

현재 사용자 메시지의 마크다운 렌더링 조건:

```typescript
// MessageItem.tsx
function hasMarkdownSignal(text: string): boolean {
  // 100자 미만이면 plain text
  if (text.length < 100) return false;
  // 마크다운 패턴 감지: #, -, *, ```, |, > 등
  return /^#{1,6}\s|^[-*]\s|```|^\|.+\||^>\s/m.test(text);
}
```

워크플로우 프롬프트를 마크다운으로 변환하면 `###`, `-`, `>` 등이 포함되므로 자동으로 마크다운 렌더링됨. 별도 조건 추가 불필요.

---

## 참고

- 메시지 렌더링: `src/components/tunaflow/MessageItem.tsx`
- 마크다운 컴포넌트: `src/components/tunaflow/chat/MarkdownComponents.tsx`
- 메시지 메타: `src/components/tunaflow/message/MessageMeta.tsx`
- 워크플로우: `src/lib/workflowOrchestration.ts`
- Plan 카드: `src/components/tunaflow/chat/PlanProposalCard.tsx`
- Subtask 리뷰: `src/components/tunaflow/context-panel/SubtaskReviewView.tsx`
- Dev 진행: `src/components/tunaflow/context-panel/DevProgressView.tsx`
- 스타일: `src/index.css`
- 기존 채팅 UI 계획: `docs/plans/chatUiParityWithTunaChatPlan.md`
- 기존 Gap 분석: `docs/reference/chatUiVsTunaChatGapReview_2026-03-29.md`
