//! Módulo de IA de FocusFlow. Punto de entrada de la interpretación:
//! [intent_parser::interpret].
//!
//! Arquitectura (spec/09):
//! ```text
//! texto libre ──► interpret(text, provider, configured)
//!                    ├─ AI ─► provider.chat_json(...) ─► JSON ─┐
//!                    └─ local ─► RuleBasedProvider ────────────┤
//!                                                              ▼
//!                                        parse_intent_json (tolerante)
//!                                                              ▼
//!                                        validate_intent (invariantes)
//!                                                              ▼
//!                                      IntentBatch + summary + confianza
//! ```

pub mod email_parser;
pub mod intent;
pub mod intent_parser;
pub mod intent_validator;
pub mod nl;
pub mod provider;
pub mod rule_based;
pub mod task_parser;
pub mod validation;

pub use provider::*;
