//! Binance, deux exports :
//! - relevé « historique des transactions » (Transaction History / statement) : [User_ID,] UTC_Time, Account,
//!   Operation, Coin, Change, Remark. Un échange s'étale sur plusieurs lignes à la même seconde (ou à une seconde
//!   près) : Buy / Sell / Transaction Buy / Spend / Revenue / Sold / Fee… ; elles sont regroupées.
//! - historique des ordres au comptant (Trade History) : Date(UTC), Market (ou Pair), Type (ou Side), Price, Amount,
//!   Total, Fee, Fee Coin.
//! Contrats à terme, marge et prêts : hors du champ de Taiseux (régime différent), donc « à vérifier ».

use std::collections::BTreeMap;

use super::commun::*;
use super::{AVerifier, Import};

const ECHANGE: &[&str] = &[
    "Buy", "Sell", "Fee", "Transaction Related", "Transaction Buy", "Transaction Spend", "Transaction Fee", "Transaction Sold",
    "Transaction Revenue", "Binance Convert", "Small assets exchange BNB", "Small Assets Exchange BNB", "BNB Fee Deduction",
    "Large OTC trading", "Buy Crypto With Card", "Stablecoins Auto-Conversion", "Liquid Swap Sell",
];
const INTERNES: &[&str] = &[
    "transfer_in", "transfer_out", "Savings purchase", "Savings Principal redemption", "Staking Purchase", "Staking Redemption",
    "Simple Earn Flexible Subscription", "Simple Earn Flexible Redemption", "Simple Earn Locked Subscription", "Simple Earn Locked Redemption",
    "Main and Funding Account Transfer", "Funding Account Transfer",
];

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["UTC_Time", "Account", "Operation", "Coin", "Change"]) { return Some(releve(t)); }
    if let Some(t) = Table::trouver(texte, &["Date(UTC)", "Price", "Amount", "Fee"]) {
        if (t.a("Market") || t.a("Pair")) && (t.a("Type") || t.a("Side")) { return Some(ordres(t)); }
    }
    None
}

fn secondes(h: &str) -> i64 {
    let p: Vec<i64> = h.split(':').filter_map(|x| x.parse().ok()).collect();
    if p.len() == 3 { p[0] * 3600 + p[1] * 60 + p[2] } else { -1 }
}

fn releve(t: Table) -> Import {
    let (i_time, i_compte, i_op, i_coin, i_change) = (t.col("UTC_Time"), t.col("Account"), t.col("Operation"), t.col("Coin"), t.col("Change"));
    let mut imp = Import { plateforme: "Binance".into(), format: "relevé des transactions".into(), ..Default::default() };
    // Lignes d'échange à regrouper : (index, date, heure).
    let mut a_regrouper: Vec<(usize, String, String)> = Vec::new();
    for (k, (n, c, brut)) in t.lignes.iter().enumerate() {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = horodatage(champ(c, i_time)) else { verifier(&mut imp, "", "date illisible"); continue };
        let op = champ(c, i_op).trim();
        let compte = champ(c, i_compte).trim();
        let a = actif(champ(c, i_coin));
        let Some(ch) = nombre(champ(c, i_change)) else { verifier(&mut imp, &d, "montant illisible"); continue };
        let note = format!("Binance : {op}");
        if INTERNES.contains(&op) || op.contains("Transfer Between") { imp.ignorees += 1; continue; }
        if compte.contains("Futures") || compte.contains("Margin") || compte.contains("Options") || op.contains("Margin") || op.contains("Futures") || op.contains("Realize profit") {
            verifier(&mut imp, &d, "contrats à terme, marge ou options : hors du champ de Taiseux (régime différent)");
            continue;
        }
        if ECHANGE.contains(&op) { a_regrouper.push((k, d, h)); continue; }
        match op {
            "Deposit" | "Withdraw" | "Fiat Deposit" | "Fiat Withdrawal" | "Fiat Withdraw" | "Send" | "P2P Trading" if est_fiat(&a) => imp.ignorees += 1,
            "Deposit" if ch > 0.0 => imp.operations.push(transfert(&d, &h, "Binance", &a, ch, 0.0, note)),
            "Withdraw" | "Send" if ch < 0.0 => imp.operations.push(transfert(&d, &h, "Binance", &a, -ch, 0.0, note)),
            o if ch > 0.0 && ["Reward", "Interest", "Airdrop", "Distribution", "Commission", "Megadrop", "Launchpool", "Cashback", "Referral", "Bonus"].iter().any(|m| o.contains(m)) =>
                imp.operations.push(revenu(&d, &h, "Binance", &a, ch, note)),
            autre => verifier(&mut imp, &d, &format!("opération « {autre} » non prise en charge")),
        }
    }
    // Regroupement : lignes d'échange à la même seconde (ou à une seconde près de la première du groupe).
    a_regrouper.sort_by(|x, y| x.1.cmp(&y.1).then(x.2.cmp(&y.2)).then(x.0.cmp(&y.0)));
    let mut i = 0;
    while i < a_regrouper.len() {
        let (_, d, h) = a_regrouper[i].clone();
        let s0 = secondes(&h);
        let mut j = i;
        while j < a_regrouper.len() && a_regrouper[j].1 == d && (secondes(&a_regrouper[j].2) - s0).abs() <= 1 { j += 1; }
        let (mut bruts, mut frais) = (BTreeMap::new(), BTreeMap::new());
        for (k, _, _) in &a_regrouper[i..j] {
            let c = &t.lignes[*k].1;
            let a = actif(champ(c, i_coin));
            let ch = nombre(champ(c, i_change)).unwrap_or(0.0);
            let op = champ(c, i_op);
            if op.contains("Fee") { *frais.entry(a).or_insert(0.0) += ch.abs(); } else { *bruts.entry(a).or_insert(0.0) += ch; }
        }
        let premiere = t.lignes[a_regrouper[i].0].0;
        match operations_echange(classer(&bruts, &frais), &d, &h, "Binance", "Binance : échange") {
            Ok(ops) => imp.operations.extend(ops),
            Err(motif) => imp.a_verifier.push(AVerifier {
                ligne: premiere, date: d.clone(), motif,
                contenu: a_regrouper[i..j].iter().map(|(k, _, _)| t.lignes[*k].2.clone()).collect::<Vec<_>>().join("\n"),
            }),
        }
        i = j;
    }
    imp
}

/// Monnaies de cotation, de la plus longue à la plus courte, pour séparer une paire « BTCEUR » en BTC et EUR.
const COTATIONS: &[&str] = &["FDUSD", "USDT", "USDC", "BUSD", "TUSD", "EUR", "USD", "GBP", "TRY", "BRL", "BTC", "ETH", "BNB", "DAI"];

pub fn separer_paire(p: &str) -> Option<(String, String)> {
    let p = p.trim().to_uppercase().replace(['/', '-', '_'], "");
    COTATIONS.iter().find(|q| p.len() > q.len() && p.ends_with(*q)).map(|q| (p[..p.len() - q.len()].to_string(), q.to_string()))
}

fn ordres(t: Table) -> Import {
    let (i_date, i_paire, i_sens) = (t.col("Date(UTC)"), t.col("Market").or_else(|| t.col("Pair")), t.col("Type").or_else(|| t.col("Side")));
    let (i_qte, i_total, i_frais, i_coin_frais) = (t.col("Executed").or_else(|| t.col("Amount")), t.col("Total").or_else(|| t.col("Amount")), t.col("Fee"), t.col("Fee Coin"));
    // Variante « Executed / Amount » : les montants portent l'actif en suffixe (« 0.5BTC », « 30000EUR »).
    let suffixe = t.a("Executed");
    let mut imp = Import { plateforme: "Binance".into(), format: "historique des ordres au comptant".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = horodatage(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let Some((base, cote)) = separer_paire(champ(c, i_paire)) else { verifier(&mut imp, &d, "paire illisible"); continue };
        let qte = nombre(champ(c, i_qte)).map(f64::abs);
        let total = nombre(champ(c, if suffixe { t.col("Amount") } else { i_total })).map(f64::abs);
        let frais = nombre(champ(c, i_frais)).unwrap_or(0.0).abs();
        let coin_frais = if suffixe { actif(champ(c, i_frais).trim_start_matches(|ch: char| ch.is_ascii_digit() || ch == '.' || ch == ',')) } else { actif(champ(c, i_coin_frais)) };
        let (Some(qte), Some(total)) = (qte, total) else { verifier(&mut imp, &d, "quantité ou total illisible"); continue };
        let achat_ = champ(c, i_sens).trim().eq_ignore_ascii_case("BUY");
        let (mut bruts, mut f) = (BTreeMap::new(), BTreeMap::new());
        bruts.insert(base.clone(), if achat_ { qte } else { -qte });
        bruts.insert(cote.clone(), if achat_ { -total } else { total });
        if frais > 0.0 && !coin_frais.is_empty() { f.insert(coin_frais, frais); }
        match operations_echange(classer(&bruts, &f), &d, &h, "Binance", "Binance : ordre au comptant") {
            Ok(ops) => imp.operations.extend(ops),
            Err(motif) => verifier(&mut imp, &d, &motif),
        }
    }
    imp
}
