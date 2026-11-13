//! Outils communs aux lecteurs : table CSV, nombres, horodatages, actifs, et fabrication des opérations.

use std::collections::BTreeMap;

use crate::engine::{Operation, TypeOperation};

/// Monnaies officielles : jamais traitées comme des actifs numériques.
pub const FIAT: &[&str] = &["EUR", "USD", "GBP", "CHF", "CAD", "AUD", "JPY", "NZD", "SEK", "NOK", "DKK", "PLN", "CZK", "HUF", "TRY", "BRL", "ZAR", "SGD", "HKD", "MXN", "RON", "BGN", "UAH", "RUB", "AED", "ARS", "NGN", "KRW", "INR", "IDR"];
pub fn est_fiat(a: &str) -> bool { FIAT.contains(&a) }

/// Découpe une ligne CSV (guillemets « " » gérés, guillemets doublés à l'intérieur).
pub fn cellules(ligne: &str, sep: char) -> Vec<String> {
    let mut champs = Vec::new();
    let mut cur = String::new();
    let mut dans = false;
    let mut it = ligne.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if dans && it.peek() == Some(&'"') => { cur.push('"'); it.next(); }
            '"' => dans = !dans,
            c if c == sep && !dans => { champs.push(cur.trim().to_string()); cur.clear(); }
            c => cur.push(c),
        }
    }
    champs.push(cur.trim().to_string());
    champs
}

/// Un fichier CSV dont l'en-tête a été trouvé (les lignes d'introduction qui le précèdent sont sautées).
pub struct Table {
    pub entete: Vec<String>,
    /// (numéro de ligne dans le fichier, cellules, ligne brute)
    pub lignes: Vec<(usize, Vec<String>, String)>,
}

impl Table {
    /// Cherche, dans les 30 premières lignes, une ligne d'en-tête contenant toutes les colonnes `requises`.
    pub fn trouver(texte: &str, requises: &[&str]) -> Option<Table> {
        // L'en-tête se cherche sur le début du fichier seulement : le reste n'est découpé que si le format est le bon
        // (un import essaie tous les lecteurs l'un après l'autre).
        for (i, l) in texte.lines().enumerate().take(30) {
            for sep in [',', ';', '\t'] {
                if !requises.iter().all(|r| l.contains(r)) { continue; }
                let e: Vec<String> = cellules(l, sep).into_iter().map(|c| c.trim_matches('\u{feff}').to_string()).collect();
                if requises.iter().all(|r| e.iter().any(|h| h == r)) {
                    let lignes = texte.lines().enumerate().skip(i + 1)
                        .filter(|(_, l)| !l.trim().is_empty())
                        .map(|(n, l)| (n + 1, cellules(l, sep), l.to_string()))
                        .collect();
                    return Some(Table { entete: e, lignes });
                }
            }
        }
        None
    }
    pub fn col(&self, nom: &str) -> Option<usize> { self.entete.iter().position(|h| h == nom) }
    /// Première colonne dont le nom vérifie `f`.
    pub fn col_si(&self, f: impl Fn(&str) -> bool) -> Option<usize> { self.entete.iter().position(|h| f(h)) }
    pub fn a(&self, nom: &str) -> bool { self.col(nom).is_some() }
}

pub fn champ(c: &[String], i: Option<usize>) -> &str { i.and_then(|i| c.get(i)).map(|s| s.as_str()).unwrap_or("") }

/// Nombre écrit à l'anglaise (« 1,234.56 ») ou à la française (« 1 234,56 »), avec ou sans symbole monétaire.
pub fn nombre(s: &str) -> Option<f64> {
    let t: String = s.chars().filter(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | 'e' | 'E')).collect();
    let t = t.trim_start_matches('+');
    if t.is_empty() || !t.chars().any(|c| c.is_ascii_digit()) { return None; }
    let (point, virgule) = (t.contains('.'), t.contains(','));
    let norm = if point && virgule {
        if t.rfind('.') > t.rfind(',') { t.replace(',', "") } else { t.replace('.', "").replace(',', ".") }
    } else if virgule {
        // « 1,234 » ou « 1,234,567 » : séparateurs de milliers ; « 0,5 » : virgule décimale.
        let parts: Vec<&str> = t.trim_start_matches('-').split(',').collect();
        if parts.len() > 1 && parts[1..].iter().all(|p| p.len() == 3) && parts[0].len() <= 3 && parts[0] != "0" { t.replace(',', "") } else { t.replace(',', ".") }
    } else { t.to_string() };
    norm.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Horodatage → (date AAAA-MM-JJ, heure HH:MM:SS). Formats ISO 8601 (avec « T », « Z », fuseau, fractions de
/// seconde) et « AAAA-MM-JJ HH:MM:SS [UTC] ». La date retenue est celle écrite dans le fichier (le plus souvent UTC).
pub fn horodatage(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    let d = s.get(0..10)?;
    let b = d.as_bytes();
    if !(b[4] == b'-' && b[7] == b'-' && d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())) { return None; }
    let h: String = s.get(11..19).unwrap_or("").chars().filter(|c| c.is_ascii_digit() || *c == ':').collect();
    Some((d.to_string(), if h.len() == 8 { h } else { String::new() }))
}

/// Code d'actif normalisé : majuscules, sans espaces.
pub fn actif(s: &str) -> String { s.trim().to_uppercase() }

fn base(date: &str, heure: &str, kind: TypeOperation, plateforme: &str, note: String) -> Operation {
    Operation { date: date.into(), heure: heure.into(), kind, plateforme: plateforme.into(), note, ..Default::default() }
}
pub fn achat(d: &str, h: &str, p: &str, a: &str, q: f64, montant: f64, note: String) -> Operation {
    Operation { actif: Some(a.into()), quantite: Some(q), montant, ..base(d, h, TypeOperation::Achat, p, note) }
}
pub fn cession(d: &str, h: &str, p: &str, a: &str, q: f64, prix: f64, frais: f64, note: String) -> Operation {
    Operation { actif: Some(a.into()), quantite: Some(q), montant: prix, frais: if frais > 0.0 { Some(frais) } else { None }, ..base(d, h, TypeOperation::Cession, p, note) }
}
pub fn echange(d: &str, h: &str, p: &str, a: &str, qa: f64, b: &str, qb: f64, note: String) -> Operation {
    Operation { actif: Some(a.into()), quantite: Some(qa), actif_recu: Some(b.into()), quantite_recue: Some(qb), ..base(d, h, TypeOperation::Echange, p, note) }
}
pub fn transfert(d: &str, h: &str, p: &str, a: &str, q: f64, frais: f64, note: String) -> Operation {
    Operation { actif: Some(a.into()), quantite: Some(q), quantite_frais: if frais > 0.0 { Some(frais) } else { None }, ..base(d, h, TypeOperation::Transfert, p, note) }
}
pub fn revenu(d: &str, h: &str, p: &str, a: &str, q: f64, note: String) -> Operation {
    Operation { actif: Some(a.into()), quantite: Some(q), ..base(d, h, TypeOperation::RevenuGratuit, p, note) }
}

/// Résultat d'un échange de plateforme (une transaction en plusieurs lignes : actif donné, actif reçu, frais).
pub enum Echange {
    Achat { actif: String, quantite: f64, montant_eur: f64 },
    Cession { actif: String, quantite: f64, prix_eur: f64, frais_eur: f64 },
    Echange { donne: String, qd: f64, recu: String, qr: f64 },
    /// Frais payés dans un troisième actif (ex. BNB) : à retirer des avoirs, sans effet fiscal propre.
    AvecFraisCrypto(Box<Echange>, String, f64),
    Inclassable(String),
}

/// Classe une transaction d'après les mouvements de chaque actif : `bruts` = somme des montants signés (hors frais),
/// `frais` = frais payés (positifs) par actif.
pub fn classer(bruts: &BTreeMap<String, f64>, frais: &BTreeMap<String, f64>) -> Echange {
    let mut net: BTreeMap<String, f64> = bruts.clone();
    for (a, f) in frais { *net.entry(a.clone()).or_insert(0.0) -= f; }
    let donnes: Vec<&String> = net.iter().filter(|(_, v)| **v < -1e-12).map(|(a, _)| a).collect();
    let recus: Vec<&String> = net.iter().filter(|(_, v)| **v > 1e-12).map(|(a, _)| a).collect();
    // Un troisième actif qui n'apparaît qu'en frais (BNB, KFEE…) : mis à part.
    let actifs_echanges: Vec<&String> = bruts.iter().filter(|(_, v)| v.abs() > 1e-12).map(|(a, _)| a).collect();
    let frais_tiers: Vec<(&String, f64)> = frais.iter().filter(|(a, f)| **f > 0.0 && !actifs_echanges.contains(a)).map(|(a, f)| (a, *f)).collect();
    if frais_tiers.len() > 1 { return Echange::Inclassable("frais payés dans plusieurs actifs".into()); }
    if let Some((fa, fq)) = frais_tiers.first() {
        if !est_fiat(fa) {
            let mut b2 = bruts.clone(); b2.retain(|a, _| a != *fa);
            let mut f2 = frais.clone(); f2.remove(*fa);
            return match classer(&b2, &f2) {
                Echange::Inclassable(m) => Echange::Inclassable(m),
                e => Echange::AvecFraisCrypto(Box::new(e), (*fa).clone(), *fq),
            };
        }
    }
    let donnes: Vec<&String> = donnes.into_iter().filter(|a| !frais_tiers.iter().any(|(f, _)| f == a)).collect();
    if donnes.len() != 1 || recus.len() != 1 {
        return Echange::Inclassable(format!("transaction à {} actif(s) donné(s) et {} reçu(s)", donnes.len(), recus.len()));
    }
    let (d, r) = (donnes[0].clone(), recus[0].clone());
    let (qd, qr) = (-net[&d], net[&r]);
    match (est_fiat(&d), est_fiat(&r)) {
        (true, false) if d == "EUR" => Echange::Achat { actif: r, quantite: qr, montant_eur: qd },
        (false, true) if r == "EUR" => Echange::Cession {
            actif: d.clone(), quantite: qd, prix_eur: bruts.get(&r).copied().unwrap_or(qr), frais_eur: frais.get(&r).copied().unwrap_or(0.0),
        },
        (false, false) => Echange::Echange { donne: d, qd, recu: r, qr },
        (true, true) => Echange::Inclassable("change entre deux monnaies officielles".into()),
        _ => Echange::Inclassable(format!("montant en {} : à convertir en euros et à saisir", if est_fiat(&d) { &d } else { &r })),
    }
}

/// Traduit un échange classé en opérations.
pub fn operations_echange(e: Echange, d: &str, h: &str, p: &str, note: &str) -> Result<Vec<Operation>, String> {
    Ok(match e {
        Echange::Achat { actif, quantite, montant_eur } => vec![achat(d, h, p, &actif, quantite, montant_eur, note.into())],
        Echange::Cession { actif, quantite, prix_eur, frais_eur } => vec![cession(d, h, p, &actif, quantite, prix_eur, frais_eur, note.into())],
        Echange::Echange { donne, qd, recu, qr } => vec![echange(d, h, p, &donne, qd, &recu, qr, note.into())],
        Echange::AvecFraisCrypto(e, fa, fq) => {
            let mut v = operations_echange(*e, d, h, p, note)?;
            v.push(transfert(d, h, p, &fa, 0.0, fq, format!("frais payés en {fa}")));
            v
        }
        Echange::Inclassable(m) => return Err(m),
    })
}
