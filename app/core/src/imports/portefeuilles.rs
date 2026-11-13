//! Outils communs aux lecteurs des portefeuilles personnels (Ledger Live, Trezor Suite, Electrum, Exodus, Sparrow,
//! Coinomi) : avertissements des envois et réceptions, dates propres à ces exports, unités de base.
//!
//! **Envois et réceptions.** Un portefeuille voit des mouvements, pas leur cause. Taiseux les traduit en
//! **transferts** (sans effet fiscal ; les frais de réseau en crypto diminuent les avoirs), et la note de chaque
//! opération dit ce qui a été supposé :
//! - une réception est supposée venir d'un autre compte de l'utilisateur (plateforme, autre portefeuille) ; si c'est
//!   un achat de gré à gré, un paiement reçu ou un don, l'utilisateur doit la remplacer par un achat ou un revenu ;
//! - un envoi est supposé aller vers un autre compte de l'utilisateur ; si c'est le paiement d'un bien ou d'un
//!   service, c'est une cession imposable à saisir.
//! Une réception qui ne peut venir d'aucun compte de l'utilisateur (récompense de minage, par exemple) part dans
//! « à vérifier » quand l'export ne permet pas de la classer avec certitude.

/// Note d'une réception (transfert entrant supposé).
pub fn note_reception(portefeuille: &str, detail: &str) -> String {
    format!(
        "{portefeuille} : réception{} — supposée venir de l'un de vos comptes (transfert, sans effet fiscal). \
         Si c'est un achat de gré à gré, un paiement reçu ou un don, remplacez-la par un achat ou un revenu.",
        if detail.is_empty() { String::new() } else { format!(" ({detail})") }
    )
}

/// Note d'un envoi (transfert sortant supposé).
pub fn note_envoi(portefeuille: &str, detail: &str) -> String {
    format!(
        "{portefeuille} : envoi{} — supposé aller vers l'un de vos comptes (transfert, sans effet fiscal). \
         Si c'est le paiement d'un bien ou d'un service, c'est une cession imposable à saisir ; un don se saisit à part.",
        if detail.is_empty() { String::new() } else { format!(" ({detail})") }
    )
}

/// Secondes depuis le 01/01/1970 (UTC) → (AAAA-MM-JJ, HH:MM:SS).
pub fn date_unix(s: &str) -> Option<(String, String)> {
    let t: i64 = s.trim().split('.').next()?.parse().ok()?;
    if !(0..=32_503_680_000).contains(&t) { return None; }
    let (jours, sec) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    // Jours depuis l'époque → date civile (algorithme « days from civil » inversé, calendrier grégorien proleptique).
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = yoe + ere * 400 + if mois <= 2 { 1 } else { 0 };
    Some((format!("{annee:04}-{mois:02}-{jour:02}"), format!("{:02}:{:02}:{:02}", sec / 3600, sec % 3600 / 60, sec % 60)))
}

const MOIS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Date écrite par JavaScript (`Date.toString()`), ex. « Fri Jan 01 2021 12:00:00 GMT+0100 (heure normale d'Europe
/// centrale) » : la date et l'heure retenues sont celles écrites (heure locale de l'ordinateur qui a exporté).
pub fn date_js(s: &str) -> Option<(String, String)> {
    let m: Vec<&str> = s.split_whitespace().collect();
    if m.len() < 5 { return None; }
    let mois = MOIS.iter().position(|x| x.eq_ignore_ascii_case(m[1]))? + 1;
    let jour: u32 = m[2].parse().ok().filter(|j| (1..=31).contains(j))?;
    let annee: u32 = m[3].parse().ok().filter(|a| (1970..=9999).contains(a))?;
    let h = m[4];
    let heure = if h.len() == 8 && h.chars().enumerate().all(|(i, c)| if i == 2 || i == 5 { c == ':' } else { c.is_ascii_digit() }) { h.to_string() } else { String::new() };
    Some((format!("{annee:04}-{mois:02}-{jour:02}"), heure))
}

/// « JJ.MM.AAAA [HH:MM:SS] » (format CoinTracking, repris par l'export de staking d'Exodus).
pub fn date_jma(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    let d = s.get(0..10)?;
    let p: Vec<&str> = d.split('.').collect();
    if p.len() != 3 || p[0].len() != 2 || p[1].len() != 2 || p[2].len() != 4 || !p.iter().all(|x| x.chars().all(|c| c.is_ascii_digit())) { return None; }
    let h: String = s.get(11..19).unwrap_or("").to_string();
    let h = if h.len() == 8 && h.chars().all(|c| c.is_ascii_digit() || c == ':') { h } else { String::new() };
    Some((format!("{}-{}-{}", p[2], p[1], p[0]), h))
}

/// Unités de base connues → (actif, diviseur) : « sat » / « sats » / « satoshi(s) » → BTC / 1e8.
pub fn unite_de_base(u: &str) -> Option<(&'static str, f64)> {
    match u.trim().to_lowercase().as_str() {
        "sat" | "sats" | "satoshi" | "satoshis" => Some(("BTC", 1e8)),
        _ => None,
    }
}

/// Un code d'actif utilisable : lettres et chiffres (avec « . », « - », « _ »), 1 à 12 caractères, et pas une adresse
/// de contrat (« 0x… »). Sinon le jeton est illisible : la ligne part dans « à vérifier ».
pub fn actif_lisible(a: &str) -> bool {
    !a.is_empty() && a.len() <= 12 && !a.starts_with("0X")
        && a.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Mouvement d'un portefeuille bitcoin dont l'export donne la **variation nette** du portefeuille (`v`, frais compris
/// pour un envoi) et, quand toutes les entrées lui appartiennent, les frais (`frais`). Électrum et Sparrow.
/// Renvoie `None` si la variation est nulle (rien à importer).
pub fn mouvement_bitcoin(d: &str, h: &str, p: &str, v: f64, frais: Option<f64>, detail: &str) -> Option<crate::engine::Operation> {
    use super::commun::transfert;
    if v > 0.0 { return Some(transfert(d, h, p, "BTC", v, 0.0, note_reception(p, detail))); }
    if v == 0.0 { return None; }
    let sortie = -v;
    Some(match frais {
        Some(f) if f > 0.0 && sortie <= f * (1.0 + 1e-9) + 1e-12 =>
            transfert(d, h, p, "BTC", 0.0, sortie, format!("{p} : envoi entre vos propres adresses (consolidation), seuls les frais sortent")),
        Some(f) if f > 0.0 => transfert(d, h, p, "BTC", sortie - f, f, note_envoi(p, detail)),
        _ => transfert(d, h, p, "BTC", sortie, 0.0, format!(
            "{} Frais non indiqués par l'export (ancien format, ou entrées d'autres participants : coinjoin, payjoin) : ils ne sont pas retirés de vos avoirs, ajoutez-les si besoin.",
            note_envoi(p, detail)
        )),
    })
}
