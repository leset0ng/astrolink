//! Not adapted: the GAP client, LE CoC owner handlers and QuickJS Feature
//! registry of this firmware have not been recovered. See
//! `docs/target-support.md` for the evidence that was checked and the reason
//! this target stops here. The module loads but publishes the Feature as
//! unsupported and never calls firmware.

crate::abi_unsupported!("xiaomi-band-11-4.100.179");
