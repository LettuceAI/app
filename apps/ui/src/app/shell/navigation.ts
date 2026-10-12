export const destinations = {
  chats: "/",
  groups: "/groups",
  discover: "/discover",
  library: "/library",
  settings: "/settings",
} as const;

export type Destination = keyof typeof destinations;

export function destinationOf(pathname: string): Destination {
  return (Object.keys(destinations) as Destination[]).find((id) => destinations[id] === pathname) ?? "chats";
}
