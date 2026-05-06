# Translation Service Learnings

## Final Implementation Choice

- **Path**: Reused the existing `test_chat` pipeline (entry/channel/adapter → upstream HTTP call → transform response).
- **Entry Resolution**: `entry_id` in request routes to a specific API entry; when `None`, falls back to the first available entry in the database.
- **Prompt Construction**: System prompt forces "output ONLY translated text"; user message includes optional source/target language hints.
- **Success/Failure Cache**: Both success (`success=true`, `translated_text` populated) and failure (`success=false`, `error` message) write to `AppState.translation_relay` cache.

## Remaining Risks / Next Iteration Points

1. **Entry Selection Heuristic**:
   - Current: Use first entry or explicit `entry_id`.
   - Risk: First entry may not be optimal for translation (e.g., wrong model, rate-limited).
   - Next: Consider a "translation-optimized" entry selection strategy (e.g., prefer entries with higher `response_ms` stability, or add a `translation_entry_id` setting).

2. **Translation Model Specialization**:
   - Current: Any LLM entry is used.
   - Risk: Some models may not be good at translation tasks.
   - Next: Add a configuration option to specify a dedicated translation model/entry.

3. **Error Classification**:
   - Current: All errors are treated equally and written to `error` field.
   - Risk: Cannot distinguish between transient network issues and permanent failures (e.g., 401/403).
   - Next: Classify errors and potentially trigger cooldown/disable logic similar to the proxy router.

4. **Streaming Support**:
   - Current: Non-streaming only.
   - Risk: Large translations may have poor UX (no incremental feedback).
   - Next: If UI requires streaming translation, implement SSE-based streaming path.

5. **Timeout Handling**:
   - Current: Relies on reqwest's default timeout.
   - Risk: Long translations may hang.
   - Next: Add explicit timeout configuration for translation requests.

6. **Translation Quality Feedback**:
   - Current: No quality validation.
   - Risk: Poor translations go undetected.
   - Next: Add optional user feedback mechanism to track translation quality.

## Verification Status

- `cargo check`: ✅ Pass
- `translate_and_store()` signature: ✅ Unchanged externally (command layer still works)
- Success path: ✅ Writes `success=true` + `translated_text` to cache
- Failure path: ✅ Writes `success=false` + `error` message to cache
