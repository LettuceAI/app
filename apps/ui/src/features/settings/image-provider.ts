export function imageProviderDraft(config?: Record<string, unknown>) {
  return {
    txt2img: typeof config?.txt2img_workflow === "string" ? config.txt2img_workflow : "",
    img2img: typeof config?.img2img_workflow === "string" ? config.img2img_workflow : "",
  };
}

/** Numeric tokens can be unquoted in legacy templates. Ignore tokens inside strings. */
export function comfyWorkflowError(template: string, required: boolean): "required" | "invalid" | null {
  if (!template.trim()) return required ? "required" : null;
  if (new TextEncoder().encode(template).length > 4 * 1024 * 1024) return "invalid";
  try {
    const normalized = template.replace(/"(?:[^"\\]|\\.)*"|%(?:WIDTH|HEIGHT|STEPS|CFG|SEED|DENOISE|IMAGE_COUNT|BATCH_SIZE)%/g, (token) => token.startsWith('"') ? token : "0");
    const graph: unknown = JSON.parse(normalized);
    if (!graph || typeof graph !== "object" || Array.isArray(graph)) return "invalid";
    const nodes = Object.values(graph);
    return nodes.length && nodes.every((node: unknown) => node !== null && typeof node === "object" && !Array.isArray(node) && "class_type" in node && typeof node.class_type === "string" && node.class_type.trim() && "inputs" in node && node.inputs !== null && typeof node.inputs === "object" && !Array.isArray(node.inputs)) ? null : "invalid";
  } catch { return "invalid"; }
}
