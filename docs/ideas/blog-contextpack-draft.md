# [개발기] 멀티 에이전트 오케스트레이터의 컨텍스트 설계 — tunaFlow ContextPack

## 문제: 에이전트에게 무엇을 알려줘야 할까

tunaFlow는 Claude, Codex, Gemini, OpenCode 같은 CLI 에이전트를 하나의 앱에서 오케스트레이션하는 데스크톱 클라이언트입니다. 에이전트 하나만 쓸 때는 비교적 단순합니다. 이전 대화만 적당히 넘겨줘도 어느 정도는 돌아갑니다.

그런데 여러 에이전트가 **하나의 프로젝트 안에서 협업**하기 시작하면 이야기가 달라집니다.

> 각 에이전트에게 **무엇을, 얼마나, 언제** 알려줘야 하는가?

일반적인 채팅이라면 이전 메시지만 넘기면 끝입니다. 하지만 멀티 에이전트 오케스트레이션에서는 그것만으로는 부족합니다. 실제로 넘겨야 하는 정보가 꽤 많습니다.

- **프로젝트 정보** — 이름, 경로, 기술 스택
- **현재 작업 계획** — Plan, subtask 목록, 현재 phase
- **다른 에이전트의 발언** — Roundtable(RT) 토론 내용, 이전 리뷰 결과
- **코드베이스 검색 결과** — rawq(코드 검색 엔진)로 찾은 관련 코드
- **장기 기억** — 이전 세션의 압축된 대화 요약
- **스킬 문서** — 프레임워크/라이브러리 사용 규칙
- **Identity** — “당신은 Architect입니다. 계획만 세우십시오” 같은 역할 지정

이걸 매 요청마다 적절히 조립해서 프롬프트에 넣어줘야 합니다. tunaFlow에서는 이 조립 결과를 **ContextPack**이라고 부릅니다.

---

### ContextPack의 구조

tunaFlow에서 모든 에이전트 요청은 하나의 함수를 통과합니다.

```rust
build_normalized_prompt_with_budget(
    conn,              // DB 연결
    conversation_id,   // 현재 대화
    prompt,            // 사용자 입력
    project_path,      // 프로젝트 경로
    active_skills,     // 활성화된 스킬 목록
    cross_session_ids, // 관련 세션 ID
    persona_fragment,  // 역할 지정 ("Architect", "Developer" 등)
    context_mode,      // Lite / Standard / Full
    budget_cap,        // 총 컨텍스트 예산 (글자 수)
)
```

Claude든 Codex든 Gemini든, 전부 같은 함수로 프롬프트를 조립합니다. 4개 엔진 공통입니다.

### **조립되는 섹션들**

대략 이런 식으로 계층이 나뉩니다.

```
┌─────────────────────────────────────────────────┐
│ Project path                                     │
│ Platform rules (PLATFORM_TIER0)                 │ ← 항상 포함
│ Agent role document (Architect/Developer/...)    │
│ Identity + Persona                              │
│ Conversation participants meta                  │
├─────────────────────────────────────────────────┤
│ Recent conversation (budget-based window)       │ ← 예산 내에서 최대한
│   + per-agent last-message guarantee            │
│   + tool-result pruning (오래된 메시지)         │
├─────────────────────────────────────────────────┤
│ Plan / Findings / Artifacts                     │ ← Standard+ 모드
│ Retrieval (과거 대화 chunk, FTS5+벡터 검색)      │
│ Compressed memory (주제별 요약)                  │
├─────────────────────────────────────────────────┤
│ Skills / rawq / code-review-graph               │ ← Full 모드 또는 조건부
│ Cross-session context                           │
│ Thread inheritance (Branch 상속)                │
└─────────────────────────────────────────────────┘
```

핵심은 “넣을 수 있는 건 많다”가 아니라, **"지금 이 요청에 필요한 정보만 적절한 밀도로 넣어야 한다"** 입니다.

---

## **핵심 설계: 3-mode 가변 조립**

모든 섹션을 항상 넣으면 토큰이 금방 터집니다. 그래서 상황에 따라 3단계로 조절 했습니다.

| **Mode**     | **포함 범위**                   | **대략적 크기** | **사용 시점**     |
| ------------ | --------------------------- | ---------- | ------------- |
| **Lite**     | 기본 identity + 최근 대화 4k      | ~6k자       | 짧은 질문, 로컬 모델  |
| **Standard** | + Plan/Findings/Retrieval   | ~15k자      | 일반 작업         |
| **Full**     | + Skills/rawq/cross-session | ~25k+      | 복잡한 작업, 명시 요청 |

모드는 자동으로 판정됩니다.

```
fn determine_context_mode(data: &ContextData) -> (ContextMode, &str) {
    // 사용자가 명시적으로 지정한 경우
    if let Some(override_mode) = &data.context_mode_override {
        return (parse_mode(override_mode), "user-override");
    }
    // 대화가 12턴 이상이면 Standard
    if data.current_messages.len() >= 12 {
        return (ContextMode::Standard, "long-conversation");
    }
    // Plan이 있으면 Standard
    if data.plan_section.is_some() {
        return (ContextMode::Standard, "has-plan");
    }
    // 기본: Lite
    (ContextMode::Lite, "default")
}
```

짧은 질문인데 Full을 넣는 것은 낭비이고, 반대로 긴 작업인데 Lite만 넣으면 맥락이 모자랍니다. 결국 중요한 것은 **모든 정보를 최대한 많이 주는 것**이 아니라, **지금 필요한 정보 밀도를 맞추는 것**입니다.

---

## **동적 예산 분배**

전체 예산(기본 60,000자) 안에서 각 섹션이 차지할 비율은 고정하지 않았습니다. **내용 크기와 가중치**를 같이 보고 동적으로 계산합니다.

```
let budget_alloc = allocate_budgets(total_budget, &[
    SectionBudget { name: "plan",       weight: 1.0, min: 500,  max: 4000 },
    SectionBudget { name: "plan-doc",   weight: 2.0, min: 1000, max: 6000 },
    SectionBudget { name: "findings",   weight: 1.0, min: 500,  max: 3000 },
    SectionBudget { name: "skills",     weight: 1.0, min: 500,  max: 3000 },
    SectionBudget { name: "rawq",       weight: 0.8, min: 500,  max: 3000 },
    SectionBudget { name: "retrieval",  weight: 1.2, min: 500,  max: 5000 },
    SectionBudget { name: "compressed", weight: 1.0, min: 500,  max: 4000 },
    SectionBudget { name: "cross",      weight: 0.6, min: 300,  max: 3000 },
]);
```

예를 들어 Plan 문서가 길면 Plan 쪽 예산을 더 가져가고, 대신 skills나 rawq 쪽 예산은 줄어듭니다. 고정 비율로 나누는 방식이 아니라, **현재 들어갈 내용의 부피와 중요도에 따라 예산이 움직이는 구조**입니다.

이게 생각보다 중요합니다. 실제 작업에서는 항상 같은 정보가 중요한 것이 아니기 때문입니다. 어떤 요청은 계획이 핵심이고, 어떤 요청은 코드 검색 결과가 핵심이고, 어떤 요청은 과거 대화 검색이 더 중요합니다.

---

## **대화 이력: 다 넣을 수도 없고, 빼자니 맥락이 끊긴다**

대화가 길어지면 이전 메시지를 전부 넣는 것은 불가능합니다. 그렇다고 막 잘라내면 흐름이 끊깁니다. 그래서 tunaFlow는 세 가지 전략을 씁니다.

### **1. Budget-based dynamic window**

최신 메시지부터 역순으로 채우고, 예산이 다 차면 멈춥니다.

```
// 최신 메시지부터 역순으로 예산 채우기
for (i, msg) in messages.iter().enumerate().rev() {
    let msg_cost = role.len() + content.len().min(max_per_msg) + 40;
    if msg_cost <= char_budget {
        trimmed.push(msg);
        char_budget -= msg_cost;
    } else if must_include.contains(&i) {
        // 예산 초과해도 이 메시지는 반드시 포함
        trimmed.push(msg);
    }
}
```

단순해 보이지만 기본 동작은 이게 맞습니다. 최신 맥락이 가장 중요하니까요.

### **2. Per-agent last-message guarantee**

멀티 에이전트에서 더 중요한 문제는 **각 에이전트의 마지막 발언이 잘리면 안 된다**는 점입니다.

예를 들어 Alice가 3턴 전에 핵심적인 반대 의견을 냈는데, 예산 때문에 그 메시지가 잘려버리면 다음 에이전트는 Alice가 무슨 입장이었는지 모른 채 토론하게 됩니다. 그러면 형식상 멀티 에이전트이지, 실제로는 병렬 독백에 가깝습니다.

그래서 각 에이전트의 마지막 메시지는 반드시 포함되도록 보장합니다.

```
// 각 에이전트의 마지막 메시지 인덱스 수집
let mut agent_last_idx: HashMap<String, usize> = HashMap::new();
for (i, msg) in messages.iter().enumerate() {
    if msg.role == "assistant" {
        agent_last_idx.insert(msg.persona.clone(), i);
    }
}
// 이 인덱스들은 예산 초과해도 반드시 포함
let must_include: HashSet<usize> = agent_last_idx.values().collect();
```

이 보장이 없으면 멀티 에이전트는 생각보다 쉽게 맥락이 무너집니다.

### **3. Compressed memory (주제별 압축)**

12턴 이상 지나가면 오래된 메시지는 그대로 들고 가지 않고, LLM으로 주제별 요약을 만듭니다.

```
## Compressed conversation memory

### 주제: API 설계 결정
Alice(claude): REST vs GraphQL 비교, REST 선택 근거 제시
Bob(codex): 동의, 다만 subscription은 WebSocket 권장

### 주제: 인증 방식
Alice(claude): JWT + refresh token 제안
Charlie(gemini): OAuth2 PKCE 추가 권장
```

원본은 DB에 그대로 보존됩니다. 필요하면 검색으로 언제든 다시 꺼낼 수 있습니다. 프롬프트에는 요약만 실어 보냅니다.

즉, 오래된 대화를 완전히 버리는 것이 아니라, **원문은 저장하고, 전달은 압축해서 한다**는 방식입니다.

---

## **4-Engine Parity: 모든 엔진이 같은 컨텍스트를 받아야 한다**

tunaFlow의 중요한 원칙 중 하나가 **4-engine parity**입니다. Claude든 Codex든 Gemini든 OpenCode든, 같은 질문이면 가능한 한 **같은 컨텍스트 품질**을 받아야 합니다.

엔진마다 차이가 생기면 “이 모델이 더 잘한다”가 아니라, 사실은 “이 모델이 더 많은 정보를 받았다”가 되어버리기 쉽습니다. 그건 비교가 아닙니다.

차이는 딱 하나만 둡니다.

- **Claude**: system prompt 분리 (--append-system-prompt-file)

- **Non-Claude**: 모든 컨텍스트를 하나의 prompt에 inline으로 합침

```
// Claude: system prompt 분리
let system_prompt = format!("{}\n\n{}", context_sections, platform_rules);
let user_prompt = user_input;

// Non-Claude: 전부 합침
let prompt = format!("{}\n\n---\n\n{}", context_sections, user_input);
```

즉, 전달 형식은 달라도 **들어가는 정보 자체는 최대한 동일하게 유지**합니다. 그래야 에이전트를 바꿨을 때 결과 차이를 해석할 수 있습니다.

---

## **RT(Roundtable)에서 생기는 문제: 토큰이 N배로 불어난다**

Roundtable은 구조상 토큰을 많이 먹습니다.

3명이 2라운드 토론하면 총 6번의 에이전트 호출이 발생합니다. 각 호출마다 ContextPack이 붙으면 대략 이런 계산이 나옵니다.

```
6 요청 × ~15k자(Standard 모드) = ~90k자 ≈ ~30k 토큰
```

Claude Pro 같은 유료 플랜에서도 이렇게 몇 번만 돌리면 하루 한도가 꽤 빠르게 줄어듭니다. 멀티 에이전트 오케스트레이터인데, 멀티 에이전트를 적극적으로 쓸수록 단일 대화 여유가 줄어드는 아이러니가 생깁니다.

### **현재 대응: RtContextCache**

그래서 RT에서는 ContextPack을 매 호출마다 새로 빌드하지 않고, **라운드 시작 시 1회만 빌드해서 캐시**합니다.

```
struct RtContextCache {
    auto_context: Option<String>,  // 상용 엔진용 (Claude, Codex, Gemini)
    lite_context: Option<String>,  // 로컬 엔진용 (Ollama, OpenCode)
}
```

같은 라운드 안에서 3명이 차례로 실행되더라도, ContextPack 관련 DB 쿼리는 1~2회만 발생합니다. N회가 아니라 1회입니다.

다만 이것은 어디까지나 **쿼리 비용 절감**입니다. 프롬프트에 실제로 들어가는 텍스트 양은 여전히 N배입니다. 즉, 캐시는 DB를 살려주지만, 토큰 비용 자체를 없애주지는 않습니다.

---

## **앞으로의 방향: Push에서 Pull로**

현재 ContextPack은 기본적으로 **Push 모델**입니다.

“에이전트가 필요할 수도 있으니 일단 넣어둡니다.”

```
현재:
[identity + project + plan + skills + rawq + memory + cross-session]
→ 매 요청마다 전부 전송
→ 10번 중 7번은 skills/rawq/memory를 안 쓰는데도 매번 포함
```

문제는 토큰 비용만이 아닙니다. 더 큰 문제는 **노이즈**입니다.

섹션이 10개, 15개씩 한 번에 들어오면 에이전트 입장에서는 “그래서 지금 무엇을 우선해서 봐야 하지?”가 흐려집니다. 결국 신호 대 잡음비가 낮아집니다. 많이 준다고 항상 좋은 것이 아닙니다.

tunaFlow에는 이미 **tool-request 마커** 시스템이 있습니다. 에이전트가 응답 중에 이런 식의 마커를 넣으면,

```
<!-- tunaflow:tool-request:docs:react hooks -->
```

tunaFlow가 이를 감지해서 검색 결과를 자동 follow-up으로 보내줍니다.

특히 context-hub(라이브러리 문서 검색)는 이미 이 Pull 방식으로 전환된 상태입니다. 남은 섹션들(skills, memory, cross-session)도 같은 패턴으로 옮길 수 있습니다.

목표는 결국 **작은 Push + 선택적 Pull**입니다.

```
Tier 0 (항상):  identity + project 기본                     ~1.5k자
Tier 1 (조건):  plan + findings + rawq(코드 질문 시)       ~2~4k자
Tier 2 (Pull):  skills, memory, cross-session              → 필요 시 에이전트가 요청
```

다만 tunaFlow는 SDK 기반이 아니라 **CLI subprocess 기반**입니다. 그래서 Pull 1회는 곧 **프로세스 재시작 + 새 입력 1회**를 의미합니다. 같은 run 안에서 tool call을 자연스럽게 해결하는 구조가 아닙니다.

즉, Pull이 1회 정도면 이득일 수 있지만, 2회 이상 반복되면 오히려 Push보다 비쌀 수 있습니다. 그래서 결론은 순수 Push도 아니고 순수 Pull도 아닙니다. **하이브리드가 맞습니다.**

---

## **정리**

ContextPack은 결국 “에이전트에게 무엇을 알려줄 것인가”에 대한 tunaFlow의 현재 답입니다.

| **설계 결정**                        | **이유**                           |
| -------------------------------- | -------------------------------- |
| 단일 함수로 4개 엔진 공통 처리               | 엔진 교체 시에도 동일한 정보 품질을 유지하기 위해     |
| 3-mode 가변 조립                     | 상황에 맞는 정보 밀도를 유지하고 토큰 낭비를 줄이기 위해 |
| 동적 예산 분배                         | 섹션마다 중요도가 매번 달라지기 때문에            |
| Per-agent last-message guarantee | 멀티 에이전트 토론에서 맥락 유실을 막기 위해        |
| RtContextCache                   | RT 라운드 내 DB 쿼리 비용을 줄이기 위해        |
| tool-request Pull (진행 중)         | Push 노이즈를 줄이고 토큰 효율을 높이기 위해      |

CLI subprocess 기반이라는 제약 안에서, 결국 지키고 싶은 원칙은 하나입니다.

**에이전트가 편해야 결과가 좋아집니다.**

컨텍스트를 많이 준다고 좋은 것이 아니라, 필요한 것을 적절한 밀도로 주는 것이 더 중요합니다. ContextPack은 그 균형점을 찾기 위한 현재의 설계입니다.

---

## **레퍼런스**

### **관련 기술 문서**

- **Anthropic — Contextual Retrieval** (2024.09): chunk에 문서 맥락 prefix를 추가하여 검색 실패율 49% 감소. ContextPack의 Tier 2 Pull 설계에 참고.
  
  https://www.anthropic.com/news/contextual-retrieval

- **Jina AI — Late Chunking** (2024): long-context 임베딩 모델로 전체 문서를 처리한 후 chunk별 pooling. BeIR 벤치마크 NDCG@10 +5~15%.
  
  https://jina.ai/news/late-chunking-in-long-context-embedding-models

- **Dense X Retrieval** (Chen et al., 2024): Proposition 기반 chunking으로 recall@5 +12~17%. 각 retrieval unit이 self-contained + atomic.
  
  https://arxiv.org/abs/2312.06648

- **RAPTOR** (Sarthi et al., 2024, Stanford): 재귀적 클러스터링 + 요약으로 다층 인덱싱. NarrativeQA accuracy +20%.
  
  https://arxiv.org/abs/2401.18059

- **ColBERT v2** (Santhanam et al., 2022): 토큰별 multi-vector representation으로 MRR@10 0.397 (BM25: 0.187).
  
  https://arxiv.org/abs/2112.01488

### **tunaFlow 내부 문서**

- docs/ideas/contextPackTieringIdea.md — ContextPack 3-Tier 하이브리드 설계 + 벡터 맥락 공유 + sqlite-vec + chunk 품질 개선

- docs/reference/multiAgentContextStrategy.md — 멀티 에이전트 컨텍스트 3-layer 전략 (participants meta + dynamic window + per-agent guarantee)

- docs/ideas/insightWorkflowIdea.md — Insight 리포트 파일 저장 + Plan 승격 UX (ContextPack 토큰 0으로 에이전트 접근)

### **관련 도구**

- **rawq** — Rust 기반 코드 검색 엔진. snowflake-arctic-embed-s (384차원) 임베딩, daemon 모드 상주. tunaFlow의 코드 검색 + 벡터 임베딩 생성 담당.

- **context-hub (chub)** — 라이브러리/프레임워크 문서 검색 CLI. tunaFlow에서는 tool-request Pull 방식으로 전환 완료.
