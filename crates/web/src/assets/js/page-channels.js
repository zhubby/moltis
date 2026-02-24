// ── Channels page (Preact + HTM + Signals) ──────────────────

import { signal, useSignal } from "@preact/signals";
import { html } from "htm/preact";
import { render } from "preact";
import { useEffect } from "preact/hooks";
import { addChannel, fetchChannelStatus, validateChannelFields } from "./channel-utils.js";
import { onEvent } from "./events.js";
import { sendRpc } from "./helpers.js";
import { updateNavCount } from "./nav-counts.js";
import { connected } from "./signals.js";
import * as S from "./state.js";
import { models as modelsSig } from "./stores/model-store.js";
import { ConfirmDialog, Modal, ModelSelect, requestConfirm, showToast } from "./ui.js";

var channels = signal([]);

export function prefetchChannels() {
	fetchChannelStatus().then((res) => {
		if (res?.ok) {
			var ch = res.payload?.channels || [];
			channels.value = ch;
			S.setCachedChannels(ch);
		}
	});
}
var senders = signal([]);
var activeTab = signal("channels");
var showAddModal = signal(false);
var editingChannel = signal(null);
var sendersAccount = signal("");

function channelType(type) {
	return type || "telegram";
}

function channelLabel(type) {
	return channelType(type) === "msteams" ? "Microsoft Teams" : "Telegram";
}

function senderSelectionKey(ch) {
	return `${channelType(ch.type)}::${ch.account_id}`;
}

function parseSenderSelectionKey(key) {
	var idx = key.indexOf("::");
	if (idx < 0) return { type: "telegram", account_id: key };
	return {
		type: key.slice(0, idx) || "telegram",
		account_id: key.slice(idx + 2),
	};
}

function loadChannels() {
	fetchChannelStatus().then((res) => {
		if (res?.ok) {
			var ch = res.payload?.channels || [];
			channels.value = ch;
			S.setCachedChannels(ch);
			updateNavCount("channels", ch.length);
		}
	});
}

function loadSenders() {
	var selected = sendersAccount.value;
	if (!selected) {
		senders.value = [];
		return;
	}
	var parsed = parseSenderSelectionKey(selected);
	sendRpc("channels.senders.list", { type: parsed.type, account_id: parsed.account_id }).then((res) => {
		if (res?.ok) senders.value = res.payload?.senders || [];
	});
}

// ── Channel icon ─────────────────────────────────────────────
function ChannelIcon({ type }) {
	if (channelType(type) === "msteams") {
		return html`<span class="icon icon-chat"></span>`;
	}
	return html`<span class="icon icon-telegram"></span>`;
}

// ── Channel card ─────────────────────────────────────────────
function ChannelCard(props) {
	var ch = props.channel;

	function onRemove() {
		requestConfirm(`Remove ${ch.name || ch.account_id}?`).then((yes) => {
			if (!yes) return;
			sendRpc("channels.remove", { type: channelType(ch.type), account_id: ch.account_id }).then((r) => {
				if (r?.ok) loadChannels();
			});
		});
	}

	var statusClass = ch.status === "connected" ? "configured" : "oauth";
	var sessionLine = "";
	if (ch.sessions && ch.sessions.length > 0) {
		var active = ch.sessions.filter((s) => s.active);
		sessionLine =
			active.length > 0
				? active.map((s) => `${s.label || s.key} (${s.messageCount} msgs)`).join(", ")
				: "No active session";
	}

	return html`<div class="provider-card" style="padding:12px 14px;border-radius:8px;margin-bottom:8px;">
    <div style="display:flex;align-items:center;gap:10px;">
	      <span style="display:inline-flex;align-items:center;justify-content:center;width:28px;height:28px;border-radius:6px;background:var(--surface2);">
	        <${ChannelIcon} type=${ch.type} />
	      </span>
	      <div style="display:flex;flex-direction:column;gap:2px;">
	        <span class="text-sm text-[var(--text-strong)]">${ch.name || ch.account_id || channelLabel(ch.type)}</span>
        ${ch.details && html`<span class="text-xs text-[var(--muted)]">${ch.details}</span>`}
        ${sessionLine && html`<span class="text-xs text-[var(--muted)]">${sessionLine}</span>`}
      </div>
      <span class="provider-item-badge ${statusClass}">${ch.status || "unknown"}</span>
    </div>
    <div class="flex gap-2">
      <button class="provider-btn provider-btn-sm provider-btn-secondary" title="Edit ${ch.account_id || "channel"}"
        onClick=${() => {
					editingChannel.value = ch;
				}}>Edit</button>
      <button class="provider-btn provider-btn-sm provider-btn-danger" title="Remove ${ch.account_id || "channel"}"
        onClick=${onRemove}>Remove</button>
    </div>
  </div>`;
}

// ── Channels tab ─────────────────────────────────────────────
function ChannelsTab() {
	if (channels.value.length === 0) {
		return html`<div style="text-align:center;padding:40px 0;">
	      <div class="text-sm text-[var(--muted)]" style="margin-bottom:12px;">No channels connected.</div>
	      <div class="text-xs text-[var(--muted)]">Click "+ Add Channel" to connect Telegram or Microsoft Teams.</div>
	    </div>`;
	}
	return html`${channels.value.map((ch) => html`<${ChannelCard} key=${senderSelectionKey(ch)} channel=${ch} />`)}`;
}

// ── Senders tab ──────────────────────────────────────────────
function SendersTab() {
	useEffect(() => {
		if (channels.value.length > 0 && !sendersAccount.value) {
			sendersAccount.value = senderSelectionKey(channels.value[0]);
		}
	}, [channels.value]);

	useEffect(() => {
		loadSenders();
	}, [sendersAccount.value]);

	if (channels.value.length === 0) {
		return html`<div class="text-sm text-[var(--muted)]">No channels configured.</div>`;
	}

	function onAction(identifier, action) {
		var rpc = action === "approve" ? "channels.senders.approve" : "channels.senders.deny";
		var parsed = parseSenderSelectionKey(sendersAccount.value);
		sendRpc(rpc, {
			type: parsed.type,
			account_id: parsed.account_id,
			identifier: identifier,
		}).then(() => {
			loadSenders();
			loadChannels();
		});
	}

	return html`<div>
    <div style="margin-bottom:12px;">
      <label class="text-xs text-[var(--muted)]" style="margin-right:6px;">Account:</label>
	      <select style="background:var(--surface2);color:var(--text);border:1px solid var(--border);border-radius:4px;padding:4px 8px;font-size:12px;"
	        value=${sendersAccount.value} onChange=${(e) => {
						sendersAccount.value = e.target.value;
					}}>
	        ${channels.value.map(
						(ch) =>
							html`<option key=${senderSelectionKey(ch)} value=${senderSelectionKey(ch)}>${ch.name || ch.account_id}</option>`,
					)}
	      </select>
    </div>
    ${senders.value.length === 0 && html`<div class="text-sm text-[var(--muted)] senders-empty">No messages received yet for this account.</div>`}
    ${
			senders.value.length > 0 &&
			html`<table class="senders-table">
      <thead><tr>
        <th class="senders-th">Sender</th><th class="senders-th">Username</th>
        <th class="senders-th">Messages</th><th class="senders-th">Last Seen</th>
        <th class="senders-th">Status</th><th class="senders-th">Action</th>
      </tr></thead>
      <tbody>
        ${senders.value.map((s) => {
					var identifier = s.username || s.peer_id;
					var lastSeenMs = s.last_seen ? s.last_seen * 1000 : 0;
					return html`<tr key=${s.peer_id}>
            <td class="senders-td">${s.sender_name || s.peer_id}</td>
            <td class="senders-td" style="color:var(--muted);">${s.username ? `@${s.username}` : "\u2014"}</td>
            <td class="senders-td">${s.message_count}</td>
            <td class="senders-td" style="color:var(--muted);font-size:12px;">${lastSeenMs ? html`<time data-epoch-ms="${lastSeenMs}">${new Date(lastSeenMs).toISOString()}</time>` : "\u2014"}</td>
            <td class="senders-td">
              ${
								s.otp_pending
									? html`<span class="provider-item-badge cursor-pointer select-none" style="background:var(--warning-bg, #fef3c7);color:var(--warning-text, #92400e);" onClick=${() => {
											navigator.clipboard.writeText(s.otp_pending.code).then(() => showToast("OTP code copied"));
										}}>OTP: <code class="text-xs">${s.otp_pending.code}</code></span>`
									: html`<span class="provider-item-badge ${s.allowed ? "configured" : "oauth"}">${s.allowed ? "Allowed" : "Denied"}</span>`
							}
            </td>
            <td class="senders-td">
              ${
								s.allowed
									? html`<button class="provider-btn provider-btn-sm provider-btn-danger" onClick=${() => onAction(identifier, "deny")}>Deny</button>`
									: html`<button class="provider-btn provider-btn-sm" onClick=${() => onAction(identifier, "approve")}>Approve</button>`
							}
            </td>
          </tr>`;
				})}
      </tbody>
    </table>`
		}
  </div>`;
}

// ── Tag-style allowlist input ────────────────────────────────
function AllowlistInput({ value, onChange }) {
	var input = useSignal("");

	function addTag(raw) {
		var tag = raw.trim().replace(/^@/, "");
		if (tag && !value.includes(tag)) onChange([...value, tag]);
		input.value = "";
	}

	function removeTag(tag) {
		onChange(value.filter((t) => t !== tag));
	}

	function onKeyDown(e) {
		if (e.key === "Enter" || e.key === ",") {
			e.preventDefault();
			if (input.value.trim()) addTag(input.value);
		} else if (e.key === "Backspace" && !input.value && value.length > 0) {
			onChange(value.slice(0, -1));
		}
	}

	return html`<div class="flex flex-wrap items-center gap-1.5 rounded border border-[var(--border)] bg-[var(--surface2)] px-2 py-1.5"
    style="min-height:38px;cursor:text;"
    onClick=${(e) => e.currentTarget.querySelector("input")?.focus()}>
    ${value.map(
			(tag) => html`<span key=${tag}
        class="inline-flex items-center gap-1 rounded-full bg-[var(--accent)]/10 px-2 py-0.5 text-xs text-[var(--accent)]">
        ${tag}
        <button type="button" class="inline-flex items-center text-[var(--muted)] hover:text-[var(--accent)]"
          style="line-height:1;font-size:14px;padding:0;background:none;border:none;cursor:pointer;"
          onClick=${(e) => {
						e.stopPropagation();
						removeTag(tag);
					}}>\u00d7</button>
      </span>`,
		)}
    <input type="text" value=${input.value}
      onInput=${(e) => {
				input.value = e.target.value;
			}}
      onKeyDown=${onKeyDown}
      placeholder=${value.length === 0 ? "Type a username and press Enter" : ""}
      class="flex-1 bg-transparent text-[var(--text)] text-sm outline-none border-none"
      style="min-width:80px;padding:2px 0;font-family:var(--font-body);" />
  </div>`;
}

// ── Add channel modal ────────────────────────────────────────
function AddChannelModal() {
	var error = useSignal("");
	var saving = useSignal(false);
	var addModel = useSignal("");
	var allowlistItems = useSignal([]);
	var channelKind = useSignal("telegram");
	var webhookSecret = useSignal("");

	function onSubmit(e) {
		e.preventDefault();
		var form = e.target.closest(".channel-form");
		var accountId = form.querySelector("[data-field=accountId]").value.trim();
		var credential = form.querySelector("[data-field=credential]").value.trim();
		var type = channelKind.value;
		var v = validateChannelFields(type, accountId, credential);
		if (!v.valid) {
			error.value = v.error;
			return;
		}
		error.value = "";
		saving.value = true;
		var addConfig = {
			dm_policy: form.querySelector("[data-field=dmPolicy]").value,
			mention_mode: form.querySelector("[data-field=mentionMode]").value,
			allowlist: allowlistItems.value,
		};
		if (type === "msteams") {
			addConfig.app_id = accountId;
			addConfig.app_password = credential;
			if (webhookSecret.value.trim()) addConfig.webhook_secret = webhookSecret.value.trim();
		} else {
			addConfig.token = credential;
		}
		if (addModel.value) {
			addConfig.model = addModel.value;
			var found = modelsSig.value.find((x) => x.id === addModel.value);
			if (found?.provider) addConfig.model_provider = found.provider;
		}
		addChannel(type, accountId, addConfig).then((res) => {
			saving.value = false;
			if (res?.ok) {
				showAddModal.value = false;
				addModel.value = "";
				allowlistItems.value = [];
				webhookSecret.value = "";
				channelKind.value = "telegram";
				loadChannels();
			} else {
				error.value = (res?.error && (res.error.message || res.error.detail)) || "Failed to connect channel.";
			}
		});
	}

	var defaultPlaceholder =
		modelsSig.value.length > 0
			? `(default: ${modelsSig.value[0].displayName || modelsSig.value[0].id})`
			: "(server default)";

	var selectStyle =
		"font-family:var(--font-body);background:var(--surface2);color:var(--text);border:1px solid var(--border);border-radius:4px;padding:8px 12px;font-size:.85rem;cursor:pointer;";
	var inputStyle =
		"font-family:var(--font-body);background:var(--surface2);color:var(--text);border:1px solid var(--border);border-radius:4px;padding:8px 12px;font-size:.85rem;";
	var isTeams = channelKind.value === "msteams";
	var accountLabel = isTeams ? "App ID / Account ID" : "Bot username";
	var credentialLabel = isTeams ? "App Password (client secret)" : "Bot Token (from @BotFather)";
	var credentialPlaceholder = isTeams ? "Azure client secret" : "123456:ABC-DEF...";

	return html`<${Modal} show=${showAddModal.value} onClose=${() => {
		showAddModal.value = false;
	}} title="Add Channel">
	    <div class="channel-form">
	      <label class="text-xs text-[var(--muted)]">Channel Type</label>
	      <select data-field="channelType" style=${selectStyle} value=${channelKind.value}
	        onChange=${(e) => {
						channelKind.value = e.target.value;
					}}>
	        <option value="telegram">Telegram</option>
	        <option value="msteams">Microsoft Teams</option>
	      </select>
	      <div class="channel-card">
	        ${
						isTeams
							? html`<div>
			            <span class="text-xs font-medium text-[var(--text-strong)]">Microsoft Teams setup</span>
			            <div class="text-xs text-[var(--muted)]">1. Create an Azure Bot registration and copy the App ID + App Password.</div>
			            <div class="text-xs text-[var(--muted)]">2. Set messaging endpoint to <code>/api/channels/msteams/&lt;account_id&gt;/webhook?secret=...</code>.</div>
			          </div>`
							: html`<div>
			            <span class="text-xs font-medium text-[var(--text-strong)]">How to create a Telegram bot</span>
			            <div class="text-xs text-[var(--muted)] channel-help">1. Open <a href="https://t.me/BotFather" target="_blank" class="text-[var(--accent)]" style="text-decoration:underline;">@BotFather</a> in Telegram</div>
			            <div class="text-xs text-[var(--muted)]">2. Send /newbot and follow the prompts to choose a name and username</div>
			            <div class="text-xs text-[var(--muted)]">3. Copy the bot token and paste it below</div>
			          </div>`
					}
	      </div>
		      <label class="text-xs text-[var(--muted)]">${accountLabel}</label>
		      <input data-field="accountId" type="text"
		        placeholder=${isTeams ? "Azure App ID or alias" : "e.g. my_assistant_bot"}
		        style=${inputStyle} />
		      <label class="text-xs text-[var(--muted)]">${credentialLabel}</label>
		      <input data-field="credential" type="password" placeholder=${credentialPlaceholder} style=${inputStyle}
		        autocomplete="new-password"
		        autocapitalize="none"
		        autocorrect="off"
		        spellcheck="false"
		        name="telegram_bot_token" />
		      ${
						isTeams &&
						html`<div>
				          <label class="text-xs text-[var(--muted)]">Webhook Secret (optional)</label>
				          <input type="text" placeholder="shared secret for ?secret=..." style=${inputStyle}
				            value=${webhookSecret.value}
				            onInput=${(e) => {
											webhookSecret.value = e.target.value;
										}} />
				        </div>`
					}
	      <label class="text-xs text-[var(--muted)]">DM Policy</label>
	      <select data-field="dmPolicy" style=${selectStyle}>
	        <option value="allowlist">Allowlist only</option>
        <option value="open">Open (anyone)</option>
        <option value="disabled">Disabled</option>
      </select>
      <label class="text-xs text-[var(--muted)]">Group Mention Mode</label>
      <select data-field="mentionMode" style=${selectStyle}>
        <option value="mention">Must @mention bot</option>
        <option value="always">Always respond</option>
        <option value="none">Don't respond in groups</option>
      </select>
      <label class="text-xs text-[var(--muted)]">Default Model</label>
      <${ModelSelect} models=${modelsSig.value} value=${addModel.value}
        onChange=${(v) => {
					addModel.value = v;
				}}
        placeholder=${defaultPlaceholder} />
      <label class="text-xs text-[var(--muted)]">DM Allowlist</label>
      <${AllowlistInput} value=${allowlistItems.value} onChange=${(v) => {
				allowlistItems.value = v;
			}} />
      ${error.value && html`<div class="text-xs text-[var(--error)] channel-error" style="display:block;">${error.value}</div>`}
	      <button class="provider-btn"
	        onClick=${onSubmit} disabled=${saving.value}>
	        ${saving.value ? "Connecting\u2026" : "Connect Channel"}
	      </button>
	    </div>
	  </${Modal}>`;
}

// ── Edit channel modal ───────────────────────────────────────
function EditChannelModal() {
	var ch = editingChannel.value;
	var error = useSignal("");
	var saving = useSignal(false);
	var editModel = useSignal("");
	var allowlistItems = useSignal([]);
	var editCredential = useSignal("");
	var editWebhookSecret = useSignal("");
	useEffect(() => {
		editModel.value = ch?.config?.model || "";
		allowlistItems.value = ch?.config?.allowlist || [];
		editCredential.value = "";
		editWebhookSecret.value = ch?.config?.webhook_secret || "";
	}, [ch]);
	if (!ch) return null;
	var cfg = ch.config || {};
	var isTeams = channelType(ch.type) === "msteams";

	function onSave(e) {
		e.preventDefault();
		var form = e.target.closest(".channel-form");
		error.value = "";
		saving.value = true;
		var updateConfig = {
			dm_policy: form.querySelector("[data-field=dmPolicy]").value,
			mention_mode: form.querySelector("[data-field=mentionMode]").value,
			allowlist: allowlistItems.value,
		};
		if (isTeams) {
			updateConfig.app_id = cfg.app_id || ch.account_id;
			updateConfig.app_password = editCredential.value || cfg.app_password || "";
			if (editWebhookSecret.value.trim()) updateConfig.webhook_secret = editWebhookSecret.value.trim();
		} else {
			updateConfig.token = cfg.token || "";
		}
		if (editModel.value) {
			updateConfig.model = editModel.value;
			var found = modelsSig.value.find((x) => x.id === editModel.value);
			if (found?.provider) updateConfig.model_provider = found.provider;
		}
		sendRpc("channels.update", {
			type: channelType(ch.type),
			account_id: ch.account_id,
			config: updateConfig,
		}).then((res) => {
			saving.value = false;
			if (res?.ok) {
				editingChannel.value = null;
				loadChannels();
			} else {
				error.value = (res?.error && (res.error.message || res.error.detail)) || "Failed to update channel.";
			}
		});
	}

	var defaultPlaceholder =
		modelsSig.value.length > 0
			? `(default: ${modelsSig.value[0].displayName || modelsSig.value[0].id})`
			: "(server default)";

	var selectStyle =
		"font-family:var(--font-body);background:var(--surface2);color:var(--text);border:1px solid var(--border);border-radius:4px;padding:8px 12px;font-size:.85rem;cursor:pointer;";
	var inputStyle =
		"font-family:var(--font-body);background:var(--surface2);color:var(--text);border:1px solid var(--border);border-radius:4px;padding:8px 12px;font-size:.85rem;";

	return html`<${Modal} show=${true} onClose=${() => {
		editingChannel.value = null;
	}} title=${`Edit ${channelLabel(ch.type)} Channel`}>
	    <div class="channel-form">
	      <div class="text-sm text-[var(--text-strong)]">${ch.name || ch.account_id}</div>
	      ${
					isTeams &&
					html`<div>
				        <label class="text-xs text-[var(--muted)]">App Password (optional: leave blank to keep existing)</label>
				        <input type="password" style=${inputStyle} value=${editCredential.value}
				          onInput=${(e) => {
										editCredential.value = e.target.value;
									}} />
				      </div>`
				}
	      ${
					isTeams &&
					html`<div>
				        <label class="text-xs text-[var(--muted)]">Webhook Secret</label>
				        <input type="text" style=${inputStyle} value=${editWebhookSecret.value}
				          onInput=${(e) => {
										editWebhookSecret.value = e.target.value;
									}} />
				      </div>`
				}
	      <label class="text-xs text-[var(--muted)]">DM Policy</label>
	      <select data-field="dmPolicy" style=${selectStyle} value=${cfg.dm_policy || "allowlist"}>
	        <option value="allowlist">Allowlist only</option>
        <option value="open">Open (anyone)</option>
        <option value="disabled">Disabled</option>
      </select>
      <label class="text-xs text-[var(--muted)]">Group Mention Mode</label>
      <select data-field="mentionMode" style=${selectStyle} value=${cfg.mention_mode || "mention"}>
        <option value="mention">Must @mention bot</option>
        <option value="always">Always respond</option>
        <option value="none">Don't respond in groups</option>
      </select>
      <label class="text-xs text-[var(--muted)]">Default Model</label>
      <${ModelSelect} models=${modelsSig.value} value=${editModel.value}
        onChange=${(v) => {
					editModel.value = v;
				}}
        placeholder=${defaultPlaceholder} />
      <label class="text-xs text-[var(--muted)]">DM Allowlist</label>
      <${AllowlistInput} value=${allowlistItems.value} onChange=${(v) => {
				allowlistItems.value = v;
			}} />
      ${error.value && html`<div class="text-xs text-[var(--error)] channel-error" style="display:block;">${error.value}</div>`}
	      <button class="provider-btn"
	        onClick=${onSave} disabled=${saving.value}>
	        ${saving.value ? "Saving\u2026" : "Save Changes"}
	      </button>
    </div>
  </${Modal}>`;
}

// ── Main page component ──────────────────────────────────────
function ChannelsPage() {
	useEffect(() => {
		// Use prefetched cache for instant render
		if (S.cachedChannels !== null) channels.value = S.cachedChannels;
		if (connected.value) loadChannels();

		var unsub = onEvent("channel", (p) => {
			var selected = parseSenderSelectionKey(sendersAccount.value || "");
			if (p.kind === "otp_resolved") {
				loadChannels();
			}
			if (
				activeTab.value === "senders" &&
				selected.account_id === p.account_id &&
				selected.type === channelType(p.channel_type) &&
				(p.kind === "inbound_message" || p.kind === "otp_challenge" || p.kind === "otp_resolved")
			) {
				loadSenders();
			}
		});
		S.setChannelEventUnsub(unsub);

		return () => {
			if (unsub) unsub();
			S.setChannelEventUnsub(null);
		};
	}, [connected.value]);

	return html`
    <div class="flex-1 flex flex-col min-w-0 p-4 gap-4 overflow-y-auto">
      <div class="flex items-center gap-3">
        <h2 class="text-lg font-medium text-[var(--text-strong)]">Channels</h2>
        <div style="display:flex;gap:4px;margin-left:12px;">
          <button class="session-action-btn" style=${activeTab.value === "channels" ? "font-weight:600;" : ""}
            onClick=${() => {
							activeTab.value = "channels";
						}}>Channels</button>
          <button class="session-action-btn" style=${activeTab.value === "senders" ? "font-weight:600;" : ""}
            onClick=${() => {
							activeTab.value = "senders";
						}}>Senders</button>
        </div>
        ${
					activeTab.value === "channels" &&
					html`
	          <button class="provider-btn"
	            onClick=${() => {
								if (connected.value) showAddModal.value = true;
							}}>+ Add Channel</button>
	        `
				}
      </div>
      ${activeTab.value === "channels" ? html`<${ChannelsTab} />` : html`<${SendersTab} />`}
    </div>
    <${AddChannelModal} />
    <${EditChannelModal} />
    <${ConfirmDialog} />
  `;
}

var _channelsContainer = null;

export function initChannels(container) {
	_channelsContainer = container;
	container.style.cssText = "flex-direction:column;padding:0;overflow:hidden;";
	activeTab.value = "channels";
	showAddModal.value = false;
	editingChannel.value = null;
	sendersAccount.value = "";
	senders.value = [];
	render(html`<${ChannelsPage} />`, container);
}

export function teardownChannels() {
	S.setRefreshChannelsPage(null);
	if (S.channelEventUnsub) {
		S.channelEventUnsub();
		S.setChannelEventUnsub(null);
	}
	if (_channelsContainer) render(null, _channelsContainer);
	_channelsContainer = null;
}
