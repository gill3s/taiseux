//! Crypto.com (application mobile) : export CSV du portefeuille crypto (et du portefeuille fiat, même en-tête).
//! Colonnes : Timestamp (UTC), Transaction Description, Currency, Amount, To Currency, To Amount, Native Currency,
//! Native Amount, Native Amount (in USD), Transaction Kind[, Transaction Hash]. Deux variantes : avec ou sans
//! « Transaction Hash ». « Native » = monnaie de référence choisie dans l'application (EUR pour un résident français).
//! Source : noms de colonnes et valeurs de « Transaction Kind » relevés dans BittyTax (`conv/parsers/cryptocom.py`) ;
//! aucun code repris. La plateforme d'échange Crypto.com Exchange a un autre export, non pris en charge ici.
//!
//! Prudence :
//! - Échanges (viban_purchase, crypto_exchange, crypto_viban_exchange…) : une seule ligne, actif donné dans
//!   Currency/Amount, actif reçu dans To Currency/To Amount. Une monnaie autre que l'euro → « à vérifier ».
//! - Paiements en crypto (crypto_payment) et rechargements de carte (card_top_up) : cessions imposables, au prix
//!   donné par « Native Amount » s'il est en euros.
//! - Envois entre utilisateurs (crypto_transfer), conversions de « poussière » (deux lignes séparées), annulations,
//!   remboursements : « à vérifier ».
//! - Si l'export du portefeuille fiat est importé en plus de celui du portefeuille crypto, les achats en euros
//!   (viban_purchase) peuvent y figurer deux fois : n'importer que l'export crypto.

use std::collections::BTreeMap;

use super::commun::*;
use super::{AVerifier, Import};

const P: &str = "Crypto.com";

const ECHANGES: &[&str] = &[
    "viban_purchase", "van_purchase", "crypto_viban_exchange", "crypto_exchange", "crypto_to_van_sell_order",
    "trading.limit_order.fiat_wallet.sell_commit", "trading.limit_order.cash_account.purchase_commit",
    "trading.limit_order.crypto_wallet.exchange", "recurring_buy_order",
];
const REVENUS: &[&str] = &[
    "crypto_earn_interest_paid", "crypto_earn_extra_interest_paid", "mco_stake_reward", "supercharger_reward_to_app_credited",
    "finance.lockup.dpos_compound_interest.crypto_wallet", "finance.dpos.non_compound_interest.crypto_wallet",
    "finance.dpos.compound_interest.crypto_wallet", "finance.crypto_earn.loyalty_program_extra_interest_paid.crypto_wallet",
    "rewards_platform_deposit_credited", "referral_bonus", "referral_gift", "referral_card_cashback", "transfer_cashback",
    "gift_card_reward", "campaign_reward",
];
const INTERNES: &[&str] = &[
    "crypto_earn_program_created", "crypto_earn_program_withdrawn", "lockup_lock", "lockup_unlock", "lockup_upgrade",
    "supercharger_deposit", "supercharger_withdrawal", "council_node_deposit_created",
    "trading.limit_order.fiat_wallet.purchase_lock", "trading.limit_order.fiat_wallet.purchase_unlock",
    "trading.limit_order.fiat_wallet.sell_lock", "trading.limit_order.fiat_wallet.sell_unlock",
    "trading.limit_order.cash_account.purchase_lock", "trading.limit_order.cash_account.purchase_unlock",
    "trading.limit_order.cash_account.sell_lock", "trading.limit_order.cash_account.sell_unlock",
    "trading.limit_order.crypto_wallet.fund_lock", "trading.limit_order.crypto_wallet.fund_unlock",
    "finance.lockup.dpos_lock.crypto_wallet", "finance.dpos.staking.crypto_wallet", "finance.dpos.unstaking.crypto_wallet",
    "viban_deposit_precredit", "viban_deposit_precredit_repayment",
];
/// Échanges internes en deux lignes (« swap » d'un jeton par un autre lors d'une migration, etc.).
const SWAPS: &[&str] = &[
    "lockup_swap_credited", "lockup_swap_debited", "dynamic_coin_swap_credited", "dynamic_coin_swap_debited",
    "dynamic_coin_swap_bonus_exchange_deposit", "interest_swap_credited", "interest_swap_debited",
    "crypto_wallet_swap_credited", "crypto_wallet_swap_debited", "dust_conversion_debited", "dust_conversion_credited",
];

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Timestamp (UTC)", "Transaction Description", "Currency", "Amount", "To Currency", "To Amount", "Native Currency", "Native Amount", "Transaction Kind"])?;
    let (i_ts, i_desc, i_cur, i_m, i_to, i_to_m, i_nat, i_nat_m, i_kind) = (
        t.col("Timestamp (UTC)"), t.col("Transaction Description"), t.col("Currency"), t.col("Amount"), t.col("To Currency"),
        t.col("To Amount"), t.col("Native Currency"), t.col("Native Amount"), t.col("Transaction Kind"),
    );
    let format = if t.a("Transaction Hash") { "historique du portefeuille (avec empreinte de transaction)" } else { "historique du portefeuille" };
    let mut imp = Import { plateforme: P.into(), format: format.into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = horodatage(champ(c, i_ts)) else { verifier(&mut imp, "", "date illisible"); continue };
        let kind = champ(c, i_kind).trim();
        let a = actif(champ(c, i_cur));
        let Some(m) = nombre(champ(c, i_m)) else { verifier(&mut imp, &d, "montant illisible"); continue };
        let (to, to_m) = (actif(champ(c, i_to)), nombre(champ(c, i_to_m)).map(f64::abs));
        let (nat, nat_m) = (actif(champ(c, i_nat)), nombre(champ(c, i_nat_m)).map(f64::abs));
        let desc = champ(c, i_desc).trim();
        let note = format!("Crypto.com : {}", if kind.is_empty() { desc } else { kind });
        let valeur_eur = if nat == "EUR" { nat_m } else { None };
        let sans_eur = |nat: &str| format!("valeur en {} : à convertir en euros et à saisir", if nat.is_empty() { "monnaie inconnue" } else { nat });

        if ECHANGES.contains(&kind) {
            let Some(qr) = to_m.filter(|_| !to.is_empty()) else { verifier(&mut imp, &d, "échange sans actif reçu lisible"); continue };
            let mut bruts = BTreeMap::new();
            *bruts.entry(a.clone()).or_insert(0.0) -= m.abs();
            *bruts.entry(to.clone()).or_insert(0.0) += qr;
            match operations_echange(classer(&bruts, &BTreeMap::new()), &d, &h, P, &note) {
                Ok(ops) => imp.operations.extend(ops),
                Err(motif) => verifier(&mut imp, &d, &motif),
            }
            continue;
        }
        if INTERNES.contains(&kind) { imp.ignorees += 1; continue; }
        if est_fiat(&a) {
            // Portefeuille fiat : dépôts, retraits, rechargements de carte en euros.
            imp.ignorees += 1;
            continue;
        }
        let q = m.abs();
        if !(q > 0.0) { verifier(&mut imp, &d, "quantité nulle ou illisible"); continue; }
        match kind {
            k if REVENUS.contains(&k) && m > 0.0 => imp.operations.push(revenu(&d, &h, P, &a, q, note)),
            "crypto_purchase" | "trading.crypto_purchase.google_pay" if m > 0.0 => match valeur_eur {
                Some(v) => imp.operations.push(achat(&d, &h, P, &a, q, v, note)),
                None => verifier(&mut imp, &d, &format!("achat par carte, {}", sans_eur(&nat))),
            },
            "crypto_payment" | "card_top_up" if m < 0.0 => match valeur_eur {
                Some(v) => imp.operations.push(cession(&d, &h, P, &a, q, v, 0.0, format!("{note} (paiement en crypto : cession)"))),
                None => verifier(&mut imp, &d, &format!("paiement en crypto (cession imposable), {}", sans_eur(&nat))),
            },
            "crypto_withdrawal" | "crypto_to_exchange_transfer" | "crypto_deposit" | "exchange_to_crypto_transfer" =>
                imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note)),
            "crypto_transfer" => verifier(&mut imp, &d, "envoi ou réception entre utilisateurs Crypto.com : don, paiement ou mouvement entre vos comptes ? À préciser"),
            k if SWAPS.contains(&k) => verifier(&mut imp, &d, "conversion en deux lignes séparées (poussière, migration de jeton) : à saisir comme un échange"),
            "card_cashback_reverted" | "reimbursement_reverted" | "reimbursement" | "admin_wallet_credited" =>
                verifier(&mut imp, &d, &format!("« {kind} » (annulation, remboursement ou crédit administratif) : à apprécier vous-même")),
            "" => verifier(&mut imp, &d, &format!("opération sans type : « {desc} »")),
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    Some(imp)
}
