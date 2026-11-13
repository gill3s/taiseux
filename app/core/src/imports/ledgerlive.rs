//! Ledger Live : export de l'historique des opérations (Paramètres → Comptes → « Exporter l'historique des
//! opérations », CSV). Une ligne par opération et par compte ; les jetons (ERC-20…) ont leur propre ligne, avec leur
//! code dans « Currency Ticker ».
//!
//! Sources : aide de Ledger (« Export your operation history ») et en-têtes relevés par BittyTax
//! (`src/bittytax/conv/parsers/ledgerlive.py`, noms de colonnes et types seulement). Variantes d'en-tête connues :
//! - avec statut : Operation Date, Status, Currency Ticker, Operation Type, Operation Amount, Operation Fees,
//!   Operation Hash, Account Name, Account xpub, Countervalue Ticker, Countervalue at Operation Date,
//!   Countervalue at CSV Export ;
//! - sans « Status » (mêmes colonnes) ;
//! - sans les colonnes « Countervalue … » ;
//! - ancienne : « Account id » au lieu de « Account xpub ».
//!
//! Traduction :
//! - IN : réception → transfert (supposé venir de vos comptes, signalé dans la note) ; les frais d'une réception
//!   sont payés par l'expéditeur, ils sont ignorés ;
//! - OUT : envoi → transfert ; « Operation Amount » comprend les frais (comme le lit BittyTax) : quantité envoyée =
//!   montant − frais, frais → `quantite_frais` ;
//! - REWARD, REWARD_PAYOUT : revenu gratuit (récompense de staking) ;
//! - FEES et opérations de gestion qui ne coûtent que des frais (REVEAL, DELEGATE, UNDELEGATE, BOND, UNBOND…) :
//!   frais seuls ; si le montant dépasse les frais, « à vérifier » ;
//! - NFT_IN, NFT_OUT, SLASH, type inconnu, statut autre que « Confirmed », jeton illisible : « à vérifier ».
//! Les échanges faits dans Ledger Live (« Swap ») n'apparaissent PAS comme tels : un envoi sur un compte et une
//! réception sur un autre. La note de chaque mouvement le rappelle.

use super::commun::*;
use super::portefeuilles::*;
use super::{AVerifier, Import};

const P: &str = "Ledger Live";
/// Opérations de gestion du compte qui ne déplacent que des frais (staking, activation, votes…).
const FRAIS_SEULS: &[&str] = &[
    "FEES", "REVEAL", "CREATE", "DELEGATE", "UNDELEGATE", "REDELEGATE", "BOND", "UNBOND", "WITHDRAW_UNBONDED",
    "FREEZE", "UNFREEZE", "VOTE", "NOMINATE", "CHILL", "SET_CONTROLLER", "APPROVE", "OPT_IN", "OPT_OUT",
];

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Operation Date", "Currency Ticker", "Operation Type", "Operation Amount"])?;
    let (i_date, i_statut, i_actif, i_type, i_q, i_frais, i_compte) = (
        t.col("Operation Date"), t.col("Status"), t.col("Currency Ticker"), t.col("Operation Type"),
        t.col("Operation Amount"), t.col("Operation Fees"), t.col("Account Name"),
    );
    let variante = match (t.a("Status"), t.a("Countervalue Ticker"), t.a("Account xpub")) {
        (true, _, _) => "historique des opérations (avec statut)",
        (false, true, _) => "historique des opérations",
        (false, false, true) => "historique des opérations (sans contre-valeur)",
        _ => "historique des opérations (ancien format, « Account id »)",
    };
    let mut imp = Import { plateforme: P.into(), format: variante.into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = horodatage(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim().to_uppercase();
        let a = actif(champ(c, i_actif));
        let compte = champ(c, i_compte).trim();
        let statut = champ(c, i_statut).trim();
        if i_statut.is_some() && !statut.eq_ignore_ascii_case("Confirmed") {
            verifier(&mut imp, &d, &format!("opération au statut « {statut} » (non confirmée ou échouée) : à saisir vous-même si des frais ont été payés"));
            continue;
        }
        if !actif_lisible(&a) { verifier(&mut imp, &d, &format!("jeton illisible (« {a} ») : à saisir vous-même")); continue; }
        let Some(q) = nombre(champ(c, i_q)).map(f64::abs) else { verifier(&mut imp, &d, "montant illisible"); continue };
        let frais = nombre(champ(c, i_frais)).map(f64::abs).unwrap_or(0.0);
        let detail = if compte.is_empty() { String::new() } else { format!("compte « {compte} »") };
        match typ.as_str() {
            "IN" => {
                let note = format!("{} Si cette réception est le produit d'un échange (Swap Ledger), saisissez plutôt un échange.", note_reception(P, &detail));
                imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note));
            }
            "OUT" => {
                if frais > q * (1.0 + 1e-9) + 1e-12 { verifier(&mut imp, &d, "frais supérieurs au montant de l'envoi"); continue; }
                let note = format!("{} Si cet envoi sert un échange (Swap Ledger), saisissez plutôt un échange.", note_envoi(P, &detail));
                imp.operations.push(transfert(&d, &h, P, &a, q - frais, frais, note));
            }
            "REWARD" | "REWARD_PAYOUT" => imp.operations.push(revenu(&d, &h, P, &a, q, format!("{P} : récompense ({typ})"))),
            o if FRAIS_SEULS.contains(&o) => {
                let f = if frais > 0.0 { frais } else if o == "FEES" { q } else { 0.0 };
                if q > f * (1.0 + 1e-9) + 1e-12 {
                    verifier(&mut imp, &d, &format!("opération « {o} » d'un montant de {q} {a}, supérieur aux frais : vérifiez si ce montant a quitté vos avoirs"));
                } else if f > 0.0 {
                    imp.operations.push(transfert(&d, &h, P, &a, 0.0, f, format!("{P} : frais de réseau ({o})")));
                } else {
                    imp.ignorees += 1;
                }
            }
            "NFT_IN" | "NFT_OUT" => verifier(&mut imp, &d, "NFT : hors du suivi automatique de Taiseux, à traiter à part"),
            "SLASH" => verifier(&mut imp, &d, "pénalité de staking (slash) : perte d'avoirs à saisir vous-même"),
            "NONE" if q == 0.0 && frais == 0.0 => imp.ignorees += 1,
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    Some(imp)
}
