//! Error types exposed by this crate.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

use crate::raft_types::SnapshotSegmentId;
use crate::LogId;
use crate::Membership;
use crate::NodeId;
use crate::StorageError;

/// Fatal is unrecoverable and shuts down raft at once.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fatal {
    #[error(transparent)]
    StorageError(#[from] StorageError),

    #[error("raft stopped")]
    Stopped,
}

/// Extract Fatal from a Result.
///
/// Fatal will shutdown the raft and needs to be dealt separately,
/// such as StorageError.
pub trait ExtractFatal
where Self: Sized
{
    fn extract_fatal(self) -> Result<Self, Fatal>;
}

impl<T, E> ExtractFatal for Result<T, E>
where E: TryInto<Fatal> + Clone
{
    fn extract_fatal(self) -> Result<Self, Fatal> {
        if let Err(e) = &self {
            let fatal = e.clone().try_into();
            if let Ok(f) = fatal {
                return Err(f);
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, thiserror::Error, derive_more::TryInto)]
pub enum AppendEntriesError {
    #[error(transparent)]
    Fatal(#[from] Fatal),
}

#[derive(Debug, Clone, thiserror::Error, derive_more::TryInto)]
pub enum VoteError {
    #[error(transparent)]
    Fatal(#[from] Fatal),
}

#[derive(Debug, Clone, thiserror::Error, derive_more::TryInto)]
pub enum InstallSnapshotError {
    #[error(transparent)]
    SnapshotMismatch(#[from] SnapshotMismatch),

    #[error(transparent)]
    Fatal(#[from] Fatal),
}

/// An error related to a client read request.
#[derive(Debug, Clone, thiserror::Error, derive_more::TryInto)]
pub enum ClientReadError {
    #[error(transparent)]
    ForwardToLeader(#[from] ForwardToLeader),

    #[error(transparent)]
    QuorumNotEnough(#[from] QuorumNotEnough),

    #[error(transparent)]
    Fatal(#[from] Fatal),
}

/// An error related to a client write request.
#[derive(Debug, Clone, thiserror::Error, derive_more::TryInto)]
pub enum ClientWriteError {
    // #[error("{0}")]
    // RaftError(#[from] RaftError),
    #[error(transparent)]
    ForwardToLeader(#[from] ForwardToLeader),

    /// When writing a change-membership entry.
    #[error(transparent)]
    ChangeMembershipError(#[from] ChangeMembershipError),

    #[error(transparent)]
    Fatal(#[from] Fatal),
}

/// The set of errors which may take place when requesting to propose a config change.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ChangeMembershipError {
    #[error("the cluster is already undergoing a configuration change at log {membership_log_id}")]
    InProgress { membership_log_id: LogId },

    #[error("new membership can not be empty")]
    EmptyMembership,

    // TODO(xp): 111 test it
    #[error("to add a member {node_id} first need to add it as learner")]
    LearnerNotFound { node_id: NodeId },

    // TODO(xp): 111 test it
    #[error("replication to learner {node_id} is lagging {distance}, matched: {matched:?}, can not add as member")]
    LearnerIsLagging {
        node_id: NodeId,
        matched: Option<LogId>,
        distance: u64,
    },

    // TODO(xp): test it in unittest
    // TODO(xp): rename this error to some elaborated name.
    // TODO(xp): 111 test it
    #[error("now allowed to change from {curr:?} to {to:?}")]
    Incompatible { curr: Membership, to: BTreeSet<NodeId> },
}

#[derive(Debug, thiserror::Error)]
pub enum AddLearnerError {
    #[error(transparent)]
    ForwardToLeader(#[from] ForwardToLeader),

    #[error("node {0} is already a learner")]
    Exists(NodeId),

    #[error(transparent)]
    Fatal(#[from] Fatal),
}

/// The set of errors which may take place when initializing a pristine Raft node.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InitializeError {
    /// The requested action is not allowed due to the Raft node's current state.
    #[error("the requested action is not allowed due to the Raft node's current state")]
    NotAllowed,

    #[error(transparent)]
    Fatal(#[from] Fatal),
}

impl From<StorageError> for AppendEntriesError {
    fn from(s: StorageError) -> Self {
        let f: Fatal = s.into();
        f.into()
    }
}
impl From<StorageError> for VoteError {
    fn from(s: StorageError) -> Self {
        let f: Fatal = s.into();
        f.into()
    }
}
impl From<StorageError> for InstallSnapshotError {
    fn from(s: StorageError) -> Self {
        let f: Fatal = s.into();
        f.into()
    }
}
impl From<StorageError> for ClientReadError {
    fn from(s: StorageError) -> Self {
        let f: Fatal = s.into();
        f.into()
    }
}
impl From<StorageError> for InitializeError {
    fn from(s: StorageError) -> Self {
        let f: Fatal = s.into();
        f.into()
    }
}
impl From<StorageError> for AddLearnerError {
    fn from(s: StorageError) -> Self {
        let f: Fatal = s.into();
        f.into()
    }
}

/// Error variants related to the Replication.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
pub enum ReplicationError {
    #[error("seen a higher term: {higher} GT mine: {mine}")]
    HigherTerm { higher: u64, mine: u64 },

    #[error("Replication is closed")]
    Closed,

    #[error("{0}")]
    LackEntry(#[from] LackEntry),

    #[error("leader committed index {committed_index} advances target log index {target_index} too many")]
    CommittedAdvanceTooMany { committed_index: u64, target_index: u64 },

    // TODO(xp): two sub type: StorageError / TransportError
    // TODO(xp): a sub error for just send_append_entries()
    #[error("{0}")]
    StorageError(#[from] StorageError),

    #[error(transparent)]
    IO {
        #[backtrace]
        #[from]
        source: std::io::Error,
    },

    #[error("timeout after {timeout:?} to replicate {id}->{target}")]
    Timeout {
        id: NodeId,
        target: NodeId,
        timeout: Duration,
    },

    #[error(transparent)]
    Network {
        #[backtrace]
        source: anyhow::Error,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("store has no log at: {index:?}")]
pub struct LackEntry {
    pub index: Option<u64>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("has to forward request to: {leader_id:?}")]
pub struct ForwardToLeader {
    pub leader_id: Option<NodeId>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("snapshot segment id mismatch, expect: {expect}, got: {got}")]
pub struct SnapshotMismatch {
    pub expect: SnapshotSegmentId,
    pub got: SnapshotSegmentId,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("not enough for a quorum, cluster: {cluster}, got: {got:?}")]
pub struct QuorumNotEnough {
    pub cluster: String,
    pub got: BTreeSet<NodeId>,
}