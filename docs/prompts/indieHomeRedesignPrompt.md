---
name: 인디 AOC 홈페이지 재작성 프롬프트 (tunaFlow_home)
status: ready
created_at: 2026-04-16
canonical: true
target_tools: [v0.dev, bolt.new, Claude, Cursor]
related:
  - ~/privateProject/tunaFlow_home  (현재 v0 결과물)
  - docs/posts/01-에이전트에게-프로세스를-줘라.md  (시리즈 톤)
  - docs/posts/side-01-gemini와-22분-혈투기.md     (외전 톤)
---

# tunaFlow 홈페이지 재작성 프롬프트

> 이 프롬프트는 **v0/bolt/Claude 중 어느 것에 던져도 통하도록** 자기완결적으로 작성되어 있습니다.
> 그대로 복사 → 도구에 붙여넣기 → 결과물에 실제 YouTube 링크/스크린샷만 교체하면 됩니다.
>
> **왜 재작성?** v0이 1차로 만든 결과물(`tunaFlow_home/`)이 Vercel/Linear/Notion 류 "SaaS 랜딩 템플릿"을 찍어냈음. 7섹션(Hero/Workflow/Surfaces/Comparison/Architecture/CTA/Footer), 광고식 헤드라인("Agent-Orchestrated Software Development, Grounded in Real Project Context"), 가짜 status chip(`tokens: 4,102`), "Watch Demo" CTA — 전형적 엔터프라이즈 피칭 구조. tunaFlow는 **1인 개발자가 만든 인디 데스크톱 도구**라 톤이 완전히 다르게 가야 함.

---

## 프롬프트 본문 (이하 복사 대상)

---

# Goal

Rewrite the homepage for **tunaFlow** — a desktop Agent Orchestration Client (AOC) built by a single indie developer.

The current version at `tunaFlow_home/` feels like a generic SaaS landing: 7 sections, enterprise pitch copy, fake dashboard widgets, "Watch Demo" CTAs. I want to strip it back to something closer to how indie developer tools present themselves on the web — first-person, honest, screenshot/video-heavy, minimal marketing.

# What tunaFlow is (short)

A desktop app (Tauri 2 + React + Rust) that orchestrates CLI coding agents — Claude Code, Codex, Gemini CLI, OpenCode — into a single workflow. The user (a solo developer) leads; the agents execute. It handles branching conversations, roundtable debates between multiple agents, plan → dev → review pipelines, long-term memory, and context packing so each agent gets exactly what it needs without token waste.

Slogan: **"Of the agent, by the agent, for the agent."**

Positioning: **Human-led AI-executed**. Not "autonomous agents doing everything". The human decides direction, reviews, approves; agents execute well when given good context and process.

# Tone rules — strict

**DO**:
- First-person voice ("I built this…", "I got tired of…")
- Plain, understated language
- Show the actual thing (screenshots, embedded videos, real commands)
- Reference the developer's own dev log / posts
- Look like a dev tool — think `ghostty.org`, `bun.sh`, `zed.dev`, `tldraw.com`

**DO NOT**:
- Use marketing phrases: "Orchestrated X, Grounded in Y", "Human-led. Agent-amplified.", "Just add tokens."
- Use gradient/grid/mesh backgrounds or radial masks
- Stack big rounded cards with drop shadows
- Include "Watch Demo" / "Start Free Trial" / "Book a Demo" style CTAs
- Create fake dashboard widgets (e.g. `tokens: 4,102` status chips)
- Add a feature comparison table vs competitors
- Include architecture diagrams as primary content
- Use hero sections with 5+ paragraphs of aspirational copy

If in doubt: **fewer words, more screenshots/video**.

# Structure — 3 sections only

## Section 1: Hero
- Very short tagline (1 line, max 10 words) — NOT a marketing slogan. E.g. *"A desktop client for running multiple AI coding agents together."*
- One-sentence first-person explainer beneath. E.g. *"I built tunaFlow because I was tired of running Claude Code, Codex, and Gemini in separate terminals and pasting between them."*
- **Primary visual: embedded YouTube demo video** (see YouTube requirements below)
- CTAs: `Download (macOS)` link + `GitHub` link. No "Watch Demo" (the video is already right there). No marketing CTA.

## Section 2: What it does (screenshots/videos, annotated)
Present 3–4 concrete features as **captioned screenshots or short embedded clips**, not as feature cards with icons. Each gets:
- Short caption (plain sentence, no bold marketing)
- The real screenshot or a 15–30s YouTube clip

Feature coverage:
1. **Roundtable** — multiple agents debating in one conversation
2. **Plan → Dev → Review pipeline** — workflow with human approval gates
3. **Branching conversations** — fork a conversation, run an experiment, merge the summary back
4. **ContextPack & long-term memory** — project-aware context assembly

(If videos don't exist yet, use screenshot placeholders with clear `[video coming]` markers — don't fabricate.)

## Section 3: Install + Dev log
- Install block (code block, mono): homebrew / curl command. If not published yet, just a GitHub release link.
- **Latest 3 dev log posts** (fetched from the project's `docs/posts/` or RSS). Rendering: just titles + dates + short excerpt, linked. No "featured article" carousel.
- GitHub link + license note + "by @d9ng" footer. No social icons soup.

# YouTube embedding requirements

- Use `lite-youtube-embed` (or equivalent lazy-load library) — NOT raw `<iframe>` tags. Performance matters.
- Default state: thumbnail + play button only. Video loads on click.
- Aspect ratio: 16:9. Full width of content column (max ~960px).
- Optional short clips in Section 2: auto-play muted, loop, controls hidden. Use `?autoplay=1&mute=1&loop=1&playlist=<ID>`.
- Provide placeholders for real video IDs: `YT_HERO_DEMO_ID`, `YT_ROUNDTABLE_CLIP_ID`, etc. I'll fill these in.

# Visual rules

- **One** accent color. Pick something muted — slate/stone/zinc + one warm accent. No rainbow per-agent colors on landing (that's in-app UX, not marketing).
- **Dark mode first**. Light mode acceptable but secondary.
- **System sans + JetBrains Mono** (or equivalent monospace). No Google Fonts carnival.
- **Flat fills, thin borders (1px)**. No heavy shadows, no neumorphism.
- **Background**: solid or very subtle 1–2% noise texture. NO grid meshes, NO radial gradients, NO animated gradients.
- Maximum content width 960–1100px. Generous vertical whitespace.

# Tech stack (existing)

Keep the current stack: Next.js 16 + Tailwind + Radix UI + next-intl (already in `tunaFlow_home/`). Don't introduce new dependencies unless strictly needed for YouTube embedding.

i18n: keep 4 locales (ko/en/ja/zh) since the repo already has `messages/`. Start with ko + en fully translated; ja/zh can be stubbed.

# Deliverables

1. Rewritten `app/[locale]/page.tsx` (3 sections, thin composition of section components)
2. Three section components: `components/hero.tsx` (new), `components/features.tsx` (new), `components/install-and-log.tsx` (new)
3. Delete the existing `workflow-section`, `surfaces-section`, `comparison-section`, `architecture-section`, `cta-section` — these are the SaaS template remnants
4. YouTube embed component (`components/yt-embed.tsx`) — lazy-load thumbnail → iframe on click
5. Updated `messages/*.json` for the new copy (keep it minimal — most of the page is visuals)
6. Updated `app/[locale]/layout.tsx` meta tags: title, description, og image — use plain language, no marketing buzzwords

# Anti-examples — specific things to remove from the current tunaFlow_home

- `hero.tsx`: delete the radial gradient grid background, "Grounded in Real Project Context" headline, "Just add tokens." subline, fake "runtime: active / tokens: 4,102" chips, the large app mockup with gradient fade
- `workflow-section.tsx`, `surfaces-section.tsx`, `comparison-section.tsx`, `architecture-section.tsx`, `cta-section.tsx` — all deleted; their content is better expressed by videos in the new Section 2
- Any "Enterprise", "Team plans", "Book a demo" language

# Example of the first-person tone I want (reference, not literal)

> "tunaFlow is a desktop app I use daily to run Claude Code, Codex, and Gemini CLI on my own projects.
>
> I started it because running each CLI in its own terminal, copy-pasting output between them, and trying to keep track of which agent said what — that got old.
>
> Now they all talk in one window. Sometimes they even talk to each other."

Then the video plays.

That's the vibe.

---

## 끝 — 복사 범위 여기까지

---

## 사용 시 체크리스트

### 결과물 받으면 확인할 것
- [ ] 섹션이 **3개**인가? 4개 이상이면 템플릿이 또 나온 것
- [ ] Hero에 **YouTube embed**가 있는가 (스크린샷 단독 아님)
- [ ] "Watch Demo" / "Book a Demo" 버튼이 **없는가**
- [ ] Fake status chip (e.g. "tokens: 4,102")이 **없는가**
- [ ] 그리드/그라디언트 배경이 **없는가**
- [ ] 최소 한 군데 이상 first-person 문장이 있는가 ("I built…", "I was tired of…")
- [ ] 비교 테이블, 아키텍처 다이어그램이 **없는가**

### 이후 교체할 항목
- `YT_HERO_DEMO_ID` → 실제 데모 영상 ID
- `YT_ROUNDTABLE_CLIP_ID`, `YT_PLAN_REVIEW_CLIP_ID`, `YT_BRANCH_CLIP_ID` → 각 기능별 30s 클립 ID
- `docs/posts/` RSS/피드 연결 경로
- 다운로드 링크 (GitHub Releases URL)
- `@d9ng` → 실제 핸들

### 툴 별 주의
- **v0.dev**: 톤 지시어를 앞에 강조해도 "Watch Demo" 버튼을 다시 넣는 버릇이 있음. 받은 결과를 한 번 더 `"remove all SaaS-style CTAs and feature comparison tables"` 로 follow-up 해야 할 수 있음
- **bolt.new**: 구조는 잘 지키지만 카피를 또 마케팅식으로 씀. 카피는 수작업으로 조정 전제
- **Claude 직통**: 이 프롬프트 톤 제일 잘 이해함. 단 한 번에 3 섹션 전체 컴포넌트를 만들기보다는 섹션별로 나눠서 요청하는 게 안정적

---

## 연관 자산

- **시리즈 포스트 톤 참조**: `docs/posts/01-에이전트에게-프로세스를-줘라.md`, `docs/posts/side-01-gemini와-22분-혈투기.md` — first-person 개발기 톤의 실제 예시. 홈페이지 카피가 이 글들과 자연스럽게 이어져야 함
- **철학 문서**: `CLAUDE.md` §1 — "Of the agent, By the agent, For the agent" + Human-led 원칙
- **RT/워크플로우 스크린**: Section 2 비디오/스샷 소스로 사용할 실제 기능 = roundtable, plan-dev-review, branch drawer, context pack meta
