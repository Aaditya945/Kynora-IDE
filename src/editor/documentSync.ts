import type { editor } from "monaco-editor";
import {
  changeJdtlsDocument,
  type JdtlsContentChange,
} from "../services/jdtls";

export interface DocumentSyncChangeResult {
  settledSyncPromise: Promise<void>;
  insertedDot: boolean;
  triggerPosition: editor.IPosition | null;
  nextVersion: number;
}

interface DocumentSyncControllerOptions {
  setContent: (content: string) => void;
  isDisposed: () => boolean;
  isJdtlsReady: () => boolean;
  getDocumentUri: () => string | null;
  cancelActiveCompletion: (reason: string) => void;
}

export function createDocumentSyncController(
  options: DocumentSyncControllerOptions,
) {
  let documentVersion = 0;

  let latestSyncPromise: Promise<void> | null =
    null;

  let documentSyncTail: Promise<void> =
    Promise.resolve();

  let nextDocumentSyncSequence = 1;

  let documentSyncHealthy = true;

  const handleDocumentChange = (
    instance: editor.IStandaloneCodeEditor,
    event: editor.IModelContentChangedEvent,
    completionGeneration: number,
  ): DocumentSyncChangeResult | null => {
    const content =
      instance.getValue();

    options.setContent(
      content,
    );

    /*
     * Any document edit makes all previous completion
     * results stale.
     */
    options.cancelActiveCompletion(
      "document-changed",
    );

    if (
      options.isDisposed() ||
      !options.isJdtlsReady() ||
      !options.getDocumentUri()
    ) {
      return null;
    }

    const model =
      instance.getModel();

    const uri =
      options.getDocumentUri();

    if (!uri) {
      return null;
    }

    const nextVersion =
      documentVersion + 1;

    documentVersion =
      nextVersion;

    const monacoVersion =
      model?.getVersionId() ??
      0;

    const documentLengthUtf16 =
      content.length;

    const documentLineCount =
      model?.getLineCount() ??
      1;

    const frontendSyncSequence =
      nextDocumentSyncSequence;

    nextDocumentSyncSequence +=
      1;

    /*
     * Convert Monaco ranges to LSP UTF-16 positions.
     */
    const incrementalChanges:
      JdtlsContentChange[] =
      event.changes
        .map(
          (
            change,
          ) => ({
            range: {
              start: {
                line:
                  change
                    .range
                    .startLineNumber -
                  1,

                character:
                  change
                    .range
                    .startColumn -
                  1,
              },

              end: {
                line:
                  change
                    .range
                    .endLineNumber -
                  1,

                character:
                  change
                    .range
                    .endColumn -
                  1,
              },
            },

            text:
              change.text,
          }),
        )
        .sort(
          (a, b) => {
            if (
              a.range!.start.line !==
              b.range!.start.line
            ) {
              return (
                b.range!.start.line -
                a.range!.start.line
              );
            }

            return (
              b.range!.start.character -
              a.range!.start.character
            );
          },
        );

    const eventHasChanges =
      incrementalChanges.length >
      0;

    const syncPromise =
      documentSyncTail.then(
        async () => {
          if (
            options.isDisposed() ||
            !options.isJdtlsReady() ||
            options.getDocumentUri() !==
              uri
          ) {
            return;
          }

          /*
           * If the previous sync failed,
           * the current edit becomes a complete
           * document resynchronization.
           */
          const requiresFullResync =
            !documentSyncHealthy ||
            !eventHasChanges;

          const outboundChanges:
            JdtlsContentChange[] =
            requiresFullResync
              ? [
                  {
                    text:
                      content,
                  },
                ]
              : incrementalChanges;

          try {
            await changeJdtlsDocument(
              uri,
              nextVersion,
              outboundChanges,
              frontendSyncSequence,
              monacoVersion,
              completionGeneration,
              documentLengthUtf16,
              documentLineCount,
            );

            documentSyncHealthy =
              true;
          } catch (
            error
          ) {
            documentSyncHealthy =
              false;

            throw error;
          }
        },
      );

    /*
     * Always settle the queue so one failure cannot
     * permanently block future document updates.
     */
    const settledSyncPromise =
      syncPromise.catch(
        (error) => {
          if (
            !options.isDisposed()
          ) {
            console.error(
              "Kynora JDTLS document sync failed:",
              {
                syncSequence:
                  frontendSyncSequence,

                version:
                  nextVersion,

                error,
              },
            );
          }
        },
      );

    documentSyncTail =
      settledSyncPromise;

    latestSyncPromise =
      settledSyncPromise;

    /*
     * Automatic "." completion.
     */
    const insertedDot =
      event.changes.some(
        (
          change,
        ) =>
          change.text.endsWith(
            ".",
          ),
      );

    const triggerPosition =
      instance.getPosition();

    return {
      settledSyncPromise,
      insertedDot,
      triggerPosition,
      nextVersion,
    };
  };

  const waitForLatestSync =
    async (): Promise<void> => {
      let observedSync =
        latestSyncPromise;

      while (
        observedSync
      ) {
        await observedSync;

        if (
          observedSync ===
          latestSyncPromise
        ) {
          break;
        }

        observedSync =
          latestSyncPromise;
      }
    };

  const resetPendingState =
    (): void => {
      documentSyncHealthy =
        true;

      documentSyncTail =
        Promise.resolve();

      latestSyncPromise =
        null;
    };

  const setVersion =
    (
      effectiveVersion: number,
    ): void => {
      documentVersion =
        effectiveVersion;

      documentSyncHealthy =
        true;

      documentSyncTail =
        Promise.resolve();

      latestSyncPromise =
        null;
    };

  const reset =
    (): void => {
      documentVersion =
        0;

      latestSyncPromise =
        null;

      documentSyncTail =
        Promise.resolve();

      documentSyncHealthy =
        true;

      nextDocumentSyncSequence =
        1;
    };

  return {
    handleDocumentChange,
    waitForLatestSync,
    resetPendingState,
    setVersion,
    reset,

    getVersion:
      (): number =>
        documentVersion,

    isHealthy:
      (): boolean =>
        documentSyncHealthy,
  };
}