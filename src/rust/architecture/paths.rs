use std::path::{Path, PathBuf};

use contracts::ArchitectureSectionGroup;
use state::paths::{delivery_dir, workspace_dir, DeliveryPhaseLocator};

pub fn architecture_request_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
) -> PathBuf {
    workspace_dir(project_root, locator)
        .join("architecture-requests")
        .join(format!("{request_id}.json"))
}

pub fn architecture_candidate_file(
    project_root: &Path,
    request_id: &str,
    section: ArchitectureSectionGroup,
) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join(format!("architecture-{}.json", section_name(section)))
}

pub fn architecture_contract_dir(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    delivery_dir(project_root, &locator.delivery_id)
        .join("contracts")
        .join("architecture")
        .join(&locator.phase_id)
}

pub fn architecture_contract_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    architecture_contract_dir(project_root, locator).join("aac.json")
}

pub fn architecture_latest_file(project_root: &Path, locator: &DeliveryPhaseLocator) -> PathBuf {
    architecture_contract_dir(project_root, locator).join("latest.json")
}

pub fn project_api_contract_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    delivery_dir(project_root, delivery_id)
        .join("contracts")
        .join("api")
        .join("current.json")
}

pub fn architecture_section_snapshot_file(
    project_root: &Path,
    locator: &DeliveryPhaseLocator,
    request_id: &str,
    section: ArchitectureSectionGroup,
) -> PathBuf {
    workspace_dir(project_root, locator)
        .join("architecture-sections")
        .join(request_id)
        .join(format!("{}.json", section_name(section)))
}

pub fn section_name(section: ArchitectureSectionGroup) -> &'static str {
    match section {
        ArchitectureSectionGroup::Foundation => "foundation",
        ArchitectureSectionGroup::DomainContract => "domain_contract",
        ArchitectureSectionGroup::Behavior => "behavior",
        ArchitectureSectionGroup::FrontendExperience => "frontend_experience",
        ArchitectureSectionGroup::RuntimeDelivery => "runtime_delivery",
        ArchitectureSectionGroup::Coverage => "coverage",
    }
}
