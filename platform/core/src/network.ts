export const NETWORK_MODES = ["localhost", "tailscale"] as const;
export type NetworkMode = (typeof NETWORK_MODES)[number];

const LOCAL_BINDS = new Set(["127.0.0.1", "localhost", "::1", "::ffff:127.0.0.1"]);

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}

export function normalizeNetworkMode(value: unknown): NetworkMode | null {
	return value === "localhost" || value === "tailscale" ? value : null;
}

export function readNetworkMode(raw: unknown): NetworkMode {
	if (!isRecord(raw)) return "localhost";
	if (!isRecord(raw.network)) return "localhost";
	return normalizeNetworkMode(raw.network.mode) ?? "localhost";
}

export function networkModeFromBindHost(bind: string): NetworkMode {
	return LOCAL_BINDS.has(bind.trim().toLowerCase()) ? "localhost" : "tailscale";
}

export function resolveNetworkBinding(mode: NetworkMode): {
	readonly host: string;
	readonly bind: string;
} {
	if (mode === "tailscale") {
		return {
			host: "127.0.0.1",
			bind: "0.0.0.0",
		};
	}

	return {
		host: "127.0.0.1",
		bind: "127.0.0.1",
	};
}
