# RT (Roundtable) 테스트 시나리오

> 목적: RT 기능 실사용 검증
> 주제: tunaInsight — Nuxt 3 기반 멀티에이전트 GitHub 저장소 비교 분석 서비스
> 작성: 2026-04-03 (세션 8)

---

## 사전 준비

1. tunaFlow에서 프로젝트 선택 (아무 프로젝트나 OK)
2. 메인 채팅에서 대화 1개 이상 생성
3. 사이드바 Chats 섹션의 [+] 버튼 또는 메시지 컨텍스트 메뉴에서 RT 생성
4. 사용 가능한 엔진 확인: claude, codex (OpenAI), gemini 최소 설치

---

## 시나리오 1: Sequential 3-엔진 혼합 (기본 검증)

### 목적
Claude, Codex, Gemini가 순서대로 발언하고, 뒤 참가자가 앞 참가자의 답변을 참고하는지 검증. 엔진별 identity, 실행 순서, context 전달 확인.

### 설정
| 항목 | 값 |
|------|---|
| 모드 | Sequential |
| 참가자 | 3명 |
| 참가자 1 | 이름: Architect, 엔진: claude, 역할: 없음 |
| 참가자 2 | 이름: Engineer, 엔진: codex, 역할: 없음 |
| 참가자 3 | 이름: Analyst, 엔진: gemini, 역할: 없음 |

### 프롬프트
```
tunaInsight의 GitHub 저장소 비교 기능을 설계하려고 합니다.

현재 구상:
- 사용자가 2개의 GitHub repo URL을 입력
- 각 repo의 구조, 기술 스택, 코드 품질을 에이전트가 분석
- 분석 결과를 나란히 비교하는 대시보드 제공

Nuxt 3 + Nitro 서버 기반에서 이 기능의 백엔드 아키텍처를 제안해 주세요.
먼저 간단히 자기소개(이름, 엔진)를 하고, 각자 독립적인 관점으로 답변하되
이전 발언이 있다면 참고해서 보완하세요.
```

### 검증 포인트
- [ ] 3명 모두 응답이 생성되는가
- [ ] Architect(claude) → Engineer(codex) → Analyst(gemini) 순서인가
- [ ] Engineer가 Architect의 내용을 참고/보완하는가
- [ ] Analyst가 앞 두 명의 내용을 참고하는가
- [ ] 각 참가자의 자기소개에서 올바른 엔진명을 말하는가
- [ ] 메시지 헤더에 엔진/모델이 정확히 표시되는가 (claude/codex/gemini)
- [ ] RoundtableView에 "Round 1" 구분선이 보이는가
- [ ] 참가자 상태(running → done)가 실시간으로 표시되는가

---

## 시나리오 2: Deliberative 3-엔진 + 역할 분리

### 목적
Deliberative 모드에서 Claude, Codex, Gemini가 동시에 실행되고, Round 1에서 독립 응답 → Round 2에서 상호 참조하는지 검증. 역할 badge, completion-order 확인.

### 설정
| 항목 | 값 |
|------|---|
| 모드 | Deliberative |
| 참가자 | 3명 |
| 참가자 1 | 이름: Frontend, 엔진: gemini, 역할: proposer |
| 참가자 2 | 이름: Backend, 엔진: claude, 역할: proposer |
| 참가자 3 | 이름: Reviewer, 엔진: codex, 역할: reviewer |

### 프롬프트
```
tunaInsight에서 "저장소 비교 보고서" 기능의 UI/UX를 설계합니다.

요구사항:
- 두 저장소의 분석 결과를 side-by-side로 비교
- 각 항목(기술 스택, 의존성, 코드 구조, 테스트 커버리지)별 점수 시각화
- 차이점 하이라이트 + 상세 드릴다운
- 보고서 PDF 내보내기

각자 역할에 맞게 독립적으로 제안하세요:
- Frontend(gemini): 컴포넌트 구조와 상태 관리
- Backend(claude): API 설계와 데이터 흐름
- Reviewer(codex): 두 제안의 실현 가능성과 리스크 평가
```

### 검증 포인트
- [ ] 3명 모두 응답이 생성되는가
- [ ] Round 1에서 서로의 답변을 참고하지 않는가 (독립 응답)
- [ ] completion-order — 빠른 엔진의 응답이 먼저 표시되는가
- [ ] 역할(proposer, reviewer) badge가 RtMessageCard에 표시되는가
- [ ] Reviewer(codex)의 관점이 다른 두 명과 구별되는가
- [ ] 참가자 3명이 동시에 "running" 상태로 표시되는가
- [ ] 각 엔진명이 메시지 헤더에 올바르게 표시되는가

### Follow-up (Round 2)
시나리오 2의 RT에서 추가 프롬프트를 전송:

```
Round 1의 세 제안을 종합하여:
1. Frontend(gemini)와 Backend(claude) 제안의 공통점과 차이점 정리
2. Reviewer(codex)가 지적한 리스크에 대한 구체적 해결 방안
3. 최종 추천 아키텍처 한 장 요약
```

### Round 2 추가 검증
- [ ] "Round 2" 구분선이 표시되는가
- [ ] Round 2에서 각 참가자가 Round 1 전체(다른 엔진 포함) 내용을 참고하는가
- [ ] "reflects on prior" 등 라운드 의미 표시가 나타나는가

---

## 시나리오 3: Blind Verifier (3-엔진 혼합)

### 목적
blind 참가자(Gemini)가 다른 참가자(Claude, Codex)의 답변을 보지 못한 상태에서 독립적으로 평가하는지 검증.

### 설정
| 항목 | 값 |
|------|---|
| 모드 | Sequential |
| 참가자 | 3명 |
| 참가자 1 | 이름: Planner, 엔진: claude, 역할: proposer |
| 참가자 2 | 이름: Implementer, 엔진: codex, 역할: proposer |
| 참가자 3 | 이름: Auditor, 엔진: gemini, 역할: verifier, **blind: ON** |

### 프롬프트
```
tunaInsight의 GitHub API 연동 전략을 결정합니다.

배경:
- GitHub REST API v3 vs GraphQL v4 선택
- Rate limit 관리 (인증 사용자 5000 req/hr)
- 저장소 메타데이터, 파일 트리, 커밋 히스토리, 이슈/PR 데이터 필요
- 대규모 저장소(10만+ 커밋)도 지원해야 함

Planner(claude): 전체 API 전략과 데이터 수집 파이프라인 설계
Implementer(codex): 구체적 구현 방식 (클라이언트 라이브러리, 캐싱, 에러 핸들링)
Auditor(gemini): 주어진 주제만 보고 독립적으로 최적 전략을 판단하세요.
```

### 검증 포인트
- [ ] Auditor(gemini)에게 blind badge(방패 아이콘)가 표시되는가
- [ ] Auditor의 답변이 Planner/Implementer의 내용을 인용하지 않는가
- [ ] Auditor가 "다른 참가자가 이렇게 말했다" 같은 참조를 하지 않는가
- [ ] Sequential이므로 Planner(claude) → Implementer(codex) → Auditor(gemini) 순서인가
- [ ] Implementer(codex)는 Planner(claude)의 답변을 참고하는가 (blind가 아니므로)
- [ ] verifier 역할 badge가 표시되는가
- [ ] 3개 엔진 모두 에러 없이 완료되는가

---

## 시나리오 4: 혼합 엔진 identity 검증 (Claude + Codex + Gemini + Ollama)

### 목적
4개 엔진이 각자 올바른 identity를 가지는지 검증. 세션 8에서 수정한 identity 모델명 포함 기능이 모든 엔진에서 동작하는지 확인.

### 설정
| 항목 | 값 |
|------|---|
| 모드 | Sequential |
| 참가자 | 4명 (또는 ollama 없으면 3명) |
| 참가자 1 | 이름: Strategist, 엔진: claude |
| 참가자 2 | 이름: Builder, 엔진: codex |
| 참가자 3 | 이름: Researcher, 엔진: gemini |
| 참가자 4 | 이름: Local, 엔진: ollama (선택) |

### 프롬프트
```
tunaInsight에 실시간 저장소 모니터링 기능을 추가하려 합니다.

요구사항:
- GitHub Webhook으로 push/PR 이벤트 수신
- Nuxt 3 Nitro 서버에서 webhook 처리
- 변경 감지 시 자동 재분석 트리거
- 분석 결과를 웹소켓으로 클라이언트에 실시간 전송

먼저 자기소개를 간단히 하고(이름, 엔진, 모델), 각자 답변하세요:
- Strategist: 전체 아키텍처와 이벤트 흐름 설계
- Builder: 코드 수준의 구현 방안
- Researcher: 기술 조사 관점에서 대안 비교
- Local: 간결한 요약과 실행 우선순위 정리
```

### 검증 포인트
- [ ] 각 참가자가 자기소개에서 **자기 엔진/모델**을 정확히 말하는가
- [ ] **다른 엔진의 이름을 자기 것으로 주장하지 않는가** (핵심)
- [ ] 특히 ollama가 자신을 Claude/Codex/Gemini라고 하지 않는가
- [ ] 각 메시지 헤더에 올바른 엔진/모델이 표시되는가
- [ ] 뒤 참가자가 앞 참가자의 답변을 정상적으로 참고하는가
- [ ] 모든 엔진이 에러 없이 완료되는가

---

## 시나리오 5: 동시 실행 격리

### 목적
RT 실행 중 메인 채팅에서 별도 에이전트를 실행해도 이벤트가 교차하지 않는지 확인.

### 방법
1. 시나리오 2 (3명 Deliberative: gemini + claude + codex)를 시작
2. RT가 실행 중인 동안 메인 채팅에서 간단한 프롬프트 전송 (예: "안녕")
3. 양쪽 모두 정상 완료 확인

### 검증 포인트
- [ ] 메인 채팅의 응답이 RT 드로어에 표시되지 않는가
- [ ] RT 참가자의 응답이 메인 채팅에 표시되지 않는가
- [ ] 양쪽 모두 에러 없이 완료되는가
- [ ] 완료 후 메시지가 누락되지 않는가 (DB에서 reload)
- [ ] 스트리밍 중 메인↔RT 전환해도 내용이 겹치지 않는가

---

## 테스트 결과 기록

| 시나리오 | 엔진 조합 | 결과 | 발견된 이슈 |
|----------|----------|------|-----------|
| 1. Sequential 기본 | claude + codex + gemini | | |
| 2. Deliberative | gemini + claude + codex | | |
| 2b. Round 2 Follow-up | (동일) | | |
| 3. Blind verifier | claude + codex + gemini(blind) | | |
| 4. Identity 검증 | claude + codex + gemini + ollama | | |
| 5. 동시 실행 격리 | (시나리오 2) + 메인 | | |
