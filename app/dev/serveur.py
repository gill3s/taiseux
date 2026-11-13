#!/usr/bin/env python3
"""Aperçu de l'interface dans un navigateur, avec le VRAI moteur (développement seulement).

Sert dev/out/ (construit par dev/preview.sh) et transmet chaque appel de l'interface (POST /invoke) au binaire
d'exemple `dev_api` (même nom de commande, mêmes arguments que l'application). Écoute sur 127.0.0.1 seulement.
Usage : python3 dev/serveur.py [port]   (8894 par défaut)
"""
import http.server, os, subprocess, sys

ICI = os.path.dirname(os.path.abspath(__file__))
BIN = os.environ.get("TAISEUX_DEV_API") or os.path.join(ICI, "..", "target", "release", "examples", "dev_api")


class Gestion(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=os.path.join(ICI, "out"), **k)

    def do_POST(self):
        if self.path != "/invoke":
            self.send_error(404)
            return
        corps = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        r = subprocess.run([BIN], input=corps, capture_output=True, timeout=120)
        sortie = r.stdout or ('{"err": %r}' % r.stderr.decode(errors="replace")[-500:]).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(sortie)))
        self.end_headers()
        self.wfile.write(sortie)

    def log_message(self, *a):
        pass


port = int(sys.argv[1]) if len(sys.argv) > 1 else 8894
http.server.ThreadingHTTPServer(("127.0.0.1", port), Gestion).serve_forever()
