//! Nexo : relevé des transactions (CSV). Source : noms de colonnes et valeurs relevés par BittyTax
//! (`conv/parsers/nexo.py`, noms de colonnes et valeurs de type seulement). Variantes :
//! - ancienne : Transaction, Type, Currency, Amount, [USD Equivalent,] Details, Outstanding Loan, Date / Time
//!   (échange : Currency « BTC/EUR », Amount « -0.1 / +3000 ») ;
//! - intermédiaire : Transaction, Type, Input Currency, Input Amount, Output Currency, Output Amount, USD Equivalent,
//!   Details, [Outstanding Loan,] Date / Time ou Date / Time (UTC) ;
//! - actuelle : …, USD Equivalent, Fee, Fee Currency, Details, Date / Time (UTC), [normalizedDisplayDetails].
//! « Date / Time » (sans UTC) est l'heure de Zurich, identique à celle de Paris : reprise telle quelle.
//! Codes propres à Nexo : EURX, USDX, GBPX (soldes en monnaie officielle chez Nexo) → EUR, USD, GBP ; BNBN → BNB,
//! NEXONEXO / NEXOBNB / NEXOBEP2 → NEXO, USDTERC → USDT.
//! Intérêts, bonus, parrainage, cashback, dividendes → revenus ; prêts, remboursements, liquidations, ventes pour
//! rembourser un prêt, achats par carte → « à vérifier » ; mouvements internes (dépôts à terme, portefeuille Pro) →
//! ignorés ; lignes « rejected » → ignorées.

use super::aides_lot3::*;
use super::commun::*;
use super::Import;

const P: &str = "Nexo";

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Transaction", "Type", "Details"])?;
    let ancienne = t.a("Currency") && t.a("Amount");
    if !(ancienne || (t.a("Input Currency") && t.a("Input Amount") && t.a("Output Currency") && t.a("Output Amount"))) { return None; }
    let i_date = t.col("Date / Time (UTC)").or_else(|| t.col("Date / Time"))?;
    let format = if ancienne { "relevé (ancien format)" } else if t.a("Fee") { "relevé" } else { "relevé (format intermédiaire)" };
    let mut imp = Import { plateforme: P.into(), format: format.into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let Some((d, h)) = horodatage(champ(c, Some(i_date))) else { verifier(&mut imp, *n, "", "date illisible", brut); continue };
        let details = champ(c, t.col("Details"));
        if details.contains("rejected") { imp.ignorees += 1; continue; }
        let typ = champ(c, t.col("Type")).trim();
        // (actif donné, quantité donnée, actif reçu, quantité reçue)
        let (ad, qd, ar, qr) = if ancienne {
            let cur = champ(c, t.col("Currency"));
            let am = champ(c, t.col("Amount"));
            if typ == "Exchange" {
                let (Some((a1, a2)), Some((m1, m2))) = (cur.split_once('/'), am.split_once('/')) else { verifier(&mut imp, *n, &d, "échange illisible", brut); continue };
                (code(a1), nombre(m1).map(f64::abs), code(a2), nombre(m2).map(f64::abs))
            } else {
                let q = nombre(am);
                (code(cur), q.map(f64::abs), code(cur), q)
            }
        } else {
            (code(champ(c, t.col("Input Currency"))), nombre(champ(c, t.col("Input Amount"))).map(f64::abs),
             code(champ(c, t.col("Output Currency"))), nombre(champ(c, t.col("Output Amount"))))
        };
        let (Some(qd), Some(qr)) = (qd, qr) else { verifier(&mut imp, *n, &d, "montant illisible", brut); continue };
        let frais = match champ(c, t.col("Fee")).trim() { "" | "-" => None, f => nombre(f).map(|q| (code(champ(c, t.col("Fee Currency"))), q.abs())) };
        let note = format!("Nexo : {typ}");
        match typ {
            "Deposit" | "ExchangeDepositedOn" | "Exchange Deposited On" | "Top up Crypto" => {
                if details.contains("Credit") || details.contains("null") || details.contains("OTC") {
                    verifier(&mut imp, *n, &d, "crédit ou achat de gré à gré en dollars : à convertir en euros et à saisir", brut);
                } else if details.contains("Airdrop") || details.contains("Earn Interest Adjustment") {
                    if est_fiat(&ar) { imp.ignorees += 1 } else { imp.operations.push(revenu(&d, &h, P, &ar, qr.abs(), note)) }
                } else if ad != ar {
                    // Dépôt converti à l'arrivée (ex. euros en EURX, ou en un autre actif).
                    if est_fiat(&ad) && est_fiat(&ar) { imp.ignorees += 1 } else { conversion(&mut imp, *n, brut, &d, &h, P, &note, &ad, qd, &ar, qr.abs(), frais) }
                } else {
                    mouvement(&mut imp, *n, brut, &d, &h, P, &ar, qr.abs(), 0.0, note);
                }
            }
            "Interest" | "FixedTermInterest" | "Fixed Term Interest" | "InterestAdditional" | "Interest Additional" if qr < 0.0 =>
                verifier(&mut imp, *n, &d, "intérêts d'un prêt : hors du champ de Taiseux, à vérifier", brut),
            "Interest" | "FixedTermInterest" | "Fixed Term Interest" | "InterestAdditional" | "Interest Additional" | "Dividend"
            | "ReferralBonus" | "Referral Bonus" | "Cashback" | "Exchange Cashback" | "Bonus" =>
                if est_fiat(&ar) { verifier(&mut imp, *n, &d, format!("revenu en {ar} : à saisir à la main"), brut) }
                else if qr > 0.0 { imp.operations.push(revenu(&d, &h, P, &ar, qr, note)) }
                else { verifier(&mut imp, *n, &d, "revenu négatif ou nul", brut) },
            "Exchange" | "Exchange Collateral" | "DepositToExchange" | "Deposit To Exchange" | "ExchangeToWithdraw" | "Exchange To Withdraw" => {
                // Frais dans l'actif donné : déjà compris dans la quantité donnée (Input Amount).
                let frais = frais.filter(|(a, _)| *a != ad);
                conversion(&mut imp, *n, brut, &d, &h, P, &note, &ad, qd, &ar, qr.abs(), frais);
            }
            "Withdrawal" | "WithdrawExchanged" | "Withdraw Exchanged" => {
                if details.contains("Transfer to Nexo User") {
                    verifier(&mut imp, *n, &d, "envoi à un autre utilisateur de Nexo (don ou paiement) : à vérifier", brut);
                } else if ad != ar && !(est_fiat(&ad) && est_fiat(&ar)) {
                    verifier(&mut imp, *n, &d, "retrait avec conversion : à vérifier", brut);
                } else {
                    let f = frais.filter(|(a, _)| *a == ad).map(|(_, q)| q).unwrap_or(0.0);
                    mouvement(&mut imp, *n, brut, &d, &h, P, &ad, (qd - f).max(0.0), f, note);
                }
            }
            "WithdrawalCredit" | "Withdrawal Credit" | "Loan Withdrawal" | "Manual Sell Order" | "Automatic Sell Order" | "Liquidation"
            | "Repayment" | "Manual Repayment" | "Forced Repayment" =>
                verifier(&mut imp, *n, &d, "prêt adossé à des cryptos (crédit, remboursement, liquidation) : hors du champ de Taiseux, à vérifier", brut),
            "Administrator" | "Administrative Deduction" | "Nexo Card Purchase" | "Assimilation" | "CreditCardStatus" | "Credit Card Status" =>
                verifier(&mut imp, *n, &d, "achat par carte ou ajustement administratif : valeur en euros à saisir à la main", brut),
            "LockingTermDeposit" | "Locking Term Deposit" | "TransferIn" | "Transfer In" | "TransferOut" | "Transfer Out" | "UnlockingTermDeposit"
            | "Unlocking Term Deposit" | "Transfer To Pro Wallet" | "Transfer From Pro Wallet" => imp.ignorees += 1,
            autre => verifier(&mut imp, *n, &d, format!("type « {autre} » non pris en charge"), brut),
        }
    }
    Some(imp)
}

/// Code d'actif Nexo → code usuel.
fn code(s: &str) -> String {
    let a = actif(s);
    match a.as_str() {
        "EURX" => "EUR".into(), "USDX" => "USD".into(), "GBPX" => "GBP".into(),
        "BNBN" => "BNB".into(), "NEXONEXO" | "NEXOBNB" | "NEXOBEP2" => "NEXO".into(), "USDTERC" => "USDT".into(),
        _ => a,
    }
}
