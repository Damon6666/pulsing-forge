pub mod gates;
pub use gates::{Letter, Verdict, CollectGate, VerdictTable,
                classify, report_is_final, extract_reject_for_relay};
