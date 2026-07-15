use std::{collections::BTreeSet, fs, path::PathBuf};

use contracts::{
    ArchitectureArtifactContract, ArchitectureDecision, ArchitectureDecisionAlternative,
    ArchitectureDecisionConsequences, ArchitectureDecisionOwnerRefs, ArchitectureNfr,
    ArchitectureNfrMeasurement, ArchitectureNfrOwnerRefs, ArchitectureNfrRefs, ArchitectureQuality,
    ArchitectureQualitySourceRefs, ArchitectureRisk, ArchitectureRiskOwnerRefs,
};

#[test]
fn tech_architecture_references_exist_and_are_operational() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    for item in [
        "core", "patterns", "system", "data", "nfr", "adr", "failure",
    ] {
        let path = repo_root
            .join("plugins/shared/loom/references/tech/arch")
            .join(format!("{item}.md"));
        assert!(
            path.exists(),
            "tech arch reference {item} must resolve to {}",
            path.display()
        );
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let line_count = content.lines().count();
        assert!(
            line_count >= 60,
            "tech arch reference {item} is too thin ({line_count} lines)"
        );
        assert!(
            content.contains("##"),
            "tech arch reference {item} must include structured guidance"
        );
    }
}

#[test]
fn tech_architecture_references_do_not_use_org_capacity_as_decision_factor() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut violations = Vec::new();
    let forbidden_terms = [
        ["head", "count"].concat(),
        ["staff", "ing"].concat(),
        ["team", " size"].concat(),
    ];
    for item in [
        "core", "patterns", "system", "data", "nfr", "adr", "failure",
    ] {
        let path = repo_root
            .join("plugins/shared/loom/references/tech/arch")
            .join(format!("{item}.md"));
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
            .to_ascii_lowercase();
        if forbidden_terms.iter().any(|term| content.contains(term)) {
            violations.push(item);
        }
    }
    assert!(
        violations.is_empty(),
        "architecture references must not use org-capacity wording as a decision factor: {violations:?}"
    );
}

#[test]
fn tech_architecture_references_cover_structural_landing_without_mcp_workflow_duplication() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("plugins/shared/loom/references/tech/arch");
    let expected = [
        ("core.md", ["Architecture Judgment", "Decision Discipline"]),
        (
            "patterns.md",
            ["Hybrid Or Custom Structure", "Pattern Decision Evidence"],
        ),
        (
            "system.md",
            [
                "Context And Trust Boundaries",
                "Capacity And Growth Triggers",
            ],
        ),
        (
            "data.md",
            ["Concurrency And Evolution", "Verification Evidence"],
        ),
        ("nfr.md", ["measurement context", "workload or condition"]),
        (
            "adr.md",
            ["Decision Quality", "At least one positive and one negative"],
        ),
        (
            "failure.md",
            ["state before the failure", "correlation evidence"],
        ),
    ];
    for (file, required) in expected {
        let content = fs::read_to_string(root.join(file)).unwrap();
        for phrase in required {
            assert!(content.contains(phrase), "{file} missing {phrase}");
        }
        for forbidden in [
            "## Repair Ownership",
            "## Output Expectations",
            "MCP",
            "TaskPlan",
            "task plan",
            "read group",
            "repair ownership",
        ] {
            assert!(
                !content.contains(forbidden),
                "{file} duplicates MCP workflow section {forbidden}"
            );
        }
    }
}

#[test]
fn architecture_quality_contract_serializes_required_decision_nfr_and_risk_ids() {
    let quality = ArchitectureQuality {
        decisions: vec![ArchitectureDecision {
            decision_id: "adr-current-001".to_string(),
            category: "architecture_style".to_string(),
            title: "Use current phase modular boundary".to_string(),
            status: "accepted".to_string(),
            context: "The current phase has cohesive stateful business behavior.".to_string(),
            decision: "Use one deployable app with explicit module ownership.".to_string(),
            alternatives_considered: vec![ArchitectureDecisionAlternative {
                name: "service split".to_string(),
                tradeoff: "More runtime isolation with higher consistency and deploy cost."
                    .to_string(),
                rejected_because:
                    "The current phase does not need an independent runtime boundary.".to_string(),
            }],
            consequences: ArchitectureDecisionConsequences {
                positive: vec!["Single transaction boundary for current behavior.".to_string()],
                negative: vec!["Future split requires preserved module interfaces.".to_string()],
                neutral: vec![],
            },
            source_refs: ArchitectureQualitySourceRefs {
                scope_refs: vec!["scope_1".to_string()],
                acceptance_refs: vec!["acc_1".to_string()],
                requirement_detail_refs: vec!["detail_1".to_string()],
            },
            owner_artifact_refs: ArchitectureDecisionOwnerRefs {
                modules: vec!["module_1".to_string()],
                interfaces: vec!["interface_1".to_string()],
            },
            verification_hints: vec!["TaskPlan should assign module-boundary tasks.".to_string()],
        }],
        nfrs: vec![ArchitectureNfr {
            nfr_id: "nfr-current-001".to_string(),
            category: "maintainability".to_string(),
            source: "derived_minimum".to_string(),
            target: "Current phase code keeps domain validation in the owning module.".to_string(),
            rationale: "Later tasks need stable boundaries.".to_string(),
            measurement: ArchitectureNfrMeasurement {
                indicator: "Business validation remains in the owning module.".to_string(),
                workload_or_condition: "Current-phase write operations.".to_string(),
                evaluation_boundary: "Static review and task verification.".to_string(),
            },
            source_refs: ArchitectureQualitySourceRefs {
                scope_refs: vec!["scope_1".to_string()],
                acceptance_refs: vec!["acc_1".to_string()],
                requirement_detail_refs: vec!["detail_1".to_string()],
            },
            architecture_refs: ArchitectureNfrRefs {
                decisions: vec!["adr-current-001".to_string()],
                risks: vec!["risk-current-001".to_string()],
            },
            owner_artifact_refs: ArchitectureNfrOwnerRefs {
                modules: vec!["module_1".to_string()],
                interfaces: vec!["interface_1".to_string()],
            },
            verification_strategy: "Review changed files for boundary ownership.".to_string(),
        }],
        risks: vec![ArchitectureRisk {
            risk_id: "risk-current-001".to_string(),
            category: "maintainability".to_string(),
            severity: "medium".to_string(),
            likelihood: "medium".to_string(),
            impact: "Business rules may drift across UI, API, and persistence.".to_string(),
            mitigation: "Assign validation and persistence tasks to the owning module.".to_string(),
            owner_artifact_refs: ArchitectureRiskOwnerRefs {
                modules: vec!["module_1".to_string()],
                interfaces: vec!["interface_1".to_string()],
                decisions: vec!["adr-current-001".to_string()],
                nfrs: vec!["nfr-current-001".to_string()],
            },
            verification_hints: vec!["TaskResult should cite validation evidence.".to_string()],
        }],
    };

    let value = serde_json::to_value(&quality).expect("architecture quality must serialize");
    let keys = value
        .as_object()
        .expect("architecture quality should be an object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "decisions".to_string(),
            "nfrs".to_string(),
            "risks".to_string()
        ])
    );
    assert_eq!(
        value
            .pointer("/decisions/0/decisionId")
            .and_then(|item| item.as_str()),
        Some("adr-current-001")
    );
    assert_eq!(
        value
            .pointer("/nfrs/0/nfrId")
            .and_then(|item| item.as_str()),
        Some("nfr-current-001")
    );
    assert_eq!(
        value
            .pointer("/risks/0/riskId")
            .and_then(|item| item.as_str()),
        Some("risk-current-001")
    );
}

#[test]
fn legacy_architecture_artifact_without_quality_deserializes_with_empty_default() {
    let value = serde_json::json!({
        "schemaVersion": "1.0",
        "architectureArtifactContractId": "aac-legacy",
        "deliveryId": "delivery-1",
        "phaseId": "phase-1",
        "status": "ready",
        "source": {
            "planningGenerationContractId": "pgc-1",
            "technicalBaselineId": "baseline-1"
        },
        "engineeringBoundary": {},
        "modules": [],
        "dataModel": {},
        "interfaces": [],
        "userFlows": [],
        "stateMachines": [],
        "acceptanceMatrix": [],
        "detailCoverage": [],
        "handoff": {
            "readyForTaskPlan": true,
            "blockingReasons": [],
            "nextNode": "task_plan"
        },
        "createdAt": "2026-06-24T10:00:00+08:00",
        "updatedAt": "2026-06-24T10:00:00+08:00"
    });

    let artifact: ArchitectureArtifactContract =
        serde_json::from_value(value).expect("legacy AAC should deserialize");
    assert!(artifact.architecture_quality.decisions.is_empty());
    assert!(artifact.architecture_quality.nfrs.is_empty());
    assert!(artifact.architecture_quality.risks.is_empty());
}
