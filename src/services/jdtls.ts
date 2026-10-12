import { Channel, invoke } from "@tauri-apps/api/core";

export interface JdtlsStartResult {
  started: boolean;
  document_uri: string;
  workspace_root: string;
  data_dir: string;
}

export interface JdtlsPosition {
  line: number;
  character: number;
}

export interface JdtlsRange {
  start: JdtlsPosition;
  end: JdtlsPosition;
}

/*
 * LSP textDocument/didChange content change.
 *
 * range:
 *   Present = incremental change.
 *
 *   Omitted = full-document replacement.
 *
 * M5 normally uses incremental changes.
 * Full-document changes remain available as a recovery path.
 */
export interface JdtlsContentChange {
  range?: JdtlsRange;
  text: string;
}

export interface JdtlsDiagnostic {
  range: JdtlsRange;
  severity?: number | null;
  message: string;
  source?: string | null;
}

export interface JdtlsDiagnosticsEvent {
  uri: string;
  diagnostics: JdtlsDiagnostic[];
}

/*
 * LSP CompletionItem
 */

export interface JdtlsCompletionLabelDetails {
  detail?: string;
  description?: string;
}

export interface JdtlsCompletionDocumentation {
  kind: "plaintext" | "markdown";
  value: string;
}

export interface JdtlsCompletionTextEdit {
  range: JdtlsRange;
  newText: string;
}

export interface JdtlsCompletionInsertReplaceEdit {
  newText: string;
  insert: JdtlsRange;
  replace: JdtlsRange;
}

export interface JdtlsCompletionItem {
  label:
    | string
    | {
        label: string;
        detail?: string;
        description?: string;
      };

  labelDetails?: JdtlsCompletionLabelDetails;

  kind?: number;

  tags?: number[];

  detail?: string;

  documentation?:
    | string
    | JdtlsCompletionDocumentation;

  sortText?: string;

  filterText?: string;

  preselect?: boolean;

  insertText?: string;

  insertTextFormat?: number;

  textEdit?:
    | JdtlsCompletionTextEdit
    | JdtlsCompletionInsertReplaceEdit;

  additionalTextEdits?: JdtlsCompletionTextEdit[];

  commitCharacters?: string[];

  command?: unknown;

  data?: unknown;
}

export interface JdtlsCompletionList {
  isIncomplete?: boolean;
  itemDefaults?: Record<string, unknown>;
  items: JdtlsCompletionItem[];
}

export interface JdtlsCompletionResult {
  isIncomplete: boolean;
  items: JdtlsCompletionItem[];
}

export async function startJdtls(
  onDiagnostics: Channel<JdtlsDiagnosticsEvent>,
): Promise<JdtlsStartResult> {
  return invoke<JdtlsStartResult>(
    "jdtls_start",
    {
      onDiagnostics,
    },
  );
}

export async function openJdtlsDocument(
  uri: string,
  languageId: string,
  text: string,
  version: number,
): Promise<number> {
  return invoke<number>(
    "jdtls_did_open",
    {
      uri,
      languageId,
      text,
      version,
    },
  );
}

/*
 * Refresh diagnostics using the JDTLS workspace command.
 */
export async function refreshJdtlsDiagnostics(
  uri: string,
): Promise<void> {
  await invoke(
    "jdtls_refresh_diagnostics",
    {
      uri,
    },
  );
}

/*
 * M5 incremental document synchronization.
 *
 * Extra metadata is diagnostic-only:
 *
 * frontendSyncSequence:
 *   Unique frontend change sequence.
 *
 * monacoVersion:
 *   Monaco model version at the moment of the edit.
 *
 * completionGeneration:
 *   Completion generation associated with the edit.
 *
 * documentLengthUtf16:
 *   Current Monaco document length in UTF-16 code units.
 *
 * documentLineCount:
 *   Current Monaco document line count.
 */
export async function changeJdtlsDocument(
  uri: string,
  version: number,
  changes: JdtlsContentChange[],
  frontendSyncSequence: number,
  monacoVersion: number,
  completionGeneration: number,
  documentLengthUtf16: number,
  documentLineCount: number,
): Promise<number> {
  return invoke<number>(
    "jdtls_did_change",
    {
      uri,
      version,
      changes,
      frontendSyncSequence:
        frontendSyncSequence,
      monacoVersion:
        monacoVersion,
      completionGeneration:
        completionGeneration,
      documentLengthUtf16:
        documentLengthUtf16,
      documentLineCount:
        documentLineCount,
    },
  );
}

/*
 * Generic completion request.
 *
 * requestId:
 *   Frontend-owned JSON-RPC id.
 *
 * triggerKind:
 *   LSP CompletionTriggerKind:
 *
 *   1 = Invoked
 *   2 = TriggerCharacter
 *   3 = TriggerForIncompleteCompletions
 *
 * triggerCharacter:
 *   Usually "." when triggerKind = 2.
 *
 * documentVersion:
 *   Document version captured before the request.
 *
 * completionGeneration:
 *   Completion generation captured before the request.
 */
export async function completionJdtlsDocument(
  uri: string,
  line: number,
  character: number,
  requestId: number,
  triggerKind: number,
  triggerCharacter: string | undefined,
  documentVersion: number,
  completionGeneration: number,
): Promise<JdtlsCompletionResult> {
  const result = await invoke<unknown>(
    "jdtls_completion",
    {
      uri,
      line,
      character,
      requestId,
      triggerKind,
      triggerCharacter:
        triggerCharacter ?? null,
      documentVersion:
        documentVersion,
      completionGeneration:
        completionGeneration,
    },
  );

  if (Array.isArray(result)) {
    return {
      isIncomplete: false,
      items:
        result as JdtlsCompletionItem[],
    };
  }

  if (
    result &&
    typeof result === "object"
  ) {
    const value =
      result as Partial<JdtlsCompletionList>;

    if (Array.isArray(value.items)) {
      return {
        isIncomplete:
          Boolean(value.isIncomplete),
        items:
          value.items,
      };
    }
  }

  return {
    isIncomplete: false,
    items: [],
  };
}

/*
 * Cancel an active LSP request.
 */
export async function cancelJdtlsRequest(
  requestId: number,
): Promise<void> {
  await invoke(
    "jdtls_cancel_request",
    {
      requestId,
    },
  );
}

/*
 * Resolve one completion item.
 */
export async function resolveJdtlsCompletionItem(
  item: JdtlsCompletionItem,
): Promise<JdtlsCompletionItem> {
  const result =
    await invoke<unknown>(
      "jdtls_completion_resolve",
      {
        item,
      },
    );

  if (
    result &&
    typeof result === "object"
  ) {
    return result as JdtlsCompletionItem;
  }

  return item;
}

export async function closeJdtlsDocument(
  uri: string,
): Promise<void> {
  await invoke(
    "jdtls_did_close",
    {
      uri,
    },
  );
}

export async function shutdownJdtls(): Promise<void> {
  await invoke(
    "jdtls_shutdown",
  );
}