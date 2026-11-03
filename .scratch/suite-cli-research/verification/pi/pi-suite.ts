import { readFileSync, appendFileSync } from "node:fs";
import { createMcpExtension, createCodemodeExtension } from "@earendil-works/pi-coding-agent";

// Prototype transport contract: launcher sets an absolute, per-process file path.
// Environment exists before extension factories; custom CLI flags need not exist yet.
export default function piSuite(pi) {
  const path = process.env.PI_SUITE_CONFIG;
  const record = (value) => appendFileSync(process.env.PI_SUITE_OBSERVATIONS, JSON.stringify(value) + "\n");
  record({ phase: "factory", suite: path ?? null });
  if (!path) return;
  const suite = JSON.parse(readFileSync(path, "utf8"));
  createMcpExtension({ loadConfig: () => ({ servers: suite.servers.map(entry => ({ ...entry, source: path, scope: "extension" })), errors: [], autoEnableCodemode: true }) })(pi);
  createCodemodeExtension({ mode: suite.codemode_mode })(pi);
  pi.on("session_start", () => {
    const active = pi.getActiveTools();
    if (!active.includes("codemode")) pi.setActiveTools([...active, "codemode"]);
    record({ phase: "session_start", active: pi.getActiveTools() });
  });
}
