# Tauri Java Editor

A minimal Tauri 2 + React + Vite + TypeScript desktop editor shell with Monaco Editor configured for Java syntax highlighting.

## Stack

- Tauri 2
- React 19
- Vite 8
- TypeScript 7
- Monaco Editor 0.57

## Structure

```text
src/
  components/
    JavaEditor.tsx
  services/
    monaco.ts
  stores/
    editorStore.ts
  App.tsx
  main.tsx
  styles.css
src-tauri/
  src/
    lib.rs
    main.rs
  build.rs
  Cargo.toml
  tauri.conf.json
```

## Run

Make sure Node.js and Rust are installed.

```bash
npm install
npm run tauri dev
```

For a frontend-only browser check:

```bash
npm run dev
```

The editor starts blank with the Monaco language mode set to Java.
