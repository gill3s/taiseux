//! Uphold : historique des transactions (CSV). Source : noms de colonnes et valeurs relevés par BittyTax
//! (`conv/parsers/uphold.py`, noms de colonnes et valeurs de type seulement). Deux variantes :
//! - actuelle : Date, Destination, Destination Amount, Destination Currency, Fee Amount, Fee Currency, Id, Origin,
//!   Origin Amount, Origin Currency, Status (completed), Type (in, out, transfer). Date en toutes lettres
//!   (« Mon Mar 15 2021 13:16:08 GMT+0000 ») ou ISO ;
//! - ancienne : date, id, type (deposit / in, withdrawal / out, transfer), value_in_<XXX>, commission_in_<XXX>, pair,
//!   rate, origin_currency, origin_amount, origin_commission, destination_currency, destination_amount,
//!   destination_commission.
//! Une opération dont l'origine et la destination sont dans des actifs différents est une conversion (achat, vente ou
//! échange) ; dans le même actif, un dépôt ou un retrait. Le montant d'origine est pris comme total donné, frais
//! compris (les frais d'Uphold sont prélevés sur l'origine).

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "Uphold";

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["Date", "Destination Amount", "Destination Currency", "Origin Amount", "Origin Currency", "Status", "Type"]) {
        let k = Cols { date: t.col("Date"), typ: t.col("Type"), statut: t.col("Status"), oa: t.col("Origin Amount"), oc: t.col("Origin Currency"),
                       da: t.col("Destination Amount"), dc: t.col("Destination Currency") };
        return Some(lire_table(&t, k, "historique des transactions"));
    }
    if let Some(t) = Table::trouver(texte, &["date", "id", "type", "pair", "rate", "origin_currency", "origin_amount", "destination_currency", "destination_amount"]) {
        let k = Cols { date: t.col("date"), typ: t.col("type"), statut: None, oa: t.col("origin_amount"), oc: t.col("origin_currency"),
                       da: t.col("destination_amount"), dc: t.col("destination_currency") };
        return Some(lire_table(&t, k, "historique des transactions (ancien format)"));
    }
    None
}

struct Cols { date: Option<usize>, typ: Option<usize>, statut: Option<usize>, oa: Option<usize>, oc: Option<usize>, da: Option<usize>, dc: Option<usize> }

/// « Mon Mar 15 2021 13:16:08 GMT+0000 (Coordinated Universal Time) » → date et heure UTC ; sinon ISO.
fn date_uphold(s: &str) -> Option<(String, String)> {
    if let Some(r) = horodatage(s) { return Some(r); }
    let m: Vec<&str> = s.split_whitespace().collect();
    if m.len() < 5 { return None; }
    let mo = mois_anglais(m[1])?;
    let (j, a) = (m[2].parse::<i64>().ok()?, m[3].parse::<i64>().ok()?);
    if !date_valide(a, mo, j) || m[4].len() != 8 { return None; }
    let dec = m.get(5).and_then(|z| decalage(z)).unwrap_or(0);
    Some(vers_utc(&format!("{a:04}-{mo:02}-{j:02}"), m[4], dec))
}

fn lire_table(t: &Table, k: Cols, format: &str) -> Import {
    let mut imp = Import { plateforme: P.into(), format: format.into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let Some((d, h)) = date_uphold(champ(c, k.date)) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        if k.statut.is_some() && champ(c, k.statut).trim() != "completed" { imp.ignorees += 1; continue; }
        let typ = champ(c, k.typ).trim().to_lowercase();
        let (oc, dc) = (actif(champ(c, k.oc)), actif(champ(c, k.dc)));
        let (Some(oa), Some(da)) = (nombre(champ(c, k.oa)).map(f64::abs), nombre(champ(c, k.da)).map(f64::abs)) else { verifier(&mut imp, *n, &d, "montants illisibles", brut); continue };
        let note = format!("Uphold : {typ}");
        match typ.as_str() {
            "in" | "deposit" | "out" | "withdrawal" | "transfer" if oc == dc => {
                if typ == "transfer" { imp.ignorees += 1; continue; } // entre deux comptes Uphold de l'utilisateur
                let entree = matches!(typ.as_str(), "in" | "deposit");
                // Retrait : la destination est ce qui part ; l'écart avec l'origine, les frais.
                let (q, frais) = if entree { (da, 0.0) } else { (da, (oa - da).max(0.0)) };
                mouvement(&mut imp, *n, brut, &d, &h, P, &oc, q, frais, note);
            }
            "in" | "deposit" | "out" | "withdrawal" | "transfer" =>
                conversion(&mut imp, *n, brut, &d, &h, P, &note, &oc, oa, &dc, da, None),
            autre => verifier(&mut imp, *n, &d, format!("type « {autre} » non pris en charge"), brut),
        }
    }
    imp
}
