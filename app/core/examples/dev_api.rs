//! Développement seulement : rejoue les commandes de l'application (mêmes noms, mêmes arguments JSON) pour que
//! l'aperçu de l'interface dans un navigateur (dev/serveur.py) utilise le vrai moteur. Lit `{"cmd":…, "args":…}`
//! sur l'entrée standard, écrit `{"ok":…}` ou `{"err":…}`. N'est pas livré dans l'application.

use std::io::Read;

use serde_json::{json, Value};
use taiseux_core::{api, csv, engine, vault};

fn arg<T: serde::de::DeserializeOwned>(a: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(a.get(k).cloned().unwrap_or(Value::Null)).map_err(|e| format!("argument {k} : {e}"))
}

fn lire(chemin: &str) -> Result<String, String> {
    std::fs::read(chemin).map(|o| api::texte_fichier(&o)).map_err(|e| format!("Lecture impossible : {e}"))
}

fn executer(cmd: &str, a: &Value) -> Result<Value, String> {
    let v = |x: &dyn EnJson| x.json();
    Ok(match cmd {
        "calculer_foyer" => v(&api::calculer_foyer(&arg::<Vec<Vec<engine::Operation>>>(a, "membres")?, &arg::<Vec<engine::Cours>>(a, "cours")?)),
        "importer_fichier" => v(&api::importer(&lire(&arg::<String>(a, "chemin")?)?)?),
        "importer_cours" => {
            let c: String = arg(a, "chemin")?;
            let actif: Option<String> = arg(a, "actif")?;
            v(&api::lire_cours(&lire(&c)?, actif.as_deref(), &c))
        }
        "fusionner_cours" => v(&api::fusionner_cours(&arg::<Vec<engine::Cours>>(a, "existants")?, &arg::<Vec<engine::Cours>>(a, "nouveaux")?)),
        "plateformes" => v(&api::plateformes()),
        "csv_operations" => json!(csv::ecrire_operations(&arg::<Vec<engine::Operation>>(a, "operations")?)),
        "csv_2086" => json!(csv::ecrire_2086(&engine::calculer_avec_cours(&arg::<Vec<engine::Operation>>(a, "operations")?, &arg::<Vec<engine::Cours>>(a, "cours")?))),
        "enregistrer_texte" => { std::fs::write(arg::<String>(a, "chemin")?, arg::<String>(a, "contenu")?).map_err(|e| e.to_string())?; Value::Null }
        "coffre_sceller" => {
            let o = vault::sceller(&arg::<vault::Dossier>(a, "dossier")?, &arg::<String>(a, "phrase")?).map_err(|e| e.to_string())?;
            std::fs::write(arg::<String>(a, "chemin")?, o).map_err(|e| e.to_string())?;
            Value::Null
        }
        "coffre_ouvrir" => {
            let o = std::fs::read(arg::<String>(a, "chemin")?).map_err(|e| e.to_string())?;
            v(&vault::ouvrir(&o, &arg::<String>(a, "phrase")?).map_err(|e| e.to_string())?)
        }
        "version" => json!(format!("{} (aperçu)", env!("CARGO_PKG_VERSION"))),
        autre => return Err(format!("commande inconnue : {autre}")),
    })
}

trait EnJson { fn json(&self) -> Value; }
impl<T: serde::Serialize> EnJson for T { fn json(&self) -> Value { serde_json::to_value(self).unwrap() } }

fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let req: Value = serde_json::from_str(&s).unwrap_or(Value::Null);
    let cmd = req.get("cmd").and_then(Value::as_str).unwrap_or("");
    let r = match executer(cmd, req.get("args").unwrap_or(&Value::Null)) { Ok(v) => json!({ "ok": v }), Err(e) => json!({ "err": e }) };
    println!("{r}");
}
