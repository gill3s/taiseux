//! Sparrow Wallet : export des transactions d'un portefeuille (onglet Transactions → « Export CSV », ou
//! Fichier → Exporter le portefeuille → Transactions).
//!
//! Sparrow n'est pas couvert par BittyTax. Source : code source de Sparrow (dépôt public sparrowwallet/sparrow,
//! `src/main/java/com/sparrowwallet/sparrow/io/WalletTransactions.java`, noms de colonnes et formats seulement).
//! En-tête : Date (UTC), Label, Value (<unité>), Balance (<unité>), Fee (<unité>), [Value (<monnaie>),] Txid.
//! - unité « BTC » (huit décimales, point ou virgule décimale selon le réglage de Sparrow) ou « sats » (entier) ;
//! - « Value (EUR) » (ou autre monnaie) n'existe que si un cours est configuré ; il est alors suivi d'une ligne de
//!   commentaire « # Historical … values … » ;
//! - « Unconfirmed » à la place de la date pour une transaction non confirmée.
//! « Value » est la variation nette du portefeuille (un envoi la diminue des frais compris) ; « Fee » n'est
//! renseigné que si toutes les entrées appartiennent au portefeuille, et vaut 0 pour une transaction de minage
//! (coinbase). Sparrow est un portefeuille bitcoin seulement : l'actif est BTC.
//!
//! Traduction : réception → transfert (supposé venir de vos comptes, signalé) ; réception de minage (frais = 0) →
//! « à vérifier » (elle ne peut venir d'aucun de vos comptes : c'est un revenu, dont la valeur est à saisir) ; envoi
//! → transfert, quantité = |value| − fee ; envoi réduit aux frais → consolidation ; envoi sans frais connus
//! (coinjoin, payjoin) → transfert signalé ; « Unconfirmed » → « à vérifier ».

use super::commun::*;
use super::portefeuilles::*;
use super::{AVerifier, Import};

const P: &str = "Sparrow";

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Date (UTC)", "Label", "Txid"])?;
    let unite = |h: &str, prefixe: &str| h.strip_prefix(prefixe).and_then(|r| r.strip_suffix(')')).map(str::to_string);
    let i_v = t.col_si(|h| matches!(unite(h, "Value (").as_deref(), Some("BTC" | "sats")))?;
    let u = unite(&t.entete[i_v], "Value (").unwrap();
    let i_frais = t.col(&format!("Fee ({u})"));
    if !t.a(&format!("Balance ({u})")) || i_frais.is_none() { return None; }
    let div = if u == "sats" { 1e8 } else { 1.0 };
    let (i_date, i_label) = (t.col("Date (UTC)"), t.col("Label"));
    let mut imp = Import { plateforme: P.into(), format: format!("export des transactions (montants en {u})"), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        if brut.trim_start().starts_with('#') { continue; } // commentaire final sur les cours
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let date = champ(c, i_date).trim();
        if date.eq_ignore_ascii_case("Unconfirmed") { verifier(&mut imp, "", "transaction non confirmée : à réimporter une fois confirmée"); continue; }
        let Some((d, h)) = horodatage(date) else { verifier(&mut imp, "", "date illisible"); continue };
        let Some(v) = nombre(champ(c, Some(i_v))).map(|v| v / div) else { verifier(&mut imp, &d, "montant illisible"); continue };
        let frais = nombre(champ(c, i_frais)).map(|f| f.abs() / div);
        if v > 0.0 && frais == Some(0.0) {
            verifier(&mut imp, &d, "récompense de minage (coinbase) : c'est un revenu, ni un achat ni un transfert ; à saisir avec sa valeur");
            continue;
        }
        let label = champ(c, i_label).trim();
        let detail = if label.is_empty() { String::new() } else { format!("« {label} »") };
        match mouvement_bitcoin(&d, &h, P, v, frais, &detail) {
            Some(o) => imp.operations.push(o),
            None => imp.ignorees += 1,
        }
    }
    Some(imp)
}
