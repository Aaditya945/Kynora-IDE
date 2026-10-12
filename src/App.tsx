import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import JavaEditor from "./components/JavaEditor";
import AITutorPanel from "./components/AITutorPanel";
import TerminalPanel from "./components/TerminalPanel";

import {
  useEditorStore,
} from "./stores/editorStore";

import {
  jdkStatus,
} from "./generated/jdk";

type Toast = {
  message: string;
  kind:
    | "info"
    | "success"
    | "error";
};

type ActiveTab =
  | "main"
  | "ai";

const DEFAULT_CLASS_NAME =
  "Main";

const DEFAULT_CLASSPATH =
  ".";

export default function App() {
  const executionState =
    useEditorStore(
      (state) =>
        state.executionState,
    );

  const interactive =
    useEditorStore(
      (state) =>
        state.interactive,
    );

  const output =
    useEditorStore(
      (state) =>
        state.output,
    );

  const errorOutput =
    useEditorStore(
      (state) =>
        state.errorOutput,
    );

  const tutorError =
    useEditorStore(
      (state) =>
        state.tutorError,
    );

  const runJavaClass =
    useEditorStore(
      (state) =>
        state.runJavaClass,
    );

  const sendJavaInput =
    useEditorStore(
      (state) =>
        state.sendJavaInput,
    );

  const clearTerminal =
    useEditorStore(
      (state) =>
        state.clearTerminal,
    );

  const [
    activeTab,
    setActiveTab,
  ] = useState<ActiveTab>(
    "main",
  );

  const [
    terminalOpen,
    setTerminalOpen,
  ] = useState(true);

  const [
    toast,
    setToast,
  ] = useState<Toast | null>(
    null,
  );

  const toastTimerRef =
    useRef<number | null>(
      null,
    );

  const showToast =
    useCallback(
      (
        nextToast: Toast,
      ) => {
        setToast(
          nextToast,
        );

        if (
          toastTimerRef.current !==
          null
        ) {
          window.clearTimeout(
            toastTimerRef.current,
          );
        }

        toastTimerRef.current =
          window.setTimeout(() => {
            setToast(null);

            toastTimerRef.current =
              null;
          }, 3000);
      },
      [],
    );

  useEffect(() => {
    return () => {
      if (
        toastTimerRef.current !==
        null
      ) {
        window.clearTimeout(
          toastTimerRef.current,
        );
      }
    };
  }, []);

  useEffect(() => {
    if (
      executionState ===
        "error" &&
      tutorError.trim()
    ) {
      setActiveTab("ai");
    }
  }, [
    executionState,
    tutorError,
  ]);

  const handleRun =
    async () => {
      if (
        executionState ===
        "running"
      ) {
        return;
      }

      if (!jdkStatus.found) {
        setTerminalOpen(
          true,
        );

        showToast({
          message:
            "JDK not found.",
          kind: "error",
        });

        return;
      }

      setTerminalOpen(
        true,
      );

      try {
        const result =
          await runJavaClass(
            DEFAULT_CLASS_NAME,
            DEFAULT_CLASSPATH,
          );

        if (
          !result.compiled
        ) {
          showToast({
            message:
              "Java compilation failed.",
            kind: "error",
          });

          return;
        }

        showToast({
          message:
            "Java program started.",
          kind: "info",
        });
      } catch (error) {
        const message =
          error instanceof Error
            ? error.message
            : String(error);

        showToast({
          message,
          kind: "error",
        });
      }
    };

  const handleInputSubmit =
    useCallback(
      async (input: string) => {
        if (
          executionState !==
          "running"
        ) {
          return;
        }

        try {
          await sendJavaInput(
            input,
          );
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : String(error);

          showToast({
            message,
            kind: "error",
          });

          throw error;
        }
      },
      [
        executionState,
        sendJavaInput,
        showToast,
      ],
    );

  return (
    <main className="app-shell">
      <header className="titlebar">
        <div className="titlebar-brand">
          <span
            className="language-dot"
            aria-hidden="true"
          />

          <span>
            Kynora
          </span>
        </div>

        <div className="toolbar">
          <span
            className={`execution-state ${executionState}`}
          >
            {executionState ===
            "running"
              ? interactive
                ? "Waiting for input"
                : "Running"
              : executionState ===
                  "error"
                ? "Error"
                : "Idle"}
          </span>

          <button
            className="run-button"
            type="button"
            onClick={
              handleRun
            }
            disabled={
              executionState ===
              "running"
            }
          >
            {executionState ===
            "running"
              ? "Running..."
              : "▶ Run"}
          </button>
        </div>
      </header>

      {!jdkStatus.found && (
        <div
          className="jdk-warning"
          role="alert"
        >
          <strong>
            JDK not found
          </strong>

          <span>
            Set JAVA_HOME or install a
            JDK before running Java code.
          </span>
        </div>
      )}

      <section className="workspace">
        <div className="tabbar">
          <button
            className={`workspace-tab ${
              activeTab === "main"
                ? "active"
                : ""
            }`}
            type="button"
            onClick={() =>
              setActiveTab(
                "main",
              )
            }
          >
            Main.java
          </button>

          <button
            className={`workspace-tab ${
              activeTab === "ai"
                ? "active"
                : ""
            }`}
            type="button"
            onClick={() =>
              setActiveTab(
                "ai",
              )
            }
          >
            <span className="ai-tab-icon">
              ✦
            </span>

            AI Tutor
          </button>

          <div className="tab-language">
            Java
          </div>
        </div>

        <div className="workspace-content">
          {activeTab ===
          "main" ? (
            <JavaEditor />
          ) : (
            <AITutorPanel
              errorMessage={
                tutorError
              }
            />
          )}
        </div>

        <TerminalPanel
          terminalOpen={
            terminalOpen
          }
          setTerminalOpen={
            setTerminalOpen
          }
          executionState={
            executionState
          }
          interactive={
            interactive
          }
          output={
            output
          }
          errorOutput={
            errorOutput
          }
          clearTerminal={
            clearTerminal
          }
          onInputSubmit={
            handleInputSubmit
          }
        />
      </section>

      <footer className="statusbar">
        <span>Java</span>
        <span>Spaces: 4</span>
        <span>UTF-8</span>
      </footer>

      {toast && (
        <div
          className={`toast toast-${toast.kind}`}
          role="status"
        >
          {toast.message}
        </div>
      )}
    </main>
  );
}