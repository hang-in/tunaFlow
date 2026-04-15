import { useChatStore } from "@/stores/chatStore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";

/**
 * Restore the given conversation's saved profile-based persona into the global
 * store (personaFragment / personaLabel).
 *
 * Why: sendWithEngine reads persona from the global store at call time. Paths
 * that trigger auto-send after a Review RT (e.g. "tunaflow:plan-completed"
 * auto-notify to Architect) fire while the global persona is still "Reviewer"
 * from the just-closed review drawer. Callers MUST invoke this before
 * sendWithEngine so the architect doesn't receive a Reviewer persona.
 */
export function restorePersonaForConversation(conversationId: string): void {
  const s = useChatStore.getState();
  const saved = s.getConversationEngine(conversationId);
  const profileId = saved?.profileId ?? null;
  if (!profileId) {
    useChatStore.setState({ personaFragment: null, personaLabel: null });
    return;
  }
  const profile = s.agentProfiles.find((p) => p.id === profileId);
  if (!profile) {
    useChatStore.setState({ personaFragment: null, personaLabel: null });
    return;
  }
  const persona = profile.personaId
    ? DEFAULT_PERSONAS.find((p) => p.id === profile.personaId)
    : null;
  useChatStore.setState({
    personaFragment: persona?.promptFragment ?? null,
    personaLabel: persona
      ? (profile.label === persona.name ? profile.label : `${profile.label} · ${persona.name}`)
      : profile.label,
  });
}
