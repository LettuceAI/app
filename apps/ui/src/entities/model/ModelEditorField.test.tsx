import { useState } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, expect, it } from "vitest";
import { createTestI18n } from "@/shared/testing/i18n";
import { parseModelConfig, type ModelConfig } from "./model-config";
import { ModelEditorField } from "./ModelEditorField";
import type { EditorKind } from "./model-editor-sections";

afterEach(cleanup);
const cloud: EditorKind = "cloud";

function Harness() {
  const [config, setConfig] = useState<ModelConfig>(() => parseModelConfig({}));
  return (
    <ModelEditorField
      id="maxOutputTokens"
      kind={cloud}
      layer="chat"
      config={config}
      defaults={undefined}
      runtimeDefault={4096}
      onChange={setConfig}
      onFile={() => undefined}
      onInvalid={() => undefined}
    />
  );
}

it("keeps the same number input while the first digit makes the value its own", async () => {
  const i18n = await createTestI18n("en");
  await act(async () => {
    render(
      <I18nextProvider i18n={i18n}>
        <Harness />
      </I18nextProvider>,
    );
  });
  const input = screen.getByLabelText("Max output tokens");
  input.focus();
  fireEvent.change(input, { target: { value: "5" } });
  const after = screen.getByLabelText("Max output tokens");
  expect(after).toBe(input);
  expect(document.activeElement).toBe(input);
  expect((after as HTMLInputElement).value).toBe("5");
});
