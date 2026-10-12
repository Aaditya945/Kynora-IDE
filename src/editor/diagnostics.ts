import type { editor } from "monaco-editor";
import { monaco } from "../services/monaco";
import type { JdtlsDiagnosticsEvent } from "../services/jdtls";

function normalizeWindowsFileUri(uri: string): string {
  return uri.replace(/^file:\/\/\/?\/?/, "file:///").replace(/^file:\/\/\/([A-Za-z]):/, "file:///$1:");
}

function mapSeverity(severity: number): editor.MarkerSeverity {
  switch (severity) {
    case 1:
      return monaco.MarkerSeverity.Error;
    case 2:
      return monaco.MarkerSeverity.Warning;
    case 3:
      return monaco.MarkerSeverity.Info;
    default:
      return monaco.MarkerSeverity.Hint;
  }
}

export function createDiagnosticsHandler(options: {
  instance: editor.IStandaloneCodeEditor;
  documentUri: () => string | null;
  isDisposed: () => boolean;
}): (event: JdtlsDiagnosticsEvent) => void {
  const { instance, documentUri, isDisposed } = options;

  return (event: JdtlsDiagnosticsEvent) => {
    if (isDisposed() || !instance) {
      return;
    }

    const model = instance.getModel();

    if (!model) {
      return;
    }

    const expectedUri = documentUri()
      ? normalizeWindowsFileUri(documentUri()!)
      : normalizeWindowsFileUri(model.uri.toString());

    const receivedUri = normalizeWindowsFileUri(event.uri);

    if (receivedUri.toLowerCase() !== expectedUri.toLowerCase()) {
      return;
    }

    const markers = event.diagnostics.map((diagnostic) => {
      const startLineNumber = Math.max(1, diagnostic.range.start.line + 1);

      const endLineNumber = Math.max(
        startLineNumber,
        diagnostic.range.end.line + 1,
      );

      const startLineContent = model.getLineContent(startLineNumber);
      const endLineContent = model.getLineContent(endLineNumber);

      const maxStartColumn = startLineContent.length + 1;
      const maxEndColumn = endLineContent.length + 1;

      const startColumn = Math.min(
        Math.max(1, diagnostic.range.start.character + 1),
        maxStartColumn,
      );

      let endColumn = Math.min(
        Math.max(1, diagnostic.range.end.character + 1),
        maxEndColumn,
      );

      if (endLineNumber === startLineNumber) {
        endColumn = Math.max(
          endColumn,
          Math.min(maxEndColumn, startColumn + 1),
        );
      }

      return {
        severity: mapSeverity(diagnostic.severity),
        message: diagnostic.source
          ? `${diagnostic.message} [${diagnostic.source}]`
          : diagnostic.message,
        startLineNumber,
        startColumn,
        endLineNumber,
        endColumn,
      };
    });

    monaco.editor.setModelMarkers(model, "jdtls", markers);
  };
}
