use contracts::{
    DeployProvider, DeploymentProviderCandidate, DeploymentProviderCandidateStatus,
    DeploymentProviderPolicy, DeploymentSourceModel,
};

use crate::{code_evidence::DeploymentCodeProbe, existing::ExistingDeploymentFiles};

#[derive(Debug, Clone)]
pub struct DeploymentStrategy {
    pub provider: DeployProvider,
    pub reason: String,
    pub policy: DeploymentProviderPolicy,
    pub candidates: Vec<DeploymentProviderCandidate>,
}

pub fn resolve_deployment_strategy(
    code_probe: &DeploymentCodeProbe,
    source_model: &DeploymentSourceModel,
    existing: &ExistingDeploymentFiles,
    policy: Option<DeploymentProviderPolicy>,
) -> DeploymentStrategy {
    let policy = normalize_provider_policy(policy);
    let provider = select_provider(source_model, existing, &policy);
    let reason = reason_for(provider, code_probe, source_model, existing, &policy);
    let candidates = provider_candidates(provider, code_probe, source_model, existing, &policy);
    DeploymentStrategy {
        provider,
        reason,
        policy,
        candidates,
    }
}

pub fn normalize_provider_policy(
    policy: Option<DeploymentProviderPolicy>,
) -> DeploymentProviderPolicy {
    let Some(mut policy) = policy else {
        return DeploymentProviderPolicy::default();
    };
    if policy.force_generate {
        policy.provider = Some(DeployProvider::Generated);
        policy.reuse_existing = false;
    }
    policy
}

fn select_provider(
    source_model: &DeploymentSourceModel,
    existing: &ExistingDeploymentFiles,
    policy: &DeploymentProviderPolicy,
) -> DeployProvider {
    if policy.force_generate {
        return DeployProvider::Generated;
    }
    if let Some(provider) = policy.provider {
        return provider;
    }
    if !policy.reuse_existing {
        return DeployProvider::Generated;
    }
    if existing.compose_path.is_some() {
        return DeployProvider::ComposeExisting;
    }
    if existing.dockerfile_path.is_some() && source_model.services.len() <= 1 {
        return DeployProvider::DockerfileExisting;
    }
    DeployProvider::Generated
}

fn reason_for(
    provider: DeployProvider,
    code_probe: &DeploymentCodeProbe,
    source_model: &DeploymentSourceModel,
    existing: &ExistingDeploymentFiles,
    policy: &DeploymentProviderPolicy,
) -> String {
    if policy.force_generate {
        return "Provider policy forces generated Dockerfile/Compose assets.".to_string();
    }
    if let Some(forced) = policy.provider {
        return format!(
            "Provider policy explicitly selected {}.",
            provider_label(forced)
        );
    }
    if !policy.reuse_existing {
        return "Provider policy disables existing deployment asset reuse.".to_string();
    }
    match provider {
        DeployProvider::ComposeExisting => {
            "Root-level Compose file exists, so Loom will try it before generated fallback."
                .to_string()
        }
        DeployProvider::DockerfileExisting => {
            "Root-level Dockerfile exists, so Loom will reuse it with a generated Compose wrapper."
                .to_string()
        }
        DeployProvider::Generated => {
            if existing.dockerfile_path.is_some() && source_model.services.len() > 1 {
                "Existing root Dockerfile cannot represent multiple application services; generated deployment assets are safer.".to_string()
            } else {
                format!(
                    "Repository probes found {:?} runtime evidence, so Loom will generate deployment assets.",
                    code_probe.kind
                )
            }
        }
    }
}

fn provider_candidates(
    selected: DeployProvider,
    code_probe: &DeploymentCodeProbe,
    source_model: &DeploymentSourceModel,
    existing: &ExistingDeploymentFiles,
    policy: &DeploymentProviderPolicy,
) -> Vec<DeploymentProviderCandidate> {
    [
        DeployProvider::ComposeExisting,
        DeployProvider::DockerfileExisting,
        DeployProvider::Generated,
    ]
    .into_iter()
    .map(|provider| DeploymentProviderCandidate {
        provider,
        status: candidate_status(provider, selected, source_model, existing, policy),
        reason: candidate_reason(provider, code_probe, source_model, existing, policy),
        commands: candidate_commands(provider),
    })
    .collect()
}

fn candidate_status(
    provider: DeployProvider,
    selected: DeployProvider,
    source_model: &DeploymentSourceModel,
    existing: &ExistingDeploymentFiles,
    policy: &DeploymentProviderPolicy,
) -> DeploymentProviderCandidateStatus {
    if provider == selected {
        return DeploymentProviderCandidateStatus::Selected;
    }
    if policy.force_generate && provider != DeployProvider::Generated {
        return DeploymentProviderCandidateStatus::Skipped;
    }
    if policy.provider.is_some_and(|forced| forced != provider) {
        return DeploymentProviderCandidateStatus::Skipped;
    }
    match provider {
        DeployProvider::ComposeExisting => existing
            .compose_path
            .as_ref()
            .map(|_| DeploymentProviderCandidateStatus::Available)
            .unwrap_or(DeploymentProviderCandidateStatus::Skipped),
        DeployProvider::DockerfileExisting => {
            if existing.dockerfile_path.is_none() || source_model.services.len() > 1 {
                DeploymentProviderCandidateStatus::Skipped
            } else {
                DeploymentProviderCandidateStatus::Available
            }
        }
        DeployProvider::Generated => DeploymentProviderCandidateStatus::Available,
    }
}

fn candidate_reason(
    provider: DeployProvider,
    code_probe: &DeploymentCodeProbe,
    source_model: &DeploymentSourceModel,
    existing: &ExistingDeploymentFiles,
    policy: &DeploymentProviderPolicy,
) -> String {
    if policy.force_generate && provider != DeployProvider::Generated {
        return "Skipped because provider policy forces generated deployment assets.".to_string();
    }
    if let Some(forced) = policy.provider {
        if forced != provider {
            return format!(
                "Skipped because provider policy explicitly selected {}.",
                provider_label(forced)
            );
        }
    }
    match provider {
        DeployProvider::ComposeExisting => existing
            .compose_path
            .as_ref()
            .map(|_| "Existing Compose file found at deployment root.".to_string())
            .unwrap_or_else(|| "No root-level Compose file was found.".to_string()),
        DeployProvider::DockerfileExisting => {
            if existing.dockerfile_path.is_none() {
                "No root-level Dockerfile was found.".to_string()
            } else if source_model.services.len() > 1 {
                "Skipped because one root Dockerfile cannot represent multiple application services.".to_string()
            } else {
                "Existing Dockerfile found at deployment root.".to_string()
            }
        }
        DeployProvider::Generated => format!(
            "Available because Loom can model {:?} runtime evidence as generated local deployment assets.",
            code_probe.kind
        ),
    }
}

fn candidate_commands(provider: DeployProvider) -> Vec<Vec<String>> {
    match provider {
        DeployProvider::ComposeExisting => {
            vec![vec![
                "docker".to_string(),
                "compose".to_string(),
                "config".to_string(),
                "--quiet".to_string(),
            ]]
        }
        DeployProvider::DockerfileExisting | DeployProvider::Generated => vec![vec![
            "docker".to_string(),
            "compose".to_string(),
            "up".to_string(),
            "-d".to_string(),
            "--build".to_string(),
        ]],
    }
}

fn provider_label(provider: DeployProvider) -> &'static str {
    match provider {
        DeployProvider::ComposeExisting => "compose-existing",
        DeployProvider::DockerfileExisting => "dockerfile-existing",
        DeployProvider::Generated => "generated",
    }
}
