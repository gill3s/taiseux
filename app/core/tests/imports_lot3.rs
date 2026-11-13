//! Imports du lot 3 (KuCoin, Bybit, OKX, Gate.io, MEXC, Bitfinex, Gemini, Uphold, Nexo), sur des fichiers fictifs
//! construits d'après les en-têtes documentés. Chaque test vérifie aussi la plateforme reconnue : un format du lot 3
//! ne doit pas être pris pour un autre (ni l'inverse, ce que les tests de `imports.rs` couvrent).

use taiseux_core::engine::{calculer_avec_cours, Cours, Operation, TypeOperation::*};
use taiseux_core::imports::{importer, Import};

fn proche(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * b.abs().max(1.0) }

fn lire(f: &str, plateforme: &str, format: &str) -> Import {
    let i = importer(f).unwrap_or_else(|e| panic!("{plateforme} / {format} : {e}"));
    assert_eq!((i.plateforme.as_str(), i.format.as_str()), (plateforme, format));
    i
}

fn achat_(o: &Operation, a: &str, q: f64, m: f64) -> bool { o.kind == Achat && o.actif.as_deref() == Some(a) && proche(o.quantite.unwrap(), q) && proche(o.montant, m) }
fn cession_(o: &Operation, a: &str, q: f64, p: f64, f: f64) -> bool {
    o.kind == Cession && o.actif.as_deref() == Some(a) && proche(o.quantite.unwrap(), q) && proche(o.montant, p) && proche(o.frais.unwrap_or(0.0), f)
}
fn echange_(o: &Operation, a: &str, qa: f64, b: &str, qb: f64) -> bool {
    o.kind == Echange && o.actif.as_deref() == Some(a) && proche(o.quantite.unwrap(), qa) && o.actif_recu.as_deref() == Some(b) && proche(o.quantite_recue.unwrap(), qb)
}
fn transfert_(o: &Operation, a: &str, q: f64, f: f64) -> bool {
    o.kind == Transfert && o.actif.as_deref() == Some(a) && proche(o.quantite.unwrap(), q) && proche(o.quantite_frais.unwrap_or(0.0), f)
}
fn revenu_(o: &Operation, a: &str, q: f64) -> bool { o.kind == RevenuGratuit && o.actif.as_deref() == Some(a) && proche(o.quantite.unwrap(), q) }

// ---------------------------------------------------------------- KuCoin

#[test]
fn kucoin_ordres_archive_avec_fuseau() {
    let f = "UID,Account Type,Order ID,Order Time(UTC+02:00),Symbol,Side,Order Type,Order Price,Order Amount,Avg. Filled Price,Filled Amount,Filled Volume,Filled Volume (USDT),Filled Time(UTC+02:00),Fee,Fee Currency,Tax,Status\n\
1,mainAccount,o1,2026-01-10 01:29:00,BTC-EUR,BUY,MARKET,0,0,50000,0.02,1000,1080,2026-01-10 01:30:00,1,EUR,0,deal\n\
1,mainAccount,o2,2026-02-01 12:00:00,ETH-BTC,SELL,LIMIT,0.05,0.1,0.05,0.1,0.005,300,2026-02-01 12:00:00,0.000005,BTC,0,deal\n\
1,mainAccount,o3,2026-02-02 12:00:00,BTC-USDT,BUY,LIMIT,60000,0.01,60000,0.01,600,600,2026-02-02 12:00:00,0.6,USDT,0,deal\n\
1,mainAccount,o4,2026-03-01 12:00:00,BTC-EUR,SELL,LIMIT,60000,0.005,60000,0.005,300,320,2026-03-01 12:00:00,0.3,EUR,0,deal\n\
1,marginAccount,o5,2026-03-02 12:00:00,BTC-EUR,BUY,LIMIT,60000,0.01,60000,0.01,600,640,2026-03-02 12:00:00,0.6,EUR,0,deal\n\
1,mainAccount,o6,2026-03-03 12:00:00,BTC-USD,SELL,LIMIT,60000,0.01,60000,0.01,600,600,2026-03-03 12:00:00,0.6,USD,0,deal\n\
1,mainAccount,o7,2026-03-04 12:00:00,BTC-EUR,BUY,LIMIT,60000,0.01,60000,0,0,0,2026-03-04 12:00:00,0,EUR,0,cancel\n";
    let i = lire(f, "KuCoin", "ordres exécutés (archive)");
    let o = &i.operations;
    assert_eq!(o.len(), 4, "{o:#?}");
    // 01:30 à UTC+2 = 23:30 UTC la veille ; frais d'achat compris dans le prix d'acquisition.
    assert!(achat_(&o[0], "BTC", 0.02, 1001.0) && o[0].date == "2026-01-09" && o[0].heure == "23:30:00", "{:?}", o[0]);
    assert!(echange_(&o[1], "ETH", 0.1, "BTC", 0.004995), "{:?}", o[1]);
    assert!(echange_(&o[2], "USDT", 600.6, "BTC", 0.01), "frais en USDT ajoutés à ce qui est donné : {:?}", o[2]);
    assert!(cession_(&o[3], "BTC", 0.005, 300.0, 0.3), "{:?}", o[3]);
    assert_eq!(i.ignorees, 1, "ordre non exécuté");
    assert_eq!(i.a_verifier.len(), 2, "{:#?}", i.a_verifier);
    assert!(i.a_verifier[0].motif.contains("marge"));
    assert!(i.a_verifier[1].motif.contains("USD"));
}

#[test]
fn kucoin_anciens_ordres_heure_de_singapour() {
    let v3 = "tradeCreatedAt,orderId,symbol,side,price,size,funds,fee,liquidity,feeCurrency,orderType\n\
2021-03-01 07:00:00,o1,ETH-EUR,buy,1500,1,1500,1.5,taker,EUR,market\n";
    let i = lire(v3, "KuCoin", "ordres (ancien format v3)");
    assert!(achat_(&i.operations[0], "ETH", 1.0, 1501.5) && i.operations[0].date == "2021-02-28" && i.operations[0].heure == "23:00:00", "{:?}", i.operations[0]);

    let v4 = "oid,symbol,dealPrice,dealValue,amount,fee,direction,createdDate,\n\
o1,BTC-EUR,30000,3000,0.1,3,SELL,2021-06-01 12:00:00,\n";
    let i = lire(v4, "KuCoin", "ordres (ancien format v4)");
    assert!(cession_(&i.operations[0], "BTC", 0.1, 3000.0, 3.0), "frais dans la monnaie de cotation : {:?}", i.operations[0]);

    let v2 = "uid,symbol,order_type,price,amount_coin,direction,funds,fee,created_at\n\
1,ETH-BTC,LIMIT,0.05,1,BUY,0.05,0.0001,2020-06-01 12:00:00\n";
    let i = lire(v2, "KuCoin", "ordres (ancien format v2)");
    assert!(echange_(&i.operations[0], "BTC", 0.0501, "ETH", 1.0), "{:?}", i.operations[0]);

    let v1 = "uid,symbol,direction,deal_price,amount,deal_value,created_at\n\
1,BTC-EUR,sell,30000,0.1,3000,2019-06-01 12:00:00\n\
1,BTC-EUR,hold,30000,0.1,3000,2019-06-01 12:00:00\n";
    let i = lire(v1, "KuCoin", "ordres (ancien format v1)");
    assert!(cession_(&i.operations[0], "BTC", 0.1, 3000.0, 0.0));
    assert_eq!(i.a_verifier.len(), 1, "sens inconnu");
}

#[test]
fn kucoin_conversions_achats_earn_mouvements_contrats() {
    let conv = "UID,Account Type,Payment Account,Sell,Buy,Price,Tax,Time of Update(UTC),Status\n\
1,mainAccount,Funding,0.1 BTC,2.5 ETH,25,0,2026-04-01 10:00:00,SUCCESS\n\
1,mainAccount,Funding,0.1 BTC,2.5 ETH,25,0,2026-04-01 11:00:00,FAILED\n";
    let i = lire(conv, "KuCoin", "conversions (Convert)");
    assert!(echange_(&i.operations[0], "BTC", 0.1, "ETH", 2.5));
    assert_eq!(i.ignorees, 1);

    let fiat = "Order ID,Currency (Crypto),Crypto Quantity,Currency (Fiat),Fiat Amount,Fee Currency (Fiat),Fee (Fiat),Fee Currency (Crypto),Fee (Crypto),Payment Method,Status,Time(UTC)\n\
f1,BTC,0.01,EUR,500,EUR,5,,,Card,SUCCEEDED,2026-04-02 10:00:00\n\
f2,BTC,0.01,USD,500,USD,5,,,Card,SUCCEEDED,2026-04-02 11:00:00\n";
    let i = lire(fiat, "KuCoin", "achats par carte ou virement (v2)");
    assert!(achat_(&i.operations[0], "BTC", 0.01, 505.0), "{:?}", i.operations[0]);
    assert_eq!(i.a_verifier.len(), 1, "achat en dollars");

    let earn = "UID,Account Type,Order ID,Time(UTC),Staked Coin,Product Type,Product Name,Earnings Coin,Earnings Type,Remarks,Amount,Amount（USDT）,Fee\n\
1,mainAccount,e1,2026-04-03 00:00:00,DOT,Staking,DOT Staking,DOT,Interest,,0.05,0.3,0\n";
    let i = lire(earn, "KuCoin", "revenus Earn");
    assert!(revenu_(&i.operations[0], "DOT", 0.05));

    let retraits = "UID,Account Type,Time(UTC),Coin,Amount,Fee,Withdrawal Address/Account,Transfer Network,Status,Remarks\n\
1,mainAccount,2026-04-04 10:00:00,BTC,0.01,0.0005,bc1q...,BTC,SUCCESS,\n\
1,mainAccount,2026-04-04 11:00:00,BTC,0.01,0.0005,bc1q...,BTC,FAILURE,\n";
    let i = lire(retraits, "KuCoin", "retraits (archive)");
    assert!(transfert_(&i.operations[0], "BTC", 0.01, 0.0005));
    assert_eq!(i.ignorees, 1);

    let depots = "Time,Coin,Network,Amount,Type,Remark,Fee\n2021-01-01 08:00:00,ETH,ERC20,1.5,Deposit,,0\n";
    let i = lire(depots, "KuCoin", "dépôts (ancien format)");
    assert!(transfert_(&i.operations[0], "ETH", 1.5, 0.0) && i.operations[0].date == "2021-01-01" && i.operations[0].heure == "00:00:00");

    let contrats = "UID,Account Type,Symbol,Close Type,Realized PNL,Total Realized PNL,Total Funding Fees,Total Trading Fees,Position Opening Time(UTC),Position Closing Time(UTC)\n\
1,futures,XBTUSDTM,Close,12,12,0.1,0.2,2026-01-01 00:00:00,2026-01-02 00:00:00\n";
    let i = lire(contrats, "KuCoin", "contrats à terme (résultats réalisés)");
    assert!(i.operations.is_empty() && i.a_verifier.len() == 1);
}

// ---------------------------------------------------------------- Bybit

#[test]
fn bybit_depots_retraits() {
    let v2 = "Date & Time(UTC),Coin,QTY,Type,Account Balance,Description\n\
2026-01-10 09:00:00,BTC,0.5,Deposit,0.5,Deposit\n\
2026-01-11 09:00:00,ETH,-0.5,Withdrawal,1,Withdrawal\n\
2026-01-12 09:00:00,USDT,-100,Transfer,0,Transfer to Derivatives Account\n\
2026-01-13 09:00:00,EUR,100,Deposit,100,Deposit\n\
2026-01-14 09:00:00,USDT,5,Bonus,5,Card Cashback\n";
    let i = lire(v2, "Bybit", "dépôts et retraits (v2)");
    assert!(transfert_(&i.operations[0], "BTC", 0.5, 0.0) && transfert_(&i.operations[1], "ETH", 0.5, 0.0));
    assert_eq!(i.ignorees, 2);
    assert_eq!(i.a_verifier.len(), 1);

    let v1 = "Type,Coin,Amount,Wallet Balance,Time(UTC)\nuserDeposit,BTC,0.1,0.1,2020-01-01 00:00:00\ninternalAccountTransferWithdrawal,BTC,-0.1,0,2020-01-02 00:00:00\n";
    let i = lire(v1, "Bybit", "dépôts et retraits (v1)");
    assert!(transfert_(&i.operations[0], "BTC", 0.1, 0.0));
    assert_eq!(i.ignorees, 1);
}

#[test]
fn bybit_registre_derives() {
    let f = "Time,Currency,Contract,Type,Direction,Quantity,Position,Filled Price,Funding,Fee Paid,Cash Flow,Change,Wallet Balance,Fee Rate,Trade ID,Order ID\n\
2026-01-10 09:00:00,BTC,,deposit,,,,,,0,0.1,0.1,0.1,,,\n\
2026-01-11 09:00:00,BTC,,exchangeOut,,,,,,0,-0.01,-0.01,0.09,,,\n\
2026-01-11 09:00:00,USDT,,exchangeIn,,,,,,0,600,600,600,,,\n\
2026-01-12 09:00:00,USDT,BTCUSDT,trade,Open Long,0.01,0.01,60000,0,0.36,0,-0.36,599.64,0.0006,t1,o1\n\
2026-01-13 09:00:00,USDT,,transferOut,,,,,,0,-100,-100,499.64,,,\n\
2026-01-14 09:00:00,BTC,,withdraw,,,,,,0.0005,-0.05,-0.0505,0.0395,,,\n";
    let i = lire(f, "Bybit", "registre du compte dérivés");
    let o = &i.operations;
    assert_eq!(o.len(), 3, "{o:#?}");
    assert!(transfert_(&o[0], "BTC", 0.1, 0.0));
    assert!(echange_(&o[1], "BTC", 0.01, "USDT", 600.0), "{:?}", o[1]);
    assert!(transfert_(&o[2], "BTC", 0.05, 0.0005));
    assert_eq!(i.ignorees, 1);
    assert_eq!(i.a_verifier.len(), 1);
    assert!(i.a_verifier[0].motif.contains("contrats"));
}

// ---------------------------------------------------------------- OKX

#[test]
fn okx_historique_de_trading() {
    let f = "\u{feff}id,Order id,Time,Trade Type,Instrument,Type,Amount,Unit,PL,Fee,Position Change,Position Balance,Balance Change,Balance,Unit\n\
1,A,2026-01-10 09:00:00,Spot,BTC-EUR,Buy,0.02,BTC,,-0.00002,,,0.01998,0.01998,BTC\n\
1,A,2026-01-10 09:00:00,Spot,BTC-EUR,Sell,1000,EUR,,0,,,-1000,0,EUR\n\
2,B,2026-02-01 09:00:00,Spot,ETH-BTC,Sell,0.1,ETH,,0,,,-0.1,0,ETH\n\
3,B,2026-02-01 09:00:00,Spot,ETH-BTC,Buy,0.005,BTC,,-0.000005,,,0.004995,0.024975,BTC\n\
4,C,2026-02-02 09:00:00,Transfer,,Transfer in,100,USDT,,0,,,100,100,USDT\n\
5,D,2026-02-03 09:00:00,Perpetual,BTC-USDT-SWAP,Buy,1,cont,,-0.1,1,1,-0.1,99.9,USDT\n";
    let i = lire(f, "OKX", "historique de trading");
    let o = &i.operations;
    assert_eq!(o.len(), 2, "{o:#?}");
    assert!(achat_(&o[0], "BTC", 0.01998, 1000.0), "{:?}", o[0]);
    assert!(echange_(&o[1], "ETH", 0.1, "BTC", 0.004995), "appariées par Order id : {:?}", o[1]);
    assert_eq!(i.ignorees, 1);
    assert_eq!(i.a_verifier.len(), 1);
}

#[test]
fn okx_financement_et_ancien_format() {
    let f = "id,Time,Type,Amount,Before Balance,After Balance,Fee,Symbol\n\
1,2026-01-10 09:00:00,Deposit,0.5,0,0.5,0,BTC\n\
2,2026-01-11 09:00:00,Withdrawal,-0.1,0.5,0.3999,0.0001,BTC\n\
3,2026-01-12 09:00:00,Staking Yield,0.01,0,0.01,0,DOT\n\
4,2026-01-13 09:00:00,Stake,-1,1,0,0,DOT\n\
5,2026-01-14 09:00:00,Mystery,1,0,1,0,DOT\n";
    let i = lire(f, "OKX", "compte de financement");
    assert!(transfert_(&i.operations[0], "BTC", 0.5, 0.0) && transfert_(&i.operations[1], "BTC", 0.1, 0.0001) && revenu_(&i.operations[2], "DOT", 0.01));
    assert_eq!((i.ignorees, i.a_verifier.len()), (1, 1));

    let v1 = "time,type,size,balance,fee,currency\n\
2020-01-10 17:00:00,buy,0.1,0.1,-0.0001,BTC\n\
2020-01-10 17:00:00,sell,-800,0,0,EUR\n";
    let i = lire(v1, "OKX", "historique de trading (ancien format)");
    assert!(achat_(&i.operations[0], "BTC", 0.0999, 800.0) && i.operations[0].heure == "09:00:00", "heure de Chine ramenée en UTC : {:?}", i.operations[0]);
}

// ---------------------------------------------------------------- Gate.io

#[test]
fn gateio_registre() {
    let f = "no,time,action_desc,action_data,type,change_amount,amount,total\n\
1,2026-01-10 09:00:00,Order Placed,111,EUR,-1000,0,0\n\
2,2026-01-10 09:00:01,Order Filled,111,BTC,0.02,0.02,0\n\
3,2026-01-10 09:00:01,Trade Fee,111,BTC,-0.00002,0.01998,0\n\
4,2026-02-01 09:00:00,Order Filled,222,BTC,-0.01,0.00998,0\n\
5,2026-02-01 09:00:00,Order Filled,222,EUR,600,600,0\n\
6,2026-02-01 09:00:00,Trade Fee,222,EUR,-1.2,598.8,0\n\
7,2026-02-02 09:00:00,Deposits,d1,ETH,1,1,0\n\
8,2026-02-03 09:00:00,HODL Interest,h1,ETH,0.001,1.001,0\n\
9,2026-02-04 09:00:00,Withdrawals,w1,EUR,-500,98.8,0\n\
10,2026-02-05 09:00:00,Margin Loan,m1,USDT,100,100,0\n";
    let i = lire(f, "Gate.io", "registre du compte");
    let o = &i.operations;
    assert_eq!(o.len(), 4, "{o:#?}");
    assert!(achat_(&o[0], "BTC", 0.01998, 1000.0), "{:?}", o[0]);
    assert!(cession_(&o[1], "BTC", 0.01, 600.0, 1.2), "{:?}", o[1]);
    assert!(transfert_(&o[2], "ETH", 1.0, 0.0) && revenu_(&o[3], "ETH", 0.001));
    assert_eq!((i.ignorees, i.a_verifier.len()), (1, 1));
}

// ---------------------------------------------------------------- MEXC

#[test]
fn mexc_transactions_deux_formats_de_frais() {
    let v3 = "UID,Pairs,Time,Side,Filled Price,Executed Amount,Total,Fee,Role\n\
1,BTC_USDT,2026-01-10 09:00:00,Buy,60000,0.01,600,0.6USDT,Taker\n\
1,BTC_EUR,2026-01-11 09:00:00,Sell,60000,0.005,300,0.3EUR,Maker\n\
1,ETH_BTC,2026-01-12 09:00:00,Buy,0.05,1,0.05,0.001ETH,Maker\n";
    let i = lire(v3, "MEXC", "historique des transactions");
    let o = &i.operations;
    assert!(echange_(&o[0], "USDT", 600.6, "BTC", 0.01), "{:?}", o[0]);
    assert!(cession_(&o[1], "BTC", 0.005, 300.0, 0.3), "{:?}", o[1]);
    assert!(echange_(&o[2], "BTC", 0.05, "ETH", 0.999), "frais dans l'actif reçu : {:?}", o[2]);

    let v1 = "Pairs,Time,Side,Filled Price,Executed Amount,Total,Fee,Role\nBTCEUR,2026-01-10 09:00:00,BUY,50000,0.02,1000,1,Taker\n";
    let i = lire(v1, "MEXC", "historique des transactions");
    assert!(achat_(&i.operations[0], "BTC", 0.02, 1001.0), "{:?}", i.operations[0]);
}

#[test]
fn mexc_ordres_depots_retraits() {
    let f = "Pairs,Time,Type,Direction,Average Filled Price,Order Price,Filled Quantity,Order Quantity,Order Amount,Status\n\
BTC_EUR,2026-01-10 09:00:00,LIMIT,Buy,50000,50000,0.02,0.02,1000,Filled\n\
BTC_EUR,2026-01-11 09:00:00,LIMIT,Buy,50000,50000,0.01,0.02,1000,Partially Filled\n\
BTC_EUR,2026-01-12 09:00:00,LIMIT,Buy,0,50000,0,0.02,1000,Cancelled\n";
    let i = lire(f, "MEXC", "historique des ordres (sans frais)");
    assert!(achat_(&i.operations[0], "BTC", 0.02, 1000.0));
    assert_eq!((i.ignorees, i.a_verifier.len()), (1, 1));

    let d = "UID,Status,Time,Crypto,Network,Deposit Amount,TxID,Progress\n1,Credited Successfully,2026-01-10 09:00:00,BTC,BTC,0.1,abc,3/3\n1,Pending,2026-01-10 10:00:00,BTC,BTC,0.1,abd,1/3\n";
    let i = lire(d, "MEXC", "dépôts");
    assert!(transfert_(&i.operations[0], "BTC", 0.1, 0.0) && i.ignorees == 1);

    let w = "Status,Time,Crypto,Network,Request Amount,Withdrawal Address,TxID,Trading Fee,Settlement Amount,Withdrawal Descriptions\n\
Withdrawal Successful,2026-01-10 09:00:00,ETH,ERC20,1,0xabc,0xdef,0.002,0.998,\n";
    let i = lire(w, "MEXC", "retraits");
    assert!(transfert_(&i.operations[0], "ETH", 1.0, 0.002));
}

// ---------------------------------------------------------------- Bitfinex

#[test]
fn bitfinex_echanges_date_jour_mois_annee() {
    let f = "#,PAIR,AMOUNT,PRICE,FEE,FEE PERC,FEE CURRENCY,DATE,ORDER ID\n\
1,BTC/EUR,0.02,50000,-0.00004,0.2%,BTC,13-01-26 10:00:00,o1\n\
2,ETH/UST,-1,2000,-4,0.2%,UST,14-02-26 10:00:00,o2\n\
3,BTC/EUR,-0.01,60000,-1.2,0.2%,EUR,01-03-26 10:00:00,o3\n\
4,BTC/USD,-0.001,60000,-0.12,0.2%,USD,02-03-26 10:00:00,o4\n";
    let i = lire(f, "Bitfinex", "échanges (trades)");
    let o = &i.operations;
    assert_eq!(o.len(), 3, "{o:#?}");
    assert!(achat_(&o[0], "BTC", 0.01996, 1000.0) && o[0].date == "2026-01-13", "{:?}", o[0]);
    assert!(echange_(&o[1], "ETH", 1.0, "USDT", 1996.0), "UST = USDT : {:?}", o[1]);
    assert!(cession_(&o[2], "BTC", 0.01, 600.0, 1.2) && o[2].date == "2026-03-01");
    assert_eq!(i.a_verifier.len(), 1);
}

#[test]
fn bitfinex_date_ambigue_et_mouvements() {
    // « 26-01-13 » : 13/01/2026 (année-mois-jour) ou 26/01/2013 (jour-mois-année) : on ne devine pas.
    let f = "#,PAIR,AMOUNT,PRICE,FEE,FEE CURRENCY,DATE,ORDER ID\n1,BTC/EUR,0.02,50000,-2,EUR,26-01-13 10:00:00,o1\n";
    let i = lire(f, "Bitfinex", "échanges (trades)");
    assert!(i.operations.is_empty() && i.a_verifier[0].motif.contains("ambigu"), "{:#?}", i.a_verifier);

    let m = "#,DATE,CURRENCY,STATUS,AMOUNT,FEES,DESCRIPTION,TRANSACTION ID,NOTE\n\
1,2026-01-10 09:00:00,BTC,COMPLETED,0.5,0,Bitcoin deposit,tx1,\n\
2,2026-01-11 09:00:00,ETH,COMPLETED,-1,-0.001,Ethereum withdrawal,tx2,\n\
3,2026-01-12 09:00:00,EUR,COMPLETED,1000,0,Wire deposit,tx3,\n\
4,2026-01-13 09:00:00,BTC,CANCELED,-1,0,Bitcoin withdrawal,tx4,\n";
    let i = lire(m, "Bitfinex", "dépôts et retraits (movements)");
    assert!(transfert_(&i.operations[0], "BTC", 0.5, 0.0) && transfert_(&i.operations[1], "ETH", 1.0, 0.001));
    assert_eq!(i.ignorees, 2);
}

// ---------------------------------------------------------------- Gemini

#[test]
fn gemini_historique() {
    let f = "Date,Time (UTC),Type,Symbol,Specification,Liquidity Indicator,Trading Fee Rate (bps),EUR Amount EUR,Fee (EUR) EUR,EUR Balance EUR,BTC Amount BTC,Fee (BTC) BTC,BTC Balance BTC,Trade ID,Order ID,Order Date,Order Time,Client Order ID,API Session,Tx Hash,Deposit Destination,Deposit Tx Output,Withdrawal Destination,Withdrawal Tx Output\n\
2026-01-09,09:00:00.000,Credit,EUR,SEPA,,,2000,,2000,,,,,,,,,,,,,,\n\
2026-01-10,09:00:00.123,Buy,BTCEUR,Exchange Buy,Taker,35,-1000,-3.5,996.5,0.02,,0.02,t1,o1,,,,,,,,,\n\
2026-02-01,10:00:00.000,Sell,BTCEUR,Exchange Sell,Maker,25,600,-1.5,1595,-0.01,,0.01,t2,o2,,,,,,,,,\n\
2026-02-02,10:00:00.000,Debit,BTC,Bitcoin,,,,,,-0.005,-0.0001,0.0049,,,,,,,tx,,,addr,\n\
2026-02-03,10:00:00.000,Interest Credit,BTC,Earn,,,,,,0.0001,,0.005,,,,,,,,,,,\n";
    let i = lire(f, "Gemini", "historique des transactions");
    let o = &i.operations;
    assert_eq!(o.len(), 3, "{o:#?}");
    assert!(achat_(&o[0], "BTC", 0.02, 1003.5) && o[0].heure == "09:00:00", "{:?}", o[0]);
    assert!(cession_(&o[1], "BTC", 0.01, 600.0, 1.5), "{:?}", o[1]);
    assert!(transfert_(&o[2], "BTC", 0.005, 0.0001));
    assert_eq!((i.ignorees, i.a_verifier.len()), (1, 1));
}

// ---------------------------------------------------------------- Uphold

#[test]
fn uphold_actuel() {
    let f = "Date,Destination,Destination Amount,Destination Currency,Fee Amount,Fee Currency,Id,Origin,Origin Amount,Origin Currency,Status,Type\n\
Mon Mar 15 2021 13:16:08 GMT+0100,uphold,0.02,BTC,,,i1,bank,1000,EUR,completed,in\n\
Tue Mar 16 2021 10:00:00 GMT+0000,uphold,0.4,ETH,,,i2,uphold,0.01,BTC,completed,transfer\n\
Wed Mar 17 2021 10:00:00 GMT+0000,bitcoin,0.0049,BTC,0.0001,BTC,i3,uphold,0.005,BTC,completed,out\n\
Thu Mar 18 2021 10:00:00 GMT+0000,bank,300,EUR,,,i4,uphold,0.1,ETH,completed,out\n\
Fri Mar 19 2021 10:00:00 GMT+0000,uphold,0.01,BTC,,,i5,card,600,USD,completed,in\n\
Sat Mar 20 2021 10:00:00 GMT+0000,uphold,0.01,BTC,,,i6,bank,600,EUR,pending,in\n";
    let i = lire(f, "Uphold", "historique des transactions");
    let o = &i.operations;
    assert_eq!(o.len(), 4, "{o:#?}");
    assert!(achat_(&o[0], "BTC", 0.02, 1000.0) && o[0].heure == "12:16:08", "GMT+0100 ramené en UTC : {:?}", o[0]);
    assert!(echange_(&o[1], "BTC", 0.01, "ETH", 0.4));
    assert!(transfert_(&o[2], "BTC", 0.0049, 0.0001), "{:?}", o[2]);
    assert!(cession_(&o[3], "ETH", 0.1, 300.0, 0.0));
    assert_eq!((i.ignorees, i.a_verifier.len()), (1, 1));
    assert!(i.a_verifier[0].motif.contains("USD"));
}

#[test]
fn uphold_ancien_format() {
    let f = "date,id,type,value_in_EUR,commission_in_EUR,pair,rate,origin_currency,origin_amount,origin_commission,destination_currency,destination_amount,destination_commission\n\
2019-05-01T10:00:00Z,i1,transfer,500,2,BTCEUR,5000,EUR,500,2,BTC,0.1,0\n\
2019-05-02T10:00:00Z,i2,deposit,100,0,,1,ETH,1,0,ETH,1,0\n";
    let i = lire(f, "Uphold", "historique des transactions (ancien format)");
    assert!(achat_(&i.operations[0], "BTC", 0.1, 500.0) && transfert_(&i.operations[1], "ETH", 1.0, 0.0));
}

// ---------------------------------------------------------------- Nexo

#[test]
fn nexo_actuel() {
    let f = "Transaction,Type,Input Currency,Input Amount,Output Currency,Output Amount,USD Equivalent,Fee,Fee Currency,Details,Date / Time (UTC)\n\
NXT1,Exchange Deposited On,EUR,1000,EURX,1000,$1080,-,-,approved / SEPA,2026-01-09 09:00:00\n\
NXT2,Exchange,EURX,-1000,BTC,0.02,$1080,-,-,approved / Exchange EURX to BTC,2026-01-10 09:00:00\n\
NXT3,Exchange,BTC,-0.01,ETH,0.2,$600,0.0001,BTC,approved / Exchange BTC to ETH,2026-02-01 09:00:00\n\
NXT4,Interest,NEXO,0.5,NEXO,0.5,$0.6,-,-,approved / AMOUNT EARNED,2026-02-02 00:00:00\n\
NXT5,Deposit,ETH,1,ETH,1,$3000,-,-,approved / 0xabc,2026-02-03 00:00:00\n\
NXT6,Withdrawal,BTC,-0.005,BTC,-0.005,$300,0.0001,BTC,approved / bc1q,2026-02-04 00:00:00\n\
NXT7,Loan Withdrawal,USDX,500,USDX,500,$500,-,-,approved / Credit,2026-02-05 00:00:00\n\
NXT8,Locking Term Deposit,ETH,-1,ETH,1,$3000,-,-,approved / Term,2026-02-06 00:00:00\n\
NXT9,Exchange,BTC,-0.01,ETH,0.2,$600,-,-,rejected / Exchange,2026-02-07 00:00:00\n\
NXT10,Exchange,BTC,-0.005,EURX,300,$320,-,-,approved / Exchange BTC to EURX,2026-02-08 00:00:00\n";
    let i = lire(f, "Nexo", "relevé");
    let o = &i.operations;
    assert_eq!(o.len(), 6, "{o:#?}");
    assert!(achat_(&o[0], "BTC", 0.02, 1000.0), "EURX = euros : {:?}", o[0]);
    assert!(echange_(&o[1], "BTC", 0.01, "ETH", 0.2), "frais en BTC compris dans la quantité donnée : {:?}", o[1]);
    assert!(revenu_(&o[2], "NEXO", 0.5));
    assert!(transfert_(&o[3], "ETH", 1.0, 0.0));
    assert!(transfert_(&o[4], "BTC", 0.0049, 0.0001), "{:?}", o[4]);
    assert!(cession_(&o[5], "BTC", 0.005, 300.0, 0.0));
    assert_eq!(i.ignorees, 3, "dépôt d'euros, dépôt à terme, opération rejetée");
    assert_eq!(i.a_verifier.len(), 1);
    assert!(i.a_verifier[0].motif.contains("prêt"));
}

#[test]
fn nexo_ancien_format() {
    let f = "Transaction,Type,Currency,Amount,USD Equivalent,Details,Outstanding Loan,Date / Time\n\
NXT1,Exchange,BTC/EUR,-0.1 / +3000,$3200,approved / Exchange,$0,2021-01-10 10:00:00\n\
NXT2,Interest,BTC,0.0001,$3,approved / interest,$0,2021-01-11 00:00:00\n\
NXT3,Interest,USDX,-2,$2,approved / loan interest,$0,2021-01-12 00:00:00\n";
    let i = lire(f, "Nexo", "relevé (ancien format)");
    assert!(cession_(&i.operations[0], "BTC", 0.1, 3000.0, 0.0) && revenu_(&i.operations[1], "BTC", 0.0001));
    assert_eq!(i.a_verifier.len(), 1, "intérêts d'emprunt");
}

// ---------------------------------------------------------------- Ensemble

#[test]
fn liste_des_plateformes_dans_le_message_d_erreur() {
    let e = importer("a,b,c\n1,2,3\n").unwrap_err();
    for p in ["KuCoin", "Bybit", "OKX", "Gate.io", "MEXC", "Bitfinex", "Gemini", "Uphold", "Nexo"] { assert!(e.contains(p), "{p}"); }
}

#[test]
fn de_bout_en_bout_kucoin() {
    // Achat de 0,02 BTC (1 001 € frais compris), puis vente de 0,005 BTC à 300 € (0,30 € de frais).
    let f = "UID,Account Type,Order ID,Symbol,Side,Order Type,Avg. Filled Price,Filled Amount,Filled Volume,Filled Volume (USDT),Filled Time(UTC),Fee,Fee Currency\n\
1,mainAccount,o1,BTC-EUR,BUY,MARKET,50000,0.02,1000,1080,2026-01-10 09:00:00,1,EUR\n\
1,mainAccount,o2,BTC-EUR,SELL,LIMIT,60000,0.005,300,320,2026-03-01 12:00:00,0.3,EUR\n";
    let i = lire(f, "KuCoin", "ordres exécutés (archive)");
    let r = calculer_avec_cours(&i.operations, &[] as &[Cours]);
    assert!(r.erreurs.is_empty(), "{:?}", r.erreurs);
    // Valeur globale : 0,02 BTC × 60 000 = 1 200 € ; plus-value = 300 − 0,3 − 1 001 × 300 / 1 200 = 49,45 €.
    assert!(proche(r.cessions[0].valeur_portefeuille, 1200.0));
    assert!(proche(r.cessions[0].plus_value, 300.0 - 0.3 - 1001.0 * 300.0 / 1200.0));
}
