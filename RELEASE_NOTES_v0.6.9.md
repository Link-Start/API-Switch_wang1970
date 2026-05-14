# API Switch v0.6.9 Release Notes

> Personal AI API Management & Forwarding Hub

---

## What's New

### Responses API Upstream Support
- Full streaming support via `/v1/responses` — upstream Responses SSE events are now correctly transformed, fixing the "upstream stream completed without valid output" error. Enables connectivity with relay stations exposing Responses API endpoints.

### Smart Routing with Model Aliases
- `/v1/models` endpoint now returns `display_name` (alias) and `group_name` alongside each model ID — consistent across all four endpoints (OpenAI, Claude, Gemini, Azure).
- Router now supports exact match by alias (`display_name`) as a new priority tier: group match → model match → **alias match** → fuzzy match → AUTO fallback.

### Desktop Performance & Stability
- **Eliminated UI freezes**: All write-heavy Tauri commands converted from sync (`pub fn`) to async (`pub async fn`) to prevent blocking the thread pool — affects channel, pool, and token operations.
- **Batch toggle IPC**: Shift+click batch toggle now uses a single `batchToggle` IPC call instead of N concurrent calls, preventing IPC storms.
- **Tray debounce**: System tray refreshes are debounced to 1500ms to avoid redundant menu rebuilds.
- **Event debounce**: `entries-changed` events are throttled (300ms) to prevent React re-render storms on rapid data changes.
- **Tab switch jitter fix**: Window-level scrollbar now always reserved (`overflow-y-scroll`) to eliminate layout shift when switching pages.

### Responsive Infinite Scroll
- All management pages (Pool, Channels, Tokens, Logs) now use infinite scroll with `useInfiniteQuery` — no more traditional pagination.
- Search inputs are debounced (300ms) to avoid excessive backend requests.
- Filter changes no longer trigger aggressive `scrollTo` behavior.
- `placeholderData` keeps previous results visible while new data loads, eliminating layout jumps.

### State Machine Driven UI Refresh
- All data mutations now bump a central state version counter.
- A 2-second polling mechanism detects version changes and automatically refreshes all active queries — eliminating the need for explicit refresh logic in most operations.
- PoolManager additionally calls `refetch({ refetchPage: () => true })` to refresh all loaded infinite scroll pages (not just page 1) when state version changes.

### Speed Test Improvements
- Speed tests now properly clear cooldown state and bump the state version on completion.
- Failed entries are batch-disabled after all tests finish (no more mid-test IPC storms).
- Results are driven by the state machine refresh mechanism — no separate update logic needed.

### Model Catalog & Metadata
- `display_name` (alias) is now stored and propagated through the catalog metadata pipeline.
- Model list responses include both `display_name` and `group_name` for better client integration.
- Backfill operations update `display_name` alongside other catalog fields.

### Other Fixes
- Channel enable/disable toggle no longer freezes the window (sync-to-async conversion).
- Drag-and-drop in PoolManager no longer requires all pages to be loaded.
- `key.txt` encrypted and hidden in the project as `Square48x48Logo.png` (XOR obfuscated, for first-run auto-setup).
- Auto-create default channel (`test api` / `glm-4-flash`) on first run with randomly selected API key from embedded pool.
- Unused local reference docs excluded from git tracking.

---

## Changes since v0.5.0

- 130+ commits
- 200+ files changed
- Version: 0.5.0 → 0.6.9

---

## Upgrade Notes

- **Settings**: The `show_conversation_model` default is now `true`. Existing installations need to enable it in Settings if previously turned off.
- **API Types**: Channel API type labels updated to: `OpenAI-compatible`, `OpenAI`, `Claude`, `Gemini`, `Azure`, `OpenAI-Responses(bate)`. No i18n translation for these labels.
- **Backward Compatibility**: All existing channels and entries remain functional. No database migration needed beyond automatic schema updates on first launch.

---

## Known Issues

- Channel AI type selection for relay stations with non-standard paths should use `OpenAI-compatible` (custom) to avoid URL path duplication.
- First-run auto-setup requires a clean (empty) database to trigger.
