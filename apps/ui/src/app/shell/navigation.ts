export const destinations = {
  chats: "/",
  groups: "/groups",
  discover: "/discover",
  library: "/library",
  settings: "/settings",
} as const;

export type Destination = keyof typeof destinations;

/** The destination a path belongs to: its own page or any page under it. */
export function destinationOf(pathname: string): Destination {
  return (
    (Object.keys(destinations) as Destination[]).find(
      (id) => id !== "chats" && (pathname === destinations[id] || pathname.startsWith(`${destinations[id]}/`)),
    ) ?? "chats"
  );
}
