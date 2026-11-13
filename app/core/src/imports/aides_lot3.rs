//! Outils partagés par les lecteurs du lot 3 (KuCoin, Bybit, OKX, Gate.io, MEXC, Bitfinex, Gemini, Uphold, Nexo) :
//! fuseaux horaires, dates en toutes lettres, paires de cotation, ordres simples et transactions en plusieurs lignes.
//! Tout passe par `classer` (commun.rs) : un montant dans une autre monnaie que l'euro y devient « à vérifier ».

use std::collections::BTreeMap;

use super::commun::*;
use super::{AVerifier, Import};

/// Ajoute une ligne « à vérifier ».
pub fn verifier(imp: &mut Import, ligne: usize, date: &str, motif: impl Into<String>, contenu: &str) {
    imp.a_verifier.push(AVerifier { ligne, date: date.into(), motif: motif.into(), contenu: contenu.into() });
}

/// Nombre de jours depuis le 01/01/1970 (calendrier grégorien proleptique).
fn jours(a: i64, m: i64, j: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let ere = a.div_euclid(400);
    let ae = a - ere * 400;
    let jda_ = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + j - 1;
    let jde = ae * 365 + ae / 4 - ae / 100 + jda_;
    ere * 146097 + jde - 719468
}

fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let ere = z.div_euclid(146097);
    let jde = z - ere * 146097;
    let ae = (jde - jde / 1460 + jde / 36524 - jde / 146096) / 365;
    let jda = jde - (365 * ae + ae / 4 - ae / 100);
    let mp = (5 * jda + 2) / 153;
    let j = jda - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (ae + ere * 400 + if m <= 2 { 1 } else { 0 }, m, j)
}

/// Date valide ?
pub fn date_valide(a: i64, m: i64, j: i64) -> bool {
    (1..=12).contains(&m) && j >= 1 && civil(jours(a, m, j)) == (a, m, j)
}

/// Ramène une date et une heure locales à l'heure UTC, connaissant le décalage du fuseau (en minutes, ex. +480 pour
/// UTC+08:00). Sans heure, la date est rendue telle quelle.
pub fn vers_utc(d: &str, h: &str, decalage_min: i64) -> (String, String) {
    if decalage_min == 0 || h.len() != 8 { return (d.to_string(), h.to_string()); }
    let n = |s: &str| s.parse::<i64>().unwrap_or(0);
    let (a, m, j) = (n(&d[0..4]), n(&d[5..7]), n(&d[8..10]));
    let s = jours(a, m, j) * 86400 + n(&h[0..2]) * 3600 + n(&h[3..5]) * 60 + n(&h[6..8]) - decalage_min * 60;
    let (a, m, j) = civil(s.div_euclid(86400));
    let r = s.rem_euclid(86400);
    (format!("{a:04}-{m:02}-{j:02}"), format!("{:02}:{:02}:{:02}", r / 3600, r % 3600 / 60, r % 60))
}

/// « UTC+08:00 », « UTC-05:30 », « UTC », « GMT+0100 » → décalage en minutes.
pub fn decalage(s: &str) -> Option<i64> {
    let s = s.trim();
    let r = s.strip_prefix("UTC").or_else(|| s.strip_prefix("GMT"))?;
    if r.is_empty() { return Some(0); }
    let signe = match r.chars().next()? { '+' => 1, '-' => -1, _ => return None };
    let ch: String = r[1..].chars().filter(|c| c.is_ascii_digit()).collect();
    let (hh, mm) = match ch.len() { 1 | 2 => (ch.parse::<i64>().ok()?, 0), 4 => (ch[0..2].parse().ok()?, ch[2..4].parse::<i64>().ok()?), _ => return None };
    Some(signe * (hh * 60 + mm))
}

/// Colonne d'horodatage dont le nom porte le fuseau, ex. « Time(UTC+08:00) » ou « Filled Time(UTC) » :
/// (index, décalage en minutes). `prefixe` = « Time », « Filled Time »…
pub fn colonne_fuseau(t: &Table, prefixe: &str) -> Option<(usize, i64)> {
    t.entete.iter().enumerate().find_map(|(i, h)| {
        let r = h.strip_prefix(prefixe)?.strip_prefix('(')?.strip_suffix(')')?;
        Some((i, decalage(r)?))
    })
}

/// Horodatage lu puis ramené en UTC.
pub fn horodatage_utc(s: &str, decalage_min: i64) -> Option<(String, String)> {
    let (d, h) = horodatage(s)?;
    Some(vers_utc(&d, &h, decalage_min))
}

/// Mois anglais abrégé → numéro.
pub fn mois_anglais(s: &str) -> Option<i64> {
    const M: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    let s = s.to_lowercase();
    M.iter().position(|m| s.starts_with(m)).map(|i| i as i64 + 1)
}

/// Sépare une paire de cotation : d'abord par un séparateur (« BTC-USDT », « BTC/EUR », « BTC_USDT »), sinon par une
/// monnaie de cotation connue en suffixe (`cotations` d'abord, puis la liste de Binance).
pub fn paire(p: &str, cotations: &[&str]) -> Option<(String, String)> {
    let p = p.trim().to_uppercase();
    for sep in ['-', '/', '_', ':'] {
        if let Some((b, q)) = p.split_once(sep) {
            return if b.is_empty() || q.is_empty() { None } else { Some((b.to_string(), q.to_string())) };
        }
    }
    let mut c: Vec<&str> = cotations.to_vec();
    c.sort_by_key(|q| std::cmp::Reverse(q.len()));
    c.iter().find(|q| p.len() > q.len() && p.ends_with(**q)).map(|q| (p[..p.len() - q.len()].to_string(), q.to_string()))
        .or_else(|| super::binance::separer_paire(&p))
}

/// Ordre au comptant sur une paire : achat (`achat_ = true`) ou vente de `qb` unités de `base` contre `qq` unités de
/// `cote` ; frais éventuels (actif, quantité). Les opérations produites, ou la ligne « à vérifier », vont dans `imp`.
#[allow(clippy::too_many_arguments)]
pub fn ordre(imp: &mut Import, n: usize, brut: &str, d: &str, h: &str, p: &str, note: &str,
             base: &str, qb: f64, cote: &str, qq: f64, achat_: bool, frais: Option<(String, f64)>) {
    let (mut bruts, mut f) = (BTreeMap::new(), BTreeMap::new());
    bruts.insert(base.to_string(), if achat_ { qb.abs() } else { -qb.abs() });
    *bruts.entry(cote.to_string()).or_insert(0.0) += if achat_ { -qq.abs() } else { qq.abs() };
    if let Some((a, q)) = frais { if q.abs() > 0.0 && !a.is_empty() { f.insert(a, q.abs()); } }
    pousser(imp, classer(&bruts, &f), n, brut, d, h, p, note);
}

/// Échange direct : `qd` unités de `donne` contre `qr` unités de `recu` (frais déjà déduits ou indiqués à part).
#[allow(clippy::too_many_arguments)]
pub fn conversion(imp: &mut Import, n: usize, brut: &str, d: &str, h: &str, p: &str, note: &str,
                  donne: &str, qd: f64, recu: &str, qr: f64, frais: Option<(String, f64)>) {
    ordre(imp, n, brut, d, h, p, note, recu, qr, donne, qd, true, frais);
}

/// Traduit un échange classé, ou l'envoie « à vérifier ».
#[allow(clippy::too_many_arguments)]
pub fn pousser(imp: &mut Import, e: Echange, n: usize, brut: &str, d: &str, h: &str, p: &str, note: &str) {
    match operations_echange(e, d, h, p, note) {
        Ok(ops) => imp.operations.extend(ops),
        Err(motif) => verifier(imp, n, d, motif, brut),
    }
}

/// Lignes d'une même transaction (actif, montant signé, est-ce un frais ?), regroupées puis classées.
#[allow(clippy::too_many_arguments)]
pub fn transaction(imp: &mut Import, n: usize, brut: &str, d: &str, h: &str, p: &str, note: &str, lignes: &[(String, f64, bool)]) {
    let (mut bruts, mut frais) = (BTreeMap::new(), BTreeMap::new());
    for (a, q, est_frais) in lignes {
        if *est_frais { *frais.entry(a.clone()).or_insert(0.0) += q.abs(); } else { *bruts.entry(a.clone()).or_insert(0.0) += q; }
    }
    pousser(imp, classer(&bruts, &frais), n, brut, d, h, p, note);
}

/// Dépôt (quantité > 0) ou retrait (< 0) : monnaie officielle → ignoré ; actif numérique → transfert entre ses
/// propres comptes. `frais` = frais de retrait, dans le même actif.
#[allow(clippy::too_many_arguments)]
pub fn mouvement(imp: &mut Import, n: usize, brut: &str, d: &str, h: &str, p: &str, a: &str, q: f64, frais: f64, note: String) {
    if est_fiat(a) { imp.ignorees += 1; return; }
    if !q.is_finite() || a.is_empty() { verifier(imp, n, d, "quantité ou actif illisible", brut); return; }
    imp.operations.push(transfert(d, h, p, a, q.abs(), frais.abs(), note));
}
