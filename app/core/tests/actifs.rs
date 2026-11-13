//! Suivi par actif (version 0.2) : valeur globale calculée à partir des cours, échanges avec et sans soulte,
//! transferts, revenus gratuits, cours manquants, foyer, fichiers de cours ; puis recoupement avec la référence
//! Python indépendante (`../tests/reference_actifs.py`, fractions exactes).

use taiseux_core::csv;
use taiseux_core::engine::{calculer, calculer_avec_cours, foyer, Cours, Operation, TypeOperation::*};

fn proche(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * b.abs().max(1.0) }
fn cours(a: &str, d: &str, p: f64) -> Cours { Cours { actif: a.into(), date: d.into(), prix: p } }
fn op(date: &str, kind: taiseux_core::TypeOperation) -> Operation { Operation { date: date.into(), kind, ..Default::default() } }
fn achat(d: &str, a: &str, q: f64, m: f64) -> Operation { Operation { actif: Some(a.into()), quantite: Some(q), montant: m, ..op(d, Achat) } }
fn vente(d: &str, a: &str, q: f64, m: f64) -> Operation { Operation { actif: Some(a.into()), quantite: Some(q), montant: m, ..op(d, Cession) } }

#[test]
fn valeur_globale_calculee_bofip_110() {
    // Le § 110 réécrit en quantités : 1 BTC acheté 1 000 € ; en mars il vaut 1 200 €, on en vend 0,375 pour 450 € ;
    // en août il vaut 2 080 € l'unité (0,625 × 2 080 = 1 300 €), on vend tout.
    let ops = [achat("2026-01-10", "btc", 1.0, 1000.0), vente("2026-03-10", "BTC", 0.375, 450.0), vente("2026-08-10", "BTC", 0.625, 1300.0)];
    let r = calculer_avec_cours(&ops, &[cours("BTC", "2026-03-10", 1200.0), cours("BTC", "2026-08-10", 2080.0)]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    assert!(r.cessions[0].valeur_calculee && proche(r.cessions[0].valeur_portefeuille, 1200.0));
    assert!(proche(r.cessions[0].plus_value, 75.0));
    assert!(proche(r.cessions[1].valeur_portefeuille, 1300.0));
    assert!(proche(r.cessions[1].plus_value, 675.0));
    assert!(r.avoirs.is_empty(), "tout a été vendu : {:?}", r.avoirs);
}

#[test]
fn cours_deduit_du_prix_de_vente() {
    // Sans fichier de cours : le prix unitaire de la vente du jour sert de cours pour l'actif vendu.
    let r = calculer(&[achat("2026-01-10", "BTC", 1.0, 1000.0), vente("2026-03-10", "BTC", 0.5, 600.0)]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    assert!(proche(r.cessions[0].valeur_portefeuille, 1200.0));
}

#[test]
fn cours_manquants_signales() {
    let ops = [achat("2026-01-10", "BTC", 1.0, 1000.0), achat("2026-01-11", "ETH", 10.0, 3000.0), vente("2026-03-10", "BTC", 0.5, 600.0)];
    let r = calculer(&ops);
    assert_eq!(r.cessions.len(), 0);
    assert_eq!(r.cours_manquants.len(), 1);
    assert_eq!((r.cours_manquants[0].actif.as_str(), r.cours_manquants[0].date.as_str()), ("ETH", "2026-03-10"));
    // Avec le cours de l'ETH : avoirs AVANT la vente = 1 BTC × 1 200 + 10 ETH × 350 = 4 700 €.
    let r = calculer_avec_cours(&ops, &[cours("ETH", "2026-03-10", 350.0)]);
    assert!(r.erreurs.is_empty() && proche(r.cessions[0].valeur_portefeuille, 4700.0));
    assert!(proche(r.cessions[0].plus_value, 600.0 - 4000.0 * 600.0 / 4700.0));
}

#[test]
fn valeur_saisie_prime_et_operations_0_1_exigent_la_saisie() {
    let ops = [achat("2026-01-10", "BTC", 1.0, 1000.0), Operation { valeur_portefeuille: Some(1500.0), ..vente("2026-03-10", "BTC", 0.5, 600.0) }];
    let r = calculer(&ops);
    assert!(!r.cessions[0].valeur_calculee && proche(r.cessions[0].valeur_portefeuille, 1500.0));
    // Un achat sans actif : le portefeuille n'est plus connu en entier.
    let mut ops2 = vec![Operation { montant: 500.0, ..op("2026-01-05", Achat) }];
    ops2.extend_from_slice(&ops[..1]);
    ops2.push(vente("2026-03-10", "BTC", 0.5, 600.0));
    let r = calculer(&ops2);
    assert_eq!(r.cessions.len(), 0);
    assert!(r.erreurs[0].contains("à saisir"));
}

#[test]
fn echange_sans_soulte_sursis() {
    let ops = [
        achat("2026-01-10", "BTC", 1.0, 1000.0),
        Operation { actif: Some("BTC".into()), quantite: Some(1.0), actif_recu: Some("ETH".into()), quantite_recue: Some(4.0), ..op("2026-02-01", Echange) },
        vente("2026-03-10", "ETH", 2.0, 700.0),
    ];
    let r = calculer(&ops);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    assert_eq!(r.cessions.len(), 1, "l'échange sans soulte n'est pas une cession");
    // Valeur globale : 4 ETH × 350 = 1 400 ; plus-value = 700 − 1 000 × 700 / 1 400 = 200.
    assert!(proche(r.cessions[0].valeur_portefeuille, 1400.0) && proche(r.cessions[0].plus_value, 200.0));
    assert!(proche(r.avoirs["ETH"], 2.0) && !r.avoirs.contains_key("BTC"));
}

#[test]
fn echange_avec_soulte_bofip_120() {
    // § 120 : actifs achetés 500 € ; en mars, tout le portefeuille (750 €) est échangé contre des actifs valant 600 €
    // et une soulte reçue de 150 € ; en octobre, les actifs reçus sont vendus 800 €.
    let ops = [
        achat("2026-01-10", "AAA", 1.0, 500.0),
        Operation { actif: Some("AAA".into()), quantite: Some(1.0), actif_recu: Some("BBB".into()), quantite_recue: Some(6.0),
                    valeur_recue: Some(600.0), soulte: Some(150.0), ..op("2026-03-10", Echange) },
        vente("2026-10-10", "BBB", 6.0, 800.0),
    ];
    let r = calculer_avec_cours(&ops, &[cours("AAA", "2026-03-10", 750.0)]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    let (c1, c2) = (&r.cessions[0], &r.cessions[1]);
    assert!(proche(c1.prix_cession, 600.0) && proche(c1.soulte_cession, 150.0) && proche(c1.prix_net_soultes, 750.0), "lignes 213, 216, 217");
    assert!(proche(c1.plus_value, 250.0));
    assert!(proche(c2.prix_total_acquisition, 1100.0) && proche(c2.pta_net, 600.0));
    assert!(proche(c2.plus_value, 200.0));
}

#[test]
fn transfert_et_revenu_gratuit() {
    let ops = [
        achat("2026-01-10", "BTC", 1.0, 1000.0),
        Operation { actif: Some("BTC".into()), quantite: Some(1.0), quantite_frais: Some(0.01), ..op("2026-01-20", Transfert) },
        Operation { actif: Some("XYZ".into()), quantite: Some(100.0), ..op("2026-02-01", RevenuGratuit) },
        vente("2026-03-10", "BTC", 0.49, 588.0),
    ];
    let r = calculer_avec_cours(&ops, &[cours("XYZ", "2026-03-10", 1.0)]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    // Avoirs avant la vente : 0,99 BTC × 1 200 + 100 XYZ × 1 = 1 288 ; prix total d'acquisition inchangé (1 000).
    assert!(proche(r.cessions[0].valeur_portefeuille, 1288.0));
    assert!(proche(r.cessions[0].prix_total_acquisition, 1000.0));
    assert!(proche(r.avoirs["BTC"], 0.5));
}

#[test]
fn vendre_plus_que_detenu_avertit() {
    let r = calculer_avec_cours(&[achat("2026-01-10", "BTC", 1.0, 1000.0), Operation { valeur_portefeuille: Some(3000.0), ..vente("2026-03-10", "BTC", 2.0, 2400.0) }], &[]);
    assert!(r.avertissements.iter().any(|a| a.contains("sortent alors que")), "{:?}", r.avertissements);
}

#[test]
fn foyer_seuil_commun() {
    // Deux déclarants à 200 € de cessions chacun : chacun seul serait exonéré, le foyer (400 €) ne l'est pas.
    let un = calculer(&[achat("2026-01-10", "BTC", 1.0, 100.0), vente("2026-03-10", "BTC", 0.5, 200.0)]);
    let deux = calculer(&[achat("2026-01-10", "ETH", 1.0, 100.0), vente("2026-03-10", "ETH", 0.5, 200.0)]);
    assert!(un.annees[0].exonere && deux.annees[0].exonere);
    let f = foyer(&[un, deux]);
    assert!(proche(f[0].total_prix_cession, 400.0) && !f[0].exonere);
    assert!(proche(f[0].resultat_net, 300.0) && f[0].case_2042c == "3AN");
    assert_eq!(f[0].par_membre.len(), 2);
}

#[test]
fn fichiers_de_cours() {
    let (c, e) = csv::lire_cours("snapped_at,price,market_cap,total_volume\n2026-03-10 00:00:00 UTC,61234.5,1,2\n2026-03-11 00:00:00 UTC,62000,1,2\n", Some("btc"));
    assert!(e.is_empty(), "{e:?}");
    assert_eq!((c.len(), c[0].actif.as_str(), c[0].date.as_str(), c[0].prix), (2, "BTC", "2026-03-10", 61234.5));
    let (c, e) = csv::lire_cours("date;actif;prix\n2026-03-10;eth;3 012,50\n", None);
    assert!(e.is_empty() && c[0].actif == "ETH" && c[0].prix == 3012.5);
    // Kraken : 1773100800 = 2026-03-10 00:00:00 UTC ; on garde la clôture (5e colonne).
    let (c, e) = csv::lire_cours("1773100800,60000,62000,59000,61500,12.3,456\n", Some("BTC"));
    assert!(e.is_empty(), "{e:?}");
    assert_eq!((c[0].date.as_str(), c[0].prix), ("2026-03-10", 61500.0));
    assert!(!csv::lire_cours("snapped_at,price\n2026-03-10,1\n", None).1.is_empty(), "CoinGecko sans actif : erreur");
}

#[test]
fn csv_0_2_aller_retour() {
    let ops = vec![
        achat("2026-01-10", "BTC", 1.0, 1000.0),
        Operation { actif: Some("BTC".into()), quantite: Some(0.5), actif_recu: Some("ETH".into()), quantite_recue: Some(4.0),
                    valeur_recue: Some(500.0), soulte: Some(-20.0), plateforme: "Kraken".into(), ..op("2026-02-01", Echange) },
    ];
    let l = csv::lire(&csv::ecrire_operations(&ops));
    assert!(l.erreurs.is_empty(), "{:?}", l.erreurs);
    assert_eq!(l.operations, ops);
}

#[test]
fn recoupement_reference_python_actifs() {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/reference_actifs.py");
    let out = match std::process::Command::new("python3").args(["-I", script, "300", "11"]).output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => { eprintln!("python3 indisponible : recoupement sauté"); return; }
    };
    let data: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let (mut n, mut ecart_max) = (0usize, 0f64);
    for (k, cas) in data["cas"].as_array().unwrap().iter().enumerate() {
        let f = |o: &serde_json::Value, key: &str| o.get(key).and_then(|v| v.as_str()).and_then(csv::nombre);
        let t = |o: &serde_json::Value, key: &str| o.get(key).and_then(|v| v.as_str()).map(String::from);
        let ops: Vec<Operation> = cas["ops"].as_array().unwrap().iter().map(|o| Operation {
            date: o["date"].as_str().unwrap().into(),
            kind: serde_json::from_value(o["type"].clone()).unwrap(),
            montant: f(o, "montant").unwrap_or(0.0),
            frais: f(o, "frais"), actif: t(o, "actif"), quantite: f(o, "quantite"), actif_recu: t(o, "actif_recu"),
            quantite_recue: f(o, "quantite_recue"), valeur_recue: f(o, "valeur_recue"), soulte: f(o, "soulte"),
            quantite_frais: f(o, "quantite_frais"), ..Default::default()
        }).collect();
        let cours: Vec<Cours> = cas["cours"].as_array().unwrap().iter()
            .map(|c| Cours { actif: c["actif"].as_str().unwrap().into(), date: c["date"].as_str().unwrap().into(), prix: csv::nombre(c["prix"].as_str().unwrap()).unwrap() })
            .collect();
        let r = calculer_avec_cours(&ops, &cours);
        assert!(r.erreurs.is_empty(), "cas {k} : {:?}", r.erreurs);
        let attendu = cas["attendu"].as_array().unwrap();
        assert_eq!(r.cessions.len(), attendu.len(), "cas {k}");
        for (c, a) in r.cessions.iter().zip(attendu) {
            let a = a.as_f64().unwrap();
            ecart_max = ecart_max.max((c.plus_value - a).abs());
            assert!((c.plus_value - a).abs() <= 1e-6 * a.abs().max(1.0), "cas {k} : {} au lieu de {a}", c.plus_value);
            n += 1;
        }
    }
    eprintln!("recoupement Python (actifs) : {n} cessions, écart maximal {ecart_max:.2e} €");
    assert!(n > 1000);
}
