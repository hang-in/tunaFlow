---
title: Codex / Gemini 온보딩 분석 응답 형식 audit
status: draft (코드 분석 기반 — 실측 raw 응답은 사용자/QA 환경에서 별도 수집 필요)
created_at: 2026-04-25
updated_at: 2026-04-25
canonical: false  # 가설 + 코드 분석 기록. 실측 결과 수집되면 본 문서 갱신.
related:
  - docs/plans/codexGeminiOnboardingAnalysisFailureInvestigationPlan_2026-04-25.md
  - https://github.com/hang-in/tunaFlow/issues/176
  - src-tauri/src/commands/project_onboarding.rs
---

# 1. 목적

Plan `codexGeminiOnboardingAnalysisFailureInvestigationPlan_2026-04-25.md` Step 1
("Audit — raw 응답 capture") 결과 정리. **모든 CLI 엔진이 동일한 dispatcher
(`call_cli_agent`) 를 거치지만 Codex / Gemini 만 분석 실패**하는 원인을 코드
경로와 모델 응답 패턴으로 추적한다.

> 본 문서는 실제 CLI 를 호출해 stdout 을 dump 한 결과는 아니다. CLI 환경
> (네트워크 + 사용자 인증) 이 worktree 에서 가용하지 않기 때문이다. 대신
> `project_onboarding.rs` 의 호출 경로를 정밀 분석하고, 각 엔진 CLI 의 일반적인
> 출력 패턴을 기반으로 가설을 좁힌다. 후속 세션이 raw 응답을 수집하면 §6
> "관찰 결과" 섹션을 채워 둔다.

# 2. 호출 경로 사실 정리

`src-tauri/src/commands/project_onboarding.rs:586-602` 의 dispatcher.

```rust
let text = match engine {
    "claude" | "codex" | "gemini" => {
        call_cli_agent(engine, engine, prompt, model, cancel).await?
    }
    "ollama"   => call_ollama(prompt, m, ep, cancel).await?,
    "lmstudio" => call_openai_compat(prompt, m, ep, cancel).await?,
    other      => return Err(format!("지원하지 않는 엔진: {other}")),
};
parse_output(&text)
```

세 CLI 엔진 모두 **동일한 prompt** 를 받고, 동일한 `parse_output` 으로 들어간다.
즉 분기점은 **(a) `call_cli_agent` 내부 CLI argv** 와 **(b) CLI subprocess 의
stdout 형식** 두 가지뿐이다.

## 2.1 `call_cli_agent` argv 차이 (project_onboarding.rs:434-449)

| 엔진 | argv | prompt 전달 방식 | output mode |
|---|---|---|---|
| `claude` | `claude -p <prompt> --max-turns 1 --output-format text [--model M]` | argv | `text` (JSON / stream-json 모드 아님) |
| `gemini` | `gemini -p <prompt> [-m M]` | argv | 기본 (모드 플래그 미지정) |
| `codex`  | `codex exec --full-auto - [--model M]` | **stdin** | 기본 (모드 플래그 미지정) |

핵심 관찰:

- Claude 만 `--output-format text` 가 명시. → 이게 정해진 plain-text 모드
  (no JSON wrapping, no envelope) 를 강제하는 핵심.
- Codex / Gemini 는 **출력 형식 플래그 없음**. 즉 각 CLI 의 default 출력으로
  떨어진다.
- `cmd.stderr(Stdio::null())` 라 stderr 는 버려진다. 만약 CLI 가 진단 메시지를
  stderr 로 따로 흘려도 우리는 stdout 만 읽는다.

## 2.2 `parse_output` 의 strict 가정 (project_onboarding.rs:358-381)

```rust
let claude_md = extract_between(text, "[CLAUDE_MD_START]", "[CLAUDE_MD_END]")
    .ok_or("AI 응답에서 CLAUDE.md 섹션을 찾을 수 없습니다")?;
let ref_index = extract_between(text, "[REF_INDEX_START]", "[REF_INDEX_END]")
    .ok_or("AI 응답에서 Reference Index 섹션을 찾을 수 없습니다")?;
```

`extract_between` 은 단순 substring search 다 (line 383-387). 즉:

- 마커 글자가 **그대로 들어간 경우만** OK.
- `\[CLAUDE_MD_START\]` (백슬래시 escape), ` [CLAUDE_MD_START] ` (공백
  surrounded 는 OK), `[ CLAUDE_MD_START ]` (마커 안에 공백) 등의 변형은 **모두 fail**.
- markdown fence 안에 들어가도 substring 매칭 자체는 OK (텍스트로 그대로
  포함되면 통과). 다만 fence 가 그 마커를 **에스케이핑** 하면 (예: 시각용으로
  대괄호 escape) 깨진다.

## 2.3 prompt 의 출력 지시 (project_onboarding.rs:266)

```
아래 두 섹션을 정확한 마커와 함께 출력하세요. 마커 외에 다른 텍스트는 추가하지 마세요.
```

지시는 한국어 1문장. "정확한 마커", "마커 외 다른 텍스트 금지" 만 명시. 다음
요소는 명시되어 있지 않다:

1. **markdown code fence 금지** (` ```markdown ... ``` ` 으로 감싸지 말 것)
2. **introduction / preamble 금지** (`다음과 같이 정리했습니다:` 같은 prefix
   금지)
3. **마커 자체를 escape 하지 말 것** (e.g. `\[CLAUDE_MD_START\]` 금지)
4. 마커는 **영어 대괄호 + ASCII** 그대로 사용 (한국어 모델이 fullwidth 괄호 등으로
   변환하지 않도록)

# 3. 가설 (실패 원인 후보)

## H1 — Gemini 가 markdown 으로 응답을 감싼다 (가능성 ★★★)

Gemini CLI 는 default 모드에서 응답을 **사람용 markdown 출력** 으로 렌더링하는
경향이 있다. 또한 Gemini 모델 자체가 자주 ` ```markdown ... ``` ` 또는 다른 언어
fence 로 응답을 감싼다.

마커가 fence 안쪽에 있어도 substring 매칭은 통과하지만, 다음과 같은 변형이면
fail:

- ` ``` ` 펜스 안에 들어가서 escape 가 발생 (대괄호 escape 는 거의 없으니
  현실적으로는 fence 자체가 문제는 아님 — 마커는 그대로 통과)
- 그러나 prompt 끝에 "마커 외 다른 텍스트는 추가하지 마세요" 가 있어도 Gemini 가
  "Of course! Here's the file..." 같은 introduction 을 붙이면 → 마커는 여전히
  포함되므로 substring 은 통과. 실패 원인이 되지 않음.

→ 결론: H1 단독으로는 실패를 100% 설명 못 한다. **H4** (마커 자체 변형) 가
보조 가설로 필요하다.

## H2 — Codex 가 응답을 JSON envelope 로 감싼다 (가능성 ★★)

Codex CLI 의 `exec` 모드는 기본적으로 plain-text 출력이지만, `exec` mode 가
업데이트되면서 **JSON 진행 로그 / tool use trace** 를 stdout 에 섞을 수 있다.
정확한 동작은 codex 버전에 따라 달라진다.

만약 stdout 이 다음과 같다면:

```
[2026-04-25 10:00:01Z] starting model gpt-5-codex
[2026-04-25 10:00:05Z] thinking...
[CLAUDE_MD_START]
...
```

마커는 여전히 포함되므로 substring 매칭은 통과.

그러나 **codex 가 응답 본체를 JSON 으로 출력** 하는 모드라면:

```json
{"role":"assistant","content":"[CLAUDE_MD_START]\n# foo\n[CLAUDE_MD_END]\n..."}
```

여기서 `\n` 이 escape 된 형태로 들어가면 `extract_between` 은 substring 으로
열리는 마커는 찾지만, `[CLAUDE_MD_END]` 까지 사이의 본문에 escape 된 `\n` 이
literally `\n` 두 글자로 들어와 있으면 결과가 깨진 markdown 으로 추출된다 →
parse 단계에서는 OK 일 수 있으나 사용자 단계에서 깨진 출력.

여기에 더해 codex 의 `exec` 가 응답 끝에 **반드시** progress / status footer 를
붙이면 (`done in 5.3s`) → 마커 매칭은 여전히 통과. 실패 직접 원인은 아니다.

→ 결론: H2 도 실패를 단독 설명하지 못함. **H4** 또는 **CLI 가 비정상 exit 코드
반환** (project_onboarding.rs:411 `if !output.status.success()` 에서 fail) 가
유력.

## H3 — CLI exit code (가능성 ★★★)

`await_cli_with_cancel` (project_onboarding.rs:392-419) 의:

```rust
if !output.status.success() {
    return Err(format!("{engine} 분석 실패 (exit: {:?})", output.status.code()));
}
```

가능성:

- Codex `exec --full-auto` 가 **인증 미설정 / 모델 불일치** 시 non-zero exit
- Gemini CLI 가 **API key 없음 / quota 초과** 시 non-zero exit (claude 는
  보통 stdout 으로 친절한 에러 메시지를 텍스트로 뽑고 exit 0)
- Codex 가 `--model` 인자에 잘못된 모델명을 받았을 때 non-zero exit (M2 16GB
  사용자가 default 모델을 쓰면 plan §2.1 에 명시된 codex 의 `--model M` 미지정
  케이스를 탄다)

이 경우 `parse_output` 도달 전에 에러로 끝난다. 사용자에게는
`project:onboarding:error` 의 `message` 가 `"codex 분석 실패 (exit: Some(1))"` 로
보일 것이다.

→ Issue #176 follow-up 의 정확한 에러 message 가 무엇이었는지 확인 필요. 만약
"분석 실패 (exit: ...)" 류 라면 **H3 가 root cause** 이고 본 plan 의 fix 방향은
바뀌어야 한다 (parse 가 아니라 CLI invocation 자체 문제).

## H4 — 모델이 마커 자체를 변형 (가능성 ★★★)

가장 흔한 모드 실패. 한국어로 prompt 를 받은 모델이:

- `[CLAUDE_MD_START]` → `【CLAUDE_MD_START】` (fullwidth 괄호) 로 변환
- `[CLAUDE_MD_START]` → `**[CLAUDE_MD_START]**` 로 markdown bold 처리
- `[CLAUDE_MD_START]` → `[CLAUDE\_MD\_START]` 로 underscore escape (markdown
  안전)
- 마커를 ` ## [CLAUDE_MD_START]` 처럼 헤더로 변형

substring 매칭은 모두 fail. claude 는 `[CLAUDE_MD_START]` 마커를 본 학습량이
많아 그대로 출력할 가능성이 높지만, Gemini / Codex 는 이를 본 적 없는 임의의
포맷 지시로 받아들이고 markdown 친화 형태로 변형할 가능성이 높다.

## H5 — Gemini 가 한 번 더 codeblock 안에 마커를 넣고 끝나지 않음 (가능성 ★★)

Gemini 가 응답을 다음과 같이 출력:

````
다음과 같이 정리했습니다:

```markdown
[CLAUDE_MD_START]
# foo
[CLAUDE_MD_END]

[REF_INDEX_START]
# bar
[REF_INDEX_END]
```

추가로 도와드릴 점이 있으면 말씀해 주세요.
````

이 경우 **substring 매칭 자체는 통과**. claude_md = "# foo", ref_index = "# bar"
가 정상 추출. → 실패 직접 원인이 아님.

# 4. 시나리오별 fix 우선순위

| 가설 | 가능성 | 영향 | fix 위치 |
|---|---|---|---|
| H1 markdown wrap | ★★★ | 매칭 자체에는 영향 적음 | (선택) parse 측에서 ` ``` ` strip |
| H2 codex JSON | ★★ | 변형 형태에 따라 가변 | (선택) JSON envelope detection |
| H3 exit code | ★★★ | 직접 fail (parse 도달 전) | **CLI argv 확정성 + 사용자에게 명확한 에러 메시지** |
| H4 마커 변형 | ★★★ | 직접 fail | **prompt 강화 + parse 측 looser 매칭** |
| H5 fence-in-fence | ★★ | 보통 통과 | n/a |

→ **Layer A (parse looser) + Layer B (prompt 강화)** 가 H1, H4 를 동시에 흡수.
H3 는 별도 문제 (CLI 환경) 라 본 plan 의 범위를 벗어남 (별 이슈로 뺄 것). H2 는
실측 후 결정.

# 5. fix 의 구체 형태 (본 plan Layer A/B 적용 후)

## Layer A — `parse_output` 강건화

substring 검색 → **regex 기반 looser matching** 으로 교체.

매칭 허용 패턴:

- 마커 좌우 공백 / newline 자유
- 마커 자체가 markdown bold (`**[CLAUDE_MD_START]**`) 로 감싸진 케이스
- 마커 앞에 fence opener (` ```markdown` 또는 ` ``` `) 가 한 줄 있는 케이스
- 마커 뒤에 fence closer (` ``` `) 가 한 줄 있는 케이스
- 한국어 fullwidth 괄호 `【】` 변형 흡수 (선택, low priority)

마커 매칭 실패 시:

- error message 에 raw 응답의 앞 200자를 포함 (디버깅용 단서)

## Layer B — prompt 의 출력 지시 강화

prompt 끝부분에 다음 지시 추가:

```
**중요**: 다음 규칙을 정확히 지키세요.
1. 응답의 첫 줄은 [CLAUDE_MD_START] 로 시작.
2. 마커는 영문 대괄호 그대로 (변형/볼드/escape 금지).
3. 마커 외 다른 텍스트(인사, 설명, 결론) 금지.
4. markdown code fence (```) 로 감싸지 마세요.
5. 모든 섹션이 끝나면 [INITIAL_SETUP_END] 직후 즉시 종료.
```

# 6. 관찰 결과 (실측 후 채우기)

> 후속 세션이 각 엔진별 raw stdout 을 trace_log 에 dump 한 후 본 섹션을 채운다.
> 현재는 코드 분석에 그친다.

| 엔진 | exit code | stdout 첫 100자 | 마커 형식 보존 여부 | parse 통과 |
|---|---|---|---|---|
| claude | TBD | TBD | TBD | TBD |
| codex  | TBD | TBD | TBD | TBD |
| gemini | TBD | TBD | TBD | TBD |

# 7. 결론

Plan 의 Layer A + Layer B 를 우선 적용 (본 PR). H3 는 실측 후 별 이슈로 분리.
Layer C (JSON 응답 강제) 는 본 PR 범위 밖 — 변경 표면이 크고 H3 가 root cause
이면 효과가 없다 (CLI 단계에서 fail). 따라서 본 PR 에서는 다루지 않는다.
