import { Outlet } from "@tanstack/react-router";

export function AppShell() {
  return (
    <div className="flex h-full min-h-screen flex-col bg-surface text-fg">
      <Outlet />
    </div>
  );
}
