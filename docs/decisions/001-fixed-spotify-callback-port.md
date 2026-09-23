# 001: Use a fixed Spotify loopback callback port

## Context

Refrain originally used a dynamically selected loopback port and documented the portless redirect URI `http://127.0.0.1/callback`.

Spotify's public redirect-URI guidance describes dynamic loopback ports, but the developer-dashboard configuration encountered during Refrain's real authentication setup rejected the portless redirect URI. Refrain needs the URI configured in the developer application and the URI sent during authorization to match reliably.

## Decision

Refrain uses this fixed Spotify OAuth redirect URI:

```text
http://127.0.0.1:43817/callback
```

During Spotify authorization, the desktop app binds only to `127.0.0.1:43817`.

Automated authentication tests may inject an ephemeral callback port so tests remain parallel-safe. This does not change the production redirect URI.

## Reason

A fixed callback URI is explicit, works with the developer-dashboard configuration used for Refrain, and prevents the registered URI from differing from the URI used at runtime.

## Consequences

- users must register the exact URI above in their Spotify developer application
- port `43817` must be available while authorization starts
- if the port is occupied, Refrain reports an error instead of choosing another production port
- this decision supersedes the earlier dynamic-port design described in `docs/TDD.md` and the Milestone 3 wording in `docs/IMPLEMENTATION.md`
