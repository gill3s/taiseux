//! Coinbase : historique des transactions (« Transaction history », CSV). Quatre variantes d'en-tête connues :
//! - v4 : Timestamp, Transaction Type, Asset, Quantity Transacted, Price Currency, Price at Transaction, Subtotal,
//!   Total (inclusive of fees and/or spread), Fees and/or Spread, Notes (avec ou sans « ID » en tête) ;
//! - v3 : idem avec « Spot Price Currency » et « Spot Price at Transaction » ;
//! - v2 : « Total (inclusive of fees) » et « Fees » ;
//! - v1 : colonnes préfixées par la monnaie (« EUR Spot Price at Transaction », « EUR Subtotal », « EUR Total
//!   (inclusive of fees) », « EUR Fees »).
//! Des lignes d'introduction précèdent souvent l'en-tête : elles sont sautées.

use super::commun::*;
use super::{AVerifier, Import};

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Timestamp", "Transaction Type", "Asset", "Quantity Transacted"])?;
    // Monnaie des montants : colonne « Price Currency » / « Spot Price Currency », ou préfixe des colonnes (v1).
    let i_monnaie = t.col("Price Currency").or_else(|| t.col("Spot Price Currency"));
    let prefixe = t.col_si(|h| h.ends_with(" Subtotal") && h.len() == 12).map(|i| t.entete[i][..3].to_string());
    let version = if t.a("Price Currency") { "v4" } else if t.a("Fees and/or Spread") { "v3" } else if prefixe.is_some() { "v1" } else { "v2" };
    let i_sub = t.col("Subtotal").or_else(|| t.col_si(|h| h.ends_with(" Subtotal")));
    let i_total = t.col_si(|h| h.starts_with("Total (inclusive of fees") || (h.len() > 4 && h[4..].starts_with("Total (inclusive of fees")));
    let i_frais = t.col("Fees and/or Spread").or_else(|| t.col("Fees")).or_else(|| t.col_si(|h| h.len() == 8 && h.ends_with(" Fees")));
    let (i_ts, i_type, i_actif, i_q, i_notes) = (t.col("Timestamp"), t.col("Transaction Type"), t.col("Asset"), t.col("Quantity Transacted"), t.col("Notes"));

    let mut imp = Import { plateforme: "Coinbase".into(), format: format!("historique des transactions ({version})"), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = horodatage(champ(c, i_ts)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim();
        let a = actif(champ(c, i_actif));
        let q = nombre(champ(c, i_q)).map(f64::abs).unwrap_or(f64::NAN);
        let monnaie = i_monnaie.map(|i| champ(c, Some(i)).trim().to_uppercase()).or_else(|| prefixe.clone()).unwrap_or_default();
        let (sub, total, frais) = (nombre(champ(c, i_sub)).map(f64::abs), nombre(champ(c, i_total)).map(f64::abs), nombre(champ(c, i_frais)).map(f64::abs).unwrap_or(0.0));
        let notes = champ(c, i_notes).trim();
        let note = format!("Coinbase : {typ}");
        let en_euros = monnaie == "EUR";
        if est_fiat(&a) { imp.ignorees += 1; continue; } // dépôts et retraits d'euros ou d'autres monnaies
        if !(q.is_finite() && q > 0.0) && typ != "Convert" { verifier(&mut imp, &d, "quantité illisible"); continue; }
        match typ {
            "Buy" | "Advanced Trade Buy" | "Advance Trade Buy" | "Credit" => match total.or(sub) {
                Some(m) if en_euros => imp.operations.push(achat(&d, &h, "Coinbase", &a, q, m, note)),
                _ => verifier(&mut imp, &d, &format!("achat en {} : à convertir en euros et à saisir", if monnaie.is_empty() { "monnaie inconnue" } else { &monnaie })),
            },
            "Sell" | "Advanced Trade Sell" | "Advance Trade Sell" => match sub.or(total.map(|t| t + frais)) {
                Some(p) if en_euros => imp.operations.push(cession(&d, &h, "Coinbase", &a, q, p, frais, note)),
                _ => verifier(&mut imp, &d, &format!("vente en {} : à convertir en euros et à saisir", if monnaie.is_empty() { "monnaie inconnue" } else { &monnaie })),
            },
            "Convert" => match conversion(notes) {
                Some((qd, ad, qr, ar)) if ad == a || a.is_empty() => imp.operations.push(echange(&d, &h, "Coinbase", &ad, qd, &ar, qr, note)),
                _ => verifier(&mut imp, &d, "conversion : la note « Converted … to … » est illisible"),
            },
            "Send" | "Exchange Deposit" | "Pro Deposit" | "Prime Deposit" | "Exchange Withdrawal" | "Pro Withdrawal" | "Retail MGX DEX Send" =>
                imp.operations.push(transfert(&d, &h, "Coinbase", &a, q, 0.0, note)),
            "Receive" if notes.contains("Coinbase Earn") || notes.contains("Coinbase Rewards") || notes.contains("Coinbase Referral") =>
                imp.operations.push(revenu(&d, &h, "Coinbase", &a, q, note)),
            "Receive" => imp.operations.push(transfert(&d, &h, "Coinbase", &a, q, 0.0, note)),
            "Rewards Income" | "Reward Income" | "Staking Income" | "Inflation Reward" | "Coinbase Earn" | "Learning Reward" | "Interest payout" =>
                imp.operations.push(revenu(&d, &h, "Coinbase", &a, q, note)),
            "Deposit" | "Withdrawal" | "Vault Withdrawal" | "Cash to Savings" | "Savings to Cash" | "Transfer" | "Retail Staking Transfer" | "Retail Unstaking Transfer" =>
                imp.ignorees += 1,
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    Some(imp)
}

/// « Converted 0.01 BTC to 0.25 ETH » → (0.01, BTC, 0.25, ETH).
fn conversion(notes: &str) -> Option<(f64, String, f64, String)> {
    let reste = notes.trim().strip_prefix("Converted ")?;
    let (gauche, droite) = reste.split_once(" to ")?;
    let mot = |s: &str| -> Option<(f64, String)> {
        let mut it = s.split_whitespace();
        let q = nombre(it.next()?)?;
        let a = actif(it.next()?);
        Some((q, a))
    };
    let ((qd, ad), (qr, ar)) = (mot(gauche)?, mot(droite)?);
    Some((qd, ad, qr, ar))
}
