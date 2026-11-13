//! Gemini : historique des transactions (« Transaction History », fourni en tableur, à enregistrer en CSV). Source :
//! noms de colonnes et valeurs relevés par BittyTax (`conv/parsers/gemini.py`, noms de colonnes et valeurs de type
//! seulement). Une variante, à colonnes variables : Date, Time (UTC), Type, Symbol, Specification, Liquidity Indicator,
//! Trading Fee Rate (bps), puis pour chaque actif « <A> Amount <A> », « Fee (<A>) <A> », « <A> Balance <A> », puis
//! Trade ID, Order ID, Order Date, Order Time, Client Order ID, API Session, Tx Hash, Deposit Destination,
//! Deposit Tx Output, Withdrawal Destination, Withdrawal Tx Output.
//! Types : Buy, Sell (Symbol = paire, ex. BTCEUR ; frais dans la monnaie de cotation), Credit, Debit (dépôts, retraits).
//! Contrats perpétuels (paires en « PERP ») et autres types → « à vérifier ».

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "Gemini";
const COTATIONS: &[&str] = &["BCH", "BTC", "DAI", "ETH", "EUR", "FIL", "GBP", "GUSD", "LTC", "RLUSD", "SGD", "SOL", "USD", "USDC", "USDT"];

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Date", "Time (UTC)", "Type", "Symbol", "Specification", "Liquidity Indicator"])?;
    let mut imp = Import { plateforme: P.into(), format: "historique des transactions".into(), ..Default::default() };
    let montant = |c: &[String], a: &str| nombre(champ(c, t.col(&format!("{a} Amount {a}")))).map(f64::abs);
    let frais = |c: &[String], a: &str| nombre(champ(c, t.col(&format!("Fee ({a}) {a}")))).map(f64::abs).unwrap_or(0.0);
    for (n, c, brut) in &t.lignes {
        if champ(c, t.col("Date")).trim().is_empty() { continue; } // lignes vides et totaux du tableur
        let Some((d, _)) = horodatage(champ(c, t.col("Date"))) else { verifier(&mut imp, *n, "", "date illisible (format AAAA-MM-JJ attendu)", brut); continue };
        let h: String = champ(c, t.col("Time (UTC)")).chars().take(8).collect();
        let h = if h.len() == 8 { h } else { String::new() };
        let typ = champ(c, t.col("Type")).trim();
        let sym = champ(c, t.col("Symbol")).trim().to_uppercase();
        match typ {
            "Credit" | "Debit" => match montant(c, &sym) {
                Some(q) => mouvement(&mut imp, *n, brut, &d, &h, P, &sym, q, if typ == "Debit" { frais(c, &sym) } else { 0.0 }, format!("Gemini : {typ}")),
                None => verifier(&mut imp, *n, &d, "montant illisible", brut),
            },
            "Buy" | "Sell" => {
                let Some((base, cote)) = (if sym.ends_with("PERP") { None } else { paire(&sym, COTATIONS) }) else {
                    verifier(&mut imp, *n, &d, "paire inconnue ou contrat perpétuel : hors du champ de Taiseux", brut); continue
                };
                let (Some(qb), Some(qq)) = (montant(c, &base), montant(c, &cote)) else { verifier(&mut imp, *n, &d, "montants illisibles", brut); continue };
                ordre(&mut imp, *n, brut, &d, &h, P, &format!("Gemini : {typ} {sym}"), &base, qb, &cote, qq, typ == "Buy", Some((cote.clone(), frais(c, &cote))));
            }
            autre => verifier(&mut imp, *n, &d, format!("type « {autre} » non pris en charge"), brut),
        }
    }
    Some(imp)
}
