# Taiseux

Calcul des plus-values de cession d'actifs numériques des particuliers en France (article 150 VH bis du CGI) et
préparation du formulaire **2086**, **sans connexion à Internet**. Application de bureau (Rust + Tauri 2), gratuite,
open source (licence MIT).

Téléchargement des fichiers officiels, vérifiés et horodatés : [taiseux.toolguild.online](https://taiseux.toolguild.online).

- **Aucune connexion.** L'application ne peut joindre aucun serveur : la politique de sécurité de l'interface
  (`app/src-tauri/tauri.conf.json`) n'autorise que les appels internes, et les droits accordés à l'interface
  (`app/src-tauri/capabilities/default.json`) se limitent aux boîtes de dialogue d'ouverture et d'enregistrement.
- **Sans trace par défaut.** Rien n'est écrit sur le disque tant que vous n'enregistrez pas.
- **Coffre chiffré.** Un seul fichier `.taiseux` : Argon2id (64 Mio, 3 passes) puis XChaCha20-Poly1305, en-tête
  authentifié (`app/core/src/vault.rs`).
- **Rien n'est envoyé au fisc.** Taiseux affiche les montants ligne par ligne ; vous les recopiez sur impots.gouv.fr.

Ce n'est pas un conseil fiscal : le logiciel applique la formule légale aux chiffres saisis et montre chaque étape.

## Ce que fait l'application

- Opérations par crypto et quantité : achat, vente, échange (avec ou sans soulte), transfert, crypto reçue.
- Valeur globale du portefeuille calculée à partir des avoirs et de cours **que vous importez** (Taiseux ne télécharge
  rien) ; une valeur saisie à la main prime.
- Imports des exports de 24 plateformes et portefeuilles (`app/core/src/imports/`), **en bêta** : les formats viennent de
  la documentation, aucun n'a encore été validé sur un vrai fichier. Une ligne non comprise n'est jamais devinée.
- Foyer fiscal (déclarant 1, déclarant 2, personne à charge), lignes 211 à 224, 51 et 52, cases 3AN / 3BN.
- Liste des comptes à l'étranger (3916-bis).

## Le calcul

```text
plus-value = (prix de cession − frais) − PTA_net × prix de cession / valeur globale du portefeuille
PTA_net    = prix total d'acquisition − fractions de capital initial déjà imputées − soultes reçues antérieures
```

- Les frais réduisent le prix de cession, pas le quotient (BOFiP, BOI-RPPM-PVBMC-30, § 50 ; notice du 2086).
- Seuil : si le total annuel des prix de cession nets de frais (ligne 51) ne dépasse pas 305 € pour le foyer, les
  plus-values sont exonérées.
- Le résultat de l'année (ligne 52) se reporte en case 3AN (plus-value) ou 3BN (moins-value) de la 2042 C.
- Une moins-value nette ne se reporte pas sur les années suivantes.

Non traités : régimes professionnels, minage, contrats à terme, marge, DeFi. Les récompenses de staking sont comptées
comme une crypto reçue au prix d'acquisition de 0 €, un traitement que la doctrine discute.

## Vérification

```bash
cd app && cargo test -p taiseux-core
```

- Exemples chiffrés du BOFiP (§ 50, § 110, § 120 sous ses deux écritures, § 160, § 170) et de la notice du 2086.
- Seuil de 305 €, cases de la 2042 C, CSV, arrondi à l'euro, foyer, échanges et soultes.
- **Recoupement** avec deux implémentations de référence écrites séparément en Python, en fractions exactes
  (`tests/reference.py` : 3 012 cessions, écart maximal 7e-12 € ; `tests/reference_actifs.py` : 4 576 cessions par
  crypto, écart maximal 6,6e-8 €).
- Coffre : aller-retour, mauvaise phrase, falsification de l'en-tête et du chiffré, paramètres absurdes refusés,
  relecture d'un coffre de la version 0.1.

## Compiler

Il faut Rust (1.77 ou plus), la CLI Tauri 2 (`cargo install tauri-cli --version "^2" --locked`) et les dépendances
système de Tauri (WebView2 sous Windows, WebKitGTK 4.1 sous Linux). Puis :

```bash
cd app/src-tauri && cargo tauri build
```

`app/build/` contient les scripts des paquets Linux et Windows (conteneurs) et de la release (empreintes SHA-256,
horodatage OpenTimestamps).

## Format CSV de Taiseux

```text
date;type;montant;valeur_portefeuille;frais;note
2026-01-10;achat;1000;;;Kraken
2026-03-10;cession;450;1200;0;
```

Séparateur `;` ou `,`, virgule ou point décimal. Les colonnes par crypto (actif, quantité, plateforme…) sont décrites
dans `app/core/src/csv.rs`.

## Soutenir

Taiseux est gratuit. Le suivi de la loi, chaque année, est financé par des contributions facultatives, qui ne donnent
accès à rien de plus : [taiseux.toolguild.online](https://taiseux.toolguild.online/#soutenir).

## Licence

Copyright © 2026 ToolGuild. Licence **MIT** (fichier `LICENSE`).
