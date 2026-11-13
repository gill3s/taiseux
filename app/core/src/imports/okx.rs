//! OKX : historique des transactions du compte de trading, et compte de financement (dépôts, retraits, Earn).
//! Sources : aide OKX « How do I download my trading history / statements » et noms de colonnes relevés par BittyTax
//! (`conv/parsers/okx.py`, noms de colonnes et valeurs de type seulement). Variantes :
//! - historique de trading (v2) : id, Order id, Time, Trade Type, Instrument, Type (Action), Amount, Unit, PL, Fee,
//!   Position Change, Position Balance, Balance Change, Balance, Unit. Un ordre au comptant donne deux lignes de même
//!   `id` (ou de même `Order id`) : Buy (actif reçu, frais dans cet actif) et Sell (actif donné). Trade Type « Spot »
//!   et « Transfer » (interne) ; tout autre (Margin, Futures, Perpetual, Options…) → « à vérifier » ;
//! - ancien historique (v1, heure de Chine, UTC+8) : time, type, size, balance, fee, currency, une ligne buy puis une
//!   ligne sell par ordre ;
//! - compte de financement : id, [colonne vide,] Time, Type, Amount, Before Balance, After Balance, Fee, Symbol.
//!   Types : Deposit, Withdrawal, Staking Yield, Fee rebate (revenus), Stake, Redeem staking, Savings subscription,
//!   Savings redemption, From / To unified trading account (internes).
//! L'heure est reprise telle qu'écrite dans le fichier (OKX laisse choisir le fuseau à l'export : à vérifier).

use std::collections::BTreeMap;

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "OKX";

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["id", "Order id", "Time", "Trade Type", "Instrument", "Type", "Amount", "Unit", "Fee", "Balance Change"]) {
        return Some(trading(&t));
    }
    if let Some(t) = Table::trouver(texte, &["id", "Time", "Type", "Amount", "Before Balance", "After Balance", "Fee", "Symbol"]) {
        return Some(financement(&t));
    }
    if let Some(t) = Table::trouver(texte, &["time", "type", "size", "balance", "fee", "currency"]) {
        return Some(ancien(&t));
    }
    None
}

fn trading(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "historique de trading".into(), ..Default::default() };
    let (i_id, i_ordre, i_t, i_tt, i_type, i_q, i_u, i_f) =
        (t.col("id"), t.col("Order id"), t.col("Time"), t.col("Trade Type"), t.col("Type"), t.col("Amount"), t.col("Unit"), t.col("Fee"));
    // Ordres au comptant regroupés par `id` ; si l'`id` ne donne pas une paire Buy + Sell, par `Order id`.
    let mut par_id: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut par_ordre: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut spot: Vec<usize> = Vec::new();
    for (k, (n, c, brut)) in t.lignes.iter().enumerate() {
        let Some((d, _)) = horodatage(champ(c, i_t)) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        match champ(c, i_tt).trim() {
            "Spot" if matches!(champ(c, i_type).trim(), "Buy" | "Sell") => {
                spot.push(k);
                par_id.entry(champ(c, i_id).to_string()).or_default().push(k);
                par_ordre.entry(champ(c, i_ordre).to_string()).or_default().push(k);
            }
            "Spot" => verifier(&mut imp, *n, &d, format!("type « {} » non pris en charge", champ(c, i_type)), brut),
            "Transfer" => imp.ignorees += 1,
            autre => verifier(&mut imp, *n, &d, format!("« {autre} » : marge, contrats à terme ou options, hors du champ de Taiseux (régime différent)"), brut),
        }
    }
    let paire_ok = |ks: &[usize]| {
        let n = |s: &str| ks.iter().filter(|&&k| champ(&t.lignes[k].1, i_type).trim() == s).count();
        n("Buy") == 1 && n("Sell") == 1
    };
    let mut fait = vec![false; t.lignes.len()];
    for k in spot {
        if fait[k] { continue; }
        let c = &t.lignes[k].1;
        let ks = [par_id.get(champ(c, i_id)), par_ordre.get(champ(c, i_ordre))].into_iter().flatten()
            .find(|ks| paire_ok(ks)).cloned();
        let (n, brut) = (t.lignes[k].0, t.lignes[k].2.clone());
        let (d, h) = horodatage(champ(c, i_t)).unwrap_or_default();
        let Some(ks) = ks else { fait[k] = true; verifier(&mut imp, n, &d, "ordre au comptant : ligne Buy ou Sell correspondante introuvable", &brut); continue };
        let mut lignes = Vec::new();
        for &j in &ks {
            fait[j] = true;
            let cj = &t.lignes[j].1;
            let a = actif(champ(cj, i_u));
            let q = nombre(champ(cj, i_q)).unwrap_or(f64::NAN).abs();
            lignes.push((a.clone(), if champ(cj, i_type).trim() == "Buy" { q } else { -q }, false));
            if let Some(f) = nombre(champ(cj, i_f)).filter(|f| *f != 0.0) { lignes.push((a, f, true)); }
        }
        if lignes.iter().any(|(_, q, _)| !q.is_finite()) { verifier(&mut imp, n, &d, "quantité illisible", &brut); continue; }
        let brut = ks.iter().map(|&j| t.lignes[j].2.clone()).collect::<Vec<_>>().join("\n");
        transaction(&mut imp, n, &brut, &d, &h, P, &format!("OKX : ordre au comptant {}", champ(c, t.col("Instrument"))), &lignes);
    }
    imp
}

fn financement(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "compte de financement".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let Some((d, h)) = horodatage(champ(c, t.col("Time"))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let a = actif(champ(c, t.col("Symbol")));
        let q = nombre(champ(c, t.col("Amount"))).unwrap_or(f64::NAN);
        let frais = nombre(champ(c, t.col("Fee"))).unwrap_or(0.0);
        let typ = champ(c, t.col("Type")).trim();
        match typ {
            "Deposit" => mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, 0.0, "OKX : dépôt".into()),
            "Withdrawal" => mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, frais, "OKX : retrait".into()),
            "Staking Yield" | "Fee rebate" if q > 0.0 && !est_fiat(&a) => imp.operations.push(revenu(&d, &h, P, &a, q, format!("OKX : {typ}"))),
            "Stake" | "Redeem staking" | "Savings subscription" | "Savings redemption" | "From unified trading account" | "To unified trading account" =>
                imp.ignorees += 1,
            autre => verifier(&mut imp, *n, &d, format!("type « {autre} » non pris en charge"), brut),
        }
    }
    imp
}

fn ancien(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "historique de trading (ancien format)".into(), ..Default::default() };
    let l = &t.lignes;
    let mut k = 0;
    while k < l.len() {
        let (n, c, brut) = &l[k];
        let Some((d, h)) = horodatage_utc(champ(c, t.col("time")), 480) else { verifier(&mut imp, *n, "", "date illisible", brut); k += 1; continue };
        let typ = |c: &[String]| champ(c, t.col("type")).trim().to_lowercase();
        if typ(c) != "buy" || k + 1 >= l.len() || typ(&l[k + 1].1) != "sell" {
            verifier(&mut imp, *n, &d, "ligne isolée : une ligne buy suivie d'une ligne sell était attendue", brut);
            k += 1; continue;
        }
        let c2 = &l[k + 1].1;
        let (a, b) = (actif(champ(c, t.col("currency"))), actif(champ(c2, t.col("currency"))));
        let qa = nombre(champ(c, t.col("size"))).unwrap_or(f64::NAN).abs();
        let qb = nombre(champ(c2, t.col("size"))).unwrap_or(f64::NAN).abs();
        let mut lignes = vec![(a.clone(), qa, false), (b, -qb, false)];
        if let Some(f) = nombre(champ(c, t.col("fee"))).filter(|f| *f != 0.0) { lignes.push((a, f, true)); }
        let brut = format!("{}\n{}", brut, l[k + 1].2);
        if !(qa.is_finite() && qb.is_finite()) { verifier(&mut imp, *n, &d, "quantité illisible", &brut); } else { transaction(&mut imp, *n, &brut, &d, &h, P, "OKX : ordre", &lignes); }
        k += 2;
    }
    imp
}
