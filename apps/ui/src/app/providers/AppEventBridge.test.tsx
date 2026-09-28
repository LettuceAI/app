import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { createApiClient } from "@/api/client";
import type { AppEvent, JobView } from "@/api/generated/bindings";
import { createMockTransport } from "@/api/mock-transport";
import { queryKeys } from "@/api/query-keys";
import { AppEventBridge } from "./AppEventBridge";
import { createQueryClient } from "./query-client";

afterEach(() => {
  cleanup();
});

const job: JobView = {
  id: "j1",
  kind: "maintenance",
  subject: { kind: "maintenance", id: "m1" },
  subject_detail: null,
  state: "succeeded",
  progress: { current: 1, total: 1, unit: null, label_code: null, bytes_per_second: null },
  created_at: 0,
  updated_at: 0,
  failure: null,
  result: null,
};

async function setup() {
  const transport = createMockTransport();
  const queryClient = createQueryClient();
  const keys = [
    queryKeys.jobs.detail("j1"),
    queryKeys.jobs.detail("j2"),
    queryKeys.jobs.lists(),
    queryKeys.conversations.detail("c1"),
    queryKeys.conversations.messages("c1"),
    queryKeys.conversations.detail("c2"),
    queryKeys.app.status(),
  ];
  const seed = () => {
    for (const key of keys) queryClient.setQueryData(key, { seeded: true });
  };
  seed();
  const invalidated = () =>
    keys.filter((key) => queryClient.getQueryState(key)?.isInvalidated).map((key) => key.join("/"));
  const view = await act(async () =>
    render(<AppEventBridge api={createApiClient(transport)} queryClient={queryClient} />),
  );
  const afterSubscribe = invalidated();
  seed();
  return { transport, invalidated, afterSubscribe, view };
}

describe("AppEventBridge", () => {
  it("invalidates every query once the subscription is live, covering events it missed", async () => {
    const { afterSubscribe } = await setup();
    expect(afterSubscribe).toEqual([
      "jobs/detail/j1",
      "jobs/detail/j2",
      "jobs/list",
      "conversations/detail/c1",
      "conversations/detail/c1/messages",
      "conversations/detail/c2",
      "app/status",
    ]);
  });

  it("invalidates the queries an app event maps to and nothing else", async () => {
    const { transport, invalidated } = await setup();
    await act(async () => transport.emitAppEvent({ type: "job_updated", job }));
    expect(invalidated()).toEqual(["jobs/detail/j1", "jobs/list"]);
  });

  it("refreshes a conversation and its messages when a generation settles", async () => {
    const { transport, invalidated } = await setup();
    await act(async () => transport.emitAppEvent({ type: "generation_settled", conversation_id: "c1", turn_id: "t1" }));
    expect(invalidated()).toEqual(["conversations/detail/c1", "conversations/detail/c1/messages"]);
  });

  it("ignores event types the table does not know", async () => {
    const { transport, invalidated } = await setup();
    await act(async () => transport.emitAppEvent({ type: "future_event" } as unknown as AppEvent));
    expect(invalidated()).toEqual([]);
  });

  it("stops listening when unmounted", async () => {
    const { transport, invalidated, view } = await setup();
    view.unmount();
    await act(async () => transport.emitAppEvent({ type: "generation_settled", conversation_id: "c1", turn_id: "t1" }));
    expect(invalidated()).toEqual([]);
  });
});
