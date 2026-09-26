# Refrain UI Screen Inventory

## Purpose

This document is a functional inventory for redesigning Refrain's desktop UI and generating screen concepts with an image model.

It describes what each screen must let the user understand or do. It is not a recommendation to preserve the current visual layout. The redesign can change navigation, hierarchy, component style, spacing, typography, and arrangement as long as the functional requirements remain clear.

Current implementation is derived from the Svelte frontend and current project docs. Planned v1 surfaces are listed separately so redesign work can anticipate features that have not been implemented yet.

## Product Context

Refrain is a desktop application for Windows, macOS, and Linux that keeps an existing local music library aligned with selectively tracked Spotify music.

The two primary workspaces are intentionally different:

- **Local** shows music that physically exists on disk.
- **Spotify** shows imported Spotify state and lets the user choose what Refrain should track and synchronize locally.

Secondary workflows handle issues, settings, acquisition, exports, mirroring, and synchronization status.

## Global Desktop Shell

These elements can appear around multiple screens and should be designed as one coherent shell.

### Primary navigation

Current primary destinations:

- **Local**
  - Shows a local track count.
- **Spotify**
  - Contains Liked Songs, Albums, and Playlists subsections.
  - Shows a collection count.
- **Settings**
- **Issues** is currently a secondary attention flow opened contextually from Local rather than a permanent primary nav item.

### Global header

Important content and actions:

- Refrain product name.
- Application version.
- Short product description.
- Connected Spotify account name when available.
- Last Spotify source refresh time when available.
- **Connect Spotify** shortcut when disconnected.

### Global synchronization status

The shell must support a prominent but compact status area for:

- synchronization currently running
- whether the active run is **Local Sync** or **Spotify Sync**
- short explanation of what the current sync is doing
- **Cancel** action
- synchronization error
- most recent synchronization result
- matched count
- missing count
- needs review count

### Spotify source refresh status

Separate from synchronization, Refrain can refresh imported Spotify state.

The shell must support:

- current refresh message
- completed / total progress when known
- **Cancel** action
- refresh error

### Global backend error

There must be a clear application-level error treatment for backend initialization or IPC failures.

---

# Implemented Screens

## 1. Local Library

### Purpose

Show the user's physical local music collection and how each local track relates to Spotify.

This screen should feel like the main "what I actually own locally" workspace.

### Header

Must show:

- **Local / Your library** identity.
- Short explanation that these tracks are physically on disk.
- **Sync local** primary action.
- **N need attention** action when actionable issues exist.

### Summary information

Important summary values:

- local tracks currently on disk
- total indexed files
- last Local Sync status
- matched count from last Local Sync
- needs review count from last Local Sync

These do not have to remain three cards. They only need to remain quickly readable.

### Main filters

Always available filters:

- search across title, artist, album, path, and Spotify collection membership
- Spotify state
  - all
  - on Spotify
  - local only
- artist
- album
- year
- audio format
- Spotify membership / collection

### Advanced filters

Advanced or secondary controls:

- path contains
- acquisition status
- match status
  - matched
  - unmatched
- explicit state
  - explicit
  - not explicit
- duration
  - under 3 minutes
  - 3 to 5 minutes
  - over 5 minutes
- clear all filters
- shown count and loaded count

### Local track row

Each visible local track needs enough information to answer "what is this file and how does Refrain understand it?"

Important row content:

- embedded local artwork when available
- neutral fallback when artwork is missing
- title
- explicit marker when applicable
- local-only or on-Spotify state
- artist
- album
- release year when known
- duration
- local file format
- local file path
- Spotify collection memberships, such as Liked Songs or playlist names
- overflow treatment when a track belongs to many Spotify collections

### Pagination / loading

Support:

- initial loading skeleton
- load more action
- loaded / total count

### Empty states

Need distinct treatments for:

- no local tracks indexed yet
  - should direct the user to choose a library folder in Settings and run Local Sync
- tracks exist but current filters return no results
- local library loading error

---

## 2. Spotify Workspace: Liked Songs

### Purpose

Show imported Liked Songs and let the user decide which tracks participate in Spotify Sync.

### Spotify workspace header

Shared across all Spotify subsections:

- **Spotify** workspace identity
- explanation that tracking persists across Spotify refreshes
- **Refresh** Spotify source action
- **Sync tracked** primary action
- latest Spotify Sync status
- latest Spotify Sync matched count
- latest Spotify Sync missing count

### Spotify subsection switcher

Must switch among:

- **Liked Songs**
- **Albums**
- **Playlists**

### Liked Songs collection controls

Must show:

- collection name
- total songs
- tracking status
  - not tracked
  - fully tracked
  - partially tracked when individual overrides exist
- **Track all by default** / **Tracking by default** toggle action
- explanation that individual overrides persist

### Track filters

Default filters:

- search title / artist / album
- tracking state
  - all
  - tracked
  - excluded
- local state
  - all
  - on disk
  - missing
- artist
- album
- year
- local audio format

Advanced filters:

- acquisition status
- match state
- explicit state
- duration
- clear filters
- shown / loaded count

### Spotify track row

Important row content:

- playlist / collection position
- Spotify artwork
- title
- explicit marker
- local-present badge when the track already exists locally
- artist
- album
- year
- local format when matched to a local file
- duration
- per-track tracking control
  - **Tracked**
  - **Excluded**
- **Reset** when the track has an explicit override and can return to the collection default

Also support an unavailable Spotify item row with:

- unavailable label
- reason or item type

### Empty / error states

- Spotify has not yet been refreshed, so Liked Songs are unavailable.
- Imported Liked Songs collection contains no tracks.
- Filters produce no results.
- Collection load or tracking update error.

---

## 3. Spotify Workspace: Saved Albums Grid

### Purpose

Browse imported saved albums visually and choose an album to inspect or configure.

### Shared workspace controls

Retain the Spotify header, **Refresh**, **Sync tracked**, subsection switcher, and last Spotify Sync status described above.

### Collection browser controls

Must support:

- search albums by name
- tracking filter
  - all
  - tracked
  - partially tracked
  - untracked
- shown collection count

### Album card

Each album card needs:

- artwork
- fallback artwork treatment
- album name
- track count
- tracking state
  - not tracked
  - tracked
  - X/Y tracked
- clear click affordance to open the album

### Pagination / loading

- load more albums
- loading state
- no albums imported state
- no albums match filters state

---

## 4. Spotify Workspace: Saved Album Detail

### Purpose

Inspect one saved album and manage which tracks Spotify Sync should follow.

### Collection header

Must show:

- clear **Back to Albums** action
- album artwork
- album name
- total track count
- album tracking state
- **Track all by default** / **Tracking by default** action
- note that individual overrides persist across refreshes

### Track area

Uses the Spotify track list behavior from Liked Songs:

- search and filters
- Advanced filters
- track position
- artwork
- title / artist / album / year / duration
- local presence
- local format when known
- per-track Tracked / Excluded toggle
- Reset override
- unavailable item treatment
- load more

### Empty / error states

- collection has no imported tracks
- filters return no tracks
- collection load or tracking update failure

---

## 5. Spotify Workspace: Playlists Grid

### Purpose

Browse accessible Spotify playlists visually, inspect their tracking state, and open a playlist.

### Shared workspace controls

Retain the Spotify header, actions, subsection switcher, and last Spotify Sync status.

### Collection browser controls

Must support:

- search playlists by name
- tracking filter
  - all
  - tracked
  - partially tracked
  - untracked
- shown collection count

### Playlist card

Each accessible playlist card needs:

- artwork
- fallback artwork treatment
- playlist name
- track count
- tracking state
- click affordance to open the playlist

### Pagination

- load more playlists

### Unavailable playlists secondary section

Unavailable or inaccessible playlists must stay out of the normal actionable playlist grid.

Provide a collapsed secondary area containing:

- unavailable playlist count
- playlist name
- access issue / reason Spotify does not expose it to Refrain

This is informational and should not look like a normal actionable issue queue.

### Empty states

- no accessible playlists imported
- no playlists match filters

---

## 6. Spotify Workspace: Playlist Detail

### Purpose

Inspect one accessible Spotify playlist and manage tracking at the collection and individual-track level.

### Collection header

Must show:

- clear **Back to Playlists** action
- playlist artwork
- playlist name
- track count
- tracking state
- **Track all by default** / **Tracking by default** action
- note that individual overrides persist

### Playlist track list

Same functional requirements as the Saved Album detail track list, plus playlist order must remain visible because intentional duplicates and source order matter.

Important:

- track position is meaningful
- duplicate occurrences must remain separate rows even if they resolve to one local file
- per-track tracking overrides apply to each source track identity as supported by the current data model

---

## 7. Issues: Resolution Queue

### Purpose

Provide a focused attention workspace for unresolved durable state that needs user awareness or action.

Issues are derived from current project state. Resolved conditions disappear after the underlying state is fixed and refreshed.

### Issues header

Must show:

- total unresolved actionable conditions
- category counts for:
  - needs review
  - missing local file
  - invalid local file
  - acquisition failed

Inaccessible Spotify collections are intentionally excluded from this actionable queue and shown under Spotify Playlists instead.

### Queue pane

Each issue item needs:

- title
- short subtitle or detail
- issue kind badge
- selected state

Issue kinds currently used by the UI:

- Needs review
- Missing
- Invalid file
- Acquisition failed

The backend also has an inaccessible collection issue kind, but the current UI filters it out here.

### Queue behavior

- independently scrollable queue
- load more
- preserve selected issue when possible after refresh
- automatically select the first actionable issue when necessary

### Detail pane

The right side or equivalent detail surface changes based on issue type.

Must be independently scrollable from the queue.

### Empty / loading / error states

- loading skeleton
- no unresolved issues success state
- issue loading error

---

## 8. Issues: Ambiguous Match Review

### Purpose

Let the user decide which local library track corresponds to an ambiguous Spotify source track.

This is the most information-dense decision screen in the app and should make comparison easy.

### Source track summary

Must show the Spotify source track being resolved:

- title
- artist
- album
- duration
- version kind / version detail when available, such as live, remix, acoustic, remaster, etc.

### Candidate match

Each candidate needs:

- candidate local track title
- artists
- album
- duration
- match confidence / score
- exact ISRC indicator when applicable
- rejected state when previously rejected
- matching warnings
- incompatibilities

### Candidate local files

For every candidate, show linked files with:

- file state
  - present
  - missing
  - invalid
- file path
- preferred-file marker

### Decision actions

For a normal candidate:

- **Reject**
- **Confirm match**

For a previously rejected candidate:

- **Clear rejection**

Decision controls must visibly disable while a decision is being saved.

### Special states

- candidate details loading
- match details no longer available
- candidate with no local files
- decision failure

---

## 9. Issues: Non-Match Issue Detail

### Purpose

Explain unresolved conditions that do not need candidate comparison.

Current examples:

- missing local file
- invalid local file
- acquisition failed

### Detail content

Must support:

- issue kind badge
- title
- subtitle
- detailed explanation
- file path when applicable
- clear explanation that the issue disappears after the underlying state is resolved and Refrain refreshes

Acquisition failure designs should leave room for future provider diagnostics and retry-related actions without exposing unnecessary Sockseek internals.

---

## 10. Settings

### Purpose

Configure external connections, the local library, acquisition, and inspect runtime information.

The current implementation is one Settings screen with multiple sections. A redesign may keep this as one page or split it into Settings subsections, but the following functions must remain discoverable.

### A. Spotify Connection

Must support:

- connection status
- Spotify Client ID field
- registered redirect URI display
- connect action
- disconnect action
- refresh Spotify action when connected
- authentication busy state
- auth error
- last source refresh summary
  - Liked Songs count
  - saved albums count
  - playlists count
  - refreshed playlists
  - unchanged playlists
  - inaccessible playlists when nonzero

Security context worth communicating unobtrusively:

- Client ID is stored locally.
- refresh credentials are stored in the operating system credential store.

### B. Local Library Index

Must support:

- current library root path
- click path field to open native OS folder picker
- **Save path**
- **Scan library**
- scan progress message
- current library overview
  - present
  - missing
  - invalid
  - total indexed
- latest scan summary
  - added
  - updated
  - moved
  - unchanged
- scan / folder-picker error

Important product distinction:

The direct **Scan library** operation indexes the folder. **Local Sync** is the broader reconciliation / normalization workflow.

### C. Acquisition / Sockseek Provider

Must support:

- acquisition enabled toggle
- Soulseek account configuration state
- Sockseek provider health state
  - not checked
  - ready
  - not ready
- Soulseek username
- Soulseek password
- save acquisition setting
- save Soulseek credentials
- check / start Sockseek health verification
- clear credentials
- provider version when available
- provider health message
- acquisition configuration errors

Important user-facing concepts:

- Sockseek is bundled with Refrain.
- Sockseek connects to Soulseek using the user's Soulseek credentials.
- There is no separate Sockseek account.
- credentials are stored in the operating system credential store.
- acquisition finds missing tracks during synchronization.
- downloads are staged and require verification before becoming canonical library files.

### D. Runtime Information

Read-only diagnostic content:

- application name
- application version
- application data directory
- persisted Spotify source counts
  - liked songs
  - saved albums
  - playlists

This information can be a quiet secondary panel rather than a visually dominant card.

---

# External / Native Flows

These are part of the user journey but should not be redesigned as fake in-app screens unless the product intentionally replaces them later.

## 11. Native Library Folder Picker

Triggered from Settings when choosing the Library root.

This is the operating system's native directory picker.

Refrain only needs a clear trigger and good return state after a folder is selected or the dialog is cancelled.

## 12. Spotify Authorization Browser Flow

Triggered by **Connect Spotify**.

Authentication happens through Spotify authorization and returns to Refrain through the registered local callback URI.

Useful in-app states around this external flow:

- waiting for Spotify
- connected
- authorization failed
- callback port unavailable

---

# Cross-Screen States to Design Once

These states recur across the product and should share one design language.

## Loading

- page / workspace skeleton
- track row skeletons
- collection grid loading
- issue detail loading
- runtime info loading

## Empty

- nothing imported yet
- no local library indexed yet
- collection contains no tracks
- no issues
- filters return zero results

## Errors

- backend unavailable
- Spotify hydration failure
- source refresh failure
- synchronization failure
- local scan failure
- collection load failure
- tracking update failure
- issue decision failure
- provider unavailable / acquisition error

## Busy / disabled actions

Buttons need understandable disabled states during:

- Spotify auth
- Spotify source refresh
- Local Sync
- Spotify Sync
- library scan
- tracking changes
- match decisions
- acquisition settings updates
- Sockseek health checks

## Status vocabulary

Keep the following user-facing states visually distinct but consistent:

- tracked
- excluded
- partially tracked
- on disk / local
- local only
- on Spotify
- missing
- queued
- downloading
- needs review
- failed
- invalid
- unavailable
- staged
- provider ready / not ready

---

# Planned v1 Screens and Surfaces

These features are in the approved v1 product plan but are not fully implemented in the current frontend. Designs generated now should account for them.

## 13. Acquisition Verification / Import State

### Purpose

Show what happens after a provider successfully downloads a candidate but before it becomes part of the canonical library.

Important future states:

- queued
- searching
- downloading
- staged
- verifying
- verification failed
- verified / importing
- imported

Potential functional information:

- source track
- candidate file
- provider-neutral acquisition status
- verification result
- reason verification failed
- retry or resolve through Issues when appropriate

The UI should not require users to understand Sockseek's internal API or daemon model.

## 14. Playlist Export

### Purpose

Export a fully resolved Spotify playlist for use outside Refrain.

### Export modes

Support two explicit modes:

1. **M3U8 playlist file**
   - references existing local library files
   - preserves source order
   - preserves intentional duplicate playlist entries

2. **Portable playlist bundle**
   - contains an M3U8 playlist
   - contains copied audio required by the playlist
   - deduplicates copied audio when the same recording appears multiple times
   - uses paths that remain valid when the bundle is moved or extracted

### Required UI concepts

- choose playlist
- show whether all supported entries are resolved
- unresolved count / export precondition failure
- choose export mode
- choose destination
- run export
- export progress / status
- success result and output path
- failure result
- export history
- timestamp
- exported playlist
- mode
- status

Partial exports must not be presented as successful when supported playlist entries remain unresolved.

## 15. Library Mirror Targets

### Purpose

Configure and run portable mirrors of the normalized local library and resolved playlists to another mounted filesystem location, usually an external drive.

### Target management

Must eventually support:

- list mirror targets
- add mirror target
- edit target
- remove target
- target path / mounted location
- target availability
- last mirror status / time

### Mirror run

Important information:

- target
- progress
- files to copy / update / remove
- completed operations
- result status
- disconnected target state
- write conflict / failure state

### Safety message

The interface should clearly communicate that Refrain only removes paths it manages on the mirror. Unrelated files already on the destination are preserved.

## 16. Synchronization Scheduling and Removal Policy

### Purpose

Configure automatic sync behavior while Refrain is running.

Expected settings:

- sync on application startup
- periodic sync enabled / disabled
- periodic sync interval
- presentation of next or current scheduled state if useful
- behavior when a scheduled trigger occurs during an active sync
- managed-track removal policy
  - keep managed audio after it disappears from all tracked Spotify collections
  - allow safe removal when no managed collection references it

Important safety distinction:

- unmanaged pre-existing local files are never automatically deleted
- external files remain protected
- managed-file cleanup uses the OS Trash / Recycle Bin path

## 17. Sync Activity / Recent Runs

The current UI only exposes compact current and recent sync summaries. v1 UX calls for clearer recent sync state.

A redesign should leave room for a richer activity surface showing:

- Local vs Spotify scope
- trigger
  - manual
  - startup
  - scheduled
- status
- start / finish time
- current phase
- matched
- missing
- needs review
- failed acquisition
- cancellation
- partial success / errors when implemented

This can be a dedicated screen, drawer, popover, or expanded section. The exact navigation is not fixed yet.

---

# Recommended Image Generation Set

For a complete UI redesign, generate at least these distinct desktop concepts:

1. Global shell + Local Library populated state
2. Local Library with Advanced filters open
3. Spotify Liked Songs
4. Spotify Saved Albums grid
5. Spotify Saved Album detail
6. Spotify Playlists grid with Unavailable section expanded
7. Spotify Playlist detail
8. Issues queue with Ambiguous Match Review selected
9. Issues queue with a non-match issue selected
10. Settings overview showing Spotify, Local Library, Acquisition, and Runtime hierarchy
11. Playlist Export future screen
12. Mirror Targets / Mirror Progress future screen
13. Scheduling / Removal Policy future settings
14. Global active synchronization / progress treatment
15. Empty, loading, error, and no-results component states as a small state sheet

The same visual system should be used across all generated screens so they can become one coherent desktop product instead of isolated mockups.

## Functional Priorities for the Redesign

When simplifying the current UI, preserve these distinctions:

1. **Local and Spotify are different workspaces.** Local represents files that exist on disk. Spotify represents source state and tracking intent.
2. **Refresh Spotify and Sync tracked are different operations.** Refresh imports Spotify state. Spotify Sync reconciles tracked selections into the local library workflow.
3. **Scan library and Local Sync are different operations.** Scan indexes physical files. Local Sync performs the broader reconciliation / normalization workflow.
4. **Collection tracking has defaults plus per-track overrides.** Users need to understand both without excessive controls in every row.
5. **Issues are actionable state, not a generic notifications page.** Match review needs strong comparison UX.
6. **Unavailable Spotify playlists are informational.** They should remain separated from actionable local / acquisition issues.
7. **Acquisition provider details should stay secondary.** Users care whether acquisition is configured, running, staged, verified, failed, or unavailable more than Sockseek implementation details.
8. **File paths and ownership matter.** Refrain performs real filesystem work, so paths, local presence, and safety-related state cannot be hidden completely.
9. **Dense lists are intentional.** This is a desktop library management tool, but density should come from hierarchy and efficient components rather than tiny unreadable text.
10. **Future Export and Mirror workflows need space in the information architecture.** Avoid a redesign that only works for the current Milestone 13 feature set.
