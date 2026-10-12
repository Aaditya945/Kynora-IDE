const OPENROUTER_ENDPOINT =
  "https://openrouter.ai/api/v1/chat/completions";

const DEFAULT_MODEL = "openrouter/free";

const API_KEY_STORAGE_KEY =
  "openrouter_api_key";

const SOCRATIC_SYSTEM_PROMPT = `
You are an AI programming tutor inside a Java IDE.

Your job is to help a student understand Java errors using the Socratic method.

Rules:
1. Do not immediately give the complete corrected code.
2. First explain what kind of problem the error indicates.
3. Ask one useful guiding question that helps the student reason about the error.
4. Give a small hint after the question when useful.
5. Explain Java concepts in simple language.
6. Focus specifically on the provided compiler or runtime error.
7. Do not invent information that is not present in the error.
8. Keep the response concise enough to be useful inside an IDE.
9. Only provide a complete corrected solution when the student explicitly asks for one.
`.trim();

interface OpenRouterResponse {
  choices?: Array<{
    message?: {
      content?: string | Array<{
        type?: string;
        text?: string;
      }>;
    };
  }>;

  error?: {
    message?: string;
  };

  model?: string;
}

function getConfiguredApiKey(): string {
  const buildTimeKey =
    import.meta.env.VITE_OPENROUTER_API_KEY;

  if (
    typeof buildTimeKey === "string" &&
    buildTimeKey.trim()
  ) {
    return buildTimeKey.trim();
  }

  const storedKey =
    window.localStorage.getItem(
      API_KEY_STORAGE_KEY,
    );

  return storedKey?.trim() ?? "";
}

export function getOpenRouterApiKey(): string {
  return getConfiguredApiKey();
}

export function saveOpenRouterApiKey(
  apiKey: string,
): void {
  const cleanKey = apiKey.trim();

  if (!cleanKey) {
    window.localStorage.removeItem(
      API_KEY_STORAGE_KEY,
    );

    return;
  }

  window.localStorage.setItem(
    API_KEY_STORAGE_KEY,
    cleanKey,
  );
}

function parseResponseContent(
  data: OpenRouterResponse,
): string {
  const content =
    data.choices?.[0]?.message?.content;

  if (typeof content === "string") {
    return content.trim();
  }

  if (Array.isArray(content)) {
    return content
      .map((part) => part.text ?? "")
      .join("")
      .trim();
  }

  return "";
}

export async function askAiTutor(
  errorMessage: string,
  apiKey?: string,
): Promise<string> {
  const cleanError =
    errorMessage.trim();

  if (!cleanError) {
    throw new Error(
      "There is no Java error to explain.",
    );
  }

  const key =
    apiKey?.trim() ||
    getConfiguredApiKey();

  if (!key) {
    throw new Error(
      "OpenRouter API key is not configured.",
    );
  }

  const response = await fetch(
    OPENROUTER_ENDPOINT,
    {
      method: "POST",

      headers: {
        Authorization: `Bearer ${key}`,
        "Content-Type": "application/json",
      },

      body: JSON.stringify({
        model: DEFAULT_MODEL,

        messages: [
          {
            role: "system",
            content: SOCRATIC_SYSTEM_PROMPT,
          },

          {
            role: "user",
            content:
              `A Java program produced this error:\n\n${cleanError}`,
          },
        ],
      }),
    },
  );

  let data: OpenRouterResponse = {};

  try {
    data =
      (await response.json()) as OpenRouterResponse;
  } catch {
    throw new Error(
      `OpenRouter returned an invalid response (${response.status}).`,
    );
  }

  if (!response.ok) {
    throw new Error(
      data.error?.message ||
        `OpenRouter request failed with HTTP ${response.status}.`,
    );
  }

  const answer =
    parseResponseContent(data);

  if (!answer) {
    throw new Error(
      "OpenRouter returned an empty tutor response.",
    );
  }

  return answer;
}