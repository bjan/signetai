/**
 * @signet/core - YAML utilities
 */

import YAML from "yaml";

/**
 * Parse a YAML string into a JavaScript object.
 *
 * Malformed user-owned YAML should degrade to an empty object instead of
 * propagating parser exceptions into daemon or CLI startup.
 */
export function parseSimpleYaml(text: string): Record<string, unknown> {
	try {
		const parsed = YAML.parse(text);
		return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>) : {};
	} catch {
		return {};
	}
}

/**
 * Format a JavaScript object as YAML.
 *
 * `_indent` is retained for internal call-site compatibility, but the
 * shared YAML library always emits 2-space indentation here.
 */
export function formatYaml(obj: Record<string, unknown>, _indent = 0): string {
	return YAML.stringify(obj, {
		indent: 2,
		simpleKeys: true,
	});
}
