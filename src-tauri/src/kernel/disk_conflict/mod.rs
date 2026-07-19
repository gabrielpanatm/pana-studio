mod gate;
mod model;
mod scanner;

pub use gate::evaluate_disk_conflict_gate;
pub use model::{
    KernelDiskConflictFileSnapshot, KernelDiskConflictGateAction, KernelDiskConflictGateDecision,
    KernelDiskConflictGateDiagnostic, KernelDiskConflictGateDiagnosticSeverity,
    KernelDiskConflictGatePolicy, KernelDiskConflictGateRequest, KernelDiskConflictGateResult,
    KernelDiskConflictKind, KernelDiskConflictSnapshot, KernelDiskConflictStatus,
    KernelDiskConflictSummary, KERNEL_DISK_CONFLICT_GATE_SCHEMA_VERSION,
    KERNEL_DISK_CONFLICT_SCHEMA_VERSION,
};
pub use scanner::scan_disk_conflicts;
