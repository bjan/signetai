/**
 * Widget-generation LlmProvider singleton.
 *
 * Separate from extraction/synthesis providers because widget generation
 * needs HTML output with design-token awareness, which may require a
 * different model configuration.
 */

import type { LlmProvider } from "@signet/core";
import { logger } from "./logger";

let provider: LlmProvider | null = null;

export function initWidgetProvider(instance: LlmProvider): void {
	if (provider) {
		logger.warn("widget", "Widget provider already initialised, skipping");
		return;
	}
	provider = instance;
}

export function getWidgetProvider(): LlmProvider {
	if (!provider) {
		throw new Error("Widget LlmProvider not initialised — call initWidgetProvider() first");
	}
	return provider;
}

export function closeWidgetProvider(): void {
	provider = null;
}
