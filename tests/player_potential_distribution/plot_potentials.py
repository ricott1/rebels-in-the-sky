"""Plot distributions of player potential (and supporting stats) from a
large pool of randomly generated players.

Run after `cargo test --test player_potential_distribution`, which writes
`potential_data.json` next to this script.

Outputs:
- `potential_overall.png`  overall potential histogram + overall-vs-potential
                           scatter + potential-vs-age scatter
- `potential_by_age.png`   per-relative-age-bin potential histograms

Usage:
    python3 tests/player_potential_distribution/plot_potentials.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

HERE = Path(__file__).resolve().parent
DATA_PATH = HERE / "potential_data.json"
OUT_OVERALL = HERE / "potential_overall.png"
OUT_BY_AGE = HERE / "potential_by_age.png"

# Relative-age bin edges. `randomize` draws rel_age uniformly in [0, 0.55],
# so we cover that range plus a small tail.
AGE_BIN_EDGES = [0.0, 0.1, 0.2, 0.3, 0.4, 0.55]

POTENTIAL_BINS = np.linspace(0.0, 20.0, 41)  # 0.5-wide bins from 0..20


def plot_overall(dump: dict) -> None:
    samples = dump["samples"]
    potentials = np.array([s["potential"] for s in samples])
    overalls = np.array([s["overall"] for s in samples])
    rel_ages = np.array([s["relative_age"] for s in samples])

    fig, axes = plt.subplots(1, 3, figsize=(16, 5))
    fig.suptitle(
        f"Random player generation - n={dump['num_players']} - seed=0x{dump['seed']:X}",
        fontsize=12,
    )

    ax = axes[0]
    ax.hist(potentials, bins=POTENTIAL_BINS, color="#444444", alpha=0.85, edgecolor="white")
    ax.axvline(potentials.mean(), color="red", linestyle="--", linewidth=1.2,
               label=f"mean={potentials.mean():.2f}")
    ax.axvline(np.median(potentials), color="orange", linestyle=":", linewidth=1.2,
               label=f"median={np.median(potentials):.2f}")
    ax.set_title("Potential distribution (all players)")
    ax.set_xlabel("potential")
    ax.set_ylabel("count")
    ax.set_xlim(0.0, 20.0)
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right", fontsize=9)

    ax = axes[1]
    ax.scatter(overalls, potentials, s=4, alpha=0.25, color="#1f77b4")
    lo, hi = 0.0, 20.0
    ax.plot([lo, hi], [lo, hi], color="grey", linestyle="--", linewidth=0.8,
            label="potential = overall")
    ax.set_title("Potential vs current overall")
    ax.set_xlabel("current overall (average skill)")
    ax.set_ylabel("potential")
    ax.set_xlim(lo, hi)
    ax.set_ylim(lo, hi)
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper left", fontsize=9)

    ax = axes[2]
    ax.scatter(rel_ages, potentials, s=4, alpha=0.25, color="#2ca02c")
    ax.set_title("Potential vs relative age at generation")
    ax.set_xlabel("relative age")
    ax.set_ylabel("potential")
    ax.set_xlim(0.0, 1.0)
    ax.set_ylim(0.0, 20.0)
    ax.grid(True, alpha=0.3)

    fig.tight_layout(rect=(0, 0, 1, 0.95))
    fig.savefig(OUT_OVERALL, dpi=120)
    print(f"wrote {OUT_OVERALL}")


def plot_by_age(dump: dict) -> None:
    samples = dump["samples"]
    rel_ages = np.array([s["relative_age"] for s in samples])
    potentials = np.array([s["potential"] for s in samples])

    edges = AGE_BIN_EDGES
    bin_labels = [f"[{edges[i]:.2f}, {edges[i + 1]:.2f})" for i in range(len(edges) - 1)]
    cmap = plt.get_cmap("viridis")
    bin_colors = [cmap(i / max(1, len(bin_labels) - 1)) for i in range(len(bin_labels))]

    # Group potentials by age bin.
    binned: list[np.ndarray] = []
    for i in range(len(edges) - 1):
        lo, hi = edges[i], edges[i + 1]
        if i == len(edges) - 2:
            mask = (rel_ages >= lo) & (rel_ages <= hi)
        else:
            mask = (rel_ages >= lo) & (rel_ages < hi)
        binned.append(potentials[mask])

    n_panels = len(bin_labels) + 1  # +1 for overlay
    cols = 3
    rows = (n_panels + cols - 1) // cols
    fig, axes = plt.subplots(rows, cols, figsize=(4.5 * cols, 3.2 * rows),
                             sharex=True, sharey=False)
    fig.suptitle(
        f"Potential distribution by relative age - n={dump['num_players']}",
        fontsize=12,
    )
    axes_flat = axes.flatten()

    for ax, label, vals, color in zip(axes_flat, bin_labels, binned, bin_colors):
        if len(vals) == 0:
            ax.set_title(f"rel_age {label} (n=0)")
            ax.set_xlim(0.0, 20.0)
            continue
        ax.hist(vals, bins=POTENTIAL_BINS, color=color, alpha=0.85, edgecolor="white")
        ax.axvline(vals.mean(), color="black", linestyle="--", linewidth=1.0)
        ax.set_title(f"rel_age {label} (n={len(vals)}, mean={vals.mean():.2f})")
        ax.set_xlim(0.0, 20.0)
        ax.grid(True, alpha=0.3)

    # Overlay panel: all bins as step histograms on one axis (density).
    ax = axes_flat[len(bin_labels)]
    for label, vals, color in zip(bin_labels, binned, bin_colors):
        if len(vals) == 0:
            continue
        ax.hist(vals, bins=POTENTIAL_BINS, color=color, alpha=0.8, histtype="step",
                linewidth=1.6, label=f"rel_age {label}", density=True)
    ax.set_title("All age bins (density)")
    ax.set_xlim(0.0, 20.0)
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right", fontsize=8)

    for ax in axes_flat[len(bin_labels) + 1:]:
        ax.set_visible(False)

    for ax in axes_flat:
        ax.set_xlabel("potential")

    fig.tight_layout(rect=(0, 0, 1, 0.96))
    fig.savefig(OUT_BY_AGE, dpi=120)
    print(f"wrote {OUT_BY_AGE}")


def main() -> int:
    if not DATA_PATH.exists():
        print(f"missing {DATA_PATH}", file=sys.stderr)
        print("run: cargo test --test player_potential_distribution", file=sys.stderr)
        return 1

    with DATA_PATH.open() as f:
        dump = json.load(f)

    plot_overall(dump)
    plot_by_age(dump)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
