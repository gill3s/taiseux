#!/usr/bin/env python3
"""Implémentation de référence INDÉPENDANTE (Python, fractions exactes) de l'art. 150 VH bis.

Écrite séparément du moteur JavaScript, à partir du texte du BOFiP (BOI-RPPM-PVBMC-30),
pour recouper ses résultats. Génère des portefeuilles aléatoires et leurs résultats attendus.

    python3 -I tests/reference.py 500 7 > tests/aleatoires.json
"""
import json
import random
import sys
from fractions import Fraction as F


def reference(ops):
    """ops déjà dans l'ordre chronologique. Retourne la liste des plus-values (Fraction) par cession."""
    acquis = F(0)
    imputees = F(0)
    soultes = F(0)
    resultats = []
    for op in ops:
        m = F(op["montant"])
        if op["type"] == "achat":
            acquis += m
        elif op["type"] == "soulte_recue":
            soultes += m
        elif op["type"] == "cession":
            v = F(op["valeur_portefeuille"])
            frais = F(op.get("frais") or 0)
            base = acquis - imputees - soultes
            part = base * m / v
            resultats.append((m - frais) - part)
            imputees += part
    return resultats


def aleatoire(rng):
    ops = []
    annee = rng.choice([2024, 2025, 2026])
    jour = 0
    valeur = F(0)  # valeur de marché simulée du portefeuille
    for _ in range(rng.randint(2, 25)):
        jour += rng.randint(1, 20)
        date = "%d-%02d-%02d" % (annee + jour // 336, 1 + (jour % 336) // 28, 1 + jour % 28)
        valeur *= F(rng.randint(70, 150), 100)  # le marché bouge
        r = rng.random()
        if valeur == 0 or r < 0.45:
            m = F(rng.randint(1, 500000), 100)
            ops.append({"date": date, "type": "achat", "montant": str(float(m))})
            valeur += m
        elif r < 0.52:
            m = F(rng.randint(1, 20000), 100)
            ops.append({"date": date, "type": "soulte_recue", "montant": str(float(m))})
        else:
            v = F(round(float(valeur) * 100), 100)
            if v <= 0:
                continue
            m = F(rng.randint(1, int(v * 100)), 100)
            frais = F(rng.randint(0, int(m * 2)), 100) if rng.random() < 0.6 else F(0)
            ops.append({"date": date, "type": "cession", "montant": str(float(m)),
                        "valeur_portefeuille": str(float(v)), "frais": str(float(frais))})
            valeur = v - m
    return ops


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 200
    graine = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    rng = random.Random(graine)
    cas = []
    for _ in range(n):
        ops = aleatoire(rng)
        cas.append({"ops": ops, "attendu": [float(x) for x in reference(ops)]})
    json.dump({"graine": graine, "cas": cas}, sys.stdout)


if __name__ == "__main__":
    main()
