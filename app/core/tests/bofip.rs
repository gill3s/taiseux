//! Contrôles du moteur : exemples chiffrés du BOFiP (BOI-RPPM-PVBMC-30) et de la notice du 2086,
//! seuil de 305 €, moins-value non reportable, CSV, puis recoupement avec la référence Python indépendante
//! (`../tests/reference.py`, fractions exactes) sur 500 portefeuilles aléatoires.

use taiseux_core::csv;
use taiseux_core::engine::{calculer, euro_entier, Operation, TypeOperation::*};

fn op(date: &str, kind: taiseux_core::TypeOperation, montant: f64, v: Option<f64>, frais: Option<f64>) -> Operation {
    Operation { date: date.into(), kind, montant, valeur_portefeuille: v, frais, ..Default::default() }
}
fn achat(d: &str, m: f64) -> Operation { op(d, Achat, m, None, None) }
fn cession(d: &str, m: f64, v: f64) -> Operation { op(d, Cession, m, Some(v), None) }
fn proche(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * b.abs().max(1.0) }

#[test]
fn bofip_110_et_notice() {
    // Achat 1 000 € ; mars : portefeuille 1 200 €, cession 450 € ; août : tout cédé, portefeuille 1 300 €.
    let r = calculer(&[achat("2026-01-10", 1000.0), cession("2026-03-10", 450.0, 1200.0), cession("2026-08-10", 1300.0, 1300.0)]);
    assert!(r.erreurs.is_empty());
    assert!(proche(r.cessions[0].fraction_imputee, 375.0));
    assert!(proche(r.cessions[0].plus_value, 75.0));
    assert!(proche(r.cessions[1].pta_net, 625.0));
    assert!(proche(r.cessions[1].fractions_capital_initial, 375.0), "ligne 221");
    assert!(proche(r.cessions[1].plus_value, 675.0));
    let a = &r.annees[0];
    assert!(proche(a.resultat_net, 750.0) && a.case_2042c == "3AN" && !a.exonere);
}

#[test]
fn bofip_120_deux_ecritures() {
    let r = calculer(&[achat("2026-01-10", 500.0), cession("2026-03-10", 750.0, 750.0), achat("2026-03-10", 600.0), cession("2026-10-10", 800.0, 800.0)]);
    assert!(proche(r.cessions[0].plus_value, 250.0));
    assert!(proche(r.cessions[1].plus_value, 200.0));
    let r = calculer(&[
        achat("2026-01-10", 500.0), cession("2026-03-10", 750.0, 750.0), achat("2026-03-10", 750.0),
        op("2026-03-10", SoulteRecue, 150.0, None, None), cession("2026-10-10", 800.0, 800.0),
    ]);
    assert!(proche(r.cessions[1].soultes_anterieures, 150.0), "ligne 222");
    assert!(proche(r.cessions[1].plus_value, 200.0));
}

#[test]
fn bofip_50_frais_hors_quotient() {
    let r = calculer(&[achat("2026-01-10", 1000.0), op("2026-03-10", Cession, 450.0, Some(1200.0), Some(10.0))]);
    let c = &r.cessions[0];
    assert!(proche(c.fraction_imputee, 375.0));
    assert!(proche(c.plus_value, 65.0));
    assert!(proche(c.prix_net_frais, 440.0) && proche(c.prix_net_frais_soultes, 440.0) && proche(c.prix_net_soultes, 450.0));
}

#[test]
fn bofip_160_170_compensation_sans_report() {
    let r = calculer(&[
        achat("2026-01-01", 1000.0),
        cession("2026-02-01", 600.0, 2000.0), // 600 − 300 = 300
        cession("2026-03-01", 200.0, 400.0),  // 200 − 350 = −150
        cession("2027-01-01", 300.0, 300.0),  // 300 − 350 = −50, mais 300 € ≤ 305 € : exonéré
    ]);
    assert!(proche(r.annees[0].resultat_net, 150.0) && proche(r.annees[0].imposable, 150.0));
    assert!(proche(r.annees[1].resultat_net, -50.0) && r.annees[1].imposable == 0.0);
    assert!(r.annees[1].exonere && r.annees[1].case_2042c.is_empty());
    // Au-dessus du seuil, une moins-value nette se déclare en 3BN.
    let r = calculer(&[achat("2026-01-01", 1000.0), cession("2026-02-01", 600.0, 2000.0), cession("2026-03-01", 200.0, 400.0), cession("2027-01-01", 310.0, 310.0)]);
    assert!(proche(r.annees[1].resultat_net, -40.0) && r.annees[1].imposable == 0.0);
    assert_eq!(r.annees[1].case_2042c, "3BN");
}

#[test]
fn seuil_305_net_de_frais() {
    let r = calculer(&[achat("2026-01-01", 10.0), cession("2026-05-01", 305.0, 1000.0)]);
    assert!(r.annees[0].exonere && r.annees[0].imposable == 0.0 && r.annees[0].case_2042c.is_empty());
    let r = calculer(&[achat("2026-01-01", 10.0), cession("2026-05-01", 305.01, 1000.0)]);
    assert!(!r.annees[0].exonere && r.annees[0].imposable > 0.0);
    let r = calculer(&[achat("2026-01-01", 10.0), op("2026-05-01", Cession, 310.0, Some(1000.0), Some(5.0))]);
    assert!(proche(r.annees[0].total_prix_cession, 305.0), "ligne 51 = somme des lignes 218");
    assert!(r.annees[0].exonere);
}

#[test]
fn saisies_refusees() {
    let r = calculer(&[cession("2026-05-01", 500.0, 100.0)]);
    assert_eq!((r.erreurs.len(), r.cessions.len()), (1, 0));
    let r = calculer(&[achat("2026-13-01", 10.0), achat("26-01-01", 10.0), op("2026-01-01", Cession, 10.0, None, None)]);
    assert_eq!(r.erreurs.len(), 3);
    let r = calculer(&[achat("2026-01-01", f64::NAN)]);
    assert_eq!(r.erreurs.len(), 1);
}

#[test]
fn meme_jour_ordre_de_saisie() {
    // Une cession saisie avant l'achat du même jour ne voit pas cet achat.
    let r = calculer(&[achat("2026-01-01", 100.0), cession("2026-02-01", 50.0, 100.0), achat("2026-02-01", 1000.0)]);
    assert!(proche(r.cessions[0].prix_total_acquisition, 100.0));
}

#[test]
fn arrondi_euro() {
    assert_eq!((euro_entier(74.5), euro_entier(74.49), euro_entier(-0.5), euro_entier(-1.49)), (75, 74, -1, -1));
}

#[test]
fn csv_aller_retour() {
    let l = csv::lire("\u{feff}date;type;montant;valeur_portefeuille;frais;note\n2026-01-10;achat;1 000,00;;;Kraken\n2026-03-10;cession;450,00;1 200;0;\"BTC; partiel\"\n");
    assert!(l.erreurs.is_empty(), "{:?}", l.erreurs);
    assert_eq!(l.operations[1].note, "BTC; partiel");
    let r = calculer(&l.operations);
    assert!(proche(r.cessions[0].plus_value, 75.0));
    let l2 = csv::lire(&csv::ecrire_operations(&l.operations));
    assert_eq!(l.operations, l2.operations);
    let l3 = csv::lire("date,type,montant,valeur_portefeuille,frais\n2026-01-10,achat,1000,,\n2026-01-11,vente,5,,\n");
    assert_eq!((l3.operations.len(), l3.erreurs.len()), (1, 1));
    let rapport = csv::ecrire_2086(&r);
    assert!(rapport.contains("2026;2026-03-10;1200.00;450.00;0.00;450.00"));
    assert!(rapport.contains("2026;450.00;75.00;non;3AN;75"), "{rapport}");
}

#[test]
fn recoupement_reference_python() {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/reference.py");
    let out = match std::process::Command::new("python3").args(["-I", script, "500", "7"]).output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => { eprintln!("python3 indisponible : recoupement sauté"); return; }
    };
    let data: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let (mut n, mut ecart_max) = (0usize, 0f64);
    for (k, cas) in data["cas"].as_array().unwrap().iter().enumerate() {
        let ops: Vec<Operation> = cas["ops"].as_array().unwrap().iter().map(|o| {
            let f = |key: &str| o.get(key).and_then(|v| v.as_str()).and_then(csv::nombre);
            Operation {
                date: o["date"].as_str().unwrap().into(),
                kind: serde_json::from_value(o["type"].clone()).unwrap(),
                montant: f("montant").unwrap(),
                valeur_portefeuille: f("valeur_portefeuille"),
                frais: f("frais"),
                ..Default::default()
            }
        }).collect();
        let r = calculer(&ops);
        assert!(r.erreurs.is_empty(), "cas {k} : {:?}", r.erreurs);
        let attendu = cas["attendu"].as_array().unwrap();
        assert_eq!(r.cessions.len(), attendu.len(), "cas {k}");
        for (c, a) in r.cessions.iter().zip(attendu) {
            let a = a.as_f64().unwrap();
            ecart_max = ecart_max.max((c.plus_value - a).abs());
            assert!(proche(c.plus_value, a), "cas {k} : {} au lieu de {a}", c.plus_value);
            n += 1;
        }
    }
    eprintln!("recoupement Python : {n} cessions, écart maximal {ecart_max:.2e} €");
    assert!(n > 2000);
}
