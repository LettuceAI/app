import type { CharacterSummary } from "@/api/generated/bindings";

export const faces = (members: readonly CharacterSummary[]) => members.map((member) => ({ name: member.name, src: member.avatar?.url ?? null }));
