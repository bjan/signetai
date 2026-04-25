// @ts-nocheck
import { afterEach, describe, expect, it } from "bun:test";
import { desktopApiBase, isDesktopShell } from "./desktop-shell";

const originalWindow = globalThis.window;

afterEach(() => {
	if (originalWindow === undefined) {
		delete globalThis.window;
	} else {
		globalThis.window = originalWindow;
	}
});

describe("desktop shell detection", () => {
	it("treats the Electron app protocol as desktop even before bridge access", () => {
		globalThis.window = { location: { protocol: "app:" } };

		expect(isDesktopShell()).toBe(true);
		expect(desktopApiBase()).toBe("");
	});

	it("uses the preload bridge daemon base when available", () => {
		globalThis.window = {
			location: { protocol: "app:" },
			signetDesktop: { daemonPort: 3850, daemonBaseUrl: "http://localhost:3850" },
		};

		expect(isDesktopShell()).toBe(true);
		expect(desktopApiBase()).toBe("http://localhost:3850");
	});
});
