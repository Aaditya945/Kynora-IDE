import type { editor } from "monaco-editor";
import type { IDisposable } from "monaco-editor";
import { monaco } from "../services/monaco";
import {
  cancelJdtlsRequest,
  completionJdtlsDocument,
  resolveJdtlsCompletionItem,
  type JdtlsCompletionItem,
  type JdtlsCompletionResult,
} from "../services/jdtls";
import {
  convertAdditionalTextEdits,
  getCompletionDetail,
  getCompletionDocumentation,
  getCompletionInsertRange,
  getCompletionInsertText,
  getCompletionLabel,
  isMarkdownDocumentation,
  mapCompletionKind,
  mapMonacoCompletionTriggerKind,
} from "./completion";

type Ref<T> = {
  current: T;
};

export type CompletionDocumentSyncController = {
  waitForLatestSync: () => Promise<void>;
  isHealthy: () => boolean;
  getVersion: () => number;
};

export type CompletionProviderOptions = {
  instance: editor.IStandaloneCodeEditor;

  isDisposed: () => boolean;

  isJdtlsReady: () => boolean;

  getDocumentUri: () => string | null;

  documentSyncController: CompletionDocumentSyncController;

  completionGenerationRef: Ref<number>;

  nextCompletionRequestIdRef: Ref<number>;

  activeCompletionRequestIdRef: Ref<number | null>;

  forcedCompletionTriggerKindRef: Ref<number | null>;

  cancelActiveCompletion: (reason: string) => void;
};

/*
 * Monaco CompletionTriggerKind:
 *
 * 0 = Invoke
 * 1 = TriggerCharacter
 * 2 = TriggerForIncompleteCompletions
 *
 * LSP CompletionTriggerKind:
 *
 * 1 = Invoked
 * 2 = TriggerCharacter
 * 3 = TriggerForIncompleteCompletions
 */
type KynoraMonacoCompletionItem =
  monaco.languages.CompletionItem & {
    __lspItem: JdtlsCompletionItem;
    __requestGeneration: number;
    __completionIndex: number;
  };

function convertCompletionItems(
  result: JdtlsCompletionResult,
  model: editor.ITextModel,
  position: monaco.Position,
  requestGeneration: number,
): monaco.languages.CompletionList {
  const suggestions = result.items.map(
    (item, index) => {
      const label = getCompletionLabel(
        item,
      );

      const documentation =
        getCompletionDocumentation(
          item,
        );

      const insertText =
        getCompletionInsertText(
          item,
        );

      const range =
        getCompletionInsertRange(
          item,
          model,
          position,
        );

      const additionalTextEdits =
        convertAdditionalTextEdits(
          item.additionalTextEdits,
        );

      const completionItem =
        {
          label,

          kind:
            mapCompletionKind(
              item.kind,
            ),

          detail:
            getCompletionDetail(
              item,
            ),

          documentation:
            documentation
              ? isMarkdownDocumentation(
                  item,
                )
                ? {
                    value:
                      documentation,
                    isTrusted:
                      false,
                  }
                : documentation
              : undefined,

          sortText:
            item.sortText,

          filterText:
            item.filterText,

          preselect:
            item.preselect,

          insertText,

          range,

          commitCharacters:
            item.commitCharacters,

          additionalTextEdits,

          __lspItem:
            item,

          __requestGeneration:
            requestGeneration,

          __completionIndex:
            index,
        } as KynoraMonacoCompletionItem;

      if (
        item.insertTextFormat ===
        2
      ) {
        completionItem.insertTextRules =
          monaco.languages
            .CompletionItemInsertTextRule
            .InsertAsSnippet;
      }

      return completionItem;
    },
  );

  return {
    incomplete:
      result.isIncomplete,

    suggestions,
  };
}

function applyResolvedCompletionItem(
  monacoItem: KynoraMonacoCompletionItem,
  resolved: JdtlsCompletionItem,
  model: editor.ITextModel,
  position: monaco.Position,
): KynoraMonacoCompletionItem {
  monacoItem.__lspItem =
    resolved;

  const detail =
    getCompletionDetail(
      resolved,
    );

  if (detail) {
    monacoItem.detail =
      detail;
  }

  const documentation =
    getCompletionDocumentation(
      resolved,
    );

  if (documentation) {
    monacoItem.documentation =
      isMarkdownDocumentation(
        resolved,
      )
        ? {
            value:
              documentation,
            isTrusted:
              false,
          }
        : documentation;
  }

  if (
    resolved.sortText
  ) {
    monacoItem.sortText =
      resolved.sortText;
  }

  if (
    resolved.filterText
  ) {
    monacoItem.filterText =
      resolved.filterText;
  }

  if (
    typeof resolved.preselect ===
    "boolean"
  ) {
    monacoItem.preselect =
      resolved.preselect;
  }

  if (
    Array.isArray(
      resolved.commitCharacters,
    )
  ) {
    monacoItem.commitCharacters =
      resolved.commitCharacters;
  }

  if (
    typeof resolved.insertText ===
    "string"
  ) {
    monacoItem.insertText =
      resolved.insertText;
  }

  if (
    resolved.textEdit
  ) {
    monacoItem.range =
      getCompletionInsertRange(
        resolved,
        model,
        position,
      );

    monacoItem.insertText =
      getCompletionInsertText(
        resolved,
      );
  }

  if (
    Array.isArray(
      resolved.additionalTextEdits,
    )
  ) {
    monacoItem.additionalTextEdits =
      convertAdditionalTextEdits(
        resolved.additionalTextEdits,
      );
  }

  if (
    resolved.insertTextFormat ===
    2
  ) {
    monacoItem.insertTextRules =
      monaco.languages
        .CompletionItemInsertTextRule
        .InsertAsSnippet;
  } else if (
    resolved.insertTextFormat ===
    1
  ) {
    monacoItem.insertTextRules =
      undefined;
  }

  return monacoItem;
}

export function createCompletionProvider({
  instance,
  isDisposed,
  isJdtlsReady,
  getDocumentUri,
  documentSyncController,
  completionGenerationRef,
  nextCompletionRequestIdRef,
  activeCompletionRequestIdRef,
  forcedCompletionTriggerKindRef,
  cancelActiveCompletion,
}: CompletionProviderOptions): IDisposable {
  return monaco.languages
    .registerCompletionItemProvider(
      "java",
      {
        provideCompletionItems:
          async (
            model,
            position,
            context,
            token,
          ) => {
            if (
              isDisposed() ||
              !isJdtlsReady()
            ) {
              return {
                suggestions: [],
              };
            }

            const uri =
              getDocumentUri();

            if (!uri) {
              return {
                suggestions: [],
              };
            }

            if (
              token.isCancellationRequested
            ) {
              return {
                suggestions: [],
              };
            }

            /*
             * Automatic "." trigger must override
             * Monaco's ordinary invocation context.
             */
            const forcedTriggerKind =
              forcedCompletionTriggerKindRef
                .current;

            forcedCompletionTriggerKindRef
              .current = null;

            const lspTriggerKind =
              forcedTriggerKind ??
              mapMonacoCompletionTriggerKind(
                context?.triggerKind,
              );

            const triggerCharacter =
              lspTriggerKind === 2
                ? forcedTriggerKind === 2
                  ? "."
                  : context?.triggerCharacter ??
                    undefined
                : undefined;

            /*
             * Wait until the latest document synchronization
             * has settled.
             */
            await documentSyncController
              .waitForLatestSync();

            if (
              isDisposed() ||
              token.isCancellationRequested ||
              !isJdtlsReady() ||
              !documentSyncController.isHealthy()
            ) {
              return {
                suggestions: [],
              };
            }

            /*
             * Cancel any older active completion.
             */
            if (
              activeCompletionRequestIdRef
                .current !==
              null
            ) {
              cancelActiveCompletion(
                "newer-completion-request",
              );
            }

            const requestId =
              nextCompletionRequestIdRef
                .current;

            nextCompletionRequestIdRef
              .current += 1;

            activeCompletionRequestIdRef
              .current =
              requestId;

            /*
             * Capture state after the synchronization
             * barrier.
             */
            const requestGeneration =
              completionGenerationRef
                .current;

            const requestVersion =
              documentSyncController
                .getVersion();

            const requestLine =
              position.lineNumber -
              1;

            const requestCharacter =
              position.column -
              1;

            /*
             * Prevent Monaco from firing two cancellation
             * calls for the same request.
             */
            let cancellationSent =
              false;

            const cancellationDisposable =
              token.onCancellationRequested(
                () => {
                  if (
                    cancellationSent
                  ) {
                    return;
                  }

                  cancellationSent =
                    true;

                  /*
                   * Only cancel if this request is
                   * still the active request.
                   */
                  if (
                    activeCompletionRequestIdRef
                      .current ===
                    requestId
                  ) {
                    activeCompletionRequestIdRef
                      .current =
                      null;

                    void cancelJdtlsRequest(
                      requestId,
                    ).catch(
                      (error) => {
                        if (
                          !isDisposed()
                        ) {
                          console.warn(
                            "Kynora completion cancellation failed:",
                            {
                              requestId,
                              error,
                            },
                          );
                        }
                      },
                    );
                  }
                },
              );

            try {
              const result =
                await completionJdtlsDocument(
                  uri,
                  requestLine,
                  requestCharacter,
                  requestId,
                  lspTriggerKind,
                  triggerCharacter,
                  requestVersion,
                  requestGeneration,
                );

              /*
               * Reject stale completion.
               */
              if (
                isDisposed() ||
                token.isCancellationRequested ||
                completionGenerationRef
                  .current !==
                  requestGeneration ||
                documentSyncController
                    .getVersion() !==
                  requestVersion
              ) {
                console.debug(
                  "Kynora discarded stale completion:",
                  {
                    requestId,
                    requestVersion,
                    currentVersion:
                      documentSyncController
                        .getVersion(),
                    requestGeneration,
                    currentGeneration:
                      completionGenerationRef
                        .current,
                  },
                );

                return {
                  suggestions: [],
                };
              }

              const currentModel =
                instance.getModel();

              if (
                !currentModel
              ) {
                return {
                  suggestions: [],
                };
              }

              const currentPosition =
                instance.getPosition();

              if (
                !currentPosition ||
                currentPosition
                  .lineNumber !==
                  position.lineNumber ||
                currentPosition
                  .column !==
                  position.column
              ) {
                return {
                  suggestions: [],
                };
              }

              const converted =
                convertCompletionItems(
                  result,
                  currentModel,
                  currentPosition,
                  requestGeneration,
                );

              console.info(
                "Kynora completion received:",
                {
                  requestId,

                  count:
                    converted
                      .suggestions
                      .length,

                  incomplete:
                    converted.incomplete,

                  line:
                    requestLine,

                  character:
                    requestCharacter,

                  triggerKind:
                    lspTriggerKind,

                  triggerCharacter:
                    triggerCharacter ??
                    null,

                  documentVersion:
                    requestVersion,

                  generation:
                    requestGeneration,
                },
              );

              return converted;
            } catch (
              error
            ) {
              if (
                !isDisposed() &&
                !token.isCancellationRequested
              ) {
                console.warn(
                  "Kynora JDTLS completion failed:",
                  {
                    requestId,

                    documentVersion:
                      requestVersion,

                    generation:
                      requestGeneration,

                    error,
                  },
                );
              }

              return {
                suggestions: [],
              };
            } finally {
              cancellationDisposable.dispose();

              if (
                activeCompletionRequestIdRef
                  .current ===
                requestId
              ) {
                activeCompletionRequestIdRef
                  .current =
                  null;
              }
            }
          },

        /*
         * ==================================================
         * COMPLETION ITEM RESOLVE
         * ==================================================
         */
        resolveCompletionItem:
          async (
            item,
            token,
          ) => {
            const kynoraItem =
              item as KynoraMonacoCompletionItem;

            const originalLspItem =
              kynoraItem.__lspItem;

            if (
              !originalLspItem
            ) {
              return item;
            }

            if (
              isDisposed() ||
              !isJdtlsReady() ||
              token.isCancellationRequested
            ) {
              return item;
            }

            const requestGeneration =
              completionGenerationRef
                .current;

            try {
              const resolved =
                await resolveJdtlsCompletionItem(
                  originalLspItem,
                );

              if (
                isDisposed() ||
                token.isCancellationRequested ||
                completionGenerationRef
                  .current !==
                  requestGeneration
              ) {
                return item;
              }

              const currentModel =
                instance.getModel();

              const currentPosition =
                instance.getPosition();

              if (
                !currentModel ||
                !currentPosition
              ) {
                return item;
              }

              const updated =
                applyResolvedCompletionItem(
                  kynoraItem,
                  resolved,
                  currentModel,
                  currentPosition,
                );

              console.info(
                "Kynora completionItem/resolve received.",
                {
                  generation:
                    requestGeneration,
                },
              );

              return updated;
            } catch (
              error
            ) {
              if (
                !isDisposed()
              ) {
                console.warn(
                  "Kynora JDTLS completionItem/resolve failed:",
                  error,
                );
              }

              return item;
            }
          },
      },
    );
}