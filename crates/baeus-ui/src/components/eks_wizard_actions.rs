//! EKS Wizard async actions — `impl AppShell` methods for SSO flow, discovery, and connection.

use crate::components::eks_wizard::{EksWizardState, EksWizardStep};
use crate::layout::app_shell::AppShell;
use crate::layout::sidebar::{
    ClusterEntry, ClusterStatus, generate_cluster_color, generate_initials,
};
use baeus_core::aws_eks::{self, AwsAuthMethod, EksAuthState};
use baeus_core::cluster::{AuthMethod, ClusterConnection};
use gpui::*;
use gpui_component::input::{InputEvent, InputState};
use std::collections::HashSet;

impl AppShell {
    /// Open the EKS wizard modal.
    pub(crate) fn open_eks_wizard(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.eks_wizard = Some(EksWizardState::new());
        cx.notify();
    }

    /// Navigate back one step in the wizard.
    pub(crate) fn eks_wizard_go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref mut wizard) = self.eks_wizard else {
            return;
        };
        wizard.error = None;
        // Clear old inputs when changing step
        wizard.inputs.clear();
        wizard._input_subs.clear();

        wizard.step = match &wizard.step {
            EksWizardStep::SsoConfig => EksWizardStep::ChooseAuthMethod,
            EksWizardStep::SsoDeviceAuth => EksWizardStep::SsoConfig,
            EksWizardStep::SsoAccountSelection => EksWizardStep::SsoConfig,
            EksWizardStep::SsoRoleSelection => EksWizardStep::SsoAccountSelection,
            EksWizardStep::AssumeIamRole => EksWizardStep::SsoRoleSelection,
            EksWizardStep::AccessKeyConfig => EksWizardStep::ChooseAuthMethod,
            EksWizardStep::AssumeRoleConfig => EksWizardStep::ChooseAuthMethod,
            EksWizardStep::RegionSelection => match wizard.auth_method {
                AwsAuthMethod::Sso => EksWizardStep::AssumeIamRole,
                AwsAuthMethod::AccessKey => EksWizardStep::AccessKeyConfig,
                AwsAuthMethod::AssumeRole => EksWizardStep::AssumeRoleConfig,
            },
            EksWizardStep::ClusterResults => EksWizardStep::RegionSelection,
            EksWizardStep::ChooseAuthMethod | EksWizardStep::Discovering => return,
        };

        self.ensure_eks_inputs(window, cx);
        cx.notify();
    }

    /// Advance the wizard to the next step.
    pub(crate) fn eks_wizard_advance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Sync input values back to wizard state before advancing
        self.sync_eks_inputs(cx);

        let Some(ref wizard) = self.eks_wizard else {
            return;
        };

        let step = wizard.step.clone();
        let auth_method = wizard.auth_method.clone();

        if let Some(ref mut w) = self.eks_wizard {
            w.error = None;
            // Clear inputs when leaving a step
            w.inputs.clear();
            w._input_subs.clear();
            w.filter_text.clear();
        }

        match step {
            EksWizardStep::ChooseAuthMethod => {
                if let Some(ref mut w) = self.eks_wizard {
                    w.step = match auth_method {
                        AwsAuthMethod::Sso => EksWizardStep::SsoConfig,
                        AwsAuthMethod::AccessKey => EksWizardStep::AccessKeyConfig,
                        AwsAuthMethod::AssumeRole => EksWizardStep::AssumeRoleConfig,
                    };
                }
                self.ensure_eks_inputs(window, cx);
                cx.notify();
            }
            EksWizardStep::SsoConfig => {
                self.eks_start_sso_flow(cx);
            }
            EksWizardStep::SsoAccountSelection => {
                self.eks_load_roles_for_account(cx);
            }
            EksWizardStep::SsoRoleSelection => {
                self.eks_get_sso_role_credentials(cx);
            }
            EksWizardStep::AssumeIamRole => {
                self.eks_assume_iam_role(cx);
            }
            EksWizardStep::AccessKeyConfig => {
                self.eks_authenticate_access_key(cx);
            }
            EksWizardStep::AssumeRoleConfig => {
                self.eks_assume_role(cx);
            }
            EksWizardStep::RegionSelection => {
                self.eks_discover_clusters(cx);
            }
            EksWizardStep::ClusterResults => {
                self.eks_connect_selected_clusters(cx);
            }
            _ => {}
        }
    }

    /// Create GPUI Input entities for the current wizard step's text fields.
    pub(crate) fn ensure_eks_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref wizard) = self.eks_wizard else {
            return;
        };

        // Determine which fields this step needs
        let fields: Vec<(&str, &str, String)> = match wizard.step {
            EksWizardStep::SsoConfig => vec![
                ("sso_start_url", "https://my-org.awsapps.com/start", wizard.sso_start_url.clone()),
                ("sso_region", "us-east-1", wizard.sso_region.clone()),
            ],
            EksWizardStep::AccessKeyConfig => vec![
                ("access_key_id", "AKIAIOSFODNN7EXAMPLE", wizard.access_key_id.clone()),
                ("secret_access_key", "wJalrXUtnFEMI/...", wizard.secret_access_key.clone()),
                ("session_token", "(optional)", wizard.session_token.clone()),
                ("access_key_region", "us-east-1", wizard.access_key_region.clone()),
            ],
            EksWizardStep::AssumeRoleConfig => vec![
                ("role_arn", "arn:aws:iam::123456789:role/MyRole", wizard.role_arn.clone()),
                ("external_id", "(optional)", wizard.external_id.clone()),
                ("assume_role_region", "us-east-1", wizard.assume_role_region.clone()),
            ],
            EksWizardStep::SsoAccountSelection => {
                vec![("account_filter", "Filter accounts...", String::new())]
            }
            EksWizardStep::SsoRoleSelection => {
                vec![("role_filter", "Filter roles...", String::new())]
            }
            EksWizardStep::AssumeIamRole => vec![(
                "iam_role_arn",
                "arn:aws:iam::123456789012:role/RoleName",
                wizard.iam_role_arn.clone(),
            )],
            EksWizardStep::ClusterResults => {
                vec![("cluster_filter", "Filter clusters...", String::new())]
            }
            _ => return,
        };

        for (name, placeholder, initial_value) in fields {
            // Skip if already created
            if self.eks_wizard.as_ref().map(|w| w.inputs.contains_key(name)).unwrap_or(false) {
                continue;
            }

            // Secret fields should be masked (password-style input)
            let is_secret = matches!(name, "secret_access_key" | "session_token");

            let input = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder(placeholder);
                if is_secret {
                    state = state.masked(true);
                }
                if !initial_value.is_empty() {
                    state.set_value(initial_value, window, cx);
                }
                state
            });

            let field_name = name.to_string();
            let sub =
                cx.subscribe(&input, move |this: &mut AppShell, entity, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let val = entity.read(cx).value().to_string();
                        if let Some(ref mut w) = this.eks_wizard {
                            match field_name.as_str() {
                                "sso_start_url" => w.sso_start_url = val,
                                "sso_region" => w.sso_region = val,
                                "access_key_id" => w.access_key_id = val,
                                "secret_access_key" => w.secret_access_key = val,
                                "session_token" => w.session_token = val,
                                "access_key_region" => w.access_key_region = val,
                                "role_arn" => w.role_arn = val,
                                "external_id" => w.external_id = val,
                                "assume_role_region" => w.assume_role_region = val,
                                "account_filter" | "role_filter" | "cluster_filter" => {
                                    w.filter_text = val
                                }
                                "iam_role_arn" => w.iam_role_arn = val,
                                _ => {}
                            }
                        }
                        cx.notify();
                    }
                });

            if let Some(ref mut w) = self.eks_wizard {
                w.inputs.insert(name.to_string(), input);
                w._input_subs.push(sub);
            }
        }

        // Special handling: create per-cluster role inputs for ClusterResults step
        if matches!(self.eks_wizard.as_ref().map(|w| &w.step), Some(EksWizardStep::ClusterResults))
        {
            let Some(ref wizard) = self.eks_wizard else { return };
            let default_role = wizard.iam_role_arn.clone();
            let cluster_fields: Vec<(String, String)> = wizard
                .selected_cluster_indices
                .iter()
                .filter(|idx| !wizard.inputs.contains_key(&format!("cluster_role_{idx}")))
                .map(|&idx| {
                    let existing = wizard
                        .per_cluster_roles
                        .get(&idx)
                        .cloned()
                        .unwrap_or_else(|| default_role.clone()); // Pre-fill with role from Assume IAM Role step
                    (format!("cluster_role_{idx}"), existing)
                })
                .collect();

            for (name, initial_value) in cluster_fields {
                let input = cx.new(|cx| {
                    let mut state = InputState::new(window, cx)
                        .placeholder("arn:aws:iam::123456789:role/role-name");
                    if !initial_value.is_empty() {
                        state.set_value(initial_value, window, cx);
                    }
                    state
                });

                let field_name = name.clone();
                let sub = cx.subscribe(
                    &input,
                    move |this: &mut AppShell, entity, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            let val = entity.read(cx).value().to_string();
                            if let Some(ref mut w) = this.eks_wizard {
                                if let Some(idx_str) = field_name.strip_prefix("cluster_role_") {
                                    if let Ok(idx) = idx_str.parse::<usize>() {
                                        if val.is_empty() {
                                            w.per_cluster_roles.remove(&idx);
                                        } else {
                                            w.per_cluster_roles.insert(idx, val);
                                        }
                                    }
                                }
                            }
                            cx.notify();
                        }
                    },
                );

                if let Some(ref mut w) = self.eks_wizard {
                    w.inputs.insert(name, input);
                    w._input_subs.push(sub);
                }
            }
        }
    }

    /// Read current input values back into wizard state fields.
    fn sync_eks_inputs(&mut self, cx: &mut Context<Self>) {
        let Some(ref mut wizard) = self.eks_wizard else {
            return;
        };
        for (name, entity) in &wizard.inputs {
            let val = entity.read(cx).value().to_string();
            match name.as_str() {
                "sso_start_url" => wizard.sso_start_url = val,
                "sso_region" => wizard.sso_region = val,
                "access_key_id" => wizard.access_key_id = val,
                "secret_access_key" => wizard.secret_access_key = val,
                "session_token" => wizard.session_token = val,
                "access_key_region" => wizard.access_key_region = val,
                "role_arn" => wizard.role_arn = val,
                "external_id" => wizard.external_id = val,
                "assume_role_region" => wizard.assume_role_region = val,
                "iam_role_arn" => wizard.iam_role_arn = val,
                other if other.starts_with("cluster_role_") => {
                    if let Some(idx_str) = other.strip_prefix("cluster_role_") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if val.is_empty() {
                                wizard.per_cluster_roles.remove(&idx);
                            } else {
                                wizard.per_cluster_roles.insert(idx, val);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Assume an IAM role using the SSO session credentials.
    /// This is for the SSO → AssumeRole → EKS pattern where the SSO role
    /// doesn't have direct k8s access.
    fn eks_assume_iam_role(&mut self, cx: &mut Context<Self>) {
        let Some(ref wizard) = self.eks_wizard else { return };

        // If no role ARN provided, skip (use SSO credentials directly)
        if wizard.iam_role_arn.trim().is_empty() {
            if let Some(ref mut w) = self.eks_wizard {
                w.step = EksWizardStep::RegionSelection;
            }
            cx.notify();
            return;
        }

        let Some(ref session) = wizard.session else {
            if let Some(ref mut w) = self.eks_wizard {
                w.error = Some("No active session — complete SSO auth first.".to_string());
            }
            cx.notify();
            return;
        };

        let role_arn = wizard.iam_role_arn.trim().to_string();
        let source_creds = session.credentials.clone();
        let region = session.region.clone();
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    let config = baeus_core::aws_eks::AssumeRoleConfig {
                        role_arn,
                        external_id: None,
                        session_name: Some("baeus-eks".to_string()),
                        region,
                    };
                    baeus_core::aws_eks::assume_role(&config, &source_creds).await
                })
                .await;
            match result {
                Ok(Ok(assumed_session)) => {
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            // Preserve original SSO creds for per-cluster role assumption
                            if w.original_sso_credentials.is_none() {
                                if let Some(ref sso_session) = w.session {
                                    w.original_sso_credentials =
                                        Some(sso_session.credentials.clone());
                                }
                            }
                            // Replace session with the assumed role's session
                            w.session = Some(assumed_session);
                            w.step = EksWizardStep::RegionSelection;
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    let msg = format!("AssumeRole failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    let msg = format!("AssumeRole task failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Start the SSO device-code flow: register client, start device auth, open browser.
    fn eks_start_sso_flow(&mut self, cx: &mut Context<Self>) {
        let Some(ref mut wizard) = self.eks_wizard else {
            return;
        };

        let start_url = wizard.sso_start_url.trim().to_string();
        let region = wizard.sso_region.trim().to_string();
        wizard.step = EksWizardStep::SsoDeviceAuth;
        wizard.auth_state = EksAuthState::WaitingForBrowser;
        cx.notify();

        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let region2 = region.clone();
            let register_result = tokio_handle
                .spawn(async move { aws_eks::sso_register_client(&region2).await })
                .await;

            let (client_id, client_secret) = match register_result {
                Ok(Ok(pair)) => pair,
                Ok(Err(e)) => {
                    let msg = format!("SSO registration failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                            w.step = EksWizardStep::SsoConfig;
                        }
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                Err(e) => {
                    let msg = format!("SSO registration failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                            w.step = EksWizardStep::SsoConfig;
                        }
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            let r = region.clone();
            let cid = client_id.clone();
            let csec = client_secret.clone();
            let surl = start_url.clone();
            let device_result = tokio_handle
                .spawn(async move { aws_eks::sso_start_device_auth(&r, &cid, &csec, &surl).await })
                .await;

            let device_auth = match device_result {
                Ok(Ok(da)) => da,
                Ok(Err(e)) => {
                    let msg = format!("Device authorization failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                            w.step = EksWizardStep::SsoConfig;
                        }
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                Err(e) => {
                    let msg = format!("Device authorization failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                            w.step = EksWizardStep::SsoConfig;
                        }
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            let browser_url = device_auth
                .verification_uri_complete
                .clone()
                .unwrap_or_else(|| device_auth.verification_uri.clone());
            let _ = open::that(&browser_url);

            let poll_interval = device_auth.poll_interval;
            let device_code = device_auth.device_code.clone();

            this.update(cx, |this, cx| {
                if let Some(ref mut w) = this.eks_wizard {
                    w.sso_client_id = Some(client_id.clone());
                    w.sso_client_secret = Some(client_secret.clone());
                    w.sso_device_auth = Some(device_auth);
                    w.auth_state = EksAuthState::PollingForToken;
                }
                cx.notify();
            })
            .ok();

            loop {
                // Sleep on tokio runtime
                let pi = poll_interval;
                let _ = tokio_handle.spawn(async move { tokio::time::sleep(pi).await }).await;

                let r = region.clone();
                let cid = client_id.clone();
                let csec = client_secret.clone();
                let dc = device_code.clone();
                let poll_result = tokio_handle
                    .spawn(async move { aws_eks::sso_poll_for_token(&r, &cid, &csec, &dc).await })
                    .await;

                match poll_result {
                    Ok(Ok(aws_eks::SsoTokenResult::Pending)) => {}
                    Ok(Ok(aws_eks::SsoTokenResult::Success { access_token, .. })) => {
                        let r = region.clone();
                        let at = access_token.clone();
                        let accounts_result = tokio_handle
                            .spawn(async move { aws_eks::sso_list_accounts(&r, &at).await })
                            .await;

                        match accounts_result {
                            Ok(Ok(accounts)) => {
                                this.update(cx, |this, cx| {
                                    if let Some(ref mut w) = this.eks_wizard {
                                        w.sso_access_token = Some(access_token);
                                        let mut sorted = accounts;
                                        sorted.sort_by(|a, b| {
                                            a.account_name
                                                .as_deref()
                                                .unwrap_or("")
                                                .to_lowercase()
                                                .cmp(
                                                    &b.account_name
                                                        .as_deref()
                                                        .unwrap_or("")
                                                        .to_lowercase(),
                                                )
                                        });
                                        w.sso_accounts = sorted;
                                        w.step = EksWizardStep::SsoAccountSelection;
                                        w.auth_state = EksAuthState::SelectingAccount;
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                            _ => {
                                this.update(cx, |this, cx| {
                                    if let Some(ref mut w) = this.eks_wizard {
                                        w.error = Some("Failed to list SSO accounts".to_string());
                                        w.step = EksWizardStep::SsoConfig;
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                        }
                        return;
                    }
                    Ok(Ok(aws_eks::SsoTokenResult::Denied(msg))) => {
                        this.update(cx, |this, cx| {
                            if let Some(ref mut w) = this.eks_wizard {
                                w.error = Some(format!("Authorization denied: {msg}"));
                                w.step = EksWizardStep::SsoConfig;
                            }
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    Ok(Err(e)) => {
                        // SDK error during polling — could be transient, but report it
                        let msg = format!("SSO token polling error: {e}");
                        this.update(cx, |this, cx| {
                            if let Some(ref mut w) = this.eks_wizard {
                                w.error = Some(msg);
                                w.step = EksWizardStep::SsoConfig;
                            }
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    Err(e) => {
                        let msg = format!("Token polling task failed: {e}");
                        this.update(cx, |this, cx| {
                            if let Some(ref mut w) = this.eks_wizard {
                                w.error = Some(msg);
                                w.step = EksWizardStep::SsoConfig;
                            }
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                }
            }
        })
        .detach();
    }

    fn eks_load_roles_for_account(&mut self, cx: &mut Context<Self>) {
        let Some(ref wizard) = self.eks_wizard else { return };
        let Some(ref account) = wizard.sso_selected_account else { return };
        let Some(ref token) = wizard.sso_access_token else { return };
        let region = wizard.sso_region.clone();
        let token = token.clone();
        let account_id = account.account_id.clone();
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    aws_eks::sso_list_account_roles(&region, &token, &account_id).await
                })
                .await;
            match result {
                Ok(Ok(mut roles)) => {
                    roles.sort_by(|a, b| {
                        a.role_name.to_lowercase().cmp(&b.role_name.to_lowercase())
                    });
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.sso_roles = roles;
                            w.step = EksWizardStep::SsoRoleSelection;
                        }
                        cx.notify();
                    })
                    .ok();
                }
                _ => {
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some("Failed to list roles".to_string());
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn eks_get_sso_role_credentials(&mut self, cx: &mut Context<Self>) {
        let Some(ref wizard) = self.eks_wizard else { return };
        let Some(ref account) = wizard.sso_selected_account else { return };
        let Some(ref role) = wizard.sso_selected_role else { return };
        let Some(ref token) = wizard.sso_access_token else { return };
        let region = wizard.sso_region.clone();
        let token = token.clone();
        let account_id = account.account_id.clone();
        let role_name = role.role_name.clone();
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    aws_eks::sso_get_role_credentials(&region, &token, &account_id, &role_name)
                        .await
                })
                .await;
            match result {
                Ok(Ok(session)) => {
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.session = Some(session);
                            w.step = EksWizardStep::AssumeIamRole;
                        }
                        cx.notify();
                    })
                    .ok();
                }
                _ => {
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some("Failed to get credentials".to_string());
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn eks_authenticate_access_key(&mut self, cx: &mut Context<Self>) {
        let Some(ref wizard) = self.eks_wizard else { return };
        let config = baeus_core::aws_eks::AccessKeyConfig {
            access_key_id: wizard.access_key_id.trim().to_string(),
            secret_access_key: wizard.secret_access_key.trim().to_string(),
            session_token: if wizard.session_token.trim().is_empty() {
                None
            } else {
                Some(wizard.session_token.trim().to_string())
            },
            region: wizard.access_key_region.trim().to_string(),
        };
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move { aws_eks::authenticate_with_access_key(&config).await })
                .await;
            match result {
                Ok(Ok(session)) => {
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.session = Some(session);
                            w.step = EksWizardStep::RegionSelection;
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    let msg = format!("Authentication failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    let msg = format!("Authentication failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn eks_assume_role(&mut self, cx: &mut Context<Self>) {
        let Some(ref wizard) = self.eks_wizard else { return };
        let config = baeus_core::aws_eks::AssumeRoleConfig {
            role_arn: wizard.role_arn.trim().to_string(),
            external_id: if wizard.external_id.trim().is_empty() {
                None
            } else {
                Some(wizard.external_id.trim().to_string())
            },
            session_name: None,
            region: wizard.assume_role_region.trim().to_string(),
        };
        let region = config.region.clone();
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    let default_config =
                        aws_config::defaults(aws_config::BehaviorVersion::latest())
                            .region(aws_types::region::Region::new(region))
                            .load()
                            .await;
                    let source_creds = match default_config.credentials_provider() {
                        Some(provider) => {
                            use aws_credential_types::provider::ProvideCredentials;
                            match provider.provide_credentials().await {
                                Ok(c) => aws_credential_types::Credentials::new(
                                    c.access_key_id(),
                                    c.secret_access_key(),
                                    c.session_token().map(|s| s.to_string()),
                                    c.expiry(),
                                    "baeus-source",
                                ),
                                Err(e) => {
                                    return Err(anyhow::anyhow!("No source credentials: {e}"));
                                }
                            }
                        }
                        None => return Err(anyhow::anyhow!("No AWS credentials found")),
                    };
                    aws_eks::assume_role(&config, &source_creds).await
                })
                .await;
            match result {
                Ok(Ok(session)) => {
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.session = Some(session);
                            w.step = EksWizardStep::RegionSelection;
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    let msg = format!("AssumeRole failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    let msg = format!("AssumeRole failed: {e}");
                    this.update(cx, |this, cx| {
                        if let Some(ref mut w) = this.eks_wizard {
                            w.error = Some(msg);
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn eks_discover_clusters(&mut self, cx: &mut Context<Self>) {
        let Some(ref mut wizard) = self.eks_wizard else { return };
        let Some(ref session) = wizard.session else {
            wizard.error = Some("No active session — authenticate first.".to_string());
            cx.notify();
            return;
        };
        let credentials = session.credentials.clone();
        let regions: Vec<String> = wizard.selected_regions.iter().cloned().collect();
        let total = regions.len();
        wizard.step = EksWizardStep::Discovering;
        wizard.discovery_progress = (0, total);
        wizard.discovered_clusters.clear();
        wizard.selected_cluster_indices.clear();
        cx.notify();
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    aws_eks::discover_eks_clusters(&credentials, &regions, |_, _| {}).await
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(ref mut w) = this.eks_wizard {
                    match result {
                        Ok(Ok(clusters)) => {
                            // Don't auto-select — let user choose which clusters to connect
                            w.discovery_progress = (total, total);
                            w.discovered_clusters = clusters;
                            w.step = EksWizardStep::ClusterResults;
                        }
                        Ok(Err(e)) => {
                            w.error = Some(format!("Discovery failed: {e}"));
                            w.step = EksWizardStep::RegionSelection;
                        }
                        Err(e) => {
                            w.error = Some(format!("Discovery failed: {e}"));
                            w.step = EksWizardStep::RegionSelection;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn eks_connect_selected_clusters(&mut self, cx: &mut Context<Self>) {
        let Some(ref wizard) = self.eks_wizard else { return };
        let Some(ref session) = wizard.session else { return };

        // Extract persistence info while wizard is borrowed.
        let auth_method_str = match wizard.auth_method {
            baeus_core::aws_eks::AwsAuthMethod::Sso => "Sso",
            baeus_core::aws_eks::AwsAuthMethod::AccessKey => "AccessKey",
            baeus_core::aws_eks::AwsAuthMethod::AssumeRole => "AssumeRole",
        };
        let sso_url = if !wizard.sso_start_url.trim().is_empty() {
            Some(wizard.sso_start_url.trim().to_string())
        } else {
            None
        };
        let sso_region = if !wizard.sso_region.trim().is_empty() {
            Some(wizard.sso_region.trim().to_string())
        } else {
            None
        };

        // Build list of (cluster, role_arn_for_this_cluster)
        // If a per-cluster role matches the role already assumed in the session
        // (wizard.iam_role_arn), skip re-assumption — the session creds already
        // have that identity.
        let already_assumed_role = wizard.iam_role_arn.trim().to_string();
        let selected: Vec<_> = wizard
            .selected_cluster_indices
            .iter()
            .filter_map(|&idx| {
                let cluster = wizard.discovered_clusters.get(idx)?.clone();
                let role = wizard
                    .per_cluster_roles
                    .get(&idx)
                    .filter(|r| !r.trim().is_empty())
                    .cloned()
                    .or_else(|| {
                        if already_assumed_role.is_empty() {
                            None
                        } else {
                            Some(already_assumed_role.clone())
                        }
                    });
                // If the per-cluster role is the same as the already-assumed role,
                // treat it as "no role" — the session already has those credentials.
                let effective_role = role.clone().filter(|r| r.trim() != already_assumed_role);
                tracing::info!(
                    "EKS wizard: cluster '{}' (region={}) — role: {:?} (already_assumed: '{}')",
                    cluster.name,
                    cluster.region,
                    effective_role,
                    already_assumed_role,
                );
                // (cluster, effective_role for live connection, full_role for persistence)
                Some((cluster, effective_role, role))
            })
            .collect();
        let base_credentials = session.credentials.clone();
        let base_region = session.region.clone();
        // For per-cluster role assumption, use original SSO creds (before step 2 assumption)
        // so we can assume any role, not just roles chainable from the step 2 role.
        let sso_credentials =
            wizard.original_sso_credentials.clone().unwrap_or_else(|| base_credentials.clone());

        for (cluster, role_arn, _full_role) in &selected {
            let context_name = aws_eks::eks_context_name(cluster);
            let display_name = format!("{} ({})", cluster.name, cluster.region);
            let conn = ClusterConnection::new(
                cluster.name.clone(),
                context_name.clone(),
                cluster.endpoint.clone(),
                AuthMethod::AwsEks,
            );
            let cluster_id = self.cluster_manager.add_connection(conn);

            // If a per-cluster role needs to be assumed, show Connecting immediately
            // so the user doesn't click the cluster and trigger a premature connection
            // with the wrong (base) credentials.
            let initial_status = if role_arn.is_some() {
                ClusterStatus::Connecting
            } else {
                ClusterStatus::Disconnected
            };
            let entry = ClusterEntry {
                id: cluster_id,
                context_name: context_name.clone(),
                display_name,
                initials: generate_initials(&cluster.name),
                color: generate_cluster_color(&context_name),
                status: initial_status,
                expanded: false,
                sections: Vec::new(),
                expanded_categories: HashSet::new(),
                custom_icon_path: None,
                source: crate::layout::sidebar::ClusterSource::AwsEks {
                    region: cluster.region.clone(),
                    account_id: None,
                },
            };
            self.sidebar.clusters.push(entry);

            // If there's a per-cluster role, we need to assume it first.
            // Do NOT store in eks_cluster_data yet — only store once we have the
            // correct (assumed) credentials. This prevents handle_connect_cluster
            // from being called with stale base credentials if the user clicks
            // the cluster before the async role assumption completes.
            if let Some(role) = role_arn {
                let tokio_handle =
                    cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();
                let creds = sso_credentials.clone(); // Use original SSO creds, not step-2 assumed creds
                let role = role.clone();
                let region = base_region.clone();
                let ctx_name = context_name.clone();
                let cluster_for_data = cluster.clone();

                tracing::info!("EKS wizard: assuming role '{}' for cluster '{}'", role, ctx_name,);

                cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
                    // First assume the role
                    let assumed_role = role.clone();
                    let result = tokio_handle
                        .spawn(async move {
                            let config = baeus_core::aws_eks::AssumeRoleConfig {
                                role_arn: role,
                                external_id: None,
                                session_name: Some("baeus-eks".to_string()),
                                region,
                            };
                            baeus_core::aws_eks::assume_role(&config, &creds).await
                        })
                        .await;

                    match result {
                        Ok(Ok(assumed_session)) => {
                            tracing::info!(
                                "EKS wizard: role assumption succeeded for '{}', identity={}",
                                ctx_name,
                                assumed_session.identity_arn,
                            );
                            // Store the assumed credentials and trigger connection
                            this.update(cx, |this, cx| {
                                this.eks_cluster_data.insert(
                                    ctx_name.clone(),
                                    (
                                        cluster_for_data,
                                        assumed_session.credentials,
                                        Some(assumed_role),
                                    ),
                                );
                                // Now trigger the standard connection with correct creds
                                this.handle_connect_cluster(&ctx_name, cx);
                            })
                            .ok();
                        }
                        Ok(Err(e)) => {
                            let msg = format!("AssumeRole failed for {ctx_name}: {e}");
                            tracing::error!("{msg}");
                            this.update(cx, |this, cx| {
                                Self::set_sidebar_cluster_status(
                                    &mut this.sidebar,
                                    &ctx_name,
                                    ClusterStatus::Error,
                                );
                                this.connection_errors.insert(ctx_name.clone(), msg);
                                cx.notify();
                            })
                            .ok();
                        }
                        Err(e) => {
                            let msg = format!("AssumeRole task failed: {e}");
                            tracing::error!("{msg}");
                            this.update(cx, |this, cx| {
                                Self::set_sidebar_cluster_status(
                                    &mut this.sidebar,
                                    &ctx_name,
                                    ClusterStatus::Error,
                                );
                                this.connection_errors.insert(ctx_name.clone(), msg);
                                cx.notify();
                            })
                            .ok();
                        }
                    }
                })
                .detach();
            } else {
                // No additional role assumption needed — session creds already have the right identity.
                // Store the already-assumed role ARN for kubeconfig generation (terminal use).
                let kubeconfig_role = if !already_assumed_role.is_empty() {
                    Some(already_assumed_role.clone())
                } else {
                    None
                };
                self.eks_cluster_data.insert(
                    context_name.clone(),
                    (cluster.clone(), base_credentials.clone(), kubeconfig_role),
                );
                self.handle_connect_cluster(&context_name, cx);
            }
        }

        // Persist the connected EKS clusters so they survive app restart.
        // Use full_role (not effective_role) so --role-arn is always in the kubeconfig.
        for (cluster, _effective_role, full_role) in &selected {
            use crate::views::preferences::SavedEksConnectionInfo;
            let info = SavedEksConnectionInfo {
                cluster_name: cluster.name.clone(),
                cluster_arn: cluster.arn.clone(),
                endpoint: cluster.endpoint.clone(),
                region: cluster.region.clone(),
                certificate_authority_data: cluster.certificate_authority_data.clone(),
                auth_method: auth_method_str.to_string(),
                sso_start_url: sso_url.clone(),
                sso_region: sso_region.clone(),
                role_arn: full_role.clone(),
            };
            // Avoid duplicates (by ARN)
            self.preferences.saved_eks_connections.retain(|c| c.cluster_arn != info.cluster_arn);
            self.preferences.saved_eks_connections.push(info);
        }
        self.save_preferences();

        // Reconnect any previously connected EKS clusters that may have been
        // disrupted by the SSO auth flow (new SSO token can invalidate old session).
        let existing_eks: Vec<String> = self
            .sidebar
            .clusters
            .iter()
            .filter(|c| c.context_name.starts_with("eks:") && c.status == ClusterStatus::Error)
            .map(|c| c.context_name.clone())
            .collect();
        for ctx in existing_eks {
            self.handle_connect_cluster(&ctx, cx);
        }

        self.eks_wizard = None;
        cx.notify();
    }
}
