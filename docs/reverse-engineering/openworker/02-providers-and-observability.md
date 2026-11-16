# 2 Model providers · 17 Observability and token usage

## 2. Model provider abstraction

- **Description**: one interface over cloud and local models; the runtime owns the loop, providers only complete turns.
- **User workflow**: Settings → Models & Keys → connect a provider or local server → pick a model.
- **Desktop**: `ProviderClient` ABC (`coworker/providers/base.py:126`) with `complete()`, `capabilities()`, optional `stream()` yielding `StreamChunk`. Implementations: Anthropic, OpenAI (Chat + Responses), Gemini, Bedrock, Codex, local servers (Ollama, llama.cpp). Registry: `providers/registry.py` (`build_provider_client`, `detect_provider`).
- **Harness**: `ModelProvider::{responses, chat_completions, anthropic}` (`harness-sdk/src/providers.rs`), endpoint suffix per API (`runtime/src/model_api.rs:28`), `Credentials::secure(key).memory_only()`; connection identity (API + endpoint) is immutable.
- **Data**: `ToolCall`, `TokenUsage { input, output, cache_read, cache_write, reasoning }` (`base.py:17-50`), `AssistantTurn`, `ModelCapabilities`.
- **Security**: keys in the app's secret store, never in model-visible files; Harness strictly validates streams (only a fully terminated, validated completion may authorize tool execution, `chat_completions.rs:1`).
- **DigitsNepal note**: the strict stream validation rejected Token Lab's extra gateway chunk; DigitsNepal's relay handles it.
- **Complexity**: Medium. **Confidence**: High.

## 17. Observability and token usage

- **Desktop**: recorder and store (`coworker/observability/{recorder,store,calculator,usage_api}.py`); turns and steps recorded per session; one calculator so every screen shows the same numbers; decision paths recorded (`observability/decision.py`).
- **Harness**: `observability` crate (SQLite/Postgres writer, query, evaluations); optional OTLP export of content-free traces (`HarnessBuilder::otlp`); `RunEvent::ProviderError` diagnostics; `budget.rs` limits.
- **Privacy**: OTLP export is content-free by default (SDK docs).
- **Complexity**: Medium. **Confidence**: Medium (structure read, not every metric traced).
