# DATA_MODEL.md 생성 프롬프트

## 역할
너는 시스템 아키텍트다.

tunadish 분석 결과를 기반으로,
tunaFlow v2의 최종 도메인 모델 명세서를 작성하라.

중요:
- 추측 금지
- 실제 확인된 개념만 사용
- 모호한 부분은 TODO로 남길 것
- 구현 코드 작성 금지
- 이 문서는 SSOT다

---

## 목표
tunaFlow의 DATA_MODEL.md 작성

---

## 반드시 포함할 것

### 1. Core Entities
다음 엔티티 정의:

- Workspace
- Project
- Conversation
- Branch
- Message
- Agent
- ResumeToken
- ContextPack
- Artifact
- Memo

각 항목 포함:
- 정의
- 책임
- 필드 (타입 포함)
- 관계

---

### 2. Relationship Model
엔티티 간 관계를 계층 구조로 명확히 설명

---

### 3. Branch Semantics
- checkpoint 개념
- parentBranchId 의미
- adopt 동작 정의
- merge가 아닌 이유 설명

---

### 4. Context Model
- ResumeToken vs ContextPack vs rawq 구분
- lifecycle
- attach 방식

---

### 5. Conversation Modes
- chat
- roundtable

차이 정의

---

### 6. Persistence Mapping
- SQLite / JSON / Memory 구분

---

### 7. State Machines
- Branch
- Message
- Run
- Artifact

---

### 8. Open TODO
확정 불가 항목 명시

---

## 출력
완전한 DATA_MODEL.md
