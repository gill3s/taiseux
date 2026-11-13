//! SwissBorg : relevé de compte (« Account statement », proposé en Excel ; à enregistrer en CSV). Une variante
//! d'en-tête connue, précédée de lignes d'introduction (nom, période) :
//! Local time, Time in UTC, Type, Currency, Gross amount, Gross amount (XXX), Fee, Fee (XXX), Net amount,
//! Net amount (XXX), Note — où XXX est la monnaie de référence du compte (EUR, CHF, GBP, USD…).
//! Types : Deposit, Withdrawal, Buy, Sell, Payouts (Note « Yield payouts » = rendement, sinon bonus, parrainage,
//! airdrop), Fee Adjustment.
//! Source : noms de colonnes et valeurs de type relevés dans BittyTax (`conv/parsers/swissborg.py`) ; aucun code repris.
//!
//! Un échange s'écrit en deux lignes de même « Time in UTC » : « Sell » (actif donné) et « Buy » (actif reçu,
//! frais dans cet actif). Elles sont regroupées et classées (achat, cession ou échange). Les colonnes « (XXX) »
//! ne servent pas : seules comptent les quantités réelles (un achat payé en CHF part donc « à vérifier »).
//! « Fee Adjustment » (remboursement de frais) : « à vérifier ».

use std::collections::BTreeMap;

use super::commun::*;
use super::{AVerifier, Import};

const P: &str = "SwissBorg";

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["Local time", "Time in UTC", "Type", "Currency", "Gross amount", "Fee", "Net amount", "Note"])?;
    if !t.entete.iter().any(|h| h.starts_with("Gross amount (")) { return None; }
    let (i_ts, i_type, i_cur, i_gross, i_fee, i_net, i_note) = (t.col("Time in UTC"), t.col("Type"), t.col("Currency"), t.col("Gross amount"), t.col("Fee"), t.col("Net amount"), t.col("Note"));
    let mut imp = Import { plateforme: P.into(), format: "relevé de compte".into(), ..Default::default() };
    // Horodatage UTC → indices des lignes Buy / Sell, dans l'ordre du fichier.
    let mut groupes: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut ordre: Vec<String> = Vec::new();

    for (k, (n, c, brut)) in t.lignes.iter().enumerate() {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let ts = champ(c, i_ts).trim();
        let Some((d, h)) = horodatage(ts) else { verifier(&mut imp, "", "date illisible"); continue };
        let typ = champ(c, i_type).trim();
        let a = actif(champ(c, i_cur));
        let gross = nombre(champ(c, i_gross)).map(f64::abs);
        let fee = nombre(champ(c, i_fee)).map(f64::abs).unwrap_or(0.0);
        let net = nombre(champ(c, i_net)).map(f64::abs);
        let note_sb = champ(c, i_note).trim();
        let note = format!("SwissBorg : {typ}{}", if note_sb.is_empty() { String::new() } else { format!(" ({note_sb})") });
        match typ {
            "Buy" | "Sell" => {
                if !groupes.contains_key(ts) { ordre.push(ts.to_string()); }
                groupes.entry(ts.to_string()).or_default().push(k);
            }
            "Deposit" | "Withdrawal" if est_fiat(&a) => imp.ignorees += 1,
            "Deposit" | "Withdrawal" => match gross.or(net) {
                Some(q) if q > 0.0 => imp.operations.push(transfert(&d, &h, P, &a, q, if typ == "Withdrawal" { fee } else { 0.0 }, note)),
                _ => verifier(&mut imp, &d, "quantité illisible"),
            },
            "Payouts" if !est_fiat(&a) => match net.or(gross) {
                Some(q) if q > 0.0 => imp.operations.push(revenu(&d, &h, P, &a, q, note)),
                _ => verifier(&mut imp, &d, "quantité illisible"),
            },
            "Fee Adjustment" => verifier(&mut imp, &d, "remboursement de frais : à apprécier vous-même"),
            autre => verifier(&mut imp, &d, &format!("type « {autre} » non pris en charge{}", if est_fiat(&a) { &" (monnaie officielle)" as &str } else { "" })),
        }
    }

    for ts in ordre {
        let ks = &groupes[&ts];
        let (n0, c0, _) = &t.lignes[ks[0]];
        let (d, h) = horodatage(champ(c0, i_ts)).unwrap_or_default();
        let (mut bruts, mut frais) = (BTreeMap::new(), BTreeMap::new());
        let (mut achats, mut ventes) = (0, 0);
        for &k in ks {
            let c = &t.lignes[k].1;
            let a = actif(champ(c, i_cur));
            let g = nombre(champ(c, i_gross)).unwrap_or(0.0).abs();
            let f = nombre(champ(c, i_fee)).unwrap_or(0.0).abs();
            if champ(c, i_type).trim() == "Buy" { achats += 1; *bruts.entry(a.clone()).or_insert(0.0) += g; } else { ventes += 1; *bruts.entry(a.clone()).or_insert(0.0) -= g; }
            if f > 0.0 { *frais.entry(a).or_insert(0.0) += f; }
        }
        let resultat = if achats == 1 && ventes == 1 {
            operations_echange(classer(&bruts, &frais), &d, &h, P, "SwissBorg : échange")
        } else {
            Err(format!("{achats} ligne(s) Buy et {ventes} ligne(s) Sell à la même seconde : échange impossible à reconstituer"))
        };
        match resultat {
            Ok(ops) => imp.operations.extend(ops),
            Err(motif) => imp.a_verifier.push(AVerifier {
                ligne: *n0, date: d.clone(), motif,
                contenu: ks.iter().map(|&k| t.lignes[k].2.clone()).collect::<Vec<_>>().join("\n"),
            }),
        }
    }
    Some(imp)
}
