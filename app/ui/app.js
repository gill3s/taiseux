// Taiseux — interface. Copyright (c) 2026 ToolGuild. Licence MIT.
// Les données ne vivent qu'en mémoire tant qu'elles ne sont pas enregistrées dans un coffre chiffré.
// Le calcul est fait par le moteur Rust (commande « calculer_foyer ») : l'interface ne calcule rien elle-même.
"use strict";
(function () {
  var T = window.__TAURI__;
  var invoke = T.core.invoke;
  var dlg = T.dialog;

  var euros = new Intl.NumberFormat("fr-FR", { style: "currency", currency: "EUR" });
  var entier = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 0 });
  var qte = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 8 });
  var $ = function (id) { return document.getElementById(id); };
  function el(tag, attrs, kids) {
    var e = document.createElement(tag);
    Object.keys(attrs || {}).forEach(function (k) {
      if (attrs[k] == null) return;
      if (k === "text") e.textContent = attrs[k];
      else if (k === "class") e.className = attrs[k];
      else e.setAttribute(k, attrs[k]);
    });
    (kids || []).forEach(function (c) { if (c) e.appendChild(typeof c === "string" ? document.createTextNode(c) : c); });
    return e;
  }
  // Arrondi à l'euro le plus proche, 0,50 au-dessus en valeur absolue (comme le moteur et la déclaration).
  function euroEntier(x) { return x >= 0 ? Math.floor(x + 0.5) : -Math.floor(-x + 0.5); }
  function num(s) {
    var t = String(s == null ? "" : s).replace(/[\s  €]/g, "").replace(",", ".");
    if (t === "") return null;
    var n = Number(t);
    return isFinite(n) ? n : NaN;
  }
  // Montants en euros : deux décimales ; quantités : toute la précision.
  function fmtSaisie(n) { return n == null ? "" : String(Math.round(n * 100) / 100).replace(".", ","); }
  function fmtQ(n) { return n == null ? "" : String(n).replace(".", ","); }
  function aujourdhui() { return new Date().toISOString().slice(0, 10); }
  function dateFr(d) { var p = d.split("-"); return p.length === 3 ? p[2] + "/" + p[1] + "/" + p[0] : d; }

  // ---------- Foyer ----------
  // Décalage des numéros de ligne du 2086 : déclarant 1 = 211…224, déclarant 2 = 251…264, personne à charge = 311…324.
  var ROLES = {
    declarant1: { libelle: "Déclarant 1", decalage: 0 },
    declarant2: { libelle: "Déclarant 2", decalage: 40 },
    personne_a_charge: { libelle: "Personne à charge", decalage: 100 }
  };
  function nomMembre(m) { return ROLES[m.role].libelle + (m.nom ? " · " + m.nom : ""); }
  function membreVide(role) { return { role: role, nom: "", lignes: [], a_verifier: [] }; }

  // ---------- État ----------
  var etat = {
    membres: [membreVide("declarant1")], courant: 0, cours: [], comptes: [],
    chemin: null, phrase: null, modifie: false, calcul: null, corresp: [], page: 0
  };
  function courant() { return etat.membres[etat.courant]; }
  function toutesLignes() { return etat.membres.reduce(function (n, m) { return n + m.lignes.length; }, 0); }

  function marquer() { etat.modifie = true; afficherEtat(); }
  function afficherEtat() {
    var box = $("etat");
    box.textContent = "";
    var aSauver = etat.modifie && (etat.chemin || toutesLignes());
    box.className = "etat" + (etat.chemin ? " coffre-ouvert" : "") + (aSauver ? " a-sauver" : "");
    if (etat.chemin) {
      box.appendChild(el("strong", { text: "Coffre chiffré" }));
      box.appendChild(el("span", { class: "fichier", text: etat.chemin.split(/[\\/]/).pop() }));
      if (aSauver) box.appendChild(el("span", { class: "modifie", text: "Modifications non enregistrées" }));
    } else {
      // L'avertissement n'apparaît que lorsqu'il y a quelque chose à perdre.
      box.appendChild(el("strong", { text: "Sans trace" }));
      box.appendChild(el("span", { class: aSauver ? "modifie" : "", text: aSauver ? "Non enregistré : tout disparaît à la fermeture." : "Rien n'est écrit sur le disque." }));
    }
    // Le bouton ne prend sa couleur pleine que lorsqu'il y a quelque chose à enregistrer.
    $("b-enregistrer").textContent = etat.chemin ? "Enregistrer" : "Enregistrer dans un coffre…";
    $("b-enregistrer").className = aSauver ? "" : "doux";
  }

  // ---------- Vues ----------
  document.querySelectorAll(".nav button").forEach(function (b) {
    b.addEventListener("click", function () { montrer(b.dataset.vue); });
  });
  function montrer(vue) {
    document.querySelectorAll(".nav button").forEach(function (b) { b.classList.toggle("actif", b.dataset.vue === vue); });
    document.querySelectorAll(".vue").forEach(function (s) { s.hidden = s.id !== "v-" + vue; });
    if (vue === "declaration") afficherDeclaration();
    if (vue === "cours") afficherCours();
    if (vue === "comptes") afficherComptes();
  }
  function vueVisible(v) { return !$("v-" + v).hidden; }

  // ---------- Membres ----------
  function rolesLibres() {
    return ["declarant2", "personne_a_charge"].filter(function (r) { return !etat.membres.some(function (m) { return m.role === r; }); });
  }
  function ajouterMembre() {
    var libres = rolesLibres();
    if (!libres.length) return;
    etat.membres.push(membreVide(libres[0]));
    etat.membres.sort(function (a, b) { return ROLES[a.role].decalage - ROLES[b.role].decalage; });
    etat.courant = etat.membres.findIndex(function (m) { return m.role === libres[0]; });
    etat.page = 0; effacerFiltre(); erreursLignes = {}; marquer(); rendreTout(); recalculer();
  }
  // Les onglets du foyer n'apparaissent qu'à partir de deux membres ; l'ajout se fait par le menu « Plus ».
  function rendreMembres() {
    var box = $("membres"), libres = rolesLibres();
    box.textContent = "";
    $("b-membre").hidden = libres.length === 0;
    $("b-membre").textContent = libres[0] === "declarant2" ? "Ajouter le déclarant 2" : "Ajouter une personne à charge";
    box.hidden = etat.membres.length < 2;
    if (box.hidden) return;
    etat.membres.forEach(function (m, k) {
      var b = el("button", { type: "button", role: "tab", class: "onglet" + (k === etat.courant ? " actif" : ""), "aria-selected": k === etat.courant ? "true" : "false" },
        [nomMembre(m), el("span", { class: "compte", text: String(m.lignes.length) })]);
      b.addEventListener("click", function () { etat.courant = k; etat.page = 0; effacerFiltre(); erreursLignes = {}; rendreTout(); recalculer(); });
      box.appendChild(b);
    });
    var nom = el("input", { type: "text", class: "nom-membre", placeholder: "Prénom (facultatif)", "aria-label": "Prénom du membre" });
    nom.value = courant().nom;
    nom.addEventListener("change", function () { courant().nom = nom.value.trim(); marquer(); rendreMembres(); });
    box.appendChild(nom);
    if (courant().role !== "declarant1") {
      var suppr = el("button", { type: "button", class: "ghost petit danger", text: "Retirer ce membre" });
      suppr.addEventListener("click", function () {
        var m = courant();
        var q = m.lignes.length ? dlg.ask("Ses " + m.lignes.length + " opérations seront retirées du dossier.", { title: "Retirer " + nomMembre(m) + " ?", kind: "warning", okLabel: "Retirer", cancelLabel: "Annuler" }) : Promise.resolve(true);
        q.then(function (ok) {
          if (!ok) return;
          etat.comptes = etat.comptes.filter(function (c) { return c.titulaire !== etat.courant; }).map(function (c) { if (c.titulaire > etat.courant) c.titulaire--; return c; });
          etat.membres.splice(etat.courant, 1); etat.courant = 0; etat.page = 0; effacerFiltre(); erreursLignes = {}; marquer(); rendreTout(); recalculer();
        });
      });
      box.appendChild(suppr);
    }
  }

  // ---------- Lignes (texte saisi) <-> opérations du moteur ----------
  var TYPES = [["achat", "Achat"], ["cession", "Cession"], ["echange", "Échange"], ["transfert", "Transfert"],
    ["revenu_gratuit", "Revenu gratuit"], ["soulte_recue", "Soulte reçue"]];
  var LIB_TYPE = {}; TYPES.forEach(function (t) { LIB_TYPE[t[0]] = t[1]; });

  function nouvelleLigne(type, o) {
    o = o || {};
    return {
      type: type, date: o.date || aujourdhui(), heure: o.heure || "", actif: o.actif || "", quantite: fmtQ(o.quantite),
      montant: (type === "echange" || type === "transfert") ? "" : fmtSaisie(o.montant === 0 && type !== "revenu_gratuit" ? null : o.montant),
      valeur: fmtSaisie(o.valeur_portefeuille), frais: fmtSaisie(o.frais), actif_recu: o.actif_recu || "",
      quantite_recue: fmtQ(o.quantite_recue), valeur_recue: fmtSaisie(o.valeur_recue), soulte: fmtSaisie(o.soulte),
      quantite_frais: fmtQ(o.quantite_frais), plateforme: o.plateforme || "", note: o.note || "", ouvert: false
    };
  }

  // Convertit une ligne en opération. null = ligne encore vide (ignorée sans message) ; une chaîne = erreur.
  function versOperation(l) {
    var champs = ["montant", "quantite", "valeur", "frais", "quantite_recue", "valeur_recue", "soulte", "quantite_frais"];
    var v = {};
    for (var k = 0; k < champs.length; k++) {
      v[champs[k]] = num(l[champs[k]]);
      if (v[champs[k]] !== null && isNaN(v[champs[k]])) return "nombre illisible (" + champs[k].replace("_", " ") + ")";
    }
    if (v.montant == null && v.quantite == null && v.valeur_recue == null && v.soulte == null) return null;
    var cession = l.type === "cession" || l.type === "echange";
    // Seuls les champs renseignés sont transmis (le moteur prend les autres par défaut) : trois fois moins de données
    // à échanger à chaque recalcul.
    var o = { date: l.date, type: l.type, montant: v.montant == null ? 0 : v.montant };
    function si(cle_, x) { if (x != null && x !== "") o[cle_] = x; }
    si("heure", l.heure);
    si("actif", l.actif.trim());
    si("quantite", v.quantite);
    if (cession) { si("valeur_portefeuille", v.valeur); si("frais", v.frais); si("soulte", v.soulte); }
    if (l.type === "echange") { si("actif_recu", l.actif_recu.trim()); si("quantite_recue", v.quantite_recue); si("valeur_recue", v.valeur_recue); }
    if (l.type === "transfert") si("quantite_frais", v.quantite_frais);
    si("plateforme", l.plateforme.trim());
    si("note", l.note);
    return o;
  }

  // La conversion est gardée sur la ligne tant qu'elle n'est pas modifiée (un gros dossier compte des milliers de
  // lignes, et chaque frappe demande un recalcul).
  function operationDe(l) { return l._op === undefined ? (l._op = versOperation(l)) : l._op; }
  function touche(l) { l._op = undefined; }

  function operationsMembre(m, erreurs) {
    var ops = [], corresp = [];
    m.lignes.forEach(function (l, i) {
      var o = operationDe(l);
      if (o === null) return;
      if (typeof o === "string") { if (erreurs) erreurs.push("Ligne " + (i + 1) + " : " + o + "."); return; }
      ops.push(o); corresp.push(i);
    });
    return { ops: ops, corresp: corresp };
  }
  // Clé d'une opération, pour repérer les doublons d'un import déjà fait.
  function cle(o) {
    var r = function (x) { return x == null ? "" : Math.round(x * 1e8) / 1e8; };
    return [o.date, o.heure, o.type, (o.actif || "").toUpperCase(), r(o.quantite), r(o.montant), (o.actif_recu || "").toUpperCase(), r(o.quantite_recue)].join("|");
  }

  // ---------- Tableau des opérations ----------
  var PAR_PAGE = 150;
  var filtre = { texte: "", annee: "", type: "" };
  var erreursLignes = {};   // indice de ligne -> message d'erreur (membre courant)
  var eclairer = null;      // indice de ligne à mettre en évidence au prochain rendu

  // Tri par date puis heure ; à égalité, l'ordre de saisie est gardé.
  function trier(m) {
    m.lignes.forEach(function (l, k) { l._rang = k; });
    m.lignes.sort(function (x, y) {
      return x.date < y.date ? -1 : x.date > y.date ? 1 : (x.heure || "") < (y.heure || "") ? -1 : (x.heure || "") > (y.heure || "") ? 1 : x._rang - y._rang;
    });
  }

  function filtreActif() { return !!(filtre.texte || filtre.annee || filtre.type); }
  function indicesVisibles(m) {
    var t = filtre.texte.trim().toLowerCase(), out = [];
    m.lignes.forEach(function (l, i) {
      if (filtre.annee && l.date.slice(0, 4) !== filtre.annee) return;
      if (filtre.type === "erreurs") { if (!erreursLignes[i]) return; }
      else if (filtre.type && l.type !== filtre.type) return;
      if (t && (l.actif + " " + l.actif_recu + " " + l.plateforme + " " + l.note + " " + LIB_TYPE[l.type]).toLowerCase().indexOf(t) < 0) return;
      out.push(i);
    });
    return out;
  }

  function rendreFiltres(m, nbVisibles) {
    // La recherche et les filtres n'apparaissent que lorsqu'il y a de quoi chercher.
    $("filtres").hidden = m.lignes.length < 8 && !filtreActif();
    var annees = {};
    m.lignes.forEach(function (l) { annees[l.date.slice(0, 4)] = 1; });
    var as = Object.keys(annees).sort();
    if (filtre.annee && !annees[filtre.annee]) filtre.annee = "";
    var sa = $("f-annee");
    sa.textContent = "";
    sa.appendChild(el("option", { value: "", text: "Toutes les années" }));
    as.forEach(function (a) { sa.appendChild(el("option", { value: a, text: a })); });
    sa.value = filtre.annee;
    var st = $("f-type");
    st.textContent = "";
    st.appendChild(el("option", { value: "", text: "Tous les types" }));
    TYPES.forEach(function (t) { st.appendChild(el("option", { value: t[0], text: t[1] })); });
    if (Object.keys(erreursLignes).length) st.appendChild(el("option", { value: "erreurs", text: "Lignes en erreur" }));
    else if (filtre.type === "erreurs") filtre.type = "";
    st.value = filtre.type;
    $("f-compte").textContent = filtreActif() ? nbVisibles + " sur " + m.lignes.length + " opérations" : m.lignes.length + " opération" + (m.lignes.length > 1 ? "s" : "");
    $("f-effacer").hidden = !filtreActif();
  }
  $("f-texte-ops").addEventListener("input", function () { filtre.texte = this.value; etat.page = 0; rendreTableau(); afficherPlusValues(); });
  $("f-annee").addEventListener("change", function () { filtre.annee = this.value; etat.page = 0; rendreTableau(); afficherPlusValues(); });
  $("f-type").addEventListener("change", function () { filtre.type = this.value; etat.page = 0; rendreTableau(); afficherPlusValues(); });
  function effacerFiltre() { filtre = { texte: "", annee: "", type: "" }; $("f-texte-ops").value = ""; }
  $("f-effacer").addEventListener("click", function () { effacerFiltre(); etat.page = 0; rendreTableau(); afficherPlusValues(); });

  function rendreTableau() {
    var m = courant(), tb = $("lignes");
    tb.textContent = "";
    var vis = indicesVisibles(m);
    var pages = Math.max(1, Math.ceil(vis.length / PAR_PAGE));
    if (etat.page >= pages) etat.page = pages - 1;
    var debut = etat.page * PAR_PAGE, fin = Math.min(vis.length, debut + PAR_PAGE);
    for (var k = debut; k < fin; k++) rendreLigne(tb, m.lignes[vis[k]], vis[k]);
    $("vide").hidden = m.lignes.length > 0;
    $("vide-filtre").hidden = !(m.lignes.length > 0 && vis.length === 0);
    $("table-ops-zone").hidden = vis.length === 0;
    rendreFiltres(m, vis.length);
    var pg = $("pager");
    pg.textContent = "";
    if (pages > 1) {
      var prec = el("button", { type: "button", class: "ghost petit", text: "‹ Précédentes" });
      var suiv = el("button", { type: "button", class: "ghost petit", text: "Suivantes ›" });
      prec.disabled = etat.page === 0; suiv.disabled = etat.page >= pages - 1;
      prec.addEventListener("click", function () { etat.page--; rendreTableau(); afficherPlusValues(); });
      suiv.addEventListener("click", function () { etat.page++; rendreTableau(); afficherPlusValues(); });
      pg.appendChild(prec);
      pg.appendChild(el("span", { text: "Opérations " + (debut + 1) + " à " + fin + " sur " + vis.length }));
      pg.appendChild(suiv);
    }
    rendreAVerifier();
    if (eclairer != null) {
      var tr = tb.querySelector('tr.op[data-i="' + eclairer + '"]');
      eclairer = null;
      if (tr) { tr.classList.add("eclat"); tr.scrollIntoView({ block: "center" }); }
    }
  }

  // Montre une ligne : retire le filtre s'il la cache, va à sa page, la met en évidence.
  function allerA(i) {
    var m = courant();
    if (indicesVisibles(m).indexOf(i) < 0) effacerFiltre();
    etat.page = Math.floor(indicesVisibles(m).indexOf(i) / PAR_PAGE);
    eclairer = i;
    rendreTableau(); afficherPlusValues();
  }

  // Le tableau se lit ; il ne se saisit pas. Un clic sur une ligne ouvre sa fiche (la même que pour une
  // nouvelle opération), où chaque champ est expliqué au moment où on le remplit.
  function nEur(s) { var n = num(s); return n == null || isNaN(n) ? "" : euros.format(n); }
  function nQte(s) { var n = num(s); return n == null || isNaN(n) ? "" : qte.format(n); }
  var LIB_COURT = { achat: "Achat", cession: "Vente", echange: "Échange", transfert: "Transfert", revenu_gratuit: "Reçu", soulte_recue: "Soulte" };

  function detailLigne(l) {
    var a = l.actif.trim().toUpperCase(), q = nQte(l.quantite);
    var quoi = (q ? q + " " : "") + a;
    if (l.type === "echange") {
      var b = l.actif_recu.trim().toUpperCase(), qb = nQte(l.quantite_recue);
      return [quoi || "…", el("span", { class: "fleche", text: " → " }), ((qb ? qb + " " : "") + b) || "…"];
    }
    return [quoi || el("span", { class: "fine", text: "—" })];
  }
  function montantLigne(l) {
    if (l.type === "transfert") return "";
    if (l.type === "echange") return num(l.soulte) ? "soulte " + nEur(l.soulte) : "";
    if (l.type === "revenu_gratuit") return num(l.montant) ? nEur(l.montant) : "";
    return nEur(l.montant);
  }
  // Sous la crypto : l'erreur de la ligne s'il y en a une, sinon la note.
  function sousTexte(l, i) {
    if (erreursLignes[i]) return el("small", { class: "err-ligne", text: erreursLignes[i] });
    // La note qu'un import écrit lui-même (« Kraken : échange R12 ») reste dans la fiche : seule une note de
    // l'utilisateur mérite une place dans le tableau.
    if (!l.note || (l.plateforme && l.note.indexOf(l.plateforme + " : ") === 0)) return null;
    return el("small", { class: "note", text: l.note, title: l.note });
  }

  function supprimerLigne(l) {
    var m = courant(), i = m.lignes.indexOf(l);
    if (i < 0) return;
    m.lignes.splice(i, 1);
    marquer(); rendreTableau(); rendreMembres(); planifier();
    annulable(LIB_COURT[l.type] + " du " + dateFr(l.date) + " : opération supprimée.", function () {
      m.lignes.splice(Math.min(i, m.lignes.length), 0, l);
      marquer(); rendreTout(); planifier();
    });
  }

  function rendreLigne(tb, l, i) {
    var suppr = el("button", { type: "button", class: "x", "aria-label": "Supprimer l'opération", title: "Supprimer", text: "×" });
    suppr.addEventListener("click", function (e) { e.stopPropagation(); supprimerLigne(l); });
    var frais = l.type === "cession" && num(l.frais) ? el("small", { text: "frais " + nEur(l.frais) }) : null;
    var tr = el("tr", { class: "op " + l.type + (erreursLignes[i] ? " faute" : ""), "data-i": String(i), tabindex: "0" }, [
      el("td", { class: "date", text: dateFr(l.date) }),
      el("td", {}, [el("span", { class: "type " + l.type, text: LIB_COURT[l.type] || l.type })]),
      el("td", { class: "quoi" }, detailLigne(l).concat([sousTexte(l, i)])),
      el("td", { class: "n" }, [montantLigne(l), frais]),
      el("td", { class: "plat", text: l.plateforme }),
      el("td", { class: "n pv" }),
      el("td", { class: "fin" }, [suppr])
    ]);
    tr.addEventListener("click", function () { ouvrirFiche(null, l); });
    tr.addEventListener("keydown", function (e) {
      if (e.target === tr && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); ouvrirFiche(null, l); }
    });
    tb.appendChild(tr);
  }

  // ---------- Annulation (bandeau en bas de fenêtre) ----------
  var minuterieToast = null, actionAnnuler = null;
  function annulable(texte, annuler) {
    clearTimeout(minuterieToast);
    $("toast-texte").textContent = texte;
    $("toast-annuler").hidden = !annuler;
    actionAnnuler = annuler;
    $("toast").hidden = false;
    minuterieToast = setTimeout(function () { $("toast").hidden = true; actionAnnuler = null; }, annuler ? 9000 : 4000);
  }
  $("toast-annuler").addEventListener("click", function () {
    var f = actionAnnuler;
    clearTimeout(minuterieToast); $("toast").hidden = true; actionAnnuler = null;
    if (f) f();
  });

  // ---------- Suggestions (cryptos et plateformes déjà saisies) ----------
  var plateformesConnues = [];
  function rendreSuggestions() {
    var actifs = {}, plats = {};
    ["BTC", "ETH", "USDT", "USDC", "SOL", "XRP", "ADA", "BNB", "DOT", "LINK"].forEach(function (a) { actifs[a] = 1; });
    plateformesConnues.forEach(function (p) { plats[p] = 1; });
    etat.membres.forEach(function (m) { m.lignes.forEach(function (l) {
      if (l.actif.trim()) actifs[l.actif.trim().toUpperCase()] = 1;
      if (l.actif_recu.trim()) actifs[l.actif_recu.trim().toUpperCase()] = 1;
      if (l.plateforme.trim()) plats[l.plateforme.trim()] = 1;
    }); });
    var remplir = function (id, o) { var d = $(id); d.textContent = ""; Object.keys(o).sort().forEach(function (k) { d.appendChild(el("option", { value: k })); }); };
    remplir("dl-actifs", actifs); remplir("dl-plateformes", plats);
  }

  // ---------- Fiche d'opération (saisie et modification) ----------
  // Chaque type n'affiche que ses champs. Un champ : [clé, libellé, genre, obligatoire, aide]. L'aide n'apparaît
  // que lorsque le champ a le curseur, dans le panneau de droite : l'explication arrive au moment de remplir.
  var AIDE = {
    date: "Le jour de l'opération, tel qu'il figure sur le relevé de la plateforme.",
    plateforme: "Où l'opération a eu lieu : une plateforme ou un portefeuille. Facultatif ; sert à préparer la liste des comptes à l'étranger.",
    heure: "Facultative. Sert seulement à remettre dans l'ordre plusieurs opérations du même jour.",
    note: "Pour vous. N'entre dans aucun calcul.",
    actif: "Son code, comme sur la plateforme : BTC, ETH, SOL… Avec la crypto et la quantité, Taiseux suit vos avoirs et calcule lui-même la valeur de votre portefeuille à chaque vente.",
    quantite: "Le nombre d'unités, par exemple 0,015.",
    valeur: "La valeur de toutes vos cryptos, sur toutes vos plateformes et portefeuilles, juste avant cette opération (ligne 212 du formulaire). Laissez vide : Taiseux la calcule à partir de vos avoirs et des cours. Si vous la saisissez, c'est votre valeur qui est retenue."
  };
  var FICHES = {
    achat: {
      libelle: "Achat", resume: "Des euros contre de la crypto",
      aide: "Un achat n'est pas imposable. Son prix entre dans votre prix d'acquisition, qui réduira la plus-value de vos ventes.",
      champs: [["date", "Date", "date", true], ["actif", "Crypto achetée", "actif"], ["quantite", "Quantité achetée", "qte"],
        ["montant", "Prix payé, en euros", "eur", true, "Tout ce que l'achat vous a coûté en euros, frais compris. C'est votre prix d'acquisition."],
        ["plateforme", "Plateforme ou portefeuille", "plateforme"]],
      avance: [["heure", "Heure", "heure"], ["note", "Note", "texte"]]
    },
    cession: {
      libelle: "Vente", resume: "De la crypto contre des euros, un bien ou un service",
      aide: "Vendre contre des euros, ou payer un bien ou un service en crypto, est une cession imposable : c'est elle qui figure sur le formulaire 2086.",
      champs: [["date", "Date", "date", true], ["actif", "Crypto vendue", "actif"], ["quantite", "Quantité vendue", "qte"],
        ["montant", "Prix de vente, en euros", "eur", true, "Le prix de vente en euros, avant déduction des frais (ligne 213 du formulaire)."],
        ["frais", "Frais de vente, en euros", "eur", false, "Les frais prélevés pour cette vente (ligne 214). Ils réduisent la plus-value."],
        ["plateforme", "Plateforme ou portefeuille", "plateforme"]],
      avance: [["valeur", "Valeur globale du portefeuille, en euros", "eur"],
        ["soulte", "Soulte, en euros", "eur", false, "« Soulte » est le mot du formulaire pour des euros qui complètent une opération (ligne 216). Reçue en plus du prix : montant positif. Versée : montant négatif. Une vente ordinaire n'en a pas."],
        ["heure", "Heure", "heure"], ["note", "Note", "texte"]]
    },
    echange: {
      libelle: "Échange", resume: "Une crypto contre une autre",
      aide: "Sans euros en plus, un échange n'est pas imposable : Taiseux met seulement vos quantités à jour, pour que la valeur de votre portefeuille reste juste.",
      champs: [["date", "Date", "date", true], ["actif", "Crypto donnée", "actif", true, "Le code de la crypto que vous donnez."],
        ["quantite", "Quantité donnée", "qte", true],
        ["actif_recu", "Crypto reçue", "actif", true, "Le code de la crypto que vous recevez."], ["quantite_recue", "Quantité reçue", "qte", true, "Le nombre d'unités reçues."],
        ["plateforme", "Plateforme ou portefeuille", "plateforme"]],
      avance: [["soulte", "Soulte, en euros", "eur", false, "Des euros reçus en plus de la crypto (montant positif) ou versés en plus (montant négatif). Avec une soulte, l'échange devient une cession imposable."],
        ["valeur_recue", "Valeur de la crypto reçue, en euros", "eur", false, "Ce que vaut en euros la crypto reçue, au moment de l'échange. Nécessaire seulement s'il y a une soulte."],
        ["frais", "Frais, en euros", "eur", false, "Les frais payés en euros pour cet échange, s'il comporte une soulte."],
        ["valeur", "Valeur globale du portefeuille, en euros", "eur", false, "Seulement s'il y a une soulte. " + AIDE.valeur],
        ["heure", "Heure", "heure"], ["note", "Note", "texte"]]
    },
    transfert: {
      libelle: "Transfert", resume: "Entre vos propres comptes",
      aide: "Déplacer de la crypto entre vos propres comptes ou portefeuilles n'a aucun effet fiscal. Seuls les frais de réseau diminuent vos avoirs.",
      champs: [["date", "Date", "date", true], ["actif", "Crypto transférée", "actif", true, "Le code de la crypto déplacée."],
        ["quantite", "Quantité transférée", "qte", true],
        ["quantite_frais", "Frais de réseau, en crypto", "qte", false, "Les frais de réseau payés dans la même crypto : ils diminuent vos avoirs."],
        ["plateforme", "Plateforme ou portefeuille", "plateforme"]],
      avance: [["heure", "Heure", "heure"], ["note", "Note", "texte"]]
    },
    revenu_gratuit: {
      libelle: "Crypto reçue", resume: "Airdrop, récompense, cadeau",
      aide: "Une crypto reçue sans rien payer entre dans votre portefeuille avec un prix d'acquisition de 0 €, sauf indication contraire.",
      champs: [["date", "Date", "date", true], ["actif", "Crypto reçue", "actif", true, "Le code de la crypto reçue."],
        ["quantite", "Quantité reçue", "qte", true], ["plateforme", "Plateforme ou portefeuille", "plateforme"]],
      avance: [["montant", "Prix d'acquisition retenu, en euros", "eur", false, "0 € par défaut, puisque vous n'avez rien payé. Les récompenses de staking sont comptées ainsi, un traitement que la doctrine discute."],
        ["heure", "Heure", "heure"], ["note", "Note", "texte"]]
    },
    // Type hérité de la version 0.1 (fichiers CSV sans crypto ni quantité) : modifiable, mais plus proposé.
    soulte_recue: {
      libelle: "Soulte reçue", resume: "Euros reçus lors d'un échange",
      aide: "Des euros reçus lors d'un échange passé, saisis à part. Pour un nouvel échange, choisissez « Échange » et indiquez la soulte dans « Plus de détails ».",
      champs: [["date", "Date", "date", true], ["montant", "Soulte reçue, en euros", "eur", true, "Les euros reçus en plus de la crypto lors de l'échange."]],
      avance: [["heure", "Heure", "heure"], ["note", "Note", "texte"]]
    }
  };
  var ORDRE_FICHES = ["achat", "cession", "echange", "transfert", "revenu_gratuit"];
  var CLES_FICHE = ["date", "heure", "actif", "quantite", "montant", "valeur", "frais", "actif_recu", "quantite_recue", "valeur_recue", "soulte", "quantite_frais", "plateforme", "note"];
  // ligne : l'opération en cours de modification (null pour une nouvelle) ; apres : à faire une fois l'ajout réussi.
  var fiche_ = { type: "achat", valeurs: {}, champs: {}, ligne: null, apres: null };

  function phraseFiche() {
    var v = fiche_.valeurs, t = fiche_.type;
    var q = function (k) { var n = num(v[k]); return n == null || isNaN(n) ? "…" : qte.format(n); };
    var e = function (k) { var n = num(v[k]); return n == null || isNaN(n) ? "…" : euros.format(n); };
    var a = function (k) { return (v[k] || "").trim().toUpperCase() || "…"; };
    var d = v.date ? "Le " + dateFr(v.date) + ", " : "";
    var ou = (v.plateforme || "").trim() ? " (" + v.plateforme.trim() + ")" : "";
    if (t === "achat") return d + "vous achetez " + (v.quantite || v.actif ? q("quantite") + " " + a("actif") + " " : "de la crypto ") + "pour " + e("montant") + ou + ".";
    if (t === "cession") return d + "vous vendez " + (v.quantite || v.actif ? q("quantite") + " " + a("actif") + " " : "de la crypto ") + "pour " + e("montant") +
      (num(v.frais) ? ", dont " + e("frais") + " de frais" : "") + ou + ". Cession imposable.";
    if (t === "echange") return d + "vous échangez " + q("quantite") + " " + a("actif") + " contre " + q("quantite_recue") + " " + a("actif_recu") + ou + ". " +
      (num(v.soulte) ? "Avec une soulte de " + e("soulte") + " : cession imposable." : "Pas d'impôt sur cet échange.");
    if (t === "transfert") return d + "vous transférez " + q("quantite") + " " + a("actif") + (num(v.quantite_frais) ? " (frais de réseau : " + q("quantite_frais") + " " + a("actif") + ")" : "") + ou + ". Aucun effet fiscal.";
    if (t === "soulte_recue") return d + "vous recevez une soulte de " + e("montant") + ".";
    return d + "vous recevez " + q("quantite") + " " + a("actif") + " sans rien payer" + ou + ".";
  }

  function conseil(titre, texte) { $("op-conseil-t").textContent = titre; $("op-aide").textContent = texte; }
  function conseilType() { conseil(FICHES[fiche_.type].libelle, FICHES[fiche_.type].aide); }
  // Valeur globale calculée par le moteur pour l'opération en cours de modification, s'il y en a une.
  function valeurCalculee() {
    var r = etat.calcul && etat.calcul.membres[etat.courant], c = etat.corresp[etat.courant];
    if (!fiche_.ligne || !r || !c) return null;
    var i = courant().lignes.indexOf(fiche_.ligne);
    var x = r.cessions.filter(function (s) { return c[s.operation - 1] === i; })[0];
    return x && x.valeur_calculee ? x.valeur_portefeuille : null;
  }
  function aideChamp(c) {
    var t = c[4] || AIDE[c[0]] || "";
    if (c[0] === "valeur" && !fiche_.valeurs.valeur) {
      var vc = valeurCalculee();
      if (vc != null) t += " Pour cette opération, Taiseux a calculé " + euros.format(vc) + ".";
    }
    return t;
  }

  function rendreFiche() {
    var F = FICHES[fiche_.type], modif = !!fiche_.ligne;
    $("op-titre").textContent = modif ? "Modifier l'opération" : "Nouvelle opération";
    var types = $("op-types");
    types.textContent = "";
    ORDRE_FICHES.concat(fiche_.type === "soulte_recue" ? ["soulte_recue"] : []).forEach(function (t) {
      var b = el("button", { type: "button", role: "radio", "aria-checked": t === fiche_.type ? "true" : "false", class: "type" + (t === fiche_.type ? " actif" : "") },
        [el("strong", { text: FICHES[t].libelle }), el("span", { text: FICHES[t].resume })]);
      b.addEventListener("click", function () { fiche_.type = t; rendreFiche(); premierChampVide(); });
      types.appendChild(b);
    });
    fiche_.champs = {};
    function construire(zone, liste) {
      zone.textContent = "";
      liste.forEach(function (c) {
        var attrs = { autocomplete: "off", spellcheck: "false" };
        if (c[2] === "date") attrs.type = "date";
        else if (c[2] === "eur" || c[2] === "qte") { attrs.inputmode = "decimal"; attrs.class = "num"; attrs.placeholder = c[2] === "eur" ? "0,00" : "0"; }
        else if (c[2] === "actif") { attrs.list = "dl-actifs"; attrs.class = "actif"; attrs.placeholder = "BTC"; }
        else if (c[2] === "plateforme") { attrs.list = "dl-plateformes"; attrs.placeholder = "Kraken, Ledger…"; }
        else if (c[2] === "heure") attrs.placeholder = "HH:MM:SS";
        if (c[0] === "valeur") attrs.placeholder = "calculée par Taiseux";
        var inp = el("input", attrs);
        inp.value = fiche_.valeurs[c[0]] || "";
        inp.addEventListener("input", function () { fiche_.valeurs[c[0]] = inp.value; inp.classList.remove("faux"); $("op-phrase").textContent = phraseFiche(); $("op-erreur").textContent = ""; });
        inp.addEventListener("focus", function () { conseil(c[1].split(",")[0], aideChamp(c)); });
        inp.addEventListener("blur", conseilType);
        fiche_.champs[c[0]] = inp;
        zone.appendChild(el("label", { class: c[2] === "texte" ? "large" : "" }, [el("span", {}, [c[1], c[3] ? el("em", { text: " *", title: "Obligatoire" }) : null]), inp]));
      });
    }
    construire($("op-champs"), F.champs);
    construire($("op-avance"), F.avance);
    $("op-avance-zone").open = F.avance.some(function (c) { return !!fiche_.valeurs[c[0]]; });
    $("op-phrase").textContent = phraseFiche();
    $("op-erreur").textContent = "";
    conseilType();
    $("op-supprimer").hidden = $("op-dupliquer").hidden = !modif;
    $("op-autre").hidden = modif;
    $("op-ok").textContent = modif ? "Enregistrer" : "Ajouter";
    $("op-annuler").textContent = modif ? "Annuler" : "Fermer";
  }
  function premierChampVide() {
    var F = FICHES[fiche_.type];
    for (var k = 0; k < F.champs.length; k++) {
      var c = F.champs[k][0];
      if (c !== "date" && !fiche_.valeurs[c]) { fiche_.champs[c].focus(); return; }
    }
    fiche_.champs[F.champs[1][0]].focus();
  }

  // Contrôle la fiche ; rend la ligne correspondante, ou null après avoir signalé ce qui manque.
  function ligneDeFiche() {
    var F = FICHES[fiche_.type], v = fiche_.valeurs, manque = [];
    F.champs.concat(F.avance).forEach(function (c) {
      var inp = fiche_.champs[c[0]], brut = (v[c[0]] || "").trim(), faux = false;
      if (c[3] && !brut) faux = true;
      if (brut && (c[2] === "eur" || c[2] === "qte")) { var n = num(brut); if (isNaN(n) || (n < 0 && c[0] !== "soulte")) faux = true; }
      inp.classList.toggle("faux", faux);
      if (faux) manque.push(c[1].toLowerCase());
    });
    if (manque.length) {
      $("op-erreur").textContent = "À compléter ou corriger : " + manque.join(" ; ") + ".";
      if (F.avance.some(function (c) { return fiche_.champs[c[0]].classList.contains("faux"); })) $("op-avance-zone").open = true;
      var premier = $("f-op").querySelector("input.faux");
      if (premier) premier.focus();
      return null;
    }
    if (fiche_.champs.actif && ((v.actif || "").trim() && !(v.quantite || "").trim() || !(v.actif || "").trim() && (v.quantite || "").trim())) {
      $("op-erreur").textContent = "Indiquez la crypto et la quantité ensemble (ou aucune des deux).";
      fiche_.champs[(v.actif || "").trim() ? "quantite" : "actif"].focus();
      return null;
    }
    // Seuls les champs de ce type sont repris : en changeant de type, les valeurs sans objet ne suivent pas.
    var l = nouvelleLigne(fiche_.type, {});
    F.champs.concat(F.avance).forEach(function (c) { l[c[0]] = (v[c[0]] || "").trim(); });
    l.actif = l.actif.toUpperCase(); l.actif_recu = l.actif_recu.toUpperCase();
    return l;
  }

  function validerFiche(continuer) {
    var l = ligneDeFiche();
    if (!l) return;
    var m = courant(), cible = fiche_.ligne;
    if (cible && m.lignes.indexOf(cible) >= 0) {
      CLES_FICHE.forEach(function (k) { cible[k] = l[k]; });
      cible.type = l.type;
      touche(cible);
      l = cible;
    } else {
      l._neuf = true;
      m.lignes.push(l);
    }
    trier(m);
    if (indicesVisibles(m).indexOf(m.lignes.indexOf(l)) < 0) effacerFiltre();
    eclairer = m.lignes.indexOf(l);
    etat.page = Math.floor(indicesVisibles(m).indexOf(eclairer) / PAR_PAGE);
    marquer(); rendreTout(); rendreSuggestions(); planifier();
    if (fiche_.apres) { fiche_.apres(); fiche_.apres = null; }
    if (!continuer) { $("dlg-op").close(); return; }
    // Saisie en série : on garde la date et la plateforme, on vide le reste.
    fiche_.valeurs = { date: fiche_.valeurs.date, plateforme: fiche_.valeurs.plateforme || "" };
    rendreFiche();
    annulable("Opération ajoutée. Saisissez la suivante.", null);
    premierChampVide();
  }

  // ligne : opération à modifier ; pre : valeurs de départ d'une nouvelle opération (copie, ligne d'import à saisir).
  function ouvrirFiche(type, ligne, pre) {
    fiche_.ligne = ligne || null;
    fiche_.apres = null;
    if (ligne) {
      fiche_.type = FICHES[ligne.type] ? ligne.type : "achat";
      fiche_.valeurs = {};
      CLES_FICHE.forEach(function (k) { fiche_.valeurs[k] = ligne[k] || ""; });
    } else {
      fiche_.type = type || fiche_.type;
      if (fiche_.type === "soulte_recue") fiche_.type = "achat";
      fiche_.valeurs = pre || { date: fiche_.valeurs.date || aujourdhui(), plateforme: fiche_.valeurs.plateforme || "" };
    }
    rendreSuggestions();
    rendreFiche();
    if (!$("dlg-op").open) $("dlg-op").showModal();
    if (!ligne) premierChampVide();
  }
  $("f-op").addEventListener("submit", function (e) { e.preventDefault(); validerFiche(false); });
  $("op-autre").addEventListener("click", function () { validerFiche(true); });
  $("op-annuler").addEventListener("click", function () { $("dlg-op").close(); });
  // Un message de la fiche (« saisissez la suivante ») n'a plus de sens une fois la fiche fermée.
  $("dlg-op").addEventListener("close", function () { if (!actionAnnuler) { clearTimeout(minuterieToast); $("toast").hidden = true; } });
  $("op-supprimer").addEventListener("click", function () { var l = fiche_.ligne; $("dlg-op").close(); if (l) supprimerLigne(l); });
  $("op-dupliquer").addEventListener("click", function () {
    var v = {};
    CLES_FICHE.forEach(function (k) { v[k] = fiche_.valeurs[k] || ""; });
    ouvrirFiche(fiche_.type, null, v);
    annulable("Copie prête : ajustez-la, puis ajoutez-la.", null);
  });
  $("f-op").addEventListener("keydown", function (e) { if (e.key === "Enter" && e.shiftKey && !fiche_.ligne) { e.preventDefault(); validerFiche(true); } });
  $("b-nouvelle").addEventListener("click", function () { ouvrirFiche(); });
  $("d-saisie").addEventListener("click", function () { ouvrirFiche("achat"); });
  $("d-exemple").addEventListener("click", function () { $("b-exemple").click(); });
  $("d-import").addEventListener("click", function () { $("b-import").click(); });
  $("b-membre").addEventListener("click", ajouterMembre);

  // Menu « Plus » : les actions rares, hors de la vue tant qu'on ne les cherche pas.
  function fermerMenu() { $("menu-ops").hidden = true; $("b-plus").setAttribute("aria-expanded", "false"); }
  $("b-plus").addEventListener("click", function (e) {
    e.stopPropagation();
    var ouvrir = $("menu-ops").hidden;
    fermerMenu();
    if (ouvrir) { $("menu-ops").hidden = false; $("b-plus").setAttribute("aria-expanded", "true"); }
  });
  document.addEventListener("click", fermerMenu);
  document.addEventListener("keydown", function (e) { if (e.key === "Escape") fermerMenu(); });

  // ---------- Lignes à vérifier (imports) ----------
  function rendreAVerifier() {
    var m = courant(), box = $("averifier");
    box.hidden = m.a_verifier.length === 0;
    if (box.hidden) return;
    $("averifier-titre").textContent = m.a_verifier.length + " ligne" + (m.a_verifier.length > 1 ? "s" : "") + " d'import à vérifier";
    var liste = $("averifier-liste");
    liste.textContent = "";
    var tout = el("button", { type: "button", class: "ghost petit", text: "Tout écarter" });
    tout.addEventListener("click", function () { m.a_verifier = []; marquer(); rendreAVerifier(); });
    liste.appendChild(el("div", { class: "barre" }, [tout]));
    m.a_verifier.slice(0, 200).forEach(function (a, k) {
      var saisir = el("button", { type: "button", class: "ghost petit", text: "Saisir" });
      var ecarter = el("button", { type: "button", class: "ghost petit", text: "Écarter" });
      saisir.addEventListener("click", function () {
        ouvrirFiche("achat", null, { date: a.date || aujourdhui(), plateforme: "", note: "À vérifier : " + a.motif + " — " + a.contenu.split("\n")[0].slice(0, 160) });
        // La ligne d'import ne quitte la liste qu'une fois l'opération réellement ajoutée.
        fiche_.apres = function () { var k2 = m.a_verifier.indexOf(a); if (k2 >= 0) { m.a_verifier.splice(k2, 1); marquer(); rendreAVerifier(); } };
      });
      ecarter.addEventListener("click", function () { m.a_verifier.splice(k, 1); marquer(); rendreAVerifier(); });
      liste.appendChild(el("div", { class: "av" }, [
        el("div", { class: "av-tete" }, [el("strong", { text: (a.date ? dateFr(a.date) + " · " : "") + a.motif }), el("span", { class: "fine", text: "ligne " + a.ligne }), saisir, ecarter]),
        el("pre", { text: a.contenu })
      ]));
    });
    if (m.a_verifier.length > 200) liste.appendChild(el("p", { class: "fine", text: "… et " + (m.a_verifier.length - 200) + " autres." }));
  }

  // ---------- Calcul ----------
  var minuterie = null;
  function planifier() { clearTimeout(minuterie); minuterie = setTimeout(recalculer, 150); }

  // Seuls les cours des jours de cession ou d'échange servent au calcul : inutile d'envoyer au moteur les milliers
  // de lignes d'un fichier de cours quotidiens à chaque frappe.
  function coursUtiles(parMembre) {
    if (!etat.cours.length) return etat.cours;
    var jours = {};
    parMembre.forEach(function (p) { p.ops.forEach(function (o) { if (o.type === "cession" || o.type === "echange") jours[o.date] = 1; }); });
    return etat.cours.filter(function (c) { return jours[c.date] === 1; });
  }

  var tour = 0;
  function recalculer() {
    clearTimeout(minuterie);
    var erreursLecture = [], monTour = ++tour;
    var parMembre = etat.membres.map(function (m) { return operationsMembre(m, m === courant() ? erreursLecture : null); });
    return invoke("calculer_foyer", { membres: parMembre.map(function (p) { return p.ops; }), cours: coursUtiles(parMembre) }).then(function (r) {
      if (monTour !== tour) return; // une réponse plus récente est attendue : celle-ci est périmée
      etat.calcul = r;
      etat.corresp = parMembre.map(function (p) { return p.corresp; });
      var rm = r.membres[etat.courant];
      // Les messages du moteur numérotent les opérations envoyées : on les ramène aux lignes du tableau.
      var c = etat.corresp[etat.courant];
      var renum = function (s) { return s.replace(/^Opération (\d+)/, function (_, n) { return "Ligne " + (c[Number(n) - 1] + 1); }); };
      var erreurs = erreursLecture.concat(rm.erreurs.map(renum));
      var avant = Object.keys(erreursLignes).join(",");
      erreursLignes = {};
      erreurs.forEach(function (e) { var x = /^Ligne (\d+) : (.*)$/.exec(e); if (x && !erreursLignes[x[1] - 1]) erreursLignes[x[1] - 1] = x[2]; });
      if (Object.keys(erreursLignes).join(",") !== avant) marquerFautes();
      afficherMessages(erreurs, rm.avertissements.map(renum));
      afficherPlusValues();
      afficherAnnees();
      var nb = r.membres.reduce(function (n, x) { return n + x.cours_manquants.length; }, 0);
      $("pastille-cours").hidden = nb === 0;
      $("pastille-cours").textContent = String(nb);
      if (vueVisible("declaration")) afficherDeclaration();
      if (vueVisible("cours")) afficherCours();
      if (vueVisible("comptes")) afficherComptes();
    }).catch(function (e) { afficherMessages(["Erreur de calcul : " + e], []); });
  }

  // Met à jour le repère « en erreur » des lignes affichées, sans tout redessiner.
  function marquerFautes() {
    var m = courant();
    $("lignes").querySelectorAll("tr.op").forEach(function (tr) {
      var i = Number(tr.dataset.i), cell = tr.querySelector(".quoi"), s = cell.querySelector("small"), l = m.lignes[i];
      tr.classList.toggle("faute", !!erreursLignes[i]);
      if (s) s.remove();
      var n = l && sousTexte(l, i);
      if (n) cell.appendChild(n);
    });
    rendreFiltres(m, indicesVisibles(m).length);
  }

  // ---------- « À compléter » : ce qui manque pour un résultat exact, et rien d'autre ----------
  function tache(classe, contenu, action) {
    return el("div", { class: "tache " + classe }, [el("span", { class: "puce", "aria-hidden": "true" }), el("div", { class: "tache-texte" }, contenu), action]);
  }
  // Un message du moteur qui commence par « Ligne N » devient « Vente du 10/03/2026 : … », avec un bouton qui ouvre la fiche.
  function tacheLigne(classe, texte) {
    var x = /^Ligne (\d+) : /.exec(texte);
    if (!x) return tache(classe, [texte]);
    var i = Number(x[1]) - 1, l = courant().lignes[i];
    if (!l) return tache(classe, [texte]);
    var b = el("button", { type: "button", class: "doux petit", text: classe === "err" ? "Corriger" : "Voir" });
    b.addEventListener("click", function () { allerA(courant().lignes.indexOf(l)); ouvrirFiche(null, l); });
    return tache(classe, [el("strong", { text: LIB_COURT[l.type] + " du " + dateFr(l.date) }), " : " + texte.slice(x[0].length)], b);
  }

  function afficherMessages(erreurs, avert) {
    var box = $("afaire"), liste = [], pied = null;
    box.textContent = "";
    var r = etat.calcul && etat.calcul.membres[etat.courant], cm = r ? r.cours_manquants : [];
    // Un cours manquant est demandé plus bas, une fois : le message du moteur sur la même vente ferait doublon.
    if (cm.length) erreurs = erreurs.filter(function (m) { return m.indexOf("cours manquant") < 0; });
    erreurs.slice(0, 5).forEach(function (m) { liste.push(tacheLigne("err", m)); });
    if (erreurs.length > 5) {
      var voir = el("button", { type: "button", class: "doux petit", text: "Les afficher" });
      voir.addEventListener("click", function () { filtre.type = "erreurs"; etat.page = 0; rendreTableau(); afficherPlusValues(); });
      liste.push(tache("err", ["… et " + (erreurs.length - 5) + " autres opérations à corriger."], voir));
    }
    if (cm.length) {
      var versCours = el("button", { type: "button", class: cm.length > 3 ? "doux petit" : "lien", text: cm.length > 3 ? "Compléter" : "Importer un fichier de cours" });
      versCours.addEventListener("click", function () { montrer("cours"); });
      if (cm.length > 3) {
        liste.push(tache("warn", [el("strong", { text: cm.length + " cours manquants" }), " pour calculer la valeur de votre portefeuille les jours de vente."], versCours));
      } else {
        // Peu de cours à fournir : on les demande ici même, sans changer d'écran.
        cm.forEach(function (c) {
          var s = saisieCours(c);
          liste.push(tache("warn", [el("strong", { text: "Prix d'un " + c.actif + " le " + dateFr(c.date) }), el("span", { class: "fine", text: " · en euros" })], el("span", { class: "tache-saisie" }, [s.champ, s.bouton])));
        });
        pied = el("p", { class: "afaire-pied" }, ["Ces prix servent à calculer la valeur de votre portefeuille le jour de la vente. Plutôt qu'un par un : ", versCours, "."]);
      }
    }
    avert.slice(0, 5).forEach(function (m) { liste.push(tacheLigne("warn", m)); });
    if (avert.length > 5) liste.push(tache("warn", ["… et " + (avert.length - 5) + " autres points à vérifier."]));
    box.hidden = liste.length === 0;
    if (box.hidden) return;
    box.appendChild(el("p", { class: "afaire-t", text: erreurs.length || cm.length ? "À compléter pour un résultat exact" : "À vérifier" }));
    liste.forEach(function (t) { box.appendChild(t); });
    if (pied) box.appendChild(pied);
    if (suiteCours && vueVisible("operations")) { suiteCours = false; var suivant = box.querySelector(".tache-saisie input"); if (suivant) suivant.focus(); }
  }

  function afficherPlusValues() {
    if (!etat.calcul) return;
    var r = etat.calcul.membres[etat.courant], c = etat.corresp[etat.courant];
    if (!r || !c) return; // membre ajouté à l'instant : le calcul qui l'inclut arrive
    var parLigne = {};
    r.cessions.forEach(function (x) { parLigne[c[x.operation - 1]] = x; });
    $("lignes").querySelectorAll("tr.op").forEach(function (tr) {
      var x = parLigne[Number(tr.dataset.i)], cell = tr.querySelector(".pv");
      cell.textContent = ""; cell.className = "n pv";
      if (x) {
        cell.textContent = (x.plus_value > 0 ? "+" : "") + euros.format(x.plus_value);
        cell.className = "n pv " + (x.plus_value >= 0 ? "pos" : "neg");
      }
    });
  }

  function texteReport(a) {
    if (a.exonere) return "Exonéré : total des cessions du foyer ≤ 305 €";
    if (a.resultat_net < 0) return "Moins-value : case 3BN, rien à payer";
    return "Plus-value : case 3AN";
  }

  // Le résultat, en tête des opérations : une carte par année de cessions (la plus récente en grand), qui mène à
  // la déclaration ; puis les avoirs.
  function afficherAnnees() {
    var box = $("bilan"), r = etat.calcul;
    box.textContent = "";
    if (!r) return;
    var rm = r.membres[etat.courant];
    if (!rm) return;
    var foyer = etat.membres.length > 1;
    var provisoire = r.membres.some(function (x) { return x.erreurs.length || x.cours_manquants.length; });
    r.foyer.slice().sort(function (a, b) { return b.annee - a.annee; }).forEach(function (f, k) {
      var a = rm.annees.filter(function (x) { return x.annee === f.annee; })[0];
      var kids = [
        el("span", { class: "t" }, ["Cessions " + f.annee + (foyer ? " · foyer" : ""), provisoire ? el("span", { class: "prov", text: "provisoire" }) : null]),
        el("span", { class: "gros " + (f.resultat_net >= 0 ? "pos" : "neg"), text: (f.resultat_net > 0 ? "+" : "") + euros.format(f.resultat_net) }),
        el("span", { class: "d", text: texteReport(f) })];
      if (foyer) kids.push(el("span", { class: "d", text: nomMembre(courant()) + " : " + (a ? euros.format(a.resultat_net) + " (" + a.nb_cessions + " cession" + (a.nb_cessions > 1 ? "s" : "") + ")" : "aucune cession") }));
      else if (k === 0 && a) kids.push(el("span", { class: "d", text: a.nb_cessions + " cession" + (a.nb_cessions > 1 ? "s" : "") + " · total " + euros.format(a.total_prix_cession) }));
      if (k === 0) kids.push(el("span", { class: "cta", text: "Voir la déclaration →" }));
      var carte = el("button", { type: "button", class: "bilan-carte" + (k === 0 ? " principal" : ""), "aria-label": "Déclaration des cessions " + f.annee }, kids);
      carte.addEventListener("click", function () {
        montrer("declaration");
        $("annee").value = String(f.annee);
        $("membre-decl").value = String(etat.courant);
        afficherDeclaration();
      });
      box.appendChild(carte);
    });
    var avoirs = Object.keys(rm.avoirs).filter(function (k) { return Math.abs(rm.avoirs[k]) > 1e-12; }).sort();
    if (avoirs.length) {
      box.appendChild(el("div", { class: "bilan-avoirs" }, [el("span", { text: "Avoirs :" })].concat(avoirs.map(function (k) {
        return el("span", { class: "a" + (rm.avoirs[k] < 0 ? " neg" : ""), text: qte.format(rm.avoirs[k]) + " " + k });
      }))));
    }
  }

  // ---------- Cours ----------
  function ajouterCours(nouveaux) {
    var m = {};
    etat.cours.forEach(function (c) { m[c.actif + "|" + c.date] = c; });
    nouveaux.forEach(function (c) { var a = c.actif.trim().toUpperCase(); m[a + "|" + c.date] = { actif: a, date: c.date, prix: c.prix }; });
    etat.cours = Object.keys(m).sort().map(function (k) { return m[k]; });
    marquer(); recalculer();
  }

  // Champ de saisie d'un cours manquant (dans « À compléter » et dans l'onglet Cours). Après validation, le curseur
  // passe au cours manquant suivant.
  var suiteCours = false;
  function saisieCours(c) {
    var inp = el("input", { class: "num", inputmode: "decimal", placeholder: "0,00", "aria-label": "Prix en euros d'un " + c.actif + " le " + dateFr(c.date) });
    var ok = el("button", { type: "button", class: "doux petit", text: "Valider" });
    var valider = function () {
      var p = num(inp.value);
      if (p == null || isNaN(p) || p <= 0) { inp.classList.add("faux"); inp.focus(); return; }
      suiteCours = true;
      ajouterCours([{ actif: c.actif, date: c.date, prix: p }]);
    };
    ok.addEventListener("click", valider);
    inp.addEventListener("input", function () { inp.classList.remove("faux"); });
    inp.addEventListener("keydown", function (e) { if (e.key === "Enter") { e.preventDefault(); valider(); } });
    return { champ: inp, bouton: ok };
  }

  // Pages « données historiques » de CoinGecko pour les cryptos courantes ; pour les autres, on renvoie à sa recherche.
  var COINGECKO = { BTC: "bitcoin", ETH: "ethereum", USDT: "tether", USDC: "usd-coin", BNB: "binancecoin", SOL: "solana", XRP: "ripple",
    ADA: "cardano", DOGE: "dogecoin", DOT: "polkadot", LINK: "chainlink", LTC: "litecoin", AVAX: "avalanche-2", MATIC: "matic-network",
    TRX: "tron", XLM: "stellar", ATOM: "cosmos", UNI: "uniswap", BCH: "bitcoin-cash", XMR: "monero", ETC: "ethereum-classic", DAI: "dai" };
  function lienCours(a) {
    return COINGECKO[a] ? "https://www.coingecko.com/en/coins/" + COINGECKO[a] + "/historical_data"
      : "https://www.coingecko.com/ (cherchez « " + a + " », puis « Données historiques »)";
  }
  function copier(texte, confirmation) {
    var fini = function () { annulable(confirmation, null); };
    if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(texte).then(fini, function () { annulable("Copie impossible : recopiez le lien à la main.", null); });
    else annulable("Copie impossible : recopiez le lien à la main.", null);
  }
  // Les cryptos pour lesquelles il manque des cours, avec un lien par crypto.
  function guideCours(liste) {
    var actifs = {};
    liste.forEach(function (c) { actifs[c.actif] = (actifs[c.actif] || 0) + 1; });
    var noms = Object.keys(actifs).sort(), box = $("cours-guide-liste");
    $("cours-guide").hidden = noms.length === 0;
    box.textContent = "";
    noms.forEach(function (a) {
      var url = lienCours(a), b = el("button", { type: "button", class: "doux petit", text: "Copier le lien" });
      b.addEventListener("click", function () { copier(url.split(" ")[0], "Lien copié : collez-le dans votre navigateur."); });
      box.appendChild(el("div", { class: "guide-ligne" }, [
        el("strong", { text: a }), el("span", { class: "fine", text: actifs[a] + " cours manquant" + (actifs[a] > 1 ? "s" : "") }),
        el("code", { text: url, title: url }), COINGECKO[a] ? b : null]));
    });
  }

  $("b-soutien").addEventListener("click", function () { copier("https://taiseux.toolguild.online/#soutenir", "Adresse copiée : collez-la dans votre navigateur."); });

  function afficherCours() {
    var r = etat.calcul, manquants = {}, liste = [], nbCessions = 0;
    if (r) r.membres.forEach(function (x) {
      nbCessions += x.cessions.length;
      x.cours_manquants.forEach(function (c) { var k = c.actif + "|" + c.date; if (!manquants[k]) { manquants[k] = 1; liste.push(c); } });
    });
    liste.sort(function (a, b) { return a.date < b.date ? -1 : a.date > b.date ? 1 : a.actif < b.actif ? -1 : 1; });
    $("manquants-zone").hidden = liste.length === 0;
    guideCours(liste);
    $("cours-ok").hidden = !(liste.length === 0 && nbCessions > 0);
    $("manquants-texte").textContent = "Le prix de chaque crypto que vous déteniez, le jour de chaque vente : il sert à calculer la valeur de votre portefeuille ce jour-là. Saisissez-le ici, ou importez un fichier.";
    var tb = $("manquants");
    tb.textContent = "";
    liste.slice(0, 300).forEach(function (c) {
      var s = saisieCours(c);
      tb.appendChild(el("tr", {}, [el("td", { text: c.actif }), el("td", { text: dateFr(c.date) }), el("td", { class: "n" }, [s.champ]), el("td", {}, [s.bouton])]));
    });
    if (suiteCours && vueVisible("cours")) { suiteCours = false; var suivant = tb.querySelector("input"); if (suivant) suivant.focus(); }
    // Résumé des cours enregistrés, par actif.
    var parActif = {};
    etat.cours.forEach(function (c) {
      var a = parActif[c.actif] || (parActif[c.actif] = { n: 0, debut: c.date, fin: c.date });
      a.n++; if (c.date < a.debut) a.debut = c.date; if (c.date > a.fin) a.fin = c.date;
    });
    var box = $("cours-resume");
    box.textContent = "";
    var actifs = Object.keys(parActif).sort();
    if (!actifs.length) { box.appendChild(el("p", { class: "fine", text: "Aucun cours enregistré." })); return; }
    var tbody = el("tbody");
    actifs.forEach(function (a) {
      var x = parActif[a];
      var suppr = el("button", { type: "button", class: "x", "aria-label": "Supprimer les cours de " + a, text: "×" });
      suppr.addEventListener("click", function () { etat.cours = etat.cours.filter(function (c) { return c.actif !== a; }); marquer(); recalculer(); });
      tbody.appendChild(el("tr", {}, [el("td", { text: a }), el("td", { class: "n", text: entier.format(x.n) }),
        el("td", { text: dateFr(x.debut) + " → " + dateFr(x.fin) }), el("td", {}, [suppr])]));
    });
    box.appendChild(el("div", { class: "table-scroll" }, [el("table", { class: "cours" }, [
      el("thead", {}, [el("tr", {}, [el("th", { text: "Crypto" }), el("th", { class: "n", text: "Jours" }), el("th", { text: "Période" }), el("th", {})])]), tbody])]));
  }

  function demanderTexte(titre, aide, libelle, valeur) {
    return new Promise(function (resolve) {
      var d = $("dlg-texte");
      $("texte-titre").textContent = titre; $("texte-aide").textContent = aide; $("texte-libelle").textContent = libelle;
      $("texte-valeur").value = valeur || "";
      var fini = function (v) { $("f-texte").onsubmit = null; d.close(); resolve(v); };
      $("texte-annuler").onclick = function () { fini(null); };
      d.oncancel = function (e) { e.preventDefault(); fini(null); };
      $("f-texte").onsubmit = function (e) { e.preventDefault(); fini($("texte-valeur").value.trim() || null); };
      d.showModal(); $("texte-valeur").focus(); $("texte-valeur").select();
    });
  }

  function messageCours(classe, texte) {
    var box = $("messages-cours");
    box.textContent = "";
    box.appendChild(el("p", { class: "msg " + classe, text: texte }));
  }

  $("b-import-cours").addEventListener("click", function () {
    dlg.open({ multiple: false, filters: [{ name: "Cours (CSV)", extensions: ["csv", "txt"] }] }).then(function (chemin) {
      if (!chemin) return;
      var lire = function (actif) {
        return invoke("importer_cours", { chemin: chemin, actif: actif }).then(function (l) {
          if (l.actif_requis) {
            return demanderTexte("De quelle crypto s'agit-il ?", "Ce fichier ne contient que des dates et des prix. Indiquez le code de la crypto, tel que vous l'écrivez dans vos opérations.", "Code de la crypto", l.actif_devine)
              .then(function (a) { if (a) return lire(a); });
          }
          if (!l.cours.length) { messageCours("err", l.erreurs[0] || "Aucun cours lu."); return; }
          ajouterCours(l.cours);
          var actifs = {};
          l.cours.forEach(function (c) { actifs[c.actif] = 1; });
          messageCours("ok", entier.format(l.cours.length) + " cours importés (" + Object.keys(actifs).join(", ") + ")" +
            (l.erreurs.length ? " ; " + l.erreurs.length + " ligne(s) illisible(s) ignorée(s)." : "."));
        });
      };
      return lire(null);
    }).catch(function (e) { dlg.message(String(e), { title: "Import impossible", kind: "error" }); });
  });

  // ---------- Import d'opérations (plateformes, portefeuilles, CSV de Taiseux) ----------
  function texteMontant(o) {
    if (o.type === "echange") return o.valeur_recue != null ? euros.format(o.valeur_recue) : "";
    if (o.type === "transfert") return "";
    return euros.format(o.montant);
  }
  function nomFichier(c) { return String(c).split(/[\\/]/).pop(); }

  // Résumé d'un fichier : période, répartition par type, cryptos.
  function resumeImport(ops) {
    if (!ops.length) return "Aucune opération lisible dans ce fichier.";
    var types = {}, actifs = {}, debut = ops[0].date, fin = ops[0].date;
    ops.forEach(function (o) {
      types[o.type] = (types[o.type] || 0) + 1;
      if (o.actif) actifs[o.actif] = 1;
      if (o.actif_recu) actifs[o.actif_recu] = 1;
      if (o.date < debut) debut = o.date; if (o.date > fin) fin = o.date;
    });
    var noms = { achat: ["achat", "achats"], cession: ["vente", "ventes"], echange: ["échange", "échanges"], transfert: ["transfert", "transferts"],
      revenu_gratuit: ["crypto reçue", "cryptos reçues"], soulte_recue: ["soulte", "soultes"] };
    var parts = TYPES.map(function (t) { return t[0]; }).filter(function (t) { return types[t]; }).map(function (t) { return types[t] + " " + noms[t][types[t] > 1 ? 1 : 0]; });
    var liste = Object.keys(actifs).sort();
    return "Du " + dateFr(debut) + " au " + dateFr(fin) + " · " + parts.join(", ") + " · " +
      (liste.length > 8 ? liste.slice(0, 8).join(", ") + " et " + (liste.length - 8) + " autres" : liste.join(", "));
  }

  // analyses : [{ nom, apercu } ou { nom, erreur }]. Rend { membre, lots: [{ analyse, operations }] } ou null.
  function apercuImport(analyses) {
    return new Promise(function (resolve) {
      var d = $("dlg-import"), choisi = 0;
      var lisibles = analyses.filter(function (a) { return a.apercu; });
      $("import-titre").textContent = analyses.length > 1 ? "Import de " + analyses.length + " fichiers" : "Import de " + analyses[0].nom;
      $("import-beta").hidden = !lisibles.some(function (a) { return a.apercu.plateforme !== "Taiseux"; });
      var sel = $("import-membre");
      sel.textContent = "";
      etat.membres.forEach(function (m, k) { var o = el("option", { value: String(k), text: nomMembre(m) }); if (k === etat.courant) o.selected = true; sel.appendChild(o); });
      sel.parentNode.hidden = etat.membres.length < 2;

      // Doublons : opérations déjà dans le dossier, ou déjà vues dans un fichier précédent du même import.
      function repartir() {
        var deja = {};
        operationsMembre(etat.membres[Number(sel.value)]).ops.forEach(function (o) { deja[cle(o)] = 1; });
        analyses.forEach(function (a) {
          if (!a.apercu) return;
          a.neuves = a.apercu.operations.filter(function (o) { var k = cle(o); if (deja[k]) return false; deja[k] = 1; return true; });
        });
      }
      function detail() {
        var a = analyses[choisi];
        $("import-detail").hidden = !a || !a.apercu;
        if (!a || !a.apercu) return;
        $("import-resume").textContent = resumeImport(a.apercu.operations);
        var tb = $("import-ops");
        tb.textContent = "";
        a.apercu.operations.slice(0, 60).forEach(function (o) {
          tb.appendChild(el("tr", {}, [el("td", { text: dateFr(o.date) }), el("td", { text: LIB_TYPE[o.type] || o.type }), el("td", { text: o.actif || "" }),
            el("td", { class: "n", text: o.quantite == null ? "" : qte.format(o.quantite) }), el("td", { class: "n", text: texteMontant(o) }),
            el("td", { text: o.actif_recu ? qte.format(o.quantite_recue || 0) + " " + o.actif_recu : "" })]));
        });
        if (a.apercu.operations.length > 60) tb.appendChild(el("tr", {}, [el("td", { colspan: "6", class: "fine", text: "… et " + (a.apercu.operations.length - 60) + " autres." })]));
        var av = $("import-averifier");
        av.textContent = "";
        if (a.apercu.a_verifier.length || a.apercu.erreurs.length) {
          var det = el("details", {}, [el("summary", { text: "Voir les " + (a.apercu.a_verifier.length + a.apercu.erreurs.length) + " lignes à vérifier de ce fichier" })]);
          a.apercu.erreurs.slice(0, 20).forEach(function (m) { det.appendChild(el("p", { class: "msg err", text: m })); });
          a.apercu.a_verifier.slice(0, 20).forEach(function (x) { det.appendChild(el("div", { class: "av" }, [el("strong", { text: "Ligne " + x.ligne + " : " + x.motif }), el("pre", { text: x.contenu })])); });
          av.appendChild(det);
        }
      }
      function tableau() {
        repartir();
        var tb = $("import-fichiers"), total = 0, averif = 0;
        tb.textContent = "";
        analyses.forEach(function (a, k) {
          var tr;
          if (a.apercu) {
            var dbl = a.apercu.operations.length - a.neuves.length;
            total += a.neuves.length; averif += a.apercu.a_verifier.length;
            tr = el("tr", { class: "fichier" + (k === choisi ? " choisi" : ""), tabindex: "0" }, [
              el("td", { text: a.nom }),
              el("td", { text: a.apercu.plateforme === "Taiseux" ? "CSV de Taiseux" : a.apercu.plateforme + " · " + a.apercu.format }),
              el("td", { class: "n" }, [String(a.neuves.length), dbl ? el("small", { text: " (+" + dbl + " déjà présentes)" }) : null]),
              el("td", { class: "n", text: String(a.apercu.a_verifier.length + a.apercu.erreurs.length) }),
              el("td", { class: "n", text: String(a.apercu.ignorees) })]);
            var voir = function () { choisi = k; tableau(); detail(); };
            tr.addEventListener("click", voir);
            tr.addEventListener("keydown", function (e) { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); voir(); } });
          } else {
            tr = el("tr", { class: "fichier refuse" }, [el("td", { text: a.nom }), el("td", { colspan: "4", text: String(a.erreur).split(". ")[0].replace(/\.$/, "") + ".", title: a.erreur })]);
          }
          tb.appendChild(tr);
        });
        var ok = $("import-ok");
        ok.disabled = total === 0 && averif === 0;
        ok.textContent = total ? "Ajouter " + entier.format(total) + " opération" + (total > 1 ? "s" : "") : averif ? "Garder les lignes à vérifier" : "Rien à ajouter";
      }
      choisi = Math.max(0, analyses.findIndex(function (a) { return a.apercu; }));
      sel.onchange = tableau;
      tableau(); detail();
      var fini = function (v) { $("f-import").onsubmit = null; d.close(); resolve(v); };
      $("import-annuler").onclick = function () { fini(null); };
      d.oncancel = function (e) { e.preventDefault(); fini(null); };
      $("f-import").onsubmit = function (e) {
        e.preventDefault();
        repartir();
        fini({ membre: Number(sel.value), lots: lisibles.map(function (a) { return { analyse: a, operations: a.neuves }; }) });
      };
      d.showModal();
    });
  }

  var numeroLot = 0;
  function importerChemins(chemins) {
    chemins = (Array.isArray(chemins) ? chemins : [chemins]).filter(Boolean);
    if (!chemins.length) return Promise.resolve();
    return Promise.all(chemins.map(function (c) {
      return invoke("importer_fichier", { chemin: c }).then(function (a) { return { nom: nomFichier(c), apercu: a }; })
        .catch(function (e) { return { nom: nomFichier(c), erreur: String(e) }; });
    })).then(apercuImport).then(function (choix) {
      if (!choix) return;
      var m = etat.membres[choix.membre], lot = ++numeroLot, nb = 0, sources = {}, avant = m.a_verifier.length;
      m.lignes.forEach(function (l) { l._neuf = false; });
      choix.lots.forEach(function (x) {
        x.operations.forEach(function (o) { var l = nouvelleLigne(o.type, o); l._lot = lot; l._neuf = true; m.lignes.push(l); nb++; });
        // Une ligne à vérifier déjà gardée (même fichier importé deux fois) n'est pas reprise.
        x.analyse.apercu.a_verifier.forEach(function (v) {
          if (!m.a_verifier.some(function (w) { return w.contenu === v.contenu && w.motif === v.motif; })) m.a_verifier.push(v);
        });
        if (x.operations.length) sources[x.analyse.apercu.plateforme === "Taiseux" ? "votre fichier" : x.analyse.apercu.plateforme] = 1;
      });
      trier(m);
      effacerFiltre();
      etat.courant = choix.membre; etat.page = 0;
      marquer(); rendreTout(); rendreSuggestions();
      return recalculer().then(function () {
        var gardees = m.a_verifier.length - avant;
        annulable((nb ? entier.format(nb) + " opération" + (nb > 1 ? "s ajoutées" : " ajoutée") + " depuis " + Object.keys(sources).join(", ") : "Aucune opération ajoutée") +
          (gardees ? " ; " + gardees + " ligne" + (gardees > 1 ? "s" : "") + " à vérifier" : "") + ".", function () {
          m.lignes = m.lignes.filter(function (l) { return l._lot !== lot; });
          m.a_verifier.length = avant;
          marquer(); rendreTout(); recalculer();
        });
      });
    }).catch(function (e) { dlg.message(String(e), { title: "Import impossible", kind: "error" }); });
  }

  $("b-import").addEventListener("click", function () {
    dlg.open({ multiple: true, filters: [{ name: "Exports (CSV)", extensions: ["csv", "txt", "tsv"] }] }).then(importerChemins)
      .catch(function (e) { dlg.message(String(e), { title: "Import impossible", kind: "error" }); });
  });

  // Glisser-déposer des fichiers sur la fenêtre (les chemins viennent du système, par l'événement de Tauri).
  try {
    var vueWeb = T.webview && T.webview.getCurrentWebview && T.webview.getCurrentWebview();
    if (vueWeb && vueWeb.onDragDropEvent) {
      vueWeb.onDragDropEvent(function (ev) {
        var p = ev.payload || {};
        if (p.type === "enter" || p.type === "over") $("depot").hidden = false;
        else if (p.type === "leave") $("depot").hidden = true;
        else if (p.type === "drop") {
          $("depot").hidden = true;
          if (document.querySelector("dialog[open]")) return;
          var fichiers = (p.paths || []).filter(function (c) { return !/\.taiseux$/i.test(c); });
          if (fichiers.length) { montrer("operations"); importerChemins(fichiers); }
        }
      });
    }
  } catch (e) { /* glisser-déposer indisponible : l'import par le bouton reste là */ }

  // ---------- Déclaration 2086 ----------
  var LIGNES = [
    [211, "Date de la cession", "date"],
    [212, "Valeur globale du portefeuille", "valeur_portefeuille"],
    [213, "Prix de cession", "prix_cession"],
    [214, "Frais de cession", "frais"],
    [215, "Prix de cession net des frais", "prix_net_frais"],
    [216, "Soulte reçue ou versée lors de la cession", "soulte_cession"],
    [217, "Prix de cession net des soultes", "prix_net_soultes"],
    [218, "Prix de cession net des frais et soultes", "prix_net_frais_soultes"],
    [220, "Prix total d'acquisition", "prix_total_acquisition"],
    [221, "Fractions de capital initial", "fractions_capital_initial"],
    [222, "Soultes reçues lors d'échanges antérieurs", "soultes_anterieures"],
    [223, "Prix total d'acquisition net", "pta_net"],
    [null, "Plus-value ou moins-value", "plus_value"]
  ];
  function numLigne(n, m) { return n == null ? "" : String(n + ROLES[m.role].decalage); }
  function valeurLigne(c, cle_) {
    if (cle_ === "date") return dateFr(c.date);
    var n = euroEntier(c[cle_]);
    return (cle_ === "plus_value" && n > 0 ? "+" : "") + entier.format(n) + (cle_ === "valeur_portefeuille" && c.valeur_calculee ? " *" : "");
  }

  function remplirSelect(sel, valeurs, libelles, defaut) {
    var actuel = sel.value;
    sel.textContent = "";
    valeurs.forEach(function (v, k) { sel.appendChild(el("option", { value: String(v), text: libelles[k] })); });
    var vs = valeurs.map(String);
    sel.value = vs.indexOf(actuel) >= 0 ? actuel : String(defaut);
    return sel.value;
  }
  $("annee").addEventListener("change", afficherDeclaration);
  $("membre-decl").addEventListener("change", afficherDeclaration);

  function pages(cessions) { var p = []; for (var i = 0; i < cessions.length; i += 5) p.push(cessions.slice(i, i + 5)); return p; }

  function tableauPage(cs, n, m) {
    var head = el("tr", {}, [el("th", { text: "Ligne" }), el("th", { text: "" })]);
    cs.forEach(function (c, k) { head.appendChild(el("th", { class: "n", text: "Cession " + (n * 5 + k + 1) })); });
    var tb = el("tbody");
    LIGNES.forEach(function (L) {
      var tr = el("tr", { class: L[2] === "plus_value" ? "total" : "" }, [el("td", { class: "l", text: numLigne(L[0], m) }), el("td", { class: "lib", text: L[1] })]);
      cs.forEach(function (c) { tr.appendChild(el("td", { class: "n", text: valeurLigne(c, L[2]) })); });
      tb.appendChild(tr);
    });
    return el("table", { class: "f2086" }, [el("thead", {}, [head]), tb]);
  }

  function selection() {
    var r = etat.calcul;
    if (!r || !r.foyer.length) return null;
    var k = Number(remplirSelect($("membre-decl"), etat.membres.map(function (_, i) { return i; }), etat.membres.map(nomMembre), etat.courant));
    $("membre-choix-zone").hidden = etat.membres.length < 2;
    var annees = r.foyer.map(function (f) { return f.annee; });
    var annee = Number(remplirSelect($("annee"), annees, annees.map(String), annees[annees.length - 1]));
    return { k: k, m: etat.membres[k], annee: annee, f: r.foyer.filter(function (x) { return x.annee === annee; })[0],
      a: r.membres[k].annees.filter(function (x) { return x.annee === annee; })[0],
      cs: r.membres[k].cessions.filter(function (c) { return c.annee === annee; }) };
  }

  function afficherDeclaration() {
    var s = selection();
    $("decl-vide").hidden = !!s;
    $("decl").hidden = !s;
    if (!s) return;
    var cartes = $("cartes"), total = numLigne(224, s.m), seul = etat.membres.length === 1;
    // Un résultat incomplet se dit ici aussi : c'est l'écran qu'on recopie.
    var incomplet = etat.calcul.membres.some(function (x) { return x.erreurs.length || x.cours_manquants.length; });
    var alerte = $("decl-alerte");
    alerte.textContent = "";
    alerte.hidden = !incomplet;
    if (incomplet) {
      var manque = el("button", { type: "button", class: "lien", text: "Voir ce qui manque" });
      manque.addEventListener("click", function () { montrer("operations"); });
      alerte.appendChild(el("p", { class: "msg warn" }, ["Résultat provisoire : des opérations sont à corriger ou des cours manquent. ", manque]));
    }
    cartes.textContent = "";
    var report = el("div", { class: "carte hero" }, [el("p", { class: "t", text: "À reporter sur la déclaration 2042 C" + (seul ? "" : " · foyer") })]);
    if (s.f.exonere) {
      report.appendChild(el("p", { class: "gros", text: "Rien à reporter" }));
      report.appendChild(el("p", { text: "Total des cessions ≤ 305 € : exonéré. Joignez tout de même le formulaire 2086." }));
    } else {
      report.appendChild(el("p", { class: "gros" }, [el("span", { class: "case", text: s.f.case_2042c }), entier.format(Math.abs(euroEntier(s.f.resultat_net))) + " €"]));
      report.appendChild(el("p", { text: s.f.case_2042c === "3AN" ? "Plus-value imposable (12,8 % + prélèvements sociaux, ou barème si vous cochez 3CN)." : "Moins-value : ni imposable, ni reportable sur les années suivantes." }));
    }
    cartes.appendChild(report);
    cartes.appendChild(el("div", { class: "carte" }, [
      el("p", { class: "t", text: "Ligne " + total + (seul ? "" : " · " + nomMembre(s.m)) }),
      el("p", { class: "gros " + (s.a && s.a.resultat_net < 0 ? "neg" : "pos"), text: s.a ? (s.a.resultat_net > 0 ? "+" : "") + entier.format(euroEntier(s.a.resultat_net)) + " €" : "—" }),
      el("p", { text: s.a ? "Plus ou moins-value globale : " + s.a.nb_cessions + " cession" + (s.a.nb_cessions > 1 ? "s" : "") + "." : "Aucune cession cette année." })
    ]));
    cartes.appendChild(el("div", { class: "carte" }, [
      el("p", { class: "t", text: "Lignes 51 et 52" + (seul ? "" : " · foyer") }),
      el("p", { class: "gros", text: entier.format(euroEntier(s.f.total_prix_cession)) + " €" }),
      el("p", { text: "Ligne 51, total des prix : " + (s.f.exonere ? "≤ 305 €, exonéré." : "> 305 €, imposable.") }),
      el("p", { text: "Ligne 52, résultat : " + (s.f.resultat_net > 0 ? "+" : "") + entier.format(euroEntier(s.f.resultat_net)) + " €" })
    ]));
    var f = $("formulaire");
    f.textContent = "";
    // Les pages du formulaire s'affichent par lots de 12 (60 cessions) : une année très active en compte des centaines.
    var tout = pages(s.cs), affichees = 0;
    var suite = el("button", { type: "button", class: "ghost", text: "" });
    function lot() {
      var fin = Math.min(tout.length, affichees + 12);
      for (var n = affichees; n < fin; n++) {
        f.insertBefore(el("div", { class: "page2086" }, [
          el("h3", { text: nomMembre(s.m) + " · page " + (n + 1) + " sur " + tout.length + " · cessions " + (n * 5 + 1) + " à " + (n * 5 + tout[n].length) }),
          el("div", { class: "table-scroll" }, [tableauPage(tout[n], n, s.m)])
        ]), suite);
      }
      affichees = fin;
      suite.hidden = affichees >= tout.length;
      suite.textContent = "Afficher les pages suivantes (" + (tout.length - affichees) + " restantes)";
    }
    suite.addEventListener("click", lot);
    f.appendChild(suite);
    lot();
    $("b-export-2086").disabled = $("b-fiche").disabled = s.cs.length === 0;
    // Les précisions n'apparaissent que si elles concernent ce qui est affiché.
    var notes = [];
    if (s.cs.length > 5) notes.push("Le formulaire compte cinq colonnes de cessions par page : au-delà, ajoutez des pages.");
    notes.push("Montants arrondis à l'euro le plus proche, comme sur la déclaration ; le détail au centime est dans l'export CSV.");
    if (s.cs.some(function (c) { return c.valeur_calculee; })) notes.push("Un astérisque signale une valeur globale calculée à partir des cours.");
    $("decl-notes").textContent = notes.join(" ");
    rappelComptes();
  }

  // Rappel des comptes à l'étranger, sur l'écran de la déclaration, seulement s'il y a lieu.
  function rappelComptes() {
    var box = $("decl-comptes"), vues = {};
    box.textContent = "";
    etat.membres.forEach(function (m) { m.lignes.forEach(function (l) {
      var p = l.plateforme.trim();
      if (p && nature(p)[0] === "etranger") vues[p.toLowerCase()] = 1;
    }); });
    var n = etat.comptes.length, v = Object.keys(vues).length;
    box.hidden = !n && !v;
    if (box.hidden) return;
    var lien = el("button", { type: "button", class: "lien", text: n ? "Voir la liste" : "Préparer la liste" });
    lien.addEventListener("click", function () { montrer("comptes"); });
    box.appendChild(el("p", { class: "msg " + (n ? "ok" : "warn") }, [n
      ? n + " compte" + (n > 1 ? "s" : "") + " à l'étranger dans votre liste : cochez la case 8UU et joignez un 3916-bis par compte. "
      : "Vos opérations citent " + v + " plateforme" + (v > 1 ? "s" : "") + " a priori établie" + (v > 1 ? "s" : "") + " à l'étranger : ces comptes se déclarent aussi (3916-bis). ", lien]));
  }

  // ---------- Fiches imprimables (fichiers HTML autonomes, en clair) ----------
  function echapper(s) { return String(s).replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); }
  var STYLE_FICHE = 'body{font:12px/1.4 -apple-system,"Segoe UI",Arial,sans-serif;color:#111;margin:24px}h1{font-size:18px}h2{font-size:13px;margin:18px 0 6px}' +
    'table{border-collapse:collapse;width:100%;margin-bottom:8px}td,th{border:1px solid #bbb;padding:4px 6px;text-align:right}td.l,td.lib,th.lib{text-align:left}' +
    'td.l{font-family:Menlo,monospace;color:#555;width:36px}tr.t td{font-weight:700;background:#eef4f1}.r{border:2px solid #1f5f4a;padding:10px 12px;margin:12px 0}' +
    '.f{color:#555;font-size:11px}@media print{.p{page-break-inside:avoid}}';
  function fiche(s) {
    var titre = "Formulaire 2086 — cessions " + s.annee + (etat.membres.length > 1 ? " — " + nomMembre(s.m) : "");
    var h = '<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>' + echapper(titre) + '</title><style>' + STYLE_FICHE + '</style></head><body>' +
      '<h1>' + echapper(titre) + '</h1><p class="f">Préparé avec Taiseux le ' + new Date().toLocaleDateString("fr-FR") +
      '. Montants arrondis à l\'euro ; * = valeur globale calculée à partir des cours. Document en clair : à conserver en lieu sûr.</p>';
    pages(s.cs).forEach(function (p, n) {
      h += '<div class="p"><h2>Page ' + (n + 1) + '</h2><table><tr><th>Ligne</th><th class="lib"></th>';
      p.forEach(function (c, k) { h += "<th>Cession " + (n * 5 + k + 1) + "</th>"; });
      h += "</tr>";
      LIGNES.forEach(function (L) {
        h += '<tr class="' + (L[2] === "plus_value" ? "t" : "") + '"><td class="l">' + numLigne(L[0], s.m) + '</td><td class="lib">' + echapper(L[1]) + "</td>";
        p.forEach(function (c) { h += "<td>" + echapper(valeurLigne(c, L[2])) + "</td>"; });
        h += "</tr>";
      });
      h += "</table></div>";
    });
    h += '<div class="r"><p><strong>Ligne ' + numLigne(224, s.m) + ' :</strong> ' + entier.format(euroEntier(s.a ? s.a.resultat_net : 0)) + ' €</p>' +
      '<p><strong>Foyer · ligne 51 :</strong> ' + entier.format(euroEntier(s.f.total_prix_cession)) + ' € · <strong>ligne 52 :</strong> ' + entier.format(euroEntier(s.f.resultat_net)) + ' €</p>' +
      '<p><strong>2042 C :</strong> ' + (s.f.exonere ? "rien à reporter (total des cessions du foyer ≤ 305 €), joindre le 2086." : "case " + s.f.case_2042c + " : " + entier.format(Math.abs(euroEntier(s.f.resultat_net))) + " €") +
      '</p></div><p class="f">Taiseux applique l\'article 150 VH bis du CGI aux chiffres saisis ; ce n\'est pas un conseil fiscal.</p></body></html>';
    return h;
  }

  // ---------- Comptes à l'étranger (3916-bis) ----------
  var PORTEFEUILLES = ["ledger live", "ledger", "trezor suite", "trezor", "electrum", "exodus", "sparrow", "coinomi"];
  var FRANCAISES = ["bitstack", "paymium", "coinhouse", "stackinsat", "deskoin"];
  function nature(p) {
    var x = p.toLowerCase();
    if (PORTEFEUILLES.some(function (w) { return x.indexOf(w) === 0; })) return ["perso", "Portefeuille personnel : rien à déclarer."];
    if (FRANCAISES.some(function (w) { return x.indexOf(w) === 0; })) return ["fr", "Plateforme établie en France : pas de 3916-bis."];
    return ["etranger", "Plateforme a priori établie à l'étranger : à déclarer, après avoir vérifié l'entité qui tient votre compte."];
  }

  function afficherComptes() {
    var vues = {};
    etat.membres.forEach(function (m, k) {
      m.lignes.forEach(function (l) {
        var p = l.plateforme.trim();
        if (!p) return;
        var x = vues[k + "|" + p.toLowerCase()] || (vues[k + "|" + p.toLowerCase()] = { k: k, p: p, n: 0, debut: l.date, fin: l.date });
        x.n++; if (l.date < x.debut) x.debut = l.date; if (l.date > x.fin) x.fin = l.date;
      });
    });
    var box = $("plateformes-vues");
    box.textContent = "";
    var liste = Object.keys(vues).sort().map(function (k) { return vues[k]; });
    if (!liste.length) box.appendChild(el("p", { class: "fine", text: "Aucune plateforme renseignée dans vos opérations (colonne « Plateforme »)." }));
    liste.forEach(function (v) {
      var n = nature(v.p);
      var deja = etat.comptes.some(function (c) { return c.titulaire === v.k && c.plateforme.toLowerCase() === v.p.toLowerCase(); });
      var kids = [el("strong", { text: v.p }), el("span", { class: "fine", text: (etat.membres.length > 1 ? nomMembre(etat.membres[v.k]) + " · " : "") + v.n + " opération(s), " + dateFr(v.debut) + " → " + dateFr(v.fin) }),
        el("span", { class: "nature " + n[0], text: n[1] })];
      if (n[0] === "etranger") {
        var b = el("button", { type: "button", class: "ghost petit", text: deja ? "Déjà dans la liste" : "Ajouter à la liste" });
        b.disabled = deja;
        b.addEventListener("click", function () {
          etat.comptes.push({ titulaire: v.k, plateforme: v.p, adresse: "", site: "", identifiant: "", ouverture: v.debut, cloture: "", note: "" });
          marquer(); afficherComptes();
        });
        kids.push(b);
      }
      box.appendChild(el("div", { class: "plateforme-vue" }, kids));
    });
    var cb = $("comptes");
    cb.textContent = "";
    if (!etat.comptes.length) cb.appendChild(el("p", { class: "fine", text: "Aucun compte dans la liste." }));
    $("comptes-aide").hidden = !etat.comptes.length;
    $("b-fiche-comptes").hidden = !etat.comptes.length;
    etat.comptes.forEach(function (c, i) {
      var g = el("div", { class: "champs compte" });
      function f(lib, cle_, attrs) {
        var inp = el("input", attrs || {});
        inp.value = c[cle_] || "";
        inp.addEventListener("input", function () { c[cle_] = inp.value; marquer(); });
        g.appendChild(el("label", {}, [el("span", { text: lib }), inp]));
      }
      if (etat.membres.length > 1) {
        var sel = el("select", { "aria-label": "Titulaire" });
        etat.membres.forEach(function (m, k) { var o = el("option", { value: String(k), text: nomMembre(m) }); if (k === c.titulaire) o.selected = true; sel.appendChild(o); });
        sel.addEventListener("change", function () { c.titulaire = Number(sel.value); marquer(); });
        g.appendChild(el("label", {}, [el("span", { text: "Titulaire" }), sel]));
      }
      f("Organisme qui tient le compte", "plateforme");
      f("Adresse de l'organisme", "adresse", { placeholder: "dans ses conditions d'utilisation" });
      f("Site", "site", { placeholder: "https://…" });
      f("Identifiant du compte", "identifiant");
      f("Ouverture", "ouverture", { type: "date" });
      f("Clôture", "cloture", { type: "date" });
      f("Note", "note");
      var x = el("button", { type: "button", class: "x", "aria-label": "Retirer ce compte", text: "×" });
      x.addEventListener("click", function () { etat.comptes.splice(i, 1); marquer(); afficherComptes(); });
      cb.appendChild(el("div", { class: "carte compte-carte" }, [x, g]));
    });
  }
  $("b-compte").addEventListener("click", function () {
    etat.comptes.push({ titulaire: etat.courant, plateforme: "", adresse: "", site: "", identifiant: "", ouverture: "", cloture: "", note: "" });
    marquer(); afficherComptes();
  });

  function ficheComptes() {
    var h = '<!doctype html><html lang="fr"><head><meta charset="utf-8"><title>Comptes d\'actifs numériques à l\'étranger</title><style>' + STYLE_FICHE +
      'td{text-align:left}</style></head><body><h1>Comptes d\'actifs numériques à l\'étranger (3916-bis)</h1><p class="f">Préparé avec Taiseux le ' +
      new Date().toLocaleDateString("fr-FR") + '. Un formulaire 3916-bis par compte ; cochez la case 8UU de la déclaration de revenus. Document en clair.</p><table><tr>' +
      ["Titulaire", "Organisme", "Adresse", "Site", "Identifiant", "Ouverture", "Clôture", "Note"].map(function (t) { return "<th>" + t + "</th>"; }).join("") + "</tr>";
    etat.comptes.forEach(function (c) {
      h += "<tr>" + [nomMembre(etat.membres[c.titulaire] || etat.membres[0]), c.plateforme, c.adresse, c.site, c.identifiant, c.ouverture ? dateFr(c.ouverture) : "", c.cloture ? dateFr(c.cloture) : "", c.note]
        .map(function (v) { return "<td>" + echapper(v || "") + "</td>"; }).join("") + "</tr>";
    });
    return h + "</table></body></html>";
  }

  // ---------- Fichiers ----------
  function enregistrerTexte(nom, ext, libelle, contenu) {
    return dlg.save({ defaultPath: nom, filters: [{ name: libelle, extensions: [ext] }] }).then(function (chemin) {
      if (!chemin) return;
      return invoke("enregistrer_texte", { chemin: chemin, contenu: contenu }).then(function () {
        dlg.message("Enregistré : " + chemin.split(/[\\/]/).pop() + ". Ce fichier est en clair : rangez-le ou supprimez-le après usage.", { title: "Fichier enregistré", kind: "info" });
      });
    }).catch(function (e) { dlg.message(String(e), { title: "Enregistrement impossible", kind: "error" }); });
  }

  $("b-export-ops").addEventListener("click", function () {
    invoke("csv_operations", { operations: operationsMembre(courant()).ops }).then(function (t) {
      return enregistrerTexte("operations" + (etat.membres.length > 1 ? "-" + courant().role : "") + ".csv", "csv", "CSV", t);
    });
  });
  $("b-export-2086").addEventListener("click", function () {
    var s = selection();
    if (s) invoke("csv_2086", { operations: operationsMembre(s.m).ops, cours: coursUtiles([operationsMembre(s.m)]) }).then(function (t) { return enregistrerTexte("taiseux-2086.csv", "csv", "CSV", t); });
  });
  $("b-fiche").addEventListener("click", function () {
    var s = selection();
    if (s) enregistrerTexte("taiseux-2086-" + s.annee + (etat.membres.length > 1 ? "-" + s.m.role : "") + ".html", "html", "Page HTML", fiche(s));
  });
  $("b-fiche-comptes").addEventListener("click", function () {
    if (etat.comptes.length) enregistrerTexte("taiseux-3916-bis.html", "html", "Page HTML", ficheComptes());
  });

  // Exemple : le § 110 du BOFiP, saisi par actif (BTC), cours fournis pour que la valeur globale soit calculée.
  $("b-exemple").addEventListener("click", function () {
    var m = courant();
    [{ date: "2026-01-10", type: "achat", montant: 1000, actif: "BTC", quantite: 0.02, plateforme: "Kraken", note: "Exemple d'après le BOFiP § 110" },
     { date: "2026-03-10", type: "cession", montant: 450, actif: "BTC", quantite: 0.0075, plateforme: "Kraken" },
     { date: "2026-05-02", type: "echange", actif: "BTC", quantite: 0.005, actif_recu: "ETH", quantite_recue: 0.12, plateforme: "Kraken", note: "Échange sans soulte : pas imposable" },
     { date: "2026-08-10", type: "cession", montant: 650, actif: "BTC", quantite: 0.0075, plateforme: "Kraken" }]
      .forEach(function (o) { m.lignes.push(nouvelleLigne(o.type, o)); });
    ajouterCours([{ actif: "BTC", date: "2026-03-10", prix: 60000 }, { actif: "BTC", date: "2026-08-10", prix: 86666.67 }, { actif: "ETH", date: "2026-08-10", prix: 5416.67 }]);
    trier(m);
    marquer(); rendreTout(); rendreSuggestions(); recalculer();
  });

  // ---------- Coffre ----------
  var dPhrase = $("dlg-phrase");
  function basculerPhrase() {
    var clair = $("phrase-voir").checked;
    $("phrase").classList.toggle("secret", !clair); $("phrase2").classList.toggle("secret", !clair);
  }
  function jauge() {
    var n = Array.from($("phrase").value).length, j = $("phrase-jauge");
    j.textContent = n === 0 ? "" : n < 10 ? n + " caractère" + (n > 1 ? "s" : "") + " sur 10 au minimum" : n + " caractères";
    j.classList.toggle("bon", n >= 10);
  }
  $("phrase-voir").addEventListener("change", basculerPhrase);
  $("phrase").addEventListener("input", jauge);
  function demanderPhrase(creation) {
    return new Promise(function (resolve) {
      $("phrase-titre").textContent = creation ? "Choisir une phrase de passe" : "Déverrouiller le coffre";
      $("phrase-texte").textContent = creation
        ? "Au moins 10 caractères ; une suite de mots est plus sûre et plus facile à retenir. Une phrase oubliée ne se récupère pas : personne ne peut ouvrir le coffre sans elle."
        : "Saisissez la phrase de passe de ce coffre.";
      $("phrase2-zone").hidden = !creation;
      $("phrase").value = ""; $("phrase2").value = ""; $("phrase-erreur").textContent = "";
      $("phrase-voir").checked = false; basculerPhrase();
      $("phrase-jauge").hidden = !creation; jauge();
      $("phrase-ok").disabled = false;
      var fini = function (v) { $("f-phrase").onsubmit = null; $("phrase-annuler").onclick = null; dPhrase.close(); $("phrase").value = ""; $("phrase2").value = ""; resolve(v); };
      $("phrase-annuler").onclick = function () { fini(null); };
      dPhrase.oncancel = function (e) { e.preventDefault(); fini(null); };
      $("f-phrase").onsubmit = function (e) {
        e.preventDefault();
        var p = $("phrase").value;
        if (creation && Array.from(p).length < 10) { $("phrase-erreur").textContent = "Au moins 10 caractères."; return; }
        if (creation && p !== $("phrase2").value) { $("phrase-erreur").textContent = "Les deux saisies diffèrent."; return; }
        if (!p) return;
        fini(p);
      };
      dPhrase.showModal();
      $("phrase").focus();
    });
  }

  function dossier() {
    return {
      version: 2, operations: [],
      membres: etat.membres.map(function (m) { return { role: m.role, nom: m.nom, operations: operationsMembre(m).ops, a_verifier: m.a_verifier }; }),
      cours: etat.cours, comptes: etat.comptes
    };
  }

  function sceller(chemin, phrase) {
    annulable("Chiffrement…", null);
    return invoke("coffre_sceller", { chemin: chemin, phrase: phrase, dossier: dossier() }).then(function () {
      etat.chemin = chemin; etat.phrase = phrase; etat.modifie = false; afficherEtat();
      annulable("Coffre enregistré : " + nomFichier(chemin) + ".", null);
    }).catch(function (e) { $("toast").hidden = true; dlg.message(String(e), { title: "Enregistrement impossible", kind: "error" }); });
  }

  function enregistrer(sous) {
    if (etat.chemin && etat.phrase && !sous) return sceller(etat.chemin, etat.phrase);
    return dlg.save({ defaultPath: "mes-cryptos.taiseux", filters: [{ name: "Coffre Taiseux", extensions: ["taiseux"] }] }).then(function (chemin) {
      if (!chemin) return;
      return demanderPhrase(true).then(function (p) { if (p) return sceller(chemin, p); });
    });
  }
  $("b-enregistrer").addEventListener("click", function () { enregistrer(false); });

  function confirmerPerte() {
    if (!etat.modifie || (!etat.chemin && !toutesLignes())) return Promise.resolve(true);
    return dlg.ask("Les modifications non enregistrées seront perdues.", { title: "Continuer ?", kind: "warning", okLabel: "Continuer", cancelLabel: "Annuler" });
  }

  function charger(d, chemin, phrase) {
    var membres = (d && d.membres && d.membres.length) ? d.membres : [{ role: "declarant1", nom: "", operations: [], a_verifier: [] }];
    etat.membres = membres.map(function (m) {
      return { role: ROLES[m.role] ? m.role : "declarant1", nom: m.nom || "", a_verifier: m.a_verifier || [],
        lignes: (m.operations || []).map(function (o) { return nouvelleLigne(o.type, o); }) };
    });
    etat.cours = (d && d.cours) || [];
    etat.comptes = (d && d.comptes) || [];
    etat.membres.forEach(trier);
    effacerFiltre(); erreursLignes = {};
    etat.courant = 0; etat.page = 0;
    etat.chemin = chemin; etat.phrase = phrase; etat.modifie = false;
    rendreSuggestions();
    afficherEtat(); rendreTout(); recalculer(); montrer("operations");
  }

  $("b-nouveau").addEventListener("click", function () {
    confirmerPerte().then(function (ok) { if (ok) charger(null, null, null); });
  });

  $("b-ouvrir").addEventListener("click", function () {
    confirmerPerte().then(function (ok) {
      if (!ok) return;
      return dlg.open({ multiple: false, filters: [{ name: "Coffre Taiseux", extensions: ["taiseux"] }] }).then(function (chemin) {
        if (!chemin) return;
        var essayer = function () {
          return demanderPhrase(false).then(function (p) {
            if (!p) return;
            return invoke("coffre_ouvrir", { chemin: chemin, phrase: p }).then(function (d) { charger(d, chemin, p); })
              .catch(function (e) { return dlg.message(String(e), { title: "Ouverture impossible", kind: "error" }).then(essayer); });
          });
        };
        return essayer();
      });
    });
  });

  document.addEventListener("keydown", function (e) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") { e.preventDefault(); enregistrer(e.shiftKey); return; }
    // « N » : nouvelle opération, hors d'un champ de saisie et d'une boîte de dialogue.
    var cible = e.target && e.target.tagName;
    if (e.key.toLowerCase() === "n" && !e.metaKey && !e.ctrlKey && !e.altKey && cible !== "INPUT" && cible !== "SELECT" && cible !== "TEXTAREA" &&
        !document.querySelector("dialog[open]") && vueVisible("operations")) { e.preventDefault(); ouvrirFiche(); }
  });

  // Fermeture : prévenir si des données non enregistrées vont disparaître.
  var fenetre = T.window.getCurrentWindow();
  fenetre.onCloseRequested(function (ev) {
    if (!etat.modifie || (!etat.chemin && !toutesLignes())) return;
    ev.preventDefault();
    var msg = etat.chemin ? "Les modifications non enregistrées seront perdues." : "Mode sans trace : vos opérations vont disparaître.";
    dlg.ask(msg, { title: "Fermer Taiseux ?", kind: "warning", okLabel: "Fermer", cancelLabel: "Annuler" })
      .then(function (ok) { if (ok) { etat.membres = []; etat.phrase = null; fenetre.destroy(); } });
  });

  function rendreTout() { rendreMembres(); rendreTableau(); afficherPlusValues(); afficherAnnees(); }

  invoke("version").then(function (v) { $("version").textContent = "Version " + v; });
  invoke("plateformes").then(function (ps) {
    var ul = $("liste-plateformes");
    ps.forEach(function (p) { ul.appendChild(el("li", {}, [el("strong", { text: p[0] }), " : " + p[1]])); });
    plateformesConnues = ps.map(function (p) { return p[0]; });
    rendreSuggestions();
  });
  afficherEtat();
  rendreTout();
  recalculer();
})();
