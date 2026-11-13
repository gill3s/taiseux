//! Electrum : historique du portefeuille (menu Portefeuille → Historique → « Exporter », format CSV).
//!
//! Sources : code source d'Electrum (export de l'historique : colonnes transaction_hash, label, confirmations,
//! value, fiat_value, fee, fiat_fee, timestamp) et en-têtes relevés par BittyTax (`electrum.py`, noms de colonnes
//! seulement). Variantes d'en-tête connues :
//! - actuelle : transaction_hash, label, confirmations, value, fiat_value, fee, fiat_fee, timestamp ;
//! - ancienne : transaction_hash, label, [confirmations,] value, timestamp (sans frais).
//! « value » est la variation nette du portefeuille, en BTC, signée : un envoi la diminue des frais compris ; « fee »
//! n'est renseigné que si toutes les entrées appartiennent au portefeuille. La date est l'heure locale de
//! l'ordinateur qui a exporté.
//!
//! **Actif supposé : BTC.** Electrum est un portefeuille bitcoin et son export n'écrit pas l'actif. Les dérivés
//! (Electrum-LTC, Electron Cash…) écrivent le même en-tête : la note de chaque opération le rappelle.
//!
//! Traduction : réception → transfert (supposé venir de vos comptes, signalé) ; envoi → transfert, quantité =
//! |value| − fee, frais → `quantite_frais` ; envoi dont la sortie se réduit aux frais → consolidation entre vos
//! adresses (frais seuls) ; variation nulle → ignorée ; date absente (transaction non confirmée) → « à vérifier ».

use super::commun::*;
use super::portefeuilles::*;
use super::{AVerifier, Import};

const P: &str = "Electrum";

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["transaction_hash", "label", "value", "timestamp"])?;
    let (i_tx, i_label, i_v, i_frais, i_ts) = (t.col("transaction_hash"), t.col("label"), t.col("value"), t.col("fee"), t.col("timestamp"));
    let format = if t.a("fee") { "historique du portefeuille (BTC)" } else { "historique du portefeuille, ancien format sans frais (BTC)" };
    let mut imp = Import { plateforme: P.into(), format: format.into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = horodatage(champ(c, i_ts)) else { verifier(&mut imp, "", "date absente ou illisible (transaction non confirmée ?)"); continue };
        let Some(v) = nombre(champ(c, i_v)) else { verifier(&mut imp, &d, "montant illisible"); continue };
        let frais = nombre(champ(c, i_frais)).map(f64::abs);
        let label = champ(c, i_label).trim();
        let mut detail = if label.is_empty() { String::new() } else { format!("« {label} »") };
        if champ(c, i_tx).trim().is_empty() { detail = if detail.is_empty() { "Lightning ?".into() } else { format!("{detail}, Lightning ?") }; }
        match mouvement_bitcoin(&d, &h, P, v, frais, &detail) {
            Some(mut o) => {
                o.note.push_str(" Actif supposé : BTC (Electrum ne l'écrit pas ; corrigez-le si l'export vient d'Electrum-LTC ou d'Electron Cash).");
                imp.operations.push(o);
            }
            None => imp.ignorees += 1,
        }
    }
    Some(imp)
}
