//! KuCoin : chaque export est un fichier séparé (ordres, conversions, achats par carte, dépôts, retraits, Earn).
//! Sources : aide KuCoin « Export de l'historique » (archive « bundle » : un CSV par rubrique) et noms de colonnes
//! relevés par BittyTax (`conv/parsers/kucoin.py`, noms de colonnes et valeurs de type seulement). Variantes :
//! - ordres exécutés, archive actuelle (« Spot Orders_Filled Orders ») : UID, Account Type, Order ID,
//!   [Order Time(UTC±hh:mm)], Symbol, Side, Order Type, [Order Price, Order Amount], Avg. Filled Price, Filled Amount,
//!   Filled Volume, Filled Volume (USDT), Filled Time(UTC±hh:mm) (ou « Filled Time », heure de Singapour), Fee,
//!   [Fee Currency], [Tax], [Maker/Taker], [Status], [Account Mode] ;
//! - anciens ordres (heure de Singapour, UTC+8) : « tradeCreatedAt, orderId, symbol, side, price, size, funds, fee,
//!   liquidity, feeCurrency, orderType » ; « oid, symbol, dealPrice, dealValue, amount, fee, direction, createdDate » ;
//!   « uid, symbol, order_type, price, amount_coin, direction, funds, fee, created_at » ; « uid, symbol, direction,
//!   deal_price, amount, deal_value, created_at » (sans frais) ;
//! - conversions (« Convert Orders ») : UID, Account Type, Payment Account, Sell, Buy, Price, Tax, Time of Update(UTC…),
//!   Status (« 0.1 BTC », « 5000 USDT ») ;
//! - achats par carte ou virement : Order ID, Order Type, Currency (Crypto), « Crypto Quantity, » (sic), Price,
//!   Currency (Fiat), Fiat Amount, Fee Currency, Fee, Status (DONE), Time(UTC…) ; et Order ID, Currency (Crypto),
//!   Crypto Quantity, Currency (Fiat), Fiat Amount, Fee Currency (Fiat), Fee (Fiat), Fee Currency (Crypto),
//!   Fee (Crypto), Payment Method, Status (SUCCEEDED), Time(UTC…) ;
//! - dépôts d'euros : Order ID, Currency (Fiat), Fiat Amount, Fee, Deposit Method, Status, Time(UTC…) → ignorés ;
//! - dépôts et retraits d'actifs : archive (UID, Account Type, Time(UTC…), Coin, Amount, Fee, …, Status (SUCCESS),
//!   Remarks ; « Withdrawal Address/Account » pour les retraits) ; anciens fichiers « Time, Coin, [Network], Amount,
//!   Type, Remark[, Fee] » (dépôts), « Time, Coin, [Network], Amount, Type, Wallet Address[/Account], Remark »
//!   (retraits) et « coin_type, type, add, hash, vol, created_at » (DEPOSIT / WITHDRAW) ;
//! - Earn (« Earn Orders_Profit History ») : …, Staked Coin, Product Type, Product Name, Earnings Coin, Earnings Type,
//!   Remarks, Amount, … → revenus ;
//! - contrats à terme (« Futures Orders_Realized PNL ») : reconnus, tout part « à vérifier ».
//! Non pris en charge : l'historique de compte (« Account History »), qui répète les ordres ligne à ligne.

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "KuCoin";
const SINGAPOUR: i64 = 480;

pub fn lire(texte: &str) -> Option<Import> {
    // Ordres : archive actuelle, puis anciennes versions.
    if let Some(t) = Table::trouver(texte, &["Symbol", "Side", "Avg. Filled Price", "Filled Amount", "Filled Volume", "Fee"]) {
        let (i_t, dec) = colonne_fuseau(&t, "Filled Time").or_else(|| t.col("Filled Time").map(|i| (i, SINGAPOUR)))?;
        let c = Cols { t: Some(i_t), dec, paire: t.col("Symbol"), sens: t.col("Side"), q: t.col("Filled Amount"), total: t.col("Filled Volume"),
                       frais: t.col("Fee"), coin_frais: t.col("Fee Currency"), compte: t.col("Account Type"), taxe: t.col("Tax") };
        return Some(ordres(&t, c, "ordres exécutés (archive)"));
    }
    if let Some(t) = Table::trouver(texte, &["tradeCreatedAt", "symbol", "side", "size", "funds", "fee", "feeCurrency"]) {
        let c = Cols { t: t.col("tradeCreatedAt"), dec: SINGAPOUR, paire: t.col("symbol"), sens: t.col("side"), q: t.col("size"), total: t.col("funds"),
                       frais: t.col("fee"), coin_frais: t.col("feeCurrency"), compte: None, taxe: None };
        return Some(ordres(&t, c, "ordres (ancien format v3)"));
    }
    if let Some(t) = Table::trouver(texte, &["oid", "symbol", "dealPrice", "dealValue", "amount", "fee", "direction", "createdDate"]) {
        let c = Cols { t: t.col("createdDate"), dec: SINGAPOUR, paire: t.col("symbol"), sens: t.col("direction"), q: t.col("amount"), total: t.col("dealValue"),
                       frais: t.col("fee"), coin_frais: None, compte: None, taxe: None };
        return Some(ordres(&t, c, "ordres (ancien format v4)"));
    }
    if let Some(t) = Table::trouver(texte, &["uid", "symbol", "order_type", "amount_coin", "direction", "funds", "fee", "created_at"]) {
        let c = Cols { t: t.col("created_at"), dec: SINGAPOUR, paire: t.col("symbol"), sens: t.col("direction"), q: t.col("amount_coin"), total: t.col("funds"),
                       frais: t.col("fee"), coin_frais: None, compte: None, taxe: None };
        return Some(ordres(&t, c, "ordres (ancien format v2)"));
    }
    if let Some(t) = Table::trouver(texte, &["uid", "symbol", "direction", "deal_price", "amount", "deal_value", "created_at"]) {
        let c = Cols { t: t.col("created_at"), dec: SINGAPOUR, paire: t.col("symbol"), sens: t.col("direction"), q: t.col("amount"), total: t.col("deal_value"),
                       frais: None, coin_frais: None, compte: None, taxe: None };
        return Some(ordres(&t, c, "ordres (ancien format v1)"));
    }
    if let Some(t) = Table::trouver(texte, &["Payment Account", "Sell", "Buy", "Price", "Status"]) {
        if let Some((i_t, dec)) = colonne_fuseau(&t, "Time of Update") { return Some(conversions(&t, i_t, dec)); }
    }
    if let Some(t) = Table::trouver(texte, &["Order ID", "Currency (Crypto)", "Currency (Fiat)", "Fiat Amount", "Status"]) {
        if let Some((i_t, dec)) = colonne_fuseau(&t, "Time") { return Some(achats_monnaie(&t, i_t, dec)); }
    }
    if let Some(t) = Table::trouver(texte, &["Order ID", "Currency (Fiat)", "Fiat Amount", "Fee", "Deposit Method", "Status"]) {
        return Some(Import { plateforme: P.into(), format: "dépôts en monnaie officielle".into(), ignorees: t.lignes.len(), ..Default::default() });
    }
    if let Some(t) = Table::trouver(texte, &["Staked Coin", "Earnings Coin", "Earnings Type", "Amount"]) {
        if let Some((i_t, dec)) = colonne_fuseau(&t, "Time") { return Some(earn(&t, i_t, dec)); }
    }
    if let Some(t) = Table::trouver(texte, &["Symbol", "Close Type", "Realized PNL", "Total Realized PNL"]) {
        let mut imp = Import { plateforme: P.into(), format: "contrats à terme (résultats réalisés)".into(), ..Default::default() };
        for (n, _, brut) in &t.lignes { verifier(&mut imp, *n, "", "contrats à terme : hors du champ de Taiseux (régime différent)", brut); }
        return Some(imp);
    }
    // Dépôts et retraits : archive (avec statut) puis anciens fichiers.
    if let Some(t) = Table::trouver(texte, &["UID", "Account Type", "Coin", "Amount", "Fee", "Status", "Remarks"]) {
        let fuseau = colonne_fuseau(&t, "Time").or_else(|| t.col("Time").map(|i| (i, SINGAPOUR)));
        if let Some((i_t, dec)) = fuseau { return Some(depots_retraits(&t, Some(i_t), dec, t.a("Withdrawal Address/Account"), true)); }
    }
    if let Some(t) = Table::trouver(texte, &["Time", "Coin", "Amount", "Type", "Remark"]) {
        let retrait = t.a("Wallet Address") || t.a("Wallet Address/Account");
        return Some(depots_retraits(&t, t.col("Time"), SINGAPOUR, retrait, false));
    }
    if let Some(t) = Table::trouver(texte, &["coin_type", "type", "add", "hash", "vol", "created_at"]) {
        let mut imp = Import { plateforme: P.into(), format: "dépôts et retraits (ancien format)".into(), ..Default::default() };
        for (n, c, brut) in &t.lignes {
            let Some((d, h)) = horodatage_utc(champ(c, t.col("created_at")), SINGAPOUR) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
            let a = actif(champ(c, t.col("coin_type")));
            let q = nombre(champ(c, t.col("vol"))).unwrap_or(f64::NAN);
            match champ(c, t.col("type")).trim() {
                "DEPOSIT" | "WITHDRAW" => mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, 0.0, format!("KuCoin : {}", champ(c, t.col("type")))),
                autre => verifier(&mut imp, *n, &d, format!("type « {autre} » non pris en charge"), brut),
            }
        }
        return Some(imp);
    }
    None
}

struct Cols {
    t: Option<usize>, dec: i64, paire: Option<usize>, sens: Option<usize>, q: Option<usize>, total: Option<usize>,
    frais: Option<usize>, coin_frais: Option<usize>, compte: Option<usize>, taxe: Option<usize>,
}

fn ordres(t: &Table, k: Cols, format: &str) -> Import {
    let mut imp = Import { plateforme: P.into(), format: format.into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        if champ(c, Some(0)).starts_with("No matching records") { continue; }
        let Some((d, h)) = horodatage_utc(champ(c, k.t), k.dec) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        if champ(c, k.compte).to_lowercase().contains("margin") || champ(c, k.compte).to_lowercase().contains("futures") {
            verifier(&mut imp, *n, &d, "ordre sur marge : hors du champ de Taiseux (régime différent)", brut); continue;
        }
        if nombre(champ(c, k.taxe)).is_some_and(|x| x != 0.0) {
            verifier(&mut imp, *n, &d, "ordre avec une taxe retenue par KuCoin : à saisir à la main", brut); continue;
        }
        let Some((base, cote)) = paire(champ(c, k.paire), &[]) else { verifier(&mut imp, *n, &d, "paire illisible", brut); continue };
        let (Some(q), Some(total)) = (nombre(champ(c, k.q)), nombre(champ(c, k.total))) else { verifier(&mut imp, *n, &d, "quantité ou total illisible", brut); continue };
        if q == 0.0 { imp.ignorees += 1; continue; } // ordre non exécuté
        let sens = champ(c, k.sens).trim().to_lowercase();
        let achat_ = match sens.as_str() { "buy" => true, "sell" => false, _ => { verifier(&mut imp, *n, &d, format!("sens « {sens} » inconnu"), brut); continue } };
        let frais = nombre(champ(c, k.frais)).unwrap_or(0.0);
        // Sans colonne de devise des frais (anciens formats), les frais sont dans la monnaie de cotation.
        let coin_frais = if k.coin_frais.is_some() { actif(champ(c, k.coin_frais)) } else { cote.clone() };
        let note = format!("KuCoin : ordre {} {}", if achat_ { "d'achat" } else { "de vente" }, champ(c, k.paire));
        ordre(&mut imp, *n, brut, &d, &h, P, &note, &base, q, &cote, total, achat_, Some((coin_frais, frais)));
    }
    imp
}

/// « 0.1 BTC » → (0.1, BTC).
fn quantite_actif(s: &str) -> Option<(f64, String)> {
    let mut it = s.split_whitespace();
    let q = nombre(it.next()?)?;
    let a = actif(it.next()?);
    if a.is_empty() { None } else { Some((q, a)) }
}

fn conversions(t: &Table, i_t: usize, dec: i64) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "conversions (Convert)".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        if champ(c, Some(0)).starts_with("No matching records") { continue; }
        let Some((d, h)) = horodatage_utc(champ(c, Some(i_t)), dec) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        if champ(c, t.col("Status")).trim() != "SUCCESS" { imp.ignorees += 1; continue; }
        if nombre(champ(c, t.col("Tax"))).is_some_and(|x| x != 0.0) { verifier(&mut imp, *n, &d, "conversion avec une taxe retenue par KuCoin : à saisir à la main", brut); continue; }
        match (quantite_actif(champ(c, t.col("Sell"))), quantite_actif(champ(c, t.col("Buy")))) {
            (Some((qd, ad)), Some((qr, ar))) => conversion(&mut imp, *n, brut, &d, &h, P, "KuCoin : conversion", &ad, qd, &ar, qr, None),
            _ => verifier(&mut imp, *n, &d, "conversion : quantités illisibles", brut),
        }
    }
    imp
}

fn achats_monnaie(t: &Table, i_t: usize, dec: i64) -> Import {
    let v2 = t.a("Fee (Fiat)");
    let mut imp = Import { plateforme: P.into(), format: format!("achats par carte ou virement{}", if v2 { " (v2)" } else { "" }), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        if champ(c, Some(0)).starts_with("No matching records") { continue; }
        let Some((d, h)) = horodatage_utc(champ(c, Some(i_t)), dec) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let statut = champ(c, t.col("Status")).trim();
        if statut != if v2 { "SUCCEEDED" } else { "DONE" } { imp.ignorees += 1; continue; }
        if champ(c, t.col("Order Type")).to_lowercase().contains("sell") {
            verifier(&mut imp, *n, &d, "vente contre monnaie officielle : format non documenté, à saisir à la main", brut); continue;
        }
        let a = actif(champ(c, t.col("Currency (Crypto)")));
        let m = actif(champ(c, t.col("Currency (Fiat)")));
        let q = nombre(champ(c, t.col("Crypto Quantity").or_else(|| t.col("Crypto Quantity,")))).unwrap_or(f64::NAN);
        let montant = nombre(champ(c, t.col("Fiat Amount"))).unwrap_or(f64::NAN);
        if !(q.is_finite() && montant.is_finite()) { verifier(&mut imp, *n, &d, "quantité ou montant illisible", brut); continue; }
        let mut lignes = vec![(a.clone(), q.abs(), false), (m.clone(), -montant.abs(), false)];
        if v2 {
            if let Some(f) = nombre(champ(c, t.col("Fee (Fiat)"))).filter(|f| *f != 0.0) { lignes.push((actif(champ(c, t.col("Fee Currency (Fiat)"))), f, true)); }
            if let Some(f) = nombre(champ(c, t.col("Fee (Crypto)"))).filter(|f| *f != 0.0) { lignes.push((actif(champ(c, t.col("Fee Currency (Crypto)"))), f, true)); }
        } else if let Some(f) = nombre(champ(c, t.col("Fee"))).filter(|f| *f != 0.0) {
            lignes.push((actif(champ(c, t.col("Fee Currency"))), f, true));
        }
        transaction(&mut imp, *n, brut, &d, &h, P, "KuCoin : achat par carte ou virement", &lignes);
    }
    imp
}

fn earn(t: &Table, i_t: usize, dec: i64) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "revenus Earn".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        if champ(c, Some(0)).starts_with("No matching records") { continue; }
        let Some((d, h)) = horodatage_utc(champ(c, Some(i_t)), dec) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let a = actif(champ(c, t.col("Earnings Coin")));
        match nombre(champ(c, t.col("Amount"))) {
            Some(q) if q > 0.0 && !a.is_empty() => imp.operations.push(revenu(&d, &h, P, &a, q, format!("KuCoin : {}", champ(c, t.col("Earnings Type"))))),
            _ => verifier(&mut imp, *n, &d, "revenu Earn illisible ou négatif", brut),
        }
    }
    imp
}

fn depots_retraits(t: &Table, i_t: Option<usize>, dec: i64, retrait: bool, statut: bool) -> Import {
    let mut imp = Import { plateforme: P.into(), format: format!("{}{}", if retrait { "retraits" } else { "dépôts" }, if statut { " (archive)" } else { " (ancien format)" }), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        if champ(c, Some(0)).starts_with("No matching records") { continue; }
        let Some((d, h)) = horodatage_utc(champ(c, i_t), dec) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        if statut && champ(c, t.col("Status")).trim() != "SUCCESS" { imp.ignorees += 1; continue; }
        let a = actif(champ(c, t.col("Coin")));
        let q = nombre(champ(c, t.col("Amount"))).unwrap_or(f64::NAN);
        let frais = nombre(champ(c, t.col("Fee"))).unwrap_or(0.0);
        mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, if retrait { frais } else { 0.0 }, format!("KuCoin : {}", if retrait { "retrait" } else { "dépôt" }));
    }
    imp
}
