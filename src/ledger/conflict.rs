// Conflict Detection - Detects double-spends in the distributed mesh

use crate::identity::PublicKey;
use crate::iou::IOUId;
use crate::ledger::state::NodeId;
use crate::vault::UTXOId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Type of conflict detected
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    /// Same UTXO spent to different recipients
    SameUtxoDifferentRecipient,
    /// Same UTXO spent with different amounts
    SameUtxoDifferentAmount,
    /// Same UTXO used in multiple transactions
    SameUtxoMultipleTransactions,
}

/// Errors from conflict detection
#[derive(Error, Debug)]
pub enum ConflictError {
    #[error("Double spend detected: UTXO {utxo_id:?} spent in conflicting transactions")]
    DoubleSpend {
        utxo_id: UTXOId,
        conflict_type: ConflictType,
        first_claim: SpendingClaim,
        second_claim: SpendingClaim,
    },

    #[error("Deserialization failed")]
    DeserializationFailed,
}

/// Resolution strategy for conflicts
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictResolution {
    /// First claim seen wins (by timestamp)
    FirstSeen,
    /// Claim with most witness attestations wins
    MostWitnesses,
    /// Custom resolution (provide a comparator)
    Custom,
}

/// A claim that a UTXO was spent in a specific transaction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpendingClaim {
    /// The UTXO being spent
    utxo_id: UTXOId,
    /// The IOU that is spending this UTXO
    spending_iou_id: IOUId,
    /// The spender's public key
    spender: PublicKey,
    /// When this claim was first seen
    timestamp: u64,
    /// Nodes that have witnessed this claim
    witnesses: HashSet<NodeId>,
}

impl SpendingClaim {
    /// Create a new spending claim
    pub fn new(utxo_id: UTXOId, spending_iou_id: IOUId, spender: PublicKey) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            utxo_id,
            spending_iou_id,
            spender,
            timestamp,
            witnesses: HashSet::new(),
        }
    }

    /// Create with explicit timestamp
    pub fn with_timestamp(
        utxo_id: UTXOId,
        spending_iou_id: IOUId,
        spender: PublicKey,
        timestamp: u64,
    ) -> Self {
        Self {
            utxo_id,
            spending_iou_id,
            spender,
            timestamp,
            witnesses: HashSet::new(),
        }
    }

    /// Get the UTXO ID
    pub fn utxo_id(&self) -> &UTXOId {
        &self.utxo_id
    }

    /// Get the spending IOU ID
    pub fn spending_iou_id(&self) -> &IOUId {
        &self.spending_iou_id
    }

    /// Get the spender's public key
    pub fn spender(&self) -> &PublicKey {
        &self.spender
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Add a witness to this claim
    pub fn add_witness(&mut self, node_id: NodeId) {
        self.witnesses.insert(node_id);
    }

    /// Get the number of witnesses
    pub fn witness_count(&self) -> usize {
        self.witnesses.len()
    }

    /// Check if a node has witnessed this claim
    pub fn has_witness(&self, node_id: &NodeId) -> bool {
        self.witnesses.contains(node_id)
    }

    /// Get all witnesses
    pub fn witnesses(&self) -> &HashSet<NodeId> {
        &self.witnesses
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ConflictError> {
        postcard::from_bytes(bytes).map_err(|_| ConflictError::DeserializationFailed)
    }
}

impl PartialEq for SpendingClaim {
    fn eq(&self, other: &Self) -> bool {
        // Two claims are equal if they claim the same UTXO with the same IOU
        self.utxo_id == other.utxo_id && self.spending_iou_id == other.spending_iou_id
    }
}

impl Eq for SpendingClaim {}

impl std::hash::Hash for SpendingClaim {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.utxo_id.as_bytes().hash(state);
        self.spending_iou_id.as_bytes().hash(state);
    }
}

/// Result of merging two conflict detectors
#[derive(Clone, Debug)]
pub struct DetectorMergeResult {
    /// Number of new claims added
    pub new_claims: usize,
    /// Number of conflicts detected during merge
    pub conflicts_detected: usize,
}

/// Conflict detector - tracks spending claims and detects double-spends
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictDetector {
    /// Map: UTXO ID -> list of claims for that UTXO
    claims: HashMap<UTXOId, Vec<SpendingClaim>>,
    /// Count of detected conflicts
    conflict_count: usize,
}

impl Default for ConflictDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ConflictDetector {
    /// Create a new conflict detector
    pub fn new() -> Self {
        Self {
            claims: HashMap::new(),
            conflict_count: 0,
        }
    }

    /// Get the number of registered claims
    pub fn claim_count(&self) -> usize {
        self.claims.values().map(|v| v.len()).sum()
    }

    /// Get the number of detected conflicts
    pub fn conflict_count(&self) -> usize {
        self.conflict_count
    }

    /// Register a spending claim
    /// Returns Ok if no conflict, Err if double-spend detected
    pub fn register_claim(&mut self, claim: SpendingClaim) -> Result<(), ConflictError> {
        let utxo_id = claim.utxo_id().clone();

        // Check if we already have claims for this UTXO
        if let Some(existing_claims) = self.claims.get(&utxo_id) {
            // Check if this exact claim already exists (idempotent)
            for existing in existing_claims {
                if existing.spending_iou_id() == claim.spending_iou_id() {
                    // Same claim, just merge witnesses and return ok
                    return Ok(());
                }
            }

            // Different IOU spending same UTXO = DOUBLE SPEND!
            let first_claim = existing_claims[0].clone();
            self.conflict_count += 1;

            // Still record the claim for conflict resolution later
            self.claims
                .entry(utxo_id.clone())
                .or_insert_with(Vec::new)
                .push(claim.clone());

            return Err(ConflictError::DoubleSpend {
                utxo_id,
                conflict_type: ConflictType::SameUtxoDifferentRecipient,
                first_claim,
                second_claim: claim,
            });
        }

        // No existing claims, register this one
        self.claims
            .entry(utxo_id)
            .or_insert_with(Vec::new)
            .push(claim);

        Ok(())
    }

    /// Get all claims for a specific UTXO
    pub fn get_claims_for_utxo(&self, utxo_id: &UTXOId) -> Vec<&SpendingClaim> {
        self.claims
            .get(utxo_id)
            .map(|claims| claims.iter().collect())
            .unwrap_or_default()
    }

    /// Get conflicting claims for a UTXO (if any)
    pub fn get_conflicts_for_utxo(&self, utxo_id: &UTXOId) -> Vec<&SpendingClaim> {
        match self.claims.get(utxo_id) {
            Some(claims) if claims.len() > 1 => claims.iter().collect(),
            _ => vec![],
        }
    }

    /// Check if a UTXO has conflicting claims
    pub fn has_conflict(&self, utxo_id: &UTXOId) -> bool {
        self.claims
            .get(utxo_id)
            .map(|claims| claims.len() > 1)
            .unwrap_or(false)
    }

    /// Resolve a conflict using the specified strategy
    pub fn resolve_conflict(
        &self,
        utxo_id: &UTXOId,
        strategy: ConflictResolution,
    ) -> Option<SpendingClaim> {
        let claims = self.claims.get(utxo_id)?;

        if claims.is_empty() {
            return None;
        }

        if claims.len() == 1 {
            return Some(claims[0].clone());
        }

        match strategy {
            ConflictResolution::FirstSeen => {
                // Return the claim with the earliest timestamp
                claims
                    .iter()
                    .min_by_key(|c| c.timestamp())
                    .cloned()
            }
            ConflictResolution::MostWitnesses => {
                // Return the claim with the most witnesses
                claims
                    .iter()
                    .max_by_key(|c| c.witness_count())
                    .cloned()
            }
            ConflictResolution::Custom => {
                // For custom, just return the first one
                // In a real implementation, you'd pass a comparator
                Some(claims[0].clone())
            }
        }
    }

    /// Merge another detector into this one
    pub fn merge(&mut self, other: &ConflictDetector) -> DetectorMergeResult {
        let mut new_claims = 0;
        let mut conflicts_detected = 0;

        for (utxo_id, other_claims) in &other.claims {
            for claim in other_claims {
                // Check if we already have this exact claim
                let existing = self.claims.get(utxo_id);
                let already_exists = existing
                    .map(|claims| {
                        claims.iter().any(|c| c.spending_iou_id() == claim.spending_iou_id())
                    })
                    .unwrap_or(false);

                if already_exists {
                    // Merge witnesses
                    if let Some(claims) = self.claims.get_mut(utxo_id) {
                        for c in claims.iter_mut() {
                            if c.spending_iou_id() == claim.spending_iou_id() {
                                for witness in claim.witnesses() {
                                    c.add_witness(witness.clone());
                                }
                            }
                        }
                    }
                    continue;
                }

                // Check if this would create a conflict
                if let Some(existing_claims) = self.claims.get(utxo_id) {
                    if !existing_claims.is_empty() {
                        // This is a new conflicting claim
                        conflicts_detected += 1;
                        self.conflict_count += 1;
                    }
                }

                // Add the claim
                self.claims
                    .entry(utxo_id.clone())
                    .or_insert_with(Vec::new)
                    .push(claim.clone());

                new_claims += 1;
            }
        }

        DetectorMergeResult {
            new_claims,
            conflicts_detected,
        }
    }

    /// Add a witness attestation to a claim
    pub fn add_witness_to_claim(
        &mut self,
        utxo_id: &UTXOId,
        iou_id: &IOUId,
        witness: NodeId,
    ) -> bool {
        if let Some(claims) = self.claims.get_mut(utxo_id) {
            for claim in claims.iter_mut() {
                if claim.spending_iou_id() == iou_id {
                    claim.add_witness(witness);
                    return true;
                }
            }
        }
        false
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).unwrap_or_default()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ConflictError> {
        postcard::from_bytes(bytes).map_err(|_| ConflictError::DeserializationFailed)
    }

    /// Get all UTXOs that have conflicts
    pub fn conflicting_utxos(&self) -> Vec<&UTXOId> {
        self.claims
            .iter()
            .filter(|(_, claims)| claims.len() > 1)
            .map(|(utxo_id, _)| utxo_id)
            .collect()
    }

    /// Clear resolved conflicts (after settlement)
    pub fn clear_conflict(&mut self, utxo_id: &UTXOId, winning_iou_id: &IOUId) {
        if let Some(claims) = self.claims.get_mut(utxo_id) {
            claims.retain(|c| c.spending_iou_id() == winning_iou_id);
            if claims.len() <= 1 {
                // No longer a conflict
                self.conflict_count = self.conflict_count.saturating_sub(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;

    #[test]
    fn test_spending_claim_creation() {
        let alice = Keypair::generate();
        let utxo_id = UTXOId::from_bytes([1u8; 32]);
        let iou_id = IOUId::from_bytes([2u8; 32]);

        let claim = SpendingClaim::new(utxo_id, iou_id, alice.public_key());

        assert!(claim.timestamp() > 0);
        assert_eq!(claim.witness_count(), 0);
    }

    #[test]
    fn test_conflict_detector_basic() {
        let mut detector = ConflictDetector::new();

        let alice = Keypair::generate();
        let utxo_id = UTXOId::from_bytes([1u8; 32]);
        let iou_id = IOUId::from_bytes([2u8; 32]);

        let claim = SpendingClaim::new(utxo_id, iou_id, alice.public_key());
        detector.register_claim(claim).unwrap();

        assert_eq!(detector.claim_count(), 1);
        assert_eq!(detector.conflict_count(), 0);
    }

    #[test]
    fn test_double_spend_detection() {
        let mut detector = ConflictDetector::new();

        let alice = Keypair::generate();
        let utxo_id = UTXOId::from_bytes([1u8; 32]);
        let iou_id1 = IOUId::from_bytes([2u8; 32]);
        let iou_id2 = IOUId::from_bytes([3u8; 32]);

        let claim1 = SpendingClaim::new(utxo_id.clone(), iou_id1, alice.public_key());
        let claim2 = SpendingClaim::new(utxo_id, iou_id2, alice.public_key());

        detector.register_claim(claim1).unwrap();
        let result = detector.register_claim(claim2);

        assert!(matches!(result, Err(ConflictError::DoubleSpend { .. })));
        assert_eq!(detector.conflict_count(), 1);
    }
}
