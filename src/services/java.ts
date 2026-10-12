import {
  Channel,
  invoke,
} from "@tauri-apps/api/core";

export interface JavaDiagnostic {
  line: number;
  column: number;
  severity:
    | "error"
    | "warning";
  message: string;
}

export interface JavaCompileResult {
  compiled: boolean;
  stdout: string;
  stderr: string;
  exitCode: number | null;
}

export interface JavaExitEvent {
  success: boolean;
  exitCode: number | null;
  timedOut: boolean;
}

export type JavaProcessEvent =
  | {
      event: "stdout";
      data: string;
    }
  | {
      event: "stderr";
      data: string;
    }
  | {
      event: "exit";
      data: JavaExitEvent;
    };

export async function diagnoseJava(
  className: string,
  classpath: string,
  sourceCode: string,
): Promise<JavaDiagnostic[]> {
  return invoke<JavaDiagnostic[]>(
    "diagnose_java",
    {
      className,
      classpath,
      sourceCode,
    },
  );
}

export async function runJavaClass(
  className: string,
  classpath: string,
  sourceCode: string,
  interactive: boolean,
  onEvent: (
    event: JavaProcessEvent,
  ) => void,
): Promise<JavaCompileResult> {
  const channel =
    new Channel<JavaProcessEvent>();

  channel.onmessage =
    onEvent;

  return invoke<JavaCompileResult>(
    "run_java_class",
    {
      className,
      classpath,
      sourceCode,
      interactive,
      onEvent: channel,
    },
  );
}

export async function sendJavaInput(
  input: string,
): Promise<void> {
  await invoke(
    "send_java_input",
    {
      input,
    },
  );
}