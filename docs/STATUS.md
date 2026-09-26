# Project Status

## Current State

Milestones 1 through 12 are complete. Milestone 13, Sockseek Sidecar Provider, is implemented locally and is awaiting its provider-level mock-daemon verification plus the required real Soulseek smoke test before release packaging. Refrain v0.1.0 was released on 2026-09-25 as the first Spotify-only desktop release, and the v1 development line now includes the observational Local Library Index, deterministic Matching Engine, Reconciliation Core, Filesystem Normalization and Ownership Safety, Issues and Manual Resolution UI, the provider-neutral Acquisition Provider Boundary, and the initial Sockseek production-provider integration.

The application includes:

- the released v0.1 Spotify authentication, synchronization, persisted browsing, desktop UI, packaging, and release automation
- a configurable observational local-library index with metadata extraction, incremental rescans, lazy hashing, file-state tracking, moved-file recovery, and paginated reads
- `library_tracks` and `local_files` persistence from Milestone 7
- `track_links` and `track_rejections` persistence for matching decisions
- deterministic metadata normalization, bounded candidate generation, compatibility rules, scoring, and automatic/review/unresolved match outcomes
- persisted user confirmations and rejections exposed through Tauri commands and typed frontend wrappers
- `sync_runs` persistence with prepare-through-normalization phase tracking
- one-active-sync coordination with cooperative cancellation at synchronization phase boundaries
- source-to-library reconciliation that reuses persisted links, creates stable `LibraryTrack` records, resolves preferred present files, and classifies desired tracks as matched, missing, or needing review
- canonical filesystem normalization for preferred present files, using Spotify metadata when linked and local canonical metadata when unmatched or local-only
- cross-platform filename sanitization, case-insensitive collision handling, stable collision suffixes, case-only rename staging, and verified copy fallback for cross-filesystem moves
- path-boundary validation plus ownership-preserving moves that keep external files external and never automatically delete external duplicates
- Trash / Recycle Bin integration guarded to managed files for future cleanup work
- paginated Library and Issues views backed by durable local-library and reconciliation state
- issue projections for ambiguous matches, missing local files, inaccessible Spotify collections, and invalid local files
- match-review controls for confirming, rejecting, and clearing persisted manual decisions, with resolved issues disappearing from the projection automatically
- durable `acquisition_jobs` persistence with provider-neutral candidate/job types and a fake-provider-tested acquisition coordinator
- bounded acquisition attempts, cooperative provider cancellation, Refrain-controlled staging directories, and a `staged` terminal state that still requires later verification/import
- acquisition status IPC plus failed acquisition jobs projected into the Issues view
- Sockseek `3.0.5` pinned as the first production acquisition provider, with checksum-verified sidecar fetching for supported release targets
- lazy Sockseek daemon startup on an available loopback port, exact version/readiness checks, owned process shutdown, and credential-redacted sidecar logging
- provider-specific HTTP search/download/status/cancel mapping kept behind the opaque acquisition-provider boundary
- SignalR progress/invalidation wakeups with reconnect behavior while durable HTTP job snapshots remain authoritative
- Soulseek credentials stored in the operating system credential store and materialized only into a restricted transient Sockseek configuration during daemon startup
- Refrain-controlled Sockseek staging output that remains in the durable `staged` acquisition state pending Milestone 14 verification/import
- Settings controls for enabling acquisition, saving/clearing Soulseek credentials, and checking Sockseek provider health
- release packaging support for the Sockseek sidecar plus AGPL license/source notices
- native Library root folder selection through the Tauri dialog plugin
- a fixed-height Issues workspace whose queue and detail panes scroll independently instead of clipping at the supported desktop window sizes
- typed sync-run Tauri commands and frontend wrappers for starting, cancelling, reading, and listing synchronization runs
- separate Local and Spotify primary workspaces with contextual sync actions instead of one mixed source/library navigation model
- scoped `local` / `spotify` sync runs and scope-filtered history queries
- persistent Spotify collection tracking defaults plus per-track include/exclude overrides used as Spotify Sync desired state
- Local library rows restricted to present local music and annotated with matched Spotify memberships
- light, neutral cross-platform desktop shell derived from the approved UI reference set, with persistent Local/Spotify navigation, nested Spotify sections, Issues, and Settings
- responsive Local/Spotify track browsing with fluid desktop tables, compact card rows when the available content width is constrained, artwork, restrained semantic status color, sortable controls, and persisted desktop column order/widths
- artwork-first Saved Albums and Playlists grids that open into collection detail views, while Liked Songs remains track-oriented
- persistent collection and per-track tracking controls in Spotify collection and track views
- search and basic state filtering kept visible while remaining state/technical filters are grouped behind the collapsible Filters control; sortable metadata columns replace redundant metadata filter controls
- Local rows with directly visible file paths and embedded artwork when present
- embedded local artwork extraction with content-hash deduplication into the application-data artwork cache, referenced from `local_files` rather than stored as SQLite blobs
- master-detail Issues workspace and native-style Settings surface using the same shared spacing, control, typography, and semantic-color system

Milestones 7 through 9 remain non-destructive with respect to user audio. Milestone 10 may move confidently resolved preferred files into the canonical library structure, but it preserves each file's ownership classification and does not automatically delete external files.

## Active Work

Milestone 13 implementation is complete locally. Static/build validation is being completed against the pinned sidecar. The remaining milestone verification is Sockseek's documented mock-daemon provider flow plus one real Soulseek acquisition smoke test before an acquisition-enabled release is packaged.

The approved v1 desktop-density and library-browsing refinement is implemented locally. Remaining active work is the Milestone 13 provider verification gate described above.

## Recent Changes

- merged Milestone 9 Reconciliation Core with durable sync runs, one-active-sync coordination, stable source-to-library reconciliation, and cancellation-preserved committed state
- added canonical normalized path construction with single-disc and multi-disc filenames plus unknown album/year fallbacks
- added NFKC filesystem sanitization, Windows reserved-name protection, deterministic component shortening, and case-insensitive collision suffixing
- added atomic rename behavior, case-only rename staging, verified cross-filesystem copy-and-move fallback, and safe path-boundary checks
- preserved preferred-file and managed/external ownership state after moves; external files are never automatically deleted
- added managed-file Trash / Recycle Bin integration for future cleanup workflows
- added Milestone 10 filesystem normalization tests covering the implementation-plan verification cases
- merged Milestone 10 Filesystem Normalization and Ownership Safety
- restored deterministic JavaScript dependency installation for CI with `package-lock.json` and `npm ci`
- added dense Library and Issues desktop views with current issue counts, details, and paginated projections
- added durable issue projections for ambiguous matches, missing/invalid local files, and inaccessible Spotify collections without introducing a separate issue table
- added match-review candidate inspection and persisted confirm/reject/clear actions that refresh the issue projection after resolution
- added frontend and Rust coverage for issue projection behavior and automatic issue removal after durable state repair
- manually verified local-library scanning with 478 indexed files: 477 present, 0 missing, and 1 invalid
- added migration 6 for durable acquisition jobs with one durable job per logical library track
- added provider-neutral acquisition types, health checks, staging, bounded retries, cancellation, status projection, and deterministic fake-provider integration coverage
- kept provider success at `staged`, preserving the later verification/import boundary instead of treating a provider download as synced audio
- projected failed acquisition jobs into Issues and repaired the Issues desktop layout so the queue and detail panes remain independently scrollable
- replaced manual Library root entry with the native operating-system folder picker while retaining explicit Save path / Scan library actions
- manually verified the Milestone 12 Issues layout and Library root folder-picker changes on macOS
- pinned Sockseek `3.0.5` and added deterministic platform-sidecar download/extraction with published SHA-256 verification
- added the Sockseek sidecar manager, loopback runtime port selection, version/readiness checks, HTTP acquisition adapter, SignalR wake/reconnect path, and owned process shutdown
- added OS credential-store persistence for Soulseek credentials with restricted transient Sockseek configuration and secret-redacted sidecar logs
- wired acquisition-enabled synchronization through the production Sockseek provider while preserving `staged` as the pre-verification boundary
- added acquisition Settings controls and third-party AGPL/source notices for distributed Sockseek builds
- fixed the Windows Rust CI Clippy failure by platform-gating the Unix-only `std::io::Write` import used for restricted Sockseek runtime configuration writes
- fixed Sockseek compatibility checks so the pinned `3.0.5` release accepts Sockseek's equivalent `3.0.5.0` server version while still rejecting real version differences
- corrected Sockseek health semantics for v3.0.5: an idle daemon with Soulseek state `None` is ready because the Soulseek client is created and logged in lazily on the first acquisition job; Settings now reflects this and shows configured accounts in green
- clarified acquisition Settings with separate account/configuration and live Sockseek connection chips, plus an explanation that missing-track downloads remain staged for later verification/import
- replaced Sockseek's unhelpful `Soulseek is not ready (None)` health text with a clearer not-ready message when no meaningful client state is reported
- added a visible Run synchronization control in the app header and empty Library state, refreshed Library/Issues/source projections after a run, and decoupled Library/Settings/Issues navigation from Spotify hydration failures
- clarified that local-library scanning indexes physical files while synchronization builds the logical Library shown in the main Library view
- fixed duplicate-key loading skeletons in Library and Issues that could crash Svelte rendering while those views reloaded
- released Refrain v0.1.0 on 2026-09-25

- extended filesystem normalization to organize unmatched and local-only preferred files using scanned local metadata when no accessible Spotify link exists
- split synchronization into Local Sync and Spotify Sync, with Local Sync remaining local-first and Spotify Sync materializing only persisted tracked selections
- added durable source-tracking rules, per-track overrides, scoped sync metadata, and acquisition filtering so stale untracked queued jobs cannot run
- replaced the old Library/Liked Songs/Saved Albums/Playlists primary navigation with Local and Spotify workspaces
- approved the follow-up desktop-density/library-browser design: compact shell and rows, collection grids for albums/playlists, expanded filters with an Advanced technical group, visible local paths, and embedded local artwork when available
- applied the approved `docs/reference/ui-redesign/` visual system across the shell, Local, Spotify, Issues, and Settings views: light neutral surfaces, persistent sidebar navigation, blue primary actions/selection, compact desktop controls, dense data rows, and restrained semantic status color
- completed a UI consistency pass that reuses the Local page's compact at-a-glance metric strip across Spotify and Issues, reduces oversized collection-detail/Settings surfaces, and standardizes shared line icons
- added reusable sortable table headers with drag-to-reorder, drag-to-resize, and per-table local persistence for Local and Spotify column layouts
- fixed horizontally overflowed Local/Spotify tables so row/header styling spans the full computed table width and Spotify headers scroll horizontally with their rows
- replaced horizontal table scrolling with fluid proportional desktop columns and a compact card-row mode when the table container narrows; Local and Spotify now use one canonical `Title / Artist / Album / Year / Duration / Format / Status` column schema with shared persisted order and widths, source-specific `Collections` or `Tracking` columns fixed after it, and Spotify row order/selection folded into the Title cell so shared headers stay aligned; compact rows preserve sorting, selection, tracking, status, metadata, and overflow actions at the 973px minimum application width
- kept Spotify album and playlist detail navigation in session memory so switching pages restores the previously open detail until the user explicitly backs out; closing and reopening the app still starts from the default view
- renamed the Local table's source-membership column to `Playlists` and limited its chips/sorting to actual Spotify playlist memberships, while Spotify matching status continues to account for all accessible Spotify source membership
- disabled the desktop webview developer context menu so normal right-clicks no longer expose Reload or Inspect Element; editable text fields retain their native editing context menu
- made Local collection membership chips width-aware so playlists that would be clipped collapse into a `+N` indicator whose hover hint lists the hidden collections
- persisted Spotify saved-album metadata including album artists, release date, release type, copyrights, Spotify URL, and label when Spotify still supplies it; album details fall back to the copyright rights holder because Spotify removed `label` from Album responses in February 2026
- refined the saved-album detail composition so the left pane keeps only the album title, actions, and compact inline operational status; album metadata and fixed-size artwork live in a fixed-width, non-scrolling inspector anchored below the top toolbar, the profile header stays fixed when the inspector is toggled, and one fixed-size toggle stays at the same detail-workspace position for both open and closed states; the toggle icon crossfades/slides in place and the inspector/main-column transition animates with reduced-motion support; Spotify/cover actions live in the overflow menu, and the redundant single Tracks tab was removed
- applied the same compact detail composition to Spotify playlists, persisted Spotify playlist artwork/external URLs at the collection level, and reused the fixed 244px inspector cover with the same overflow actions as albums
- use playlist artwork directly from `/me/playlists`, which includes user-assigned covers, avoiding one redundant cover request per playlist on every refresh
- replaced the hand-authored UI SVG icon set with on-demand Lucide and Simple Icons components through `unplugin-icons`, keeping one shared `Icon.svelte` interface across the application
- aligned Spotify track metadata with the Local table so Year and Format can be sorted directly, and removed redundant metadata filters from the Local/Spotify advanced filter panels
- replaced Saved Albums and Playlists master-detail browsing with artwork-first collection grids and explicit grid-to-detail/back navigation
- kept search visible while moving state and secondary metadata/technical filters behind a single compact Filters control across Local, Liked Songs, Saved Albums, and Playlists; expanded filter panels now use evenly sized labeled fields, dropdown-based state filters, and a small active-filter count on the trigger
- fixed the primary left sidebar at the current 214 px desktop width at every window size and added narrow-window reflow for headers, metrics, toolbars, filters, collection grids, album/playlist detail controls, and Issues master-detail content; the default window remains 1100×720 with a 973×697 minimum window size
- aligned the Local Library summary and list spacing with Spotify library screens, removed the redundant small track/album/playlist count labels above tables and collection grids, kept the summary strip as the single top-level count source with clearer labels: `Total Tracks`, `Indexed Files`, `Local + Spotify`, and `Local Only`, and moved paginated track-table Load More actions into a centered row at the bottom of the table
- restored direct per-song Spotify tracking controls and row multi-selection; collection-level overflow menus beside the tracking toggle now expose select-all/clear plus one stateful bulk include/exclude action, while album/playlist detail actions live in the Details inspector; selection changes repaint immediately and bulk tracking persists selected source-track overrides in one transactional backend command before refreshing the collection once
- corrected collection tracking semantics so Liked Songs, album, and playlist switches reflect whether the whole collection is effectively tracked; changing a collection switch now atomically clears stale per-track overrides and applies the requested state to the collection, while later per-track include/exclude actions can create fresh exceptions
- added matching Local Library row multi-selection with select-all/clear, bulk file-path copying, and bulk track-name copying; Local and Spotify bulk-action menus now derive their disabled/selected state reactively so selection actions enable immediately as selection changes
- improved Spotify rate-limit handling so long `Retry-After` values are shown as readable durations such as `2 hr 51 min`; Refrain waits automatically only for short limits, remembers the active cooldown in process memory, and rejects repeat Refresh/Sync attempts locally until the deadline expires
- reduced Spotify refresh request volume by reusing `/me/playlists` artwork and `/me/albums` album objects instead of issuing per-playlist cover and per-album detail requests; unchanged inaccessible playlists with the same Spotify snapshot are also reused instead of repeating a guaranteed-forbidden playlist-items request on every refresh, while changed playlists and oversized albums still fetch their required pagination
- aligned Liked Songs, Saved Albums, and Playlists around the same compact local-state rhythm: full collection summaries expose local-copy coverage and the tracked subset without a local copy; `Spotify Only` filters Spotify material absent from disk, while `Needs Local Copy` isolates tracked Spotify material that still lacks a present local file; collection tracking uses compact toggles, while Spotify sync/refresh controls, refresh progress/cancel, compact warnings, and last-sync/last-refresh timestamps live in the sidebar footer above Settings instead of floating over workspace content
- normalized Local and Spotify track rows onto one status vocabulary: `Local + Spotify` means the track exists locally and is linked to accessible Spotify source state, `Local Only` means it exists only in the Local library, `Spotify Only` means it exists in Spotify without a local copy and is not tracked, and `Needs Local Copy` means the tracked Spotify item still lacks a local copy; both track tables now share the same status renderer and status-filter labels, with green for `Local + Spotify`, orange for `Local Only`, and blue for `Spotify Only` and `Needs Local Copy`
- aligned Spotify track-status visuals with Local using the same colored-dot treatment, and lowered Spotify's compact-row cutoff so closing the Details inspector at the default 1100px window returns the track list to the fluid table while narrow windows and an open inspector still use card rows
- preserved the Spotify Details inspector open/closed state across page and collection switches for the current app session, kept Actions pinned as the rightmost table column even when restoring older saved layouts, and added local audio date/year tag persistence so the Local Library Year column comes from the preferred file's embedded metadata when available
- simplified synchronization terminology in the desktop UI: the sidebar now exposes `Sync Library` for the full local-then-Spotify workflow and `Refresh Spotify` for source-only refreshes, while the Local workspace and Settings use `Scan Files` for lightweight local indexing; internal local/Spotify sync scopes remain intact for execution and history
- made overflow menus functional across Local and Spotify track rows and Spotify collection details: Local tracks can reveal/copy their preferred file path, Spotify tracks can open/copy their Spotify link and manage tracking, and album/playlist overflow menus expose Spotify and cover actions
- expanded Spotify per-track overflow actions so every available track can open in Spotify, copy its Spotify link, copy its track/artist name, and toggle repeatedly between included and excluded tracking states with one stateful action; the redundant per-row reset action was removed, and Spotify links fall back to the persisted provider track ID if an external URL is absent
- fixed overflow-menu activation inside scrollable track tables so pointer actions run before focus/scroll can dismiss the fixed menu, while keyboard activation remains supported; synchronous action failures are now caught by the shared menu handler as well
- fixed stale Spotify row-menu tracking availability after an include/exclude update by making row-menu construction explicitly depend on the current per-track and bulk tracking busy state, so the menu action re-enables when the operation finishes
- fixed Spotify list-view row actions by positioning virtualized rows with absolute `top` offsets instead of transforms, preventing each row from trapping the fixed overflow menu in its own stacking context
- grouped Local and Spotify bulk-selection menus into selection controls followed by a visual divider and actions that operate on the selected tracks, keeping `Clear Selection` with the selection controls; Spotify bulk tracking now uses one stateful include/exclude action based on the current selected-track state, with the bulk menu bound directly to `TrackList` selection state so action availability cannot lag behind checkbox changes
- emphasized collection tracking controls with a stronger blue glow while Liked Songs, albums, or playlists are not fully tracked and a restrained glow once enabled; ambiguous icon-only controls in these surfaces now expose immediate native hover hints
- split track selection/order into a dedicated leading table column across Local and Spotify track screens so Title headers align with the actual title text; Spotify labels the leading checkbox/number column `Order`, while Local leaves that selection column unlabeled
- made Spotify `Order` a real sortable column and the default Spotify track sort, so the active sort state and direction indicator live on the Order header instead of being represented as a hidden default alongside Title
- separated Local and Spotify track-table/card headers from their scrolling bodies so the native macOS scrollbar can no longer be composited behind the header; the structure now applies to Local Library plus Spotify Liked Songs, album details, and playlist details in both full table and compact card views
- corrected the user-facing Issues model around the two library/source discrepancies: present local tracks absent from accessible Spotify source state are `Local Only`, while tracked Spotify selections without a confirmed present local file are `Needs Local Copy`; untracked Spotify material no longer inflates issue state, and ambiguous match review is limited to tracked source tracks
- repaired Liked Songs collection tracking after the redesign by binding its toggle and summary metrics to the live loaded collection state; album and playlist tracking labels now use `Untracked` for zero tracked entries and `N/N Tracked` otherwise, detail breadcrumbs include the active collection name, and macOS uses an overlay title bar with the native title hidden
- kept album/playlist detail inspectors flush to the window edge while restoring the normal right content gutter for the detail breadcrumb/account toolbar so the account block no longer clips into the rounded window edge
- persisted the authenticated Spotify profile image URL from `/me` and exposed it through the source overview so Spotify account avatars use the real profile image with the display-name initial as fallback
- restored macOS window dragging after removing the visible title bar by granting the Tauri window-drag capability and wiring the top drag strip to the native `startDragging()` API
- added a macOS-only draggable top inset for the overlay-title-bar window so the app can still be moved from the blank top area while preserving normal content interaction on Windows and Linux
- added visible Local file paths plus embedded-cover extraction, hashed application-data artwork caching, and scoped Tauri asset serving
- added migration `0008_local_artwork.sql` for cached artwork references and MIME metadata on `local_files`
- added migration `0011_source_account_profile_image.sql` for persisted source-account profile artwork

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

Milestone 13 has not yet completed the documented Sockseek mock-daemon integration verification or the real Soulseek download smoke test. Do not treat the provider as release-verified until those checks pass.

## Next

Complete the Milestone 13 Sockseek provider verification using the documented mock daemon and one real Soulseek download smoke test. After the provider passes that gate, proceed to **Milestone 14: Acquisition Verification and Import**.

## Blockers

No implementation blocker is currently documented.

Public code signing and macOS notarization remain credential-dependent and are not required for local unsigned development builds.

## Open Decisions

The saved-album source-model decision is recorded in ADR 002. The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the technical architecture, local-library scanner, matching engine, reconciliation engine, and desktop data flow.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/reference/codebase-map.md` maps the current source structure.
- `docs/reference/setup.md` documents local setup and library-root configuration.
- `docs/reference/testing.md` documents validation coverage, including local-library, matcher, reconciliation, filesystem-normalization, and issue/manual-resolution tests.
- `docs/reference/release.md` documents tag-driven release packaging.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
