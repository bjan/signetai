import { afterEach, describe, expect, it, mock } from "bun:test";
import {
	createAgentMessage,
	listAgentMessages,
	listAgentPresence,
	relayMessageViaAcp,
	removeAgentPresence,
	resetCrossAgentStateForTest,
	subscribeCrossAgentEvents,
	touchAgentPresence,
	upsertAgentPresence,
} from "./cross-agent";

afterEach(() => {
	resetCrossAgentStateForTest();
});

describe("cross-agent presence", () => {
	it("upserts and lists peer sessions while excluding self", () => {
		upsertAgentPresence({
			sessionKey: "sess-a",
			agentId: "alpha",
			harness: "openclaw",
			project: "/repo",
		});
		upsertAgentPresence({
			sessionKey: "sess-b",
			agentId: "beta",
			harness: "opencode",
			project: "/repo",
		});

		const peers = listAgentPresence({
			agentId: "alpha",
			sessionKey: "sess-a",
			includeSelf: false,
		});

		expect(peers.length).toBe(1);
		expect(peers[0]?.agentId).toBe("beta");
		expect(peers[0]?.sessionKey).toBe("sess-b");
	});

	it("touches and removes session presence", () => {
		upsertAgentPresence({
			sessionKey: "sess-x",
			agentId: "alpha",
			harness: "openclaw",
		});

		const touched = touchAgentPresence("sess-x");
		expect(touched).not.toBeNull();
		expect(touched?.agentId).toBe("alpha");

		const removed = removeAgentPresence("sess-x");
		expect(removed).toBe(true);
		expect(listAgentPresence().length).toBe(0);
	});

	it("keeps ephemeral presence keys distinct when fields include colons", () => {
		upsertAgentPresence({
			agentId: "foo:bar",
			harness: "baz",
			project: "/repo",
		});
		upsertAgentPresence({
			agentId: "foo",
			harness: "bar:baz",
			project: "/repo",
		});

		expect(listAgentPresence().length).toBe(2);
	});
});

describe("cross-agent messages", () => {
	it("stores direct messages and lists inbox for recipient", () => {
		upsertAgentPresence({
			sessionKey: "sess-a",
			agentId: "alpha",
			harness: "openclaw",
		});
		upsertAgentPresence({
			sessionKey: "sess-b",
			agentId: "beta",
			harness: "opencode",
		});

		createAgentMessage({
			fromAgentId: "alpha",
			fromSessionKey: "sess-a",
			toAgentId: "beta",
			content: "Need help with migration rollout plan.",
			type: "assist_request",
		});

		const inbox = listAgentMessages({ agentId: "beta" });
		expect(inbox.length).toBe(1);
		expect(inbox[0]?.type).toBe("assist_request");
		expect(inbox[0]?.content).toContain("migration rollout");
	});

	it("includes broadcast messages in recipient inbox", () => {
		createAgentMessage({
			fromAgentId: "alpha",
			content: "CI is currently red on main.",
			broadcast: true,
			type: "decision_update",
		});

		const inbox = listAgentMessages({ agentId: "beta" });
		expect(inbox.length).toBe(1);
		expect(inbox[0]?.broadcast).toBe(true);
	});

	it("allows ACP messages without local target identifiers", () => {
		const message = createAgentMessage({
			fromAgentId: "alpha",
			content: "Escalating to external ACP helper.",
			deliveryPath: "acp",
			deliveryStatus: "queued",
		});

		expect(message.deliveryPath).toBe("acp");
		expect(message.toAgentId).toBeUndefined();
		expect(message.toSessionKey).toBeUndefined();
	});

	it("still requires target for local delivery", () => {
		expect(() =>
			createAgentMessage({
				fromAgentId: "alpha",
				content: "No recipient should fail.",
				deliveryPath: "local",
			}),
		).toThrow("target required for local delivery");
	});

	it("deep-clones delivery receipts to avoid external mutation", () => {
		const message = createAgentMessage({
			fromAgentId: "alpha",
			toAgentId: "beta",
			content: "Receipt immutability check.",
			deliveryReceipt: {
				status: 200,
				nested: { ok: true },
			},
		});

		if (message.deliveryReceipt) {
			message.deliveryReceipt.status = 999;
			message.deliveryReceipt.nested = { ok: false };
		}

		const inbox = listAgentMessages({ agentId: "beta" });
		expect(inbox[0]?.deliveryReceipt).toEqual({
			status: 200,
			nested: { ok: true },
		});
	});

	it("emits events for presence and messages", () => {
		const seen: string[] = [];
		const unsubscribe = subscribeCrossAgentEvents((event) => {
			seen.push(event.type);
		});

		upsertAgentPresence({
			sessionKey: "sess-a",
			agentId: "alpha",
			harness: "openclaw",
		});
		createAgentMessage({
			fromAgentId: "alpha",
			toAgentId: "beta",
			content: "status?",
		});

		unsubscribe();
		expect(seen).toEqual(["presence", "message"]);
	});

	it("keeps session-targeted routing stable if session key is later reused", () => {
		upsertAgentPresence({
			sessionKey: "sess-reused",
			agentId: "beta",
			harness: "openclaw",
		});
		createAgentMessage({
			fromAgentId: "alpha",
			toSessionKey: "sess-reused",
			content: "This message should stay with beta.",
		});

		removeAgentPresence("sess-reused");
		upsertAgentPresence({
			sessionKey: "sess-reused",
			agentId: "gamma",
			harness: "opencode",
		});

		const betaInbox = listAgentMessages({ agentId: "beta" });
		const gammaInbox = listAgentMessages({ agentId: "gamma" });
		expect(betaInbox.length).toBe(1);
		expect(gammaInbox.length).toBe(0);
	});
});

describe("ACP relay", () => {
	it("posts a run request and returns run id", async () => {
		const originalFetch = globalThis.fetch;
		const capture: { url?: string; body?: string } = {};

		globalThis.fetch = mock(async (input: string | URL | Request, init?: RequestInit) => {
			capture.url = typeof input === "string" ? input : input.toString();
			capture.body = typeof init?.body === "string" ? init.body : "";
			return new Response(JSON.stringify({ run_id: "run-123", status: "running" }), {
				status: 201,
				headers: { "Content-Type": "application/json" },
			});
		}) as unknown as typeof fetch;

		const result = await relayMessageViaAcp({
			baseUrl: "https://acp.example.com/",
			targetAgentName: "helper-agent",
			content: "Can you verify this deployment plan?",
			fromAgentId: "alpha",
			fromSessionKey: "sess-a",
		});

		globalThis.fetch = originalFetch;

		expect(capture.url).toBe("https://acp.example.com/runs");
		const body = JSON.parse(capture.body ?? "{}");
		expect(body.agent_name).toBe("helper-agent");
		expect(body.input?.[0]?.parts?.[0]?.content).toContain("deployment plan");
		expect(result.ok).toBe(true);
		expect(result.runId).toBe("run-123");
	});

	it("rejects private ACP origins unless allowlisted", async () => {
		const result = await relayMessageViaAcp({
			baseUrl: "http://localhost:9000/",
			targetAgentName: "helper-agent",
			content: "test",
		});

		expect(result.ok).toBe(false);
		expect(result.error).toContain("allowlisted");
	});

	it("allows allowlisted private ACP origins", async () => {
		const originalFetch = globalThis.fetch;
		const originalAllowlist = process.env.SIGNET_ACP_ALLOWED_ORIGINS;
		const capture: { url?: string } = {};
		process.env.SIGNET_ACP_ALLOWED_ORIGINS = "http://localhost:9000";
		globalThis.fetch = mock(async (input: string | URL | Request) => {
			capture.url = typeof input === "string" ? input : input.toString();
			return new Response(JSON.stringify({ run_id: "run-allowlisted" }), {
				status: 200,
				headers: { "Content-Type": "application/json" },
			});
		}) as unknown as typeof fetch;

		let result: Awaited<ReturnType<typeof relayMessageViaAcp>> | null = null;
		try {
			result = await relayMessageViaAcp({
				baseUrl: "http://localhost:9000/",
				targetAgentName: "helper-agent",
				content: "allowlisted",
			});
		} finally {
			globalThis.fetch = originalFetch;
			if (originalAllowlist === undefined) {
				process.env.SIGNET_ACP_ALLOWED_ORIGINS = undefined;
			} else {
				process.env.SIGNET_ACP_ALLOWED_ORIGINS = originalAllowlist;
			}
		}

		expect(result?.ok).toBe(true);
		expect(capture.url).toBe("http://localhost:9000/runs");
	});
});
