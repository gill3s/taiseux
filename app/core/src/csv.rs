//! Import et export CSV. Format de base, identique au simulateur en ligne :
//! `date;type;montant;valeur_portefeuille;frais[;note]` (séparateur « ; » ou « , », en-tête obligatoire,
//! virgule ou point décimal, espaces de milliers acceptés). Colonnes facultatives de la version 0.2 (suivi par actif) :
//! `actif;quantite;actif_recu;quantite_recue;valeur_recue;soulte;quantite_frais;plateforme;heure`.
//! Types : achat, cession, soulte_recue, echange, transfert, revenu_gratuit.

use crate::engine::{Annee, Cession, Operation, Resultat, TypeOperation};

pub struct Lecture {
    pub operations: Vec<Operation>,
    pub erreurs: Vec<String>,
}

/// Lit un nombre écrit à la française ou à l'anglaise : « 1 200,50 », « 1200.50 ». Vide = None.
pub fn nombre(s: &str) -> Option<f64> {
    let t: String = s.trim().chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\u{202f}' && *c != '€').collect();
    if t.is_empty() { return None; }
    t.replace(',', ".").parse::<f64>().ok()
}

fn decouper(ligne: &str, sep: char) -> Vec<String> {
    // Champs éventuellement entre guillemets ("…"), guillemets doublés à l'intérieur.
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

pub fn lire(texte: &str) -> Lecture {
    let texte = texte.trim_start_matches('\u{feff}');
    let lignes: Vec<&str> = texte.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut erreurs = Vec::new();
    if lignes.is_empty() {
        return Lecture { operations: vec![], erreurs: vec!["Fichier vide.".into()] };
    }
    let sep = if lignes[0].contains(';') { ';' } else { ',' };
    let entete: Vec<String> = decouper(lignes[0], sep).into_iter().map(|s| s.to_lowercase()).collect();
    let idx = |k: &str| entete.iter().position(|h| h == k);
    let (Some(i_date), Some(i_type), Some(i_montant)) = (idx("date"), idx("type"), idx("montant")) else {
        return Lecture { operations: vec![], erreurs: vec!["En-tête attendu : date;type;montant;valeur_portefeuille;frais;note".into()] };
    };
    let (i_valeur, i_frais, i_note) = (idx("valeur_portefeuille"), idx("frais"), idx("note"));
    let (i_actif, i_qte, i_actif_r, i_qte_r, i_val_r) = (idx("actif"), idx("quantite"), idx("actif_recu"), idx("quantite_recue"), idx("valeur_recue"));
    let (i_soulte, i_qte_f, i_plat, i_heure) = (idx("soulte"), idx("quantite_frais"), idx("plateforme"), idx("heure"));
    let mut operations = Vec::new();
    for (n, l) in lignes.iter().enumerate().skip(1) {
        let c = decouper(l, sep);
        let champ = |i: Option<usize>| i.and_then(|i| c.get(i)).map(|s| s.as_str()).unwrap_or("");
        let kind = match champ(Some(i_type)).to_lowercase().as_str() {
            "achat" => TypeOperation::Achat,
            "cession" => TypeOperation::Cession,
            "soulte_recue" => TypeOperation::SoulteRecue,
            "echange" | "échange" => TypeOperation::Echange,
            "transfert" => TypeOperation::Transfert,
            "revenu_gratuit" => TypeOperation::RevenuGratuit,
            t => { erreurs.push(format!("CSV ligne {} : type « {t} » inconnu (achat, cession, soulte_recue, echange, transfert, revenu_gratuit).", n + 1)); continue; }
        };
        // Le montant peut manquer pour un échange ou un transfert (il n'y sert pas).
        let brut = champ(Some(i_montant));
        let montant = match nombre(brut) {
            Some(m) => m,
            None if brut.trim().is_empty() && matches!(kind, TypeOperation::Echange | TypeOperation::Transfert | TypeOperation::RevenuGratuit) => 0.0,
            None => { erreurs.push(format!("CSV ligne {} : montant illisible.", n + 1)); continue; }
        };
        let texte = |i: Option<usize>| { let t = champ(i).trim(); if t.is_empty() { None } else { Some(t.to_string()) } };
        operations.push(Operation {
            date: champ(Some(i_date)).to_string(),
            heure: champ(i_heure).to_string(),
            kind,
            montant,
            valeur_portefeuille: nombre(champ(i_valeur)),
            frais: nombre(champ(i_frais)),
            note: champ(i_note).to_string(),
            actif: texte(i_actif),
            quantite: nombre(champ(i_qte)),
            actif_recu: texte(i_actif_r),
            quantite_recue: nombre(champ(i_qte_r)),
            valeur_recue: nombre(champ(i_val_r)),
            soulte: nombre(champ(i_soulte)),
            quantite_frais: nombre(champ(i_qte_f)),
            plateforme: champ(i_plat).to_string(),
        });
    }
    Lecture { operations, erreurs }
}

fn champ_csv(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}
fn n2(x: f64) -> String { format!("{x:.2}") }
fn opt(x: Option<f64>) -> String { x.map(n2).unwrap_or_default() }

/// Exporte les opérations dans le format d'import (réimportable tel quel).
pub fn ecrire_operations(ops: &[Operation]) -> String {
    let mut s = String::from("date;type;montant;valeur_portefeuille;frais;note;actif;quantite;actif_recu;quantite_recue;valeur_recue;soulte;quantite_frais;plateforme;heure\n");
    let q = |x: Option<f64>| x.map(|v| format!("{v}")).unwrap_or_default();
    let t = |x: &Option<String>| champ_csv(x.as_deref().unwrap_or(""));
    for o in ops {
        let k = match o.kind {
            TypeOperation::Achat => "achat", TypeOperation::Cession => "cession", TypeOperation::SoulteRecue => "soulte_recue",
            TypeOperation::Echange => "echange", TypeOperation::Transfert => "transfert", TypeOperation::RevenuGratuit => "revenu_gratuit",
        };
        s += &format!("{};{};{};{};{};{};{};{};{};{};{};{};{};{};{}\n", o.date, k, n2(o.montant), opt(o.valeur_portefeuille), opt(o.frais),
            champ_csv(&o.note), t(&o.actif), q(o.quantite), t(&o.actif_recu), q(o.quantite_recue), opt(o.valeur_recue), opt(o.soulte),
            q(o.quantite_frais), champ_csv(&o.plateforme), o.heure);
    }
    s
}

/// Lit un fichier de cours. Formats acceptés :
/// - générique, avec en-tête : `date;actif;prix` (prix en euros pour une unité) ;
/// - export « données historiques » de CoinGecko (`snapped_at,price,...`, dans la devise choisie : l'euro) — l'actif
///   n'y figure pas : il est donné par `actif_par_defaut` (nom du fichier, choix de l'utilisateur) ;
/// - fichier de bougies quotidiennes de l'archive publique de Kraken (`<horodatage>,<ouverture>,<haut>,<bas>,
///   <clôture>,<volume>,<transactions>`, sans en-tête) : on garde la clôture du jour.
pub fn lire_cours(texte: &str, actif_par_defaut: Option<&str>) -> (Vec<crate::engine::Cours>, Vec<String>) {
    use crate::engine::Cours;
    let texte = texte.trim_start_matches('\u{feff}');
    let lignes: Vec<&str> = texte.lines().filter(|l| !l.trim().is_empty()).collect();
    let (mut cours, mut erreurs) = (Vec::new(), Vec::new());
    let Some(premiere) = lignes.first() else { return (cours, vec!["Fichier de cours vide.".into()]) };
    let sep = if premiere.contains(';') { ';' } else { ',' };
    let entete: Vec<String> = decouper(premiere, sep).into_iter().map(|s| s.to_lowercase()).collect();
    let defaut = actif_par_defaut.map(|a| a.trim().to_uppercase()).filter(|a| !a.is_empty());
    let jour = |s: &str| -> Option<String> { let d = s.trim().get(0..10)?; if d.as_bytes().get(4) == Some(&b'-') { Some(d.to_string()) } else { None } };
    if entete.iter().any(|h| h == "snapped_at") {
        let (Some(i_d), Some(i_p)) = (entete.iter().position(|h| h == "snapped_at"), entete.iter().position(|h| h == "price")) else {
            return (cours, vec!["Export CoinGecko : colonnes snapped_at et price attendues.".into()]);
        };
        let Some(a) = defaut else { return (cours, vec!["Export CoinGecko : indiquez de quelle crypto il s'agit.".into()]) };
        for (n, l) in lignes.iter().enumerate().skip(1) {
            let c = decouper(l, sep);
            match (c.get(i_d).and_then(|d| jour(d)), c.get(i_p).and_then(|p| nombre(p))) {
                (Some(date), Some(prix)) => cours.push(Cours { actif: a.clone(), date, prix }),
                _ => erreurs.push(format!("Cours ligne {} : illisible.", n + 1)),
            }
        }
    } else if entete.iter().any(|h| h == "date") && entete.iter().any(|h| h == "prix") {
        let i_d = entete.iter().position(|h| h == "date").unwrap();
        let i_p = entete.iter().position(|h| h == "prix").unwrap();
        let i_a = entete.iter().position(|h| h == "actif");
        for (n, l) in lignes.iter().enumerate().skip(1) {
            let c = decouper(l, sep);
            let a = i_a.and_then(|i| c.get(i)).map(|s| s.trim().to_uppercase()).filter(|s| !s.is_empty()).or_else(|| defaut.clone());
            match (a, c.get(i_d).and_then(|d| jour(d)), c.get(i_p).and_then(|p| nombre(p))) {
                (Some(actif), Some(date), Some(prix)) => cours.push(Cours { actif, date, prix }),
                _ => erreurs.push(format!("Cours ligne {} : illisible (date, actif ou prix).", n + 1)),
            }
        }
    } else if decouper(premiere, ',').len() >= 5 && decouper(premiere, ',')[0].trim().chars().all(|c| c.is_ascii_digit()) {
        // Kraken : horodatage Unix (secondes), clôture en 5e colonne.
        let Some(a) = defaut else { return (cours, vec!["Fichier Kraken : indiquez de quelle crypto il s'agit (ex. XBTEUR → BTC).".into()]) };
        for (n, l) in lignes.iter().enumerate() {
            let c = decouper(l, ',');
            match (c.first().and_then(|t| t.trim().parse::<i64>().ok()), c.get(4).and_then(|p| nombre(p))) {
                (Some(t), Some(prix)) => cours.push(Cours { actif: a.clone(), date: date_unix(t), prix }),
                _ => erreurs.push(format!("Cours ligne {} : illisible.", n + 1)),
            }
        }
    } else {
        erreurs.push("Format de cours non reconnu (attendu : date;actif;prix, un export CoinGecko ou un fichier Kraken).".into());
    }
    (cours, erreurs)
}

/// Date AAAA-MM-JJ (UTC) d'un horodatage Unix en secondes, sans dépendance (algorithme des jours civils).
fn date_unix(t: i64) -> String {
    let z = t.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

/// Exporte le détail du formulaire 2086 : une ligne par cession, une colonne par ligne du formulaire,
/// puis le récapitulatif par année (lignes 224, 51, 52 et case de la 2042 C).
pub fn ecrire_2086(r: &Resultat) -> String {
    let mut s = String::from("annee;211_date;212_valeur_globale;213_prix_cession;214_frais;215_net_frais;216_soulte;217_net_soultes;218_net_frais_soultes;220_prix_total_acquisition;221_fractions_capital_initial;222_soultes_anterieures;223_pta_net;plus_ou_moins_value;note\n");
    for c in &r.cessions {
        let Cession { annee, date, valeur_portefeuille, prix_cession, frais, prix_net_frais, soulte_cession, prix_net_soultes, prix_net_frais_soultes, prix_total_acquisition, fractions_capital_initial, soultes_anterieures, pta_net, plus_value, note, .. } = c;
        s += &format!("{annee};{date};{};{};{};{};{};{};{};{};{};{};{};{};{}\n",
            n2(*valeur_portefeuille), n2(*prix_cession), n2(*frais), n2(*prix_net_frais), n2(*soulte_cession), n2(*prix_net_soultes),
            n2(*prix_net_frais_soultes), n2(*prix_total_acquisition), n2(*fractions_capital_initial), n2(*soultes_anterieures),
            n2(*pta_net), n2(*plus_value), champ_csv(note));
    }
    s += "\nannee;51_total_prix_cession;224_52_resultat;exonere;case_2042c;montant_a_reporter\n";
    for Annee { annee, total_prix_cession, resultat_net, exonere, case_2042c, .. } in &r.annees {
        let report = if *exonere { 0 } else { crate::engine::euro_entier(resultat_net.abs()) };
        s += &format!("{annee};{};{};{};{case_2042c};{report}\n", n2(*total_prix_cession), n2(*resultat_net), if *exonere { "oui" } else { "non" });
    }
    s
}
