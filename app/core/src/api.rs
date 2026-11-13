//! Fonctions appelées par l'application (et par l'aperçu de développement) : elles assemblent le moteur, les
//! imports et les cours. Aucune ne touche au disque ni au réseau : l'appelant fournit les octets ou le texte.

use serde::{Deserialize, Serialize};

use crate::engine::{self, AnneeFoyer, Cours, Operation, Resultat};
use crate::imports::{self, AVerifier};
use crate::csv;

/// Résultat d'un foyer : le calcul de chaque membre (dans l'ordre fourni) et les totaux du foyer par année.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CalculFoyer {
    pub membres: Vec<Resultat>,
    pub foyer: Vec<AnneeFoyer>,
}

/// Chaque membre est calculé avec son propre portefeuille ; les cours sont communs.
pub fn calculer_foyer(membres: &[Vec<Operation>], cours: &[Cours]) -> CalculFoyer {
    let membres: Vec<Resultat> = membres.iter().map(|ops| engine::calculer_avec_cours(ops, cours)).collect();
    let foyer = engine::foyer(&membres);
    CalculFoyer { membres, foyer }
}

/// Ce qu'un import propose à l'utilisateur, avant qu'il n'accepte.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Apercu {
    pub plateforme: String,
    pub format: String,
    pub operations: Vec<Operation>,
    pub a_verifier: Vec<AVerifier>,
    pub ignorees: usize,
    /// Erreurs de lecture du format CSV de Taiseux (lignes illisibles).
    pub erreurs: Vec<String>,
}

/// Texte d'un fichier exporté : UTF-8 (avec ou sans BOM), UTF-16 avec BOM (certains tableurs), sinon Windows-1252
/// approché par Latin-1 (les exports « CSV » d'Excel en français). Ne refuse jamais : au pire des accents abîmés.
pub fn texte_fichier(octets: &[u8]) -> String {
    if let Some(r) = octets.strip_prefix(&[0xEF, 0xBB, 0xBF]) { return String::from_utf8_lossy(r).into_owned(); }
    let utf16 = |le: bool, r: &[u8]| -> String {
        let u: Vec<u16> = r.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&u)
    };
    if let Some(r) = octets.strip_prefix(&[0xFF, 0xFE]) { return utf16(true, r); }
    if let Some(r) = octets.strip_prefix(&[0xFE, 0xFF]) { return utf16(false, r); }
    match std::str::from_utf8(octets) {
        Ok(s) => s.to_string(),
        Err(_) => octets.iter().map(|&b| b as char).collect(),
    }
}

/// Reconnaît un export de plateforme ou de portefeuille ; à défaut, le format CSV de Taiseux.
pub fn importer(texte: &str) -> Result<Apercu, String> {
    match imports::importer(texte) {
        Ok(i) => Ok(Apercu { plateforme: i.plateforme, format: i.format, operations: i.operations, a_verifier: i.a_verifier, ignorees: i.ignorees, erreurs: vec![] }),
        Err(message) => {
            let premiere = texte.trim_start_matches('\u{feff}').lines().next().unwrap_or("").to_lowercase();
            if !(premiere.contains("date") && premiere.contains("type")) { return Err(message); }
            let l = csv::lire(texte);
            if l.operations.is_empty() && !l.erreurs.is_empty() { return Err(message); }
            Ok(Apercu { plateforme: "Taiseux".into(), format: "CSV de Taiseux".into(), operations: l.operations, erreurs: l.erreurs, ..Default::default() })
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LectureCours {
    pub cours: Vec<Cours>,
    pub erreurs: Vec<String>,
    /// Vrai si le fichier ne dit pas de quelle crypto il s'agit (CoinGecko, Kraken) et qu'aucune n'a été indiquée.
    pub actif_requis: bool,
    /// Actif deviné d'après le nom du fichier (« btc-eur-max.csv » → BTC, « XBTEUR_1440.csv » → BTC).
    pub actif_devine: String,
}

/// Lit un fichier de cours. `nom` (nom du fichier) sert à deviner l'actif si le contenu ne le dit pas.
pub fn lire_cours(texte: &str, actif: Option<&str>, nom: &str) -> LectureCours {
    let devine = deviner_actif(nom);
    let actif = actif.map(str::trim).filter(|a| !a.is_empty());
    let (cours, erreurs) = csv::lire_cours(texte, actif);
    let actif_requis = cours.is_empty() && actif.is_none() && erreurs.iter().any(|e| e.contains("indiquez de quelle crypto"));
    LectureCours { cours, erreurs, actif_requis, actif_devine: devine }
}

fn deviner_actif(nom: &str) -> String {
    let base = nom.rsplit(['/', '\\']).next().unwrap_or("").to_uppercase();
    let base = base.split('.').next().unwrap_or("");
    // Kraken : « XBTEUR_1440 », « ETHEUR_1440 ».
    if let Some(paire) = base.split('_').next() {
        if let Some(a) = paire.strip_suffix("EUR").filter(|a| (2..=6).contains(&a.len()) && a.chars().all(|c| c.is_ascii_alphanumeric())) {
            return crate::imports::kraken_actif(a);
        }
    }
    // CoinGecko : « btc-eur-max », « eth-eur-max ».
    let morceaux: Vec<&str> = base.split(['-', '_', ' ']).collect();
    if morceaux.len() >= 2 && morceaux[1] == "EUR" && (2..=6).contains(&morceaux[0].len()) { return morceaux[0].to_string(); }
    String::new()
}

/// Ajoute des cours à ceux du dossier : un nouveau cours remplace l'ancien pour le même actif et le même jour.
pub fn fusionner_cours(existants: &[Cours], nouveaux: &[Cours]) -> Vec<Cours> {
    let mut m: std::collections::BTreeMap<(String, String), f64> = existants.iter().map(|c| ((c.actif.clone(), c.date.clone()), c.prix)).collect();
    for c in nouveaux { m.insert((c.actif.trim().to_uppercase(), c.date.clone()), c.prix); }
    m.into_iter().map(|((actif, date), prix)| Cours { actif, date, prix }).collect()
}

/// Plateformes et formats reconnus (pour l'aide).
pub fn plateformes() -> Vec<(String, String)> {
    imports::PLATEFORMES.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devine_l_actif_d_apres_le_nom() {
        assert_eq!(deviner_actif("/x/XBTEUR_1440.csv"), "BTC");
        assert_eq!(deviner_actif("ETHEUR_1440.csv"), "ETH");
        assert_eq!(deviner_actif("C:\\dl\\btc-eur-max.csv"), "BTC");
        assert_eq!(deviner_actif("cours.csv"), "");
    }

    #[test]
    fn encodages() {
        assert_eq!(texte_fichier(b"\xEF\xBB\xBFdate;prix"), "date;prix");
        assert_eq!(texte_fichier(b"Op\xe9ration"), "Opération");
        assert_eq!(texte_fichier(&[0xFF, 0xFE, b'a', 0, 0xE9, 0]), "aé");
    }

    #[test]
    fn import_csv_taiseux_et_format_inconnu() {
        let a = importer("date;type;montant\n2026-01-10;achat;1000\n").unwrap();
        assert_eq!((a.plateforme.as_str(), a.operations.len()), ("Taiseux", 1));
        assert!(importer("foo,bar\n1,2\n").unwrap_err().contains("Format non reconnu"));
    }

    #[test]
    fn cours_fusionnes() {
        let a = vec![Cours { actif: "BTC".into(), date: "2026-01-01".into(), prix: 1.0 }];
        let b = vec![Cours { actif: "btc".into(), date: "2026-01-01".into(), prix: 2.0 }, Cours { actif: "ETH".into(), date: "2026-01-01".into(), prix: 3.0 }];
        let f = fusionner_cours(&a, &b);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].prix, 2.0);
    }
}
