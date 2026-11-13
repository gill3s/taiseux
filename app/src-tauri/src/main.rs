// Taiseux — application de bureau. Copyright (c) 2026 ToolGuild. Licence MIT.
//
// L'interface (ui/) appelle ces commandes ; elles ne font que calculer, lire ou écrire le fichier que l'utilisateur
// vient de choisir dans une boîte de dialogue. Aucune requête réseau : ni ici, ni dans l'interface (CSP), ni dans
// les droits accordés (capabilities/default.json).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use taiseux_core::{api, csv, engine, vault};
use zeroize::Zeroize;

fn lire_fichier(chemin: &Path) -> Result<String, String> {
    let octets = fs::read(chemin).map_err(|e| format!("Lecture impossible : {e}"))?;
    if octets.len() > 200 * 1024 * 1024 { return Err("Fichier trop gros (plus de 200 Mo).".into()); }
    Ok(api::texte_fichier(&octets))
}

/// Calcul du foyer : un jeu d'opérations par membre, cours communs.
#[tauri::command]
fn calculer_foyer(membres: Vec<Vec<engine::Operation>>, cours: Vec<engine::Cours>) -> api::CalculFoyer {
    api::calculer_foyer(&membres, &cours)
}

/// Lit un export de plateforme ou de portefeuille (ou un CSV de Taiseux) et renvoie l'aperçu, sans rien ajouter.
#[tauri::command]
fn importer_fichier(chemin: PathBuf) -> Result<api::Apercu, String> {
    api::importer(&lire_fichier(&chemin)?)
}

#[tauri::command]
fn importer_cours(chemin: PathBuf, actif: Option<String>) -> Result<api::LectureCours, String> {
    let nom = chemin.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
    Ok(api::lire_cours(&lire_fichier(&chemin)?, actif.as_deref(), &nom))
}

#[tauri::command]
fn fusionner_cours(existants: Vec<engine::Cours>, nouveaux: Vec<engine::Cours>) -> Vec<engine::Cours> {
    api::fusionner_cours(&existants, &nouveaux)
}

#[tauri::command]
fn plateformes() -> Vec<(String, String)> {
    api::plateformes()
}

#[tauri::command]
fn csv_operations(operations: Vec<engine::Operation>) -> String {
    csv::ecrire_operations(&operations)
}

#[tauri::command]
fn csv_2086(operations: Vec<engine::Operation>, cours: Vec<engine::Cours>) -> String {
    csv::ecrire_2086(&engine::calculer_avec_cours(&operations, &cours))
}

/// Écrit un fichier de façon atomique : fichier voisin puis renommage, pour ne jamais laisser un coffre à moitié écrit.
fn ecrire_atomique(chemin: &Path, octets: &[u8]) -> Result<(), String> {
    let tmp = chemin.with_extension(format!("{}.part", chemin.extension().and_then(|e| e.to_str()).unwrap_or("tmp")));
    let mut f = fs::File::create(&tmp).map_err(|e| format!("Écriture impossible : {e}"))?;
    f.write_all(octets).and_then(|_| f.sync_all()).map_err(|e| format!("Écriture impossible : {e}"))?;
    drop(f);
    fs::rename(&tmp, chemin).map_err(|e| { let _ = fs::remove_file(&tmp); format!("Écriture impossible : {e}") })
}

/// Enregistre un texte (export CSV, fiche) à l'endroit choisi par l'utilisateur.
#[tauri::command]
fn enregistrer_texte(chemin: PathBuf, contenu: String) -> Result<(), String> {
    ecrire_atomique(&chemin, contenu.as_bytes())
}

#[tauri::command]
async fn coffre_sceller(chemin: PathBuf, mut phrase: String, dossier: vault::Dossier) -> Result<(), String> {
    let r = vault::sceller(&dossier, &phrase).map_err(|e| e.to_string());
    phrase.zeroize();
    ecrire_atomique(&chemin, &r?)
}

/// Ouvre un coffre (format 0.1 ou 0.2) ; le dossier rendu est toujours au format 2.
#[tauri::command]
async fn coffre_ouvrir(chemin: PathBuf, mut phrase: String) -> Result<vault::Dossier, String> {
    let octets = fs::read(&chemin).map_err(|e| format!("Lecture impossible : {e}"))?;
    let r = vault::ouvrir(&octets, &phrase).map_err(|e| e.to_string());
    phrase.zeroize();
    r
}

#[tauri::command]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            calculer_foyer, importer_fichier, importer_cours, fusionner_cours, plateformes, csv_operations, csv_2086,
            enregistrer_texte, coffre_sceller, coffre_ouvrir, version
        ])
        .run(tauri::generate_context!())
        .expect("Taiseux n'a pas pu démarrer");
}
