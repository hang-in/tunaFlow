# SDK as Interface Layer — API 호출이 아닌 인터페이스 표준화 계층

> Status: idea
> Created: 2026-04-02
> 선행 문서: `sdkIntegrationIdea.md` (유료 API 중심), `smallModelStressTesterIdea.md` (Ollama/소형 모델)

---

## 1. 관점 전환: SDK ≠ 유료 API 호출

`sdkIntegrationIdea.md`는 SDK를 "유료 API를 직접 호출하는 수단"으로 봤다. 이 문서는 **API 키 없이도 SDK에서 얻을 수 있는 가치**에 집중한다.

SDK의 진짜 가치는 세 가지:
1. **인터페이스 표준화** — 어떤 백엔드든 같은 코드로 동작
2. **스키마 정의 + 검증** — 타입 시스템으로 에이전트 출력을 구조화
3. **로컬 모델 통합** — Ollama, LM Studio 등 무료 백엔드를 동일 인터페이스로

---

## 2. OpenAI SDK = 사실상의 LLM 인터페이스 표준

### OpenAI 호환 API를 제공하는 서비스/도구

| 제공자 | 종류 | API 키 | 비용 |
|--------|------|--------|------|
| OpenAI | 클라우드 | 필요 | 유료 |
| Ollama | 로컬 | 불필요 | 무료 |
| LM Studio | 로컬 | 불필요 | 무료 |
| vLLM | 셀프호스트 | 불필요 | 무료 (인프라 비용) |
| Together AI | 클라우드 | 필요 | 유료 (저가) |
| Groq | 클라우드 | 필요 | 유료 (저가) |
| Fireworks AI | 클라우드 | 필요 | 유료 |
| Anyscale | 클라우드 | 필요 | 유료 |
| LocalAI | 로컬 | 불필요 | 무료 |

**하나의 코드로 전부 동작**:

```typescript
import OpenAI from "openai"

// OpenAI (유료)
const openai = new OpenAI({ apiKey: process.env.OPENAI_API_KEY })

// Ollama (무료, 로컬)
const ollama = new OpenAI({ baseURL: "http://localhost:11434/v1", apiKey: "x" })

// LM Studio (무료, 로컬)
const lmstudio = new OpenAI({ baseURL: "http://localhost:1234/v1", apiKey: "x" })

// Together AI (유료, 저가)
const together = new OpenAI({ baseURL: "https://api.together.xyz/v1", apiKey: "..." })

// 동일한 호출 코드
async function chat(client: OpenAI, model: string, prompt: string) {
  return client.chat.completions.create({
    model,
    messages: [{ role: "user", content: prompt }],
    tools: [reviewVerdictTool],  // function calling
    stream: true,
  })
}
```

### Rust에서의 구현

tunaFlow 백엔드는 Rust이므로, `reqwest`로 OpenAI 호환 API를 직접 호출:

```rust
// 별도 SDK 의존성 없이 reqwest만으로 충분
pub struct OpenAICompatClient {
    base_url: String,       // "http://localhost:11434/v1" 또는 "https://api.openai.com/v1"
    api_key: Option<String>, // 로컬이면 None
    client: reqwest::Client,
}

impl OpenAICompatClient {
    pub async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse> {
        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.as_deref().unwrap_or("x")))
            .json(&request)
            .send().await?;
        resp.json().await.map_err(Into::into)
    }
    
    pub async fn chat_completion_stream(&self, request: ChatRequest) -> Result<impl Stream<Item = ChatChunk>> {
        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key.as_deref().unwrap_or("x")))
            .json(&ChatRequest { stream: Some(true), ..request })
            .send().await?;
        Ok(sse_stream(resp))  // SSE 파싱
    }
}
```

**핵심**: `base_url`만 바꾸면 OpenAI, Ollama, LM Studio, vLLM 전부 동작.

---

## 3. 스키마 정의 + 검증: API 호출 없는 구조화

### 3.1 현재 마커 파싱의 문제

```markdown
<!-- tunaflow:review-verdict -->
verdict: pass
findings:
- 구현이 plan과 일치합니다
<!-- /tunaflow:review-verdict -->
```

- 형식이 자유롭다 → 에이전트마다 다른 형태로 출력
- 파서가 YAML-like 구문을 수동 파싱 → edge case 다수
- 검증이 느슨하다 → "findings가 배열인가?" 같은 타입 검증 없음

### 3.2 스키마를 CLI 프롬프트에 주입

SDK가 아니라 **JSON Schema만** 사용해도 구조화 가능:

```typescript
// 스키마 정의 (SDK 타입에서 파생하거나 zod로 직접 정의)
import { z } from "zod"

const ReviewVerdictSchema = z.object({
  verdict: z.enum(["pass", "fail", "conditional"]),
  findings: z.array(z.object({
    severity: z.enum(["critical", "major", "minor"]),
    file: z.string().optional(),
    line: z.number().optional(),
    description: z.string(),
  })),
  recommendations: z.array(z.object({
    action: z.string(),
    priority: z.number().min(1).max(5),
  })).optional(),
})

// JSON Schema로 변환
const jsonSchema = zodToJsonSchema(ReviewVerdictSchema)
```

이 스키마를 **CLI 프롬프트에 텍스트로 삽입**:

```
리뷰 결과를 아래 JSON 스키마에 맞춰 출력하세요.
반드시 유효한 JSON으로 응답하고, 다른 텍스트는 포함하지 마세요.

{
  "type": "object",
  "properties": {
    "verdict": { "type": "string", "enum": ["pass", "fail", "conditional"] },
    "findings": { ... }
  },
  "required": ["verdict", "findings"]
}
```

**API 호출 방식과 무관하게** (CLI spawn이든 SDK든) 동작:
- CLI spawn → stdout에서 JSON 추출 → zod로 검증
- SDK function calling → tool_call.arguments에서 직접 추출

### 3.3 Typia 패턴: 파싱 + 검증 + 피드백 루프

`smallModelStressTesterIdea.md`에서 분석한 패턴을 **API 키 없이** 구현:

```typescript
async function getStructuredOutput<T>(
  schema: z.ZodType<T>,
  prompt: string,
  engine: "cli" | "sdk",
  maxRetries: number = 3,
): Promise<T> {
  const jsonSchema = zodToJsonSchema(schema)
  
  for (let i = 0; i < maxRetries; i++) {
    // 1. 에이전트 호출 (CLI든 SDK든)
    const raw = engine === "sdk"
      ? await sdkCall(prompt, jsonSchema)
      : await cliCall(promptWithSchema(prompt, jsonSchema))
    
    // 2. JSON 추출 (자유 텍스트에서 JSON 블록 찾기)
    const jsonStr = extractJson(raw)
    if (!jsonStr) {
      prompt = `이전 응답에서 유효한 JSON을 찾을 수 없습니다. 반드시 JSON만 출력하세요.\n\n${prompt}`
      continue
    }
    
    // 3. 스키마 검증
    const result = schema.safeParse(JSON.parse(jsonStr))
    if (result.success) return result.data
    
    // 4. 피드백: 검증 에러를 프롬프트에 포함
    const errors = result.error.issues.map(e => 
      `- ${e.path.join(".")}: ${e.message}`
    ).join("\n")
    prompt = `이전 응답이 스키마를 위반했습니다:\n${errors}\n\n수정하여 다시 출력하세요.\n\n${prompt}`
  }
  
  throw new Error("Max retries exceeded for structured output")
}
```

**이 패턴의 핵심**: API 키 없이, CLI spawn으로도 구조화 출력을 얻을 수 있다. SDK function calling은 "더 효율적인 방법"이지, "유일한 방법"이 아니다.

---

## 4. 로컬 모델 통합: Ollama 경로

### 4.1 `smallModelStressTesterIdea.md`와의 연결

해당 문서에서 계획한 Ollama 통합:

```
필요한 변경:
1. src-tauri/src/agents/ollama.rs        (~100줄, subprocess spawn + 출력 파싱)
2. executor.rs match arm 1줄 추가        ("ollama" => ollama::run(input))
3. model_discovery.rs ollama 추가         (ollama ls --json 파싱)
4. types/index.ts engine 타입 확장        ("ollama" 추가)
5. lib.rs command 등록                    (start_ollama_run)
```

### 4.2 OpenAI 호환 인터페이스로 단순화

위 5개 파일 대신, **OpenAI 호환 클라이언트 하나**로 대체:

```
필요한 변경:
1. src-tauri/src/agents/openai_compat.rs  — 범용 OpenAI 호환 클라이언트 (~150줄)
2. executor.rs match arm 추가             — "ollama" | "lmstudio" | "openai-api" 등
3. model_discovery.rs                     — ollama ls --json (기존 계획 동일)
4. types/index.ts                         — engine 타입 확장
5. lib.rs                                 — command 등록
```

차이점: `openai_compat.rs` 하나로 Ollama, LM Studio, vLLM, OpenAI API **전부** 커버. 엔진별 파일 불필요.

```rust
// agents/openai_compat.rs
pub struct OpenAICompatEngine {
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: String,
}

impl OpenAICompatEngine {
    pub fn ollama() -> Self {
        Self {
            base_url: "http://localhost:11434/v1".into(),
            api_key: None,
            default_model: "qwen3.5:9b".into(),
        }
    }
    
    pub fn lmstudio() -> Self {
        Self {
            base_url: "http://localhost:1234/v1".into(),
            api_key: None,
            default_model: "default".into(),
        }
    }
    
    pub fn openai(api_key: String) -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: Some(api_key),
            default_model: "gpt-4o".into(),
        }
    }
    
    pub async fn run(&self, input: RunInput) -> Result<RunOutput, AppError> {
        // 동일한 코드로 모든 백엔드 처리
    }
}
```

### 4.3 로컬 모델의 실용적 용도

| 용도 | 모델 | API 키 | 설명 |
|------|------|--------|------|
| **R&D: 파서 stress test** | qwen 9b | 불필요 | 마커/스키마 견고성 검증 |
| **R&D: 프롬프트 벤치마크** | qwen 9b | 불필요 | 프롬프트 명확성 측정 |
| **RT: blind verifier** | qwen 27b+ | 불필요 | 저비용 추가 검증자 |
| **Embeddings: 로컬** | nomic-embed | 불필요 | rawq 보완/대체 |
| **요약/분류** | phi-4 등 | 불필요 | 메시지 분류, 자동 태깅 |
| **오프라인 작업** | 아무 로컬 모델 | 불필요 | 네트워크 없이 작업 |

---

## 5. 3-tier 엔진 아키텍처

```
┌─────────────────────────────────────────────────────────────────┐
│                      EngineAdapter trait                         │
│  fn send(req) -> Stream<Chunk>                                  │
│  fn capabilities() -> Capabilities                              │
├──────────────────┬──────────────────┬───────────────────────────┤
│   Tier 1: CLI    │  Tier 2: OpenAI  │   Tier 3: Native SDK      │
│   (현재 방식)     │  Compatible      │   (벤더 전용)              │
│                  │  (새로 추가)      │   (선택적)                 │
├──────────────────┼──────────────────┼───────────────────────────┤
│ claude CLI       │ Ollama           │ Anthropic API (tool use)  │
│ codex CLI        │ LM Studio        │ Gemini SDK (caching)      │
│ gemini CLI       │ vLLM             │ OpenAI SDK (embeddings)   │
│ opencode CLI     │ OpenAI API       │                           │
│                  │ Together/Groq    │                           │
├──────────────────┼──────────────────┼───────────────────────────┤
│ API 키: 불필요    │ API 키: 선택적    │ API 키: 필수              │
│ 비용: CLI 구독    │ 비용: 무료~저가   │ 비용: 유료                │
│ 기능: 텍스트 I/O  │ 기능: + streaming │ 기능: + caching           │
│                  │ + function call  │ + embeddings              │
│                  │ + 구조화 출력     │ + 벤더 전용 기능           │
└──────────────────┴──────────────────┴───────────────────────────┘
```

### Tier 승격 경로

```
Tier 1 (CLI)
  ↓ 사용자가 Ollama 설치 (무료)
Tier 2 (OpenAI Compatible)
  ↓ 사용자가 API 키 입력 (유료)  
Tier 3 (Native SDK)
```

각 Tier는 이전 Tier의 **상위 호환**. 사용자가 아무것도 설정하지 않아도 Tier 1로 현재와 동일하게 동작. Ollama만 설치하면 Tier 2의 function calling + streaming 사용 가능. API 키를 입력하면 Tier 3의 caching + embeddings까지.

### capabilities() 기반 기능 분기

```rust
pub struct EngineCapabilities {
    pub streaming: bool,
    pub function_calling: bool,
    pub structured_output: bool,
    pub context_caching: bool,
    pub embeddings: bool,
    pub vision: bool,
    pub tool_use_concurrent: bool,
}

impl EngineAdapter for OpenAICompatEngine {
    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities {
            streaming: true,
            function_calling: true,          // Ollama도 지원
            structured_output: true,         // Ollama도 지원 (모델 의존)
            context_caching: false,          // OpenAI API만 (자동)
            embeddings: self.has_embedding_model(),
            vision: self.model_supports_vision(),
            tool_use_concurrent: false,
        }
    }
}
```

워크플로우 코드가 capability를 체크해서 분기:

```rust
if engine.capabilities().function_calling {
    // 구조화된 tool call로 verdict 수집
    let verdict = engine.send_with_tools(prompt, &[review_verdict_tool]).await?;
} else {
    // 마커 기반 파싱 fallback
    let text = engine.send(prompt).await?;
    let verdict = parse_review_verdict_marker(&text)?;
}
```

---

## 6. 스키마 중심 워크플로우: 마커 → 스키마 점진 전환

### 전환 전략: 마커와 스키마 공존

```
Phase 1 (현재): 마커만 사용
  <!-- tunaflow:review-verdict -->
  verdict: pass
  <!-- /tunaflow:review-verdict -->

Phase 2 (Tier 2 추가 후): 스키마 우선, 마커 fallback
  if (engine.capabilities.function_calling) {
    // JSON Schema로 구조화 출력 요청
    submit_review_verdict({ verdict: "pass", findings: [...] })
  } else {
    // 기존 마커 파싱 유지
    parseReviewVerdictMarker(text)
  }

Phase 3 (장기): 마커 제거, 스키마만
  // 모든 엔진이 function calling 지원
  // 마커 파서 코드 deprecated → 삭제
```

### zod 스키마 → 마커 파서 + function calling 동시 생성

```typescript
// 하나의 스키마 정의로 두 가지 파서 생성
const ReviewVerdictSchema = z.object({
  verdict: z.enum(["pass", "fail", "conditional"]),
  findings: z.array(z.object({
    severity: z.enum(["critical", "major", "minor"]),
    file: z.string().optional(),
    line: z.number().optional(),
    description: z.string(),
  })),
  recommendations: z.array(z.object({
    action: z.string(),
    priority: z.number(),
  })).optional(),
})

// 1. Function calling tool 정의 (Tier 2/3)
const reviewVerdictTool = {
  type: "function",
  function: {
    name: "submit_review_verdict",
    parameters: zodToJsonSchema(ReviewVerdictSchema),
  }
}

// 2. 마커 파서 검증기 (Tier 1 fallback)
function parseAndValidateVerdict(markerContent: string): ParsedReviewVerdict {
  const raw = parseYamlLike(markerContent)  // 기존 파서
  const result = ReviewVerdictSchema.safeParse(raw)
  if (!result.success) {
    console.warn("Verdict validation failed:", result.error)
    return fallbackParse(markerContent)  // graceful degradation
  }
  return result.data
}

// 3. CLI 프롬프트 주입용 스키마 텍스트 (Tier 1 강화)
function schemaPromptBlock(): string {
  return `응답은 아래 JSON 스키마를 따르세요:\n${JSON.stringify(zodToJsonSchema(ReviewVerdictSchema), null, 2)}`
}
```

**하나의 zod 정의**에서:
- Function calling tool 스키마 (Tier 2/3)
- 마커 파싱 후 검증기 (Tier 1)
- 프롬프트 주입용 텍스트 (Tier 1 강화)

세 가지가 동시에 파생된다. 스키마가 SSOT.

---

## 7. 구현 로드맵 (API 키 불필요 항목 우선)

### Phase 1: 스키마 정의 인프라 (즉시, API 키 불필요)

```
변경:
1. package.json                        — zod 의존성 추가 (이미 있을 수 있음)
2. src/lib/schemas/                    — 워크플로우 스키마 정의 (5개)
   ├── planProposal.ts
   ├── implPlan.ts  
   ├── reviewVerdict.ts
   ├── subtaskDone.ts
   └── implComplete.ts
3. src/lib/planProposalParser.ts       — zod 검증 추가 (기존 파서 위에)
```

가치: 마커 파서의 검증 강화, 에러 메시지 개선. CLI 방식 그대로.

### Phase 2: OpenAI Compatible 클라이언트 (단기, API 키 선택적)

```
변경:
1. src-tauri/src/agents/openai_compat.rs   — 범용 HTTP 클라이언트 (~150줄)
2. src-tauri/src/agents/mod.rs             — 모듈 추가
3. src-tauri/src/commands/agents_helpers/executor.rs — match arm 추가
4. src-tauri/src/db/models.rs              — TokenUsage 확장
5. Settings UI                             — Custom endpoint 설정
```

가치: Ollama/LM Studio 즉시 사용 가능. 무료 로컬 모델로 RT 확장.

### Phase 3: Function Calling 통합 (단기, Tier 2 활용)

```
변경:
1. src/lib/schemas/ → JSON Schema 변환 유틸
2. ContextPack → 스키마 프롬프트 블록 주입 (Tier 1 강화)
3. openai_compat.rs → tool call 요청/응답 처리
4. workflowOrchestration.ts → capability 기반 분기
```

가치: function calling 지원 엔진은 구조화 출력, 나머지는 마커 fallback.

### Phase 4: 로컬 모델 R&D 도구 (단기, API 키 불필요)

```
변경:
1. 파서 fixture에 Ollama 실제 출력 추가
2. 프롬프트 명확성 벤치마크 스크립트
3. RT stress test 자동화
```

가치: `smallModelStressTesterIdea.md` Phase 1-2 실행.

### Phase 5: Native SDK (장기, API 키 필요)

`sdkIntegrationIdea.md`의 Phase 1-6 실행.

---

## 8. 핵심 인사이트

### "SDK를 쓴다" ≠ "유료 API를 호출한다"

SDK는 세 가지 독립적 가치를 제공한다:

| 가치 | API 키 필요? | 예시 |
|------|-------------|------|
| **인터페이스 표준화** | X | OpenAI 호환으로 Ollama/LM Studio/vLLM 통합 |
| **스키마 정의 + 검증** | X | zod 스키마 → 마커 파서 강화 + 프롬프트 주입 |
| **고급 API 기능** | O | Context caching, embeddings, batch |

### "비용 없이 기능 강화"가 가능한 영역

1. **마커 파서 견고성** — zod 검증 추가만으로 에러 감지/복구 개선
2. **구조화 출력 유도** — JSON Schema를 프롬프트에 주입 (CLI 방식 그대로)
3. **로컬 모델 R&D** — Ollama로 파서 stress test, 프롬프트 벤치마크
4. **RT 저비용 확장** — 로컬 모델을 blind verifier로 투입
5. **오프라인 개발** — 네트워크 없이 로컬 모델로 작업

### 다른 아이디어 문서와의 연결

| 문서 | 이 문서와의 관계 |
|------|----------------|
| `sdkIntegrationIdea.md` | Tier 3 (유료) 부분. 이 문서는 Tier 1-2 (무료) 보완 |
| `smallModelStressTesterIdea.md` | Ollama 통합 경로가 OpenAI 호환으로 단순화 |
| `rtAlgorithmEnhancementIdeas.md` | Structured Verdict Rubric이 zod 스키마로 직접 구현 가능 |
| `vectorDbAndRetrievalAlgorithmsIdea.md` | 로컬 임베딩 모델(nomic-embed)을 Ollama 경로로 통합 |
| `guardrailImprovementIdeas.md` | 구조화 출력이 guardrail 자체를 강화 |

---

## 참고 자료

- OpenAI API 호환 사양: `POST /v1/chat/completions`
- Ollama OpenAI 호환: `http://localhost:11434/v1`
- LM Studio OpenAI 호환: `http://localhost:1234/v1`
- zod: https://zod.dev
- zod-to-json-schema: https://github.com/StefanTerdell/zod-to-json-schema
- tunaFlow 마커 파서: `src/lib/planProposalParser.ts`
- Claude Code Tool 인터페이스: `_research/_util/claude-code/src/Tool.ts`
