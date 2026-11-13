//! Kraken : registre « ledgers » (CSV). Colonnes : txid, refid, time, type, subtype, aclass, [subclass,] asset,
//! [wallet,] amount, fee, balance[, amountusd]. Un échange s'étale sur plusieurs lignes de même `refid` (types trade,
//! spend, receive, settled) : elles sont regroupées. Les codes d'actifs de Kraken sont normalisés (XXBT → BTC,
//! ZEUR → EUR, suffixes de staking « .S », « .M », « .F »… retirés).

use std::collections::BTreeMap;

use super::commun::*;
use super::{AVerifier, Import};

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["txid", "refid", "time", "type", "asset", "amount", "fee"])?;
    let (i_tx, i_ref, i_time, i_type, i_sub, i_actif, i_amount, i_fee) =
        (t.col("txid"), t.col("refid"), t.col("time"), t.col("type"), t.col("subtype"), t.col("asset"), t.col("amount"), t.col("fee"));
    let mut imp = Import { plateforme: "Kraken".into(), format: "registre (ledgers)".into(), ..Default::default() };
    // refid → lignes d'un même échange, dans l'ordre du fichier.
    let mut groupes: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut ordre_groupes: Vec<String> = Vec::new();

    for (k, (n, c, brut)) in t.lignes.iter().enumerate() {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        if champ(c, i_tx).is_empty() { imp.ignorees += 1; continue; } // opération échouée
        let Some((d, h)) = horodatage(champ(c, i_time)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim().to_lowercase();
        let sub = champ(c, i_sub).trim().to_lowercase();
        let a = actif_kraken(champ(c, i_actif));
        let (amount, fee) = (nombre(champ(c, i_amount)).unwrap_or(f64::NAN), nombre(champ(c, i_fee)).unwrap_or(0.0).abs());
        if !amount.is_finite() { verifier(&mut imp, &d, "montant illisible"); continue; }
        let note = format!("Kraken : {typ}{}", if sub.is_empty() { String::new() } else { format!(" / {sub}") });
        match typ.as_str() {
            "trade" | "spend" | "receive" | "settled" => {
                let r = champ(c, i_ref).to_string();
                if !groupes.contains_key(&r) { ordre_groupes.push(r.clone()); }
                groupes.entry(r).or_default().push(k);
            }
            "deposit" | "withdrawal" if est_fiat(&a) => imp.ignorees += 1,
            "deposit" if amount > 0.0 => imp.operations.push(transfert(&d, &h, "Kraken", &a, amount, fee, note)),
            "withdrawal" if amount < 0.0 => imp.operations.push(transfert(&d, &h, "Kraken", &a, -amount, fee, note)),
            "staking" | "dividend" | "invite bonus" if amount > 0.0 => imp.operations.push(revenu(&d, &h, "Kraken", &a, amount - fee, note)),
            "earn" if matches!(sub.as_str(), "reward" | "airdrop" | "") && amount > 0.0 => imp.operations.push(revenu(&d, &h, "Kraken", &a, amount - fee, note)),
            "earn" if matches!(sub.as_str(), "migration" | "autoallocate" | "allocation" | "deallocation" | "autoallocation" | "") => imp.ignorees += 1,
            "transfer" if matches!(sub.as_str(), "spottostaking" | "stakingtospot" | "spotfromstaking" | "stakingfromspot" | "spottofutures" | "spotfromfutures") => imp.ignorees += 1,
            "transfer" if sub == "airdrop" && amount > 0.0 => imp.operations.push(revenu(&d, &h, "Kraken", &a, amount, note)),
            "margin" | "rollover" => verifier(&mut imp, &d, "opération sur marge : hors du champ de Taiseux (régime différent)"),
            _ => verifier(&mut imp, &d, &format!("type « {typ}{} » non pris en charge", if sub.is_empty() { String::new() } else { format!(" / {sub}") })),
        }
    }

    for r in ordre_groupes {
        let ks = &groupes[&r];
        let (n0, c0, _) = &t.lignes[ks[0]];
        let (d, h) = horodatage(champ(c0, i_time)).unwrap_or_default();
        let (mut bruts, mut frais) = (BTreeMap::new(), BTreeMap::new());
        for &k in ks {
            let c = &t.lignes[k].1;
            let a = actif_kraken(champ(c, i_actif));
            *bruts.entry(a.clone()).or_insert(0.0) += nombre(champ(c, i_amount)).unwrap_or(0.0);
            let f = nombre(champ(c, i_fee)).unwrap_or(0.0).abs();
            if f > 0.0 { *frais.entry(a).or_insert(0.0) += f; }
        }
        match operations_echange(classer(&bruts, &frais), &d, &h, "Kraken", &format!("Kraken : échange {r}")) {
            Ok(ops) => imp.operations.extend(ops),
            Err(motif) => imp.a_verifier.push(AVerifier {
                ligne: *n0, date: d.clone(), motif,
                contenu: ks.iter().map(|&k| t.lignes[k].2.clone()).collect::<Vec<_>>().join("\n"),
            }),
        }
    }
    Some(imp)
}

/// Code Kraken → code usuel : XXBT/XBT → BTC, XETH → ETH, ZEUR → EUR, XXDG → DOGE ; suffixes de staking retirés.
pub fn actif_kraken(s: &str) -> String {
    let mut a = actif(s);
    if let Some(p) = a.find('.') {
        a.truncate(p);
        // « ETH2.S », « DOT28.S » : les chiffres collés avant le suffixe désignent la variante bloquée.
        let sans: String = a.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
        if !sans.is_empty() { a = sans; }
    }
    let a = match a.as_str() {
        "XXBT" | "XBT" => "BTC".to_string(),
        "XXDG" | "XDG" => "DOGE".to_string(),
        "ETH2" => "ETH".to_string(),
        // Anciens codes à quatre lettres de Kraken (liste fermée : ZETA, par exemple, est un vrai actif).
        "XETC" | "XETH" | "XLTC" | "XMLN" | "XREP" | "XXLM" | "XXMR" | "XXRP" | "XZEC" | "ZEUR" | "ZUSD" | "ZGBP" | "ZCAD" | "ZJPY" | "ZAUD" | "ZCHF" => a[1..].to_string(),
        s => s.to_string(),
    };
    if a == "XBT" { "BTC".into() } else { a }
}
