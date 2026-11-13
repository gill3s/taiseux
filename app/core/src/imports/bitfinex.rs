//! Bitfinex : rapports « Trades » et « Movements » (dépôts et retraits), CSV. Sources : article « Reporting Tools –
//! Change Log 2.1 » (blog Bitfinex : choix du format de date à l'export) et noms de colonnes relevés par BittyTax
//! (`conv/parsers/bitfinex.py`, noms de colonnes et valeurs de type seulement). Variantes :
//! - échanges : #, PAIR, AMOUNT (> 0 achat, < 0 vente de l'actif de base), PRICE, FEE (négatif), [FEE PERC,]
//!   FEE CURRENCY, DATE, ORDER ID ;
//! - dépôts et retraits : #, DATE ou STARTED + UPDATED, CURRENCY, STATUS (COMPLETED), AMOUNT (> 0 dépôt),
//!   FEES, DESCRIPTION, TRANSACTION ID, [NOTE].
//! Non pris en charge : le registre (« Ledgers »), qui répète les échanges et les mouvements.
//!
//! **Dates** : l'utilisateur choisit le format à l'export (année-mois-jour, jour-mois-année ou mois-jour-année, année
//! sur deux ou quatre chiffres). Le format est déduit du fichier entier, en écartant ceux qui donnent une date
//! impossible (ou antérieure à 2012, ouverture de Bitfinex) ; s'il en reste plusieurs, les lignes partent « à
//! vérifier » plutôt que de deviner.
//! Codes propres à Bitfinex : UST → USDT, EUT → EURT (Tether) ; paires « BTC/EUR », « BTC:USD » ou « BTCEUR ».

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "Bitfinex";
const COTATIONS: &[&str] = &["EUR", "USD", "GBP", "JPY", "UST", "EUT", "BTC", "ETH", "CNHT", "XCHF", "MIM", "TRY"];

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["#", "PAIR", "AMOUNT", "PRICE", "FEE", "FEE CURRENCY", "DATE", "ORDER ID"]) {
        return Some(echanges(&t));
    }
    if let Some(t) = Table::trouver(texte, &["#", "CURRENCY", "STATUS", "AMOUNT", "FEES", "DESCRIPTION", "TRANSACTION ID"]) {
        if t.a("DATE") || t.a("UPDATED") { return Some(mouvements(&t)); }
    }
    None
}

fn code(a: &str) -> String {
    match actif(a).as_str() { "UST" => "USDT".into(), "EUT" => "EURT".into(), x => x.into() }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Ordre { AMJ, JMA, MJA }

/// (trois nombres de la date, heure) ; « 22-01-13 10:00:00 », « 13/01/2022 10:00:00.123 »…
fn morceaux(s: &str) -> Option<([i64; 3], [usize; 3], String)> {
    let s = s.trim();
    let (d, h) = s.split_once(' ').unwrap_or((s, ""));
    let p: Vec<&str> = d.split(['-', '/', '.']).collect();
    if p.len() != 3 { return None; }
    let n: Vec<i64> = p.iter().filter_map(|x| x.parse().ok()).collect();
    if n.len() != 3 { return None; }
    let h: String = h.chars().take(8).collect();
    let h = if h.len() == 8 && h.as_bytes()[2] == b':' { h } else { String::new() };
    Some(([n[0], n[1], n[2]], [p[0].len(), p[1].len(), p[2].len()], h))
}

fn appliquer(o: Ordre, m: &([i64; 3], [usize; 3], String)) -> Option<(String, String)> {
    let (n, l) = (m.0, m.1);
    let (a, la, mo, j) = match o { Ordre::AMJ => (n[0], l[0], n[1], n[2]), Ordre::JMA => (n[2], l[2], n[1], n[0]), Ordre::MJA => (n[2], l[2], n[0], n[1]) };
    let a = match la { 2 => 2000 + a, 4 => a, _ => return None };
    if !(2012..=2100).contains(&a) || !date_valide(a, mo, j) { return None; }
    Some((format!("{a:04}-{mo:02}-{j:02}"), m.2.clone()))
}

/// Format de date du fichier : celui (et le seul) qui lit toutes les dates.
fn format_dates(valeurs: &[&str]) -> Result<Ordre, String> {
    let ms: Vec<_> = valeurs.iter().filter_map(|v| morceaux(v)).collect();
    if ms.is_empty() { return Err("date illisible".into()); }
    let ok: Vec<Ordre> = [Ordre::AMJ, Ordre::JMA, Ordre::MJA].into_iter().filter(|o| ms.iter().all(|m| appliquer(*o, m).is_some())).collect();
    match ok.as_slice() {
        [o] => Ok(*o),
        [] => Err("format de date non reconnu".into()),
        _ => Err("format de date ambigu (jour, mois et année indiscernables) : réexportez avec le format année-mois-jour".into()),
    }
}

fn echanges(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "échanges (trades)".into(), ..Default::default() };
    let dates: Vec<&str> = t.lignes.iter().map(|(_, c, _)| champ(c, t.col("DATE"))).collect();
    let fmt = format_dates(&dates);
    for (n, c, brut) in &t.lignes {
        let d_h = match &fmt { Ok(o) => morceaux(champ(c, t.col("DATE"))).and_then(|m| appliquer(*o, &m)), Err(_) => None };
        let Some((d, h)) = d_h else { verifier(&mut imp, *n, "", fmt.clone().err().unwrap_or_else(|| "date illisible".into()), brut); continue };
        let Some((base, cote)) = paire(champ(c, t.col("PAIR")), COTATIONS) else { verifier(&mut imp, *n, &d, "paire illisible", brut); continue };
        let (base, cote) = (code(&base), code(&cote));
        let (Some(q), Some(prix)) = (nombre(champ(c, t.col("AMOUNT"))), nombre(champ(c, t.col("PRICE")))) else { verifier(&mut imp, *n, &d, "quantité ou prix illisible", brut); continue };
        if q == 0.0 { imp.ignorees += 1; continue; }
        let frais = nombre(champ(c, t.col("FEE"))).map(|f| (code(champ(c, t.col("FEE CURRENCY"))), f));
        ordre(&mut imp, *n, brut, &d, &h, P, &format!("Bitfinex : ordre {}", champ(c, t.col("ORDER ID"))), &base, q, &cote, q.abs() * prix, q > 0.0, frais);
    }
    imp
}

fn mouvements(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "dépôts et retraits (movements)".into(), ..Default::default() };
    let i_date = t.col("UPDATED").or_else(|| t.col("DATE"));
    let dates: Vec<&str> = t.lignes.iter().map(|(_, c, _)| champ(c, i_date)).collect();
    let fmt = format_dates(&dates);
    for (n, c, brut) in &t.lignes {
        let d_h = match &fmt { Ok(o) => morceaux(champ(c, i_date)).and_then(|m| appliquer(*o, &m)), Err(_) => None };
        let Some((d, h)) = d_h else { verifier(&mut imp, *n, "", fmt.clone().err().unwrap_or_else(|| "date illisible".into()), brut); continue };
        if champ(c, t.col("STATUS")).trim() != "COMPLETED" { imp.ignorees += 1; continue; }
        let a = code(champ(c, t.col("CURRENCY")));
        let q = nombre(champ(c, t.col("AMOUNT"))).unwrap_or(f64::NAN);
        let frais = if q < 0.0 { nombre(champ(c, t.col("FEES"))).unwrap_or(0.0) } else { 0.0 };
        mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, frais, format!("Bitfinex : {}", if q < 0.0 { "retrait" } else { "dépôt" }));
    }
    imp
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dates() {
        assert_eq!(format_dates(&["13-01-22 10:00:00", "05-03-22 10:00:00"]), Ok(Ordre::JMA));
        assert!(format_dates(&["22-01-13 10:00:00"]).is_err(), "2022-01-13 ou 22/01/2013 : ambigu");
        assert_eq!(format_dates(&["22-01-13 10:00:00", "05-03-24 10:00:00"]), Ok(Ordre::JMA), "05 = 2005 en AMJ : écarté");
        assert_eq!(format_dates(&["2022-01-13 10:00:00"]), Ok(Ordre::AMJ));
        assert_eq!(format_dates(&["01-13-22 10:00:00"]), Ok(Ordre::MJA));
    }
}
