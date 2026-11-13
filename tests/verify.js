#!/usr/bin/env node
/*
 * Contrôles du moteur :
 *  1. exemples chiffrés du BOFiP (BOI-RPPM-PVBMC-30, § 110, § 120, § 160, § 170) ;
 *  2. règles annuelles (seuil de 305 €, moins-value non reportable) ;
 *  3. recoupement avec l'implémentation de référence Python (fractions exactes) sur des portefeuilles aléatoires.
 *
 *   python3 -I tests/reference.py 500 7 > /tmp/aleatoires.json && node tests/verify.js /tmp/aleatoires.json
 */
"use strict";
var fs = require("fs");
var path = require("path");
var M = require(path.join(__dirname, "engine.js"));

var ok = 0, ko = 0;
function egal(nom, obtenu, attendu, tol) {
  tol = tol === undefined ? 1e-9 : tol;
  if (Math.abs(obtenu - attendu) <= tol * Math.max(1, Math.abs(attendu))) { ok++; }
  else { ko++; console.log("ÉCHEC  " + nom + " : obtenu " + obtenu + ", attendu " + attendu); }
}
function vrai(nom, cond) { if (cond) ok++; else { ko++; console.log("ÉCHEC  " + nom); } }

// § 110 — achat 1 000 € ; mars : portefeuille 1 200 €, cession 450 € ; août : tout cédé, portefeuille 1 300 €.
var r = M.calculer([
  { date: "2026-01-10", type: "achat", montant: 1000 },
  { date: "2026-03-10", type: "cession", montant: 450, valeur_portefeuille: 1200 },
  { date: "2026-08-10", type: "cession", montant: 1300, valeur_portefeuille: 1300 }
]);
egal("BOFiP §110 mars : fraction de capital initial", r.cessions[0].fraction_imputee, 375);
egal("BOFiP §110 mars : plus-value", r.cessions[0].plus_value, 75);
egal("BOFiP §110 août : PTA net", r.cessions[1].pta_net, 625);
egal("BOFiP §110 août : plus-value", r.cessions[1].plus_value, 675);

// § 120 — achat 500 € ; mars : portefeuille 750 €, cédé contre actifs (600 €) + soulte 150 € = 750 € ;
// les actifs reçus valent 600 € d'acquisition (le BOFiP écrit (500 + 750) − 500 − 150) ; octobre : tout cédé, 800 €.
r = M.calculer([
  { date: "2026-01-10", type: "achat", montant: 500 },
  { date: "2026-03-10", type: "cession", montant: 750, valeur_portefeuille: 750 },
  { date: "2026-03-10", type: "achat", montant: 600 },
  { date: "2026-10-10", type: "cession", montant: 800, valeur_portefeuille: 800 }
]);
egal("BOFiP §120 mars : plus-value", r.cessions[0].plus_value, 250);
egal("BOFiP §120 octobre : plus-value", r.cessions[1].plus_value, 200);

// Variante de § 120 écrite avec une soulte reçue explicite (même arithmétique : 500 + 750 − 500 − 150).
r = M.calculer([
  { date: "2026-01-10", type: "achat", montant: 500 },
  { date: "2026-03-10", type: "cession", montant: 750, valeur_portefeuille: 750 },
  { date: "2026-03-10", type: "achat", montant: 750 },
  { date: "2026-03-10", type: "soulte_recue", montant: 150 },
  { date: "2026-10-10", type: "cession", montant: 800, valeur_portefeuille: 800 }
]);
egal("BOFiP §120 (soulte explicite) octobre : plus-value", r.cessions[1].plus_value, 200);

// § 50 — les frais réduisent le prix de cession, pas le quotient.
r = M.calculer([
  { date: "2026-01-10", type: "achat", montant: 1000 },
  { date: "2026-03-10", type: "cession", montant: 450, valeur_portefeuille: 1200, frais: 10 }
]);
egal("§50 frais : fraction inchangée", r.cessions[0].fraction_imputee, 375);
egal("§50 frais : plus-value", r.cessions[0].plus_value, 65);

// § 160 / § 170 — compensation annuelle ; moins-value nette non imposable, non reportée.
r = M.calculer([
  { date: "2026-01-01", type: "achat", montant: 1000 },
  { date: "2026-02-01", type: "cession", montant: 600, valeur_portefeuille: 2000 }, // 600 − 300 = 300
  { date: "2026-03-01", type: "cession", montant: 200, valeur_portefeuille: 400 },  // 200 − 350 = −150
  { date: "2027-01-01", type: "cession", montant: 300, valeur_portefeuille: 300 }   // 300 − 350 = −50
]);
egal("§160 résultat net 2026", r.annees[0].resultat_net, 150);
egal("§160 imposable 2026", r.annees[0].imposable, 150);
egal("§170 résultat net 2027", r.annees[1].resultat_net, -50);
egal("§170 imposable 2027", r.annees[1].imposable, 0);

// Seuil de 305 € : total des prix de cession de l'année (exonéré si ≤ 305 €).
r = M.calculer([
  { date: "2026-01-01", type: "achat", montant: 10 },
  { date: "2026-05-01", type: "cession", montant: 305, valeur_portefeuille: 1000 }
]);
vrai("seuil 305 € : exonéré à 305 €", r.annees[0].exonere && r.annees[0].imposable === 0);
r = M.calculer([
  { date: "2026-01-01", type: "achat", montant: 10 },
  { date: "2026-05-01", type: "cession", montant: 305.01, valeur_portefeuille: 1000 }
]);
vrai("seuil 305 € : imposable à 305,01 €", !r.annees[0].exonere && r.annees[0].imposable > 0);
// Le seuil porte sur les prix nets de frais (2086, ligne 51 = somme des lignes 218).
r = M.calculer([
  { date: "2026-01-01", type: "achat", montant: 10 },
  { date: "2026-05-01", type: "cession", montant: 310, valeur_portefeuille: 1000, frais: 5 }
]);
egal("seuil 305 € : total net de frais", r.annees[0].total_cessions, 305);
vrai("seuil 305 € : exonéré à 310 € bruts − 5 € de frais", r.annees[0].exonere && r.annees[0].imposable === 0);

// Saisie incohérente refusée.
r = M.calculer([{ date: "2026-05-01", type: "cession", montant: 500, valeur_portefeuille: 100 }]);
vrai("cession > valeur globale : erreur", r.erreurs.length === 1 && r.cessions.length === 0);

// CSV, virgule décimale, séparateur « ; ».
var csv = M.lireCSV("date;type;montant;valeur_portefeuille;frais\n2026-01-10;achat;1000;;\n2026-03-10;cession;450,00;1 200;0\n");
r = M.calculer(csv.ops);
egal("CSV : plus-value", r.cessions[0].plus_value, 75);

// Recoupement avec la référence Python.
var fichier = process.argv[2];
if (fichier) {
  var data = JSON.parse(fs.readFileSync(fichier, "utf8"));
  var nbCessions = 0, ecartMax = 0;
  data.cas.forEach(function (c, k) {
    var res = M.calculer(c.ops);
    vrai("aléatoire " + k + " : aucune erreur (" + res.erreurs.join(" ") + ")", res.erreurs.length === 0);
    vrai("aléatoire " + k + " : nombre de cessions", res.cessions.length === c.attendu.length);
    c.attendu.forEach(function (a, j) {
      var o = res.cessions[j] ? res.cessions[j].plus_value : NaN;
      ecartMax = Math.max(ecartMax, Math.abs(o - a));
      egal("aléatoire " + k + " cession " + j, o, a, 1e-9);
      nbCessions++;
    });
  });
  console.log("Recoupement Python (graine " + data.graine + ") : " + data.cas.length + " portefeuilles, " +
    nbCessions + " cessions, écart maximal " + ecartMax.toExponential(2) + " €");
}

console.log(ok + " contrôles réussis, " + ko + " échecs");
process.exit(ko ? 1 : 0);
