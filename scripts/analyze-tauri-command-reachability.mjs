import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import ts from "typescript";

const SOURCE_EXTENSIONS = new Set([".ts", ".js", ".svelte"]);

// A command belongs here only when it is intentionally callable outside the
// frontend. Keep the list small and pair every entry with a concrete reason.
export const TAURI_COMMAND_REACHABILITY_ALLOWLIST = new Map();

function matchingParen(source, openIndex) {
  let depth = 0;
  for (let index = openIndex; index < source.length; index += 1) {
    if (source[index] === "(") depth += 1;
    if (source[index] === ")") depth -= 1;
    if (depth === 0) return index;
  }
  throw new Error("Registry-ul Tauri conține o listă de comenzi neterminată.");
}

export function parseTauriCommandRegistry(source) {
  const macroStart = source.indexOf("macro_rules! pana_tauri_commands");
  if (macroStart < 0) throw new Error("Nu am găsit macro-ul canonic pana_tauri_commands.");
  const invocationStart = source.indexOf("$consumer!(", macroStart);
  if (invocationStart < 0) throw new Error("Nu am găsit lista canonică $consumer!(...).");
  const openIndex = source.indexOf("(", invocationStart);
  const body = source.slice(openIndex + 1, matchingParen(source, openIndex));
  const commands = body
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
  for (const command of commands) {
    if (!/^[a-z][a-z0-9_]*$/.test(command)) {
      throw new Error(`Identificator Tauri invalid în registry: ${command}`);
    }
  }
  return commands;
}

function scriptSources(source, fileName) {
  if (!fileName.endsWith(".svelte")) return [source];
  return [...source.matchAll(/<script(?:\s[^>]*)?>([\s\S]*?)<\/script>/g)].map(
    (match) => match[1],
  );
}

export function collectCommandLiteralsFromSource(source, commands, fileName = "source.ts") {
  const knownCommands = commands instanceof Set ? commands : new Set(commands);
  const found = new Set();
  for (const script of scriptSources(source, fileName)) {
    const tree = ts.createSourceFile(
      fileName,
      script,
      ts.ScriptTarget.Latest,
      true,
      fileName.endsWith(".js") ? ts.ScriptKind.JS : ts.ScriptKind.TS,
    );
    const visit = (node) => {
      if (ts.isCallExpression(node) && node.arguments.length > 0) {
        const firstArgument = node.arguments[0];
        if (
          (ts.isStringLiteral(firstArgument) || ts.isNoSubstitutionTemplateLiteral(firstArgument))
          && knownCommands.has(firstArgument.text)
        ) {
          found.add(firstArgument.text);
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(tree);
  }
  return found;
}

function sourceFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return sourceFiles(entryPath);
    return SOURCE_EXTENSIONS.has(path.extname(entry.name)) ? [entryPath] : [];
  });
}

export function collectFrontendCommandLiterals(files, commands) {
  const found = new Set();
  for (const file of files) {
    const source = readFileSync(file, "utf8");
    for (const command of collectCommandLiteralsFromSource(source, commands, file)) {
      found.add(command);
    }
  }
  return found;
}

export function findUnreachableTauriCommands(commands, frontendCommands, allowlist = new Map()) {
  const registered = new Set(commands);
  for (const [command, reason] of allowlist) {
    if (!registered.has(command)) throw new Error(`Allowlist Tauri depășit: ${command} nu este în registry.`);
    if (typeof reason !== "string" || reason.trim().length < 12) {
      throw new Error(`Allowlist Tauri nejustificat: ${command}.`);
    }
  }
  return commands.filter((command) => !frontendCommands.has(command) && !allowlist.has(command));
}

export function analyzeTauriCommandReachability({
  registryPath = path.resolve("src-tauri/src/tauri_command_registry.rs"),
  sourceRoot = path.resolve("src"),
  allowlist = TAURI_COMMAND_REACHABILITY_ALLOWLIST,
} = {}) {
  const commands = parseTauriCommandRegistry(readFileSync(registryPath, "utf8"));
  const frontendCommands = collectFrontendCommandLiterals(sourceFiles(sourceRoot), commands);
  return {
    registeredCount: commands.length,
    consumedCount: frontendCommands.size,
    allowlisted: [...allowlist].map(([command, reason]) => ({ command, reason })),
    unreachable: findUnreachableTauriCommands(commands, frontendCommands, allowlist),
  };
}

function main() {
  const report = analyzeTauriCommandReachability();
  console.log(JSON.stringify(report, null, 2));
  if (report.unreachable.length > 0) process.exitCode = 1;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
