//! Coinomi : historique des transactions (Paramètres → « Export transactions », CSV).
//!
//! Source : en-tête relevé par BittyTax (`coinomi.py`, noms de colonnes seulement) ; Coinomi ne publie pas de
//! description de son export. Variante d'en-tête connue (une seule) :
//! Asset, AccountName, Address, AddressName, Value, Symbol, Fees, InternalTransfer, TransactionID, Time(UTC),
//! Time(ISO8601-UTC), BlockExplorer.
//! « Value » est signée ; pour un envoi elle comprend les frais (« Fees », dans le même actif), comme le lit
//! BittyTax : quantité envoyée = |Value| − Fees.
//!
//! Traduction : « InternalTransfer » vrai → transfert entre vos comptes Coinomi (certain, sans avertissement) ;
//! sinon réception → transfert (supposé venir de vos comptes, signalé), envoi → transfert (signalé), frais →
//! `quantite_frais`. Les échanges faits dans Coinomi (services d'échange intégrés) apparaissent comme un envoi et
//! une réception : la note le rappelle. Jeton illisible, montant illisible → « à vérifier ».

use super::commun::*;
use super::portefeuilles::*;
use super::{AVerifier, Import};

const P: &str = "Coinomi";

pub fn lire(texte: &str) -> Option<Import> {
    let t = Table::trouver(texte, &["AccountName", "Value", "Symbol", "Fees", "InternalTransfer", "Time(ISO8601-UTC)"])?;
    let (i_v, i_actif, i_frais, i_interne, i_ts, i_compte, i_nom) = (
        t.col("Value"), t.col("Symbol"), t.col("Fees"), t.col("InternalTransfer"), t.col("Time(ISO8601-UTC)"), t.col("AccountName"), t.col("AddressName"),
    );
    let mut imp = Import { plateforme: P.into(), format: "historique des transactions".into(), ..Default::default() };
    for (n, c, brut) in &t.lignes {
        let verifier = |imp: &mut Import, date: &str, motif: &str| imp.a_verifier.push(AVerifier { ligne: *n, date: date.into(), motif: motif.into(), contenu: brut.clone() });
        let Some((d, h)) = horodatage(champ(c, i_ts)) else { verifier(&mut imp, "", "date illisible"); continue };
        let a = actif(champ(c, i_actif));
        if !actif_lisible(&a) { verifier(&mut imp, &d, &format!("jeton illisible (« {a} ») : à saisir vous-même")); continue; }
        let Some(v) = nombre(champ(c, i_v)) else { verifier(&mut imp, &d, "montant illisible"); continue };
        let frais = nombre(champ(c, i_frais)).map(f64::abs).unwrap_or(0.0);
        let interne = matches!(champ(c, i_interne).trim().to_lowercase().as_str(), "true" | "yes" | "1" | "oui");
        let (compte, nom) = (champ(c, i_compte).trim(), champ(c, i_nom).trim());
        let detail = [compte, nom].iter().filter(|s| !s.is_empty()).map(|s| format!("« {s} »")).collect::<Vec<_>>().join(", ");
        let echange = " Si ce mouvement fait partie d'un échange fait dans Coinomi, saisissez plutôt un échange.";
        if v > 0.0 {
            let note = if interne { format!("{P} : transfert entre vos comptes Coinomi (réception)") } else { format!("{}{echange}", note_reception(P, &detail)) };
            imp.operations.push(transfert(&d, &h, P, &a, v, 0.0, note));
        } else if v < 0.0 {
            let sortie = -v;
            if frais > sortie * (1.0 + 1e-9) + 1e-12 { verifier(&mut imp, &d, "frais supérieurs au montant de l'envoi"); continue; }
            let note = if interne { format!("{P} : transfert entre vos comptes Coinomi (envoi)") } else { format!("{}{echange}", note_envoi(P, &detail)) };
            imp.operations.push(transfert(&d, &h, P, &a, sortie - frais, frais, note));
        } else if frais > 0.0 {
            imp.operations.push(transfert(&d, &h, P, &a, 0.0, frais, format!("{P} : frais de réseau")));
        } else {
            imp.ignorees += 1;
        }
    }
    Some(imp)
}
