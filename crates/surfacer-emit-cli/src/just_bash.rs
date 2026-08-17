use crate::escaping::escape_ts as escape_js;
use crate::url::base_url;
use surfacer_ir::{
    resolve_auth, AuthMode, CredentialLocation, OperationKind, OperationTransport, RenewalStrategy,
    SecretRef, SiteDescriptor, TokenUse,
};

pub fn emit_executor_config(descriptor: &SiteDescriptor) -> String {
    let site = &descriptor.meta.site_name;
    let display = &descriptor.meta.display_name;
    let source_url = &descriptor.meta.source_url;
    let surface = descriptor.http.as_ref();

    let mut tools = Vec::new();
    // Shell functions the just-bash runtime registers as customCommands. Each
    // authed operation gets one; it resolves the token in shell (env var, an
    // OAuth2 POST piped through `jq`, or the token.json the `auth login` capture
    // baked) and attaches it before the real `curl`.
    let mut shell_fns = Vec::new();

    for op in &descriptor.operations {
        let cmd_name = op.command_path.join(".");
        let description = escape_js(&op.description);
        let op_kind = match op.operation_kind {
            OperationKind::Read => "read",
            OperationKind::Write => "write",
            OperationKind::Other => "other",
        };

        let endpoint = match &op.transport {
            OperationTransport::Http(http) => descriptor
                .http
                .as_ref()
                .and_then(|surface| surface.endpoints.get(http.endpoint_index)),
            OperationTransport::Ax(_) => None,
        };

        let url = match (&op.transport, endpoint) {
            // `path` still carries the recon-time query template
            // (`/user?id={id}`); parameters are supplied at call time, so only
            // the path portion belongs in the base URL.
            (OperationTransport::Http(_), Some(endpoint)) => format!(
                "{}{}",
                base_url(source_url),
                endpoint.path.split('?').next().unwrap_or(&endpoint.path)
            ),
            (OperationTransport::Http(_), None) => format!("{}/unknown", base_url(source_url)),
            (OperationTransport::Ax(_), _) => format!("ax://{site}/{cmd_name}"),
        };

        let params_doc = endpoint
            .map(|e| e.params.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|p| {
                let example = p
                    .example
                    .as_deref()
                    .map(|v| format!(" (e.g. {})", escape_js(v)))
                    .unwrap_or_default();
                format!("      //   {}{}", escape_js(&p.name), example)
            })
            .collect::<Vec<_>>();

        let params_comment = if params_doc.is_empty() {
            String::new()
        } else {
            format!(
                "\n      // Parameters observed during recon:\n{}\n",
                params_doc.join("\n")
            )
        };

        // The auth the IR resolves for this operation: its own override, else
        // the surface default, else None. `None`/no-auth keeps the direct
        // `fetch`; a real mode routes through a generated shell function.
        let auth = resolve_auth(op, surface).filter(|a| !matches!(a, AuthMode::None));

        let request_body = match auth {
            None => "const res = await fetch(url);\n        \
                 return { status: res.status, url, body: await res.text() };"
                .to_string(),
            Some(mode) => {
                let fn_name = shell_fn_name(site, &cmd_name);
                shell_fns.push(emit_shell_auth_fn(&fn_name, site, mode));
                // The just-bash runtime owns `$` command execution; the inline
                // tool shells to the generated function so the token never
                // touches JavaScript. `$` returns the request body on stdout.
                format!(
                    "const res = await $`{fn_name} ${{url}}`;\n        \
                     return {{ status: res.exitCode === 0 ? 200 : res.exitCode, url, body: res.stdout }};"
                )
            }
        };

        tools.push(format!(
            r#"    "{site}.{cmd_name}": {{
      description: "{description}",{params_comment}
      execute: async (args) => {{
        assertAllowed("{site}.{cmd_name}", "{op_kind}");
        const url = withQuery("{url}", args);
        {request_body}
      }},
    }}"#
        ));
    }

    let tools_str = tools.join(",\n");
    let shell_block = if shell_fns.is_empty() {
        String::new()
    } else {
        // `qsep` picks `?` or `&` so a query-located token appends cleanly to a
        // URL that may already carry parameters. Emitted once, shared by every
        // query-location attacher.
        let helper = "qsep() { case \"$1\" in *\\?*) printf '&';; *) printf '?';; esac; }";
        format!(
            "\n/**\n * Shell functions the runtime registers as customCommands.\n \
             * Each attaches the operation's auth in shell before the `curl`, per\n \
             * the auth mode the IR resolved. Exported as a single string so the\n \
             * config stays inspectable and dependency-free.\n */\nexport const surfacerShellAuth = String.raw`\n{helper}\n\n{}\n`;\n",
            shell_fns.join("\n\n")
        )
    };

    format!(
        r#"// Auto-generated by surfacer emit just-bash
// Source: {source_url}
// Site: {site} ({display})
// Generated: {timestamp}
//
// Usage:
//   import {{ Bash }} from "just-bash";
//   import {{ createExecutor }} from "@just-bash/executor";
//   import {{ createSurfacerExecutor }} from "./{site}.config.js";
//
//   const executor = await createSurfacerExecutor(createExecutor);
//   const bash = new Bash({{
//     javascript: {{ invokeTool: executor.invokeTool }},
//     customCommands: executor.commands,
//   }});
//   await bash.exec("{site} --help");

/**
 * Operation kinds allowed to run without explicit opt-in.
 *
 * Inline tools bypass the executor's `onToolApproval` pipeline (the caller owns
 * `execute`), so the trust gate lives here instead.
 */
const ALLOWED_KINDS = new Set(["read"]);

function assertAllowed(path, operationKind) {{
  if (!ALLOWED_KINDS.has(operationKind)) {{
    throw new Error(
      `blocked: ${{path}} is a ${{operationKind}} operation. ` +
        `Add "${{operationKind}}" to ALLOWED_KINDS to permit it.`,
    );
  }}
}}

function withQuery(url, args) {{
  if (!args || typeof args !== "object") return url;
  const entries = Object.entries(args).filter(([, v]) => v !== undefined && v !== null);
  if (entries.length === 0) return url;
  const target = new URL(url);
  for (const [key, value] of entries) {{
    target.searchParams.set(key, String(value));
  }}
  return target.toString();
}}

export const surfacerTools = {{
{tools_str}
}};
{shell_block}
/**
 * Build an executor handle for this site.
 *
 * Takes `createExecutor` as a parameter so this file stays dependency-free and
 * inspectable; the caller supplies the implementation from
 * `@just-bash/executor`.
 */
export async function createSurfacerExecutor(createExecutor, config = {{}}) {{
  return createExecutor({{ ...config, tools: {{ ...surfacerTools, ...(config.tools ?? {{}}) }} }});
}}
"#,
        timestamp = generated_at(),
    )
}

/// A shell-safe function name for an operation's auth attacher.
///
/// `.` and any non-word char become `_` so `sunat.f616.periodo` is a legal
/// bash identifier.
fn shell_fn_name(site: &str, cmd_name: &str) -> String {
    let mut name = format!("__surfacer_auth_{site}_{cmd_name}");
    name = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    name
}

/// Emit the shell function that attaches this operation's auth and runs the
/// real `curl`. One function per authed operation, all three IR modes:
///
/// - `ApiKey`   -> read the secret (env var or baked file) and send it as the header.
/// - `OAuth2`   -> POST the token URL for the grant, pipe the JSON through `jq`
///   for `.access_token`, then `curl` with the resolved header.
/// - `BrowserBootstrappedToken` -> read the token from the `auth login` file
///   (`jq -r .token`), check `.expiresAt` against now, and print the reauth
///   hint + `return 1` when it is missing or stale. Never a browser flow.
///
/// The single argument is the request URL (already query-expanded by the JS
/// tool). `None` never reaches here.
fn emit_shell_auth_fn(fn_name: &str, site: &str, mode: &AuthMode) -> String {
    let body = match mode {
        AuthMode::None => unreachable!("None auth never generates a shell function"),
        AuthMode::ApiKey(api) => emit_api_key_body(api),
        AuthMode::OAuth2(oauth) => emit_oauth2_body(oauth),
        AuthMode::BrowserBootstrappedToken(browser) => emit_browser_token_body(site, browser),
    };
    format!("{fn_name}() {{\n  local url=\"$1\"\n{body}\n}}")
}

/// `ApiKey`: read the secret from its `SecretRef` source, then attach it as the
/// header (or query param) with the optional prefix.
fn emit_api_key_body(api: &surfacer_ir::ApiKeyAuth) -> String {
    let read = read_secret_shell(&api.secret_ref, "token");
    let prefix = api.value_prefix.as_deref().unwrap_or("");
    match api.location {
        CredentialLocation::Header => format!(
            "{read}\n  curl -sS -H \"{name}: {prefix}${{token}}\" \"$url\"",
            name = api.name
        ),
        CredentialLocation::Query => format!(
            "{read}\n  curl -sS \"${{url}}$(qsep \"$url\"){name}={prefix}${{token}}\"",
            name = api.name
        ),
    }
}

/// `OAuth2`: acquire the token headless (POST the token URL for the grant,
/// piped through `jq` for `.access_token`), then attach it per `TokenUse`
/// (default `Authorization: Bearer`).
fn emit_oauth2_body(oauth: &surfacer_ir::OAuth2Auth) -> String {
    let creds = read_secret_shell(&oauth.credentials, "grant");
    let grant_type = match oauth.grant {
        surfacer_ir::OAuth2Grant::Password => "password",
        surfacer_ir::OAuth2Grant::ClientCredentials => "client_credentials",
        surfacer_ir::OAuth2Grant::AuthorizationCode => "authorization_code",
    };
    let scope_arg = if oauth.scopes.is_empty() {
        String::new()
    } else {
        format!(" --data-urlencode \"scope={}\"", oauth.scopes.join(" "))
    };
    let attach = attach_token_shell(oauth.token_use.as_ref());
    format!(
        "{creds}\n  \
         local token\n  \
         token=$(curl -sS -X POST \"{token_url}\" \
--data-urlencode \"grant_type={grant_type}\" \
--data-urlencode \"credentials=${{grant}}\"{scope_arg} | jq -r '.access_token')\n  \
         if [ -z \"$token\" ] || [ \"$token\" = \"null\" ]; then\n    \
         echo \"error: token endpoint returned no access_token\" >&2\n    \
         return 1\n  fi\n  \
         {attach}",
        token_url = oauth.token_url,
    )
}

/// `BrowserBootstrappedToken`: read the token the `auth login` capture baked
/// into `$HOME/.surfacer/sites/<site>/token.json`, expiry-check `.expiresAt`
/// against now (with a 60s skew buffer, matching sunat-cli's `hasFreshToken`),
/// and print the reauth hint + `return 1` when missing or stale. The renewal
/// strategy is always `promptReauth` for this mode; `reacquire` would spin a
/// headless client forever waiting on a human.
fn emit_browser_token_body(
    site: &str,
    browser: &surfacer_ir::BrowserBootstrappedTokenAuth,
) -> String {
    // AU-R9: reacquire is illegal for Mode B; the honest path is a human reauth.
    debug_assert!(matches!(browser.ttl.on_expiry, RenewalStrategy::PromptReauth));
    let attach = attach_token_shell(Some(&browser.use_));
    format!(
        "  local cache=\"$HOME/.surfacer/sites/{site}/token.json\"\n  \
         if [ ! -f \"$cache\" ]; then\n    \
         echo \"run: surfacer auth login {site}\" >&2\n    \
         return 1\n  fi\n  \
         local token expires_at now\n  \
         token=$(jq -r .token < \"$cache\")\n  \
         expires_at=$(jq -r .expiresAt < \"$cache\")\n  \
         now=$(date +%s)\n  \
         if [ -z \"$token\" ] || [ \"$token\" = \"null\" ] || [ \"$expires_at\" -le $((now + 60)) ]; then\n    \
         echo \"run: surfacer auth login {site}\" >&2\n    \
         return 1\n  fi\n  \
         {attach}"
    )
}

/// Shell that resolves a `SecretRef` into the named local variable. Env and
/// file are read directly; `Acquired` cannot be read statically (it is produced
/// by an acquisition step), so it errors honestly.
fn read_secret_shell(secret: &SecretRef, var: &str) -> String {
    match secret {
        SecretRef::Env { var: env } => format!(
            "  local {var}=\"${{{env}}}\"\n  \
             if [ -z \"${{{var}}}\" ]; then\n    \
             echo \"error: set the {env} environment variable\" >&2\n    \
             return 1\n  fi",
        ),
        SecretRef::File { path } => format!(
            "  local {var}\n  \
             {var}=$(cat \"$HOME/{path}\" 2>/dev/null)\n  \
             if [ -z \"${{{var}}}\" ]; then\n    \
             echo \"error: missing secret file $HOME/{path}\" >&2\n    \
             return 1\n  fi",
        ),
        SecretRef::Acquired => format!(
            "  echo \"error: this secret is acquired at runtime, not readable statically\" >&2\n  \
             return 1\n  local {var}=\"\""
        ),
    }
}

/// Shell that attaches an already-resolved token (in `$token`) per `TokenUse`,
/// then runs the real `curl`. Defaults to `Authorization: Bearer <token>` when
/// the IR omits `TokenUse`.
fn attach_token_shell(token_use: Option<&TokenUse>) -> String {
    match token_use {
        None => "curl -sS -H \"Authorization: Bearer ${token}\" \"$url\"".to_string(),
        Some(u) => {
            let prefix = u.value_prefix.as_deref().unwrap_or("");
            match u.location {
                CredentialLocation::Header => {
                    format!("curl -sS -H \"{name}: {prefix}${{token}}\" \"$url\"", name = u.name)
                }
                CredentialLocation::Query => format!(
                    "curl -sS \"${{url}}$(qsep \"$url\"){name}={prefix}${{token}}\"",
                    name = u.name
                ),
            }
        }
    }
}

fn generated_at() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("epoch {secs}")
}
