//! Imports des plateformes, sur des fichiers fictifs construits d'après les en-têtes documentés : détection du
//! format, traduction en opérations, lignes « à vérifier », et calcul de bout en bout.

use taiseux_core::engine::{calculer_avec_cours, Cours, TypeOperation::*};
use taiseux_core::imports::importer;

fn proche(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * b.abs().max(1.0) }

#[test]
fn coinbase_v4_avec_introduction() {
    let f = "Transactions\nUser,Jean Dupont,abc\n\
ID,Timestamp,Transaction Type,Asset,Quantity Transacted,Price Currency,Price at Transaction,Subtotal,Total (inclusive of fees and/or spread),Fees and/or Spread,Notes\n\
1,2026-01-10T09:00:00Z,Buy,BTC,0.02,EUR,€50000.00,\"€1,000.00\",\"€1,015.00\",€15.00,Bought 0.02 BTC\n\
2,2026-02-01T10:00:00Z,Convert,BTC,-0.01,EUR,€52000.00,€520.00,€520.00,€0.00,Converted 0.01 BTC to 0.2 ETH\n\
3,2026-03-01T11:00:00Z,Sell,ETH,-0.1,EUR,€2800.00,€280.00,€276.00,€4.00,Sold 0.1 ETH\n\
4,2026-03-02T11:00:00Z,Staking Income,ETH,0.001,EUR,€2800.00,€2.80,€2.80,€0.00,\n\
5,2026-03-03T11:00:00Z,Send,BTC,-0.005,EUR,€55000.00,€275.00,€275.00,€0.00,To wallet\n\
6,2026-03-04T11:00:00Z,Deposit,EUR,100,EUR,€1.00,€100.00,€100.00,€0.00,\n\
7,2026-03-05T11:00:00Z,Sell,BTC,-0.001,USD,$60000.00,$60.00,$59.00,$1.00,Sold\n\
8,2026-03-06T11:00:00Z,Mystery Thing,BTC,0.001,EUR,€1,€1,€1,€0,\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Coinbase", "historique des transactions (v4)"));
    let o = &i.operations;
    assert_eq!(o.len(), 5, "{o:#?}");
    assert!(o[0].kind == Achat && o[0].actif.as_deref() == Some("BTC") && proche(o[0].quantite.unwrap(), 0.02) && proche(o[0].montant, 1015.0), "frais compris : {:?}", o[0]);
    assert!(o[1].kind == Echange && o[1].actif.as_deref() == Some("BTC") && o[1].actif_recu.as_deref() == Some("ETH") && proche(o[1].quantite_recue.unwrap(), 0.2));
    assert!(o[2].kind == Cession && proche(o[2].montant, 280.0) && proche(o[2].frais.unwrap(), 4.0) && proche(o[2].quantite.unwrap(), 0.1));
    assert!(o[3].kind == RevenuGratuit && o[4].kind == Transfert);
    assert_eq!(o[0].heure, "09:00:00");
    assert_eq!(i.ignorees, 1, "le dépôt d'euros est ignoré");
    assert_eq!(i.a_verifier.len(), 2, "vente en dollars et type inconnu : {:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("USD"));
}

#[test]
fn coinbase_v1_colonnes_prefixees() {
    let f = "Timestamp,Transaction Type,Asset,Quantity Transacted,EUR Spot Price at Transaction,EUR Subtotal,EUR Total (inclusive of fees),EUR Fees,Notes\n\
2021-05-01T10:00:00Z,Buy,ETH,1.5,2000,3000,3030,30,\n";
    let i = importer(f).unwrap();
    assert_eq!(i.format, "historique des transactions (v1)");
    assert!(i.operations[0].kind == Achat && proche(i.operations[0].montant, 3030.0));
}

#[test]
fn kraken_ledgers() {
    let f = "txid,refid,time,type,subtype,aclass,asset,wallet,amount,fee,balance\n\
L1,R1,2026-01-10 09:00:00.1234,trade,,currency,ZEUR,spot / main,-1000.0000,2.6000,0\n\
L2,R1,2026-01-10 09:00:00.1234,trade,,currency,XXBT,spot / main,0.0200000000,0,0.02\n\
L3,R2,2026-02-01 10:00:00,trade,,currency,XXBT,spot / main,-0.0100000000,0,0.01\n\
L4,R2,2026-02-01 10:00:00,trade,,currency,XETH,spot / main,0.2000000000,0.0004,0.1996\n\
L5,R3,2026-03-01 11:00:00,trade,,currency,XETH,spot / main,-0.1000000000,0,0.0996\n\
L6,R3,2026-03-01 11:00:00,trade,,currency,ZEUR,spot / main,280.0000,0.7300,279.27\n\
L7,R4,2026-03-02 11:00:00,staking,,currency,DOT.S,earn / bonded,0.5000000000,0,0.5\n\
L8,R5,2026-03-03 11:00:00,withdrawal,,currency,XXBT,spot / main,-0.0050000000,0.0001,0.0049\n\
L9,R6,2026-03-04 11:00:00,deposit,,currency,ZEUR,spot / main,500.0000,0,500\n\
L10,R7,2026-03-05 11:00:00,transfer,spottostaking,currency,DOT,spot / main,-0.5,0,0\n\
,R8,2026-03-06 11:00:00,withdrawal,,currency,XXBT,spot / main,-1,0,0\n\
L11,R9,2026-03-07 11:00:00,margin,,currency,ZEUR,spot / main,-3,0,0\n";
    let i = importer(f).unwrap();
    assert_eq!(i.plateforme, "Kraken");
    let o = &i.operations;
    assert_eq!(o.len(), 5, "{o:#?}");
    assert!(o[0].kind == Achat && o[0].actif.as_deref() == Some("BTC") && proche(o[0].montant, 1002.6) && proche(o[0].quantite.unwrap(), 0.02), "{:?}", o[0]);
    assert!(o[1].kind == Echange && o[1].actif_recu.as_deref() == Some("ETH") && proche(o[1].quantite_recue.unwrap(), 0.1996), "frais en ETH déduits : {:?}", o[1]);
    assert!(o[2].kind == Cession && proche(o[2].montant, 280.0) && proche(o[2].frais.unwrap(), 0.73));
    assert!(o[3].kind == RevenuGratuit && o[3].actif.as_deref() == Some("DOT"), "DOT.S → DOT : {:?}", o[3]);
    assert!(o[4].kind == Transfert && proche(o[4].quantite_frais.unwrap(), 0.0001));
    assert_eq!(i.ignorees, 3, "dépôt d'euros, transfert interne, opération échouée");
    assert_eq!(i.a_verifier.len(), 1, "marge : {:#?}", i.a_verifier);
}

#[test]
fn binance_releve() {
    let f = "User_ID,UTC_Time,Account,Operation,Coin,Change,Remark\n\
123,2026-01-10 09:00:00,Spot,Transaction Buy,BTC,0.02,\n\
123,2026-01-10 09:00:00,Spot,Transaction Spend,EUR,-1000,\n\
123,2026-01-10 09:00:01,Spot,Transaction Fee,BNB,-0.01,\n\
123,2026-02-01 10:00:00,Spot,Transaction Sold,BTC,-0.01,\n\
123,2026-02-01 10:00:00,Spot,Transaction Revenue,USDT,550,\n\
123,2026-02-01 10:00:00,Spot,Transaction Fee,USDT,-0.55,\n\
123,2026-03-01 11:00:00,Spot,Sell,BTC,-0.005,\n\
123,2026-03-01 11:00:00,Spot,Buy,EUR,280,\n\
123,2026-03-01 11:00:00,Spot,Fee,EUR,-0.28,\n\
123,2026-03-02 11:00:00,Earn,Simple Earn Flexible Interest,USDT,0.5,\n\
123,2026-03-03 11:00:00,Spot,Withdraw,BTC,-0.001,\n\
123,2026-03-04 11:00:00,Spot,Deposit,EUR,500,\n\
123,2026-03-05 11:00:00,Spot,Simple Earn Flexible Subscription,USDT,-100,\n\
123,2026-03-06 11:00:00,USDT-Futures,Realize profit and loss,USDT,12,\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Binance", "relevé des transactions"));
    let o = &i.operations;
    assert_eq!(o.len(), 6, "{o:#?}");
    assert!(o[0].kind == Achat && proche(o[0].montant, 1000.0) && proche(o[0].quantite.unwrap(), 0.02), "{:?}", o[0]);
    assert!(o[1].kind == Transfert && o[1].actif.as_deref() == Some("BNB") && proche(o[1].quantite_frais.unwrap(), 0.01), "frais en BNB : {:?}", o[1]);
    assert!(o[2].kind == Echange && o[2].actif_recu.as_deref() == Some("USDT") && proche(o[2].quantite_recue.unwrap(), 549.45), "{:?}", o[2]);
    assert!(o[3].kind == Cession && proche(o[3].montant, 280.0) && proche(o[3].frais.unwrap(), 0.28));
    assert!(o[4].kind == RevenuGratuit && o[5].kind == Transfert);
    assert_eq!(i.ignorees, 2);
    assert_eq!(i.a_verifier.len(), 1, "contrats à terme : {:#?}", i.a_verifier);
}

#[test]
fn binance_ordres_au_comptant() {
    let f = "Date(UTC),Market,Type,Price,Amount,Total,Fee,Fee Coin\n\
2026-01-10 09:00:00,BTCEUR,BUY,50000,0.02,1000,0.00002,BTC\n\
2026-02-01 10:00:00,ETHBTC,SELL,0.05,0.1,0.005,0.000005,BTC\n";
    let i = importer(f).unwrap();
    assert_eq!(i.format, "historique des ordres au comptant");
    let o = &i.operations;
    assert!(o[0].kind == Achat && proche(o[0].quantite.unwrap(), 0.01998) && proche(o[0].montant, 1000.0), "{:?}", o[0]);
    assert!(o[1].kind == Echange && o[1].actif.as_deref() == Some("ETH") && o[1].actif_recu.as_deref() == Some("BTC") && proche(o[1].quantite_recue.unwrap(), 0.004995));
}

#[test]
fn format_inconnu() {
    let e = importer("a,b,c\n1,2,3\n").unwrap_err();
    assert!(e.contains("Coinbase") && e.contains("Kraken") && e.contains("Binance"));
}

#[test]
fn de_bout_en_bout_kraken() {
    // Achat de 0,02 BTC (1 002,60 € frais compris), échange contre de l'ETH (sursis), vente de 0,1 ETH à 280 €.
    let f = "txid,refid,time,type,subtype,aclass,asset,amount,fee,balance\n\
L1,R1,2026-01-10 09:00:00,trade,,currency,ZEUR,-1000,2.6,0\n\
L2,R1,2026-01-10 09:00:00,trade,,currency,XXBT,0.02,0,0.02\n\
L3,R2,2026-02-01 10:00:00,trade,,currency,XXBT,-0.02,0,0\n\
L4,R2,2026-02-01 10:00:00,trade,,currency,XETH,0.4,0,0.4\n\
L5,R3,2026-03-01 11:00:00,trade,,currency,XETH,-0.1,0,0.3\n\
L6,R3,2026-03-01 11:00:00,trade,,currency,ZEUR,280,0.7,279.3\n";
    let i = importer(f).unwrap();
    let r = calculer_avec_cours(&i.operations, &[] as &[Cours]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    // Valeur globale au 01/03 : 0,4 ETH × 2 800 (cours déduit de la vente) = 1 120 € ;
    // plus-value = 280 − 0,7 − 1 002,6 × 280 / 1 120 = 28,65 €.
    assert!(proche(r.cessions[0].valeur_portefeuille, 1120.0));
    assert!(proche(r.cessions[0].plus_value, 280.0 - 0.7 - 1002.6 * 280.0 / 1120.0));
}
