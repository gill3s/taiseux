//! Coffre chiffré : aller-retour, mauvaise phrase, falsification de l'en-tête et du chiffré, paramètres absurdes.

use taiseux_core::engine::{Cours, Operation, TypeOperation};
use taiseux_core::vault::{ouvrir, sceller, sceller_avec, CompteEtranger, Dossier, ErreurCoffre, Membre};

fn dossier() -> Dossier {
    Dossier {
        version: 2,
        membres: vec![
            Membre { role: "declarant1".into(), nom: "Camille".into(), operations: vec![Operation {
                date: "2026-01-10".into(), kind: TypeOperation::Achat, montant: 1000.0,
                note: "Ledger — épargne".into(), ..Default::default()
            }], ..Default::default() },
            Membre { role: "declarant2".into(), ..Default::default() },
        ],
        cours: vec![Cours { actif: "BTC".into(), date: "2026-03-10".into(), prix: 81234.5 }],
        comptes: vec![CompteEtranger { plateforme: "Kraken".into(), identifiant: "AA12 N84G".into(), ..Default::default() }],
        ..Default::default()
    }
}
const PHRASE: &str = "cheval correct pile agrafe";
// Paramètres légers pour la vitesse des tests (le minimum accepté).
fn leger(d: &Dossier, p: &str) -> Vec<u8> { sceller_avec(d, p, 8 * 1024, 1, 1).unwrap() }

#[test]
fn aller_retour_et_rien_en_clair() {
    let c = leger(&dossier(), PHRASE);
    assert_eq!(ouvrir(&c, PHRASE).unwrap(), dossier());
    let texte = String::from_utf8_lossy(&c);
    assert!(!texte.contains("Ledger") && !texte.contains("2026-01-10") && !texte.contains("1000"));
}

#[test]
fn parametres_par_defaut() {
    let c = sceller(&dossier(), PHRASE).unwrap();
    assert_eq!(u32::from_le_bytes(c[8..12].try_into().unwrap()), 64 * 1024);
    assert_eq!(ouvrir(&c, PHRASE).unwrap(), dossier());
}

#[test]
fn sel_et_nonce_neufs() {
    let (a, b) = (leger(&dossier(), PHRASE), leger(&dossier(), PHRASE));
    assert_ne!(a[20..60], b[20..60]);
}

#[test]
fn mauvaise_phrase() {
    let c = leger(&dossier(), PHRASE);
    assert_eq!(ouvrir(&c, "cheval correct pile agrafF"), Err(ErreurCoffre::Dechiffrement));
}

#[test]
fn falsifications() {
    let c = leger(&dossier(), PHRASE);
    for i in [12usize, 25, 40, 70, c.len() - 1] {
        let mut f = c.clone();
        f[i] ^= 1;
        assert!(ouvrir(&f, PHRASE).is_err(), "octet {i} modifié non détecté");
    }
    assert_eq!(ouvrir(b"pas un coffre du tout, vraiment pas, non non non non non non non non", PHRASE), Err(ErreurCoffre::PasUnCoffre));
    assert_eq!(c[7], 2, "un coffre neuf est au format 2");
    let mut v = c.clone();
    v[7] = 3;
    assert_eq!(ouvrir(&v, PHRASE), Err(ErreurCoffre::VersionInconnue));
}

#[test]
fn parametres_absurdes_refuses() {
    let mut c = leger(&dossier(), PHRASE);
    c[8..12].copy_from_slice(&(4u32 * 1024 * 1024).to_le_bytes()); // 4 Gio : refusé avant tout calcul
    assert_eq!(ouvrir(&c, PHRASE), Err(ErreurCoffre::ParametresInvalides));
}

#[test]
fn phrase_trop_courte() {
    assert_eq!(sceller(&dossier(), "court"), Err(ErreurCoffre::PhraseTropCourte));
}

/// Un vrai coffre écrit par Taiseux 0.1.0 (fabriqué avec le code de la release, commit 20e4888) : la 0.2 l'ouvre et
/// range ses opérations chez le déclarant 1 ; réenregistré, il passe au format 2.
#[test]
fn coffre_de_la_version_0_1() {
    let octets = include_bytes!("donnees/coffre-0.1.taiseux");
    assert_eq!(octets[7], 1);
    let d = ouvrir(octets, "coffre de la version 0.1").unwrap();
    assert_eq!(d.version, 2);
    assert!(d.operations.is_empty());
    assert_eq!(d.membres.len(), 1);
    assert_eq!(d.membres[0].role, "declarant1");
    let ops = &d.membres[0].operations;
    assert_eq!(ops.len(), 2);
    assert_eq!((ops[0].kind, ops[0].montant, ops[0].note.as_str()), (TypeOperation::Achat, 1000.0, "Exemple BOFiP § 110"));
    assert_eq!((ops[1].kind, ops[1].valeur_portefeuille), (TypeOperation::Cession, Some(1200.0)));
    let r = taiseux_core::calculer(ops);
    assert!((r.cessions[0].plus_value - 75.0).abs() < 1e-9);
    let c = leger(&d, PHRASE);
    assert_eq!(c[7], 2);
    assert_eq!(ouvrir(&c, PHRASE).unwrap(), d);
}
