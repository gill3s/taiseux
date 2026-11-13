//! Bybit : dépôts et retraits, et registre du compte dérivés. Sources : aide Bybit « How to Self-Export Account
//! Data » (rubriques exportables) et noms de colonnes relevés par BittyTax (`conv/parsers/bybit.py`, noms de colonnes
//! et valeurs de type seulement). Variantes :
//! - dépôts et retraits v2 : Date & Time(UTC), Coin, QTY, Type, Account Balance, Description (Deposit, Withdrawal,
//!   Transfer to / from Derivatives Account) ;
//! - dépôts et retraits v1 : Type (userDeposit, internalAccountTransferDeposit / Withdrawal), Coin, Amount,
//!   Wallet Balance, Time(UTC) ;
//! - registre du compte dérivés : Time, Currency, Contract, Type, Direction, Quantity, Position, Filled Price, Funding,
//!   Fee Paid, Cash Flow, Change, Wallet Balance, Fee Rate, Trade ID, Order ID. Types : deposit, withdraw,
//!   exchangeIn / exchangeOut (conversion, deux lignes à la même seconde), transferIn / transferOut (internes) ;
//!   trade, funding, liquidation (contrats) → « à vérifier ».
//! **Non pris en charge : l'historique des ordres au comptant (« Spot Trade History »)** : ni Bybit ni BittyTax ne
//! documentent ses colonnes (seule l'API fiscale décrit des champs, sans garantir qu'ils soient ceux du CSV).

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "Bybit";

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["Date & Time(UTC)", "Coin", "QTY", "Type", "Account Balance", "Description"]) {
        let mut imp = Import { plateforme: P.into(), format: "dépôts et retraits (v2)".into(), ..Default::default() };
        for (n, c, brut) in &t.lignes {
            let Some((d, h)) = horodatage(champ(c, t.col("Date & Time(UTC)"))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
            let a = actif(champ(c, t.col("Coin")));
            let q = nombre(champ(c, t.col("QTY"))).unwrap_or(f64::NAN);
            match champ(c, t.col("Description")).trim() {
                x @ ("Deposit" | "Withdrawal") => mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, 0.0, format!("Bybit : {x}")),
                "Transfer to Derivatives Account" | "Transfer from Derivatives Account" => imp.ignorees += 1,
                autre => verifier(&mut imp, *n, &d, format!("opération « {autre} » non prise en charge"), brut),
            }
        }
        return Some(imp);
    }
    if let Some(t) = Table::trouver(texte, &["Type", "Coin", "Amount", "Wallet Balance", "Time(UTC)"]) {
        let mut imp = Import { plateforme: P.into(), format: "dépôts et retraits (v1)".into(), ..Default::default() };
        for (n, c, brut) in &t.lignes {
            let Some((d, h)) = horodatage(champ(c, t.col("Time(UTC)"))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
            let a = actif(champ(c, t.col("Coin")));
            let q = nombre(champ(c, t.col("Amount"))).unwrap_or(f64::NAN);
            match champ(c, t.col("Type")).trim() {
                "userDeposit" => mouvement(&mut imp, *n, brut, &d, &h, P, &a, q, 0.0, "Bybit : dépôt".into()),
                "internalAccountTransferDeposit" | "internalAccountTransferWithdrawal" => imp.ignorees += 1,
                autre => verifier(&mut imp, *n, &d, format!("type « {autre} » non pris en charge"), brut),
            }
        }
        return Some(imp);
    }
    if let Some(t) = Table::trouver(texte, &["Time", "Currency", "Contract", "Type", "Direction", "Cash Flow", "Change", "Fee Paid", "Order ID"]) {
        return Some(derives(&t));
    }
    None
}

fn derives(t: &Table) -> Import {
    let mut imp = Import { plateforme: P.into(), format: "registre du compte dérivés".into(), ..Default::default() };
    // Conversions : lignes exchangeIn / exchangeOut à la même seconde.
    let mut conv: Vec<(String, String, usize)> = Vec::new();
    for (k, (n, c, brut)) in t.lignes.iter().enumerate() {
        let Some((d, h)) = horodatage(champ(c, t.col("Time"))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let a = actif(champ(c, t.col("Currency")));
        let flux = nombre(champ(c, t.col("Cash Flow"))).unwrap_or(f64::NAN);
        let typ = champ(c, t.col("Type")).trim();
        match typ {
            "deposit" => mouvement(&mut imp, *n, brut, &d, &h, P, &a, flux, 0.0, "Bybit : dépôt".into()),
            "withdraw" => mouvement(&mut imp, *n, brut, &d, &h, P, &a, flux, nombre(champ(c, t.col("Fee Paid"))).unwrap_or(0.0), "Bybit : retrait".into()),
            "exchangeIn" | "exchangeOut" => conv.push((d, h, k)),
            "transferIn" | "transferOut" => imp.ignorees += 1,
            "trade" | "funding" | "liquidation" | "settlement" =>
                verifier(&mut imp, *n, &d, "contrats à terme ou perpétuels : hors du champ de Taiseux (régime différent)", brut),
            autre => verifier(&mut imp, *n, &d, format!("type « {autre} » non pris en charge"), brut),
        }
    }
    let mut i = 0;
    while i < conv.len() {
        let mut j = i;
        while j < conv.len() && conv[j].0 == conv[i].0 && conv[j].1 == conv[i].1 { j += 1; }
        let lignes: Vec<(String, f64, bool)> = conv[i..j].iter().map(|(_, _, k)| {
            let c = &t.lignes[*k].1;
            (actif(champ(c, t.col("Currency"))), nombre(champ(c, t.col("Change"))).unwrap_or(0.0), false)
        }).collect();
        let brut = conv[i..j].iter().map(|(_, _, k)| t.lignes[*k].2.clone()).collect::<Vec<_>>().join("\n");
        let (d, h) = (conv[i].0.clone(), conv[i].1.clone());
        transaction(&mut imp, t.lignes[conv[i].2].0, &brut, &d, &h, P, "Bybit : conversion", &lignes);
        i = j;
    }
    imp
}
