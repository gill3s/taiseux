//! MEXC : historique des transactions, des ordres, des dépôts et des retraits (CSV). Source : noms de colonnes et
//! valeurs relevés par BittyTax (`conv/parsers/mexc.py`, noms de colonnes et valeurs de type seulement). Variantes,
//! chacune avec ou sans « UID » en tête :
//! - historique des transactions (recommandé : il porte les frais) : Pairs, Time, Side, Filled Price,
//!   Executed Amount, Total, Fee, Role. Frais soit nombre seul (monnaie de cotation, ancien format, Side BUY / SELL),
//!   soit suffixé par l'actif (« 0.01USDT », format actuel, Side Buy / Sell) ;
//! - historique des ordres : Pairs, Time, Type, Direction, Average Filled Price, Order Price, Filled Quantity,
//!   Order Quantity, Order Amount, Status (sans frais) ;
//! - dépôts : Status (Credited Successfully), Time, Crypto, Network, Deposit Amount, TxID, Progress ;
//! - retraits : Status (Withdrawal Successful), Time, Crypto, Network, Request Amount, Withdrawal Address, [memo,]
//!   TxID, Trading Fee, Settlement Amount, Withdrawal Descriptions ;
//! - contrats à terme : … Futures Trading Pair, Direction, Leverage … → « à vérifier ».
//! Paires « BTC_USDT » ou « BTCUSDT » (monnaies de cotation : BRL, BTC, ETH, EUR, USD1, USDC, USDE, USDF, USDT).

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "MEXC";
const COTATIONS: &[&str] = &["BRL", "BTC", "ETH", "EUR", "USD1", "USDC", "USDE", "USDF", "USDT"];

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["Pairs", "Time", "Side", "Filled Price", "Executed Amount", "Total", "Fee", "Role"]) {
        return Some(transactions(&t));
    }
    if let Some(t) = Table::trouver(texte, &["Pairs", "Time", "Type", "Direction", "Average Filled Price", "Filled Quantity", "Order Quantity", "Order Amount", "Status"]) {
        return Some(ordres(&t));
    }
    if let Some(t) = Table::trouver(texte, &["Status", "Time", "Crypto", "Network", "Deposit Amount", "TxID"]) {
        return Some(mouvements(&t, false));
    }
    if let Some(t) = Table::trouver(texte, &["Status", "Time", "Crypto", "Network", "Request Amount", "Withdrawal Address", "Trading Fee", "Settlement Amount"]) {
        return Some(mouvements(&t, true));
    }
    if let Some(t) = Table::trouver(texte, &["Futures Trading Pair", "Direction", "Leverage", "Closing PNL"]) {
        let mut imp = Import { plateforme: P.into(), format: "contrats à terme".into(), ..Default::default() };
        for (n, _, brut) in &t.lignes { verifier(&mut imp, *n, "", "contrats à terme : hors du champ de Taiseux (régime différent)", brut); }
        return Some(imp);
    }
    None
}

fn transactions(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "historique des transactions".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let Some((d, h)) = horodatage(champ(c, t.col("Time"))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let Some((base, cote)) = paire(champ(c, t.col("Pairs")), COTATIONS) else { verifier(&mut imp, *n, &d, "paire illisible", brut); continue };
        let (Some(q), Some(total)) = (nombre(champ(c, t.col("Executed Amount"))), nombre(champ(c, t.col("Total")))) else { verifier(&mut imp, *n, &d, "quantité ou total illisible", brut); continue };
        let achat_ = match champ(c, t.col("Side")).trim().to_lowercase().as_str() { "buy" => true, "sell" => false, s => { verifier(&mut imp, *n, &d, format!("sens « {s} » inconnu"), brut); continue } };
        // Frais : « 0.01USDT » (actif en suffixe) ou nombre seul (monnaie de cotation).
        let f = champ(c, t.col("Fee")).trim();
        let chiffres: String = f.chars().take_while(|ch| ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-')).collect();
        let suffixe = &f[chiffres.len()..];
        let frais = match (nombre(&chiffres), suffixe.is_empty()) {
            (None, _) => None,
            (Some(q), true) => Some((cote.clone(), q)),
            (Some(q), false) => Some((actif(&suffixe), q)),
        };
        ordre(&mut imp, *n, brut, &d, &h, P, &format!("MEXC : {} {}", champ(c, t.col("Side")), champ(c, t.col("Pairs"))), &base, q, &cote, total, achat_, frais);
    }
    imp
}

fn ordres(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "historique des ordres (sans frais)".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let Some((d, h)) = horodatage(champ(c, t.col("Time"))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let Some((base, cote)) = paire(champ(c, t.col("Pairs")), COTATIONS) else { verifier(&mut imp, *n, &d, "paire illisible", brut); continue };
        let (fq, oq, total) = (nombre(champ(c, t.col("Filled Quantity"))), nombre(champ(c, t.col("Order Quantity"))), nombre(champ(c, t.col("Order Amount"))));
        let Some(fq) = fq else { verifier(&mut imp, *n, &d, "quantité exécutée illisible", brut); continue };
        if fq == 0.0 { imp.ignorees += 1; continue; } // ordre annulé sans exécution
        // « Order Amount » n'est le montant exécuté que si l'ordre l'a été en entier.
        let (Some(oq), Some(total)) = (oq, total) else { verifier(&mut imp, *n, &d, "montant de l'ordre illisible", brut); continue };
        if (oq - fq).abs() > 1e-12 * oq.abs().max(1.0) { verifier(&mut imp, *n, &d, "ordre exécuté en partie : montant exécuté inconnu, utilisez l'historique des transactions", brut); continue; }
        let achat_ = match champ(c, t.col("Direction")).trim().to_lowercase().as_str() { "buy" => true, "sell" => false, s => { verifier(&mut imp, *n, &d, format!("sens « {s} » inconnu"), brut); continue } };
        ordre(&mut imp, *n, brut, &d, &h, P, &format!("MEXC : ordre {}", champ(c, t.col("Pairs"))), &base, fq, &cote, total, achat_, None);
    }
    imp
}

fn mouvements(t: &Table, retrait: bool) -> Import {
    let mut imp = Import { plateforme: P.into(), format: (if retrait { "retraits" } else { "dépôts" }).into(), ..Default::default() };
    let reussi = if retrait { "Withdrawal Successful" } else { "Credited Successfully" };
    for (n, c, brut) in &t.lignes {
        let Some((d, h)) = horodatage(champ(c, t.col("Time"))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        if champ(c, t.col("Status")).trim() != reussi { imp.ignorees += 1; continue; }
        let a = actif(champ(c, t.col("Crypto")));
        let q = nombre(champ(c, t.col(if retrait { "Request Amount" } else { "Deposit Amount" }))).unwrap_or(f64::NAN);
        let frais = if retrait { nombre(champ(c, t.col("Trading Fee"))).unwrap_or(0.0) } else { 0.0 };
        mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, frais, format!("MEXC : {}", if retrait { "retrait" } else { "dépôt" }));
    }
    imp
}
