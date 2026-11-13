//! Bitvavo : historique des transactions (« Transaction history » → Export, CSV). Une variante d'en-tête connue :
//! Timezone, Date, Time, Type, Currency, Amount, Quote Currency, Quote Price, Received / Paid Currency,
//! Received / Paid Amount, Fee currency, Fee amount, Status, Transaction ID, Address.
//! Date et heure sont locales (colonne Timezone, ex. « Europe/Amsterdam ») ; l'heure peut porter des millisecondes.
//! Types vus : buy, sell, deposit, withdrawal. Statut : « Completed ».
//! Sources : Bitvavo n'est pas dans BittyTax. Aide de Bitvavo (« How can I view and download my transaction
//! history », support.bitvavo.com : procédure d'export, sans description des colonnes) ; exemple d'export
//! anonymisé publié dans le projet open source Export-To-Ghostfolio (`samples/bitvavo-export.csv`, noms de colonnes
//! et valeurs seulement, aucun code repris) ; liste des catégories d'opérations de l'aide de Blockpit (staking,
//! affiliation, airdrops, bonus de bienvenue, « remove dust », garantie de prix, vente à découvert).
//!
//! Dans l'exemple, un achat porte « Received / Paid Amount » = −10,00 € pour 0,0053543 ETH à 1 862,80 € et
//! 0,026 € de frais : le montant payé comprend les frais (0,0053543 × 1 862,8 + 0,026 = 10,00). Le lecteur ne le
//! suppose pas : il compare les deux hypothèses (frais compris ou non) à quantité × cours, retient la plus proche,
//! et renvoie la ligne « à vérifier » si aucune ne tombe à moins de 2 %.
//! Les noms exacts des types autres que buy, sell, deposit et withdrawal ne sont pas documentés : staking,
//! affiliate, airdrop et welcome bonus sont lus comme revenus gratuits ; tout autre type part « à vérifier ».

use super::commun::*;
use super::{AVerifier, Import};

const P: &str = "Bitvavo";

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Timezone", "Date", "Time", "Type", "Currency", "Amount", "Received / Paid Currency", "Received / Paid Amount"])?;
    let (i_date, i_heure, i_type, i_cur, i_m, i_cours, i_rp_cur, i_rp_m, i_f_cur, i_f_m, i_statut) = (
        t.col("Date"), t.col("Time"), t.col("Type"), t.col("Currency"), t.col("Amount"), t.col("Quote Price"),
        t.col("Received / Paid Currency"), t.col("Received / Paid Amount"), t.col("Fee currency"), t.col("Fee amount"), t.col("Status"),
    );
    let mut imp = Import { plateforme: P.into(), format: "historique des transactions".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, _)) = horodatage(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let h: String = champ(c, i_heure).trim().chars().take(8).collect();
        let h = if h.len() == 8 && h.as_bytes()[2] == b':' { h } else { String::new() };
        let statut = champ(c, i_statut).trim();
        if !statut.is_empty() && !statut.eq_ignore_ascii_case("completed") { imp.ignorees += 1; continue; }
        let typ = champ(c, i_type).trim().to_lowercase();
        let a = actif(champ(c, i_cur));
        let Some(q) = nombre(champ(c, i_m)).map(f64::abs) else { verifier(&mut imp, &d, "quantité illisible"); continue };
        let (rp_cur, rp) = (actif(champ(c, i_rp_cur)), nombre(champ(c, i_rp_m)).map(f64::abs));
        let (f_cur, frais) = (actif(champ(c, i_f_cur)), nombre(champ(c, i_f_m)).map(f64::abs).unwrap_or(0.0));
        let cours = nombre(champ(c, i_cours)).map(f64::abs);
        let note = format!("Bitvavo : {typ}");

        if est_fiat(&a) {
            if matches!(typ.as_str(), "deposit" | "withdrawal") { imp.ignorees += 1; } else { verifier(&mut imp, &d, &format!("opération « {typ} » sur {a} non prise en charge")); }
            continue;
        }
        if !(q > 0.0) { verifier(&mut imp, &d, "quantité nulle"); continue; }
        match typ.as_str() {
            "buy" | "sell" => {
                let vente = typ == "sell";
                if rp_cur != "EUR" {
                    verifier(&mut imp, &d, &format!("{} en {} : à convertir en euros et à saisir", if vente { "vente" } else { "achat" }, if rp_cur.is_empty() { "monnaie inconnue" } else { &rp_cur }));
                    continue;
                }
                if frais > 0.0 && f_cur != "EUR" { verifier(&mut imp, &d, &format!("frais payés en {f_cur} : à saisir vous-même")); continue; }
                let Some(rp) = rp else { verifier(&mut imp, &d, "montant en euros illisible"); continue };
                // Hypothèse A : montant payé frais compris (achat) / montant reçu net de frais (vente).
                // Hypothèse B : montant hors frais.
                let (a_ok, b_ok) = if vente { (rp + frais, rp) } else { (rp, rp + frais) };
                // Valeur hors frais attendue sous chaque hypothèse, comparée à quantité × cours.
                let total = match cours {
                    Some(px) if px > 0.0 => {
                        let brut = q * px;
                        let ea = ((if vente { rp + frais } else { rp - frais }) - brut).abs();
                        let eb = (rp - brut).abs();
                        if ea.min(eb) > 0.02 * brut { verifier(&mut imp, &d, "montant en euros incohérent avec quantité × cours : à vérifier"); continue; }
                        if ea <= eb { a_ok } else { b_ok }
                    }
                    _ => a_ok,
                };
                if vente { imp.operations.push(cession(&d, &h, P, &a, q, total, frais, note)) } else { imp.operations.push(achat(&d, &h, P, &a, q, total, note)) }
            }
            "deposit" => imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note)),
            "withdrawal" => imp.operations.push(transfert(&d, &h, P, &a, q, if f_cur == a { frais } else { 0.0 }, note)),
            "staking" | "affiliate" | "airdrop" | "welcome bonus" => imp.operations.push(revenu(&d, &h, P, &a, q, note)),
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    Some(imp)
}
