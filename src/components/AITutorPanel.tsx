import {
  type FormEvent,
  useEffect,
  useState,
} from "react";

import {
  askAiTutor,
  getOpenRouterApiKey,
  saveOpenRouterApiKey,
} from "../services/aiTutor";

interface AITutorPanelProps {
  errorMessage: string;
}

export default function AITutorPanel({
  errorMessage,
}: AITutorPanelProps) {
  const [
    apiKey,
    setApiKey,
  ] = useState("");

  const [
    response,
    setResponse,
  ] = useState("");

  const [
    loading,
    setLoading,
  ] = useState(false);

  const [
    requestError,
    setRequestError,
  ] = useState("");

  useEffect(() => {
    setApiKey(
      getOpenRouterApiKey(),
    );
  }, []);

  useEffect(() => {
    setResponse("");
    setRequestError("");
  }, [errorMessage]);

  const handleSaveKey =
    () => {
      saveOpenRouterApiKey(
        apiKey,
      );

      setRequestError("");

      if (apiKey.trim()) {
        setResponse(
          "OpenRouter API key saved locally.",
        );
      } else {
        setResponse("");
      }
    };

  const handleAskTutor =
    async (
      event: FormEvent<HTMLFormElement>,
    ) => {
      event.preventDefault();

      if (!errorMessage.trim()) {
        return;
      }

      setLoading(true);
      setResponse("");
      setRequestError("");

      try {
        const answer =
          await askAiTutor(
            errorMessage,
            apiKey,
          );

        setResponse(answer);
      } catch (error) {
        const message =
          error instanceof Error
            ? error.message
            : String(error);

        setRequestError(
          message,
        );
      } finally {
        setLoading(false);
      }
    };

  return (
    <section className="ai-tutor-page">
      <div className="ai-tutor-page-header">
        <div>
          <div className="ai-tutor-page-title">
            <span className="ai-tutor-icon">
              ✦
            </span>

            AI Tutor
          </div>

          <div className="ai-tutor-page-subtitle">
            Understand the error step by step.
          </div>
        </div>
      </div>

      {!errorMessage.trim() ? (
        <div className="ai-tutor-empty">
          <div className="ai-tutor-empty-icon">
            ✦
          </div>

          <div className="ai-tutor-empty-title">
            No error to explain
          </div>

          <div className="ai-tutor-empty-text">
            Run your Java program. When the
            compiler or runtime reports an
            error, its explanation will appear
            here.
          </div>
        </div>
      ) : (
        <div className="ai-tutor-content">
          <div className="ai-tutor-error-card">
            <div className="ai-tutor-section-label">
              Current Java error
            </div>

            <pre>
              {errorMessage}
            </pre>
          </div>

          <div className="ai-tutor-key-row">
            <input
              className="ai-tutor-key-input"
              type="password"
              value={apiKey}
              onChange={(event) =>
                setApiKey(
                  event.target.value,
                )
              }
              placeholder="OpenRouter API key"
              aria-label="OpenRouter API key"
            />

            <button
              className="ai-tutor-save-button"
              type="button"
              onClick={
                handleSaveKey
              }
            >
              Save Key
            </button>

            <form
              onSubmit={
                handleAskTutor
              }
            >
              <button
                className="ai-tutor-ask-button"
                type="submit"
                disabled={
                  loading ||
                  !apiKey.trim()
                }
              >
                {loading
                  ? "Thinking..."
                  : "Ask AI Tutor"}
              </button>
            </form>
          </div>

          {response && (
            <div className="ai-tutor-response">
              <div className="ai-tutor-section-label">
                Tutor
              </div>

              <div className="ai-tutor-response-text">
                {response}
              </div>
            </div>
          )}

          {requestError && (
            <div
              className="ai-tutor-request-error"
              role="alert"
            >
              {requestError}
            </div>
          )}

          <div className="ai-tutor-note">
            The tutor uses Socratic guidance:
            it should help you reason toward
            the fix instead of immediately
            dumping corrected code.
          </div>
        </div>
      )}
    </section>
  );
}