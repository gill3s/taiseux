//! Taiseux : plus-values sur actifs numériques (formulaire 2086), hors ligne.
//! Copyright (C) 2026 ToolGuild. Licence MIT (voir LICENSE).
//!
//! Ce crate ne fait aucun accès réseau. Les seuls accès disque sont ceux que l'application lui demande
//! (le crate lui-même ne fait que des calculs sur des octets et des structures).

pub mod api;
pub mod csv;
pub mod engine;
pub mod imports;
pub mod vault;

pub use engine::{calculer, Annee, Cession, Operation, Resultat, TypeOperation, SEUIL_EXONERATION};
