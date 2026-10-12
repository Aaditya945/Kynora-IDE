import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/editor/editor.worker.js?worker";

declare global {
  interface Window {
    MonacoEnvironment: {
      getWorker: (_moduleId: string, _label: string) => Worker;
    };
  }
}

window.MonacoEnvironment = {
  getWorker() {
    return new editorWorker();
  },
};

export { monaco };