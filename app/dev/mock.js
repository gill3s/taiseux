// Aperçu de l'interface dans un navigateur, SANS Tauri : imite window.__TAURI__. Les commandes partent au serveur de
// développement (dev/serveur.py), qui appelle le vrai moteur Rust ; les dialogues de fichiers sont des prompt().
// Sert uniquement au développement (dev/preview.sh) : l'application, elle, n'a aucun accès réseau.
(function () {
  function invoke(cmd, args) {
    return fetch("/invoke", { method: "POST", body: JSON.stringify({ cmd: cmd, args: args || {} }) })
      .then(function (r) { return r.json(); })
      .then(function (r) { if ("err" in r) throw r.err; return r.ok; });
  }
  window.__TAURI__ = {
    core: { invoke: invoke },
    dialog: {
      open: function () { return Promise.resolve(prompt("Chemin du fichier à ouvrir")); },
      save: function (o) { return Promise.resolve(prompt("Enregistrer sous", o && o.defaultPath)); },
      ask: function (m) { return Promise.resolve(confirm(m)); },
      message: function (m) { alert(m); return Promise.resolve(); }
    },
    window: { getCurrentWindow: function () { return { onCloseRequested: function () {}, destroy: function () {} }; } }
  };
})();
