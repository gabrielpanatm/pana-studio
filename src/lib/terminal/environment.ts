const appImageEnvironmentVariables = [
  "APPDIR",
  "APPIMAGE",
  "ARGV0",
  "LD_LIBRARY_PATH",
  "PYTHONHOME",
  "PYTHONPATH",
];

export const terminalShell = "/bin/bash";
export const terminalShellLauncher = "/usr/bin/env";

export function createTerminalShellArgs() {
  return [...appImageEnvironmentVariables.flatMap((name) => ["-u", name]), terminalShell, "-i"];
}

export function createTerminalEnvironment() {
  return {
    COLORTERM: "truecolor",
    SHELL: terminalShell,
    TERM: "xterm-256color",
  };
}
