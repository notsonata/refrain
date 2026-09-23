# Spotify Authentication

Refrain uses Spotify Authorization Code with PKCE. It does not require or store a Spotify client secret.

## Developer Application Setup

1. Create a Spotify developer application and copy its Client ID.
2. Add this exact redirect URI to the application:

   ```text
   http://127.0.0.1:43817/callback
   ```

3. Enter the Client ID in Refrain and choose **Connect Spotify**.

Refrain binds its temporary OAuth callback listener to `127.0.0.1:43817` during authorization. The redirect URI sent to Spotify and the URI registered in the Spotify developer application are therefore identical.

If another application is already using port `43817`, Refrain cannot start the Spotify callback listener. Close the application using that port and try connecting again.

## Credential Handling

- the Spotify Client ID is persisted in Refrain's SQLite settings
- the Spotify refresh credential is stored in the operating system credential store
- Spotify access tokens remain in memory only
- PKCE verifiers and OAuth state values are generated for each authorization attempt and are not persisted
- authentication secrets and authorization headers must not be written to logs

The initial scopes are:

```text
user-library-read
playlist-read-private
playlist-read-collaborative
```

If Spotify rejects a stored refresh credential as expired or revoked, Refrain clears it and requires the user to connect again.

## Manual Authentication Smoke Test

Milestone 3 requires one real authentication smoke test before it is considered complete:

1. Configure a Spotify developer application with the redirect URI above.
2. Start Refrain and enter the application's Client ID.
3. Choose **Connect Spotify** and complete authorization in the system browser.
4. Confirm Refrain reports **Connected**.
5. Restart Refrain and confirm it still reports **Connected** without repeating full authorization.
6. Disconnect and confirm Refrain reports **Not connected**.
7. Connect again and confirm authorization succeeds.

This smoke test requires a real user-owned Spotify developer application and is therefore performed manually outside automated CI.
