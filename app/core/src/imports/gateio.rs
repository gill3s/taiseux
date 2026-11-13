//! Gate.io (Gate) : registre du compte (« Account Book » / historique des fonds, CSV). Source : noms de colonnes et
//! valeurs relevés par BittyTax (`conv/parsers/gateio.py`, noms de colonnes et valeurs de type seulement). Une seule
//! variante connue : no, time, action_desc, action_data, type (l'actif), change_amount, amount (solde), total.
//! Un ordre s'étale sur plusieurs lignes de même `action_data` (Order Placed, Order Filled, Order Fullfilled,
//! Trading Fees, Trade Fee) : elles sont regroupées ; de même pour la conversion des petits soldes (Dust Swap).
//! Deposits / Withdrawals → transferts ; HODL Interest, Interest Income, Referral Superior Rebate, Airdrop → revenus.

use std::collections::BTreeMap;

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "Gate.io";
const ORDRE: &[&str] = &["Order Placed", "Order Filled", "Order Fullfilled", "Trading Fees", "Trade Fee",
    "Dust Swap-Small Balances Deducted", "Dust Swap-GT Added"];

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["no", "time", "action_desc", "action_data", "type", "change_amount"])?;
    let mut imp = Import { plateforme: P.into(), format: "registre du compte".into(), ..Default::default() };
    let (i_t, i_desc, i_data, i_actif, i_q) = (t.col("time"), t.col("action_desc"), t.col("action_data"), t.col("type"), t.col("change_amount"));
    let mut groupes: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut ordre_groupes: Vec<String> = Vec::new();
    for (k, (n, c, brut)) in t.lignes.iter().enumerate() {
        let Some((d, h)) = horodatage(champ(c, i_t)) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let desc = champ(c, i_desc).trim();
        let a = actif(champ(c, i_actif));
        let Some(q) = nombre(champ(c, i_q)) else { verifier(&mut imp, *n, &d, "montant illisible", brut); continue };
        let note = format!("Gate.io : {desc}");
        if ORDRE.contains(&desc) {
            let id = champ(c, i_data).to_string();
            if !groupes.contains_key(&id) { ordre_groupes.push(id.clone()); }
            groupes.entry(id).or_default().push(k);
            continue;
        }
        match desc {
            "Deposits" | "Deposit" | "Withdrawals" | "withdraw" => mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, 0.0, note),
            "Referral Superior Rebate" | "HODL Interest" | "Interest Income" | "Airdrop" | "Airdrop bonus" if q > 0.0 && !est_fiat(&a) =>
                imp.operations.push(revenu(&d, &h, P, &a, q, note)),
            autre => verifier(&mut imp, *n, &d, format!("opération « {autre} » non prise en charge"), brut),
        }
    }
    for id in ordre_groupes {
        let ks = &groupes[&id];
        let c0 = &t.lignes[ks[0]].1;
        let (d, h) = horodatage(champ(c0, i_t)).unwrap_or_default();
        let lignes: Vec<(String, f64, bool)> = ks.iter().map(|&k| {
            let c = &t.lignes[k].1;
            let frais = matches!(champ(c, i_desc).trim(), "Trading Fees" | "Trade Fee");
            (actif(champ(c, i_actif)), nombre(champ(c, i_q)).unwrap_or(0.0), frais)
        }).collect();
        let brut = ks.iter().map(|&k| t.lignes[k].2.clone()).collect::<Vec<_>>().join("\n");
        transaction(&mut imp, t.lignes[ks[0]].0, &brut, &d, &h, P, &format!("Gate.io : ordre {id}"), &lignes);
    }
    Some(imp)
}
