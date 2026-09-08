use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Shared lifecycle objects used to connect TaskResult, Review, V-SEFM, and
/// repair outcomes without changing the existing delivery routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationLifecycleStatus {
    Pending,
    Checked,
    Passed,
    Failed,
    Blocked,
    RepairPending,
    Reverified,
    CompletedWithNotes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisCompleteness {
    Complete,
    Incomplete,
}

/// The two operational failure classes needed by the delivery loop. A code
/// failure can be repaired in the repository; an environment block needs a
/// dependency or runtime action before verification can continue.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VerificationFailureClass {
    CodeFailure,
    EnvironmentBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationAssertion {
    pub assertion_id: String,
    pub target: String,
    #[serde(default)]
    pub preconditions: Vec<String>,
    pub expected_result: String,
    #[serde(default)]
    pub evidence_requirements: Vec<String>,
    pub source_ref: String,
    pub contract_version: String,
    pub status: VerificationLifecycleStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationEvidenceRecord {
    pub evidence_id: String,
    #[serde(default)]
    pub assertion_refs: Vec<String>,
    pub source: String,
    pub location: String,
    pub observed_result: String,
    pub captured_at: String,
}

impl VerificationEvidenceRecord {
    pub fn is_locatable(&self) -> bool {
        !self.location.trim().is_empty()
    }

    pub fn is_complete(&self) -> bool {
        self.is_locatable()
            && !self.source.trim().is_empty()
            && !self.observed_result.trim().is_empty()
            && !self.captured_at.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationFinding {
    pub finding_id: String,
    pub severity: String,
    pub failure_owner: String,
    pub classification: VerificationFailureClass,
    pub message: String,
    #[serde(default)]
    pub affected_assertion_refs: Vec<String>,
    pub remediation: String,
}

impl VerificationFinding {
    pub fn is_environment_blocked(&self) -> bool {
        matches!(
            self.classification,
            VerificationFailureClass::EnvironmentBlocked
        )
    }

    pub fn is_code_failure(&self) -> bool {
        matches!(self.classification, VerificationFailureClass::CodeFailure)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepairContract {
    pub repair_id: String,
    #[serde(default)]
    pub finding_refs: Vec<String>,
    #[serde(default)]
    pub affected_assertion_refs: Vec<String>,
    pub repair_scope: String,
    #[serde(default)]
    pub required_checks: Vec<String>,
    pub source_contract_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_contract_version: Option<String>,
    pub status: VerificationLifecycleStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationAttempt {
    pub attempt_id: String,
    pub attempt_number: u32,
    #[serde(default)]
    pub assertion_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub source_version: String,
    pub contract_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_ref: Option<String>,
    pub status: VerificationLifecycleStatus,
    pub started_at: String,
    pub completed_at: String,
}

impl VerificationAttempt {
    pub fn is_verified(&self, evidence: &[VerificationEvidenceRecord]) -> bool {
        self.is_completion_eligible(evidence)
    }

    fn has_complete_evidence_for_assertions(
        &self,
        evidence: &[VerificationEvidenceRecord],
    ) -> bool {
        !self.assertion_refs.is_empty()
            && !self.evidence_refs.is_empty()
            && self.assertion_refs.iter().all(|assertion_ref| {
                self.evidence_refs.iter().any(|evidence_ref| {
                    evidence.iter().any(|item| {
                        item.evidence_id == *evidence_ref
                            && item.is_complete()
                            && item
                                .assertion_refs
                                .iter()
                                .any(|item_assertion| item_assertion == assertion_ref)
                    })
                })
            })
            && self.evidence_refs.iter().all(|evidence_ref| {
                evidence.iter().any(|item| {
                    item.evidence_id == *evidence_ref
                        && item.is_complete()
                        && item
                            .assertion_refs
                            .iter()
                            .any(|assertion_ref| self.assertion_refs.contains(assertion_ref))
                })
            })
    }

    pub fn can_pass(&self, evidence: &[VerificationEvidenceRecord]) -> bool {
        matches!(
            self.status,
            VerificationLifecycleStatus::Checked
                | VerificationLifecycleStatus::Passed
                | VerificationLifecycleStatus::Reverified
        ) && !self.source_version.trim().is_empty()
            && !self.contract_version.trim().is_empty()
            && !self.started_at.trim().is_empty()
            && !self.completed_at.trim().is_empty()
            && self.has_complete_evidence_for_assertions(evidence)
    }

    pub fn is_completion_eligible(&self, evidence: &[VerificationEvidenceRecord]) -> bool {
        matches!(
            self.status,
            VerificationLifecycleStatus::Passed | VerificationLifecycleStatus::Reverified
        ) && self.can_pass(evidence)
    }

    pub fn is_fresh_for_repair(&self, repair: &RepairContract) -> bool {
        self.repair_ref.as_deref() == Some(repair.repair_id.as_str())
            && self.contract_version == repair.source_contract_ref
            && !self.evidence_refs.is_empty()
            && repair
                .affected_assertion_refs
                .iter()
                .all(|assertion_ref| self.assertion_refs.contains(assertion_ref))
    }
}

/// Append-only verification history owned by the persistence boundary.
///
/// This is intentionally a contract-level guard rather than a storage
/// implementation. State writers can use it to reject duplicate or stale
/// attempts before persisting a new record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationHistory {
    #[serde(default)]
    pub attempts: Vec<VerificationAttempt>,
    #[serde(default)]
    pub repairs: Vec<RepairContract>,
    #[serde(default)]
    pub evidence: Vec<VerificationEvidenceRecord>,
}

impl VerificationHistory {
    pub fn append_evidence(
        &mut self,
        evidence: VerificationEvidenceRecord,
    ) -> Result<(), &'static str> {
        if evidence.evidence_id.trim().is_empty() {
            return Err("evidence id must not be empty");
        }
        if evidence.assertion_refs.is_empty()
            || !references_are_unique_and_nonempty(&evidence.assertion_refs)
        {
            return Err("evidence must reference at least one assertion");
        }
        if !evidence.is_complete() {
            return Err("evidence must include source, location, result, and capture time");
        }
        if self
            .evidence
            .iter()
            .any(|existing| existing.evidence_id == evidence.evidence_id)
        {
            return Err("evidence id already exists");
        }
        self.evidence.push(evidence);
        Ok(())
    }

    pub fn append_repair(&mut self, repair: RepairContract) -> Result<(), &'static str> {
        if repair.repair_id.trim().is_empty() {
            return Err("repair id must not be empty");
        }
        if repair.source_contract_ref.trim().is_empty() {
            return Err("repair source contract must not be empty");
        }
        if repair.repair_scope.trim().is_empty() {
            return Err("repair scope must not be empty");
        }
        if repair.affected_assertion_refs.is_empty()
            || !references_are_unique_and_nonempty(&repair.affected_assertion_refs)
        {
            return Err("repair must affect at least one assertion");
        }
        if self
            .repairs
            .iter()
            .any(|existing| existing.repair_id == repair.repair_id)
        {
            return Err("repair id already exists");
        }
        self.repairs.push(repair);
        Ok(())
    }

    pub fn append_attempt(&mut self, attempt: VerificationAttempt) -> Result<(), &'static str> {
        if attempt.attempt_id.trim().is_empty() {
            return Err("attempt id must not be empty");
        }
        if attempt.attempt_number == 0 {
            return Err("attempt number must be positive");
        }
        if attempt.source_version.trim().is_empty() {
            return Err("attempt source version must not be empty");
        }
        if attempt.contract_version.trim().is_empty() {
            return Err("attempt contract version must not be empty");
        }
        if attempt.started_at.trim().is_empty() || attempt.completed_at.trim().is_empty() {
            return Err("attempt timestamps must not be empty");
        }
        if attempt.assertion_refs.is_empty()
            || !references_are_unique_and_nonempty(&attempt.assertion_refs)
        {
            return Err("attempt must reference at least one assertion");
        }
        if attempt.evidence_refs.is_empty()
            || !references_are_unique_and_nonempty(&attempt.evidence_refs)
        {
            return Err("attempt must reference at least one evidence record");
        }
        if self
            .attempts
            .iter()
            .any(|existing| existing.attempt_id == attempt.attempt_id)
        {
            return Err("attempt id already exists");
        }
        if let Some(previous) = self.attempts.last() {
            if attempt.attempt_number <= previous.attempt_number {
                return Err("attempt number must increase monotonically");
            }
        }
        if let Some(repair_ref) = attempt.repair_ref.as_deref() {
            if !self
                .repairs
                .iter()
                .any(|repair| repair.repair_id == repair_ref)
            {
                return Err("attempt references an unknown repair");
            }
            let repair = self
                .repairs
                .iter()
                .find(|repair| repair.repair_id == repair_ref)
                .expect("repair was checked above");
            if repair
                .affected_assertion_refs
                .iter()
                .any(|assertion_ref| !attempt.assertion_refs.contains(assertion_ref))
            {
                return Err("repair attempt must cover every affected assertion");
            }
            if matches!(
                attempt.status,
                VerificationLifecycleStatus::Checked | VerificationLifecycleStatus::Passed
            ) {
                return Err("a repair attempt requires a separate reverification result");
            }
        } else if matches!(attempt.status, VerificationLifecycleStatus::Reverified) {
            return Err("reverified attempt requires a repair reference");
        }
        if !attempt.has_complete_evidence_for_assertions(&self.evidence) {
            return Err("attempt must have complete evidence for every assertion");
        }

        self.attempts.push(attempt);
        Ok(())
    }

    pub fn validate(&self, delivery: Option<&DeliveryResult>) -> Result<(), &'static str> {
        let mut canonical = Self::default();
        for evidence in &self.evidence {
            canonical.append_evidence(evidence.clone())?;
        }
        for repair in &self.repairs {
            canonical.append_repair(repair.clone())?;
        }
        for attempt in &self.attempts {
            canonical.append_attempt(attempt.clone())?;
        }

        let known_assertions = self
            .attempts
            .iter()
            .flat_map(|attempt| attempt.assertion_refs.iter())
            .chain(
                self.repairs
                    .iter()
                    .flat_map(|repair| repair.affected_assertion_refs.iter()),
            )
            .collect::<BTreeSet<_>>();
        for evidence in &self.evidence {
            if evidence
                .assertion_refs
                .iter()
                .any(|assertion_ref| !known_assertions.contains(assertion_ref))
            {
                return Err("evidence references an unknown assertion");
            }
        }
        for attempt in &self.attempts {
            if attempt.evidence_refs.iter().any(|evidence_ref| {
                !self
                    .evidence
                    .iter()
                    .any(|record| record.evidence_id == *evidence_ref)
            }) {
                return Err("attempt references an unknown evidence record");
            }
            if matches!(
                attempt.status,
                VerificationLifecycleStatus::Passed | VerificationLifecycleStatus::Reverified
            ) && !attempt.is_completion_eligible(&self.evidence)
            {
                return Err("successful attempt lacks complete evidence");
            }
            if let Some(repair_ref) = attempt.repair_ref.as_deref() {
                let repair = self
                    .repairs
                    .iter()
                    .find(|repair| repair.repair_id == repair_ref)
                    .ok_or("attempt references an unknown repair")?;
                if matches!(attempt.status, VerificationLifecycleStatus::Reverified)
                    && !attempt.is_fresh_for_repair(repair)
                {
                    return Err("reverification does not match the repair contract");
                }
            }
        }
        if let Some(delivery) = delivery {
            self.validate_delivery_result(delivery, &self.evidence)?;
        }
        Ok(())
    }

    pub fn validate_delivery_result(
        &self,
        delivery: &DeliveryResult,
        evidence: &[VerificationEvidenceRecord],
    ) -> Result<(), &'static str> {
        let attempt = self
            .attempts
            .iter()
            .find(|attempt| attempt.attempt_id == delivery.latest_attempt_ref)
            .ok_or("delivery references an unknown attempt")?;
        if delivery.delivery_result_id.trim().is_empty()
            || delivery.latest_attempt_ref.trim().is_empty()
            || delivery.latest_source_ref.trim().is_empty()
            || delivery.contract_version.trim().is_empty()
        {
            return Err("delivery result is missing required references");
        }
        if self
            .attempts
            .last()
            .map(|latest| latest.attempt_id.as_str())
            != Some(attempt.attempt_id.as_str())
        {
            return Err("delivery must reference the latest verification attempt");
        }
        if delivery.latest_source_ref != attempt.source_version {
            return Err("delivery source does not match the referenced attempt");
        }
        if delivery.contract_version != attempt.contract_version {
            return Err("delivery contract does not match the referenced attempt");
        }
        if delivery.is_completion_evidence() {
            if !attempt.is_completion_eligible(evidence) {
                return Err("delivery cannot be successful without verified evidence");
            }
            if matches!(attempt.status, VerificationLifecycleStatus::Reverified)
                && attempt.repair_ref.is_none()
            {
                return Err("reverified delivery requires a repair-linked attempt");
            }
            if let Some(repair_ref) = attempt.repair_ref.as_deref() {
                let repair = self
                    .repairs
                    .iter()
                    .find(|repair| repair.repair_id == repair_ref)
                    .ok_or("delivery references an attempt with an unknown repair")?;
                if !matches!(attempt.status, VerificationLifecycleStatus::Reverified)
                    || !attempt.is_fresh_for_repair(repair)
                {
                    return Err("delivery repair lineage is incomplete");
                }
            }
        }
        Ok(())
    }

    pub fn latest_verified_attempt(
        &self,
        evidence: &[VerificationEvidenceRecord],
    ) -> Option<&VerificationAttempt> {
        self.attempts
            .iter()
            .rev()
            .find(|attempt| attempt.is_completion_eligible(evidence))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryResult {
    pub delivery_result_id: String,
    pub latest_attempt_ref: String,
    pub latest_source_ref: String,
    pub contract_version: String,
    pub status: VerificationLifecycleStatus,
    #[serde(default)]
    pub finding_refs: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl DeliveryResult {
    pub fn is_completion_evidence(&self) -> bool {
        matches!(
            self.status,
            VerificationLifecycleStatus::Passed
                | VerificationLifecycleStatus::Reverified
                | VerificationLifecycleStatus::CompletedWithNotes
        )
    }
}

fn references_are_unique_and_nonempty(references: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    references
        .iter()
        .all(|reference| !reference.trim().is_empty() && seen.insert(reference))
}

/// Read-only projection of verification history for MCP and structured result
/// consumers. It is derived from committed records and never becomes a new
/// source of delivery truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationAnalysisSummary {
    pub status: Option<VerificationLifecycleStatus>,
    pub attempt_count: u32,
    pub repair_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_attempt_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_result_ref: Option<String>,
    pub telemetry_complete: bool,
    pub completeness: AnalysisCompleteness,
    #[serde(default)]
    pub missing_evidence: Vec<String>,
    #[serde(default)]
    pub failure_classes: Vec<VerificationFailureClass>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

impl VerificationAnalysisSummary {
    pub fn from_history(
        attempts: &[VerificationAttempt],
        repairs: &[RepairContract],
        delivery_result: Option<&DeliveryResult>,
        telemetry: &[TrainingTelemetryRecord],
        findings: &[VerificationFinding],
    ) -> Self {
        Self::from_history_with_evidence(
            attempts,
            repairs,
            delivery_result,
            telemetry,
            findings,
            None,
        )
    }

    pub fn from_history_with_evidence(
        attempts: &[VerificationAttempt],
        repairs: &[RepairContract],
        delivery_result: Option<&DeliveryResult>,
        telemetry: &[TrainingTelemetryRecord],
        findings: &[VerificationFinding],
        evidence: Option<&[VerificationEvidenceRecord]>,
    ) -> Self {
        let latest = attempts.last();
        let telemetry_attempts = telemetry
            .iter()
            .map(|record| record.source_attempt_ref.as_str())
            .collect::<BTreeSet<_>>();
        let mut source_refs = BTreeSet::new();
        let mut evidence_refs = BTreeSet::new();
        for attempt in attempts {
            source_refs.insert(attempt.source_version.clone());
            evidence_refs.extend(attempt.evidence_refs.iter().cloned());
        }

        let mut missing_evidence = Vec::new();
        if attempts.is_empty() {
            missing_evidence.push("verification_attempts".to_string());
        }
        for attempt in attempts {
            if attempt.source_version.trim().is_empty() {
                missing_evidence.push(format!("source:{}", attempt.attempt_id));
            }
            if attempt.evidence_refs.is_empty() {
                missing_evidence.push(format!("evidence:{}", attempt.attempt_id));
            }
        }
        if let Some(evidence) = evidence {
            for evidence_ref in &evidence_refs {
                if !evidence
                    .iter()
                    .any(|item| item.evidence_id == *evidence_ref && item.is_complete())
                {
                    missing_evidence.push(format!("evidence_ref:{evidence_ref}"));
                }
            }
        }
        if let Some(delivery) = delivery_result {
            match latest {
                Some(attempt) if delivery.latest_attempt_ref == attempt.attempt_id => {}
                Some(_) => missing_evidence.push("delivery_latest_attempt".to_string()),
                None => missing_evidence.push("delivery_attempt".to_string()),
            }
            if delivery.latest_source_ref.trim().is_empty() {
                missing_evidence.push("delivery_source".to_string());
            } else if latest
                .map(|attempt| attempt.source_version.as_str())
                .filter(|source| *source != delivery.latest_source_ref)
                .is_some()
            {
                missing_evidence.push("delivery_source_mismatch".to_string());
            }
            if !delivery.is_completion_evidence() {
                missing_evidence.push("delivery_not_complete".to_string());
            }
            if let Some(evidence) = evidence {
                let history = VerificationHistory {
                    attempts: attempts.to_vec(),
                    repairs: repairs.to_vec(),
                    evidence: evidence.to_vec(),
                };
                if history
                    .validate_delivery_result(delivery, evidence)
                    .is_err()
                {
                    missing_evidence.push("delivery_unverified".to_string());
                }
            }
        } else {
            missing_evidence.push("delivery_result".to_string());
        }

        missing_evidence.sort();
        missing_evidence.dedup();

        Self {
            status: latest.map(|attempt| attempt.status.clone()),
            attempt_count: attempts.len() as u32,
            repair_count: repairs.len() as u32,
            latest_attempt_ref: latest.map(|attempt| attempt.attempt_id.clone()),
            delivery_result_ref: delivery_result.map(|result| result.delivery_result_id.clone()),
            telemetry_complete: !attempts.is_empty()
                && attempts
                    .iter()
                    .all(|attempt| telemetry_attempts.contains(attempt.attempt_id.as_str())),
            completeness: if missing_evidence.is_empty() {
                AnalysisCompleteness::Complete
            } else {
                AnalysisCompleteness::Incomplete
            },
            missing_evidence,
            failure_classes: findings
                .iter()
                .map(|finding| finding.classification)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            source_refs: source_refs.into_iter().collect(),
            evidence_refs: evidence_refs.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingTelemetryStateTransition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<VerificationLifecycleStatus>,
    pub to: VerificationLifecycleStatus,
    pub trigger: String,
}

/// A derived learning signal. It keeps the source verification references so
/// future consumers can reconstruct the delivery trajectory without treating
/// telemetry as a second delivery result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingTelemetryRecord {
    pub telemetry_id: String,
    pub source_event_ref: String,
    pub source_attempt_ref: String,
    pub outcome: String,
    pub state_transition: TrainingTelemetryStateTransition,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub schema_version: String,
}

impl TrainingTelemetryRecord {
    pub fn from_attempt(
        telemetry_id: impl Into<String>,
        source_event_ref: impl Into<String>,
        attempt: &VerificationAttempt,
        outcome: impl Into<String>,
        from: Option<VerificationLifecycleStatus>,
        trigger: impl Into<String>,
    ) -> Option<Self> {
        let source_event_ref = source_event_ref.into();
        if source_event_ref.trim().is_empty() || attempt.attempt_id.trim().is_empty() {
            return None;
        }

        Some(Self {
            telemetry_id: telemetry_id.into(),
            source_event_ref,
            source_attempt_ref: attempt.attempt_id.clone(),
            outcome: outcome.into(),
            state_transition: TrainingTelemetryStateTransition {
                from,
                to: attempt.status.clone(),
                trigger: trigger.into(),
            },
            evidence_refs: attempt.evidence_refs.clone(),
            schema_version: attempt.contract_version.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(location: &str) -> VerificationEvidenceRecord {
        VerificationEvidenceRecord {
            evidence_id: "evidence-1".to_string(),
            assertion_refs: vec!["assertion-1".to_string()],
            source: "cargo test".to_string(),
            location: location.to_string(),
            observed_result: "passed".to_string(),
            captured_at: "2026-09-07T00:00:00Z".to_string(),
        }
    }

    fn attempt() -> VerificationAttempt {
        VerificationAttempt {
            attempt_id: "attempt-1".to_string(),
            attempt_number: 1,
            assertion_refs: vec!["assertion-1".to_string()],
            evidence_refs: vec!["evidence-1".to_string()],
            source_version: "source-1".to_string(),
            contract_version: "contract-1".to_string(),
            repair_ref: None,
            status: VerificationLifecycleStatus::Checked,
            started_at: "2026-09-07T00:00:00Z".to_string(),
            completed_at: "2026-09-07T00:00:01Z".to_string(),
        }
    }

    #[test]
    fn telemetry_keeps_source_attempt_and_evidence_references() {
        let record = TrainingTelemetryRecord::from_attempt(
            "telemetry-1",
            "event-1",
            &attempt(),
            "passed",
            Some(VerificationLifecycleStatus::Pending),
            "verification_committed",
        )
        .expect("valid attempt should produce telemetry");

        assert_eq!(record.source_event_ref, "event-1");
        assert_eq!(record.source_attempt_ref, "attempt-1");
        assert_eq!(record.evidence_refs, vec!["evidence-1"]);
        assert_eq!(
            record.state_transition.to,
            VerificationLifecycleStatus::Checked
        );
    }

    #[test]
    fn telemetry_requires_a_source_event_and_attempt() {
        assert!(TrainingTelemetryRecord::from_attempt(
            "telemetry-1",
            "",
            &attempt(),
            "passed",
            None,
            "verification_committed",
        )
        .is_none());
    }

    #[test]
    fn pass_requires_locatable_evidence_for_the_assertion() {
        assert!(attempt().can_pass(&[evidence("tests/result.txt:12")]));
        assert!(!attempt().can_pass(&[evidence("")]));
    }

    #[test]
    fn pass_requires_complete_evidence_metadata() {
        let mut incomplete = evidence("tests/result.txt:12");
        incomplete.source.clear();
        assert!(!attempt().can_pass(&[incomplete]));
    }

    #[test]
    fn pending_attempt_cannot_pass_even_with_evidence() {
        let mut pending = attempt();
        pending.status = VerificationLifecycleStatus::Pending;
        assert!(!pending.can_pass(&[evidence("tests/result.txt:12")]));
    }

    #[test]
    fn every_assertion_needs_locatable_evidence() {
        let mut attempt = attempt();
        attempt.assertion_refs.push("assertion-2".to_string());
        assert!(!attempt.can_pass(&[evidence("tests/result.txt:12")]));
    }

    #[test]
    fn repair_requires_a_new_attempt_bound_to_the_repair() {
        let repair = RepairContract {
            repair_id: "repair-1".to_string(),
            finding_refs: vec!["finding-1".to_string()],
            affected_assertion_refs: vec!["assertion-1".to_string()],
            repair_scope: "fix implementation".to_string(),
            required_checks: vec!["cargo test".to_string()],
            source_contract_ref: "contract-1".to_string(),
            verification_contract_version: Some("v1".to_string()),
            status: VerificationLifecycleStatus::RepairPending,
        };
        let mut attempt = attempt();
        assert!(!attempt.is_fresh_for_repair(&repair));
        attempt.repair_ref = Some("repair-1".to_string());
        assert!(attempt.is_fresh_for_repair(&repair));
    }

    #[test]
    fn findings_keep_code_and_environment_failures_distinct() {
        let mut finding = VerificationFinding {
            finding_id: "finding-1".to_string(),
            severity: "high".to_string(),
            failure_owner: "repository".to_string(),
            classification: VerificationFailureClass::CodeFailure,
            message: "assertion failed".to_string(),
            affected_assertion_refs: vec!["assertion-1".to_string()],
            remediation: "repair the implementation".to_string(),
        };
        assert!(finding.is_code_failure());
        assert!(!finding.is_environment_blocked());
        finding.classification = VerificationFailureClass::EnvironmentBlocked;
        assert!(!finding.is_code_failure());
        assert!(finding.is_environment_blocked());
    }

    #[test]
    fn reverified_attempts_can_pass_but_pending_and_blocked_cannot() {
        let mut attempt = attempt();
        attempt.status = VerificationLifecycleStatus::Reverified;
        assert!(attempt.can_pass(&[evidence("tests/result.txt:12")]));

        for status in [
            VerificationLifecycleStatus::Pending,
            VerificationLifecycleStatus::Failed,
            VerificationLifecycleStatus::Blocked,
            VerificationLifecycleStatus::RepairPending,
        ] {
            attempt.status = status;
            assert!(!attempt.can_pass(&[evidence("tests/result.txt:12")]));
        }
    }

    #[test]
    fn history_is_append_only_and_rejects_stale_attempts() {
        let mut history = VerificationHistory::default();
        history
            .append_evidence(evidence("tests/result.txt:12"))
            .expect("evidence");
        history.append_attempt(attempt()).expect("first attempt");

        let mut stale = attempt();
        stale.attempt_id = "attempt-2".to_string();
        assert_eq!(
            history.append_attempt(stale),
            Err("attempt number must increase monotonically")
        );
        assert_eq!(history.attempts.len(), 1);
    }

    #[test]
    fn history_rejects_attempts_without_complete_evidence_coverage() {
        let mut history = VerificationHistory::default();
        let candidate = attempt();

        assert_eq!(
            history.append_attempt(candidate.clone()),
            Err("attempt must have complete evidence for every assertion")
        );

        history
            .append_evidence(evidence("tests/result.txt:12"))
            .expect("evidence");
        let mut uncovered = candidate;
        uncovered.assertion_refs = vec!["assertion-2".to_string()];

        assert_eq!(
            history.append_attempt(uncovered),
            Err("attempt must have complete evidence for every assertion")
        );
        assert!(history.attempts.is_empty());
    }

    #[test]
    fn history_rejects_untraceable_attempts_and_repairs() {
        let mut history = VerificationHistory::default();
        let mut invalid_attempt = attempt();
        invalid_attempt.source_version.clear();
        assert_eq!(
            history.append_attempt(invalid_attempt),
            Err("attempt source version must not be empty")
        );

        let invalid_repair = RepairContract {
            repair_id: "repair-1".to_string(),
            finding_refs: vec![],
            affected_assertion_refs: vec![],
            repair_scope: "fix implementation".to_string(),
            required_checks: vec!["cargo test".to_string()],
            source_contract_ref: "contract-1".to_string(),
            verification_contract_version: None,
            status: VerificationLifecycleStatus::RepairPending,
        };
        assert_eq!(
            history.append_repair(invalid_repair),
            Err("repair must affect at least one assertion")
        );
    }

    #[test]
    fn delivery_result_must_match_verified_attempt_source() {
        let history = VerificationHistory {
            attempts: vec![attempt()],
            repairs: vec![],
            evidence: vec![evidence("tests/result.txt:12")],
        };
        let delivery = DeliveryResult {
            delivery_result_id: "delivery-1".to_string(),
            latest_attempt_ref: "attempt-1".to_string(),
            latest_source_ref: "different-source".to_string(),
            contract_version: "contract-1".to_string(),
            status: VerificationLifecycleStatus::Passed,
            finding_refs: vec![],
            notes: vec![],
        };
        assert_eq!(
            history.validate_delivery_result(&delivery, &[evidence("tests/result.txt:12")]),
            Err("delivery source does not match the referenced attempt")
        );
    }

    #[test]
    fn repair_attempt_cannot_be_recorded_as_verified_without_reverification() {
        let mut history = VerificationHistory::default();
        history
            .append_repair(RepairContract {
                repair_id: "repair-1".to_string(),
                finding_refs: vec![],
                affected_assertion_refs: vec!["assertion-1".to_string()],
                repair_scope: "fix implementation".to_string(),
                required_checks: vec!["cargo test".to_string()],
                source_contract_ref: "contract-1".to_string(),
                verification_contract_version: None,
                status: VerificationLifecycleStatus::RepairPending,
            })
            .expect("repair");

        let mut repaired = attempt();
        repaired.attempt_id = "attempt-2".to_string();
        repaired.attempt_number = 2;
        repaired.repair_ref = Some("repair-1".to_string());
        assert_eq!(
            history.append_attempt(repaired),
            Err("a repair attempt requires a separate reverification result")
        );
        assert!(history.attempts.is_empty());
    }

    #[test]
    fn reverified_attempt_requires_a_repair_reference() {
        let mut history = VerificationHistory::default();
        history
            .append_evidence(evidence("tests/result.txt:12"))
            .expect("evidence");
        let mut reverified = attempt();
        reverified.status = VerificationLifecycleStatus::Reverified;

        assert_eq!(
            history.append_attempt(reverified),
            Err("reverified attempt requires a repair reference")
        );
        assert!(history.attempts.is_empty());
    }

    #[test]
    fn delivery_rejects_an_unlinked_reverified_attempt() {
        let mut reverified = attempt();
        reverified.status = VerificationLifecycleStatus::Reverified;
        let history = VerificationHistory {
            attempts: vec![reverified],
            repairs: vec![],
            evidence: vec![evidence("tests/result.txt:12")],
        };
        let delivery = DeliveryResult {
            delivery_result_id: "delivery-1".to_string(),
            latest_attempt_ref: "attempt-1".to_string(),
            latest_source_ref: "source-1".to_string(),
            contract_version: "contract-1".to_string(),
            status: VerificationLifecycleStatus::Passed,
            finding_refs: vec![],
            notes: vec![],
        };

        assert_eq!(
            history.validate_delivery_result(&delivery, &history.evidence),
            Err("reverified delivery requires a repair-linked attempt")
        );
    }

    #[test]
    fn history_resolves_reverification_after_repair() {
        let mut history = VerificationHistory::default();
        history
            .append_evidence(evidence("tests/result.txt:12"))
            .expect("evidence");
        history
            .append_repair(RepairContract {
                repair_id: "repair-1".to_string(),
                finding_refs: vec![],
                affected_assertion_refs: vec!["assertion-1".to_string()],
                repair_scope: "fix implementation".to_string(),
                required_checks: vec!["cargo test".to_string()],
                source_contract_ref: "contract-1".to_string(),
                verification_contract_version: None,
                status: VerificationLifecycleStatus::RepairPending,
            })
            .expect("repair");

        let mut reverification = attempt();
        reverification.attempt_id = "attempt-2".to_string();
        reverification.attempt_number = 2;
        reverification.repair_ref = Some("repair-1".to_string());
        reverification.status = VerificationLifecycleStatus::Reverified;
        history
            .append_attempt(reverification)
            .expect("reverification");

        assert_eq!(
            history
                .latest_verified_attempt(&[evidence("tests/result.txt:12")])
                .map(|item| item.attempt_id.as_str()),
            Some("attempt-2")
        );
    }

    #[test]
    fn history_round_trips_through_persistence_shape() {
        let mut history = VerificationHistory::default();
        history
            .append_evidence(evidence("tests/result.txt:12"))
            .expect("evidence");
        history.append_attempt(attempt()).expect("first attempt");

        let encoded = serde_json::to_string(&history).expect("serialize history");
        let restored: VerificationHistory =
            serde_json::from_str(&encoded).expect("deserialize history");

        assert_eq!(restored, history);
        assert_eq!(restored.attempts[0].attempt_id, "attempt-1");
        assert_eq!(restored.attempts[0].evidence_refs, vec!["evidence-1"]);
        assert_eq!(restored.evidence[0].evidence_id, "evidence-1");
    }

    #[test]
    fn lifecycle_statuses_keep_stable_wire_names() {
        let statuses = [
            (VerificationLifecycleStatus::Pending, "pending"),
            (VerificationLifecycleStatus::Checked, "checked"),
            (VerificationLifecycleStatus::Passed, "passed"),
            (VerificationLifecycleStatus::Failed, "failed"),
            (VerificationLifecycleStatus::Blocked, "blocked"),
            (VerificationLifecycleStatus::RepairPending, "repair_pending"),
            (VerificationLifecycleStatus::Reverified, "reverified"),
            (
                VerificationLifecycleStatus::CompletedWithNotes,
                "completed_with_notes",
            ),
        ];

        for (status, expected) in statuses {
            assert_eq!(
                serde_json::to_string(&status).unwrap(),
                format!("\"{expected}\"")
            );
        }
    }

    #[test]
    fn analysis_summary_keeps_latest_result_and_detects_missing_telemetry() {
        let mut second = attempt();
        second.attempt_id = "attempt-2".to_string();
        second.attempt_number = 2;
        second.source_version = "source-2".to_string();
        second.repair_ref = Some("repair-1".to_string());
        second.status = VerificationLifecycleStatus::Reverified;
        let repair = RepairContract {
            repair_id: "repair-1".to_string(),
            finding_refs: vec!["finding-1".to_string()],
            affected_assertion_refs: vec!["assertion-1".to_string()],
            repair_scope: "fix implementation".to_string(),
            required_checks: vec!["cargo test".to_string()],
            source_contract_ref: "contract-1".to_string(),
            verification_contract_version: None,
            status: VerificationLifecycleStatus::Reverified,
        };
        let delivery = DeliveryResult {
            delivery_result_id: "delivery-1".to_string(),
            latest_attempt_ref: "attempt-2".to_string(),
            latest_source_ref: "source-2".to_string(),
            contract_version: "contract-1".to_string(),
            status: VerificationLifecycleStatus::Reverified,
            finding_refs: vec!["finding-1".to_string()],
            notes: Vec::new(),
        };
        let finding = VerificationFinding {
            finding_id: "finding-1".to_string(),
            severity: "high".to_string(),
            failure_owner: "repository".to_string(),
            classification: VerificationFailureClass::CodeFailure,
            message: "assertion failed".to_string(),
            affected_assertion_refs: vec!["assertion-1".to_string()],
            remediation: "repair the implementation".to_string(),
        };
        let telemetry = TrainingTelemetryRecord::from_attempt(
            "telemetry-1",
            "event-1",
            &second,
            "reverified",
            Some(VerificationLifecycleStatus::Failed),
            "repair_committed",
        )
        .expect("valid attempt should produce telemetry");

        let summary = VerificationAnalysisSummary::from_history(
            &[attempt(), second],
            &[repair],
            Some(&delivery),
            &[telemetry],
            &[finding],
        );

        assert_eq!(
            summary.status,
            Some(VerificationLifecycleStatus::Reverified)
        );
        assert_eq!(summary.attempt_count, 2);
        assert_eq!(summary.repair_count, 1);
        assert_eq!(summary.latest_attempt_ref.as_deref(), Some("attempt-2"));
        assert_eq!(summary.delivery_result_ref.as_deref(), Some("delivery-1"));
        assert!(!summary.telemetry_complete);
        assert_eq!(
            summary.failure_classes,
            vec![VerificationFailureClass::CodeFailure]
        );
        assert_eq!(summary.completeness, AnalysisCompleteness::Complete);
        assert!(summary.missing_evidence.is_empty());
    }

    #[test]
    fn analysis_summary_marks_missing_evidence_instead_of_inferencing_success() {
        let mut incomplete = attempt();
        incomplete.source_version.clear();
        incomplete.evidence_refs.clear();

        let summary = VerificationAnalysisSummary::from_history_with_evidence(
            &[incomplete],
            &[],
            None,
            &[],
            &[],
            Some(&[]),
        );

        assert_eq!(summary.completeness, AnalysisCompleteness::Incomplete);
        assert!(summary
            .missing_evidence
            .iter()
            .any(|item| item == "source:attempt-1"));
        assert!(summary
            .missing_evidence
            .iter()
            .any(|item| item == "evidence:attempt-1"));
    }

    #[test]
    fn analysis_summary_marks_unresolved_delivery_attempt() {
        let delivery = DeliveryResult {
            delivery_result_id: "delivery-1".to_string(),
            latest_attempt_ref: "missing-attempt".to_string(),
            latest_source_ref: "source-1".to_string(),
            contract_version: "contract-1".to_string(),
            status: VerificationLifecycleStatus::Reverified,
            finding_refs: vec![],
            notes: vec![],
        };

        let summary =
            VerificationAnalysisSummary::from_history(&[attempt()], &[], Some(&delivery), &[], &[]);

        assert_eq!(summary.completeness, AnalysisCompleteness::Incomplete);
        assert_eq!(
            summary.missing_evidence,
            vec!["delivery_latest_attempt".to_string()]
        );
    }

    #[test]
    fn analysis_summary_requires_delivery_linkage() {
        let summary = VerificationAnalysisSummary::from_history(&[attempt()], &[], None, &[], &[]);
        assert_eq!(summary.completeness, AnalysisCompleteness::Incomplete);
        assert!(summary
            .missing_evidence
            .iter()
            .any(|item| item == "delivery_result"));
    }

    #[test]
    fn history_requires_complete_evidence_before_accepting_delivery() {
        let mut history = VerificationHistory::default();
        history
            .append_evidence(evidence("tests/result.txt:12"))
            .expect("evidence");
        let mut verified = attempt();
        verified.status = VerificationLifecycleStatus::Passed;
        history.append_attempt(verified).expect("attempt");
        let delivery = DeliveryResult {
            delivery_result_id: "delivery-1".to_string(),
            latest_attempt_ref: "attempt-1".to_string(),
            latest_source_ref: "source-1".to_string(),
            contract_version: "contract-1".to_string(),
            status: VerificationLifecycleStatus::Passed,
            finding_refs: vec![],
            notes: vec![],
        };

        assert_eq!(history.validate(Some(&delivery)), Ok(()));
    }

    #[test]
    fn history_rejects_delivery_with_a_mismatched_contract() {
        let mut history = VerificationHistory::default();
        history
            .append_evidence(evidence("tests/result.txt:12"))
            .expect("evidence");
        let mut verified = attempt();
        verified.status = VerificationLifecycleStatus::Passed;
        history.append_attempt(verified).expect("attempt");
        let delivery = DeliveryResult {
            delivery_result_id: "delivery-1".to_string(),
            latest_attempt_ref: "attempt-1".to_string(),
            latest_source_ref: "source-1".to_string(),
            contract_version: "different-contract".to_string(),
            status: VerificationLifecycleStatus::Passed,
            finding_refs: vec![],
            notes: vec![],
        };

        assert_eq!(
            history.validate(Some(&delivery)),
            Err("delivery contract does not match the referenced attempt")
        );
    }
}
