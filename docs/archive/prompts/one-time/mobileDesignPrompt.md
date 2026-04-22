# tunaFlow Mobile — UI Design Prompt

> For: v0 (Vercel) or Stitch
> Target: Mobile-first responsive web app (PWA)
> Stack: React + TypeScript + Tailwind CSS
> Design reference: Linear mobile, Raycast mobile, ChatGPT mobile

---

## Product Context

tunaFlow is a multi-agent orchestration client. Users talk to AI agents (Claude, Codex, Gemini) to plan, implement, and review code. The mobile version is a **companion app** — users monitor agent activity, read conversations, and send quick messages while away from the desktop.

---

## Layout Structure

**The entire screen is the chat.** No bottom tab bar. No tab switching. Chat is the primary and only full-screen view.

### Three layers:

```
┌─────────────────────────┐
│ [☰]  Project Name    ⚡ │  ← Minimal top bar (floating, semi-transparent)
│                         │
│                         │
│   Chat messages         │  ← Full-screen scrollable chat
│   (assistant bubbles,   │
│    user bubbles,        │
│    streaming indicator) │
│                         │
│                         │
│                         │
│ ┌─────────────────────┐ │
│ │ Message input bar    │ │  ← Fixed bottom input (always visible)
│ └─────────────────────┘ │
└─────────────────────────┘

← Swipe right from left edge: Menu drawer
  Swipe left from right edge: Status panel →
```

### 1. Chat (Main — always visible)

Full-screen chat view. This IS the app.

- **Message bubbles**: Assistant messages are left-aligned (full width, no bubble background), user messages are right-aligned (colored bubble, rounded).
- **Streaming indicator**: When agent is running, show a pulsing dot + "Agent is thinking..." at the bottom of the message list, above the input bar.
- **Markdown rendering**: Assistant messages render markdown (code blocks with syntax highlighting, tables, headings, lists). Use `react-markdown` + `remark-gfm`.
- **Long-press message**: Show context menu (Copy, Branch from here).
- **Auto-scroll**: Stick to bottom during streaming, release when user scrolls up.

**Input bar (fixed bottom)**:
- Text input with auto-growing height (1-4 lines)
- Send button (right side, appears when text is entered)
- Engine selector pill (left side, compact: "Claude ▾" — tap to switch engine)
- When agent is running: input becomes disabled, send button becomes stop button (■)

### 2. Menu (Left drawer — swipe from left edge or tap ☰)

Slides in from left, 80% screen width, with backdrop overlay.

Contents (top to bottom):
- **Project name + path** (header)
- **Conversations list**: Recent conversations, grouped. Active one highlighted. Tap to switch.
- **Branches section**: Active branches with status badges (active/adopted). Tap to open branch conversation.
- **Roundtables section**: Active RTs with participant count. Tap to open.
- **Settings gear icon** (bottom)

Design:
- Dark surface (slightly elevated from main background)
- Compact list items (44px height, 14px font)
- Status badges: colored dots (green=active, blue=adopted, orange=running)
- Swipe back or tap backdrop to close

### 3. Status Panel (Right drawer — swipe from right edge)

Slides in from right, 80% screen width. This is the "what's happening" view.

Contents (top to bottom):
- **Agent status card**: Currently running agent (engine, model, elapsed time, token count). Pulsing green dot when active.
- **Active Plan card**: If a plan exists — title, phase (draft/approved/implementing/review), progress bar (subtasks done/total). Tap to expand subtask list.
- **Recent branches**: Last 3 branches with last message preview. Compact cards.
- **Quick actions**: "New Branch", "New Roundtable" buttons.

Design:
- Cards with subtle borders (not heavy shadows)
- Progress indicators use the app's accent color
- Real-time updates (agent status should animate/pulse)
- Swipe back or tap backdrop to close

---

## Top Bar

Minimal, floating over the chat content. Semi-transparent background with blur.

```
[☰]  Project Name                    ⚡
```

- **☰** (left): Opens menu drawer
- **Project Name** (center): Current project. Tap for project switcher dropdown.
- **⚡** (right): Status indicator. Green pulse when agent is running. Tap opens status panel (same as right swipe).

When scrolling down, top bar fades out. When scrolling up or at top, it fades in. The chat content extends behind it.

---

## Color System

Dark mode primary. Light mode secondary.

**Dark mode:**
- Background: `#0a0a0c` (near black)
- Surface: `#141418` (cards, drawers)
- Surface elevated: `#1c1c22` (input bar, modals)
- Text primary: `#e8e8ec`
- Text secondary: `#8888a0`
- Accent: `oklch(0.65 0.18 270)` (blue-purple, same as desktop)
- User bubble: `oklch(0.25 0.06 270)` (subtle accent tint)
- Success: `oklch(0.65 0.18 155)`
- Warning: `oklch(0.70 0.18 80)`
- Error: `oklch(0.55 0.22 25)`

**Light mode:**
- Background: `#ffffff`
- Surface: `#f5f5f7`
- Text primary: `#1d1d1f`
- Accent: `oklch(0.55 0.20 270)`

---

## Typography

- **Font**: Pretendard Variable (Korean + Latin), Inter fallback
- **Base size**: 15px (mobile optimal)
- **Message text**: 15px / 1.6 line-height
- **Code blocks**: JetBrains Mono, 13px
- **UI labels**: 13px, medium weight
- **Timestamps**: 11px, secondary color

---

## Interactions & Gestures

| Gesture | Action |
|---------|--------|
| Swipe right from left edge | Open menu drawer |
| Swipe left from right edge | Open status panel |
| Swipe drawer back | Close drawer |
| Tap backdrop | Close drawer |
| Long-press message | Context menu |
| Pull down at top | Refresh / load older messages |
| Tap ☰ | Open menu drawer |
| Tap ⚡ | Open status panel |
| Tap engine pill | Engine selector (bottom sheet) |

---

## Key Screens to Design

1. **Chat (main)** — Full-screen chat with streaming message, input bar, floating top bar
2. **Chat with menu open** — Left drawer overlaying chat
3. **Chat with status open** — Right drawer overlaying chat
4. **Engine selector** — Bottom sheet with engine options (Claude, Codex, Gemini, Ollama)
5. **Empty state** — No conversation selected, onboarding prompt
6. **Streaming state** — Agent actively responding, pulsing indicators

---

## Constraints

- Mobile viewport: 375-428px width (iPhone standard)
- No bottom navigation bar — maximize chat area
- Drawers should feel native (smooth spring animation, velocity-based)
- Must work as PWA (no native APIs assumed)
- Input bar must stay above iOS keyboard when focused
- Safe area insets for notch/dynamic island devices

---

## What NOT to include

- Desktop layout / responsive breakpoints — this is mobile-only
- Complex settings screens — settings are minimal (server URL, auth token)
- File upload UI — not supported in mobile version
- Branch/RT creation wizard — simplified to quick actions in status panel
- Workflow management (Plan/Dev/Review) — view only, not control
