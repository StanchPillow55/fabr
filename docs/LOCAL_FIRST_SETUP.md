# Local-first setup (Ollama / MLX)

Run Fabr end to end against a local LLM, with no cloud key. The agent client
speaks the **OpenAI-compatible Chat Completions API** (`POST {base_url}/chat/completions`),
so any server that exposes that endpoint works: Ollama, LM Studio, or `mlx_lm.server`.
One code path (`LocalModel`) covers all of them; the only thing that changes is
`local_base_url`.

This guide is plug-and-play: install a local server, drop in a two-line
`settings.toml`, and test. The **definitive checklist** of what you must do by
hand is at the end under [Plug-and-play checklist](#plug-and-play-checklist).

> Accuracy note: where an exact path or behavior is not yet wired, it is flagged
> inline and again under [Known gaps / next](#known-gaps--next). Assumptions are
> called out as "Assumption:".

---

## 1. Install and run a local LLM

Fabr needs two things from a local server:

- a **tool-calling chat model** for the pipeline (define, plan, execute, evaluate)
- an **embedding model** for local semantic memory (optional; only the
  `localmem_*` tools need it)

### Option A — Ollama (recommended, simplest)

```sh
# install
brew install ollama

# start the server (leave it running; listens on http://localhost:11434)
ollama serve

# in another terminal: pull a tool-calling chat model
ollama pull qwen2.5

# pull the embedding model used by local memory
ollama pull nomic-embed-text
```

- Chat completions endpoint: `http://localhost:11434/v1` (note the `/v1`).
- Native embeddings endpoint: `http://localhost:11434/api/embeddings` (Fabr's
  local memory calls this directly; you only set the base `http://localhost:11434`).
- Ollama needs **no API key**.

Pick any Ollama model that supports tool/function calling. `qwen2.5` is a good
default. Whatever you pull, the `model` value in `settings.toml` must match the
Ollama model tag exactly (e.g. `qwen2.5`, `qwen2.5:7b`, `llama3.1`).

### Option B — LM Studio or MLX (`mlx_lm.server`)

Any server that exposes an OpenAI-compatible endpoint on port `1234` works the
same way.

- **LM Studio**: load a tool-calling model, open the **Local Server** tab, and
  Start Server. It serves `http://localhost:1234/v1`.
- **MLX**: `mlx_lm.server --port 1234` serves `http://localhost:1234/v1`.

Then set in `settings.toml`:

```toml
local_base_url = "http://localhost:1234/v1"
```

The `base_url` must already include the `/v1` suffix; Fabr appends
`/chat/completions`. No API key is required (if your server wants one, see
[Optional API key](#optional-api-key)).

> Embeddings caveat: local **memory** (`localmem_*`) currently calls the
> **Ollama-native** embeddings endpoint (`/api/embeddings`), not the
> OpenAI `/v1/embeddings` shape. If you use LM Studio/MLX for chat, you still
> need Ollama (or another server exposing `/api/embeddings`) running for
> embeddings, pointed at by `FABR_LOCAL_BASE_URL`. See
> [Known gaps / next](#known-gaps--next).

---

## 2. settings.toml / secrets.toml

Fabr reads two files from the fabric-space directory. For the desktop app and
the default home space that is:

```
~/.fabric-space/settings.toml
~/.fabric-space/secrets.toml
```

The agent reads three keys for provider selection (all in `settings.toml`):

| key | default | meaning |
|---|---|---|
| `provider` | `anthropic` | `local` selects the OpenAI-compatible client; `anthropic` selects the cloud client |
| `model` | — (required) | model id/tag; must match what your server serves |
| `local_base_url` | `http://localhost:11434/v1` | base url including `/v1`, used only when `provider = "local"` |

In local mode **no Anthropic key is read or required** — `require_ready` only
demands the `folder` and `model` setup steps for `provider = "local"`
(it demands `folder`, `api_key`, `model` for `anthropic`).

### Local (Ollama) — full example `~/.fabric-space/settings.toml`

```toml
default_folder = "/Users/you/notes"
provider = "local"
model = "qwen2.5"
local_base_url = "http://localhost:11434/v1"

# housekeeping (optional; shown with defaults)
auto_mode = false
question_budget = 5
heartbeat_interval_s = 10

[setup]
# For a local provider only these two steps must be present.
completed_steps = ["folder", "model"]
completed = true
```

Local — `~/.fabric-space/secrets.toml` (needs **no** `anthropic_api_key`):

```toml
# In local mode this file can be empty. Fabr still reads it, so it must exist
# and must be valid TOML. Leave it empty or add only sponsor keys you use.
```

> The file must exist and parse — `load_agent_settings` reads both
> `settings.toml` and `secrets.toml`. An empty-but-present `secrets.toml` is
> fine in local mode.

### Local (LM Studio / MLX) variant

Identical to the above, with the port and (if needed) the model tag changed:

```toml
provider = "local"
model = "your-loaded-model-id"
local_base_url = "http://localhost:1234/v1"
```

### Cloud (Anthropic) variant — for comparison

```toml
# ~/.fabric-space/settings.toml
default_folder = "/Users/you/notes"
provider = "anthropic"          # or omit entirely; anthropic is the default
model = "claude-sonnet-5-5"

[setup]
completed_steps = ["folder", "api_key", "model"]
completed = true
```

```toml
# ~/.fabric-space/secrets.toml
anthropic_api_key = "sk-ant-..."
```

### Optional API key

If your local server requires a bearer token, Fabr's `LocalModel` can send
`Authorization: Bearer <key>`.

> Assumption / gap: there is no dedicated settings key wired to inject a local
> bearer token from `settings.toml`/`secrets.toml` today — the client supports
> `with_api_key`, but the runner builds the local backend without a key (Ollama
> and LM Studio both work without one). If your server mandates a key, that
> wiring is a small follow-up. See [Known gaps / next](#known-gaps--next).

---

## 3. Connectors and what each needs from you (plug-and-play auth)

These are the local-first connectors. None of them talk to the cloud. Each is a
read or a local-file operation, so "auth" here means **pointing Fabr at your own
data** or **pulling a model**, not entering a token.

### Granola (local meeting-notes cache) — read-only, auto-detected

- **What it does:** reads one local Granola JSON cache and exposes
  `granola_list`, `granola_search`, `granola_get`. Never opens a network
  connection.
- **Auth / setup:** none. It auto-detects the macOS cache at
  `~/Library/Application Support/Granola` (tries `cache-v3.json`, then
  `cache-v2.json`, then `cache.json`).
- **Override:** set `FABR_GRANOLA_PATH` to either the Granola directory or a
  specific cache file:

  ```sh
  export FABR_GRANOLA_PATH="$HOME/Library/Application Support/Granola"
  # or point straight at a file
  export FABR_GRANOLA_PATH="/path/to/cache-v3.json"
  ```

- **If you don't use Granola:** the connector degrades to empty (no notes, no
  error). Nothing to do.
- **Flag:** this reads **your real Granola data path**. If you want it to see
  your notes, make sure Granola has written a cache (open the app at least once)
  or set `FABR_GRANOLA_PATH`.

### Local memory (fully local vector store) — needs an embedding model

- **What it does:** a folder-scoped semantic store. `localmem_remember` /
  `localmem_recall` / `localmem_forget` embed text with Ollama and do cosine
  top-k in pure Rust. Each node owns
  `<node_dir>/.fabric-space/memory/embeddings.json`; a global store lives at
  `~/.fabric-space/memory/embeddings.json`.
- **Auth / setup:** pull the embedding model once:

  ```sh
  ollama pull nomic-embed-text
  ```

- **Overrides:**

  ```sh
  export FABR_EMBED_MODEL="nomic-embed-text"          # default
  export FABR_LOCAL_BASE_URL="http://localhost:11434" # default (NO /v1 here)
  ```

  Note: for embeddings `FABR_LOCAL_BASE_URL` is the **bare** base
  (`http://localhost:11434`), because Fabr appends `/api/embeddings`. This is a
  different variable from the `local_base_url` chat setting in `settings.toml`
  (which does include `/v1`).
- **Errors are honest:** a missing model returns
  `model '...' not found; run ollama pull ...`; a down server returns
  `embedding endpoint unavailable (is ollama running at ...?)`.

### Learned preferences (local, authoritative) — no setup

- **What it does:** `~/.fabric-space/learned-preferences.json` holds confirmed
  user preferences (`propose` → `confirm` → `recall`). Pure local file; no
  network, no model.
- **Auth / setup:** none.
- **Note:** the former cloud "Moss" index is **not integrated**
  (`moss_index_available` always returns `NotIntegrated`); recall reads the
  local file only.

### Sponsor connectors (Bright Data, Nessie, Apify, Cartesia, Moss, Fo, …)

- **Not local-first.** These appear only when `~/.fabric-space/secrets.toml`
  parses and contains that provider's key, and each remote call needs its own
  approved grant. They are **optional** and orthogonal to local mode. See
  [docs/SPONSOR_TRACKS.md](SPONSOR_TRACKS.md) and
  [docs/sponsor-credentials.md](sponsor-credentials.md). You do not need any of
  them to run locally.

### Not yet wired (flagged)

- **Google Workspace / Gmail connector:** future. There is no Gmail/Workspace
  connector in the tree today.
- **Telegram / personal data bank / learning-from-answers:** designed, not
  built (per the top-level README).

---

## 4. How to run

### Build / dev

```sh
# one-time: install JS deps
cd app && npm install

# dev (hot reload)
cd app && npm run tauri dev

# or a packaged macOS build
cd app && npm run tauri build
```

Rust/JS sanity checks (optional):

```sh
cargo test --workspace
cd app && npm test
```

### First run

1. **Start your local server first** (`ollama serve`, LM Studio, or
   `mlx_lm.server`) and confirm the models are pulled/loaded.
2. **Open Fabr.** A 6-screen setup wizard asks for a default folder, a model,
   and (for cloud) an Anthropic key.

   > Assumption / gap: the wizard's validation path is still Anthropic-centric
   > (it verifies an Anthropic key and model against `api.anthropic.com`). There
   > is **no "choose Local" toggle in the wizard UI yet**. To run local today,
   > complete or skip the wizard and then **set `provider = "local"`,
   > `model = "qwen2.5"`, and `local_base_url` in
   > `~/.fabric-space/settings.toml` by hand**, with
   > `completed_steps = ["folder", "model"]`. The agent runner reads those keys
   > directly and builds the local backend with no key. See
   > [Known gaps / next](#known-gaps--next).

3. **Create a reminder.** In the folder it belongs to, type a title on the node,
   press Enter, fill the sheet, and Save. That writes a Markdown task in
   `<folder>/.fabric-space/tasks/` in the `reminder` state.
4. **Delegate.** Press Generate with AI. The pipeline runs: gather context →
   define (and ask you what's missing) → plan.
5. **Approve.** Review the plan and its assertions, then approve. (Auto mode,
   if enabled, skips the tap for reversible plans.)
6. **Execute.** The tool loop runs inside the task's folder with 4 local tools
   (read any file, list, write Markdown, make a folder).
7. **Two-pass final confirmation.** Before any **irreversible external effect**
   (send an email, make a payment, and similar `ExternalEffect`/`Payment`
   tools), execution **halts a second time** at a `final_confirm` gate bound to
   that exact artifact. Nothing is dispatched until you approve or refuse it.
   The confirmation is bound to the invocation id, so a confirmation for one
   send can never authorize a different one. Local reads, lists, Markdown
   writes, and make-directory do **not** trigger this second gate.

   > In pure local mode with only the 4 local tools and no sponsor keys, you
   > will typically not hit the second gate — it exists to guard outbound
   > effects, which require their own grants.
8. **Evaluate.** A fresh-context judge scores each assertion from the evidence
   (the execute log and file reads).

---

## Known gaps / next

Honest status of what is stubbed vs working for local-first use.

**Working**
- OpenAI-compatible local chat client (`LocalModel`) covering define, plan,
  execute (tool calls), and evaluate — one path for Ollama and LM Studio/MLX,
  selected by `local_base_url`.
- Provider switch in the runner: `provider = "local"` builds the local backend
  and requires **no** Anthropic key (`require_ready` enforces `folder` + `model`
  only for local).
- Local memory: fully local vector store with Ollama embeddings, cosine top-k,
  folder-scoped and global stores, atomic `0o600` writes.
- Learned preferences: local, authoritative, no network.
- Granola connector: read-only, auto-detected, env override, degrades to empty.
- Two-pass final confirmation gate before irreversible external effects.
- M2 scope: the execute tool loop stays inside the task's folder — a
  **read-root / write-scoped** containment (read any file in scope, write only
  Markdown, make folders). Scope containment is property-tested.

**Stubbed / not wired**
- **First-run wizard has no Local toggle.** Local mode is enabled by editing
  `settings.toml` (`provider`, `model`, `local_base_url`). The wizard still
  validates against Anthropic. (Assumption based on the current app code having
  no `provider`/`local_base_url` UI.)
- **Local bearer token not plumbed from config.** `LocalModel::with_api_key`
  exists, but the runner builds the local backend without a key. Fine for Ollama
  and LM Studio; a server that mandates a key needs a small wiring follow-up.
- **Embeddings use the Ollama-native endpoint** (`/api/embeddings`), not the
  OpenAI `/v1/embeddings` shape. If you run LM Studio/MLX for chat, you still
  need an Ollama-style `/api/embeddings` server for local memory, pointed at by
  `FABR_LOCAL_BASE_URL`.
- **Moss index not integrated** (`moss_index_available` → `NotIntegrated`);
  recall reads the local preferences file only.
- **Google Workspace / Gmail connector:** future — not present in the tree.
- **Skills** are loaded into the model's context (not executed as a separate
  runtime). The sample skill lives at
  `.fabric-space-example/skills/todos/skill.md`.
- **Sponsor connectors** (Bright Data, Nessie, Apify, Cartesia, Moss, Fo, …) are
  cloud and key-gated; optional and unrelated to local mode.
- **Telegram, personal data bank, learning-from-answers:** designed, not built.

---

## Plug-and-play checklist

Do these, in order, to run fully local:

1. `brew install ollama`
2. `ollama serve` (leave running)
3. `ollama pull qwen2.5` (tool-calling chat model)
4. `ollama pull nomic-embed-text` (embeddings for local memory)
5. Create `~/.fabric-space/settings.toml` with `provider = "local"`,
   `model = "qwen2.5"`, `local_base_url = "http://localhost:11434/v1"`,
   `default_folder = "<your folder>"`, and
   `[setup] completed_steps = ["folder", "model"]`, `completed = true`.
6. Create `~/.fabric-space/secrets.toml` (may be empty; it must exist and parse —
   **no `anthropic_api_key` needed** in local mode).
7. (LM Studio/MLX instead of Ollama for chat) start the server on `:1234` and
   set `local_base_url = "http://localhost:1234/v1"`. Still run an Ollama-style
   `/api/embeddings` server for local memory (`FABR_LOCAL_BASE_URL`).
8. (Optional) `export FABR_GRANOLA_PATH=...` to point at your Granola cache, or
   leave it to auto-detect `~/Library/Application Support/Granola`.
9. (Optional) `export FABR_EMBED_MODEL` / `FABR_LOCAL_BASE_URL` to override the
   embedding model or its base url.
10. `cd app && npm install && npm run tauri dev` (or `npm run tauri build`),
    open Fabr, create a reminder, Delegate, approve the plan, and watch it
    execute — approving the two-pass final confirmation if an outbound effect is
    ever requested.
