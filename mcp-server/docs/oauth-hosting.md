# Host Pline MCP with OAuth

This runs the existing Streamable HTTP server at `https://<host>/mcp`. Users sign in with their
Pline account, choose a workspace and active API key, and approve the AI client. Each MCP request
uses that user's Supabase OAuth token to resolve the selected key through the Pline web app. The MCP
deployment has no shared Pline API key.

## Configure the Pline web app

1. Apply `supabase/migrations/20261008000000_mcp_connections.sql` from the
   `pline-hades-api-platform` repository to the production Supabase database.
2. In Supabase Authentication → OAuth Server, enable the OAuth 2.1 server and dynamic client
   registration. Set the authorization UI path to `/oauth/consent`.
3. Set the Supabase Site URL to the public Pline app origin and allow its OAuth callback/redirect
   domains as required by your project settings.
4. Deploy the updated `pline-hades-api-platform` so `/oauth/consent` and
   `/api/mcp/credential` are available.

## Configure the MCP host

Run the released Docker image with the Pline API origin and platform origin. Set the three OAuth
environment variables to the production values:

```sh
docker run --rm -p 8080:8080 \
  -e PLINE_BASE_URL=https://apix.pline.ai/v1 \
  -e PLINE_PLATFORM_BASE_URL=https://app.pline.ai \
  -e PLINE_SUPABASE_URL=https://<project-ref>.supabase.co \
  -e PLINE_MCP_PUBLIC_URL=https://<mcp-host> \
  -e PLINE_MCP_ALLOWED_HOSTS=<mcp-host> \
  897722692245.dkr.ecr.us-east-1.amazonaws.com/pline-mcp:<release>
```

`PLINE_PLATFORM_BASE_URL` must reach the deployed Pline web app. `PLINE_SUPABASE_URL` is the
Supabase project origin (without `/auth/v1`). `PLINE_MCP_PUBLIC_URL` is the public origin of this
server. The server advertises Supabase OAuth discovery through the protected-resource metadata route
and challenges unauthenticated MCP clients with `WWW-Authenticate`.

The endpoint requires HTTPS and should be exposed through the platform's public gateway. Do not
configure `PLINE_API_KEY` on this shared deployment. Local stdio installs continue using each user's
key in their agent configuration.

## Connect an agent

Use the URL `https://<mcp-host>/mcp` and select OAuth when the client asks for authentication. During
the Pline authorization step, the user selects an active workspace API key. The stored selection is
scoped to the signed-in Pline user and that OAuth client. The API-key plaintext is returned only to
the hosted MCP process and is not included in tool output.

Users can change or disconnect a saved choice at `https://app.pline.ai/mcp/connections`. A revoked
key fails through the normal Pline API key validation.

## Production rollout

1. Apply the `pline-mcp` ECR repository change in `pline-hades-infra`.
2. Apply the Supabase migration and enable OAuth server plus dynamic registration.
3. Release the updated `pline-hades-api-platform` with the consent and credential routes.
4. Publish the Pline MCP `v0.2.0` release, then run **Deploy Pline MCP to production** with that tag
   to build and push the image into ECR.
5. Merge the `pline-mcp` production app values in `pline-hades-gitops`; Argo CD then deploys the
   image at `https://mcp.prd.pline.ai/mcp`.

The MCP deploy workflow only publishes the ECR image. The GitOps change is the deployment action.
