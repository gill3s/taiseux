//! Trezor Suite : export des transactions d'un compte (« Exporter » → CSV).
//!
//! Sources : code source de Trezor Suite (`packages/suite/src/utils/wallet/exportTransactionsUtils.ts`, dépôt public
//! trezor/trezor-suite : noms de colonnes, une ligne par destinataire, frais écrits une seule fois par transaction,
//! types `SENT`, `RECV`, `SELF`, `JOINT`, `CONTRACT`, `FAILED`, `UNKNOWN`) et en-têtes relevés par BittyTax
//! (`trezorsuite.py` et `trezor.py`, noms de colonnes seulement). Variantes d'en-tête connues :
//! - actuelle : Timestamp, Date, Time, Type, Transaction ID, Fee, Fee unit, Address, Label, Amount, Amount unit,
//!   Fiat (EUR) (ou autre monnaie), Other ;
//! - ancienne Trezor Suite : Date & Time, Type, Transaction ID, Addresses, Fee, Total ;
//! - ancien portefeuille web Trezor : Date, Time, TX id, Address, [Address Label,] TX type, Value, TX total, Balance.
//! Les deux anciennes variantes **n'indiquent pas l'actif** (il n'était que dans le nom du fichier, que Taiseux ne
//! voit pas) : elles sont reconnues, mais toutes leurs lignes partent dans « à vérifier », avec le conseil de refaire
//! l'export depuis une version récente de Trezor Suite.
//!
//! Traduction (variante actuelle), ligne par ligne :
//! - SENT : envoi → transfert (supposé vers vos comptes, signalé) ; « Amount » est la somme reçue par le destinataire,
//!   hors frais ; les frais (première ligne de la transaction) vont dans `quantite_frais`, ou dans un transfert à part
//!   s'ils sont payés dans un autre actif (envoi d'un jeton : frais en ETH) ;
//! - RECV : réception → transfert (supposé venir de vos comptes, signalé) ;
//! - SELF : envoi à vous-même → transfert, frais seuls ;
//! - FAILED : transaction échouée → seuls les frais sont perdus ;
//! - JOINT (coinjoin), CONTRACT (appel de contrat : échange décentralisé, staking…), UNKNOWN, opérations de staking
//!   Cardano, NFT (« ID … »), jeton sans symbole : « à vérifier ».
//! Les montants sont écrits en unités (BTC, ETH…) ; une unité « sat » est convertie en BTC par sécurité.

use super::commun::*;
use super::portefeuilles::*;
use super::{AVerifier, Import};

const P: &str = "Trezor Suite";
const STAKING_CARDANO: &[&str] = &["Stake", "Unstake", "Claim rewards", "Change delegate"];

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["Timestamp", "Type", "Transaction ID", "Fee", "Fee unit", "Amount", "Amount unit"]) {
        return Some(actuel(t));
    }
    if let Some(t) = Table::trouver(texte, &["Date & Time", "Type", "Transaction ID", "Addresses", "Fee", "Total"]) {
        return Some(sans_actif(t, "Date & Time", "export de l'ancienne Trezor Suite"));
    }
    if let Some(t) = Table::trouver(texte, &["Date", "Time", "TX id", "TX type", "Value", "TX total"]) {
        return Some(sans_actif(t, "Date", "export de l'ancien portefeuille web Trezor"));
    }
    None
}

/// Quantité et actif, unités de base converties.
fn quantite(q: &str, unite: &str) -> Option<(f64, String)> {
    let v = nombre(q)?.abs();
    Some(match unite_de_base(unite) { Some((a, div)) => (v / div, a.to_string()), None => (v, actif(unite)) })
}

fn actuel(t: Table) -> Import {
    let (i_ts, i_type, i_tx, i_frais, i_ufrais, i_adr, i_label, i_q, i_unite) = (
        t.col("Timestamp"), t.col("Type"), t.col("Transaction ID"), t.col("Fee"), t.col("Fee unit"),
        t.col("Address"), t.col("Label"), t.col("Amount"), t.col("Amount unit"),
    );
    let mut imp = Import { plateforme: P.into(), format: "export des transactions".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = date_unix(champ(c, i_ts)) else { verifier(&mut imp, "", "date illisible (Timestamp)"); continue };
        let typ = champ(c, i_type).trim().to_uppercase();
        let (q_brut, adresse, label) = (champ(c, i_q).trim(), champ(c, i_adr).trim(), champ(c, i_label).trim());
        let tx = champ(c, i_tx).trim();
        let frais = if champ(c, i_frais).trim().is_empty() { None } else { quantite(champ(c, i_frais), champ(c, i_ufrais)) };
        let frais = frais.filter(|(f, a)| *f > 0.0 && !a.is_empty());
        let detail = if label.is_empty() { String::new() } else { format!("« {label} »") };

        if q_brut.starts_with("ID ") { verifier(&mut imp, &d, "NFT : hors du suivi automatique de Taiseux, à traiter à part"); continue; }
        if STAKING_CARDANO.contains(&adresse) {
            verifier(&mut imp, &d, &format!("opération de staking Cardano (« {adresse} ») : à saisir vous-même (récompenses retirées, frais, dépôt)"));
            continue;
        }
        match typ.as_str() {
            "JOINT" => { verifier(&mut imp, &d, "coinjoin : vos avoirs restent à vous, mais les frais (coordinateur, réseau) sont à saisir vous-même"); continue; }
            "SENT" | "RECV" | "SELF" | "FAILED" => {}
            "CONTRACT" => { verifier(&mut imp, &d, "appel de contrat (échange décentralisé, staking…) : nature inconnue, à saisir vous-même"); continue; }
            autre => { verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")); continue; }
        }
        // Ligne sans montant (sortie OP_RETURN, transaction échouée…) : seuls les frais comptent.
        let mouvement = if q_brut.is_empty() || typ == "FAILED" { None } else {
            match quantite(q_brut, champ(c, i_unite)) {
                Some((q, a)) if actif_lisible(&a) => Some((q, a)),
                Some((_, a)) => { verifier(&mut imp, &d, &format!("jeton illisible (« {a} ») : à saisir vous-même")); continue; }
                None => { verifier(&mut imp, &d, "montant illisible"); continue; }
            }
        };
        let note_frais = if typ == "FAILED" { format!("{P} : transaction échouée {tx}, seuls les frais sont perdus") } else { format!("{P} : frais de réseau de la transaction {tx}") };
        let avant = imp.operations.len();
        let mut frais_restants = frais.clone();
        if let Some((q, a)) = mouvement {
            // Frais dans le même actif : sur la même opération ; sinon, transfert à part (plus bas).
            let f = match &frais { Some((f, fa)) if *fa == a && typ != "RECV" => { frais_restants = None; *f } _ => 0.0 };
            let note = match typ.as_str() {
                "SENT" => note_envoi(P, &detail),
                "RECV" => note_reception(P, &detail),
                _ => format!("{P} : envoi à vous-même (transfert interne au portefeuille)"),
            };
            imp.operations.push(transfert(&d, &h, P, &a, q, f, note));
        }
        if typ == "RECV" { frais_restants = None; } // frais payés par l'expéditeur
        if let Some((f, fa)) = frais_restants { imp.operations.push(transfert(&d, &h, P, &fa, 0.0, f, note_frais)); }
        if imp.operations.len() == avant { imp.ignorees += 1; } // ni montant ni frais (sortie OP_RETURN seule…)
    }
    imp
}

fn sans_actif(t: Table, col_date: &str, format: &str) -> Import {
    let i_date = t.col(col_date);
    let mut imp = Import { plateforme: P.into(), format: format!("{format} (actif non indiqué)"), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let date = horodatage(champ(c, i_date)).map(|(d, _)| d).unwrap_or_default();
        imp.a_verifier.push(AVerifier {
            ligne: *n, date,
            motif: "cet ancien export Trezor n'indique pas l'actif (il n'était que dans le nom du fichier) : refaites l'export \
                    depuis une version récente de Trezor Suite, ou saisissez l'opération vous-même".into(),
            contenu: brut.clone(),
        });
    }
    imp
}
