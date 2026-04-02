# SDK 직접 통합 — CLI subprocess에서 API SDK로의 전환

> Status: idea
> Created: 2026-04-02
> 출처: Claude Code 소스 분석 (`_research/_util/claude-code`), Gemini SDK, OpenAI SDK

---

## 1. 현재 아키텍처: CLI Subprocess 방식

### 실행 흐름

```
tunaFlow (Rust backend)
  → spawn("claude", ["--print", "--output-format", "json", ...])
  → spawn("codex", ["--full-auto", ...])
  → spawn("gemini", ["-p", prompt])
  → spawn("opencode", ["run", "-p", prompt])
  → stdout 캡처 → 텍스트 파싱 → DB 저장
```

### 장점
- 엔진 추가가 간단 (`agents/` 모듈 + `executor.rs` match arm)
- CLI 도구의 전체 기능을 그대로 활용 (파일 편집, 터미널, MCP 등)
- 인증/설정을 CLI 도구에 위임 (API 키 관리 불필요)

### 한계
| 한계 | 영향 |
|------|------|
| 출력이 자유 형식 텍스트 | 마커 파싱 의존 (`<!-- tunaflow:review-verdict -->` 등), 형식 이탈 시 실패 |
| 정확한 토큰/비용 추적 불가 | trace_log의 usage_status가 추정치, 세션 비용 관리 불가 |
| 스트리밍이 synthetic | Codex JSONL, Gemini 텍스트 — 실시간 progress 제한적 |
| 에이전트 → tunaFlow 콜백 불가 | 마커 감지 → UI 버튼 → 사용자 클릭 → 액션 (3단계 지연) |
| Context caching 불가 | 매 요청마다 identity/plan/skills 전체 재전송 |
| Structured output 불가 | 에이전트 출력 구조를 강제할 수 없음 |
| Tool use/Function calling 불가 | 에이전트가 tunaFlow 기능을 직접 호출할 수 없음 |

---

## 2. Claude Code 소스 분석 — SDK 통합 패턴

### 2.1 API 호출 패턴

Claude Code는 `@anthropic-ai/sdk`를 직접 사용한다. HTTP 레이어 없이 SDK가 전부.

```typescript
// services/api/claude.ts:1822-1831
const result = await anthropic.beta.messages
  .create(
    { ...params, stream: true },
    { signal, headers: { [CLIENT_REQUEST_ID_HEADER]: clientRequestId } },
  )
  .withResponse()
```

요청 구성 (10+ 파라미터):
- `model`, `messages`, `system`, `tools`, `max_tokens`
- `thinking`: adaptive 또는 budget-based
- `betas`: 동적 배열 (tool-search, prompt-caching, structured-outputs, fast-mode 등)
- `speed`: fast 모드
- `metadata`: user ID, session ID, device ID
- `output_config`: task budgets, effort, structured outputs

**tunaFlow 시사점**: 현재 `RunInput { prompt, model, system_prompt, project_path }` 4개 필드 → SDK 전환 시 10+ 파라미터 구조체로 확장 필요. 하지만 `betas` 동적 배열이 기능 분기를 깔끔하게 처리.

### 2.2 스트리밍 아키텍처

```typescript
// claude.ts:1940
for await (const part of stream) {
  resetStreamIdleTimer()
  // content_block_start, content_block_delta, content_block_stop
  // 타입별 분기: text, tool_use, thinking
}
```

핵심 패턴:
- **Idle timeout watchdog** (90초) — 응답 없으면 능동 중단
- **Stall detection** — 청크 간 30초 초과 시 기록
- **Resource cleanup** — `Response.body.cancel()` 명시 호출 (Node.js TLS/소켓 메모리 누수 방지)
- **Non-streaming fallback** — 스트리밍 실패 시 (529 overload, timeout) 동기 요청으로 전환

**tunaFlow 시사점**: 현재 RT의 `run()` 동기 사용 문제를 SDK 네이티브 스트리밍으로 해결 가능. Idle timeout + stall detection은 에이전트 hang 감지에 직접 활용.

### 2.3 Tool Use: 정의 → 호출 → 결과 처리

Claude Code의 Tool 인터페이스:

```typescript
// Tool.ts:362-695
type Tool<Input, Output, Progress> = {
  name: string
  inputSchema: ZodType              // Zod → JSON Schema 자동 변환
  
  call(args, context, ...): Promise<ToolResult>
  checkPermissions(input, context): Promise<PermissionResult>
  validateInput?(input, context): Promise<ValidationResult>
  
  isConcurrencySafe(input): boolean  // true면 병렬 실행 가능
  isReadOnly(input): boolean
  isDestructive?(input): boolean
  
  maxResultSizeChars: number         // 초과 시 디스크 저장 + 참조
}
```

실행 오케스트레이션 (`toolOrchestration.ts`):
1. Tool call을 배치로 분할: read-only → 병렬, non-read-only → 직렬
2. 최대 동시 실행 수 제한 (기본 10)
3. Context modifier는 배치 완료 후 적용

**tunaFlow 시사점**: SDK의 function calling을 통해 에이전트가 tunaFlow 기능을 직접 호출하는 구조가 가능. 마커 파싱 → 구조화된 tool call 전환의 핵심.

### 2.4 System Prompt 조립

```typescript
// systemPrompt.ts:41-123
function buildEffectiveSystemPrompt({
  overrideSystemPrompt,      // 우선순위 1: 강제 (loop mode 등)
  coordinatorSystemPrompt,   // 우선순위 2: coordinator mode
  agentSystemPrompt,         // 우선순위 3: agent definition
  customSystemPrompt,        // 우선순위 4: --system-prompt
  defaultSystemPrompt,       // 우선순위 5: 기본
  appendSystemPrompt,        // 항상 끝에 추가
}): SystemPrompt (string[])
```

- **배열 기반 조립** — 섹션별 cache breakpoint 삽입 가능
- **Memory 블록 lazy loading** — CLAUDE.md, dynamic context
- **Feature-gated 내용** — MCP instructions, Chrome tools 등 조건부 포함

**tunaFlow 시사점**: 현재 `build_normalized_prompt_with_budget()`의 section 기반 조립과 구조적으로 유사. SDK 전환 시 system prompt를 배열로 전달하면 prompt caching 최적화 가능.

### 2.5 비용/토큰 추적

```typescript
// cost-tracker.ts:278-323
function addToTotalSessionCost(cost, usage, model) {
  // 모델별 누적: input, output, cache_read, cache_creation, web_search
  // OTel metrics 발행
  // Advisor 비용 재귀 처리
  // 세션 config에 영속
}
```

- API 응답의 `usage` 필드에서 정확한 토큰 수 추출
- 모델별 단가 테이블로 USD 계산
- 세션 ID 기반 영속 (프로젝트 config에 저장)
- OTel counters로 관측성

**tunaFlow 시사점**: 현재 `trace_log.usage_status`가 추정치인데, SDK 응답의 `usage` 필드로 정확한 추적 가능. 세션 비용 합산 → 프로젝트 비용 관리.

### 2.6 Multi-Agent: Coordinator 패턴

```
Phase 1: Research (병렬)
  coordinator → Agent("Research X"), Agent("Research Y"), Agent("Research Z")
  
Phase 2: Synthesis (coordinator가 직접)
  coordinator가 findings를 이해하고 구체적 implementation spec 작성

Phase 3: Implementation (직렬 또는 병렬)
  coordinator → SendMessage(agent, "Fix null pointer in src/auth/validate.ts:42...")

Phase 4: Verification
  coordinator → Agent(subagent_type: "verification", ...)
```

핵심 원칙:
- **Workers는 stateless** — self-contained prompt 필수
- **Coordinator는 stateful** — 모든 worker 결과를 읽음
- **Synthesis 필수** — research → implementation 사이에 coordinator가 반드시 이해
- **Context overlap** — 재사용 가능하면 `SendMessage`, 아니면 새 `Agent` 생성

**tunaFlow 시사점**: 현재 워크플로우(Architect → Developer → Reviewer)와 구조적으로 유사하지만, tunaFlow는 각 단계가 별도 Branch/RT인 반면 Claude Code는 coordinator가 단일 세션에서 관리. SDK 전환 시 coordinator 패턴의 "synthesis 필수" 원칙 채택 가치 있음.

### 2.7 Skill/Plugin/MCP 아키텍처

**Skills**: frontmatter 기반 정의 + tool 허용목록 + hook 등록
```typescript
type BundledSkillDefinition = {
  name: string
  allowedTools?: string[]        // 이 skill에서 사용 가능한 tools
  context?: 'inline' | 'fork'   // 실행 컨텍스트
  agent?: string                 // 위임할 agent 타입
  hooks?: HooksSettings          // pre/post hooks
}
```

**Plugins**: skill + hook + MCP server를 번들로 제공
```typescript
type BuiltinPluginDefinition = {
  skills?: BundledSkillDefinition[]
  hooks?: HooksConfig
  mcpServers?: MCPConfig[]
}
```

**MCP**: 양방향 JSON-RPC, 도구 발견/실행, OAuth 인증
- Agent별 MCP server 분리 (parent + agent-specific)
- Tool wrapping으로 인증/권한 처리

**tunaFlow 시사점**: tunaFlow의 Skills snapshot 시스템은 읽기 전용 주입. Claude Code의 Skills는 tool 허용목록 + 실행 컨텍스트 분리까지 포함. SDK 전환 시 "skill이 에이전트의 tool set을 제한"하는 패턴 채택 가능.

---

## 3. SDK 전환으로 열리는 구체적 시나리오

### 3.1 Function Calling → 마커 대체

**현재**:
```markdown
<!-- tunaflow:review-verdict -->
verdict: pass
findings:
- 구현이 plan과 일치합니다
<!-- /tunaflow:review-verdict -->
```
에이전트가 자유 형식으로 출력 → 파서가 마커 추출 → 형식 이탈 시 실패

**SDK 전환 후**:
```typescript
// Gemini SDK
const result = await model.generateContent({
  contents: [...],
  tools: [{
    functionDeclarations: [{
      name: "submit_review_verdict",
      parameters: {
        type: "object",
        properties: {
          verdict: { type: "string", enum: ["pass", "fail", "conditional"] },
          findings: {
            type: "array",
            items: {
              type: "object",
              properties: {
                severity: { type: "string", enum: ["critical", "major", "minor"] },
                file: { type: "string" },
                line: { type: "number" },
                description: { type: "string" }
              }
            }
          },
          recommendations: {
            type: "array",
            items: { type: "object", properties: { action: { type: "string" }, priority: { type: "number" } } }
          }
        },
        required: ["verdict", "findings"]
      }
    }]
  }]
});

// OpenAI SDK
const response = await openai.chat.completions.create({
  model: "gpt-4o",
  messages: [...],
  tools: [{
    type: "function",
    function: {
      name: "submit_review_verdict",
      parameters: { /* 동일 JSON Schema */ }
    }
  }]
});
```

에이전트가 **구조화된 JSON**으로 반환 → 파서 불필요, 스키마 검증 자동.

**적용 대상**:

| 현재 마커 | SDK tool call |
|----------|---------------|
| `plan-proposal` | `submit_plan_proposal({ title, description, subtasks[] })` |
| `impl-plan` | `submit_impl_plan({ files[], dependencies[], risks[] })` |
| `impl-complete` | `mark_implementation_complete({ summary, completedSubtasks[] })` |
| `subtask-done:N` | `mark_subtask_done({ subtaskNumber, summary })` |
| `review-verdict` | `submit_review_verdict({ verdict, findings[], recommendations[] })` |

**`smallModelStressTesterIdea.md`와의 연결**: 마커를 JSON 스키마로 강화하면 소형 모델 stress test가 의미를 가진다. SDK의 function calling이 바로 그 "JSON 스키마 + 검증기" 패턴.

### 3.2 에이전트 → tunaFlow 콜백 (Tool Use)

SDK tool use로 에이전트가 tunaFlow 기능을 직접 호출:

```typescript
// 에이전트에게 제공하는 tools
const tunaFlowTools = [
  // 워크플로우 제어
  { name: "mark_subtask_done", parameters: { subtask_id: "string" } },
  { name: "save_artifact", parameters: { title: "string", content: "string", type: "string" } },
  { name: "request_plan_revision", parameters: { reason: "string", suggested_changes: "string" } },
  
  // 코드베이스 접근
  { name: "search_codebase", parameters: { query: "string", limit: "number" } },
  { name: "read_file", parameters: { path: "string", start_line: "number", end_line: "number" } },
  
  // 컨텍스트 확장
  { name: "fetch_skill", parameters: { skill_name: "string" } },
  { name: "get_plan_details", parameters: { plan_id: "string" } },
];
```

**현재 vs SDK**:
```
현재:  에이전트 출력 → 마커 감지 → UI 버튼 → 사용자 클릭 → 액션 (3단계)
SDK:   에이전트 → tool call → tunaFlow 자동 실행 → 결과 반환 → 에이전트 계속 (0단계)
```

**Claude Code 참고 패턴**:
- `isConcurrencySafe`: read-only tools는 병렬, write tools는 직렬
- `checkPermissions`: tool별 권한 검사 (에이전트 역할에 따라 제한)
- `maxResultSizeChars`: 큰 결과는 디스크 저장 + 참조

### 3.3 Context Caching (Gemini)

```typescript
// Gemini SDK context caching
const cache = await cacheManager.create({
  model: "gemini-2.5-pro",
  contents: [
    // Static sections (대화 중 변하지 않는 부분)
    { role: "user", parts: [{ text: identityBlock }] },    // ~500 tokens
    { role: "user", parts: [{ text: planContext }] },       // ~2000 tokens
    { role: "user", parts: [{ text: skillsBlock }] },       // ~3000 tokens
  ],
  ttlSeconds: 3600,
});

// 이후 요청은 캐시 참조 + 동적 부분만 전송
const result = await model.generateContent({
  cachedContent: cache.name,
  contents: [
    // Dynamic sections만
    { role: "user", parts: [{ text: userPrompt }] },
  ],
});
```

**비용 절감 추정**:
- ContextPack 평균 크기: ~15,000 tokens
- Static 비율 (identity + plan + skills): ~60% = ~9,000 tokens
- 캐싱 시 입력 비용: 9,000 × $0.00025/1K (캐시) + 6,000 × $0.00125/1K (일반) = **$9.75 → $2.25 + $7.50 절감**
- 실제 절감율: 요청당 ~40-50% (Gemini 캐시 가격 = 일반의 1/5)

**Claude Code 참고 패턴**:
- `cache_control` breakpoint를 system prompt 배열의 특정 위치에 삽입
- `prompt_caching_scope` beta로 활성화
- `cache_read_input_tokens` / `cache_creation_input_tokens` 별도 추적

### 3.4 네이티브 스트리밍 — RT 실시간 가시성

**현재**: RT에서 `run()` 동기 호출 → 참가자 실행 중 progress 없음

```typescript
// OpenAI SDK 네이티브 스트리밍
const stream = await openai.chat.completions.create({
  model: "gpt-4o",
  messages: [...],
  stream: true,
});

for await (const chunk of stream) {
  // 참가자별 실시간 청크 emit
  emit("roundtable:chunk", {
    participantName: participant.name,
    engine: participant.engine,
    text: chunk.choices[0]?.delta?.content ?? "",
  });
}

// Gemini SDK 네이티브 스트리밍
const result = await model.generateContentStream({
  contents: [...],
});

for await (const chunk of result.stream) {
  emit("roundtable:chunk", {
    participantName: participant.name,
    engine: participant.engine,
    text: chunk.text(),
  });
}
```

**Claude Code 참고 패턴**:
- Idle timeout watchdog (90초) — 응답 멈춤 감지
- Stall detection (30초) — 청크 간 지연 기록
- Resource cleanup — `Response.body.cancel()` 명시 호출

### 3.5 Embeddings API — Vector DB

`vectorDbAndRetrievalAlgorithmsIdea.md`의 Hybrid Search 구현에 직결:

```typescript
// OpenAI Embeddings
const embedding = await openai.embeddings.create({
  model: "text-embedding-3-small",
  input: "검색 쿼리",
  dimensions: 256,  // MRL 차원 축소
});

// Gemini Embeddings
const result = await model.embedContent({
  content: "검색 쿼리",
  taskType: "RETRIEVAL_QUERY",
});
```

현재 rawq가 로컬 임베딩(sidecar)을 담당하지만, SDK embeddings는:
- 더 높은 품질의 임베딩 (대형 모델 기반)
- MRL (Matryoshka) 차원 축소로 저장 효율
- Cross-session retrieval에서 의미적 유사도 향상

rawq(로컬, 빠름, 무료) + SDK embeddings(원격, 정확, 유료)의 하이브리드 가능.

---

## 4. 아키텍처 전환 설계

### 4.1 Dual Path: CLI + SDK 공존

Claude는 SDK가 제한적이므로(Claude Code CLI가 파일 편집/터미널 등 전체 기능 제공), dual path 필수:

```
┌─────────────────────────────────────────────────────────┐
│                    EngineAdapter trait                    │
│  fn send(request: EngineRequest) -> Stream<EngineChunk>  │
│  fn embed(text: &str) -> Vec<f32>                        │
│  fn capabilities() -> EngineCapabilities                 │
├─────────────┬──────────────┬──────────────┬──────────────┤
│ ClaudeAdapter│ GeminiAdapter│ OpenAIAdapter│ OpenCodeAdapt│
│ (CLI spawn)  │ (SDK HTTP)   │ (SDK HTTP)   │ (CLI spawn)  │
│              │              │              │              │
│ subprocess   │ reqwest +    │ reqwest +    │ subprocess   │
│ stdout parse │ SSE stream   │ SSE stream   │ stdout parse │
│              │ tool calls   │ tool calls   │              │
│              │ caching      │ embeddings   │              │
│              │ embeddings   │              │              │
└─────────────┴──────────────┴──────────────┴──────────────┘
```

### 4.2 EngineRequest 확장

```rust
// 현재
pub struct RunInput {
    pub prompt: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub project_path: Option<String>,
}

// SDK 전환 후
pub struct EngineRequest {
    pub prompt: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    pub project_path: Option<String>,
    
    // SDK-specific
    pub tools: Option<Vec<ToolDefinition>>,       // function calling
    pub structured_output: Option<JsonSchema>,     // 구조화 출력 강제
    pub cache_key: Option<String>,                 // context caching
    pub stream: bool,                              // 스트리밍 여부
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

pub struct EngineResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,                 // function call 결과
    pub usage: TokenUsage,                         // 정확한 토큰 수
    pub cost_usd: Option<f64>,                     // 정확한 비용
    pub model: String,                             // 실제 사용된 모델
    pub cached_tokens: Option<u32>,                // 캐시 히트 토큰
}
```

### 4.3 ContextPack 캐시 분리

```rust
// ContextPack을 static/dynamic으로 분리
pub struct ContextPackStatic {
    pub identity: String,           // 세션 중 불변
    pub plan_section: Option<String>, // plan 승인 후 불변
    pub skills: String,             // 세션 중 불변
    pub platform_tier0: String,     // 항상 불변
}

pub struct ContextPackDynamic {
    pub recent_context: String,     // 매 요청 변경
    pub retrieval_chunks: String,   // 매 요청 변경
    pub compressed_memory: Option<String>, // 가끔 변경
    pub cross_session: Option<String>,     // 가끔 변경
}

// Gemini: static → cachedContents, dynamic → contents
// OpenAI: static → system prompt (자동 캐싱), dynamic → user messages
// Claude: 전체 합산 → CLI prompt (현재와 동일)
```

### 4.4 Tool Call Handler

```rust
pub struct ToolCallHandler {
    tools: HashMap<String, Box<dyn ToolHandler>>,
}

#[async_trait]
trait ToolHandler {
    fn schema(&self) -> JsonSchema;
    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<serde_json::Value>;
    fn is_read_only(&self) -> bool;
    fn requires_approval(&self) -> bool;  // 사용자 확인 필요 여부
}

// 구현 예시
struct MarkSubtaskDoneHandler;
impl ToolHandler for MarkSubtaskDoneHandler {
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let subtask_id = input["subtask_id"].as_str()?;
        update_subtask_status(&ctx.conn, subtask_id, "done")?;
        Ok(json!({ "status": "success" }))
    }
    fn is_read_only(&self) -> bool { false }
    fn requires_approval(&self) -> bool { false }  // 자동 실행
}
```

### 4.5 인증 관리

```rust
pub enum AuthMethod {
    ApiKey(String),           // 환경변수 또는 설정
    OAuth { token: String, refresh: String },  // Claude Code 패턴
    CliDelegated,             // CLI 도구에 위임 (현재 방식)
}

// Gemini: API 키 (GEMINI_API_KEY) 또는 Google Cloud ADC
// OpenAI: API 키 (OPENAI_API_KEY)
// Claude: CLI 위임 유지 (또는 Anthropic API 키)
// OpenCode: CLI 위임 유지
```

---

## 5. Claude Code에서 채택할 패턴

### 5.1 직접 채택 (높은 가치)

| 패턴 | Claude Code 구현 | tunaFlow 적용 |
|------|-----------------|---------------|
| **Tool 동시성 분류** | `isConcurrencySafe` + 배치 분할 | RT에서 read-only tool은 병렬, write tool은 직렬 |
| **Idle timeout watchdog** | 90초 timer + 능동 중단 | 에이전트 hang 감지 + 사용자 알림 |
| **비용 추적 파이프라인** | usage → 모델별 누적 → 세션 영속 | trace_log 정확도 개선 + 프로젝트 비용 관리 |
| **System prompt 배열 조립** | 섹션별 cache breakpoint | ContextPack section → 배열 변환 + caching |
| **Resource cleanup** | `Response.body.cancel()` | Rust reqwest 스트림 명시 정리 |

### 5.2 변형 채택 (tunaFlow 맞춤)

| 패턴 | Claude Code 방식 | tunaFlow 변형 |
|------|-----------------|---------------|
| **Coordinator** | 단일 세션, Agent tool로 worker 생성 | Branch 기반 분리 유지, SDK tool call로 자동화 강화 |
| **Skill → Tool 허용목록** | skill.allowedTools 필드 | 역할별 tool 제한 (Developer: read+write, Reviewer: read-only) |
| **Plan mode enforcement** | 환경변수 기반 | 워크플로우 phase 기반 (impl phase에서만 write 허용) |
| **Verification agent** | 별도 agent type, project 수정 금지 | Review RT의 검증 참가자에 적용 |

### 5.3 불채택 (tunaFlow와 맞지 않음)

| 패턴 | 이유 |
|------|------|
| **Agent-in-agent 재귀** | tunaFlow는 Branch/RT로 분리, 재귀 생성 불필요 |
| **AsyncLocalStorage 격리** | Rust backend는 thread 기반, 다른 격리 패턴 사용 |
| **MCP 서버 직접 관리** | tunaFlow는 에이전트 CLI의 MCP를 그대로 활용 |
| **GrowthBook/Statsig** | tunaFlow는 로컬 앱, feature flag 불필요 |

---

## 6. 엔진별 SDK 가용 기능

| 기능 | Claude (Anthropic SDK) | Gemini (Google AI SDK) | OpenAI SDK |
|------|----------------------|----------------------|------------|
| Streaming | O (SSE) | O (SSE) | O (SSE) |
| Function calling | O (tool_use) | O (functionDeclarations) | O (tools) |
| Structured output | O (output_config) | O (responseSchema) | O (response_format) |
| Context caching | O (prompt caching, 자동) | O (cachedContents, 명시적) | O (자동 prompt caching) |
| Embeddings | X (별도 API 없음) | O (embedContent) | O (embeddings.create) |
| Vision/Multimodal | O | O | O |
| System prompt 분리 | O (system 필드) | O (systemInstruction) | O (system role) |
| Token counting | O (usage 응답) | O (usageMetadata) | O (usage 응답) |
| Batch API | O | O | O |

### Claude SDK 직접 사용 검토

현재 Claude는 CLI(`claude --print`)로 호출하는데, **Anthropic SDK를 직접 사용하는 것도 가능**:

```rust
// Rust: anthropic-sdk 또는 reqwest로 직접 HTTP
let response = client.post("https://api.anthropic.com/v1/messages")
    .header("x-api-key", api_key)
    .header("anthropic-version", "2023-06-01")
    .json(&request)
    .send().await?;
```

장점: tool use, streaming, 정확한 usage, prompt caching 모두 가능
단점: Claude Code CLI의 파일 편집/터미널/MCP 기능을 잃음

**판단**: 워크플로우 파이프라인(Developer/Reviewer)은 SDK가 더 적합. 일반 대화는 CLI 유지.

---

## 7. 구현 로드맵

### Phase 1: Gemini SDK 직접 통합 (P0)

Gemini는 현재도 CLI(`gemini -p`)가 제한적이므로 SDK 전환 가치가 가장 높다.

```
변경 파일:
1. src-tauri/Cargo.toml                    — reqwest + serde 의존성
2. src-tauri/src/agents/gemini_sdk.rs      — SDK HTTP 호출 (~200줄)
3. src-tauri/src/agents/mod.rs             — gemini_sdk 모듈 추가
4. src-tauri/src/commands/agents_helpers/executor.rs — "gemini" match arm 변경
5. src-tauri/src/db/models.rs              — TokenUsage 구조체 확장
```

검증 기준:
- 스트리밍 동작 (실시간 청크)
- Function calling 동작 (review-verdict 구조화 반환)
- 정확한 토큰/비용 추적
- Context caching 비용 절감 측정

### Phase 2: OpenAI SDK 직접 통합 (P0)

Codex CLI를 OpenAI Chat Completions API로 대체.

```
변경 파일:
1. src-tauri/src/agents/openai_sdk.rs      — SDK HTTP 호출 (~200줄)
2. executor.rs match arm 변경
```

### Phase 3: Tool Call Handler 프레임워크 (P1)

에이전트가 호출할 수 있는 tunaFlow tool 정의 + 실행 프레임워크.

```
tools:
- mark_subtask_done(subtask_id)
- submit_review_verdict(verdict, findings[], recommendations[])
- submit_plan_proposal(title, description, subtasks[])
- save_artifact(title, content, type)
- search_codebase(query)
- read_file(path)
```

### Phase 4: Claude SDK 선택적 통합 (P1)

워크플로우 파이프라인(Developer/Reviewer)은 Anthropic API 직접 호출.
일반 대화는 Claude CLI 유지 (파일 편집, MCP 등 활용).

### Phase 5: Context Caching 최적화 (P2)

ContextPack static/dynamic 분리 + 엔진별 캐싱 전략.

### Phase 6: Embeddings 통합 (P2)

rawq + SDK embeddings 하이브리드 → Vector DB 연동.

---

## 8. 리스크와 트레이드오프

| 리스크 | 영향 | 완화 |
|--------|------|------|
| **CLI 기능 손실** (Claude: 파일 편집, 터미널, MCP) | Developer 역할 품질 저하 | 워크플로우용 SDK + 대화용 CLI dual path |
| **API 키 관리 부담** | 사용자 설정 복잡도 증가 | CLI 인증 재사용 또는 키 입력 UI |
| **Rate limit 직접 처리** | CLI가 해주던 retry/backoff 직접 구현 | Claude Code의 retry 패턴 채택 |
| **SDK 버전 호환성** | API 변경 시 업데이트 필요 | reqwest 직접 HTTP → SDK 의존성 최소화 |
| **Dual path 복잡도** | 유지보수 비용 증가 | EngineAdapter trait로 인터페이스 통일 |

### 핵심 판단

**CLI 방식은 "에이전트를 외부 도구로 호출"하는 패턴이고, SDK 전환은 "에이전트를 런타임 파트너로 통합"하는 전환이다.**

tunaFlow의 철학 — "에이전트가 편해야 결과가 좋다" — 과 정확히 맞는다. 에이전트가 tunaFlow의 tool을 직접 호출하고, 구조화된 출력으로 응답하고, 캐시된 컨텍스트로 빠르게 동작하는 것이 "에이전트가 편한" 환경.

다만 **모든 엔진을 한번에 전환하는 것은 과도한 리스크**. Gemini SDK → OpenAI SDK → Claude SDK(선택적) 순서로 점진 전환하되, 각 단계마다 CLI fallback 유지.

---

## 참고 자료

- Claude Code 소스: `/Users/d9ng/privateProject/_research/_util/claude-code/`
  - API 통합: `src/services/api/claude.ts` (3,419줄)
  - Tool 정의: `src/Tool.ts` (793줄)
  - Tool 오케스트레이션: `src/services/tools/toolOrchestration.ts` (189줄)
  - 비용 추적: `src/cost-tracker.ts` (324줄)
  - Coordinator: `src/coordinator/coordinatorMode.ts`
  - Agent tool: `src/tools/AgentTool/AgentTool.tsx`
- Gemini API: https://ai.google.dev/gemini-api/docs
- OpenAI API: https://platform.openai.com/docs/api-reference
- Anthropic API: https://docs.anthropic.com/en/api
- tunaFlow 관련 문서:
  - 마커 파서: `src/lib/planProposalParser.ts`
  - 워크플로우: `src/lib/workflowOrchestration.ts`
  - ContextPack: `src-tauri/src/commands/agents_helpers/send_common.rs`
  - 소형 모델 분석: `docs/ideas/smallModelStressTesterIdea.md`
  - RT 알고리즘: `docs/ideas/rtAlgorithmEnhancementIdeas.md`
  - Vector DB: `docs/ideas/vectorDbAndRetrievalAlgorithmsIdea.md`
