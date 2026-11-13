//! Calcul des plus ou moins-values de cession d'actifs numériques des particuliers (France),
//! article 150 VH bis du CGI, doctrine BOI-RPPM-PVBMC-30, formulaire 2086 et sa notice.
//!
//! ```text
//! plus-value = (prix de cession ± soulte − frais) − PTA_net × (prix de cession ± soulte) / valeur globale du portefeuille
//! PTA_net    = prix total d'acquisition − fractions de capital initial déjà imputées − soultes reçues antérieures
//! ```
//!
//! - Les frais réduisent le prix de cession mais pas le quotient (BOFiP § 50 ; notice du 2086).
//! - Seuil d'exonération : total annuel des prix de cession **nets de frais** ≤ 305 € (B du II de l'article ;
//!   ligne 51 du 2086 = somme des lignes 218). Pour un foyer, voir [`foyer`].
//! - Une moins-value nette de l'année n'est ni imputable ailleurs ni reportable (BOFiP § 170).
//!
//! **Version 0.2 : suivi par actif.** Une opération peut porter un actif et une quantité. Le moteur suit alors les
//! quantités détenues, et la valeur globale du portefeuille à chaque cession se calcule seule :
//! somme, pour chaque actif détenu juste avant la cession, de la quantité × le cours du jour. Une valeur globale
//! saisie à la main prime toujours. Les opérations sans actif (montants en euros seuls, version 0.1) restent
//! valables ; dès qu'il y en a une, le portefeuille n'est plus connu en entier et la valeur globale doit être saisie.
//!
//! Fonctions pures : aucun accès disque ni réseau.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Seuil d'exonération annuel, en euros.
pub const SEUIL_EXONERATION: f64 = 305.0;
const EPS: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TypeOperation {
    #[default]
    /// Acquisition contre euros, biens ou services. `montant` = prix payé, frais compris.
    Achat,
    /// Cession imposable contre euros, un bien ou un service. `montant` = prix de cession (ligne 213).
    Cession,
    /// Soulte reçue lors d'un échange crypto contre crypto antérieur (réduit le prix total d'acquisition, ligne 222).
    /// Forme de la version 0.1 ; avec le suivi par actif, préférer `Echange` avec une soulte.
    SoulteRecue,
    /// Échange crypto contre crypto. Sans soulte : sursis d'imposition, seules les quantités changent.
    /// Avec soulte (reçue > 0, versée < 0) : cession imposable (lignes 213 à 218, ligne 216 = soulte), puis
    /// acquisition des actifs reçus pour leur valeur (`valeur_recue`), comme l'exemple du BOFiP § 120.
    Echange,
    /// Transfert entre ses propres comptes ou portefeuilles : aucun effet fiscal. Des frais de réseau payés en
    /// crypto (`quantite_frais`) diminuent les avoirs.
    Transfert,
    /// Actifs reçus sans contrepartie (airdrop, récompense, don reçu) : la quantité s'ajoute aux avoirs ; `montant`
    /// est le prix d'acquisition retenu (0 par défaut).
    RevenuGratuit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Operation {
    /// Date au format AAAA-MM-JJ.
    pub date: String,
    /// Heure HH:MM:SS (facultative) : départage les opérations d'un même jour (imports de plateformes).
    #[serde(default)]
    pub heure: String,
    #[serde(rename = "type")]
    pub kind: TypeOperation,
    /// Montant en euros : prix payé (achat), prix de cession (cession), soulte (soulte reçue), prix d'acquisition
    /// retenu (revenu gratuit). Ignoré pour un échange et un transfert.
    #[serde(default)]
    pub montant: f64,
    /// Valeur globale du portefeuille juste avant la cession, saisie à la main (prime sur le calcul).
    #[serde(default)]
    pub valeur_portefeuille: Option<f64>,
    /// Frais de cession, en euros.
    #[serde(default)]
    pub frais: Option<f64>,
    /// Actif concerné (cédé, acheté, donné en échange, transféré, reçu), ex. « BTC ».
    #[serde(default)]
    pub actif: Option<String>,
    /// Quantité de cet actif.
    #[serde(default)]
    pub quantite: Option<f64>,
    /// Échange : actif reçu, sa quantité et sa valeur en euros (obligatoire si l'échange comporte une soulte).
    #[serde(default)]
    pub actif_recu: Option<String>,
    #[serde(default)]
    pub quantite_recue: Option<f64>,
    #[serde(default)]
    pub valeur_recue: Option<f64>,
    /// Soulte en euros lors d'une cession ou d'un échange : positive si reçue, négative si versée (ligne 216).
    #[serde(default)]
    pub soulte: Option<f64>,
    /// Transfert : frais de réseau payés en crypto (même actif), en quantité.
    #[serde(default)]
    pub quantite_frais: Option<f64>,
    /// Plateforme ou portefeuille (information, et pour le 3916-bis).
    #[serde(default)]
    pub plateforme: String,
    /// Note libre, jamais utilisée dans le calcul.
    #[serde(default)]
    pub note: String,
}

/// Cours d'un actif un jour donné, en euros pour une unité.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cours {
    pub actif: String,
    pub date: String,
    pub prix: f64,
}

/// Une cession et toutes les lignes du formulaire 2086 qui la concernent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cession {
    /// Numéro de l'opération dans la saisie (à partir de 1).
    pub operation: usize,
    pub annee: i32,
    /// 211
    pub date: String,
    /// 212
    pub valeur_portefeuille: f64,
    /// La valeur globale a-t-elle été calculée par Taiseux (sinon : saisie) ?
    pub valeur_calculee: bool,
    /// 213
    pub prix_cession: f64,
    /// 214
    pub frais: f64,
    /// 215 = 213 − 214
    pub prix_net_frais: f64,
    /// 216 : soulte reçue (+) ou versée (−) lors de la cession
    pub soulte_cession: f64,
    /// 217 = 213 ± 216
    pub prix_net_soultes: f64,
    /// 218 = 213 − 214 ± 216
    pub prix_net_frais_soultes: f64,
    /// 220
    pub prix_total_acquisition: f64,
    /// 221
    pub fractions_capital_initial: f64,
    /// 222
    pub soultes_anterieures: f64,
    /// 223 = 220 − 221 − 222
    pub pta_net: f64,
    /// 223 × 217 / 212 : fraction de capital initial imputée par cette cession.
    pub fraction_imputee: f64,
    /// 218 − 223 × 217 / 212
    pub plus_value: f64,
    /// Actif cédé, s'il est connu.
    pub actif: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annee {
    pub annee: i32,
    pub nb_cessions: usize,
    /// Ligne 51 (pour un déclarant seul) : total des prix de cession nets de frais et soultes (lignes 218).
    pub total_prix_cession: f64,
    pub plus_values: f64,
    pub moins_values: f64,
    /// Ligne 224 : plus ou moins-value globale du déclarant (et ligne 52 s'il est seul).
    pub resultat_net: f64,
    pub exonere: bool,
    /// Montant imposable : 0 si exonéré ou si moins-value nette.
    pub imposable: f64,
    /// Case de la 2042 C où reporter le résultat : "3AN", "3BN" ou "" (exonéré, rien à reporter).
    pub case_2042c: String,
}

/// Un cours qui manque pour calculer la valeur globale d'une cession.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoursManquant {
    pub actif: String,
    pub date: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Resultat {
    pub cessions: Vec<Cession>,
    pub annees: Vec<Annee>,
    pub erreurs: Vec<String>,
    pub avertissements: Vec<String>,
    /// Cours à fournir pour que Taiseux calcule les valeurs globales manquantes (triés, sans doublon).
    pub cours_manquants: Vec<CoursManquant>,
    /// Quantités détenues après la dernière opération, par actif (actifs suivis seulement).
    pub avoirs: BTreeMap<String, f64>,
}

fn date_valide(d: &str) -> Option<i32> {
    let b = d.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' { return None; }
    if !d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) { return None; }
    let annee: i32 = d[0..4].parse().ok()?;
    let mois: u32 = d[5..7].parse().ok()?;
    let jour: u32 = d[8..10].parse().ok()?;
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) { return None; }
    Some(annee)
}

/// Nom d'actif normalisé : majuscules, sans espaces. Vide = pas d'actif.
pub fn actif_norm(a: &Option<String>) -> Option<String> {
    let s = a.as_deref().unwrap_or("").trim().to_uppercase();
    if s.is_empty() { None } else { Some(s) }
}

fn positif(x: Option<f64>) -> Option<f64> { x.filter(|v| v.is_finite() && *v > 0.0) }

/// Calcule sans cours fournis (valeurs globales saisies, ou déduites des prix des opérations du jour).
pub fn calculer(ops: &[Operation]) -> Resultat { calculer_avec_cours(ops, &[]) }

/// Calcule toutes les cessions et le résultat de chaque année. Les opérations sont triées par date ; à date égale,
/// l'ordre de saisie est conservé. `cours` sert à valoriser le portefeuille quand la valeur globale n'est pas saisie.
pub fn calculer_avec_cours(ops: &[Operation], cours: &[Cours]) -> Resultat {
    let mut r = Resultat::default();
    let mut ordre: Vec<usize> = (0..ops.len()).collect();
    ordre.sort_by(|&a, &b| ops[a].date.cmp(&ops[b].date).then(ops[a].heure.cmp(&ops[b].heure)).then(a.cmp(&b)));

    // Cours fournis, puis cours déduits des opérations elles-mêmes (prix unitaire d'un achat ou d'une cession du
    // même jour, valeur reçue d'un échange) : ils ne remplacent jamais un cours fourni.
    // Seuls les jours de cession ou d'échange demandent une valorisation : les autres cours sont écartés d'emblée
    // (un fichier de cours quotidiens compte des milliers de lignes par crypto). Tables par actif puis par date,
    // pour chercher sans allouer.
    let jours_utiles: HashSet<&str> = ops.iter()
        .filter(|o| matches!(o.kind, TypeOperation::Cession | TypeOperation::Echange))
        .map(|o| o.date.as_str()).collect();
    let mut table: HashMap<String, HashMap<&str, f64>> = HashMap::new();
    for c in cours {
        if !jours_utiles.contains(c.date.as_str()) { continue; }
        if c.prix.is_finite() && c.prix >= 0.0 {
            let a = c.actif.trim();
            let par_date = match table.get_mut(a) {
                Some(t) => t,
                None => table.entry(a.to_uppercase()).or_default(),
            };
            par_date.insert(c.date.as_str(), c.prix);
        }
    }
    let mut deduits: HashMap<String, HashMap<&str, f64>> = HashMap::new();
    for o in ops {
        if !jours_utiles.contains(o.date.as_str()) { continue; }
        let q = positif(o.quantite);
        match (o.kind, actif_norm(&o.actif), q) {
            (TypeOperation::Achat | TypeOperation::Cession, Some(a), Some(q)) if o.montant.is_finite() && o.montant > 0.0 => {
                deduits.entry(a).or_default().entry(o.date.as_str()).or_insert(o.montant / q);
            }
            _ => {}
        }
        if o.kind == TypeOperation::Echange {
            if let (Some(a), Some(q), Some(v)) = (actif_norm(&o.actif_recu), positif(o.quantite_recue), positif(o.valeur_recue)) {
                deduits.entry(a).or_default().entry(o.date.as_str()).or_insert(v / q);
            }
        }
    }
    let prix_du = |a: &str, d: &str| -> Option<f64> {
        table.get(a).and_then(|t| t.get(d)).or_else(|| deduits.get(a).and_then(|t| t.get(d))).copied()
    };

    let (mut pta, mut fractions, mut soultes) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut avoirs: BTreeMap<String, f64> = BTreeMap::new();
    let mut portefeuille_complet = true; // faux dès qu'une acquisition n'a pas d'actif
    let mut manquants: Vec<CoursManquant> = Vec::new();

    for i in ordre {
        let o = &ops[i];
        let ligne = i + 1;
        let Some(annee) = date_valide(&o.date) else {
            r.erreurs.push(format!("Opération {ligne} : date invalide (format AAAA-MM-JJ)."));
            continue;
        };
        let m = o.montant;
        if !(m.is_finite() && m >= 0.0) {
            r.erreurs.push(format!("Opération {ligne} : montant invalide."));
            continue;
        }
        let actif = actif_norm(&o.actif);
        let quantite = o.quantite.unwrap_or(0.0);
        if o.quantite.is_some() && !(quantite.is_finite() && quantite >= 0.0) {
            r.erreurs.push(format!("Opération {ligne} : quantité invalide."));
            continue;
        }
        let suivie = actif.is_some() && o.quantite.is_some();

        // Une cession (ou un échange avec soulte) : données communes.
        let cession = |r: &mut Resultat, prix: f64, soulte: f64, frais: f64, avoirs: &BTreeMap<String, f64>,
                           pta: f64, fractions: &mut f64, soultes: f64, manquants: &mut Vec<CoursManquant>| -> bool {
            if !(frais.is_finite() && frais >= 0.0) { r.erreurs.push(format!("Opération {ligne} : frais invalides.")); return false; }
            if !soulte.is_finite() { r.erreurs.push(format!("Opération {ligne} : soulte invalide.")); return false; }
            // Valeur globale : saisie, sinon calculée sur les avoirs juste avant l'opération.
            let (v, calculee) = match o.valeur_portefeuille {
                Some(v) => (v, false),
                None if !portefeuille_complet => {
                    r.erreurs.push(format!("Opération {ligne} : valeur globale du portefeuille à saisir (des opérations sans actif ni quantité empêchent de la calculer)."));
                    return false;
                }
                None if avoirs.values().all(|q| *q <= EPS) => {
                    r.erreurs.push(format!("Opération {ligne} : valeur globale du portefeuille manquante ou nulle."));
                    return false;
                }
                None => {
                    let mut total = 0.0;
                    let mut ok = true;
                    for (a, q) in avoirs.iter().filter(|(_, q)| **q > EPS) {
                        match prix_du(a, &o.date) {
                            Some(p) => total += q * p,
                            None => { ok = false; manquants.push(CoursManquant { actif: a.clone(), date: o.date.clone() }); }
                        }
                    }
                    if !ok {
                        r.erreurs.push(format!("Opération {ligne} : cours manquant pour calculer la valeur globale du portefeuille au {}.", o.date));
                        return false;
                    }
                    (total, true)
                }
            };
            if !(v.is_finite() && v > 0.0) {
                r.erreurs.push(format!("Opération {ligne} : valeur globale du portefeuille manquante ou nulle."));
                return false;
            }
            let prix_net_soultes = prix + soulte;
            if prix_net_soultes > v * (1.0 + 1e-6) + 0.005 {
                r.erreurs.push(format!("Opération {ligne} : le prix de cession dépasse la valeur globale du portefeuille ({v:.2} €)."));
                return false;
            }
            let pta_net = pta - *fractions - soultes;
            if pta_net < 0.0 {
                r.avertissements.push(format!("Opération {ligne} : prix total d'acquisition net négatif ({pta_net:.2} €) ; vérifiez les achats et soultes saisis."));
            }
            let fraction = pta_net * prix_net_soultes / v;
            let prix_net_frais_soultes = prix_net_soultes - frais;
            r.cessions.push(Cession {
                operation: ligne, annee, date: o.date.clone(), valeur_portefeuille: v, valeur_calculee: calculee,
                prix_cession: prix, frais, prix_net_frais: prix - frais, soulte_cession: soulte, prix_net_soultes,
                prix_net_frais_soultes, prix_total_acquisition: pta, fractions_capital_initial: *fractions,
                soultes_anterieures: soultes, pta_net, fraction_imputee: fraction, plus_value: prix_net_frais_soultes - fraction,
                actif: actif.clone().unwrap_or_default(), note: o.note.clone(),
            });
            *fractions += fraction;
            true
        };

        let retirer = |r: &mut Resultat, avoirs: &mut BTreeMap<String, f64>, a: &str, q: f64| {
            let h = avoirs.entry(a.to_string()).or_insert(0.0);
            if q > *h * (1.0 + 1e-9) + 1e-12 {
                r.avertissements.push(format!("Opération {ligne} : {q} {a} sortent alors que seuls {} sont détenus ; une opération manque peut-être (achat, transfert entrant).", *h));
            }
            *h = (*h - q).max(0.0);
        };

        match o.kind {
            TypeOperation::Achat => {
                pta += m;
                if suivie { *avoirs.entry(actif.clone().unwrap()).or_insert(0.0) += quantite; } else { portefeuille_complet = false; }
            }
            TypeOperation::SoulteRecue => soultes += m,
            TypeOperation::RevenuGratuit => {
                pta += m;
                if suivie { *avoirs.entry(actif.clone().unwrap()).or_insert(0.0) += quantite; } else { portefeuille_complet = false; }
            }
            TypeOperation::Transfert => {
                if let Some(a) = &actif {
                    let f = o.quantite_frais.unwrap_or(0.0);
                    if !(f.is_finite() && f >= 0.0) { r.erreurs.push(format!("Opération {ligne} : frais de réseau invalides.")); continue; }
                    if f > 0.0 { retirer(&mut r, &mut avoirs, a, f); }
                }
            }
            TypeOperation::Cession => {
                let frais = o.frais.unwrap_or(0.0);
                let soulte = o.soulte.unwrap_or(0.0);
                if !cession(&mut r, m, soulte, frais, &avoirs, pta, &mut fractions, soultes, &mut manquants) { continue; }
                if suivie { retirer(&mut r, &mut avoirs, actif.as_deref().unwrap(), quantite); }
            }
            TypeOperation::Echange => {
                let (Some(a), Some(b)) = (actif.clone(), actif_norm(&o.actif_recu)) else {
                    r.erreurs.push(format!("Opération {ligne} : un échange doit indiquer l'actif donné et l'actif reçu."));
                    continue;
                };
                let qb = o.quantite_recue.unwrap_or(f64::NAN);
                if !(qb.is_finite() && qb >= 0.0) || o.quantite.is_none() {
                    r.erreurs.push(format!("Opération {ligne} : quantités de l'échange manquantes ou invalides."));
                    continue;
                }
                let soulte = o.soulte.unwrap_or(0.0);
                if soulte.abs() > 0.0 {
                    // Échange avec soulte : cession imposable, puis acquisition des actifs reçus pour leur valeur.
                    let Some(v_recue) = o.valeur_recue.filter(|v| v.is_finite() && *v >= 0.0) else {
                        r.erreurs.push(format!("Opération {ligne} : échange avec soulte : indiquez la valeur en euros des actifs reçus."));
                        continue;
                    };
                    if !cession(&mut r, v_recue, soulte, o.frais.unwrap_or(0.0), &avoirs, pta, &mut fractions, soultes, &mut manquants) { continue; }
                    pta += v_recue;
                }
                retirer(&mut r, &mut avoirs, &a, quantite);
                *avoirs.entry(b).or_insert(0.0) += qb;
            }
        }
    }

    let mut par_annee: BTreeMap<i32, Annee> = BTreeMap::new();
    for c in &r.cessions {
        let a = par_annee.entry(c.annee).or_insert_with(|| Annee {
            annee: c.annee, nb_cessions: 0, total_prix_cession: 0.0, plus_values: 0.0, moins_values: 0.0,
            resultat_net: 0.0, exonere: false, imposable: 0.0, case_2042c: String::new(),
        });
        a.nb_cessions += 1;
        a.total_prix_cession += c.prix_net_frais_soultes;
        if c.plus_value >= 0.0 { a.plus_values += c.plus_value } else { a.moins_values -= c.plus_value }
    }
    for (_, mut a) in par_annee {
        conclure(&mut a);
        r.annees.push(a);
    }
    manquants.sort();
    manquants.dedup();
    r.cours_manquants = manquants;
    r.avoirs = avoirs.into_iter().filter(|(_, q)| *q > EPS).collect();
    r
}

fn conclure(a: &mut Annee) {
    a.resultat_net = a.plus_values - a.moins_values;
    a.exonere = a.total_prix_cession <= SEUIL_EXONERATION;
    a.imposable = if a.exonere { 0.0 } else { a.resultat_net.max(0.0) };
    a.case_2042c = if a.exonere { String::new() } else if a.resultat_net >= 0.0 { "3AN".into() } else { "3BN".into() };
}

/// Une année du foyer : lignes 51 et 52 du 2086 (déclarants 1 et 2, personne à charge).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnneeFoyer {
    pub annee: i32,
    /// Ligne 224, 264 ou 324 de chaque membre (dans l'ordre fourni ; 0 si aucune cession cette année-là).
    pub par_membre: Vec<f64>,
    /// Ligne 51 : total des prix de cession du foyer.
    pub total_prix_cession: f64,
    /// Ligne 52 : total des plus et moins-values du foyer.
    pub resultat_net: f64,
    /// Le seuil de 305 € s'apprécie au niveau du foyer.
    pub exonere: bool,
    pub imposable: f64,
    pub case_2042c: String,
}

/// Agrège les résultats des membres d'un foyer (chacun calculé avec son propre portefeuille).
pub fn foyer(membres: &[Resultat]) -> Vec<AnneeFoyer> {
    let mut annees: BTreeMap<i32, AnneeFoyer> = BTreeMap::new();
    for (k, m) in membres.iter().enumerate() {
        for a in &m.annees {
            let f = annees.entry(a.annee).or_insert_with(|| AnneeFoyer {
                annee: a.annee, par_membre: vec![0.0; membres.len()], total_prix_cession: 0.0, resultat_net: 0.0,
                exonere: false, imposable: 0.0, case_2042c: String::new(),
            });
            f.par_membre[k] += a.resultat_net;
            f.total_prix_cession += a.total_prix_cession;
            f.resultat_net += a.resultat_net;
        }
    }
    annees.into_values().map(|mut f| {
        f.exonere = f.total_prix_cession <= SEUIL_EXONERATION;
        f.imposable = if f.exonere { 0.0 } else { f.resultat_net.max(0.0) };
        f.case_2042c = if f.exonere { String::new() } else if f.resultat_net >= 0.0 { "3AN".into() } else { "3BN".into() };
        f
    }).collect()
}

/// Arrondi à l'euro le plus proche, comme sur la déclaration (0,50 € arrondi au-dessus, en valeur absolue).
pub fn euro_entier(x: f64) -> i64 {
    if x >= 0.0 { (x + 0.5).floor() as i64 } else { -((-x + 0.5).floor() as i64) }
}
