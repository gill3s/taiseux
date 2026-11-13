//! Coffre chiffré : un seul fichier, chiffré par une phrase de passe. Rien n'est jamais écrit en clair.
//!
//! Format (version 2 depuis Taiseux 0.2 ; la version 1 de Taiseux 0.1 reste lisible) :
//! ```text
//! "TAISEUX" 0x02 | m_cost u32 LE (Kio) | t_cost u32 LE | p_cost u32 LE | sel (16 o) | nonce (24 o) | chiffré
//! ```
//! Clé = Argon2id(phrase, sel, m_cost, t_cost, p_cost) sur 32 octets ; chiffrement XChaCha20-Poly1305, l'en-tête
//! entier (tout ce qui précède le chiffré) étant authentifié comme données associées : modifier un paramètre ou le
//! sel fait échouer l'ouverture. Contenu = le dossier en JSON. L'octet de version passe à 2 avec la 0.2 (foyer, cours,
//! comptes) pour qu'une 0.1 refuse ce coffre au lieu d'en perdre une partie en l'enregistrant. Clé et clair sont effacés de la mémoire après usage.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::engine::{Cours, Operation};
use crate::imports::AVerifier;

const MAGIC: &[u8; 8] = b"TAISEUX\x02";
/// Versions de format que cette version sait ouvrir.
const VERSIONS_LUES: &[u8] = &[1, 2];
const ENTETE: usize = 8 + 12 + 16 + 24;
/// Paramètres Argon2id par défaut : 64 Mio, 3 passes, 1 fil (≈ 0,3 à 1 s selon la machine).
pub const M_COST: u32 = 64 * 1024;
pub const T_COST: u32 = 3;
pub const P_COST: u32 = 1;
/// Longueur minimale de la phrase de passe, en caractères.
pub const PHRASE_MIN: usize = 10;

/// Un membre du foyer fiscal et ses opérations (chacun a son propre portefeuille).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Membre {
    /// « declarant1 », « declarant2 » ou « personne_a_charge » (lignes 21x, 25x, 31x du 2086).
    pub role: String,
    /// Prénom ou libellé libre, pour s'y retrouver.
    #[serde(default)]
    pub nom: String,
    #[serde(default)]
    pub operations: Vec<Operation>,
    /// Lignes d'imports que Taiseux n'a pas su traduire, gardées jusqu'à ce que l'utilisateur les traite.
    #[serde(default)]
    pub a_verifier: Vec<AVerifier>,
}

/// Compte d'actifs numériques ouvert, détenu, utilisé ou clos à l'étranger (formulaire 3916-bis).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompteEtranger {
    /// Indice du membre titulaire dans `membres`.
    #[serde(default)]
    pub titulaire: usize,
    #[serde(default)]
    pub plateforme: String,
    #[serde(default)]
    pub adresse: String,
    #[serde(default)]
    pub site: String,
    #[serde(default)]
    pub identifiant: String,
    #[serde(default)]
    pub ouverture: String,
    #[serde(default)]
    pub cloture: String,
    #[serde(default)]
    pub note: String,
}

/// Ce que contient un coffre.
/// Version 1 (Taiseux 0.1) : `operations` d'un seul déclarant. Version 2 : `membres`, `cours`, `comptes` ;
/// `operations` reste vide. [`ouvrir`] convertit une version 1 en version 2.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dossier {
    pub version: u32,
    #[serde(default)]
    pub operations: Vec<Operation>,
    #[serde(default)]
    pub membres: Vec<Membre>,
    /// Cours importés ou saisis par l'utilisateur.
    #[serde(default)]
    pub cours: Vec<Cours>,
    #[serde(default)]
    pub comptes: Vec<CompteEtranger>,
}

impl Dossier {
    /// Forme version 2 : les opérations d'une version 1 deviennent celles du déclarant 1.
    pub fn normaliser(mut self) -> Self {
        if self.membres.is_empty() || !self.operations.is_empty() {
            let ops = std::mem::take(&mut self.operations);
            if self.membres.is_empty() {
                self.membres.push(Membre { role: "declarant1".into(), ..Default::default() });
            }
            self.membres[0].operations.splice(0..0, ops);
        }
        self.version = 2;
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErreurCoffre {
    PhraseTropCourte,
    PasUnCoffre,
    VersionInconnue,
    ParametresInvalides,
    /// Mauvaise phrase de passe, ou fichier modifié ou abîmé : impossible de distinguer les deux, et c'est voulu.
    Dechiffrement,
    Contenu(String),
}

impl std::fmt::Display for ErreurCoffre {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PhraseTropCourte => write!(f, "La phrase de passe doit compter au moins {PHRASE_MIN} caractères."),
            Self::PasUnCoffre => write!(f, "Ce fichier n'est pas un coffre Taiseux."),
            Self::VersionInconnue => write!(f, "Ce coffre vient d'une version plus récente de Taiseux."),
            Self::ParametresInvalides => write!(f, "Les paramètres de ce coffre sont invalides."),
            Self::Dechiffrement => write!(f, "Phrase de passe incorrecte, ou fichier modifié ou abîmé."),
            Self::Contenu(e) => write!(f, "Contenu du coffre illisible : {e}"),
        }
    }
}

fn cle(phrase: &str, sel: &[u8], m: u32, t: u32, p: u32) -> Result<Zeroizing<[u8; 32]>, ErreurCoffre> {
    // Bornes : refuser un fichier qui imposerait une mémoire absurde (déni de service) ou des paramètres trop faibles.
    if !(8 * 1024..=1024 * 1024).contains(&m) || !(1..=20).contains(&t) || !(1..=8).contains(&p) {
        return Err(ErreurCoffre::ParametresInvalides);
    }
    let params = Params::new(m, t, p, Some(32)).map_err(|_| ErreurCoffre::ParametresInvalides)?;
    let mut k = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(phrase.as_bytes(), sel, k.as_mut())
        .map_err(|_| ErreurCoffre::ParametresInvalides)?;
    Ok(k)
}

/// Chiffre un dossier. Sel et nonce neufs à chaque écriture.
pub fn sceller(dossier: &Dossier, phrase: &str) -> Result<Vec<u8>, ErreurCoffre> {
    sceller_avec(dossier, phrase, M_COST, T_COST, P_COST)
}

/// Comme [`sceller`], avec des paramètres Argon2id choisis (les tests utilisent des paramètres légers).
pub fn sceller_avec(dossier: &Dossier, phrase: &str, m: u32, t: u32, p: u32) -> Result<Vec<u8>, ErreurCoffre> {
    if phrase.chars().count() < PHRASE_MIN { return Err(ErreurCoffre::PhraseTropCourte); }
    let mut sel = [0u8; 16];
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut sel);
    OsRng.fill_bytes(&mut nonce);
    let mut out = Vec::with_capacity(ENTETE + 1024);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&m.to_le_bytes());
    out.extend_from_slice(&t.to_le_bytes());
    out.extend_from_slice(&p.to_le_bytes());
    out.extend_from_slice(&sel);
    out.extend_from_slice(&nonce);
    let k = cle(phrase, &sel, m, t, p)?;
    let clair = Zeroizing::new(serde_json::to_vec(&dossier.clone().normaliser()).map_err(|e| ErreurCoffre::Contenu(e.to_string()))?);
    let chiffre = XChaCha20Poly1305::new(k.as_ref().into())
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: &clair, aad: &out })
        .map_err(|_| ErreurCoffre::Dechiffrement)?;
    out.extend_from_slice(&chiffre);
    Ok(out)
}

/// Déchiffre un coffre.
pub fn ouvrir(octets: &[u8], phrase: &str) -> Result<Dossier, ErreurCoffre> {
    if octets.len() < ENTETE + 16 || &octets[..7] != &MAGIC[..7] { return Err(ErreurCoffre::PasUnCoffre); }
    if !VERSIONS_LUES.contains(&octets[7]) { return Err(ErreurCoffre::VersionInconnue); }
    let u = |i: usize| u32::from_le_bytes(octets[i..i + 4].try_into().unwrap());
    let (m, t, p) = (u(8), u(12), u(16));
    let sel = &octets[20..36];
    let nonce = &octets[36..60];
    let k = cle(phrase, sel, m, t, p)?;
    let clair = Zeroizing::new(
        XChaCha20Poly1305::new(k.as_ref().into())
            .decrypt(XNonce::from_slice(nonce), Payload { msg: &octets[ENTETE..], aad: &octets[..ENTETE] })
            .map_err(|_| ErreurCoffre::Dechiffrement)?,
    );
    serde_json::from_slice::<Dossier>(&clair).map(Dossier::normaliser).map_err(|e| ErreurCoffre::Contenu(e.to_string()))
}
