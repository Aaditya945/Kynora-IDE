import { create } from "zustand";

import {
  runJavaClass,
  sendJavaInput,
  type JavaCompileResult,
  type JavaExitEvent,
  type JavaProcessEvent,
} from "../services/java";

export type ExecutionState =
  | "idle"
  | "running"
  | "error";

interface EditorState {
  language: "java";

  content: string;

  executionState:
    | ExecutionState;

  interactive: boolean;

  output: string;
  errorOutput: string;

  tutorError: string;

  exitCode:
    | number
    | null;

  timedOut: boolean;

  setContent: (
    content: string,
  ) => void;

  appendOutput: (
    output: string,
  ) => void;

  appendError: (
    error: string,
  ) => void;

  clearTerminal: () => void;

  finishExecution: (
    result: JavaExitEvent,
  ) => void;

  runJavaClass: (
    className: string,
    classpath: string,
  ) => Promise<JavaCompileResult>;

  sendJavaInput: (
    input: string,
  ) => Promise<void>;
}

export const useEditorStore =
  create<EditorState>(
    (set, get) => ({
      language: "java",

      content: "",

      executionState:
        "idle",

      interactive: false,

      output: "",

      errorOutput: "",

      tutorError: "",

      exitCode: null,

      timedOut: false,

      setContent: (
        content,
      ) => {
        set({ content });
      },

      appendOutput: (
        output,
      ) => {
        set((state) => ({
          output:
            state.output +
            output,
        }));
      },

      appendError: (
        error,
      ) => {
        set((state) => ({
          errorOutput:
            state.errorOutput +
            error,

          tutorError:
            state.tutorError +
            error,
        }));
      },

      clearTerminal: () => {
        set({
          output: "",
          errorOutput: "",
          exitCode: null,
          timedOut: false,
        });
      },

      finishExecution: (
        result,
      ) => {
        set({
          executionState:
            result.success
              ? "idle"
              : "error",

          interactive: false,

          exitCode:
            result.exitCode,

          timedOut:
            result.timedOut,
        });
      },

      runJavaClass: async (
        className,
        classpath,
      ) => {
        const sourceCode =
          get().content;

        /*
         * Simple source-level detection
         * for programs that are expected
         * to read stdin.
         */
        const interactive =
          /\bScanner\b/.test(
            sourceCode,
          ) ||
          /\bSystem\.in\b/.test(
            sourceCode,
          ) ||
          /\bBufferedReader\b/.test(
            sourceCode,
          ) ||
          /\bInputStreamReader\b/.test(
            sourceCode,
          ) ||
          /\bConsole\b/.test(
            sourceCode,
          );

        set({
          executionState:
            "running",

          interactive,

          output: "",

          errorOutput: "",

          tutorError: "",

          exitCode: null,

          timedOut: false,
        });

        const handleEvent = (
          event: JavaProcessEvent,
        ) => {
          switch (event.event) {
            case "stdout":
              set((state) => ({
                output:
                  state.output +
                  event.data,
              }));

              break;

            case "stderr":
              set((state) => ({
                errorOutput:
                  state.errorOutput +
                  event.data,

                tutorError:
                  state.tutorError +
                  event.data,
              }));

              break;

            case "exit":
              set({
                executionState:
                  event.data.success
                    ? "idle"
                    : "error",

                interactive: false,

                exitCode:
                  event.data.exitCode,

                timedOut:
                  event.data.timedOut,
              });

              break;
          }
        };

        try {
          const result =
            await runJavaClass(
              className,
              classpath,
              sourceCode,
              interactive,
              handleEvent,
            );

          /*
           * javac failed.
           */
          if (!result.compiled) {
            set({
              executionState:
                "error",

              interactive: false,

              output:
                result.stdout,

              errorOutput:
                result.stderr,

              tutorError:
                result.stderr,

              exitCode:
                result.exitCode,

              timedOut: false,
            });

            return result;
          }

          if (
            result.stdout.trim()
          ) {
            set((state) => ({
              output:
                state.output +
                result.stdout,
            }));
          }

          return result;
        } catch (error) {
          const message =
            error instanceof Error
              ? error.message
              : String(error);

          set({
            executionState:
              "error",

            interactive: false,

            output: "",

            errorOutput:
              message,

            tutorError:
              message,

            exitCode: null,

            timedOut: false,
          });

          throw error;
        }
      },

      sendJavaInput:
        async (input) => {
          await sendJavaInput(
            input,
          );
        },
    }),
  );