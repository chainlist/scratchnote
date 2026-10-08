"""Train the Labels classifier (SPEC 3.14) and write classifier.json.

It reads a space whose notes are labelled: the app must have embedded them
(its `.scratchnote/space.db` holds the vectors) and `labels.csv` beside its
`notes/` gives each note's labels, one row per note:

    note_id,date,life,job
    01K6F5WK58GF326S3DZVZAK59E,2025-10-01,work,finance-accounting
    01K6FAJAP48WS52WYCDQ02V9J4,2025-10-01,social,

`job` is empty unless `life` is `work`. It scores the two classifiers by
5-fold cross-validation, then trains them on every note and writes the
weights the app builds in.

    python -m venv .venv
    .venv/Scripts/pip install -r scripts/classifier/requirements.txt
    .venv/Scripts/python scripts/classifier/train.py "<notes root>/spaces/Label Training"
"""

import argparse
import collections
import csv
import json
import os
import sqlite3

import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import StratifiedKFold

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "..", "src-tauri", "src", "embed", "classifier.json")
# Regularisation, best of 1, 4, 16 and 64 for both classifiers.
C = 4
# Work notes that touch on life were filed under family or home with every
# label weighted alike; three times the weight keeps 93% of them as work.
WORK_WEIGHT = 3
# The share under which the app hides a label, for the report.
SURE = 0.5


def load(space):
    db = sqlite3.connect(f"file:{os.path.join(space, '.scratchnote', 'space.db')}?mode=ro", uri=True)
    vectors = {id: np.frombuffer(blob, dtype="<f4") for id, blob in db.execute("SELECT id, vector FROM vectors")}
    recipe = dict(db.execute("SELECT key, value FROM meta").fetchall())["vectors_model"]
    with open(os.path.join(space, "labels.csv"), encoding="utf-8") as f:
        rows = [row for row in csv.DictReader(f) if row["note_id"] in vectors]
    x = np.stack([vectors[row["note_id"]] for row in rows]).astype(np.float64)
    # The app's vectors are unit length already; this keeps it so.
    x /= np.linalg.norm(x, axis=1, keepdims=True)
    life = np.array([row["life"] for row in rows])
    job = np.array([row["job"] for row in rows])
    return x, life, job, recipe


def life_weights(life):
    labels = sorted(set(life))
    weights = {label: len(life) / (len(labels) * np.sum(life == label)) for label in labels}
    weights["work"] *= WORK_WEIGHT
    return weights


def report(name, x, y, weights):
    """Cross-validated scores, and the labels most often missed."""
    predicted = np.empty(len(y), dtype=object)
    sure = np.zeros(len(y))
    for train, test in StratifiedKFold(5, shuffle=True, random_state=0).split(x, y):
        model = LogisticRegression(C=C, class_weight=weights, max_iter=5000).fit(x[train], y[train])
        shares = model.predict_proba(x[test])
        predicted[test] = model.classes_[shares.argmax(1)]
        sure[test] = shares.max(1)
    recall = {label: np.mean(predicted[y == label] == label) for label in sorted(set(y))}
    shown = sure >= SURE
    print(f"\n{name}: {len(y)} notes, {len(recall)} labels")
    print(f"  right: {np.mean(predicted == y):.1%} of notes, {np.mean(list(recall.values())):.1%} per label on average")
    print(f"  {SURE:.0%} or more: {shown.mean():.0%} of notes, {np.mean(predicted[shown] == y[shown]):.1%} of them right")
    for label, share in sorted(recall.items(), key=lambda kv: kv[1])[:5]:
        missed = collections.Counter(predicted[(y == label) & (predicted != label)]).most_common(2)
        print(f"  {label:20} {share:.0%}  taken for {', '.join(f'{other} x{n}' for other, n in missed) or '-'}")
    return predicted


def head(model):
    return {
        "labels": list(model.classes_),
        "weights": np.round(model.coef_, 6).tolist(),
        "bias": np.round(model.intercept_, 6).tolist(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("space", help="the labelled space's folder")
    parser.add_argument("--out", default=OUT, help="where to write the weights")
    args = parser.parse_args()

    x, life, job, recipe = load(args.space)
    work = life == "work"
    weights = life_weights(life)
    life_predicted = report("Part of life", x, life, weights)
    job_predicted = report("Job family, work notes", x[work], job[work], "balanced")
    both = np.mean((life_predicted[work] == "work") & (job_predicted == job[work]))
    print(f"\nWork notes with both right: {both:.1%}")

    life_model = LogisticRegression(C=C, class_weight=weights, max_iter=5000).fit(x, life)
    job_model = LogisticRegression(C=C, class_weight="balanced", max_iter=5000).fit(x[work], job[work])
    model = {
        "embedding": recipe,
        "normalize": "l2",
        "trained_on": f"{len(life)} notes, {len(life_model.classes_)} life labels, {len(job_model.classes_)} job families",
        "life": head(life_model),
        "job": head(job_model),
    }
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(model, f)
    print(f"wrote {os.path.normpath(args.out)} ({os.path.getsize(args.out) // 1024} KB)")


if __name__ == "__main__":
    main()
