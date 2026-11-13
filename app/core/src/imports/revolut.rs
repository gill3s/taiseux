//! Revolut : relevé du compte crypto (application → Crypto → relevé, CSV). Deux variantes d'en-tête connues :
//! - actuelle : Symbol, Type, Quantity, Price, Value, Fees, Date ; montants avec symbole monétaire (« €1,000.00 »),
//!   date locale. Types : Buy, Sell, Send, Receive, Learn reward, Staking reward, Stake, Unstake ;
//! - ancienne : Type, Product, Started Date, Completed Date, Description, Amount, Currency, Fiat amount,
//!   Fiat amount (inc. fees), Fee, Base currency, State, Balance ; Currency = la crypto, montants en « Base
//!   currency ». Types : EXCHANGE (achat si Amount > 0, vente sinon), CARD_PAYMENT (paiement en crypto), TRANSFER.
//! Sources : noms de colonnes et valeurs de type de la variante actuelle relevés dans BittyTax
//! (`conv/parsers/revolut.py`) ; variante ancienne d'après l'exemple publié par le projet open source revolutax
//! (`example/revolut-2023.csv`) ; aucun code repris. Revolut ne documente pas le format publiquement.
//!
//! Doutes :
//! - Variante actuelle : on suit BittyTax (Value hors frais, Fees en sus) : prix d'achat = Value + Fees, prix de
//!   vente = Value, frais = Fees. Des outils signalent que Revolut écrit des quantités brutes (frais compris) : à
//!   confirmer sur un vrai export. Montant en dollars ou en livres → « à vérifier ».
//! - Le format exact de la date n'est pas documenté : ISO 8601 et l'écriture anglaise (« Jan 15, 2024, 10:33:12 AM »)
//!   sont lus ; sinon « date illisible ».
//! - Les relevés du compte courant (sans colonnes « Fiat amount ») ne sont pas reconnus : ils ne donnent pas la
//!   contre-valeur en euros des opérations crypto.

use super::bitstamp::date_anglaise;
use super::commun::*;
use super::{AVerifier, Import};

const P: &str = "Revolut";

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["Symbol", "Type", "Quantity", "Price", "Value", "Fees", "Date"]) { return Some(actuelle(t)); }
    let t = Table::trouver(texte, &["Type", "Product", "Started Date", "Completed Date", "Amount", "Currency", "Fiat amount", "Fiat amount (inc. fees)", "Fee", "Base currency"])?;
    Some(ancienne(t))
}

/// Monnaie d'un montant écrit avec son symbole ou son code : « €1,000.00 », « -€5.00 », « EUR 12.00 », « 12.00 USD ».
fn monnaie(s: &str) -> String {
    let s = s.trim();
    if s.contains('€') { return "EUR".into(); }
    if s.contains('$') { return "USD".into(); }
    if s.contains('£') { return "GBP".into(); }
    let lettres: String = s.chars().filter(|c| c.is_ascii_alphabetic()).collect::<String>().to_uppercase();
    if lettres.len() == 3 { lettres } else { String::new() }
}

/// Montant sans son code de monnaie (« EUR 12.00 » : le « E » serait lu comme un exposant).
fn valeur(s: &str) -> Option<f64> { nombre(&s.chars().filter(|c| !c.is_ascii_alphabetic()).collect::<String>()) }

fn date(s: &str) -> Option<(String, String)> { horodatage(s).or_else(|| date_anglaise(s)) }

fn actuelle(t: Table) -> Import {
    let (i_sym, i_type, i_q, i_v, i_f, i_date) = (t.col("Symbol"), t.col("Type"), t.col("Quantity"), t.col("Value"), t.col("Fees"), t.col("Date"));
    let mut imp = Import { plateforme: P.into(), format: "relevé du compte crypto".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = date(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim();
        let a = actif(champ(c, i_sym));
        let Some(q) = nombre(champ(c, i_q)).map(f64::abs).filter(|q| *q > 0.0) else { verifier(&mut imp, &d, "quantité illisible"); continue };
        let (vs, fs) = (champ(c, i_v), champ(c, i_f));
        let v = valeur(vs).map(f64::abs);
        let f = valeur(fs).map(f64::abs).unwrap_or(0.0);
        let mv = monnaie(vs);
        let mf = if f > 0.0 { monnaie(fs) } else { mv.clone() };
        let note = format!("Revolut : {typ}");
        if est_fiat(&a) { imp.ignorees += 1; continue; }
        match typ {
            "Buy" | "Sell" => {
                let vente = typ == "Sell";
                let Some(v) = v else { verifier(&mut imp, &d, "montant illisible"); continue };
                if mv != "EUR" || mf != "EUR" {
                    let m = if mv != "EUR" { &mv } else { &mf };
                    verifier(&mut imp, &d, &format!("{} en {} : à convertir en euros et à saisir", if vente { "vente" } else { "achat" }, if m.is_empty() { "monnaie inconnue" } else { m }));
                    continue;
                }
                if vente { imp.operations.push(cession(&d, &h, P, &a, q, v, f, note)) } else { imp.operations.push(achat(&d, &h, P, &a, q, v + f, note)) }
            }
            "Send" | "Receive" => imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note)),
            "Learn reward" | "Staking reward" => imp.operations.push(revenu(&d, &h, P, &a, q, note)),
            "Stake" | "Unstake" => imp.ignorees += 1,
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    imp
}

fn ancienne(t: Table) -> Import {
    let (i_type, i_fin, i_debut, i_desc, i_m, i_cur, i_fiat, i_f, i_base, i_etat) = (
        t.col("Type"), t.col("Completed Date"), t.col("Started Date"), t.col("Description"), t.col("Amount"), t.col("Currency"),
        t.col("Fiat amount"), t.col("Fee"), t.col("Base currency"), t.col("State"),
    );
    let mut imp = Import { plateforme: P.into(), format: "relevé du compte crypto (ancien format)".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let etat = champ(c, i_etat).trim();
        if !etat.is_empty() && !etat.eq_ignore_ascii_case("COMPLETED") { imp.ignorees += 1; continue; }
        let ds = if champ(c, i_fin).trim().is_empty() { champ(c, i_debut) } else { champ(c, i_fin) };
        let Some((d, h)) = date(ds) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim().to_uppercase();
        let a = actif(champ(c, i_cur));
        let base = actif(champ(c, i_base));
        let Some(m) = nombre(champ(c, i_m)) else { verifier(&mut imp, &d, "quantité illisible"); continue };
        let q = m.abs();
        let fiat = nombre(champ(c, i_fiat)).map(f64::abs);
        let f = nombre(champ(c, i_f)).map(f64::abs).unwrap_or(0.0);
        let note = format!("Revolut : {typ} {}", champ(c, i_desc).trim());
        if est_fiat(&a) { imp.ignorees += 1; continue; }
        if !(q > 0.0) { verifier(&mut imp, &d, "quantité nulle"); continue; }
        match typ.as_str() {
            "EXCHANGE" | "CARD_PAYMENT" => {
                if base != "EUR" { verifier(&mut imp, &d, &format!("montants en {} : à convertir en euros et à saisir", if base.is_empty() { "monnaie inconnue" } else { &base })); continue; }
                let Some(v) = fiat else { verifier(&mut imp, &d, "montant en euros illisible"); continue };
                if typ == "EXCHANGE" && m > 0.0 {
                    imp.operations.push(achat(&d, &h, P, &a, q, v + f, note));
                } else if m < 0.0 {
                    let note = if typ == "CARD_PAYMENT" { format!("{note} (paiement en crypto : cession)") } else { note };
                    imp.operations.push(cession(&d, &h, P, &a, q, v, f, note));
                } else {
                    verifier(&mut imp, &d, "paiement par carte avec un montant positif (remboursement ?) : à apprécier vous-même");
                }
            }
            "TRANSFER" => imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note)),
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    imp
}
