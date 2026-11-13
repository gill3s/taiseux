//! Bitpanda : historique des transactions (« Transaction history », CSV, compte → Historique → Exporter). Deux
//! variantes d'en-tête connues :
//! - actuelle : Transaction ID, Timestamp, Transaction Type, In/Out, Amount Fiat, Fiat, Amount Asset, Asset,
//!   Asset market price, Asset market price currency, Asset class, Product ID, Fee, Fee asset, Spread,
//!   Spread Currency[, Tax Fiat] ; précédée de lignes d'introduction (avertissement, identifiant) ;
//! - ancienne : ID, Type, In/Out, Amount Fiat, Fee, Fiat, Amount Asset, Asset, Status, Created at.
//! Types : deposit, withdrawal, buy, sell, transfer. Les cellules vides valent « - ».
//! Sources : noms de colonnes et valeurs de type relevés dans BittyTax (`conv/parsers/bitpanda.py`) et dans la
//! documentation du crate Rust `bitpanda-csv` (exemple anonymisé, docs.rs) ; aucun code repris.
//!
//! Prudence :
//! - « Asset class » autre que crypto (actions, ETF, métaux, « derivative ») : hors du 2086, « à vérifier ».
//!   Certaines cryptos (MATIC, SHIB) sont étiquetées « Fiat » : on se fie au code de l'actif, pas à la classe.
//! - « transfer » couvre à la fois des récompenses (BEST, staking) et des mouvements internes : « à vérifier ».
//! - Un achat et une vente au même instant peuvent être un échange crypto contre crypto (Bitpanda Swap), qui est
//!   en sursis d'imposition : les deux lignes partent « à vérifier » plutôt que d'inventer une cession.
//! - Frais distincts sur un achat ou une vente : on ne sait pas s'ils sont compris dans « Amount Fiat » → « à vérifier ».

use std::collections::BTreeMap;

use super::commun::*;
use super::{AVerifier, Import};

const P: &str = "Bitpanda";

pub fn lire(texte: &str) -> Option<Import> {
    let (t, actuelle) = if let Some(t) = Table::trouver(texte, &["Transaction ID", "Timestamp", "Transaction Type", "In/Out", "Amount Fiat", "Amount Asset", "Asset class"]) {
        (t, true)
    } else {
        (Table::trouver(texte, &["ID", "Type", "In/Out", "Amount Fiat", "Fiat", "Amount Asset", "Asset", "Status", "Created at"])?, false)
    };
    let i_date = if actuelle { t.col("Timestamp") } else { t.col("Created at") };
    let i_type = if actuelle { t.col("Transaction Type") } else { t.col("Type") };
    let (i_fiat_m, i_fiat, i_q, i_actif, i_classe, i_frais, i_frais_actif, i_statut) = (
        t.col("Amount Fiat"), t.col("Fiat"), t.col("Amount Asset"), t.col("Asset"), t.col("Asset class"), t.col("Fee"), t.col("Fee asset"), t.col("Status"),
    );
    let format = if actuelle { "historique des transactions" } else { "historique des transactions (ancien format)" };
    let mut imp = Import { plateforme: P.into(), format: format.into(), ..Default::default() };

    // Achats et ventes de crypto par horodatage exact : un achat et une vente simultanés = échange possible.
    let mut sens_par_instant: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for (_, c, _) in &t.lignes {
        let typ = champ(c, i_type).trim().to_lowercase();
        if !est_fiat(&actif(champ(c, i_actif))) && (typ == "buy" || typ == "sell") {
            let e = sens_par_instant.entry(champ(c, i_date).trim().to_string()).or_default();
            if typ == "buy" { e.0 = true } else { e.1 = true }
        }
    }

    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        // Fichier à plusieurs tableaux : lignes de séparation et en-têtes répétés.
        if c.len() <= 1 || *c == t.entete { continue; }
        let Some((d, h)) = horodatage(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        if i_statut.is_some() && !champ(c, i_statut).trim().eq_ignore_ascii_case("finished") { imp.ignorees += 1; continue; }
        let typ = champ(c, i_type).trim().to_lowercase();
        let a = actif(champ(c, i_actif));
        let classe = champ(c, i_classe).trim();
        let q = nombre(champ(c, i_q)).map(f64::abs);
        let fiat_m = nombre(champ(c, i_fiat_m)).map(f64::abs);
        let fiat = actif(champ(c, i_fiat));
        let frais = nombre(champ(c, i_frais)).map(f64::abs).unwrap_or(0.0);
        let frais_actif = actif(champ(c, i_frais_actif));
        let note = format!("Bitpanda : {typ}");

        if est_fiat(&a) || q.is_none() && matches!(typ.as_str(), "deposit" | "withdrawal") {
            if matches!(typ.as_str(), "deposit" | "withdrawal") { imp.ignorees += 1; } else { verifier(&mut imp, &d, &format!("opération « {typ} » sur une monnaie officielle ({a}) non prise en charge")); }
            continue;
        }
        if !classe.is_empty() && !classe.to_lowercase().contains("crypto") && classe != "Fiat" {
            verifier(&mut imp, &d, &format!("actif de classe « {classe} » (actions, ETF, métaux…) : hors du champ du 2086"));
            continue;
        }
        let Some(q) = q.filter(|q| *q > 0.0) else { verifier(&mut imp, &d, "quantité illisible"); continue };
        match typ.as_str() {
            "deposit" => imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note)),
            "withdrawal" => imp.operations.push(transfert(&d, &h, P, &a, q, if frais_actif == a || frais_actif.is_empty() { frais } else { 0.0 }, note)),
            "buy" | "sell" => {
                let (achete, vendu) = sens_par_instant.get(champ(c, i_date).trim()).copied().unwrap_or_default();
                if achete && vendu {
                    verifier(&mut imp, &d, "achat et vente au même instant : peut-être un échange crypto contre crypto (en sursis), à saisir vous-même");
                } else if fiat != "EUR" {
                    verifier(&mut imp, &d, &format!("{} en {} : à convertir en euros et à saisir", if typ == "buy" { "achat" } else { "vente" }, if fiat.is_empty() { "monnaie inconnue" } else { &fiat }));
                } else if frais > 0.0 {
                    verifier(&mut imp, &d, "frais distincts : impossible de savoir s'ils sont compris dans le montant en euros");
                } else if let Some(m) = fiat_m {
                    if typ == "buy" { imp.operations.push(achat(&d, &h, P, &a, q, m, note)) } else { imp.operations.push(cession(&d, &h, P, &a, q, m, 0.0, note)) }
                } else {
                    verifier(&mut imp, &d, "montant en euros illisible");
                }
            }
            "transfer" => verifier(&mut imp, &d, "transfert Bitpanda : récompense (revenu gratuit) ou mouvement entre vos comptes ? À préciser"),
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    Some(imp)
}
