import { useMemo, type MouseEvent } from "react";
import { openExternalUrl } from "@/api/external-links";
import { FailureState } from "@/shared/ui/FailureState";
import { repositoryUrl } from "./data";
import { useSettingsAction } from "../data";
import { readmeHtml } from "./readme-html";

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
  const base = repositoryUrl(repositoryBase, repo);
  const html = useMemo(
    () =>
      readmeHtml(
        markdown,
        `${base}/blob/${revision}/README.md`,
        `${base}/resolve/${revision}/README.md`,
      ),
    [markdown, base, revision],
  );
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
      <div
        className="min-w-0 overflow-x-auto text-base leading-relaxed text-fg [&_p]:my-2 [&_h1]:mb-2 [&_h1]:mt-5 [&_h1]:text-xl [&_h1]:font-bold [&_h2]:mb-2 [&_h2]:mt-5 [&_h2]:text-lg [&_h2]:font-semibold [&_h3]:my-2 [&_h3]:font-semibold [&_a]:text-accent [&_a]:underline [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_blockquote]:my-3 [&_blockquote]:border-l-2 [&_blockquote]:border-line [&_blockquote]:pl-3 [&_code]:font-mono [&_pre]:my-3 [&_pre]:overflow-x-auto [&_pre]:rounded-lg [&_pre]:bg-fill [&_pre]:p-3 [&_img]:my-3 [&_img]:inline-block [&_img]:h-auto [&_img]:max-w-full [&_table]:my-3 [&_table]:border-collapse [&_th]:border [&_th]:border-line [&_th]:px-3 [&_th]:py-2 [&_td]:border [&_td]:border-line [&_td]:px-3 [&_td]:py-2 [&_hr]:my-4 [&_hr]:border-line"
        dangerouslySetInnerHTML={{ __html: html }}
      />
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}
