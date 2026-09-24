# ADR 002: Model saved albums as source collections

## Context

The original v0.1 scope imported Liked Songs and Spotify playlists. Before Milestone 6 hardening, saved Spotify albums were promoted into the v0.1 source layer and desktop experience.

The existing source model already separates collection membership from Spotify track identity:

- `source_collections` represents imported Spotify collections
- `collection_entries` preserves ordered membership
- `source_tracks` stores shared Spotify track identity and metadata

Adding a separate album-specific persistence hierarchy would duplicate that model and complicate later reconciliation.

## Decision

Represent each saved Spotify album as a normal source collection with:

- `kind = saved_album`
- `provider_collection_id = spotify:album:<spotify-album-id>`
- one ordered collection entry for each album track
- the album save timestamp stored as the entry `added_at` value when Spotify provides it

Album tracks reuse the same `source_tracks` rows as Liked Songs and playlists when their Spotify track IDs match. A saved album therefore adds desired-state membership without creating a second audio identity for the same track.

Saved albums are refreshed from Spotify's saved-albums endpoint using the existing `user-library-read` authorization scope. Unsaved albums remove only the corresponding saved-album collection and entries. Shared source tracks remain available when referenced by another managed collection.

No SQLite migration is required because the existing collection schema already supports additional collection kinds.

Spotify's saved-album payload exposes simplified track objects. When refreshing those tracks, Refrain preserves richer persisted identity data such as an existing ISRC when the simplified response does not contain it.

This decision supersedes the earlier technical-design assumption that source-collection `kind` initially supports only `liked_songs` and `playlist`.

## Reason

This keeps saved albums inside the same collection/track boundary already used throughout Refrain, preserves album order, avoids duplicate source-track identities, and requires the smallest change to the current v0.1 architecture.

## Consequences

- Saved Albums becomes a first-class v0.1 browsing section alongside Liked Songs and Playlists.
- Later local-library reconciliation treats album membership like any other desired-state collection membership.
- Unsaving an album does not imply deleting audio while another managed collection still references the same logical track.
- Album-specific metadata beyond what the current generic collection and track model needs remains out of scope until a concrete product requirement requires it.
