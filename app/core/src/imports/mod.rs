//! Imports des exports CSV des plateformes. Chaque lecteur reconnaît son format à l'en-tête, puis traduit les lignes
//! en opérations Taiseux (achat, cession, échange, transfert, revenu gratuit).
//!
//! Règle d'or : **ne jamais deviner une ligne fiscale.** Ce qu'un lecteur ne sait pas classer avec certitude (type
//! inconnu, montant dans une autre monnaie que l'euro, opération à plusieurs actifs, produits dérivés) part dans
//! `a_verifier`, avec la ligne d'origine, pour que l'utilisateur la saisisse lui-même.
//!
//! Les formats viennent de la documentation des plateformes et des en-têtes relevés par des logiciels open source
//! (noms de colonnes seulement) ; ils n'ont pas tous été vérifiés sur de vrais exports : l'application présente
//! les imports en bêta, avec un aperçu avant de les ajouter.

mod binance;
mod coinbase;
mod commun;
mod kraken;
mod bitpanda;
mod cryptocom;
mod bitvavo;
mod bitstamp;
mod revolut;
mod swissborg;
// Lot 3 : plateformes internationales.
mod aides_lot3;
mod kucoin;
mod bybit;
mod okx;
mod gateio;
mod mexc;
mod bitfinex;
mod gemini;
mod uphold;
mod nexo;
mod ledgerlive;
mod trezor;
mod electrum;
mod exodus;
mod sparrow;
mod coinomi;
mod portefeuilles;

use serde::{Deserialize, Serialize};

use crate::engine::Operation;

/// Une ligne que l'import n'a pas transformée en opération : à saisir ou à écarter par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AVerifier {
    /// Numéro de ligne dans le fichier (à partir de 1, en-tête compris).
    pub ligne: usize,
    pub date: String,
    pub motif: String,
    /// La ligne d'origine, telle qu'elle figure dans le fichier.
    pub contenu: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Import {
    /// Plateforme reconnue, ex. « Coinbase ».
    pub plateforme: String,
    /// Variante du format, ex. « historique des transactions (v4) ».
    pub format: String,
    pub operations: Vec<Operation>,
    pub a_verifier: Vec<AVerifier>,
    /// Lignes volontairement ignorées (dépôts et retraits en euros, mouvements internes), comptées pour information.
    pub ignorees: usize,
}

/// Code d'actif de Kraken → code usuel (XBT → BTC…), pour deviner l'actif d'un fichier de cours Kraken.
pub fn kraken_actif(s: &str) -> String { kraken::actif_kraken(s) }

type Lecteur = fn(&str) -> Option<Import>;
const LECTEURS: &[Lecteur] = &[
    coinbase::lire, kraken::lire, binance::lire,
    bitpanda::lire, cryptocom::lire, bitvavo::lire, bitstamp::lire, revolut::lire, swissborg::lire,
    kucoin::lire, bybit::lire, okx::lire, gateio::lire, mexc::lire, bitfinex::lire, gemini::lire, uphold::lire, nexo::lire,
    ledgerlive::lire, trezor::lire, electrum::lire, exodus::lire, sparrow::lire, coinomi::lire,
];

/// Plateformes et formats reconnus, pour l'aide de l'application.
pub const PLATEFORMES: &[(&str, &str)] = &[
    ("Coinbase", "historique des transactions (CSV, versions 1 à 4)"),
    ("Kraken", "registre « ledgers » (CSV)"),
    ("Binance", "relevé « historique des transactions » (CSV) et historique des ordres au comptant"),
    ("Bitpanda", "historique des transactions (CSV, format actuel et ancien)"),
    ("Crypto.com", "historique du portefeuille de l'application (CSV)"),
    ("Bitvavo", "historique des transactions (CSV)"),
    ("Bitstamp", "historique des transactions (CSV, format actuel et ancien)"),
    ("Revolut", "relevé du compte crypto (CSV, format actuel et ancien)"),
    ("SwissBorg", "relevé de compte (Excel enregistré en CSV)"),
    ("KuCoin", "ordres exécutés (archive et anciens formats), conversions, achats par carte, dépôts, retraits, Earn (CSV)"),
    ("Bybit", "dépôts et retraits, registre du compte dérivés (CSV) ; pas l'historique des ordres au comptant"),
    ("OKX", "historique de trading et compte de financement (CSV)"),
    ("Gate.io", "registre du compte (CSV)"),
    ("MEXC", "historique des transactions, des ordres, dépôts et retraits (CSV)"),
    ("Bitfinex", "échanges (trades) et dépôts et retraits (movements) (CSV)"),
    ("Gemini", "historique des transactions (tableur enregistré en CSV)"),
    ("Uphold", "historique des transactions (CSV)"),
    ("Nexo", "relevé des transactions (CSV)"),
    ("Ledger Live", "historique des opérations (CSV)"),
    ("Trezor Suite", "export des transactions (CSV)"),
    ("Electrum", "historique du portefeuille (CSV, bitcoin)"),
    ("Exodus", "export des transactions (CSV) et récompenses de staking"),
    ("Sparrow", "export des transactions (CSV, BTC ou sats)"),
    ("Coinomi", "historique des transactions (CSV)"),
];

/// Reconnaît le format d'un export et le lit. Erreur si aucun lecteur ne reconnaît l'en-tête.
pub fn importer(texte: &str) -> Result<Import, String> {
    let texte = texte.trim_start_matches('\u{feff}');
    for lire in LECTEURS {
        if let Some(mut i) = lire(texte) {
            i.operations.sort_by(|a, b| a.date.cmp(&b.date).then(a.heure.cmp(&b.heure)));
            return Ok(i);
        }
    }
    Err(format!(
        "Format non reconnu. Plateformes reconnues : {}. Vous pouvez aussi utiliser le format CSV de Taiseux.",
        PLATEFORMES.iter().map(|(p, _)| *p).collect::<Vec<_>>().join(", ")
    ))
}
