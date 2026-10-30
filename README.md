# Kynora IDE

Kynora is a Java-focused desktop IDE built with **Tauri 2, React, TypeScript, Rust, and Monaco Editor**.

> **Source-available and proprietary.** This repository publishes Kynora's original source code for public inspection. It is not an open-source release. See [`LICENSE`](https://github.com/Aaditya945/Kynora-IDE/blob/main/LICENSE) for the proprietary terms. Because this repository is public, GitHub's Terms of Service allow users to view and fork it through GitHub; the custom notice does not override those platform permissions.

## Features

- **Java editing:** Monaco Editor configured for Java.
- **Compile and run:** invokes the local JDK's `javac` and `java` tools.
- **Terminal output and input:** displays program output and errors, with interactive standard input for programs that read input (for example, `Scanner`).
- **Java language-server integration:** Kynora's source contains Eclipse JDT Language Server Protocol integration for diagnostics and completion. The JDTLS runtime itself is **not included** in this repository and must be supplied separately.
- **AI Tutor:** uses OpenRouter to give step-by-step, Socratic-style guidance on Java errors. You provide your own API key in the app.
- **JDK detection:** checks `JAVA_HOME` and common installation locations before development/build commands.

## Technology

- Tauri 2 / Rust native backend
- React 19 / TypeScript 7 / Vite 8
- Monaco Editor 0.57
- Zustand state management
- Java JDK tools and an externally supplied Eclipse JDT Language Server runtime
- OpenRouter API for the AI Tutor

## Requirements

This repository is currently configured for a Windows desktop build.

- Node.js **20.19+** or **22.12+**, with npm
- Rust toolchain **1.90+**
- The Windows build prerequisites required by Tauri 2 (including Microsoft C++ Build Tools)
- A JDK with both `java` and `javac`; **JDK 21 or newer** is required for the JDTLS integration
- A compatible Eclipse JDT Language Server distribution if you want language-server features or want to package the full desktop app

## Getting started

1. Clone this repository and enter the project directory:

   git clone [https://github.com/Aaditya945/Kynora-IDE.git](https://github.com/Aaditya945/Kynora-IDE.git) cd Kynora-IDE
2. Install Java and set `JAVA_HOME` to the JDK directory if it is not already discoverable through one of the locations checked by the JDK detection script.
3. Install JavaScript dependencies:

   npm ci
4. **Set up JDTLS separately.** Download a compatible distribution from the [official Eclipse JDT Language Server project](https://github.com/eclipse-jdtls/eclipse.jdt.ls) and unpack it into a `jdtls/` directory at the project root. The directory should include the distribution's `plugins/` directory and the matching platform configuration. Review and comply with the upstream and bundled third-party license notices. The `jdtls/` directory is intentionally excluded from this repository.
5. Start the desktop app in development mode:

   npm run tauri dev
6. To build the configured Windows installer (after completing the Tauri and JDTLS setup):

   npm run tauri build

For a frontend-only browser preview, run `npm run dev`. Native Tauri commands and the full IDE workflow require the desktop app.

## AI Tutor setup and privacy

Open the AI Tutor panel in Kynora, enter your own OpenRouter API key, and select **Save Key**. The key is stored in the app's local WebView storage. When you ask the tutor for help, Kynora sends the current Java error text to OpenRouter to request an explanation. Do not submit secrets or sensitive information in error text. Do not commit API keys, `.env` files, or build-time secrets to this public repository.

## Project layout
```
src/                  React/TypeScript UI, editor, terminal, AI Tutor
src-tauri/src/        Rust backend, Java execution, JDTLS/LSP integration
src-tauri/            Tauri configuration, capabilities, icons
scripts/              Local JDK detection script

```

## Development notes

- `npm run dev` starts Vite and runs the JDK detection script.
- `npm run build` runs TypeScript checks and creates the frontend build.
- `npm run tauri dev` starts the Tauri desktop app.
- `npm run tauri build` builds the configured Windows installer. The Tauri configuration expects the external `jdtls/` runtime to be present for packaging.
- The JDK detection script writes `src/generated/jdk.ts` based on the local machine. Review this generated file before committing changes; do not publish a personal machine-specific JDK path.

## Licensing and ownership

Kynora's original source code is proprietary and all rights are reserved as stated in [`LICENSE`](https://github.com/Aaditya945/Kynora-IDE/blob/main/LICENSE). No general permission is granted to use, modify, redistribute, sell, or incorporate Kynora's original code into another product without prior written permission, subject to applicable law and GitHub's platform terms. Third-party libraries and tools remain under their own licenses; this notice does not relicense them.

This repository provides source code, not a prebuilt installer. Features and setup requirements may change as the project evolves.

---

## Project Status and Closure

<badge color="secondary">Permanently Discontinued</badge>

**Closure Date: 30 October 2026**

Kynora IDE was permanently discontinued on 30 October 2026.

The decision to close the project followed increasing development complexity, recurring bugs, and the difficulties encountered during optimization and code maintenance. The effort required to keep improving and optimizing the codebase became increasingly difficult to manage.

Kynora was not discontinued because its original idea lacked potential. The project was closed because maintaining its growing complexity and resolving recurring technical issues had become unsustainable.

The repository is preserved as a historical record of the project's source code, development work, and original vision.

## Lessons Learned

Kynora's development provided valuable practical lessons in software engineering:

1. **Plan before optimizing:** Understand the architecture, requirements, and limitations before attempting optimization.
2. **Think before writing more:** Consider maintainability and complexity before continuously adding new features or code.
3. **Refactor before the codebase becomes a dump:** Improve structure and organization regularly instead of allowing technical debt to accumulate.
4. **Stability before features:** A reliable feature is more valuable than several unfinished or unstable features.
5. **Optimization should have a purpose:** Optimization should solve measurable problems rather than become an endless cycle of changes.

These lessons remain an important part of Kynora's development journey.

**Kynora's development is closed. Its source code and the lessons learned from building it remain.**
