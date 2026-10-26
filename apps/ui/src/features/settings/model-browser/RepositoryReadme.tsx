import type { MouseEvent } from "react";
import { Prose } from "@lettuceai/crisp";
import { openExternalUrl } from "@/api/external-links";
import { FailureState } from "@/shared/ui/FailureState";
import { repositoryUrl } from "./data";
import { useSettingsAction } from "../data";

export function RepositoryReadme({
  repo,
  repositoryBase,
  revision = "main",
  markdown,
}: {
  repo: string;
  repositoryBase: string;
  revision?: string | undefined;
  markdown: string;
}) {
  const action = useSettingsAction();
  const open = (event: MouseEvent<HTMLDivElement>) => {
    const target = event.target;
    const anchor = target instanceof Element ? target.closest("a[href]") : null;
    const href = anchor?.getAttribute("href");
    if (!href) return;
    event.preventDefault();
    action.mutate(() =>
      openExternalUrl(
        new URL(href, `${repositoryUrl(repositoryBase, repo)}/blob/${revision}/README.md`).href,
      ),
    );
  };
  return (
    <div onClick={open} onAuxClick={open}>
      <Prose>{markdown}</Prose>
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
