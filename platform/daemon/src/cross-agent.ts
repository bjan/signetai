import { randomUUID } from "node:crypto";
import { isIP } from "node:net";

export type AgentMessageType = "assist_request" | "decision_update" | "info" | "question";

export type AgentMessageDeliveryPath = "local" | "acp";
export type AgentMessageDeliveryStatus = "queued" | "delivered" | "failed";

export interface AgentPresence {
	readonly key: string;
	readonly sessionKey?: string;
	readonly agentId: string;
	readonly harness: string;
	readonly project?: string;
	readonly runtimePath?: "plugin" | "legacy";
	readonly provider?: string;
	readonly startedAt: string;
	readonly lastSeenAt: string;
}

export interface UpsertAgentPresenceInput {
	readonly sessionKey?: string;
	readonly agentId?: string;
	readonly harness: string;
	readonly project?: string;
	readonly runtimePath?: "plugin" | "legacy";
	readonly provider?: string;
}

export interface ListAgentPresenceOptions {
	readonly agentId?: string;
	readonly includeSelf?: boolean;
	readonly sessionKey?: string;
	readonly project?: string;
	readonly limit?: number;
}

interface MutableAgentPresence {
	key: string;
	sessionKey?: string;
	agentId: string;
	harness: string;
	project?: string;
	runtimePath?: "plugin" | "legacy";
	provider?: string;
	startedAt: string;
	lastSeenAt: string;
}

export interface AgentMessage {
	readonly id: string;
	readonly createdAt: string;
	readonly fromAgentId: string;
	readonly fromSessionKey?: string;
	readonly toAgentId?: string;
	readonly toSessionKey?: string;
	readonly toSessionAgentId?: string;
	readonly content: string;
	readonly type: AgentMessageType;
	readonly broadcast: boolean;
	readonly deliveryPath: AgentMessageDeliveryPath;
	readonly deliveryStatus: AgentMessageDeliveryStatus;
	readonly deliveryError?: string;
	readonly deliveryReceipt?: Record<string, unknown>;
}

interface MutableAgentMessage {
	id: string;
	createdAt: string;
	fromAgentId: string;
	fromSessionKey?: string;
	toAgentId?: string;
	toSessionKey?: string;
	toSessionAgentId?: string;
	content: string;
	type: AgentMessageType;
	broadcast: boolean;
	deliveryPath: AgentMessageDeliveryPath;
	deliveryStatus: AgentMessageDeliveryStatus;
	deliveryError?: string;
	deliveryReceipt?: Record<string, unknown>;
}

export interface CreateAgentMessageInput {
	readonly fromAgentId?: string;
	readonly fromSessionKey?: string;
	readonly toAgentId?: string;
	readonly toSessionKey?: string;
	readonly content: string;
	readonly type?: AgentMessageType;
	readonly broadcast?: boolean;
	readonly deliveryPath?: AgentMessageDeliveryPath;
	readonly deliveryStatus?: AgentMessageDeliveryStatus;
	readonly deliveryError?: string;
	readonly deliveryReceipt?: Record<string, unknown>;
}

export interface ListAgentMessageOptions {
	readonly agentId?: string;
	readonly sessionKey?: string;
	readonly since?: string;
	readonly includeSent?: boolean;
	readonly includeBroadcast?: boolean;
	readonly limit?: number;
}

export interface AgentPresenceEvent {
	readonly type: "presence";
	readonly action: "upsert" | "remove";
	readonly presence: AgentPresence;
	readonly activeCount: number;
	readonly timestamp: string;
}

export interface AgentMessageEvent {
	readonly type: "message";
	readonly message: AgentMessage;
	readonly timestamp: string;
}

export type CrossAgentEvent = AgentPresenceEvent | AgentMessageEvent;

export interface AcpRelayRequest {
	readonly baseUrl: string;
	readonly targetAgentName: string;
	readonly content: string;
	readonly fromAgentId?: string;
	readonly fromSessionKey?: string;
	readonly timeoutMs?: number;
	readonly metadata?: Record<string, unknown>;
}

export interface AcpRelayResult {
	readonly ok: boolean;
	readonly status: number;
	readonly runId?: string;
	readonly error?: string;
}

const PRESENCE_STALE_MS = 4 * 60 * 60 * 1000;
const MESSAGE_RETENTION_MS = 7 * 24 * 60 * 60 * 1000;
const MAX_MESSAGES = 1000;
const ACP_TIMEOUT_MS = 20_000;
const ACP_ALLOWED_ORIGINS_ENV = "SIGNET_ACP_ALLOWED_ORIGINS";

const presenceByKey = new Map<string, MutableAgentPresence>();
const messages: MutableAgentMessage[] = [];
const subscribers = new Set<(event: CrossAgentEvent) => void>();

function normalizeText(value: string | undefined): string | undefined {
	if (!value) return undefined;
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function cloneDeliveryReceipt(value: Record<string, unknown> | undefined): Record<string, unknown> | undefined {
	if (!value) return undefined;

	try {
		if (typeof globalThis.structuredClone === "function") {
			const cloned = globalThis.structuredClone(value);
			return isRecord(cloned) ? cloned : undefined;
		}

		const cloned = JSON.parse(JSON.stringify(value));
		return isRecord(cloned) ? cloned : undefined;
	} catch {
		return undefined;
	}
}

function presenceKey(input: UpsertAgentPresenceInput): string {
	const sessionKey = normalizeText(input.sessionKey);
	if (sessionKey) return `session:${sessionKey}`;

	const agentId = normalizeText(input.agentId) ?? "default";
	const harness = normalizeText(input.harness) ?? "unknown";
	const project = normalizeText(input.project) ?? "*";
	return `ephemeral:${encodeURIComponent(agentId)}:${encodeURIComponent(harness)}:${encodeURIComponent(project)}`;
}

function clonePresence(presence: MutableAgentPresence): AgentPresence {
	return {
		key: presence.key,
		sessionKey: presence.sessionKey,
		agentId: presence.agentId,
		harness: presence.harness,
		project: presence.project,
		runtimePath: presence.runtimePath,
		provider: presence.provider,
		startedAt: presence.startedAt,
		lastSeenAt: presence.lastSeenAt,
	};
}

function cloneMessage(message: MutableAgentMessage): AgentMessage {
	return {
		id: message.id,
		createdAt: message.createdAt,
		fromAgentId: message.fromAgentId,
		fromSessionKey: message.fromSessionKey,
		toAgentId: message.toAgentId,
		toSessionKey: message.toSessionKey,
		toSessionAgentId: message.toSessionAgentId,
		content: message.content,
		type: message.type,
		broadcast: message.broadcast,
		deliveryPath: message.deliveryPath,
		deliveryStatus: message.deliveryStatus,
		deliveryError: message.deliveryError,
		deliveryReceipt: cloneDeliveryReceipt(message.deliveryReceipt),
	};
}

function emit(event: CrossAgentEvent): void {
	for (const subscriber of subscribers) {
		try {
			subscriber(event);
		} catch {
			// Subscribers are external; a faulty subscriber must not block others.
		}
	}
}

function parseIsoTimestamp(value: string | undefined): number | null {
	if (!value) return null;
	const parsed = Date.parse(value);
	return Number.isFinite(parsed) ? parsed : null;
}

function pruneState(nowMs = Date.now()): void {
	const nowIso = new Date(nowMs).toISOString();
	for (const [key, presence] of presenceByKey.entries()) {
		const seenAt = parseIsoTimestamp(presence.lastSeenAt);
		if (seenAt === null) {
			presenceByKey.delete(key);
			emit({
				type: "presence",
				action: "remove",
				presence: clonePresence(presence),
				activeCount: presenceByKey.size,
				timestamp: nowIso,
			});
			continue;
		}
		if (nowMs - seenAt > PRESENCE_STALE_MS) {
			presenceByKey.delete(key);
			emit({
				type: "presence",
				action: "remove",
				presence: clonePresence(presence),
				activeCount: presenceByKey.size,
				timestamp: nowIso,
			});
		}
	}

	const minCreatedAt = nowMs - MESSAGE_RETENTION_MS;
	let retainedFrom = 0;
	for (let i = 0; i < messages.length; i++) {
		const createdAt = parseIsoTimestamp(messages[i]?.createdAt);
		if (createdAt !== null && createdAt >= minCreatedAt) {
			retainedFrom = i;
			break;
		}
		retainedFrom = i + 1;
	}
	if (retainedFrom > 0) {
		messages.splice(0, retainedFrom);
	}

	if (messages.length > MAX_MESSAGES) {
		messages.splice(0, messages.length - MAX_MESSAGES);
	}
}

function agentForSession(sessionKey: string): string | undefined {
	const presence = presenceByKey.get(`session:${sessionKey}`);
	return presence?.agentId;
}

export function getAgentPresenceForSession(sessionKey: string): AgentPresence | null {
	const normalized = normalizeText(sessionKey);
	if (!normalized) return null;
	pruneState();
	const presence = presenceByKey.get(`session:${normalized}`);
	return presence ? clonePresence(presence) : null;
}

function includesMessageForAgent(
	message: MutableAgentMessage,
	agentId: string | undefined,
	sessionKey: string | undefined,
	includeBroadcast: boolean,
): boolean {
	if (sessionKey && message.toSessionKey === sessionKey) {
		return true;
	}

	if (agentId && message.toAgentId === agentId) {
		return true;
	}

	if (agentId && message.toSessionAgentId === agentId) {
		return true;
	}

	// Legacy fallback: when no captured recipient owner exists, resolve toSessionKey
	// through the current presence map and treat that owner as authoritative.
	if (agentId && message.toSessionKey && !message.toSessionAgentId) {
		const owner = agentForSession(message.toSessionKey);
		if (owner === agentId) return true;
	}

	if (includeBroadcast && message.broadcast) {
		return true;
	}

	return false;
}

export function isMessageVisibleToAgent(
	message: AgentMessage,
	options: {
		readonly agentId?: string;
		readonly sessionKey?: string;
		readonly includeBroadcast?: boolean;
	},
): boolean {
	const mutableLike: MutableAgentMessage = {
		id: message.id,
		createdAt: message.createdAt,
		fromAgentId: message.fromAgentId,
		fromSessionKey: message.fromSessionKey,
		toAgentId: message.toAgentId,
		toSessionKey: message.toSessionKey,
		toSessionAgentId: message.toSessionAgentId,
		content: message.content,
		type: message.type,
		broadcast: message.broadcast,
		deliveryPath: message.deliveryPath,
		deliveryStatus: message.deliveryStatus,
		deliveryError: message.deliveryError,
		deliveryReceipt: message.deliveryReceipt,
	};
	return includesMessageForAgent(
		mutableLike,
		normalizeText(options.agentId),
		normalizeText(options.sessionKey),
		options.includeBroadcast !== false,
	);
}

export function upsertAgentPresence(input: UpsertAgentPresenceInput): AgentPresence {
	pruneState();

	const key = presenceKey(input);
	const now = new Date().toISOString();
	const sessionKey = normalizeText(input.sessionKey);
	const agentId = normalizeText(input.agentId) ?? "default";
	const harness = normalizeText(input.harness) ?? "unknown";
	const project = normalizeText(input.project);
	const provider = normalizeText(input.provider);

	const existing = presenceByKey.get(key);
	if (existing) {
		existing.sessionKey = sessionKey;
		existing.agentId = agentId;
		existing.harness = harness;
		existing.project = project;
		existing.runtimePath = input.runtimePath;
		existing.provider = provider;
		existing.lastSeenAt = now;

		const out = clonePresence(existing);
		emit({
			type: "presence",
			action: "upsert",
			presence: out,
			activeCount: presenceByKey.size,
			timestamp: now,
		});
		return out;
	}

	const created: MutableAgentPresence = {
		key,
		sessionKey,
		agentId,
		harness,
		project,
		runtimePath: input.runtimePath,
		provider,
		startedAt: now,
		lastSeenAt: now,
	};
	presenceByKey.set(key, created);

	const out = clonePresence(created);
	emit({
		type: "presence",
		action: "upsert",
		presence: out,
		activeCount: presenceByKey.size,
		timestamp: now,
	});
	return out;
}

export function touchAgentPresence(sessionKey: string, agentId?: string): AgentPresence | null {
	const normalized = normalizeText(sessionKey);
	if (!normalized) return null;
	pruneState();
	const presence = presenceByKey.get(`session:${normalized}`);
	if (!presence) return null;
	// Guard: if caller provides agentId, verify record ownership before touching.
	if (agentId && presence.agentId !== agentId) return null;
	presence.lastSeenAt = new Date().toISOString();
	return clonePresence(presence);
}

export function removeAgentPresence(sessionKey: string): boolean {
	const normalized = normalizeText(sessionKey);
	if (!normalized) return false;
	pruneState();
	const key = `session:${normalized}`;
	const existing = presenceByKey.get(key);
	if (!existing) return false;

	presenceByKey.delete(key);
	const now = new Date().toISOString();
	emit({
		type: "presence",
		action: "remove",
		presence: clonePresence(existing),
		activeCount: presenceByKey.size,
		timestamp: now,
	});
	return true;
}

export function listAgentPresence(options: ListAgentPresenceOptions = {}): AgentPresence[] {
	pruneState();

	const agentId = normalizeText(options.agentId);
	const sessionKey = normalizeText(options.sessionKey);
	const project = normalizeText(options.project);
	const includeSelf = options.includeSelf !== false;
	const limit = options.limit && options.limit > 0 ? options.limit : 50;

	const items = [...presenceByKey.values()]
		.filter((presence) => {
			if (project && presence.project !== project) return false;
			if (!agentId) return true;
			if (includeSelf) return true;
			// Excluding self keeps only other agents, plus other sessions owned
			// by the same agent when a current sessionKey is provided.
			if (presence.agentId !== agentId) return true;
			if (sessionKey && presence.sessionKey && presence.sessionKey !== sessionKey) return true;
			return false;
		})
		.sort((a, b) => b.lastSeenAt.localeCompare(a.lastSeenAt))
		.slice(0, limit)
		.map(clonePresence);

	return items;
}

export function createAgentMessage(input: CreateAgentMessageInput): AgentMessage {
	pruneState();

	const content = normalizeText(input.content);
	if (!content) {
		throw new Error("content is required");
	}

	const toAgentId = normalizeText(input.toAgentId);
	const toSessionKey = normalizeText(input.toSessionKey);
	const toSessionAgentId = toSessionKey ? (toAgentId ?? agentForSession(toSessionKey)) : undefined;
	const broadcast = input.broadcast === true;
	const deliveryPath = input.deliveryPath ?? "local";
	if (deliveryPath === "local" && !broadcast && !toAgentId && !toSessionKey) {
		throw new Error("target required for local delivery (toAgentId, toSessionKey, or broadcast=true)");
	}

	const now = new Date().toISOString();
	const message: MutableAgentMessage = {
		id: randomUUID(),
		createdAt: now,
		fromAgentId: normalizeText(input.fromAgentId) ?? "default",
		fromSessionKey: normalizeText(input.fromSessionKey),
		toAgentId,
		toSessionKey,
		toSessionAgentId,
		content,
		type: input.type ?? "info",
		broadcast,
		deliveryPath,
		deliveryStatus: input.deliveryStatus ?? "delivered",
		deliveryError: normalizeText(input.deliveryError),
		deliveryReceipt: cloneDeliveryReceipt(input.deliveryReceipt),
	};

	messages.push(message);
	pruneState();

	const out = cloneMessage(message);
	emit({
		type: "message",
		message: out,
		timestamp: now,
	});
	return out;
}

export function listAgentMessages(options: ListAgentMessageOptions = {}): AgentMessage[] {
	pruneState();

	const agentId = normalizeText(options.agentId);
	const sessionKey = normalizeText(options.sessionKey);
	const includeSent = options.includeSent === true;
	const includeBroadcast = options.includeBroadcast !== false;
	const limit = options.limit && options.limit > 0 ? options.limit : 100;

	const sinceMs = parseIsoTimestamp(options.since);

	const filtered = messages.filter((message) => {
		const createdAtMs = parseIsoTimestamp(message.createdAt);
		if (sinceMs !== null && createdAtMs !== null && createdAtMs < sinceMs) {
			return false;
		}

		const isRecipient = includesMessageForAgent(message, agentId, sessionKey, includeBroadcast);
		if (isRecipient) return true;

		if (includeSent && agentId && message.fromAgentId === agentId) {
			return true;
		}

		return false;
	});

	return filtered
		.sort((a, b) => b.createdAt.localeCompare(a.createdAt))
		.slice(0, limit)
		.map(cloneMessage);
}

export function subscribeCrossAgentEvents(subscriber: (event: CrossAgentEvent) => void): () => void {
	subscribers.add(subscriber);
	return () => {
		subscribers.delete(subscriber);
	};
}

function parseAllowedAcpOrigins(raw: string | undefined): ReadonlySet<string> {
	const out = new Set<string>();
	const source = normalizeText(raw);
	if (!source) return out;

	for (const candidate of source.split(",")) {
		const trimmed = normalizeText(candidate);
		if (!trimmed) continue;
		try {
			const parsed = new URL(trimmed);
			if (parsed.protocol !== "http:" && parsed.protocol !== "https:") continue;
			out.add(parsed.origin);
		} catch {
			// Ignore malformed entries and continue with valid origins.
		}
	}

	return out;
}

function parseIpv4Octets(hostname: string): number[] | null {
	const parts = hostname.split(".");
	if (parts.length !== 4) return null;

	const octets: number[] = [];
	for (const part of parts) {
		if (!/^\d+$/.test(part)) return null;
		const value = Number.parseInt(part, 10);
		if (!Number.isInteger(value) || value < 0 || value > 255) return null;
		octets.push(value);
	}
	return octets;
}

function isPrivateIpv4(hostname: string): boolean {
	const octets = parseIpv4Octets(hostname);
	if (!octets) return false;

	const [a, b] = octets;
	if (a === 10) return true;
	if (a === 127) return true;
	if (a === 169 && b === 254) return true;
	if (a === 172 && b >= 16 && b <= 31) return true;
	if (a === 192 && b === 168) return true;
	if (a === 100 && b >= 64 && b <= 127) return true;
	return false;
}

function isPrivateIpv6(hostname: string): boolean {
	const normalized = hostname.toLowerCase();
	if (normalized === "::1" || normalized === "::") return true;
	if (normalized.startsWith("fe80:")) return true;
	if (normalized.startsWith("fc") || normalized.startsWith("fd")) return true;
	if (normalized.startsWith("::ffff:")) {
		const mapped = normalized.slice("::ffff:".length);
		return isPrivateIpv4(mapped);
	}
	return false;
}

function isPrivateOrLocalHostname(hostname: string): boolean {
	const normalized = hostname.toLowerCase();
	if (normalized === "localhost" || normalized.endsWith(".localhost") || normalized.endsWith(".local")) {
		return true;
	}

	const ipType = isIP(normalized);
	if (ipType === 4) return isPrivateIpv4(normalized);
	if (ipType === 6) return isPrivateIpv6(normalized);

	// Single-label hosts are typically local network names.
	return !normalized.includes(".");
}

function resolveAcpRunsUrl(baseUrl: string): { ok: true; url: string } | { ok: false; error: string } {
	let parsed: URL;
	try {
		parsed = new URL(baseUrl);
	} catch {
		return { ok: false, error: "acp.baseUrl must be a valid absolute URL" };
	}

	if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
		return { ok: false, error: "acp.baseUrl must use http or https" };
	}
	if (parsed.username || parsed.password) {
		return { ok: false, error: "acp.baseUrl must not include credentials" };
	}

	const allowlist = parseAllowedAcpOrigins(process.env[ACP_ALLOWED_ORIGINS_ENV]);
	const allowlisted = allowlist.has(parsed.origin);

	if (!allowlisted) {
		if (parsed.protocol !== "https:") {
			return {
				ok: false,
				error: "acp.baseUrl must use https unless explicitly allowlisted via SIGNET_ACP_ALLOWED_ORIGINS",
			};
		}

		if (isPrivateOrLocalHostname(parsed.hostname)) {
			return {
				ok: false,
				error: "acp.baseUrl host is private/local and must be explicitly allowlisted via SIGNET_ACP_ALLOWED_ORIGINS",
			};
		}
	}

	return { ok: true, url: new URL("/runs", parsed).toString() };
}

function readStringField(record: Record<string, unknown>, key: string): string | undefined {
	const value = record[key];
	return typeof value === "string" && value.trim().length > 0 ? value : undefined;
}

function extractRunId(value: unknown): string | undefined {
	if (!isRecord(value)) return undefined;

	const direct = readStringField(value, "run_id") ?? readStringField(value, "runId") ?? readStringField(value, "id");
	if (direct) return direct;

	const nested = value.run;
	if (!isRecord(nested)) return undefined;
	return readStringField(nested, "run_id") ?? readStringField(nested, "runId") ?? readStringField(nested, "id");
}

export async function relayMessageViaAcp(request: AcpRelayRequest): Promise<AcpRelayResult> {
	const baseUrl = normalizeText(request.baseUrl);
	if (!baseUrl) {
		return { ok: false, status: 0, error: "acp.baseUrl is required" };
	}

	const targetAgentName = normalizeText(request.targetAgentName);
	if (!targetAgentName) {
		return { ok: false, status: 0, error: "acp.targetAgentName is required" };
	}

	const content = normalizeText(request.content);
	if (!content) {
		return { ok: false, status: 0, error: "content is required" };
	}

	const timeoutMs = typeof request.timeoutMs === "number" && request.timeoutMs > 0 ? request.timeoutMs : ACP_TIMEOUT_MS;

	const runsUrl = resolveAcpRunsUrl(baseUrl);
	if (!runsUrl.ok) {
		return { ok: false, status: 0, error: runsUrl.error };
	}

	const payload: Record<string, unknown> = {
		agent_name: targetAgentName,
		mode: "sync",
		input: [
			{
				role: "user",
				parts: [
					{
						content_type: "text/plain",
						content,
					},
				],
			},
		],
	};

	const metadata: Record<string, unknown> = {
		from_agent_id: normalizeText(request.fromAgentId) ?? "default",
	};
	const fromSessionKey = normalizeText(request.fromSessionKey);
	if (fromSessionKey) {
		metadata.from_session_key = fromSessionKey;
	}
	if (request.metadata && Object.keys(request.metadata).length > 0) {
		metadata.signet = cloneDeliveryReceipt(request.metadata);
	}
	payload.metadata = metadata;

	try {
		const response = await fetch(runsUrl.url, {
			method: "POST",
			headers: {
				"Content-Type": "application/json",
			},
			body: JSON.stringify(payload),
			signal: AbortSignal.timeout(timeoutMs),
		});

		let parsedBody: unknown = null;
		try {
			parsedBody = await response.json();
		} catch {
			parsedBody = null;
		}

		if (!response.ok) {
			const error =
				isRecord(parsedBody) && typeof parsedBody.error === "string"
					? parsedBody.error
					: `ACP request failed with ${response.status}`;
			return {
				ok: false,
				status: response.status,
				error,
			};
		}

		return {
			ok: true,
			status: response.status,
			runId: extractRunId(parsedBody),
		};
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		return {
			ok: false,
			status: 0,
			error: message,
		};
	}
}

/** Remove all presence entries (for graceful shutdown). Returns count cleared. */
export function clearAllPresence(): number {
	const now = new Date().toISOString();
	let count = 0;
	for (const [, presence] of presenceByKey) {
		emit({
			type: "presence",
			action: "remove",
			presence: clonePresence(presence),
			activeCount: presenceByKey.size - count - 1,
			timestamp: now,
		});
		count++;
	}
	presenceByKey.clear();
	return count;
}

export function resetCrossAgentStateForTest(): void {
	presenceByKey.clear();
	messages.length = 0;
	subscribers.clear();
}
