//! Mesure de vitesse (développement) : un gros dossier synthétique, temps du calcul et du passage en JSON
//! (ce que coûte chaque recalcul demandé par l'interface). `cargo run --release -p taiseux-core --example mesure`
use std::time::Instant;
use taiseux_core::engine::{Cours, Operation, TypeOperation};
use taiseux_core::{api, imports};

fn main() {
    let actifs = ["BTC", "ETH", "SOL", "ADA", "DOT", "XRP", "LINK", "USDT"];
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let jours = 2_000usize;
    let date = |j: usize| { let (a, r) = (2020 + j / 336, j % 336); format!("{a}-{:02}-{:02}", r / 28 + 1, r % 28 + 1) };
    let mut ops = Vec::with_capacity(n);
    let mut x = 12345u64;
    let mut alea = || { x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (x >> 33) as usize };
    for i in 0..n {
        let j = i * jours / n;
        let a = actifs[alea() % actifs.len()];
        let kind = match alea() % 10 { 0..=5 => TypeOperation::Achat, 6..=7 => TypeOperation::Cession, 8 => TypeOperation::Echange, _ => TypeOperation::Transfert };
        let mut o = Operation { date: date(j), kind, actif: Some(a.into()), quantite: Some(0.01), plateforme: "Kraken".into(), ..Default::default() };
        match kind {
            TypeOperation::Achat => o.montant = 100.0,
            TypeOperation::Cession => o.montant = 1.0,
            TypeOperation::Echange => { o.actif_recu = Some(actifs[alea() % actifs.len()].into()); o.quantite_recue = Some(0.01); }
            _ => {}
        }
        ops.push(o);
    }
    let mut cours = Vec::new();
    for a in actifs { for j in 0..jours { cours.push(Cours { actif: a.into(), date: date(j), prix: 100.0 + j as f64 }); } }
    let t = Instant::now();
    let r = api::calculer_foyer(&[ops.clone()], &cours);
    let calcul = t.elapsed();
    let t = Instant::now();
    let js_ops = serde_json::to_string(&ops).unwrap();
    let js_cours = serde_json::to_string(&cours).unwrap();
    let _: Vec<Operation> = serde_json::from_str(&js_ops).unwrap();
    let _: Vec<Cours> = serde_json::from_str(&js_cours).unwrap();
    let js_res = serde_json::to_string(&r).unwrap();
    let json = t.elapsed();
    println!("{n} opérations, {} cours : {} cessions, {} erreurs", cours.len(), r.membres[0].cessions.len(), r.membres[0].erreurs.len());
    println!("calcul            : {:>8.1} ms", calcul.as_secs_f64() * 1e3);
    println!("JSON aller-retour : {:>8.1} ms  (opérations {} Ko, cours {} Ko, résultat {} Ko)", json.as_secs_f64() * 1e3, js_ops.len() / 1024, js_cours.len() / 1024, js_res.len() / 1024);
    // Import : un registre Kraken synthétique.
    let mut f = String::from("txid,refid,time,type,subtype,aclass,asset,wallet,amount,fee,balance\n");
    for i in 0..n / 2 {
        let d = date(i * jours / (n / 2));
        f += &format!("L{i}a,R{i},{d} 09:00:00,trade,,currency,ZEUR,spot / main,-100.0,0.26,0\nL{i}b,R{i},{d} 09:00:00,trade,,currency,XXBT,spot / main,0.002,0,0\n");
    }
    let t = Instant::now();
    let i = imports::importer(&f).unwrap();
    println!("import Kraken     : {:>8.1} ms  ({} lignes, {} opérations, {} Ko)", t.elapsed().as_secs_f64() * 1e3, n, i.operations.len(), f.len() / 1024);
}
