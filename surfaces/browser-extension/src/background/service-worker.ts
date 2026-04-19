/**
 * Signet background service worker
 * Handles: context menu, health polling, badge updates, message routing
 */

import { checkHealth } from "../shared/api.js";
import { getConfig } from "../shared/config.js";

// --- Context Menu ---

chrome.runtime.onInstalled.addListener(() => {
	chrome.contextMenus.create({
		id: "signet-remember",
		title: "Remember with Signet",
		contexts: ["selection"],
	});
});

chrome.contextMenus.onClicked.addListener((info, tab) => {
	if (info.menuItemId !== "signet-remember") return;
	if (!tab?.id || !info.selectionText) return;

	chrome.tabs.sendMessage(tab.id, {
		action: "show-save-panel",
		text: info.selectionText,
		pageUrl: info.pageUrl ?? tab.url ?? "",
		pageTitle: tab.title ?? "",
	});
});

// --- Keyboard shortcut ---

chrome.commands.onCommand.addListener((command) => {
	if (command !== "save-selection") return;

	chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
		const tab = tabs[0];
		if (!tab?.id) return;

		chrome.tabs.sendMessage(tab.id, {
			action: "trigger-save-shortcut",
		});
	});
});

// --- Health polling & badge ---

type HealthState = "healthy" | "degraded" | "offline";

const BADGE_COLORS: Record<HealthState, string> = {
	healthy: "#4a7a5e",
	degraded: "#8a7a4a",
	offline: "#8a4a48",
};

const BADGE_TEXT: Record<HealthState, string> = {
	healthy: "",
	degraded: "!",
	offline: "X",
};

let currentState: HealthState = "offline";

async function pollHealth(): Promise<void> {
	const health = await checkHealth();
	let newState: HealthState;

	if (health === null) {
		newState = "offline";
	} else if (health.status === "ok" || health.status === "healthy") {
		newState = "healthy";
	} else {
		newState = "degraded";
	}

	if (newState !== currentState) {
		currentState = newState;
		chrome.action.setBadgeBackgroundColor({ color: BADGE_COLORS[newState] });
		chrome.action.setBadgeText({ text: BADGE_TEXT[newState] });
	}
}

// Poll every 60 seconds
chrome.alarms.create("signet-health-poll", { periodInMinutes: 1 });

chrome.alarms.onAlarm.addListener((alarm) => {
	if (alarm.name === "signet-health-poll") {
		pollHealth();
	}
});

// Initial poll on startup
pollHealth();

// --- Message routing ---

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
	if (message.action === "get-health") {
		pollHealth().then(() => {
			sendResponse({ state: currentState });
		});
		return true;
	}

	if (message.action === "get-daemon-url") {
		getConfig().then((config) => {
			sendResponse({ url: config.daemonUrl });
		});
		return true;
	}

	return false;
});
