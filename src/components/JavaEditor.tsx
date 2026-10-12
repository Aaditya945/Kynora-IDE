import { useEffect, useRef } from "react";
import type { editor } from "monaco-editor";
import { Channel } from "@tauri-apps/api/core";
import { monaco } from "../services/monaco";
import {
  cancelJdtlsRequest,
  closeJdtlsDocument,
  openJdtlsDocument,
  startJdtls,
  type JdtlsDiagnosticsEvent,
} from "../services/jdtls";
import { useEditorStore } from "../stores/editorStore";
import { createDiagnosticsHandler } from "../editor/diagnostics";
import { createDocumentSyncController } from "../editor/documentSync";
import { createCompletionProvider } from "../editor/completionProvider";

export default function JavaEditor() {
  const containerRef =
    useRef<HTMLDivElement | null>(
      null,
    );

  const editorRef =
    useRef<
      editor.IStandaloneCodeEditor | null
    >(null);

  const documentUriRef =
    useRef<string | null>(
      null,
    );

  const jdtlsReadyRef =
    useRef(false);

  const completionGenerationRef =
    useRef(0);

  /*
   * Frontend-owned completion JSON-RPC IDs.
   */
  const nextCompletionRequestIdRef =
    useRef(1_000_000_000);

  /*
   * Currently active completion request.
   */
  const activeCompletionRequestIdRef =
    useRef<number | null>(
      null,
    );

  /*
   * Custom "." trigger marker.
   */
  const forcedCompletionTriggerKindRef =
    useRef<number | null>(
      null,
    );

  const diagnosticsChannelRef =
    useRef<
      Channel<JdtlsDiagnosticsEvent> | null
    >(null);

  const setContent =
    useEditorStore(
      (state) =>
        state.setContent,
    );

  useEffect(() => {
    const initialContent =
      useEditorStore.getState()
        .content;

    if (
      !containerRef.current
    ) {
      return;
    }

    let disposed =
      false;

    const instance =
      monaco.editor.create(
        containerRef.current,
        {
          value:
            initialContent,

          language:
            "java",

          theme:
            "vs-dark",

          automaticLayout:
            true,

          minimap: {
            enabled:
              false,
          },

          fontSize:
            14,

          fontFamily:
            'Consolas, "Cascadia Code", monospace',

          padding: {
            top:
              12,
          },

          scrollBeyondLastLine:
            false,

          tabSize:
            4,

          insertSpaces:
            true,

          wordWrap:
            "off",

          renderWhitespace:
            "selection",

          renderValidationDecorations:
            "on",

          smoothScrolling:
            true,
        },
      );

    editorRef.current =
      instance;

    instance.focus();

    const handleDiagnostics =
      createDiagnosticsHandler({
        instance,

        documentUri: () =>
          documentUriRef.current,

        isDisposed: () =>
          disposed,
      });

    const cancelActiveCompletion =
      (
        reason: string,
      ) => {
        const requestId =
          activeCompletionRequestIdRef
            .current;

        if (
          requestId ===
          null
        ) {
          return;
        }

        activeCompletionRequestIdRef
          .current = null;

        console.debug(
          "Kynora completion cancellation:",
          {
            requestId,
            reason,
          },
        );

        void cancelJdtlsRequest(
          requestId,
        ).catch(
          (error) => {
            if (!disposed) {
              console.warn(
                "Kynora completion cancellation failed:",
                {
                  requestId,
                  reason,
                  error,
                },
              );
            }
          },
        );
      };

    /*
     * ========================================================
     * DOCUMENT SYNC
     * ========================================================
     */
    const documentSyncController =
      createDocumentSyncController({
        setContent,

        isDisposed: () =>
          disposed,

        isJdtlsReady: () =>
          jdtlsReadyRef.current,

        getDocumentUri: () =>
          documentUriRef.current,

        cancelActiveCompletion,
      });

    const changeSubscription =
      instance.onDidChangeModelContent(
        (event) => {
          completionGenerationRef.current +=
            1;

          const syncResult =
            documentSyncController.handleDocumentChange(
              instance,
              event,
              completionGenerationRef.current,
            );

          if (!syncResult) {
            return;
          }

          const {
            settledSyncPromise,
            insertedDot,
            triggerPosition,
            nextVersion,
          } = syncResult;

          if (
            insertedDot &&
            triggerPosition
          ) {
            void settledSyncPromise.then(
              () => {
                if (
                  disposed ||
                  !jdtlsReadyRef.current ||
                  !documentSyncController.isHealthy()
                ) {
                  return;
                }

                if (
                  documentSyncController.getVersion() !==
                  nextVersion
                ) {
                  return;
                }

                const currentPosition =
                  instance.getPosition();

                if (
                  !currentPosition
                ) {
                  return;
                }

                if (
                  currentPosition
                    .lineNumber !==
                    triggerPosition
                      .lineNumber ||
                  currentPosition
                    .column !==
                    triggerPosition
                      .column
                ) {
                  return;
                }

                forcedCompletionTriggerKindRef
                  .current = 2;

                instance.trigger(
                  "kynora-dot-trigger",
                  "editor.action.triggerSuggest",
                  {},
                );
              },
            );
          }
        },
      );

    /*
     * ========================================================
     * COMPLETION PROVIDER
     * ========================================================
     */
    const completionProvider =
      createCompletionProvider({
        instance,

        isDisposed: () =>
          disposed,

        isJdtlsReady: () =>
          jdtlsReadyRef.current,

        getDocumentUri: () =>
          documentUriRef.current,

        documentSyncController,

        completionGenerationRef,

        nextCompletionRequestIdRef,

        activeCompletionRequestIdRef,

        forcedCompletionTriggerKindRef,

        cancelActiveCompletion,
      });

    /*
     * ========================================================
     * JDTLS SETUP
     * ========================================================
     */
    const setupJdtls =
      async () => {
        try {
          const diagnosticsChannel =
            new Channel<JdtlsDiagnosticsEvent>();

          diagnosticsChannel.onmessage =
            (
              event,
            ) => {
              handleDiagnostics(
                event,
              );
            };

          diagnosticsChannelRef.current =
            diagnosticsChannel;

          if (disposed) {
            return;
          }

          const started =
            await startJdtls(
              diagnosticsChannel,
            );

          if (disposed) {
            return;
          }

          documentUriRef.current =
            started.document_uri;

          documentSyncController
            .resetPendingState();

          const effectiveVersion =
            await openJdtlsDocument(
              started.document_uri,
              "java",
              instance.getValue(),
              1,
            );

          if (disposed) {
            await closeJdtlsDocument(
              started.document_uri,
            ).catch(
              () => undefined,
            );

            return;
          }

          documentSyncController.setVersion(
            effectiveVersion,
          );

          jdtlsReadyRef.current =
            true;

          console.info(
            "Kynora JDTLS ready.",
            {
              documentVersion:
                effectiveVersion,
            },
          );
        } catch (
          error
        ) {
          console.error(
            "Kynora could not start JDTLS:",
            error,
          );
        }
      };

    void setupJdtls();

    /*
     * ========================================================
     * CLEANUP
     * ========================================================
     */
    return () => {
      disposed = true;

      jdtlsReadyRef.current =
        false;

      completionGenerationRef.current +=
        1;

      /*
       * Cleanup cancellation uses the same single-cancel
       * helper, so duplicate $/cancelRequest is avoided.
       */
      cancelActiveCompletion(
        "editor-disposed",
      );

      forcedCompletionTriggerKindRef
        .current = null;

      changeSubscription.dispose();

      completionProvider.dispose();

      const uri =
        documentUriRef.current;

      documentUriRef.current =
        null;

      documentSyncController.reset();

      diagnosticsChannelRef.current =
        null;

      if (uri) {
        void closeJdtlsDocument(
          uri,
        ).catch(
          () => undefined,
        );
      }

      const model =
        instance.getModel();

      if (model) {
        monaco.editor.setModelMarkers(
          model,
          "jdtls",
          [],
        );
      }

      editorRef.current =
        null;

      instance.dispose();
    };
  }, [setContent]);

  return (
    <div
      ref={containerRef}
      className="editor"
      aria-label="Java code editor"
    />
  );
}