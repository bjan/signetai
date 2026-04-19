import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Command } from "commander";
import { registerAgentCommands } from "./agent";

const prevLog = console.log;
const tempDirs: string[] = [];

afterEach(() => {
	console.log = prevLog;
	while (tempDirs.length > 0) {
		const dir = tempDirs.pop();
		if (!dir) continue;
		rmSync(dir, { recursive: true, force: true });
	}
});

describe("registerAgentCommands", () => {
	test("agent add writes canonical nested roster memory config", async () => {
		const dir = mkdtempSync(join(tmpdir(), "signet-agent-command-"));
		tempDirs.push(dir);
		console.log = () => {};

		const program = new Command();
		registerAgentCommands(program, {
			AGENTS_DIR: dir,
			fetchFromDaemon: async () => null,
		});

		await program.parseAsync(["node", "test", "agent", "add", "writer", "--memory", "group", "--group", "writers"]);

		const yaml = readFileSync(join(dir, "agent.yaml"), "utf-8");
		expect(yaml).toContain("- name: writer");
		expect(yaml).toContain("type: group");
		expect(yaml).toContain("group: writers");
		expect(existsSync(join(dir, "agents", "writer", "SOUL.md"))).toBe(true);
		expect(existsSync(join(dir, "agents", "writer", "IDENTITY.md"))).toBe(true);
	});

	test("agent list reads canonical nested roster memory config while daemon is offline", async () => {
		const dir = mkdtempSync(join(tmpdir(), "signet-agent-command-"));
		tempDirs.push(dir);
		console.log = () => {};

		const addProgram = new Command();
		registerAgentCommands(addProgram, {
			AGENTS_DIR: dir,
			fetchFromDaemon: async () => null,
		});
		await addProgram.parseAsync(["node", "test", "agent", "add", "writer", "--memory", "group", "--group", "writers"]);

		const lines: string[] = [];
		console.log = (line?: unknown) => {
			lines.push(String(line ?? ""));
		};
		const listProgram = new Command();
		registerAgentCommands(listProgram, {
			AGENTS_DIR: dir,
			fetchFromDaemon: async () => null,
		});

		await listProgram.parseAsync(["node", "test", "agent", "list"]);

		expect(lines.some((line) => line.includes("Daemon offline"))).toBe(true);
		expect(lines.some((line) => line.includes("writer") && line.includes("group") && line.includes("writers"))).toBe(
			true,
		);
	});
});
