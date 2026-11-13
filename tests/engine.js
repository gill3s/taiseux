/*
 * Moteur de calcul des plus-values sur actifs numériques (particuliers, France),
 * art. 150 VH bis du CGI, doctrine BOI-RPPM-PVBMC-30 (BOFiP).
 *
 *   plus-value = (prix de cession − frais) − PTA_net × prix de cession / valeur globale du portefeuille
 *   PTA_net    = prix total d'acquisition − fractions de capital initial déjà imputées − soultes reçues antérieures
 *
 * Les frais réduisent le prix de cession mais pas le quotient (BOFiP § 50).
 * Aucun appel réseau, aucun stockage : fonction pure. Utilisable dans le navigateur et sous Node.
 */
(function (root) {
  "use strict";

  var SEUIL_EXONERATION = 305; // € : total annuel des prix de cession nets de frais (art. 150 VH bis, B du II ;
                               // formulaire 2086, ligne 51 = somme des lignes 218). Exonéré si total ≤ 305 €.

  var TYPES = {
    ACHAT: "achat",              // acquisition contre euros, biens ou services (montant payé)
    CESSION: "cession",          // cession imposable : vers euros, biens ou services
    SOULTE_RECUE: "soulte_recue" // soulte reçue lors d'un échange crypto ↔ crypto non imposé
  };

  function num(x) {
    if (typeof x === "number") return x;
    if (x === null || x === undefined) return NaN;
    var s = String(x).trim().replace(/\s| /g, "").replace(",", ".");
    if (s === "") return NaN;
    return Number(s);
  }

  /**
   * ops : [{date: "AAAA-MM-JJ", type, montant, valeur_portefeuille?, frais?}]
   * retourne {cessions: [...], annees: [...], erreurs: [...], avertissements: [...]}
   */
  function calculer(ops) {
    var erreurs = [], avertissements = [];
    var liste = ops.map(function (o, i) { return { o: o, i: i }; });
    liste.sort(function (a, b) {
      if (a.o.date < b.o.date) return -1;
      if (a.o.date > b.o.date) return 1;
      return a.i - b.i; // même jour : ordre de saisie
    });

    var pta = 0, fractions = 0, soultes = 0;
    var cessions = [];

    liste.forEach(function (e) {
      var o = e.o, ligne = e.i + 1;
      if (!/^\d{4}-\d{2}-\d{2}$/.test(o.date || "")) {
        erreurs.push("Ligne " + ligne + " : date invalide (format AAAA-MM-JJ).");
        return;
      }
      var m = num(o.montant);
      if (!(m >= 0)) { erreurs.push("Ligne " + ligne + " : montant invalide."); return; }

      if (o.type === TYPES.ACHAT) {
        pta += m;
      } else if (o.type === TYPES.SOULTE_RECUE) {
        soultes += m;
      } else if (o.type === TYPES.CESSION) {
        var v = num(o.valeur_portefeuille);
        var f = o.frais === undefined || o.frais === "" ? 0 : num(o.frais);
        if (!(v > 0)) { erreurs.push("Ligne " + ligne + " : valeur globale du portefeuille manquante ou nulle."); return; }
        if (!(f >= 0)) { erreurs.push("Ligne " + ligne + " : frais invalides."); return; }
        if (m > v * (1 + 1e-9)) {
          erreurs.push("Ligne " + ligne + " : le prix de cession dépasse la valeur globale du portefeuille.");
          return;
        }
        var ptaNet = pta - fractions - soultes;
        if (ptaNet < 0) {
          avertissements.push("Ligne " + ligne + " : prix total d'acquisition net négatif (" +
            ptaNet.toFixed(2) + " €) ; vérifiez les achats et soultes saisis.");
        }
        var fraction = ptaNet * m / v;
        var pv = (m - f) - fraction;
        cessions.push({
          ligne: ligne,
          date: o.date,
          annee: Number(o.date.slice(0, 4)),
          valeur_portefeuille: v,
          prix_cession: m,
          frais: f,
          prix_cession_net: m - f,
          prix_total_acquisition: pta,
          fractions_anterieures: fractions,
          soultes_anterieures: soultes,
          pta_net: ptaNet,
          fraction_imputee: fraction,
          plus_value: pv
        });
        fractions += fraction;
      } else {
        erreurs.push("Ligne " + ligne + " : type d'opération inconnu (" + o.type + ").");
      }
    });

    var parAnnee = {};
    cessions.forEach(function (c) {
      var a = parAnnee[c.annee] || (parAnnee[c.annee] = { annee: c.annee, nb: 0, total_cessions: 0, plus_values: 0, moins_values: 0 });
      a.nb += 1;
      a.total_cessions += c.prix_cession_net; // ligne 218 du 2086 (net des frais)
      if (c.plus_value >= 0) a.plus_values += c.plus_value; else a.moins_values += -c.plus_value;
    });
    var annees = Object.keys(parAnnee).sort().map(function (k) {
      var a = parAnnee[k];
      a.resultat_net = a.plus_values - a.moins_values;
      a.exonere = a.total_cessions <= SEUIL_EXONERATION;
      a.imposable = a.exonere ? 0 : Math.max(a.resultat_net, 0);
      // Une moins-value nette ne se reporte pas sur les années suivantes (BOFiP § 170).
      return a;
    });

    return { cessions: cessions, annees: annees, erreurs: erreurs, avertissements: avertissements };
  }

  /* CSV : date;type;montant;valeur_portefeuille;frais  (séparateur ; ou , ; en-tête obligatoire) */
  function lireCSV(texte) {
    var lignes = texte.replace(/\r/g, "").split("\n").filter(function (l) { return l.trim() !== ""; });
    if (lignes.length === 0) return { ops: [], erreurs: ["Fichier vide."] };
    var sep = lignes[0].indexOf(";") >= 0 ? ";" : ",";
    var entete = lignes[0].split(sep).map(function (s) { return s.trim().toLowerCase(); });
    var idx = {};
    ["date", "type", "montant", "valeur_portefeuille", "frais"].forEach(function (k) { idx[k] = entete.indexOf(k); });
    if (idx.date < 0 || idx.type < 0 || idx.montant < 0) {
      return { ops: [], erreurs: ["En-tête attendu : date;type;montant;valeur_portefeuille;frais"] };
    }
    var ops = [], erreurs = [];
    lignes.slice(1).forEach(function (l, i) {
      var c = l.split(sep).map(function (s) { return s.trim(); });
      var type = (c[idx.type] || "").toLowerCase();
      if ([TYPES.ACHAT, TYPES.CESSION, TYPES.SOULTE_RECUE].indexOf(type) < 0) {
        erreurs.push("CSV ligne " + (i + 2) + " : type « " + type + " » inconnu (achat, cession, soulte_recue).");
        return;
      }
      ops.push({
        date: c[idx.date],
        type: type,
        montant: c[idx.montant],
        valeur_portefeuille: idx.valeur_portefeuille >= 0 ? c[idx.valeur_portefeuille] : "",
        frais: idx.frais >= 0 ? c[idx.frais] : ""
      });
    });
    return { ops: ops, erreurs: erreurs };
  }

  var api = { calculer: calculer, lireCSV: lireCSV, TYPES: TYPES, SEUIL_EXONERATION: SEUIL_EXONERATION };
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.Moteur2086 = api;
})(this);
