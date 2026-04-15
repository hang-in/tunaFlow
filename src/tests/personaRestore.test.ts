import { describe, it, expect, beforeEach } from "vitest";
import { useChatStore } from "@/stores/chatStore";
import { restorePersonaForConversation } from "@/lib/personaRestore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";
import type { AgentProfile } from "@/types";

const architectProfile: AgentProfile = {
  id: "architect-claude",
  label: "Architect Claude",
  engine: "claude",
  model: "claude-opus-4-6",
  defaultSkills: [],
  personaId: "persona_architect",
};

const reviewerProfile: AgentProfile = {
  id: "reviewer-codex",
  label: "Reviewer Codex",
  engine: "codex",
  defaultSkills: [],
  personaId: "persona_reviewer",
};

describe("restorePersonaForConversation (Reviewer→Architect stale fix)", () => {
  beforeEach(() => {
    useChatStore.setState({
      agentProfiles: [architectProfile, reviewerProfile],
      _convEngineMap: {
        "arch-conv": { profileId: architectProfile.id, engine: "claude", model: "claude-opus-4-6" },
        "rev-conv":  { profileId: reviewerProfile.id,  engine: "codex" },
        "no-profile-conv": { profileId: null, engine: "claude" },
      },
      personaFragment: null,
      personaLabel: null,
    });
  });

  it("restores Architect persona when global store is stale as Reviewer", () => {
    // Simulate the stale state left by a just-closed Review RT drawer:
    const reviewerPersona = DEFAULT_PERSONAS.find((p) => p.id === "persona_reviewer")!;
    useChatStore.setState({
      personaFragment: reviewerPersona.promptFragment,
      personaLabel: `Reviewer Codex · ${reviewerPersona.name}`,
    });

    restorePersonaForConversation("arch-conv");

    const { personaLabel, personaFragment } = useChatStore.getState();
    // Architect persona may or may not exist in DEFAULT_PERSONAS; either way
    // the label MUST NOT contain "Reviewer".
    expect(personaLabel).not.toMatch(/Reviewer/);
    // And the prompt fragment must be different from the reviewer fragment.
    expect(personaFragment).not.toBe(reviewerPersona.promptFragment);
  });

  it("clears persona when the target conversation has no profile bound", () => {
    useChatStore.setState({
      personaFragment: "reviewer fragment",
      personaLabel: "Reviewer Codex · Reviewer",
    });

    restorePersonaForConversation("no-profile-conv");

    const { personaLabel, personaFragment } = useChatStore.getState();
    expect(personaLabel).toBeNull();
    expect(personaFragment).toBeNull();
  });

  it("applies reviewer persona when the target IS the reviewer conversation", () => {
    // Sanity check: function works in the other direction too.
    useChatStore.setState({ personaFragment: null, personaLabel: null });

    restorePersonaForConversation("rev-conv");

    const { personaLabel } = useChatStore.getState();
    expect(personaLabel).toMatch(/Reviewer/);
  });

  it("is idempotent — repeated calls yield the same persona state", () => {
    restorePersonaForConversation("arch-conv");
    const first = {
      label: useChatStore.getState().personaLabel,
      frag: useChatStore.getState().personaFragment,
    };
    restorePersonaForConversation("arch-conv");
    const second = {
      label: useChatStore.getState().personaLabel,
      frag: useChatStore.getState().personaFragment,
    };
    expect(second).toEqual(first);
  });
});
