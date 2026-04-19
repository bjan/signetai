import { spawnSync } from "node:child_process";
import { cpSync, existsSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

function fail(msg: string): never {
	console.error(msg);
	process.exit(1);
}

function run(cmd: string, args: readonly string[], cwd: string): void {
	const res = spawnSync(cmd, args, {
		cwd,
		stdio: "inherit",
		env: {
			...process.env,
			TERM: process.env.TERM || "xterm",
		},
	});
	if (res.status === 0) return;
	fail(`[dashboard-bundle] ${cmd} ${args.join(" ")} failed in ${cwd}`);
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const root = resolve(__dirname, "..");
const targetArg = process.argv[2];

if (!targetArg) {
	fail("[dashboard-bundle] target package path is required");
}

const src = join(root, "surfaces", "dashboard");
const build = join(src, "build");
const target = resolve(process.cwd(), targetArg);
const out = join(target, "dashboard");

run("bun", ["run", "build"], src);

if (!existsSync(join(build, "index.html"))) {
	fail(`[dashboard-bundle] expected ${join(build, "index.html")} after dashboard build`);
}

rmSync(out, { recursive: true, force: true });
cpSync(build, out, { recursive: true });

if (!existsSync(join(out, "index.html"))) {
	fail(`[dashboard-bundle] expected ${join(out, "index.html")} after copy`);
}
