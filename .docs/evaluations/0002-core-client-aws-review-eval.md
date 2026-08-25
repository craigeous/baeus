# Evaluation: 0002-core-client-aws-review

Verdict: PASS
Round: 0
Reviewed against: cited files under `crates/baeus-core/src/` — `client.rs`, `aws_eks.rs`, `aws_sso.rs`, `auth.rs`, `kubeconfig.rs`, `informer.rs`, `watch.rs`, `cluster.rs`, `logs.rs`.

## Findings

- [MINOR] Strengths bullet cites `aws_eks.rs:66-72` for zeroize-on-drop of `AccessKeyConfig`, which is accurate; the sibling reference to `secrecy::SecretString` at `aws_eks.rs:853` is also accurate — noted here only because Finding #6 later argues the crate should extend `SecretString` usage. The tension between the strength and the finding is consistent (praises the one correct use, criticizes broader plain-`String` use) and not a contradiction.
- [MINOR] Finding #4 lists nine async functions as untested. All function line numbers (237, 262, 298, 348, 432, 493, 530, 828, 724) resolve correctly; `#[tokio::test]` grep returned zero hits in both `aws_eks.rs` and `aws_sso.rs`, confirming the claim mechanically. (`sso_list_account_roles` at 388 is not in the list but is likewise untested — omission from the enumeration is not a defect.)
- [MINOR] Finding #14 mentions the `kind:Config` (no space) case is handled; verified at `kubeconfig.rs:316` (`contents.contains("kind: Config") || contents.contains("kind:Config")`). The note is accurate and appropriately scoped as "Low".

## Required changes (for FAIL)

None — verdict is PASS.

## Notes

Spot-checked citations mechanically:

- Finding #1: `client.rs:1181` (`watch_events`) and `client.rs:1246` (`watch_resources`) both use `kube_runtime::watcher(...).default_backoff()` with `while let Some(...)` loops and no cancellation parameter. Confirmed at lines 1190-1195 and 1281-1289.
- Finding #2: `X-Amz-Expires = "60"` confirmed at `aws_eks.rs:774`. `create_eks_client` at `aws_eks.rs:828-873` calls `generate_eks_token` once and embeds the token in a static `Kubeconfig` with no refresh hook. `ClusterConnection.token_expiry` field exists at `cluster.rs:39`.
- Finding #3: Both inject functions use the double-nested `if let Some(...) = ai.exec` (`aws_sso.rs:80-81`, `aws_sso.rs:139-140`) and reach `Ok(())` unconditionally when the exec block is absent. `create_client_from_path_with_aws_creds` at `client.rs:199` does depend on this injection.
- Finding #5: `tokio::process::Command::new("aws")` confirmed at `aws_sso.rs:25`; `authenticate_with_access_key` uses direct SDK at `client.rs:508-513` (actually `aws_eks.rs:508-513` — the artifact says `client.rs:508-513` which is a minor mislabel: the direct SDK call is in `aws_eks.rs::authenticate_with_access_key`, not `client.rs`. This is a citation-label inaccuracy but the underlying claim — that a direct SDK call exists in the crate — is true.)
- Finding #6: `AccessKeyConfig.secret_access_key: String` at `aws_eks.rs:43`, session_token at 44, zeroize impl at 66-72, `AwsSession.sso_access_token: Option<String>` at 159 — all confirmed.
- Finding #7-8: `InformerState` at `informer.rs:22-29` has no task handle; `watch.rs:44-45` sets Running and inserts into `watcher_ids` without spawning; test at `watch.rs:659` documents the orphan behavior — confirmed.
- Finding #9: `ListParams::default()` at `client.rs:326`, `.limit(100)` for events at `client.rs:327` — confirmed.
- Finding #10-11: Sequential `describe_cluster` at `aws_eks.rs:681`, per-region parallel spawn at `aws_eks.rs:622`, fabricated ARN string at `aws_eks.rs:481` — all confirmed.
- Finding #12-14: `logs.rs:167` `self.streams.remove(index)`, `kubeconfig.rs:521` `unwrap_or_else(|e| e.into_inner())`, `kubeconfig.rs:316-321` string-contains heuristic — all confirmed.

The note is a descriptive/analytical review — it does not smuggle a decision. "Candidate opportunities" is properly framed as suggestions for a downstream ADR/plan author rather than as directives. Format conformance: correct `Status: Research Review` line, has Summary / Strengths / Findings / Candidate opportunities / Citations sections. Citations section lists source files but individual claims are already anchored to specific `file:line` locations throughout the body, satisfying the rubric's per-claim citation requirement.

One minor citation-label glitch (Finding #5 says `client.rs:508-513` where the correct file is `aws_eks.rs:508-513`) does not rise to a blocker or a major — the reader can still locate the referenced code because both the surrounding text and the citations-section entry name `aws_eks.rs` as the home of `authenticate_with_access_key`.
