//! Exodus : export des transactions (Paramètres → « Export all transactions », un CSV par actif ou pour tous) et
//! export des récompenses de staking.
//!
//! Source : en-têtes relevés par BittyTax (`exodus.py`, noms de colonnes et types seulement) ; Exodus ne publie pas
//! de description de ses colonnes. Variantes d'en-tête connues :
//! - ancienne (deux côtés par ligne) : DATE, TYPE, FROMPORTFOLIO, TOPORTFOLIO, OUTAMOUNT, OUTCURRENCY, FEEAMOUNT,
//!   FEECURRENCY, [FROMADDRESS,] TOADDRESS, OUTTXID, OUTTXURL, INAMOUNT, INCURRENCY, INTXID, INTXURL, ORDERID,
//!   PERSONALNOTE (TOADDRESS parfois en dernier) ; types deposit, withdrawal, exchange, deposit (failed),
//!   withdrawal (failed) ;
//! - actuelle (un côté par ligne) : TXID, TXURL, DATE, TYPE, FROMPORTFOLIO, TOPORTFOLIO, COINAMOUNT (« -0.01 BTC »),
//!   FEE (« 0.0001 BTC »), BALANCE, EXCHANGE, PERSONALNOTE ; types deposit, withdrawal et leurs « (failed) » ;
//! - récompenses de staking (format CoinTracking) : Type, Buy, Cur., Exchange, Group, Comment, Date ; type Staking.
//! DATE est écrite par JavaScript (« Fri Jan 01 2021 12:00:00 GMT+0100 (…) ») ou en ISO 8601 : la date retenue
//! est celle écrite.
//!
//! Traduction : deposit → transfert entrant (supposé venir de vos comptes, signalé ; les frais d'une réception sont
//! payés par l'expéditeur et ignorés) ; withdrawal → transfert sortant (signalé), frais → `quantite_frais` (ou
//! transfert à part si payés dans un autre actif) ; exchange (ancienne variante) → échange crypto contre crypto,
//! frais de réseau à part ; échec → frais seuls ; Staking → revenu gratuit.
//! Variante actuelle : un échange (swap) y est découpé en un retrait et un dépôt portant la même valeur dans la
//! colonne EXCHANGE ; Taiseux ne les réunit en un échange que s'il y a exactement un retrait et un dépôt d'actifs
//! différents pour cette valeur ; sinon, ces lignes partent dans « à vérifier ». Une monnaie officielle dans un
//! échange, un type inconnu ou un montant illisible partent aussi dans « à vérifier ».

use std::collections::BTreeMap;

use super::commun::*;
use super::portefeuilles::*;
use super::{AVerifier, Import};

const P: &str = "Exodus";

pub fn lire(texte: &str) -> Option<Import> {
    if let Some(t) = Table::trouver(texte, &["DATE", "TYPE", "OUTAMOUNT", "OUTCURRENCY", "INAMOUNT", "INCURRENCY", "FEEAMOUNT", "FEECURRENCY"]) {
        return Some(ancienne(t));
    }
    if let Some(t) = Table::trouver(texte, &["TXID", "DATE", "TYPE", "COINAMOUNT", "FEE", "EXCHANGE"]) {
        return Some(actuelle(t));
    }
    if let Some(t) = Table::trouver(texte, &["Type", "Buy", "Cur.", "Exchange", "Group", "Comment", "Date"]) {
        // Le format CoinTracking complet a aussi « Sell » : ce n'est pas l'export de staking d'Exodus.
        if !t.a("Sell") { return Some(staking(t)); }
    }
    None
}

fn date(s: &str) -> Option<(String, String)> { horodatage(s).or_else(|| date_js(s)).or_else(|| date_jma(s)) }

/// « -0.01 BTC » → (0.01, « BTC »).
fn montant_actif(s: &str) -> Option<(f64, String)> {
    let mut it = s.split_whitespace();
    let q = nombre(it.next()?)?.abs();
    let a = actif(it.next()?);
    if it.next().is_some() || !actif_lisible(&a) { return None; }
    Some((q, a))
}

fn note_perso(n: &str) -> String { if n.trim().is_empty() { String::new() } else { format!("« {} »", n.trim()) } }

/// Pousse un envoi (avec frais dans le même actif ou à part).
fn envoi(imp: &mut Import, d: &str, h: &str, a: &str, q: f64, frais: Option<(f64, String)>, detail: &str) {
    match frais {
        Some((f, fa)) if fa == a => imp.operations.push(transfert(d, h, P, a, q, f, note_envoi(P, detail))),
        Some((f, fa)) => {
            imp.operations.push(transfert(d, h, P, a, q, 0.0, note_envoi(P, detail)));
            imp.operations.push(transfert(d, h, P, &fa, 0.0, f, format!("{P} : frais de réseau de l'envoi de {a}")));
        }
        None => imp.operations.push(transfert(d, h, P, a, q, 0.0, note_envoi(P, detail))),
    }
}

fn ancienne(t: Table) -> Import {
    let (i_date, i_type, i_qo, i_ao, i_qi, i_ai, i_qf, i_af, i_note) = (
        t.col("DATE"), t.col("TYPE"), t.col("OUTAMOUNT"), t.col("OUTCURRENCY"), t.col("INAMOUNT"), t.col("INCURRENCY"),
        t.col("FEEAMOUNT"), t.col("FEECURRENCY"), t.col("PERSONALNOTE"),
    );
    let mut imp = Import { plateforme: P.into(), format: "export des transactions (ancien format)".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        if c.iter().all(|x| x.is_empty()) { continue; }
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = date(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim().to_lowercase();
        let cote = |iq, ia| -> Option<(f64, String)> {
            let a = actif(champ(c, ia));
            let q = nombre(champ(c, iq))?.abs();
            if actif_lisible(&a) { Some((q, a)) } else { None }
        };
        let frais = cote(i_qf, i_af).filter(|(f, _)| *f > 0.0);
        let detail = note_perso(champ(c, i_note));
        match typ.as_str() {
            "deposit" => match cote(i_qi, i_ai) {
                Some((q, a)) if !est_fiat(&a) => imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note_reception(P, &detail))),
                _ => verifier(&mut imp, &d, "dépôt : montant ou actif illisible"),
            },
            "withdrawal" => match cote(i_qo, i_ao) {
                Some((q, a)) if !est_fiat(&a) => envoi(&mut imp, &d, &h, &a, q, frais, &detail),
                _ => verifier(&mut imp, &d, "retrait : montant ou actif illisible"),
            },
            "exchange" => match (cote(i_qo, i_ao), cote(i_qi, i_ai)) {
                (Some((qo, ao)), Some((qi, ai))) if !est_fiat(&ao) && !est_fiat(&ai) && ao != ai => {
                    imp.operations.push(echange(&d, &h, P, &ao, qo, &ai, qi, format!("{P} : échange dans le portefeuille")));
                    if let Some((f, fa)) = frais { imp.operations.push(transfert(&d, &h, P, &fa, 0.0, f, format!("{P} : frais de réseau de l'échange"))); }
                }
                _ => verifier(&mut imp, &d, "échange : actifs ou montants illisibles, ou monnaie officielle (à saisir vous-même)"),
            },
            "deposit (failed)" | "withdrawal (failed)" => match frais {
                Some((f, fa)) => imp.operations.push(transfert(&d, &h, P, &fa, 0.0, f, format!("{P} : transaction échouée, seuls les frais sont perdus"))),
                None => imp.ignorees += 1,
            },
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    imp
}

fn actuelle(t: Table) -> Import {
    let (i_date, i_type, i_q, i_frais, i_ech, i_note) = (t.col("DATE"), t.col("TYPE"), t.col("COINAMOUNT"), t.col("FEE"), t.col("EXCHANGE"), t.col("PERSONALNOTE"));
    let mut imp = Import { plateforme: P.into(), format: "export des transactions".into(), ..Default::default() };
    // Valeur de la colonne EXCHANGE → indices des lignes d'un même échange.
    let mut echanges: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (k, (n, c, brut)) in t.lignes.iter().enumerate() {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = date(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim().to_lowercase();
        let frais = montant_actif(champ(c, i_frais)).filter(|(f, _)| *f > 0.0);
        let detail = note_perso(champ(c, i_note));
        let ech = champ(c, i_ech).trim();
        if !ech.is_empty() && matches!(typ.as_str(), "deposit" | "withdrawal") { echanges.entry(ech.to_string()).or_default().push(k); continue; }
        match typ.as_str() {
            "deposit" => match montant_actif(champ(c, i_q)) {
                Some((q, a)) if !est_fiat(&a) => imp.operations.push(transfert(&d, &h, P, &a, q, 0.0, note_reception(P, &detail))),
                _ => verifier(&mut imp, &d, "dépôt : montant ou actif illisible"),
            },
            "withdrawal" => match montant_actif(champ(c, i_q)) {
                Some((q, a)) if !est_fiat(&a) => envoi(&mut imp, &d, &h, &a, q, frais, &detail),
                _ => verifier(&mut imp, &d, "retrait : montant ou actif illisible"),
            },
            "deposit (failed)" | "withdrawal (failed)" => match frais {
                Some((f, fa)) => imp.operations.push(transfert(&d, &h, P, &fa, 0.0, f, format!("{P} : transaction échouée, seuls les frais sont perdus"))),
                None => imp.ignorees += 1,
            },
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    for (ech, ks) in echanges {
        let lu = |k: usize| {
            let c = &t.lignes[k].1;
            (champ(c, i_type).trim().to_lowercase(), montant_actif(champ(c, i_q)), montant_actif(champ(c, i_frais)).filter(|(f, _)| *f > 0.0), date(champ(c, i_date)))
        };
        let lignes: Vec<_> = ks.iter().map(|&k| (k, lu(k))).collect();
        let retraits: Vec<_> = lignes.iter().filter(|(_, l)| l.0 == "withdrawal").collect();
        let depots: Vec<_> = lignes.iter().filter(|(_, l)| l.0 == "deposit").collect();
        let paire = match (retraits.as_slice(), depots.as_slice()) {
            ([(_, (_, Some((qo, ao)), frais, Some((d, h))))], [(_, (_, Some((qi, ai)), _, _))]) if ao != ai && !est_fiat(ao) && !est_fiat(ai) =>
                Some((d.clone(), h.clone(), ao.clone(), *qo, ai.clone(), *qi, frais.clone())),
            _ => None,
        };
        match paire {
            Some((d, h, ao, qo, ai, qi, frais)) => {
                imp.operations.push(echange(&d, &h, P, &ao, qo, &ai, qi, format!("{P} : échange dans le portefeuille ({ech}), reconstitué à partir du retrait et du dépôt")));
                if let Some((f, fa)) = frais { imp.operations.push(transfert(&d, &h, P, &fa, 0.0, f, format!("{P} : frais de réseau de l'échange"))); }
            }
            None => {
                let (n0, c0, _) = &t.lignes[ks[0]];
                imp.a_verifier.push(AVerifier {
                    ligne: *n0,
                    date: date(champ(c0, i_date)).map(|(d, _)| d).unwrap_or_default(),
                    motif: format!("échange « {ech} » : impossible de réunir exactement un retrait et un dépôt d'actifs différents ; à saisir vous-même"),
                    contenu: ks.iter().map(|&k| t.lignes[k].2.clone()).collect::<Vec<_>>().join("\n"),
                });
            }
        }
    }
    imp
}

fn staking(t: Table) -> Import {
    let (i_type, i_q, i_a, i_date, i_com) = (t.col("Type"), t.col("Buy"), t.col("Cur."), t.col("Date"), t.col("Comment"));
    let mut imp = Import { plateforme: P.into(), format: "récompenses de staking".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = date(champ(c, i_date)) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim();
        let a = actif(champ(c, i_a));
        let q = nombre(champ(c, i_q)).map(f64::abs);
        match (typ, q) {
            ("Staking", Some(q)) if actif_lisible(&a) && !est_fiat(&a) && q > 0.0 => {
                let com = champ(c, i_com).trim();
                imp.operations.push(revenu(&d, &h, P, &a, q, format!("{P} : récompense de staking{}", if com.is_empty() { String::new() } else { format!(" ({com})") })));
            }
            ("Staking", _) => verifier(&mut imp, &d, "récompense de staking : montant ou actif illisible"),
            (autre, _) => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge")),
        }
    }
    imp
}
