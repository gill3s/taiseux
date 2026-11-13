//! Imports du lot 2 (Bitpanda, Crypto.com, Bitvavo, Bitstamp, Revolut, SwissBorg), sur des fichiers fictifs
//! construits d'après les en-têtes documentés : une variante d'en-tête par test, opérations attendues calculées à
//! la main, lignes « à vérifier », et spécificité de la détection.

use taiseux_core::engine::{calculer_avec_cours, Cours, Operation, TypeOperation::*};
use taiseux_core::imports::importer;

fn proche(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * b.abs().max(1.0) }
fn est(o: &Operation, kind: taiseux_core::engine::TypeOperation, actif: &str, q: f64) -> bool {
    o.kind == kind && o.actif.as_deref() == Some(actif) && proche(o.quantite.unwrap_or(f64::NAN), q)
}

// ---------------------------------------------------------------- Bitpanda

const BITPANDA: &str = "\"Disclaimer: All data is without guarantee, errors and changes are reserved.\"\n\
\"User ID: 1234\"\n\
Transaction ID,Timestamp,Transaction Type,In/Out,Amount Fiat,Fiat,Amount Asset,Asset,Asset market price,Asset market price currency,Asset class,Product ID,Fee,Fee asset,Spread,Spread Currency,Tax Fiat\n\
F1,2026-01-05T10:00:00+01:00,deposit,incoming,1000.00,EUR,-,EUR,-,-,Fiat,-,0.00,EUR,-,-,0.00\n\
T1,2026-01-10T09:00:00+01:00,buy,outgoing,250.00,EUR,0.00500000,BTC,50000.00,EUR,Cryptocurrency,1,-,-,-,-,0.00\n\
T2,2026-01-11T09:00:00+01:00,buy,outgoing,100.00,EUR,200.00000000,MATIC,0.50,EUR,Fiat,38,-,-,-,-,0.00\n\
T3,2026-02-01T10:00:00+01:00,sell,incoming,120.00,EUR,0.00200000,BTC,60000.00,EUR,Cryptocurrency,1,-,-,-,-,0.00\n\
T4,2026-02-02T10:00:00+01:00,buy,outgoing,150.00,EUR,1.40000000,AMZN,105.00,EUR,\"Stock (derivative)\",73,-,-,0.15000000,EUR,0.00\n\
C1,2026-02-03T10:00:00+01:00,withdrawal,outgoing,0,EUR,0.00100000,BTC,0.00,-,Cryptocurrency,1,0.00006000,BTC,-,-,0.00\n\
X1,2026-02-04T10:00:00+01:00,transfer,incoming,0.00,EUR,0.50000000,BEST,0.34,EUR,Cryptocurrency,33,-,-,-,-,0.00\n\
S1,2026-02-05T10:00:00+01:00,sell,incoming,50.00,EUR,0.00100000,BTC,50000.00,EUR,Cryptocurrency,1,-,-,-,-,0.00\n\
S2,2026-02-05T10:00:00+01:00,buy,outgoing,50.00,EUR,0.02000000,ETH,2500.00,EUR,Cryptocurrency,5,-,-,-,-,0.00\n\
T5,2026-02-06T10:00:00+01:00,buy,outgoing,100.00,USD,0.00200000,BTC,50000.00,USD,Cryptocurrency,1,-,-,-,-,0.00\n";

#[test]
fn bitpanda_format_actuel() {
    let i = importer(BITPANDA).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Bitpanda", "historique des transactions"));
    let o = &i.operations;
    assert_eq!(o.len(), 4, "{o:#?}");
    assert!(est(&o[0], Achat, "BTC", 0.005) && proche(o[0].montant, 250.0), "{:?}", o[0]);
    assert!(est(&o[1], Achat, "MATIC", 200.0) && proche(o[1].montant, 100.0), "MATIC étiqueté Fiat reste une crypto : {:?}", o[1]);
    assert!(est(&o[2], Cession, "BTC", 0.002) && proche(o[2].montant, 120.0) && o[2].frais.is_none());
    assert!(est(&o[3], Transfert, "BTC", 0.001) && proche(o[3].quantite_frais.unwrap(), 0.00006));
    assert_eq!(o[0].date, "2026-01-10");
    assert_eq!(i.ignorees, 1, "dépôt d'euros");
    let motifs: Vec<&str> = i.a_verifier.iter().map(|v| v.motif.as_str()).collect();
    assert_eq!(motifs.len(), 5, "{motifs:#?}");
    assert!(motifs[0].contains("Stock"), "action : {}", motifs[0]);
    assert!(motifs[1].contains("transfert"));
    assert!(motifs[2].contains("même instant") && motifs[3].contains("même instant"), "swap possible");
    assert!(motifs[4].contains("USD"));
    assert_eq!(i.a_verifier[0].ligne, 8, "numéro de ligne dans le fichier, introduction comprise");
}

#[test]
fn bitpanda_ancien_format() {
    let f = "ID,Type,In/Out,Amount Fiat,Fee,Fiat,Amount Asset,Asset,Status,Created at\n\
1,buy,outgoing,100.00,0.00,EUR,0.01,ETH,finished,2019-03-01 10:00:00\n\
2,sell,incoming,50.00,0.00,EUR,0.005,ETH,canceled,2019-03-02 10:00:00\n\
3,sell,incoming,60.00,0.50,EUR,0.005,ETH,finished,2019-03-03 10:00:00\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Bitpanda", "historique des transactions (ancien format)"));
    assert_eq!(i.operations.len(), 1);
    assert!(est(&i.operations[0], Achat, "ETH", 0.01) && proche(i.operations[0].montant, 100.0));
    assert_eq!(i.operations[0].heure, "10:00:00");
    assert_eq!(i.ignorees, 1, "ordre annulé");
    assert_eq!(i.a_verifier.len(), 1);
    assert!(i.a_verifier[0].motif.contains("frais"), "frais distincts : {:?}", i.a_verifier);
}

// ---------------------------------------------------------------- Crypto.com

const CRYPTOCOM: &str = "Timestamp (UTC),Transaction Description,Currency,Amount,To Currency,To Amount,Native Currency,Native Amount,Native Amount (in USD),Transaction Kind,Transaction Hash\n\
2026-01-10 09:00:00,Buy BTC,EUR,-1000.00,BTC,0.02,EUR,1000.00,1080.00,viban_purchase,\n\
2026-01-11 09:00:00,Buy CRO,CRO,500,,,EUR,50.00,54.00,crypto_purchase,\n\
2026-02-01 10:00:00,BTC -> ETH,BTC,-0.01,ETH,0.2,EUR,520.00,560.00,crypto_exchange,\n\
2026-03-01 11:00:00,ETH -> EUR,ETH,-0.1,EUR,280.00,EUR,280.00,300.00,crypto_viban_exchange,\n\
2026-03-02 11:00:00,Crypto Earn,ETH,0.001,,,EUR,2.80,3.00,crypto_earn_interest_paid,\n\
2026-03-03 11:00:00,Withdraw BTC,BTC,-0.005,,,EUR,275.00,300.00,crypto_withdrawal,0xabc\n\
2026-03-04 11:00:00,Card Cashback,CRO,10,,,EUR,1.00,1.10,referral_card_cashback,\n\
2026-03-05 11:00:00,Crypto Earn Deposit,ETH,-0.05,,,EUR,140.00,150.00,crypto_earn_program_created,\n\
2026-03-06 11:00:00,EUR Deposit,EUR,500,,,EUR,500.00,540.00,,\n\
2026-03-07 11:00:00,ETH -> USD,ETH,-0.01,USD,30.00,EUR,28.00,30.00,crypto_viban_exchange,\n\
2026-03-08 11:00:00,Transfer,BTC,0.001,,,EUR,50.00,55.00,crypto_transfer,\n\
2026-03-09 11:00:00,Payment,CRO,-20,,,EUR,2.00,2.20,crypto_payment,\n";

#[test]
fn cryptocom_avec_empreinte() {
    let i = importer(CRYPTOCOM).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Crypto.com", "historique du portefeuille (avec empreinte de transaction)"));
    let o = &i.operations;
    assert_eq!(o.len(), 8, "{o:#?}");
    assert!(est(&o[0], Achat, "BTC", 0.02) && proche(o[0].montant, 1000.0), "{:?}", o[0]);
    assert!(est(&o[1], Achat, "CRO", 500.0) && proche(o[1].montant, 50.0));
    assert!(est(&o[2], Echange, "BTC", 0.01) && o[2].actif_recu.as_deref() == Some("ETH") && proche(o[2].quantite_recue.unwrap(), 0.2));
    assert!(est(&o[3], Cession, "ETH", 0.1) && proche(o[3].montant, 280.0));
    assert!(est(&o[4], RevenuGratuit, "ETH", 0.001));
    assert!(est(&o[5], Transfert, "BTC", 0.005));
    assert!(est(&o[6], RevenuGratuit, "CRO", 10.0));
    assert!(est(&o[7], Cession, "CRO", 20.0) && proche(o[7].montant, 2.0), "paiement en crypto = cession : {:?}", o[7]);
    assert_eq!(i.ignorees, 2, "blocage Earn et dépôt d'euros");
    assert_eq!(i.a_verifier.len(), 2, "{:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("USD"));
    assert!(i.a_verifier[1].motif.contains("utilisateurs"));
}

#[test]
fn cryptocom_sans_empreinte() {
    let f = "Timestamp (UTC),Transaction Description,Currency,Amount,To Currency,To Amount,Native Currency,Native Amount,Native Amount (in USD),Transaction Kind\n\
2021-06-01 08:00:00,Buy CRO,CRO,1000,,,USD,100.00,100.00,crypto_purchase\n\
2021-06-02 08:00:00,Dust,CRO,5,,,USD,0.5,0.5,dust_conversion_credited\n";
    let i = importer(f).unwrap();
    assert_eq!(i.format, "historique du portefeuille");
    assert!(i.operations.is_empty());
    assert_eq!(i.a_verifier.len(), 2, "achat valorisé en dollars, conversion en deux lignes : {:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("USD"));
}

// ---------------------------------------------------------------- Bitvavo

#[test]
fn bitvavo() {
    // Ligne 3 reprise de l'exemple public : 0,0053543 × 1 862,8 = 9,97399 € + 0,026 € de frais = 10,00 € payés.
    let f = "Timezone,Date,Time,Type,Currency,Amount,Quote Currency,Quote Price,Received / Paid Currency,Received / Paid Amount,Fee currency,Fee amount,Status,Transaction ID,Address\n\
Europe/Amsterdam,2026-01-05,10:00:00,deposit,EUR,1000,,,,,EUR,0,Completed,id1,NL12***34\n\
Europe/Amsterdam,2026-01-10,09:00:00.123,buy,ETH,0.0053543,EUR,1862.8,EUR,-10.00,EUR,0.02600996,Completed,id2,\n\
Europe/Amsterdam,2026-01-11,09:00:00,buy,BTC,0.001,EUR,50000,EUR,-50.00,EUR,0.125,Completed,id3,\n\
Europe/Amsterdam,2026-02-01,10:00:00,sell,ETH,-0.002,EUR,2000,EUR,3.99,EUR,0.01,Completed,id4,\n\
Europe/Amsterdam,2026-02-02,10:00:00,withdrawal,BTC,-0.0005,,,,,BTC,0.00003,Completed,id5,bc1qxyz\n\
Europe/Amsterdam,2026-02-03,10:00:00,staking,ETH,0.0001,,,,,EUR,0,Completed,id6,\n\
Europe/Amsterdam,2026-02-04,10:00:00,buy,ETH,0.001,EUR,2000,EUR,-1.00,EUR,0,Completed,id7,\n\
Europe/Amsterdam,2026-02-05,10:00:00,price guarantee,ETH,0.0001,,,,,EUR,0,Completed,id8,\n\
Europe/Amsterdam,2026-02-06,10:00:00,buy,ETH,0.001,EUR,2000,EUR,-2.00,EUR,0,Canceled,id9,\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Bitvavo", "historique des transactions"));
    let o = &i.operations;
    assert_eq!(o.len(), 5, "{o:#?}");
    assert!(est(&o[0], Achat, "ETH", 0.0053543) && proche(o[0].montant, 10.0), "frais compris dans le montant payé : {:?}", o[0]);
    assert_eq!(o[0].heure, "09:00:00", "millisecondes retirées");
    assert!(est(&o[1], Achat, "BTC", 0.001) && proche(o[1].montant, 50.125), "frais en sus : {:?}", o[1]);
    assert!(est(&o[2], Cession, "ETH", 0.002) && proche(o[2].montant, 4.0) && proche(o[2].frais.unwrap(), 0.01), "reçu net de frais : {:?}", o[2]);
    assert!(est(&o[3], Transfert, "BTC", 0.0005) && proche(o[3].quantite_frais.unwrap(), 0.00003));
    assert!(est(&o[4], RevenuGratuit, "ETH", 0.0001));
    assert_eq!(i.ignorees, 2, "dépôt d'euros, ordre annulé");
    assert_eq!(i.a_verifier.len(), 2, "{:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("incohérent"));
    assert!(i.a_verifier[1].motif.contains("price guarantee"));
}

// ---------------------------------------------------------------- Bitstamp

const BITSTAMP: &str = "ID,Account,Type,Subtype,Datetime,Amount,Amount currency,Value,Value currency,Rate,Rate currency,Fee,Fee currency,Order ID\n\
1,Main Account,Deposit,,2026-01-05T10:00:00Z,1000.00,EUR,,,,,,,\n\
2,Main Account,Market,Buy,2026-01-10T09:00:00Z,0.02000000,BTC,1000.00,EUR,50000.00,EUR,3.00,EUR,111\n\
3,Main Account,Market,Sell,2026-03-01T11:00:00Z,0.01000000,BTC,550.00,EUR,55000.00,EUR,1.65,EUR,112\n\
4,Main Account,Market,Buy,2026-03-02T11:00:00Z,0.10000000,ETH,0.00500000,BTC,0.05000000,BTC,0.00001250,BTC,113\n\
5,Main Account,Staking reward,,2026-03-03T11:00:00Z,0.00010000,ETH2R,,,,,,,\n\
6,Main Account,Withdrawal,,2026-03-04T11:00:00Z,0.00200000,BTC,,,,,,,\n\
7,Main Account,Market,Buy,2026-03-05T11:00:00Z,0.00100000,BTC,60.00,USD,60000.00,USD,0.20,USD,114\n\
8,Main Account,Sub Account transfer,,2026-03-06T11:00:00Z,1.00000000,BTC,,,,,,,\n";

#[test]
fn bitstamp_format_actuel() {
    let i = importer(BITSTAMP).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Bitstamp", "historique des transactions"));
    let o = &i.operations;
    assert_eq!(o.len(), 5, "{o:#?}");
    assert!(est(&o[0], Achat, "BTC", 0.02) && proche(o[0].montant, 1003.0), "valeur + frais : {:?}", o[0]);
    assert!(est(&o[1], Cession, "BTC", 0.01) && proche(o[1].montant, 550.0) && proche(o[1].frais.unwrap(), 1.65));
    assert!(est(&o[2], Echange, "BTC", 0.0050125) && o[2].actif_recu.as_deref() == Some("ETH") && proche(o[2].quantite_recue.unwrap(), 0.1), "frais en BTC ajoutés au donné : {:?}", o[2]);
    assert!(est(&o[3], RevenuGratuit, "ETH", 0.0001), "ETH2R → ETH : {:?}", o[3]);
    assert!(est(&o[4], Transfert, "BTC", 0.002));
    assert_eq!(i.ignorees, 1);
    assert_eq!(i.a_verifier.len(), 2, "{:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("USD"));
    assert!(i.a_verifier[1].motif.contains("Sub Account transfer"));
}

#[test]
fn bitstamp_ancien_format() {
    let f = "Type,Datetime,Account,Amount,Value,Rate,Fee,Sub Type\n\
Deposit,\"Jan. 05, 2021, 08:00 AM\",Main Account,15000.00 EUR,,,,\n\
Market,\"Jan. 10, 2021, 09:15 AM\",Main Account,0.50000000 BTC,15000.00 EUR,30000.00 EUR,7.50 EUR,Buy\n\
Market,\"May 05, 2021, 02:30 PM\",Main Account,0.10000000 BTC,5000.00 EUR,50000.00 EUR,2.50 EUR,Sell\n\
Market,\"Sept. 01, 2021, 12:05 AM\",Main Account,0.10000000 BTC,4000.00 EUR,40000.00 EUR,2.00 EUR,Sell\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Bitstamp", "historique des transactions (ancien format)"));
    let o = &i.operations;
    assert_eq!(o.len(), 3, "{o:#?} {:#?}", i.a_verifier);
    assert!(est(&o[0], Achat, "BTC", 0.5) && proche(o[0].montant, 15007.5));
    assert_eq!((o[0].date.as_str(), o[0].heure.as_str()), ("2021-01-10", "09:15:00"));
    assert!(est(&o[1], Cession, "BTC", 0.1) && proche(o[1].montant, 5000.0) && proche(o[1].frais.unwrap(), 2.5));
    assert_eq!((o[1].date.as_str(), o[1].heure.as_str()), ("2021-05-05", "14:30:00"));
    assert_eq!((o[2].date.as_str(), o[2].heure.as_str()), ("2021-09-01", "00:05:00"), "minuit en AM");
    assert_eq!(i.ignorees, 1);
}

#[test]
fn de_bout_en_bout_bitstamp() {
    // Achat de 0,02 BTC (1 003 € frais compris), vente de 0,01 BTC à 550 € (1,65 € de frais).
    // Valeur globale : 0,02 BTC × 55 000 € (cours déduit de la vente) = 1 100 € ;
    // plus-value = 550 − 1,65 − 1 003 × 550 / 1 100 = 46,85 €.
    let f: String = BITSTAMP.lines().take(4).collect::<Vec<_>>().join("\n");
    let i = importer(&f).unwrap();
    let r = calculer_avec_cours(&i.operations, &[] as &[Cours]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    assert!(proche(r.cessions[0].valeur_portefeuille, 1100.0));
    assert!(proche(r.cessions[0].plus_value, 550.0 - 1.65 - 1003.0 * 550.0 / 1100.0));
}

// ---------------------------------------------------------------- Revolut

#[test]
fn revolut_format_actuel() {
    let f = "Symbol,Type,Quantity,Price,Value,Fees,Date\n\
BTC,Buy,0.02,\"€50,000.00\",\"€1,000.00\",€14.90,\"Jan 10, 2026, 9:00:00 AM\"\n\
BTC,Sell,0.01,\"€55,000.00\",€550.00,€8.25,\"Mar 1, 2026, 11:00:00 AM\"\n\
ETH,Staking reward,0.001,\"€2,800.00\",€2.80,,\"Mar 2, 2026, 11:00:00 AM\"\n\
ETH,Stake,0.5,\"€2,800.00\",\"€1,400.00\",,\"Mar 3, 2026, 11:00:00 AM\"\n\
BTC,Send,0.005,\"€55,000.00\",€275.00,,2026-03-04 11:00:00\n\
BTC,Buy,0.001,\"$60,000.00\",$60.00,$1.00,\"Mar 5, 2026, 11:00:00 AM\"\n\
ETH,Swap,0.1,\"€2,800.00\",€280.00,,\"Mar 6, 2026, 11:00:00 PM\"\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Revolut", "relevé du compte crypto"));
    let o = &i.operations;
    assert_eq!(o.len(), 4, "{o:#?} {:#?}", i.a_verifier);
    assert!(est(&o[0], Achat, "BTC", 0.02) && proche(o[0].montant, 1014.9), "valeur + frais : {:?}", o[0]);
    assert_eq!((o[0].date.as_str(), o[0].heure.as_str()), ("2026-01-10", "09:00:00"));
    assert!(est(&o[1], Cession, "BTC", 0.01) && proche(o[1].montant, 550.0) && proche(o[1].frais.unwrap(), 8.25));
    assert!(est(&o[2], RevenuGratuit, "ETH", 0.001));
    assert!(est(&o[3], Transfert, "BTC", 0.005));
    assert_eq!(i.ignorees, 1, "mise en staking");
    assert_eq!(i.a_verifier.len(), 2, "{:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("USD"));
    assert!(i.a_verifier[1].motif.contains("Swap"));
    assert_eq!(i.a_verifier[1].date, "2026-03-06");
}

#[test]
fn revolut_ancien_format() {
    let f = "Type,Product,Started Date,Completed Date,Description,Amount,Currency,Fiat amount,Fiat amount (inc. fees),Fee,Base currency,State,Balance\n\
EXCHANGE,Current,2026-01-01 10:00:00,2026-01-01 10:00:00,Exchanged to ETH,1.0000,ETH,2000.00,2029.90,29.90,EUR,COMPLETED,1.0000\n\
EXCHANGE,Current,2026-01-02 10:00:00,2026-01-02 10:00:00,Exchanged to EUR,-0.3000,ETH,-700.00,-689.50,10.50,EUR,COMPLETED,0.7000\n\
CARD_PAYMENT,Current,2026-01-03 10:00:00,2026-01-03 10:00:00,Payment to Shop,-0.0100,ETH,-25.00,-24.63,0.37,EUR,COMPLETED,0.6900\n\
EXCHANGE,Current,2026-01-04 10:00:00,,Exchanged to ETH,0.1000,ETH,200.00,203.00,3.00,EUR,REVERTED,0.6900\n\
EXCHANGE,Current,2026-01-05 10:00:00,2026-01-05 10:00:00,Exchanged to SEK,-0.1000,ETH,-2000.00,-1990.00,10.00,SEK,COMPLETED,0.5900\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("Revolut", "relevé du compte crypto (ancien format)"));
    let o = &i.operations;
    assert_eq!(o.len(), 3, "{o:#?}");
    assert!(est(&o[0], Achat, "ETH", 1.0) && proche(o[0].montant, 2029.9));
    assert!(est(&o[1], Cession, "ETH", 0.3) && proche(o[1].montant, 700.0) && proche(o[1].frais.unwrap(), 10.5));
    assert!(est(&o[2], Cession, "ETH", 0.01) && proche(o[2].montant, 25.0) && proche(o[2].frais.unwrap(), 0.37), "paiement par carte : {:?}", o[2]);
    assert_eq!(i.ignorees, 1, "opération annulée");
    assert_eq!(i.a_verifier.len(), 1);
    assert!(i.a_verifier[0].motif.contains("SEK"));
}

// ---------------------------------------------------------------- SwissBorg

#[test]
fn swissborg() {
    let f = "SwissBorg account statement\nName,Jean Dupont\n\n\
Local time,Time in UTC,Type,Currency,Gross amount,Gross amount (EUR),Fee,Fee (EUR),Net amount,Net amount (EUR),Note\n\
2026-01-05 11:00:00,2026-01-05 10:00:00,Deposit,EUR,1000,1000,0,0,1000,1000,\n\
2026-01-10 10:00:00,2026-01-10 09:00:00,Sell,EUR,1000,1000,0,0,1000,1000,Exchanged to BTC\n\
2026-01-10 10:00:00,2026-01-10 09:00:00,Buy,BTC,0.02,1000,0.0001,5,0.0199,995,Exchanged from EUR\n\
2026-02-01 11:00:00,2026-02-01 10:00:00,Sell,BTC,0.01,520,0,0,0.01,520,Exchanged to ETH\n\
2026-02-01 11:00:00,2026-02-01 10:00:00,Buy,ETH,0.2,520,0.001,2.6,0.199,517.4,Exchanged from BTC\n\
2026-03-01 12:00:00,2026-03-01 11:00:00,Sell,ETH,0.1,280,0,0,0.1,280,Exchanged to EUR\n\
2026-03-01 12:00:00,2026-03-01 11:00:00,Buy,EUR,280,280,1.4,1.4,278.6,278.6,Exchanged from ETH\n\
2026-03-02 12:00:00,2026-03-02 11:00:00,Payouts,USDC,0.5,0.46,0,0,0.5,0.46,Yield payouts\n\
2026-03-03 12:00:00,2026-03-03 11:00:00,Withdrawal,BTC,0.005,275,0.0001,5.5,0.0049,269.5,\n\
2026-03-04 12:00:00,2026-03-04 11:00:00,Fee Adjustment,CHSB,1,0.2,0,0,1,0.2,\n\
2026-03-05 12:00:00,2026-03-05 11:00:00,Sell,CHF,100,104,0,0,100,104,Exchanged to BTC\n\
2026-03-05 12:00:00,2026-03-05 11:00:00,Buy,BTC,0.002,104,0,0,0.002,104,Exchanged from CHF\n";
    let i = importer(f).unwrap();
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), ("SwissBorg", "relevé de compte"));
    let o = &i.operations;
    assert_eq!(o.len(), 5, "{o:#?} {:#?}", i.a_verifier);
    assert!(est(&o[0], Achat, "BTC", 0.0199) && proche(o[0].montant, 1000.0), "frais en BTC déduits : {:?}", o[0]);
    assert!(est(&o[1], Echange, "BTC", 0.01) && o[1].actif_recu.as_deref() == Some("ETH") && proche(o[1].quantite_recue.unwrap(), 0.199));
    assert!(est(&o[2], Cession, "ETH", 0.1) && proche(o[2].montant, 280.0) && proche(o[2].frais.unwrap(), 1.4));
    assert!(est(&o[3], RevenuGratuit, "USDC", 0.5));
    assert!(est(&o[4], Transfert, "BTC", 0.005) && proche(o[4].quantite_frais.unwrap(), 0.0001));
    assert_eq!(i.ignorees, 1);
    assert_eq!(i.a_verifier.len(), 2, "{:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("frais"));
    assert!(i.a_verifier[1].motif.contains("CHF") && i.a_verifier[1].contenu.lines().count() == 2, "les deux lignes de l'échange : {:?}", i.a_verifier[1]);
}

// ---------------------------------------------------------------- Détection

/// Chaque format n'est reconnu que par son lecteur : sans sa colonne distinctive, aucun lecteur ne le prend
/// (sinon un autre format l'aurait « reconnu » à tort).
#[test]
fn detection_specifique() {
    let cas: &[(&str, &str)] = &[
        ("Transaction ID,Timestamp,Transaction Type,In/Out,Amount Fiat,Fiat,Amount Asset,Asset,Asset class", "In/Out"),
        ("ID,Type,In/Out,Amount Fiat,Fee,Fiat,Amount Asset,Asset,Status,Created at", "In/Out"),
        ("Timestamp (UTC),Transaction Description,Currency,Amount,To Currency,To Amount,Native Currency,Native Amount,Native Amount (in USD),Transaction Kind", "Transaction Kind"),
        ("Timezone,Date,Time,Type,Currency,Amount,Quote Currency,Quote Price,Received / Paid Currency,Received / Paid Amount,Fee currency,Fee amount,Status,Transaction ID,Address", "Received / Paid Amount"),
        ("ID,Account,Type,Subtype,Datetime,Amount,Amount currency,Value,Value currency,Rate,Rate currency,Fee,Fee currency,Order ID", "Datetime"),
        ("Type,Datetime,Account,Amount,Value,Rate,Fee,Sub Type", "Sub Type"),
        ("Symbol,Type,Quantity,Price,Value,Fees,Date", "Symbol"),
        ("Type,Product,Started Date,Completed Date,Description,Amount,Currency,Fiat amount,Fiat amount (inc. fees),Fee,Base currency,State,Balance", "Fiat amount (inc. fees)"),
        ("Local time,Time in UTC,Type,Currency,Gross amount,Gross amount (EUR),Fee,Fee (EUR),Net amount,Net amount (EUR),Note", "Gross amount (EUR)"),
    ];
    for (entete, distinctive) in cas {
        let complet = format!("{entete}\n");
        assert!(importer(&complet).is_ok(), "en-tête non reconnu : {entete}");
        let sans: Vec<&str> = entete.split(',').filter(|c| c != distinctive).collect();
        let e = importer(&format!("{}\n", sans.join(","))).err();
        assert!(e.is_some(), "sans « {distinctive} », l'en-tête « {entete} » est encore reconnu");
    }
    // Les formats des lots précédents gardent leur plateforme.
    assert_eq!(importer("Timestamp,Transaction Type,Asset,Quantity Transacted,Price Currency,Subtotal\n").unwrap().plateforme, "Coinbase");
    assert_eq!(importer("txid,refid,time,type,asset,amount,fee\n").unwrap().plateforme, "Kraken");
    assert_eq!(importer("User_ID,UTC_Time,Account,Operation,Coin,Change,Remark\n").unwrap().plateforme, "Binance");
    // Un relevé de compte courant Revolut (sans contre-valeur en euros des cryptos) n'est pas reconnu.
    assert!(importer("Type,Product,Started Date,Completed Date,Description,Amount,Fee,Currency,State,Balance\n").is_err());
}

#[test]
fn format_inconnu_cite_le_lot2() {
    let e = importer("a,b,c\n1,2,3\n").unwrap_err();
    for p in ["Bitpanda", "Crypto.com", "Bitvavo", "Bitstamp", "Revolut", "SwissBorg"] { assert!(e.contains(p), "{p} absent : {e}"); }
}
