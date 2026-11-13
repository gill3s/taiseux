#!/usr/bin/env python3
"""Référence INDÉPENDANTE du suivi par actif (version 0.2), en fractions exactes.

Écrite à partir des règles (article 150 VH bis, BOFiP, formulaire 2086), pas du code Rust, pour recouper le moteur :
- la valeur globale du portefeuille à chaque cession = somme des quantités détenues × cours du jour ;
- échange crypto contre crypto sans soulte : sursis, seules les quantités changent ;
- échange avec soulte (reçue > 0, versée < 0) : cession au prix « valeur reçue + soulte », puis acquisition des
  actifs reçus pour leur valeur (comme le BOFiP § 120) ;
- transfert entre ses comptes : sans effet fiscal, les frais de réseau en crypto diminuent les avoirs ;
- revenu gratuit (airdrop) : quantité ajoutée, prix d'acquisition 0.

    python3 -I tests/reference_actifs.py 300 11 > /tmp/actifs.json
"""
import json
import random
import sys
from fractions import Fraction as F

ACTIFS = ["BTC", "ETH", "SOL", "ADA"]


def s(x):
    """Nombre décimal court, lu tel quel par les deux implémentations."""
    return format(float(x), ".10g")


def scenario(rng):
    jours = [f"2026-{m:02d}-{d:02d}" for m in range(1, 13) for d in (3, 10, 17, 24)]
    # Cours quotidiens : marche aléatoire par actif.
    base = {"BTC": 60000, "ETH": 3000, "SOL": 150, "ADA": 1}
    cours = {}
    for a in ACTIFS:
        p = F(base[a])
        for j in jours:
            p = p * F(rng.randint(85, 118), 100)
            p = F(s(p))
            cours[(a, j)] = p
    ops, avoirs = [], {a: F(0) for a in ACTIFS}
    for j in jours:
        for _ in range(rng.randint(0, 2)):
            r = rng.random()
            tenus = [a for a in ACTIFS if avoirs[a] > 0]
            if r < 0.35 or not tenus:
                a = rng.choice(ACTIFS)
                q = F(s(F(rng.randint(1, 4000), 1000) * 1000 / cours[(a, j)]))
                if q <= 0:
                    continue
                montant = F(s(q * cours[(a, j)] * F(rng.randint(100, 103), 100)))  # frais d'achat compris
                ops.append({"date": j, "type": "achat", "actif": a, "quantite": s(q), "montant": s(montant)})
                avoirs[a] += q
            elif r < 0.6:
                a = rng.choice(tenus)
                q = F(s(avoirs[a] * F(rng.randint(5, 100), 100)))
                if q <= 0 or q > avoirs[a]:
                    continue
                prix = F(s(q * cours[(a, j)]))
                frais = F(s(prix * F(rng.randint(0, 15), 1000)))
                ops.append({"date": j, "type": "cession", "actif": a, "quantite": s(q), "montant": s(prix), "frais": s(frais)})
                avoirs[a] -= q
            elif r < 0.75:
                a = rng.choice(tenus)
                b = rng.choice([x for x in ACTIFS if x != a])
                q = F(s(avoirs[a] * F(rng.randint(5, 100), 100)))
                if q <= 0 or q > avoirs[a]:
                    continue
                donne = q * cours[(a, j)]
                op = {"date": j, "type": "echange", "actif": a, "quantite": s(q), "actif_recu": b}
                k = rng.random()
                if k < 0.5:  # sans soulte
                    qb = F(s(donne / cours[(b, j)]))
                    op["quantite_recue"] = s(qb)
                else:
                    soulte = F(s(donne * F(rng.randint(5, 30), 100))) * (1 if k < 0.75 else -1)
                    v_recue = F(s(donne - soulte))  # reçue : on reçoit moins d'actifs ; versée : plus
                    qb = F(s(v_recue / cours[(b, j)]))
                    op.update({"quantite_recue": s(qb), "valeur_recue": s(v_recue), "soulte": s(soulte)})
                ops.append(op)
                avoirs[a] -= q
                avoirs[b] += qb
            elif r < 0.88:
                a = rng.choice(tenus)
                fr = F(s(avoirs[a] * F(rng.randint(0, 20), 10000)))
                ops.append({"date": j, "type": "transfert", "actif": a, "quantite": s(avoirs[a] / 2), "quantite_frais": s(fr)})
                avoirs[a] -= fr
            else:
                a = rng.choice(ACTIFS)
                q = F(s(F(rng.randint(1, 50), 1000) * 100 / cours[(a, j)]))
                ops.append({"date": j, "type": "revenu_gratuit", "actif": a, "quantite": s(q), "montant": "0"})
                avoirs[a] += q
    return ops, [{"actif": a, "date": j, "prix": s(p)} for (a, j), p in cours.items()]


def reference(ops, cours):
    """Plus ou moins-value de chaque cession (y compris échanges avec soulte), dans l'ordre chronologique."""
    prix = {(c["actif"], c["date"]): F(c["prix"]) for c in cours}
    avoirs, pta, imputees, resultats = {}, F(0), F(0), []
    G = lambda o, k: F(o[k]) if o.get(k) not in (None, "") else F(0)
    for o in sorted(enumerate(ops), key=lambda t: (t[1]["date"], t[0])):
        o = o[1]
        t, a = o["type"], o.get("actif")

        def ceder(p217, frais):
            nonlocal imputees
            v = sum(q * prix[(x, o["date"])] for x, q in avoirs.items() if q > 0)
            part = (pta - imputees) * p217 / v
            resultats.append(p217 - frais - part)
            imputees += part

        if t == "achat":
            pta += G(o, "montant")
            avoirs[a] = avoirs.get(a, F(0)) + G(o, "quantite")
        elif t == "revenu_gratuit":
            pta += G(o, "montant")
            avoirs[a] = avoirs.get(a, F(0)) + G(o, "quantite")
        elif t == "transfert":
            avoirs[a] -= G(o, "quantite_frais")
        elif t == "cession":
            ceder(G(o, "montant"), G(o, "frais"))
            avoirs[a] -= G(o, "quantite")
        elif t == "echange":
            if G(o, "soulte") != 0:
                ceder(G(o, "valeur_recue") + G(o, "soulte"), G(o, "frais"))
                pta += G(o, "valeur_recue")
            avoirs[a] -= G(o, "quantite")
            b = o["actif_recu"]
            avoirs[b] = avoirs.get(b, F(0)) + G(o, "quantite_recue")
    return resultats


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 100
    rng = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 1)
    cas = []
    for _ in range(n):
        ops, cours = scenario(rng)
        cas.append({"ops": ops, "cours": cours, "attendu": [float(x) for x in reference(ops, cours)]})
    json.dump({"cas": cas}, sys.stdout)


if __name__ == "__main__":
    main()
