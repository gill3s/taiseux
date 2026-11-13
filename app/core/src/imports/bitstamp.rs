//! Bitstamp : historique des transactions (« Transactions » → Export, CSV). Deux variantes d'en-tête connues :
//! - actuelle : ID, Account, Type, Subtype, Datetime, Amount, Amount currency, Value, Value currency, Rate,
//!   Rate currency, Fee, Fee currency, Order ID ;
//! - ancienne : Type, Datetime, Account, Amount, Value, Rate, Fee, Sub Type ; les montants y portent leur
//!   monnaie (« 0.50000000 BTC », « 15000.00 EUR ») et la date est écrite à l'anglaise (« Jan. 05, 2021, 10:15 AM »).
//! Types : Deposit, Withdrawal, Ripple deposit, Ripple payment, Market (sous-type Buy ou Sell), Staking reward.
//! Source : noms de colonnes et valeurs de type relevés dans BittyTax (`conv/parsers/bitstamp.py`) ; aucun code repris.
//!
//! Un ordre « Market » : Amount = quantité de crypto, Value = contre-valeur (Amount × Rate), Fee = frais en sus,
//! dans la monnaie de cotation. Achat contre euros : prix payé = Value + Fee ; vente : prix = Value, frais = Fee.
//! Paires cotées dans une autre monnaie que l'euro → « à vérifier » ; crypto contre crypto → échange.

use std::collections::BTreeMap;

use super::commun::*;
use super::{AVerifier, Import};

const P: &str = "Bitstamp";

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["ID", "Account", "Type", "Subtype", "Datetime", "Amount", "Amount currency", "Value", "Value currency"]) {
        return Some(traduire(t, true));
    }
    let t = Table::trouver(texte, &["Type", "Datetime", "Account", "Amount", "Value", "Rate", "Fee", "Sub Type"])?;
    Some(traduire(t, false))
}

/// Montant et monnaie : colonnes séparées (format actuel) ou « 0.5 BTC » (ancien format).
fn montant(c: &[String], i: Option<usize>, i_monnaie: Option<usize>, actuel: bool) -> (Option<f64>, String) {
    let s = champ(c, i).trim();
    if actuel { return (nombre(s), norm(champ(c, i_monnaie))); }
    match s.rsplit_once(' ') {
        Some((q, m)) => (nombre(q), norm(m)),
        None => (nombre(s), String::new()),
    }
}
fn norm(a: &str) -> String {
    let a = actif(a);
    if a == "ETH2R" || a == "ETH2" { "ETH".into() } else { a }
}

fn traduire(t: Table, actuel: bool) -> Import {
    let (i_type, i_sub, i_date) = (t.col("Type"), t.col("Subtype").or_else(|| t.col("Sub Type")), t.col("Datetime"));
    let (i_m, i_m_cur, i_v, i_v_cur, i_f, i_f_cur) = (t.col("Amount"), t.col("Amount currency"), t.col("Value"), t.col("Value currency"), t.col("Fee"), t.col("Fee currency"));
    let format = if actuel { "historique des transactions" } else { "historique des transactions (ancien format)" };
    let mut imp = Import { plateforme: P.into(), format: format.into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let brut_date = champ(c, i_date);
        let Some((d, h)) = horodatage(brut_date).or_else(|| date_anglaise(brut_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim();
        let sub = champ(c, i_sub).trim();
        let (q, a) = montant(c, i_m, i_m_cur, actuel);
        let Some(q) = q.map(f64::abs) else { verifier(&mut imp, &d, "quantité illisible"); continue };
        let note = format!("Bitstamp : {typ}{}", if sub.is_empty() { String::new() } else { format!(" / {sub}") });
        match typ {
            "Deposit" | "Ripple deposit" | "Withdrawal" | "Ripple payment" if est_fiat(&a) => imp.ignorees += 1,
            "Deposit" | "Ripple deposit" | "Withdrawal" | "Ripple payment" => imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note)),
            "Staking reward" if q > 0.0 => imp.operations.push(revenu(&d, &h, P, &a, q, note)),
            "Market" => {
                let (v, v_cur) = montant(c, i_v, i_v_cur, actuel);
                let (f, f_cur) = montant(c, i_f, i_f_cur, actuel);
                let Some(v) = v.map(f64::abs) else { verifier(&mut imp, &d, "contre-valeur illisible"); continue };
                let achete = match sub { "Buy" => true, "Sell" => false, _ => { verifier(&mut imp, &d, &format!("ordre de sous-type « {sub} » non pris en charge")); continue } };
                let (mut bruts, mut frais) = (BTreeMap::new(), BTreeMap::new());
                bruts.insert(a.clone(), if achete { q } else { -q });
                *bruts.entry(v_cur.clone()).or_insert(0.0) += if achete { -v } else { v };
                if let Some(f) = f.map(f64::abs).filter(|f| *f > 0.0) {
                    // Ancien format : frais sans monnaie écrite = monnaie de cotation.
                    frais.insert(if f_cur.is_empty() { v_cur.clone() } else { f_cur }, f);
                }
                match operations_echange(classer(&bruts, &frais), &d, &h, P, &note) {
                    Ok(ops) => imp.operations.extend(ops),
                    Err(motif) => verifier(&mut imp, &d, &motif),
                }
            }
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    imp
}

/// Date écrite à l'anglaise : « Jan. 05, 2021, 10:15 AM », « May 05, 2021, 10:15:30 PM », « Sept. 1, 2021, 9:05 AM ».
/// Rendue (AAAA-MM-JJ, HH:MM:SS). Utilisée aussi par le lecteur Revolut.
pub fn date_anglaise(s: &str) -> Option<(String, String)> {
    const MOIS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let s = s.trim();
    let mut parts = s.splitn(3, ',');
    let (md, an, reste) = (parts.next()?.trim(), parts.next()?.trim(), parts.next().unwrap_or("").trim());
    let (m, j) = md.split_once(' ')?;
    let m = m.trim_end_matches('.').to_lowercase();
    let mois = MOIS.iter().position(|x| m.starts_with(x))? + 1;
    let j: u32 = j.trim().parse().ok()?;
    let an: u32 = an.parse().ok()?;
    if !(1..=31).contains(&j) || !(1970..=2200).contains(&an) { return None; }
    let mut heure = String::new();
    if !reste.is_empty() {
        let (hms, ampm) = match reste.rsplit_once(' ') { Some((x, y)) => (x.trim(), y.trim().to_uppercase()), None => (reste, String::new()) };
        let p: Vec<u32> = hms.split(':').map(|x| x.parse().ok()).collect::<Option<Vec<_>>>()?;
        if p.len() < 2 || p.len() > 3 { return None; }
        let mut hh = p[0];
        match ampm.as_str() { "PM" if hh < 12 => hh += 12, "AM" if hh == 12 => hh = 0, "AM" | "PM" | "" => {}, _ => return None }
        heure = format!("{:02}:{:02}:{:02}", hh, p[1], p.get(2).copied().unwrap_or(0));
    }
    Some((format!("{an:04}-{mois:02}-{j:02}"), heure))
}
