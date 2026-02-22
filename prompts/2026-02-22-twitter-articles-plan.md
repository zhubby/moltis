# Twitter Article Ideas — February 2026

Based on the past week of shipping (0.9.5 → 0.9.10, GraphQL API, voice overhaul, cron delivery, 6 releases in 7 days).

---

## 1. "Your AI agent just got a GraphQL API"

**Hook:** Most claw apps give you a JSON-RPC blob and wish you luck. Moltis now ships a full GraphQL API — queries, mutations, real-time subscriptions — with a built-in GraphiQL playground. One `POST /graphql` to rule them all.

**Punch lines:**
- 26 query namespaces, 25 mutation namespaces, 16 subscriptions — typed, introspectable, zero guessing
- OpenClaw: no GraphQL. NanoClaw: no GraphQL. PicoClaw: no GraphQL. Moltis: `/graphql`
- Bridge pattern means zero duplicated service logic — the GraphQL layer is a typed window into the same engine
- Build dashboards, integrations, Slack bots — anything that speaks GraphQL can now drive your agent

**Why it matters:** GraphQL turns Moltis from "a chatbot" into "a platform." Developers can build on top of it without reverse-engineering WebSocket payloads.

---

## 2. "6 releases in 7 days, and nobody noticed"

**Hook:** We shipped 0.9.5 through 0.9.10 this week. No downtime, no breaking changes, no migration guides. That's what a single 44 MB binary gets you.

**Punch lines:**
- OpenClaw is 430K lines of TypeScript. Moltis is ~150K lines of Rust with 2,300+ tests. Smaller surface = fewer things to break
- Zero `unsafe` code, workspace-wide. Not "we try to avoid it" — the compiler enforces it
- Single binary means upgrades are "download and restart." No `npm install`, no dependency tree roulette
- Each release was a real fix or feature, not a version bump for the changelog — cron delivery, voice auto-config, quota surfacing, DeepSeek tool support

**Why it matters:** Shipping velocity without instability. Rust's type system is your QA department.

---

## 3. "Your cron job now talks to Telegram"

**Hook:** Schedule an agent task. It runs at 8am. The output lands in your Telegram group. No Zapier. No webhooks. No glue code. Just a toggle in the UI.

**Punch lines:**
- Cron → Agent Turn → LLM completion → Telegram delivery. One pipeline, zero external services
- Supports cron expressions, fixed intervals, one-shot timestamps — with timezone awareness
- Each run is logged with token usage, duration, and status. You know exactly what it cost
- Rate-limited (10 jobs/min default), stuck-job detection (auto-clears after 2h), isolated sessions
- OpenClaw cron: doesn't exist. NanoClaw cron: doesn't exist. Moltis: built-in with delivery

**Why it matters:** This is the "autonomous agent" people keep talking about, except it actually runs reliably on a schedule instead of vibing in a loop.

---

## 4. "Voice I/O with 15 providers and zero config"

**Hook:** Moltis 0.9.10 auto-detects your OpenAI key, enables TTS and Whisper STT, and you're talking to your agent. No voice config file. No separate API key. It just works.

**Punch lines:**
- 8 TTS providers (ElevenLabs, OpenAI, Google, Piper, Coqui…), 9 STT providers (Whisper, Groq, Deepgram, local Sherpa…)
- Mix cloud and local — Whisper for transcription, Piper for offline TTS. Your choice, per operation
- Voice works in the web UI AND through Telegram. Send a voice memo → transcribed → agent responds → TTS audio back
- Key resolution fallback chain: voice config → env var → LLM provider config. Reuses what you already have
- OpenClaw voice: none. NanoClaw voice: none. ZeroClaw voice: none. Moltis: 15 providers, built-in

**Why it matters:** Voice isn't a novelty — it's how people interact with agents on mobile. Having it built-in (not bolted-on) changes the UX.

---

## 5. "44 MB binary, zero dependencies, runs on a Raspberry Pi"

**Hook:** OpenClaw needs Node.js. NanoClaw needs Node.js. PicoClaw needs Go. Moltis needs... nothing. One binary. Download, run, done.

**Punch lines:**
- 44 MB single binary vs. `node_modules` black holes
- Runs on Mac Mini, Raspberry Pi, a $5 VPS. Your hardware, your data, your keys
- Memory-safe without a garbage collector. Ownership model, zero `unsafe`, no GC pauses
- Agent loop is ~5K LoC (runner.rs + model.rs). OpenClaw's equivalent? 430K LoC. Read it in an afternoon
- Password + Passkey + API key auth built-in. Not "add a reverse proxy." Built. In.

**Why it matters:** Self-hosted AI shouldn't require a DevOps team. Moltis is closer to "download and double-click" than anything else in the space.

---

## 6. "DeepSeek tool calling was broken everywhere. We fixed it in one commit"

**Hook:** DeepSeek was registered through a generic provider that doesn't support tool calling. One commit moved it to the OpenAI-compatible provider table. Tools work now. That's it.

**Punch lines:**
- The fix was 1 commit, clear diff, no hacks. That's what happens when your provider abstraction is clean
- `deepseek-chat` and `deepseek-reasoner` now have full tool support — web search, code execution, file ops
- OpenClaw probably has a 47-file PR for this with 3 "LGTM" reviews and a merge conflict
- Rust's type system caught the mismatch at compile time in the test suite

**Why it matters:** Provider support isn't about listing model names. It's about each model actually working with all your tools.

---

## Recommended posting order

1. **#5** (single binary) — broad appeal, easy to understand, shareable
2. **#4** (voice) — visual/demo-friendly, record a video of talking to the agent
3. **#3** (cron → Telegram) — "autonomous agent" angle is trending
4. **#1** (GraphQL) — developer audience, very shareable in dev circles
5. **#2** (6 releases) — establishes shipping cadence credibility
6. **#6** (DeepSeek) — timely if DeepSeek is still in the news cycle
