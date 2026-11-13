//! Imports des portefeuilles personnels (lot 4) : Ledger Live, Trezor Suite, Electrum, Exodus, Sparrow, Coinomi.
//! Fichiers fictifs construits d'après les en-têtes documentés ; résultats attendus calculés à la main.

use taiseux_core::engine::{calculer_avec_cours, Cours, Operation, TypeOperation::*};
use taiseux_core::imports::{importer, Import};

fn proche(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * b.abs().max(1.0) }

/// Transfert attendu : actif, quantité, frais de réseau.
fn transfert(o: &Operation, a: &str, q: f64, frais: f64) -> bool {
    o.kind == Transfert && o.actif.as_deref() == Some(a) && proche(o.quantite.unwrap(), q) && proche(o.quantite_frais.unwrap_or(0.0), frais)
}
fn motifs(i: &Import) -> String { i.a_verifier.iter().map(|v| format!("[{}] {}", v.ligne, v.motif)).collect::<Vec<_>>().join("\n") }

// ---------------------------------------------------------------- Ledger Live

#[test]
fn ledger_live_avec_statut() {
    let f = "Operation Date,Status,Currency Ticker,Operation Type,Operation Amount,Operation Fees,Operation Hash,Account Name,Account xpub,Countervalue Ticker,Countervalue at Operation Date,Countervalue at CSV Export\n\
2026-01-10T09:00:00.000Z,Confirmed,BTC,IN,0.05,0.00001,h1,Bitcoin 1,xpub1,EUR,2500,3000\n\
2026-02-01T10:00:00.000Z,Confirmed,BTC,OUT,0.02001,0.00001,h2,Bitcoin 1,xpub1,EUR,1000,1200\n\
2026-02-02T10:00:00.000Z,Confirmed,ETH,FEES,0.0005,0.0005,h3,Ethereum 1,xpub2,EUR,1,1\n\
2026-02-03T10:00:00.000Z,Confirmed,DOT,REWARD,1.5,,h4,Polkadot 1,xpub3,EUR,9,9\n\
2026-02-04T10:00:00.000Z,Confirmed,ATOM,DELEGATE,10,0.002,h5,Cosmos 1,xpub4,EUR,80,80\n\
2026-02-05T10:00:00.000Z,Failed,ETH,OUT,0.1,0.001,h6,Ethereum 1,xpub2,EUR,250,250\n\
2026-02-06T10:00:00.000Z,Confirmed,USDT,IN,100,,h7,Tether USD,xpub2,EUR,92,92\n\
2026-02-07T10:00:00.000Z,Confirmed,ETH,NFT_IN,0,,h8,Ethereum 1,xpub2,EUR,0,0\n\
2026-02-08T10:00:00.000Z,Confirmed,BTC,WEIRD,1,,h9,Bitcoin 1,xpub1,EUR,0,0\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Ledger Live", "historique des opérations (avec statut)"));
    let o = &i.operations;
    assert_eq!(o.len(), 5, "{o:#?}");
    // Réception : les frais (payés par l'expéditeur) sont ignorés ; la note signale la supposition.
    assert!(transfert(&o[0], "BTC", 0.05, 0.0), "{:?}", o[0]);
    assert!(o[0].note.contains("supposée venir de l'un de vos comptes") && o[0].note.contains("Bitcoin 1"), "{}", o[0].note);
    assert_eq!((o[0].date.as_str(), o[0].heure.as_str(), o[0].plateforme.as_str()), ("2026-01-10", "09:00:00", "Ledger Live"));
    // Envoi : 0,02001 frais compris → 0,02 envoyés + 0,00001 de frais.
    assert!(transfert(&o[1], "BTC", 0.02, 0.00001), "{:?}", o[1]);
    assert!(o[1].note.contains("cession imposable"));
    assert!(transfert(&o[2], "ETH", 0.0, 0.0005), "{:?}", o[2]);
    assert!(o[3].kind == RevenuGratuit && o[3].actif.as_deref() == Some("DOT") && proche(o[3].quantite.unwrap(), 1.5));
    assert!(transfert(&o[4], "USDT", 100.0, 0.0));
    assert_eq!(i.a_verifier.len(), 4, "{}", motifs(&i));
    let m = motifs(&i);
    assert!(m.contains("DELEGATE") && m.contains("Failed") && m.contains("NFT") && m.contains("WEIRD"), "{m}");
    assert_eq!(i.a_verifier[0].ligne, 6, "numéro de ligne dans le fichier, en-tête compris");
}

#[test]
fn ledger_live_variantes_d_en_tete() {
    let sans_cv = "Operation Date,Currency Ticker,Operation Type,Operation Amount,Operation Fees,Operation Hash,Account Name,Account xpub\n\
2026-03-01T08:00:00.000Z,ETH,OUT,1.01,0.01,h1,Ethereum 1,xpub\n";
    let i = importer(sans_cv).unwrap();
    assert_eq!(i.format, "historique des opérations (sans contre-valeur)");
    assert!(transfert(&i.operations[0], "ETH", 1.0, 0.01), "{:?}", i.operations[0]);

    let ancien = "Operation Date,Currency Ticker,Operation Type,Operation Amount,Operation Fees,Operation Hash,Account Name,Account id\n\
2026-03-01T08:00:00.000Z,XRP,IN,25,,h1,XRP 1,id1\n";
    let i = importer(ancien).unwrap();
    assert!(i.format.contains("Account id"));
    assert!(transfert(&i.operations[0], "XRP", 25.0, 0.0));

    let sans_statut = "Operation Date,Currency Ticker,Operation Type,Operation Amount,Operation Fees,Operation Hash,Account Name,Account xpub,Countervalue Ticker,Countervalue at Operation Date,Countervalue at CSV Export\n\
2026-03-01T08:00:00.000Z,BTC,IN,0.1,,h1,Bitcoin 1,xpub,EUR,5000,6000\n";
    let i = importer(sans_statut).unwrap();
    assert_eq!(i.format, "historique des opérations");
    assert_eq!(i.operations.len(), 1);
}

// ---------------------------------------------------------------- Trezor Suite

#[test]
fn trezor_suite() {
    let f = "Timestamp,Date,Time,Type,Transaction ID,Fee,Fee unit,Address,Label,Amount,Amount unit,Fiat (EUR),Other\n\
1768035600,10/01/2026,\"10:00:00 GMT+1\",RECV,t1,,,bc1moi,,0.05,BTC,2500,\n\
1769940000,01/02/2026,11:00:00,SENT,t2,0.00002,BTC,bc1x,Ami,0.01,BTC,500,\n\
1769940000,01/02/2026,11:00:00,SENT,t2,,,bc1y,,0.02,BTC,1000,\n\
1770026400,02/02/2026,11:00:00,SENT,t3,0.001,ETH,0xabc,,50,USDT,46,\n\
1770112800,03/02/2026,11:00:00,SELF,t4,0.00001,BTC,bc1moi2,,0.5,BTC,25000,\n\
1770199200,04/02/2026,11:00:00,FAILED,t5,0.002,ETH,0xdef,,0.3,ETH,750,\n\
1770199200,04/02/2026,11:00:00,JOINT,t6,0.0001,BTC,bc1z,,0.1,BTC,5000,\n\
1770199200,04/02/2026,11:00:00,SENT,t7,0.001,ETH,0x1,,ID 42,,,\n\
1770199200,04/02/2026,11:00:00,SENT,t8,0.001,ETH,0x2,,10,0x1234567890abcdef1234,,\n\
1709251199,29/02/2024,23:59:59,RECV,t9,,,bc1moi,,100000,sats,40,\n\
1770199200,04/02/2026,11:00:00,SENT,t10,,,,,,,,OP_RETURN 6a\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Trezor Suite", "export des transactions"));
    let o = &i.operations;
    assert_eq!(o.len(), 8, "{o:#?}");
    // Tri par date : la réception de 2024 (100 000 sats) vient en premier.
    assert!(transfert(&o[0], "BTC", 0.001, 0.0) && o[0].date == "2024-02-29" && o[0].heure == "23:59:59", "{:?}", o[0]);
    assert!(transfert(&o[1], "BTC", 0.05, 0.0) && o[1].date == "2026-01-10" && o[1].heure == "09:00:00", "date UTC tirée de Timestamp : {:?}", o[1]);
    assert!(o[1].note.contains("supposée venir"));
    // Envoi à deux destinataires : les frais n'apparaissent qu'une fois.
    assert!(transfert(&o[2], "BTC", 0.01, 0.00002) && o[2].note.contains("Ami"), "{:?}", o[2]);
    assert!(transfert(&o[3], "BTC", 0.02, 0.0));
    // Envoi d'un jeton : frais payés en ETH, transfert à part.
    assert!(transfert(&o[4], "USDT", 50.0, 0.0) && transfert(&o[5], "ETH", 0.0, 0.001), "{:?} {:?}", o[4], o[5]);
    assert!(transfert(&o[6], "BTC", 0.5, 0.00001) && o[6].note.contains("vous-même"));
    assert!(transfert(&o[7], "ETH", 0.0, 0.002) && o[7].note.contains("échouée"), "seuls les frais d'un échec : {:?}", o[7]);
    assert_eq!(i.ignorees, 1, "sortie OP_RETURN sans montant ni frais");
    let m = motifs(&i);
    assert_eq!(i.a_verifier.len(), 3, "{m}");
    assert!(m.contains("coinjoin") && m.contains("NFT") && m.contains("jeton illisible"), "{m}");
}

#[test]
fn trezor_anciens_exports_sans_actif() {
    let suite_v1 = "Date & Time,Type,Transaction ID,Addresses,Fee,Total\n\
01/02/2021 10:00,RECV,t1,bc1,,0.1\n\
02/02/2021 10:00,SENT,t2,bc1x,0.0001,0.05\n";
    let i = importer(suite_v1).unwrap();
    assert_eq!(i.plateforme, "Trezor Suite");
    assert!(i.operations.is_empty() && i.a_verifier.len() == 2);
    assert!(i.a_verifier[0].motif.contains("n'indique pas l'actif"));

    let web = "Date,Time,TX id,Address,Address Label,TX type,Value,TX total,Balance\n\
2020-01-10,10:00:00,t1,bc1,,IN,0.1,0.1,0.1\n";
    let i = importer(web).unwrap();
    assert!(i.format.contains("ancien portefeuille web") && i.operations.is_empty() && i.a_verifier.len() == 1);
    assert_eq!(i.a_verifier[0].date, "2020-01-10");
}

// ---------------------------------------------------------------- Electrum

#[test]
fn electrum() {
    let f = "transaction_hash,label,confirmations,value,fiat_value,fee,fiat_fee,timestamp\n\
t1,Retrait plateforme,100,0.05,2500,,,2026-01-10 09:00\n\
t2,,90,-0.02003,-1000,0.00003,1.5,2026-02-01 10:00\n\
t3,,80,-0.00001,-0.5,0.00001,0.5,2026-02-02 10:00\n\
t4,,0,0.1,,,,\n\
t5,,10,0,0,,,2026-02-03 10:00\n\
t6,,10,-0.01,,,,2026-02-04 10:00\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Electrum", "historique du portefeuille (BTC)"));
    let o = &i.operations;
    assert_eq!(o.len(), 4, "{o:#?}");
    assert!(transfert(&o[0], "BTC", 0.05, 0.0) && o[0].note.contains("Retrait plateforme") && o[0].note.contains("Actif supposé : BTC"), "{:?}", o[0]);
    // Envoi : valeur nette frais compris → 0,02 + 0,00003.
    assert!(transfert(&o[1], "BTC", 0.02, 0.00003), "{:?}", o[1]);
    // Sortie réduite aux frais : consolidation entre ses adresses.
    assert!(transfert(&o[2], "BTC", 0.0, 0.00001) && o[2].note.contains("consolidation"));
    // Envoi sans frais connus : signalé.
    assert!(transfert(&o[3], "BTC", 0.01, 0.0) && o[3].note.contains("Frais non indiqués"));
    assert_eq!(i.ignorees, 1, "variation nulle");
    assert_eq!(i.a_verifier.len(), 1, "transaction sans date : {}", motifs(&i));

    let ancien = "transaction_hash,label,value,timestamp\nt1,,-0.5,2019-05-01 12:00\n";
    let i = importer(ancien).unwrap();
    assert!(i.format.contains("ancien format"));
    assert!(transfert(&i.operations[0], "BTC", 0.5, 0.0));
}

// ---------------------------------------------------------------- Exodus

#[test]
fn exodus_ancien_format() {
    let f = "DATE,TYPE,FROMPORTFOLIO,TOPORTFOLIO,OUTAMOUNT,OUTCURRENCY,FEEAMOUNT,FEECURRENCY,TOADDRESS,OUTTXID,OUTTXURL,INAMOUNT,INCURRENCY,INTXID,INTXURL,ORDERID,PERSONALNOTE\n\
Sat Jan 10 2026 10:00:00 GMT+0100 (Central European Standard Time),deposit,,,,,,,,,,0.05,BTC,i1,,,\n\
2026-02-01T10:00:00.000Z,exchange,,,-0.01,BTC,0.00002,BTC,,o2,,0.2,ETH,i2,,ord2,\n\
2026-02-02T10:00:00.000Z,withdrawal,,,-0.1,ETH,0.001,ETH,0xdest,o3,,,,,,,Cadeau\n\
2026-02-03T10:00:00.000Z,withdrawal (failed),,,-0.1,ETH,0.0005,ETH,0xdest,o4,,,,,,,\n\
2026-02-04T10:00:00.000Z,mystery,,,,,,,,,,,,,,,\n\
2026-02-05T10:00:00.000Z,exchange,,,-100,EUR,,,,o6,,0.001,BTC,i6,,ord6,\n\
,,,,,,,,,,,,,,,,\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Exodus", "export des transactions (ancien format)"));
    let o = &i.operations;
    assert_eq!(o.len(), 5, "{o:#?}");
    assert!(transfert(&o[0], "BTC", 0.05, 0.0) && o[0].date == "2026-01-10" && o[0].heure == "10:00:00", "date JavaScript : {:?}", o[0]);
    assert!(o[1].kind == Echange && o[1].actif.as_deref() == Some("BTC") && proche(o[1].quantite.unwrap(), 0.01)
        && o[1].actif_recu.as_deref() == Some("ETH") && proche(o[1].quantite_recue.unwrap(), 0.2), "{:?}", o[1]);
    assert!(transfert(&o[2], "BTC", 0.0, 0.00002), "frais de l'échange : {:?}", o[2]);
    assert!(transfert(&o[3], "ETH", 0.1, 0.001) && o[3].note.contains("Cadeau"));
    assert!(transfert(&o[4], "ETH", 0.0, 0.0005));
    assert_eq!(i.a_verifier.len(), 2, "{}", motifs(&i));
    assert!(motifs(&i).contains("mystery") && motifs(&i).contains("monnaie officielle"));

    // Variante avec FROMADDRESS.
    let g = "DATE,TYPE,FROMPORTFOLIO,TOPORTFOLIO,OUTAMOUNT,OUTCURRENCY,FEEAMOUNT,FEECURRENCY,FROMADDRESS,TOADDRESS,OUTTXID,OUTTXURL,INAMOUNT,INCURRENCY,INTXID,INTXURL,ORDERID,PERSONALNOTE\n\
2026-02-01T10:00:00.000Z,withdrawal,,,-0.3,SOL,0.000005,SOL,a,b,o1,,,,,,,\n";
    let i = importer(g).unwrap();
    assert!(transfert(&i.operations[0], "SOL", 0.3, 0.000005));
}

#[test]
fn exodus_format_actuel() {
    let f = "TXID,TXURL,DATE,TYPE,FROMPORTFOLIO,TOPORTFOLIO,COINAMOUNT,FEE,BALANCE,EXCHANGE,PERSONALNOTE\n\
t1,,2026-01-10T09:00:00.000Z,deposit,,,0.05 BTC,,0.05 BTC,,\n\
t2,,2026-02-01T10:00:00.000Z,withdrawal,,,-0.01 BTC,0.00002 BTC,0.03998 BTC,X1,\n\
t3,,2026-02-01T10:05:00.000Z,deposit,,,0.2 ETH,,0.2 ETH,X1,\n\
t4,,2026-02-02T10:00:00.000Z,withdrawal,,,-50 USDT,0.001 ETH,0 USDT,,\n\
t5,,2026-02-03T10:00:00.000Z,deposit,,,0.1 ETH,,0.3 ETH,X2,\n\
t6,,2026-02-04T10:00:00.000Z,withdrawal (failed),,,-0.1 ETH,0.0003 ETH,,,\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Exodus", "export des transactions"));
    let o = &i.operations;
    assert_eq!(o.len(), 6, "{o:#?}");
    assert!(transfert(&o[0], "BTC", 0.05, 0.0));
    // Retrait et dépôt de même EXCHANGE : un échange, à l'heure du retrait, frais à part.
    assert!(o[1].kind == Echange && o[1].actif.as_deref() == Some("BTC") && proche(o[1].quantite.unwrap(), 0.01)
        && o[1].actif_recu.as_deref() == Some("ETH") && proche(o[1].quantite_recue.unwrap(), 0.2) && o[1].heure == "10:00:00", "{:?}", o[1]);
    assert!(transfert(&o[2], "BTC", 0.0, 0.00002));
    assert!(transfert(&o[3], "USDT", 50.0, 0.0) && transfert(&o[4], "ETH", 0.0, 0.001), "{:?} {:?}", o[3], o[4]);
    assert!(transfert(&o[5], "ETH", 0.0, 0.0003));
    assert_eq!(i.a_verifier.len(), 1, "dépôt X2 sans retrait : {}", motifs(&i));
    assert!(i.a_verifier[0].motif.contains("X2"));
}

#[test]
fn exodus_staking() {
    let f = "Type,Buy,Cur.,Exchange,Group,Comment,Date\n\
Staking,0.5,SOL,Exodus,,reward,2026-03-01 00:00:00\n\
Staking,0.1,ADA,Exodus,,,02.03.2026 12:00:00\n\
Deposit,1,ADA,Exodus,,,2026-03-03 00:00:00\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Exodus", "récompenses de staking"));
    let o = &i.operations;
    assert!(o.len() == 2 && o.iter().all(|o| o.kind == RevenuGratuit), "{o:#?}");
    assert!(o[0].actif.as_deref() == Some("SOL") && proche(o[0].quantite.unwrap(), 0.5));
    assert!(o[1].actif.as_deref() == Some("ADA") && o[1].date == "2026-03-02" && o[1].heure == "12:00:00");
    assert_eq!(i.a_verifier.len(), 1);
}

// ---------------------------------------------------------------- Sparrow

#[test]
fn sparrow_btc_avec_cours() {
    let f = "Date (UTC),Label,Value (BTC),Balance (BTC),Fee (BTC),Value (EUR),Txid\n\
2026-01-10 09:00:00,Retrait plateforme,0.05000000,0.05000000,,2500.00,tx1\n\
2026-02-01 10:00:00,,-0.02003000,0.02997000,0.00003000,-1000.00,tx2\n\
2026-02-02 10:00:00,Minage,0.00100000,0.03097000,0.00000000,50.00,tx3\n\
2026-02-03 10:00:00,Coinjoin,-0.01000000,0.02097000,,-500.00,tx4\n\
Unconfirmed,,0.00100000,0.02197000,,50.00,tx5\n\
\n\
# Historical EUR values are taken from daily rates and should only be considered as approximate.\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Sparrow", "export des transactions (montants en BTC)"));
    let o = &i.operations;
    assert_eq!(o.len(), 3, "{o:#?}");
    assert!(transfert(&o[0], "BTC", 0.05, 0.0) && o[0].note.contains("Retrait plateforme") && o[0].note.contains("supposée venir"));
    assert!(transfert(&o[1], "BTC", 0.02, 0.00003), "{:?}", o[1]);
    assert!(transfert(&o[2], "BTC", 0.01, 0.0) && o[2].note.contains("coinjoin"));
    let m = motifs(&i);
    assert_eq!(i.a_verifier.len(), 2, "{m}");
    assert!(m.contains("minage") && m.contains("non confirmée"), "{m}");
    assert_eq!(i.ignorees, 0, "la ligne de commentaire n'est pas une opération");
}

#[test]
fn sparrow_sats_et_virgule_decimale() {
    let sats = "Date (UTC),Label,Value (sats),Balance (sats),Fee (sats),Txid\n\
2026-02-01 10:00:00,,-2003000,2997000,3000,tx2\n\
2026-02-02 10:00:00,,-1000,2996000,1000,tx3\n";
    let i = importer(sats).unwrap();
    assert_eq!(i.format, "export des transactions (montants en sats)");
    assert!(transfert(&i.operations[0], "BTC", 0.02, 0.00003), "{:?}", i.operations[0]);
    assert!(transfert(&i.operations[1], "BTC", 0.0, 0.00001) && i.operations[1].note.contains("consolidation"));

    // Réglage « virgule décimale » de Sparrow : les montants sont entre guillemets.
    let virgule = "Date (UTC),Label,Value (BTC),Balance (BTC),Fee (BTC),Txid\n\
2026-02-01 10:00:00,,\"-0,02003000\",\"0,02997000\",\"0,00003000\",tx2\n";
    let i = importer(virgule).unwrap();
    assert!(transfert(&i.operations[0], "BTC", 0.02, 0.00003), "{:?}", i.operations[0]);
}

// ---------------------------------------------------------------- Coinomi

#[test]
fn coinomi() {
    let f = "Asset,AccountName,Address,AddressName,Value,Symbol,Fees,InternalTransfer,TransactionID,Time(UTC),Time(ISO8601-UTC),BlockExplorer\n\
Bitcoin,Mon BTC,bc1a,,0.05,BTC,0,false,t1,2026-01-10 09:00:00,2026-01-10T09:00:00Z,https://exemple/t1\n\
Bitcoin,Mon BTC,bc1b,Fournisseur,-0.02003,BTC,0.00003,false,t2,2026-02-01 10:00:00,2026-02-01T10:00:00Z,\n\
Ethereum,Mon ETH,0xa,,-0.5,ETH,0.001,true,t3,2026-02-02 10:00:00,2026-02-02T10:00:00Z,\n\
Ethereum,Épargne ETH,0xb,,0.499,ETH,0,true,t3,2026-02-02 10:00:01,2026-02-02T10:00:01Z,\n\
Jeton,Mon ETH,0xa,,10,0xdeadbeefdeadbeef,0,false,t4,2026-02-03 10:00:00,2026-02-03T10:00:00Z,\n\
Bitcoin,Mon BTC,bc1a,,abc,BTC,0,false,t5,2026-02-04 10:00:00,2026-02-04T10:00:00Z,\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Coinomi", "historique des transactions"));
    let o = &i.operations;
    assert_eq!(o.len(), 4, "{o:#?}");
    assert!(transfert(&o[0], "BTC", 0.05, 0.0) && o[0].note.contains("supposée venir"));
    assert!(transfert(&o[1], "BTC", 0.02, 0.00003) && o[1].note.contains("Fournisseur") && o[1].note.contains("cession imposable"));
    // Transfert interne déclaré par Coinomi : certain, sans avertissement.
    assert!(transfert(&o[2], "ETH", 0.499, 0.001) && !o[2].note.contains("supposé"), "{:?}", o[2]);
    assert!(transfert(&o[3], "ETH", 0.499, 0.0) && o[3].note.contains("entre vos comptes"));
    assert_eq!(i.a_verifier.len(), 2, "{}", motifs(&i));
}

// ---------------------------------------------------------------- Détection

#[test]
fn chaque_format_reconnu_par_son_seul_lecteur() {
    // Les en-têtes des plateformes du lot 1 ne sont pas pris pour un portefeuille…
    let coinbase = "Timestamp,Transaction Type,Asset,Quantity Transacted,Price Currency,Price at Transaction,Subtotal,Total (inclusive of fees and/or spread),Fees and/or Spread,Notes\n2026-01-10T09:00:00Z,Buy,BTC,0.02,EUR,50000,1000,1015,15,\n";
    assert_eq!(importer(coinbase).unwrap().plateforme, "Coinbase");
    let kraken = "txid,refid,time,type,subtype,aclass,asset,amount,fee,balance\nL1,R1,2026-01-10 09:00:00,deposit,,currency,XXBT,0.1,0,0.1\n";
    assert_eq!(importer(kraken).unwrap().plateforme, "Kraken");
    let binance = "User_ID,UTC_Time,Account,Operation,Coin,Change,Remark\n1,2026-01-10 09:00:00,Spot,Deposit,BTC,0.1,\n";
    assert_eq!(importer(binance).unwrap().plateforme, "Binance");
    // … et un fichier CoinTracking complet (avec « Sell ») n'est pas pris pour l'export de staking d'Exodus.
    let cointracking = "Type,Buy,Cur.,Sell,Cur.,Fee,Cur.,Exchange,Group,Comment,Date\nStaking,0.5,SOL,,,,,Exodus,,,2026-03-01 00:00:00\n";
    assert!(importer(cointracking).map(|i| i.plateforme != "Exodus").unwrap_or(true));
    // Un en-tête Trezor Suite n'est pas pris pour Coinbase malgré sa colonne « Timestamp ».
    let trezor = "Timestamp,Date,Time,Type,Transaction ID,Fee,Fee unit,Address,Label,Amount,Amount unit,Fiat (EUR),Other\n1768035600,,,RECV,t1,,,a,,0.1,BTC,,\n";
    assert_eq!(importer(trezor).unwrap().plateforme, "Trezor Suite");
    // Le message d'erreur cite les portefeuilles.
    let e = importer("a,b,c\n1,2,3\n").unwrap_err();
    for p in ["Ledger Live", "Trezor Suite", "Electrum", "Exodus", "Sparrow", "Coinomi"] { assert!(e.contains(p), "{p} absent de « {e} »"); }
}

#[test]
fn de_bout_en_bout_portefeuille() {
    // Achat sur une plateforme (saisi), envoi vers Sparrow (réception = transfert), puis envoi depuis Sparrow :
    // seuls les frais de réseau diminuent les avoirs.
    let achat = Operation { date: "2026-01-09".into(), kind: Achat, actif: Some("BTC".into()), quantite: Some(0.05), montant: 2500.0, ..Default::default() };
    let f = "Date (UTC),Label,Value (BTC),Balance (BTC),Fee (BTC),Txid\n\
2026-01-10 09:00:00,,0.05000000,0.05000000,,tx1\n\
2026-02-01 10:00:00,,-0.02003000,0.02997000,0.00003000,tx2\n";
    let mut ops = vec![achat];
    ops.extend(importer(f).unwrap().operations);
    ops.push(Operation { date: "2026-03-01".into(), kind: Cession, actif: Some("BTC".into()), quantite: Some(0.01), montant: 600.0, ..Default::default() });
    let r = calculer_avec_cours(&ops, &[Cours { date: "2026-03-01".into(), actif: "BTC".into(), prix: 60000.0 }]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    // Avoirs avant la cession : 0,05 − 0,00003 = 0,04997 BTC × 60 000 € = 2 998,20 €.
    assert!(proche(r.cessions[0].valeur_portefeuille, 0.04997 * 60000.0), "{:?}", r.cessions[0]);
}
