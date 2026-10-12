import {
  memo,
  useState,
  type FormEvent,
  type SyntheticEvent,
} from "react";

type ExecutionState =
  | "idle"
  | "running"
  | "error";

type TerminalPanelProps = {
  terminalOpen: boolean;
  setTerminalOpen: React.Dispatch<
    React.SetStateAction<boolean>
  >;

  executionState: ExecutionState;
  interactive: boolean;

  output: string;
  errorOutput: string;

  clearTerminal: () => void;

  onInputSubmit: (
    input: string,
  ) => Promise<void>;
};

function friendlyJavaError(
  rawError: string,
): string {
  const raw =
    rawError.trim();

  if (!raw) {
    return "";
  }

  if (
    raw ===
    "Java source code is empty."
  ) {
    return (
      "No Java code was found.\n\n" +
      "The editor is empty, so there is " +
      "nothing for javac to compile."
    );
  }

  const compilerErrors =
    Array.from(
      raw.matchAll(
        /(?:^|\r?\n).*?\.java:(\d+): error: (.+)/g,
      ),
    );

  if (
    compilerErrors.length > 0
  ) {
    const explanations =
      compilerErrors.map(
        (match) => {
          const line =
            match[1];

          const message =
            match[2].trim();

          let explanation =
            message;

          if (
            message.includes(
              "'{' expected",
            )
          ) {
            explanation =
              "Java expected an opening brace `{` here. Check the class, method, or block structure.";
          } else if (
            message.includes(
              "';' expected",
            )
          ) {
            explanation =
              "Java expected a semicolon `;` here. Check the end of the statement.";
          } else if (
            message.includes(
              "incompatible types",
            )
          ) {
            explanation =
              "The value's data type does not match the type Java expects here.";
          } else if (
            message.includes(
              "cannot find symbol",
            )
          ) {
            explanation =
              "Java cannot find the variable, method, or class name. Check its spelling and whether it was declared.";
          } else if (
            message.includes(
              "class, interface, enum, or record expected",
            )
          ) {
            explanation =
              "Java found code outside the expected class or block structure. Check your braces `{ }`.";
          } else if (
            message.includes(
              "illegal start",
            )
          ) {
            explanation =
              "Java found a piece of code in a place where that kind of syntax is not allowed.";
          } else if (
            message.includes(
              "unclosed string literal",
            )
          ) {
            explanation =
              'A string starts with `"` but Java could not find its closing `"`.';
          }

          return `Line ${line}: ${explanation}`;
        },
      );

    const unique =
      Array.from(
        new Set(
          explanations,
        ),
      );

    return (
      `Java found ${unique.length} ${
        unique.length === 1
          ? "problem"
          : "problems"
      }:\n\n` +
      unique
        .map(
          (item, index) =>
            `${index + 1}. ${item}`,
        )
        .join("\n")
    );
  }

  if (
    raw.includes(
      "ArithmeticException",
    )
  ) {
    return (
      "Runtime error: the program tried " +
      "to perform an invalid arithmetic " +
      "operation, commonly division by zero."
    );
  }

  if (
    raw.includes(
      "NullPointerException",
    )
  ) {
    return (
      "Runtime error: the program tried " +
      "to use an object reference that is `null`."
    );
  }

  if (
    raw.includes(
      "ArrayIndexOutOfBoundsException",
    )
  ) {
    return (
      "Runtime error: the program tried " +
      "to access an array position outside " +
      "its valid range."
    );
  }

  if (
    raw.includes(
      "NumberFormatException",
    )
  ) {
    return (
      "Runtime error: Java tried to convert " +
      "text into a number, but the text was " +
      "not a valid number."
    );
  }

  if (
    raw.includes(
      "ClassNotFoundException",
    )
  ) {
    return (
      "Java could not find the class it was " +
      "asked to run. Check the class name " +
      "and classpath."
    );
  }

  return (
    "Java reported an error while compiling " +
    "or running the program."
  );
}

const TerminalInput = memo(
  function TerminalInput({
    onSubmit,
  }: {
    onSubmit: (
      input: string,
    ) => Promise<void>;
  }) {
    const [
      value,
      setValue,
    ] = useState("");

    const handleSubmit =
      async (
        event: FormEvent<HTMLFormElement>,
      ) => {
        event.preventDefault();

        const input =
          value;

        try {
          await onSubmit(
            input,
          );

          setValue("");
        } catch {
          // The parent already shows the error toast.
        }
      };

    const isolateEvent = (
      event: SyntheticEvent,
    ) => {
      event.stopPropagation();
    };

    return (
      <form
        className="terminal-input"
        onSubmit={
          handleSubmit
        }
        onPointerDown={
          isolateEvent
        }
        onMouseDown={
          isolateEvent
        }
        onClick={
          isolateEvent
        }
      >
        <span className="terminal-input-label">
          stdin &gt;
        </span>

        <input
          className="terminal-input-field"
          type="text"
          value={value}
          onChange={(
            event,
          ) =>
            setValue(
              event.target.value,
            )
          }
          placeholder="Enter Scanner input..."
          autoComplete="off"
          spellCheck={false}
          aria-label="Java program input"
          onPointerDown={
            isolateEvent
          }
          onMouseDown={
            isolateEvent
          }
          onClick={
            isolateEvent
          }
          onFocus={
            isolateEvent
          }
          onKeyDown={
            isolateEvent
          }
        />

        <button
          className="terminal-send-button"
          type="submit"
          onPointerDown={
            isolateEvent
          }
          onMouseDown={
            isolateEvent
          }
          onClick={
            isolateEvent
          }
        >
          Send
        </button>
      </form>
    );
  },
);

export default function TerminalPanel({
  terminalOpen,
  setTerminalOpen,
  executionState,
  interactive,
  output,
  errorOutput,
  clearTerminal,
  onInputSubmit,
}: TerminalPanelProps) {
  const hasOutput =
    output.trim().length >
    0;

  const hasError =
    errorOutput.trim().length >
    0;

  const friendlyError =
    friendlyJavaError(
      errorOutput,
    );

  return (
    <section
      className={`terminal-panel ${
        terminalOpen
          ? "terminal-open"
          : "terminal-collapsed"
      }`}
    >
      <div className="terminal-header">
        <button
          className="terminal-title-button"
          type="button"
          onClick={() =>
            setTerminalOpen(
              (current) =>
                !current,
            )
          }
          aria-expanded={
            terminalOpen
          }
        >
          <span className="terminal-title">
            <span className="terminal-icon">
              &gt;_
            </span>

            Terminal
          </span>
        </button>

        <div className="terminal-actions">
          <button
            className="terminal-clear-button"
            type="button"
            onClick={
              clearTerminal
            }
          >
            Clear
          </button>

          <button
            className="terminal-collapse-button"
            type="button"
            onClick={() =>
              setTerminalOpen(
                (current) =>
                  !current,
              )
            }
            aria-label={
              terminalOpen
                ? "Collapse terminal"
                : "Expand terminal"
            }
          >
            {terminalOpen
              ? "⌄"
              : "⌃"}
          </button>
        </div>
      </div>

      {terminalOpen && (
        <div className="terminal-body">
          {!hasOutput &&
            !hasError &&
            executionState !==
              "running" && (
              <div className="terminal-empty">
                Run a Java class to see
                its output here.
              </div>
            )}

          {executionState ===
            "running" &&
            !hasOutput &&
            !hasError && (
              <div className="terminal-running">
                {interactive
                  ? "Java is waiting for input."
                  : "Java process is running..."}
              </div>
            )}

          {hasOutput && (
            <pre className="terminal-stdout">
              {output}
            </pre>
          )}

          {hasError && (
            <div className="terminal-error-area">
              <div className="terminal-friendly-error">
                {friendlyError}
              </div>

              <details className="terminal-raw-details">
                <summary>
                  Show raw compiler output
                </summary>

                <pre className="terminal-stderr">
                  {errorOutput}
                </pre>
              </details>
            </div>
          )}

          {executionState ===
            "running" &&
            interactive && (
              <TerminalInput
                onSubmit={
                  onInputSubmit
                }
              />
            )}
        </div>
      )}
    </section>
  );
}