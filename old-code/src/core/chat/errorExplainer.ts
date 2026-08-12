export interface ExplainedChatError {
  status?: number;
  category:
    | "auth"
    | "billing"
    | "rate_limit"
    | "context_too_long"
    | "content_blocked"
    | "bad_request"
    | "not_found"
    | "provider_down"
    | "provider_overloaded"
    | "timeout"
    | "network"
    | "aborted"
    | "unknown";
  title: string;
  explanation: string;
  suggestion?: string;
  retryable: boolean;
  raw: string;
}

const STATUS_REGEXES = [
  /\b(?:HTTP|status(?:\s*code)?|Error|code)\s*[:=]?\s*(\d{3})\b/i,
  /\b(\d{3})\s+(?:Bad Request|Unauthorized|Forbidden|Not Found|Payment Required|Too Many Requests|Internal Server Error|Bad Gateway|Service Unavailable|Gateway Timeout|Request Timeout|Conflict|Payload Too Large|Unprocessable Entity|Unavailable For Legal Reasons)\b/i,
];

function extractStatus(raw: string): number | undefined {
  for (const regex of STATUS_REGEXES) {
    const match = raw.match(regex);
    if (match) {
      const code = parseInt(match[1], 10);
      if (code >= 100 && code <= 599) return code;
    }
  }
  return undefined;
}

function lower(raw: string): string {
  return raw.toLowerCase();
}

function classifyByText(raw: string): ExplainedChatError["category"] | null {
  const text = lower(raw);

  if (
    text.includes("aborted by user") ||
    text.includes("cancelled") ||
    text.includes("canceled") ||
    text.includes("request aborted")
  ) {
    return "aborted";
  }

  if (text.includes("content_blocked") || text.includes("safety") || text.includes("blocked by content filter")) {
    return "content_blocked";
  }

  if (
    text.includes("failed to fetch") ||
    text.includes("network error") ||
    text.includes("connect error") ||
    text.includes("dns error") ||
    text.includes("connection refused") ||
    text.includes("connection reset") ||
    text.includes("no internet")
  ) {
    return "network";
  }

  if (text.includes("timed out") || text.includes("timeout")) {
    return "timeout";
  }

  if (text.includes("context length") || text.includes("maximum context") || text.includes("too long")) {
    return "context_too_long";
  }

  if (text.includes("api key") || text.includes("invalid key") || text.includes("authentication")) {
    return "auth";
  }

  if (text.includes("insufficient") && (text.includes("credit") || text.includes("balance") || text.includes("quota"))) {
    return "billing";
  }

  if (text.includes("rate limit") || text.includes("too many requests")) {
    return "rate_limit";
  }

  return null;
}

interface Template {
  title: string;
  explanation: string;
  suggestion: string;
  retryable: boolean;
}

const STATUS_TEMPLATES: Record<number, Template> = {
  400: {
    title: "The provider rejected the request",
    explanation:
      "The provider says the request was malformed. Often this means the model doesn't support something in the prompt (an image, a tool, or an unusual setting) or the chat history contains content the provider won't process.",
    suggestion: "Try swapping to a different model, removing the last attachment, or starting a fresh chat.",
    retryable: false,
  },
  401: {
    title: "API key was rejected",
    explanation:
      "The provider didn't accept the API key for this request. It may be missing, mistyped, expired, or revoked.",
    suggestion: "Open the Providers page in Settings, check the key for the active provider, and try saving it again.",
    retryable: false,
  },
  402: {
    title: "Out of credits",
    explanation:
      "The provider says your account doesn't have enough credit or balance to complete the request.",
    suggestion: "Add credits to your provider account, or switch to another provider/model in chat settings.",
    retryable: false,
  },
  403: {
    title: "Access denied by the provider",
    explanation:
      "The provider refused this request. Common reasons: the key doesn't have access to the chosen model, your region is restricted, or the account is suspended.",
    suggestion: "Try a different model, check your provider dashboard, or verify your account is in good standing.",
    retryable: false,
  },
  404: {
    title: "Model not found",
    explanation:
      "The provider couldn't find the model this character is set to use. The model ID may have been renamed, removed, or never existed on this provider.",
    suggestion: "Open the character settings and pick a different model, or refresh the model list in Settings under Models.",
    retryable: false,
  },
  408: {
    title: "Request timed out",
    explanation: "The provider took too long to start responding and gave up.",
    suggestion: "Try again. If it keeps happening, switch to a faster model or check your connection.",
    retryable: true,
  },
  409: {
    title: "Request conflict",
    explanation: "The provider refused because the request conflicts with another in-flight request or current state.",
    suggestion: "Wait a moment and try again.",
    retryable: true,
  },
  413: {
    title: "Chat is too long for this model",
    explanation:
      "You're sending more content than the model's context window can hold. This happens with long chats, big attachments, or models with small context windows.",
    suggestion: "Switch to a model with a larger context, summarise/clear older messages, or remove large attachments.",
    retryable: false,
  },
  422: {
    title: "Provider couldn't process the prompt",
    explanation:
      "The provider understood the request but refused to process it. Usually a formatting or validation issue on the provider's side.",
    suggestion: "Try a different model or remove unusual prompt customisations from the character.",
    retryable: false,
  },
  429: {
    title: "Rate limit hit",
    explanation:
      "The provider rejected this request with a rate limit. That can mean you've sent too many requests too quickly, you've hit your daily/minute quota (free tiers are stricter), or the provider's servers are overloaded right now and throttling everyone.",
    suggestion: "Wait a minute and try again. If it keeps happening, switch to another provider key, swap models, or upgrade your plan.",
    retryable: true,
  },
  451: {
    title: "Blocked for legal or policy reasons",
    explanation: "The provider refused this request because of its content policies or local legal restrictions.",
    suggestion: "Try a different model, or rephrase the message and any character description that may have triggered the filter.",
    retryable: false,
  },
  500: {
    title: "The provider hit an internal error",
    explanation:
      "Something broke on the provider's servers. This isn't caused by anything you did and is usually temporary.",
    suggestion: "Try again in a few seconds. If it keeps failing, switch to another provider or check their status page.",
    retryable: true,
  },
  502: {
    title: "Provider gateway error",
    explanation:
      "The provider's edge servers couldn't reach the model backend. Usually transient.",
    suggestion: "Try again. If it persists, switch model or provider.",
    retryable: true,
  },
  503: {
    title: "Provider is unavailable",
    explanation:
      "The provider is down for maintenance or temporarily refusing new requests.",
    suggestion: "Wait a moment and retry. Check the provider's status page if it lasts.",
    retryable: true,
  },
  504: {
    title: "Provider response timeout",
    explanation: "The model started responding but didn't finish in time, or the provider's gateway timed out.",
    suggestion: "Try again. Lowering the max output length or picking a faster model can help.",
    retryable: true,
  },
  529: {
    title: "Provider is overloaded",
    explanation: "The provider's servers are slammed right now and asked you to back off.",
    suggestion: "Wait 10–30 seconds and try again, or switch to another provider.",
    retryable: true,
  },
};

const CATEGORY_TEMPLATES: Record<ExplainedChatError["category"], Template> = {
  auth: STATUS_TEMPLATES[401],
  billing: STATUS_TEMPLATES[402],
  rate_limit: STATUS_TEMPLATES[429],
  context_too_long: STATUS_TEMPLATES[413],
  content_blocked: {
    title: "Blocked by the provider's safety filter",
    explanation:
      "The provider refused to generate a reply because the prompt or chat history tripped its content policy. This is decided by the provider, not by LettuceAI.",
    suggestion: "Try rephrasing the last message, pick a more permissive model, or switch providers.",
    retryable: false,
  },
  bad_request: STATUS_TEMPLATES[400],
  not_found: STATUS_TEMPLATES[404],
  provider_down: STATUS_TEMPLATES[503],
  provider_overloaded: STATUS_TEMPLATES[529],
  timeout: STATUS_TEMPLATES[504],
  network: {
    title: "Couldn't reach the provider",
    explanation:
      "The request never made it to the provider. This usually means no internet, a firewall blocking the connection, or DNS trouble.",
    suggestion: "Check your connection or VPN, then try again.",
    retryable: true,
  },
  aborted: {
    title: "Request cancelled",
    explanation: "You cancelled the request before the provider finished replying.",
    suggestion: "Send the message again if you want a reply.",
    retryable: true,
  },
  unknown: {
    title: "Something went wrong",
    explanation: "The provider returned an error LettuceAI doesn't have a specific explanation for.",
    suggestion: "Check the raw error below for clues, then try again or switch model.",
    retryable: true,
  },
};

function categoryForStatus(status: number): ExplainedChatError["category"] {
  if (status === 401 || status === 407) return "auth";
  if (status === 402) return "billing";
  if (status === 403) return "auth";
  if (status === 404) return "not_found";
  if (status === 408 || status === 504) return "timeout";
  if (status === 413) return "context_too_long";
  if (status === 429) return "rate_limit";
  if (status === 451) return "content_blocked";
  if (status === 503) return "provider_down";
  if (status === 529) return "provider_overloaded";
  if (status >= 500) return "provider_down";
  if (status >= 400) return "bad_request";
  return "unknown";
}

export function explainChatError(raw: string | null | undefined): ExplainedChatError | null {
  if (!raw) return null;
  const trimmed = raw.trim();
  if (!trimmed) return null;

  const status = extractStatus(trimmed);
  const textCategory = classifyByText(trimmed);

  if (textCategory === "aborted") {
    return {
      ...CATEGORY_TEMPLATES.aborted,
      category: "aborted",
      raw: trimmed,
    };
  }

  let template: Template | undefined;
  let category: ExplainedChatError["category"] | undefined;

  if (status && STATUS_TEMPLATES[status]) {
    template = STATUS_TEMPLATES[status];
    category = categoryForStatus(status);
  } else if (textCategory) {
    template = CATEGORY_TEMPLATES[textCategory];
    category = textCategory;
  } else if (status) {
    category = categoryForStatus(status);
    template = CATEGORY_TEMPLATES[category];
  } else {
    template = CATEGORY_TEMPLATES.unknown;
    category = "unknown";
  }

  return {
    status,
    category,
    title: template.title,
    explanation: template.explanation,
    suggestion: template.suggestion,
    retryable: template.retryable,
    raw: trimmed,
  };
}
