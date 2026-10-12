import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

const outputDir = path.join(projectRoot, "src", "generated");
const outputFile = path.join(outputDir, "jdk.ts");

function exists(targetPath) {
  try {
    return fs.existsSync(targetPath);
  } catch {
    return false;
  }
}

function isJdk(directory) {
  const javac =
    process.platform === "win32"
      ? "javac.exe"
      : "javac";

  return exists(path.join(directory, "bin", javac));
}

function listDirectories(directory) {
  if (!exists(directory)) {
    return [];
  }

  try {
    return fs
      .readdirSync(directory, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => path.join(directory, entry.name));
  } catch {
    return [];
  }
}

function findInRoot(root) {
  if (isJdk(root)) {
    return root;
  }

  const candidates = listDirectories(root)
    .filter((directory) =>
      /(jdk|java|openjdk|temurin|corretto|zulu|liberica|microsoft)/i.test(
        path.basename(directory),
      ),
    )
    .sort((a, b) =>
      path.basename(b).localeCompare(
        path.basename(a),
        undefined,
        { numeric: true },
      ),
    );

  for (const candidate of candidates) {
    if (isJdk(candidate)) {
      return candidate;
    }
  }

  return null;
}

function getCommonRoots() {
  const home =
    process.env.USERPROFILE ||
    process.env.HOME ||
    "";

  const localAppData =
    process.env.LOCALAPPDATA ||
    path.join(home, "AppData", "Local");

  const programFiles =
    process.env.ProgramFiles ||
    "C:\\Program Files";

  const programFilesX86 =
    process.env["ProgramFiles(x86)"] ||
    "C:\\Program Files (x86)";

  if (process.platform === "win32") {
    return [
      path.join(programFiles, "Java"),
      path.join(programFiles, "Eclipse Adoptium"),
      path.join(programFiles, "Microsoft"),
      path.join(programFiles, "Zulu"),
      path.join(programFiles, "Amazon Corretto"),
      path.join(programFiles, "BellSoft"),
      path.join(programFiles, "OpenJDK"),
      path.join(programFilesX86, "Java"),
      path.join(
        localAppData,
        "Programs",
        "Eclipse Adoptium",
      ),
      path.join(
        localAppData,
        "Programs",
        "Microsoft",
      ),
    ];
  }

  if (process.platform === "darwin") {
    return [
      "/Library/Java/JavaVirtualMachines",
      path.join(
        home,
        "Library",
        "Java",
        "JavaVirtualMachines",
      ),
    ];
  }

  return [
    "/usr/lib/jvm",
    "/usr/java",
    path.join(
      home,
      ".sdkman",
      "candidates",
      "java",
    ),
  ];
}

function detectJdk() {
  // 1. JAVA_HOME has priority.
  const javaHome = process.env.JAVA_HOME?.trim();

  if (javaHome) {
    const resolved = path.resolve(javaHome);

    if (isJdk(resolved)) {
      return {
        found: true,
        source: "JAVA_HOME",
        path: resolved,
      };
    }
  }

  // 2. Search common installation locations.
  const commonRoots = getCommonRoots();

  for (const root of commonRoots) {
    const found = findInRoot(root);

    if (found) {
      return {
        found: true,
        source: "common path",
        path: found,
      };
    }
  }

  return {
    found: false,
    source: null,
    path: null,
  };
}

const result = detectJdk();

fs.mkdirSync(outputDir, {
  recursive: true,
});

// Always generate valid TypeScript.
const generatedContent =
  "export const jdkStatus = " +
  JSON.stringify(result, null, 2) +
  " as const;\n";

fs.writeFileSync(
  outputFile,
  generatedContent,
  "utf8",
);

if (result.found) {
  console.log(
    `[JDK] Found via ${result.source}: ${result.path}`,
  );
} else {
  console.warn(
    "[JDK] JDK not found. Set JAVA_HOME or install a JDK.",
  );
}