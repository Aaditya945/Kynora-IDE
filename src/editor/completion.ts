import type { editor } from "monaco-editor";
import { monaco } from "../services/monaco";
import type { JdtlsCompletionItem, JdtlsCompletionResult, JdtlsRange } from "../services/jdtls";

export function mapMonacoCompletionTriggerKind(
  triggerKind?: number,
): number {
  switch (triggerKind) {
    case 1:
      return 2;

    case 2:
      return 3;

    default:
      return 1;
  }
}

export function mapCompletionKind(
  kind?: number,
): number {
  switch (kind) {
    case 1:
      return monaco.languages.CompletionItemKind.Text;

    case 2:
      return monaco.languages.CompletionItemKind.Method;

    case 3:
      return monaco.languages.CompletionItemKind.Function;

    case 4:
      return monaco.languages.CompletionItemKind.Constructor;

    case 5:
      return monaco.languages.CompletionItemKind.Field;

    case 6:
      return monaco.languages.CompletionItemKind.Variable;

    case 7:
      return monaco.languages.CompletionItemKind.Class;

    case 8:
      return monaco.languages.CompletionItemKind.Interface;

    case 9:
      return monaco.languages.CompletionItemKind.Module;

    case 10:
      return monaco.languages.CompletionItemKind.Property;

    case 11:
      return monaco.languages.CompletionItemKind.Unit;

    case 12:
      return monaco.languages.CompletionItemKind.Value;

    case 13:
      return monaco.languages.CompletionItemKind.Enum;

    case 14:
      return monaco.languages.CompletionItemKind.Keyword;

    case 15:
      return monaco.languages.CompletionItemKind.Snippet;

    case 16:
      return monaco.languages.CompletionItemKind.Color;

    case 17:
      return monaco.languages.CompletionItemKind.File;

    case 18:
      return monaco.languages.CompletionItemKind.Reference;

    case 19:
      return monaco.languages.CompletionItemKind.Folder;

    case 20:
      return monaco.languages.CompletionItemKind.EnumMember;

    case 21:
      return monaco.languages.CompletionItemKind.Constant;

    case 22:
      return monaco.languages.CompletionItemKind.Struct;

    case 23:
      return monaco.languages.CompletionItemKind.Event;

    case 24:
      return monaco.languages.CompletionItemKind.Operator;

    case 25:
      return monaco.languages.CompletionItemKind.TypeParameter;

    default:
      return monaco.languages.CompletionItemKind.Text;
  }
}

export function getCompletionLabel(
  item: JdtlsCompletionItem,
): string {
  if (
    typeof item.label ===
    "string"
  ) {
    return item.label;
  }

  return item.label.label;
}

export function getCompletionDetail(
  item: JdtlsCompletionItem,
): string | undefined {
  if (item.detail) {
    return item.detail;
  }

  if (
    typeof item.label !==
      "string" &&
    item.label.detail
  ) {
    return item.label.detail;
  }

  if (
    item.labelDetails?.detail
  ) {
    return item.labelDetails.detail;
  }

  return undefined;
}

export function getCompletionDocumentation(
  item: JdtlsCompletionItem,
): string | undefined {
  if (!item.documentation) {
    return undefined;
  }

  if (
    typeof item.documentation ===
    "string"
  ) {
    return item.documentation;
  }

  return item.documentation.value;
}

export function isMarkdownDocumentation(
  item: JdtlsCompletionItem,
): boolean {
  return Boolean(
    item.documentation &&
      typeof item.documentation ===
        "object" &&
      item.documentation.kind ===
        "markdown",
  );
}

export function isLspTextEdit(
  value:
    | JdtlsCompletionItem["textEdit"],
): value is {
  range: JdtlsRange;
  newText: string;
} {
  return Boolean(
    value &&
      "range" in value &&
      value.range &&
      typeof value.newText ===
        "string",
  );
}

export function isLspInsertReplaceEdit(
  value:
    | JdtlsCompletionItem["textEdit"],
): value is {
  newText: string;
  insert: JdtlsRange;
  replace: JdtlsRange;
} {
  return Boolean(
    value &&
      "insert" in value &&
      "replace" in value &&
      typeof value.newText ===
        "string",
  );
}

export function lspRangeToMonacoRange(
  range: JdtlsRange,
): monaco.Range {
  return new monaco.Range(
    range.start.line + 1,
    range.start.character + 1,
    range.end.line + 1,
    range.end.character + 1,
  );
}

export function getCompletionInsertRange(
  item: JdtlsCompletionItem,
  model: editor.ITextModel,
  position: monaco.Position,
): monaco.Range {
  if (
    isLspTextEdit(
      item.textEdit,
    )
  ) {
    return lspRangeToMonacoRange(
      item.textEdit.range,
    );
  }

  if (
    isLspInsertReplaceEdit(
      item.textEdit,
    )
  ) {
    return lspRangeToMonacoRange(
      item.textEdit.replace,
    );
  }

  const word =
    model.getWordUntilPosition(
      position,
    );

  return new monaco.Range(
    position.lineNumber,
    word.startColumn,
    position.lineNumber,
    word.endColumn,
  );
}

export function getCompletionInsertText(
  item: JdtlsCompletionItem,
): string {
  if (
    isLspTextEdit(
      item.textEdit,
    )
  ) {
    return item.textEdit.newText;
  }

  if (
    isLspInsertReplaceEdit(
      item.textEdit,
    )
  ) {
    return item.textEdit.newText;
  }

  if (
    typeof item.insertText ===
    "string"
  ) {
    return item.insertText;
  }

  return getCompletionLabel(
    item,
  );
}

export function convertAdditionalTextEdits(
  edits:
    | JdtlsCompletionItem["additionalTextEdits"],
):
  | editor.ISingleEditOperation[]
  | undefined {
  if (
    !Array.isArray(edits) ||
    edits.length === 0
  ) {
    return undefined;
  }

  return edits.map(
    (
      edit,
    ) => ({
      range:
        lspRangeToMonacoRange(
          edit.range,
        ),

      text:
        edit.newText,
    }),
  );
}
