"""Plot player skill evolution for every training focus.

Run after `cargo test --test player_evolution`, which writes
`evolution_data.json` next to this script.

Outputs two files:
- `evolution.png`         all 11 constant focuses + Switching on the 2x3 grid
- `evolution_switching.png`  dedicated view of the switching scenario

Usage:
    python tests/player_evolution/plot_evolution.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import matplotlib.pyplot as plt

HERE = Path(__file__).resolve().parent
DATA_PATH = HERE / "evolution_data.json"
OUT_MAIN = HERE / "evolution.png"
OUT_SWITCH = HERE / "evolution_switching.png"

# Skill index ranges per category (must match modify_skill ordering in player.rs).
CATEGORIES = [
    ("Athletics", range(0, 4)),
    ("Offense", range(4, 8)),
    ("Defense", range(8, 12)),
    ("Technical", range(12, 16)),
    ("Mental", range(16, 20)),
]


def category_avg(skills: list[float], rng: range) -> float:
    vals = [skills[i] for i in rng]
    return sum(vals) / len(vals)


def style_for(focus: str) -> dict:
    """Pick a consistent color + linestyle per focus label."""
    cat_colors = {
        "None": ("#777777", "-", 1.5),
        "Athletics": ("#1f77b4", "-", 1.5),
        "Offense": ("#d62728", "-", 1.5),
        "Defense": ("#2ca02c", "-", 1.5),
        "Technical": ("#9467bd", "-", 1.5),
        "Mental": ("#ff7f0e", "-", 1.5),
        "Switching": ("black", "-", 2.2),
    }
    if focus in cat_colors:
        c, ls, lw = cat_colors[focus]
        return {"color": c, "linestyle": ls, "linewidth": lw}
    # Position(N) — dashed, viridis-shaded
    if focus.startswith("Position("):
        n = int(focus[len("Position(") : -1])
        cmap = plt.get_cmap("viridis")
        return {
            "color": cmap(n / 4),
            "linestyle": "--",
            "linewidth": 1.0,
            "alpha": 0.9,
        }
    return {"color": "grey", "linestyle": ":", "linewidth": 1.0}


def plot_main(dump: dict) -> None:
    title = (
        f"{dump['player_name']} — {dump['population']} — "
        f"potential {dump['potential']:.2f} — best_pos {dump['best_position']}"
    )

    fig, axes = plt.subplots(2, 3, figsize=(16, 9), sharex=True, sharey=True)
    fig.suptitle(title, fontsize=12)
    axes = axes.flatten()

    panels = [("Overall", None)] + CATEGORIES

    for ax, (panel_name, panel_range) in zip(axes, panels):
        for s in dump["series"]:
            if s["focus"] == "Switching":
                continue
            snaps = s["snapshots"]
            xs = [snap["rel_age"] for snap in snaps]
            if panel_range is None:
                ys = [snap["overall"] for snap in snaps]
            else:
                ys = [category_avg(snap["skills"], panel_range) for snap in snaps]
            ax.plot(xs, ys, label=s["focus"], **style_for(s["focus"]))

        ax.set_title(
            panel_name
            if panel_range is None
            else f"{panel_name} avg (idx {panel_range.start}-{panel_range.stop - 1})"
        )
        ax.grid(True, alpha=0.3)
        ax.axhline(
            dump["potential"], linestyle="--", color="black", alpha=0.4, linewidth=0.8
        )
        ax.axvline(0.7, linestyle=":", color="grey", alpha=0.5, linewidth=0.8)

    axes[0].set_xlim(0.0, 1.0)
    axes[0].set_ylim(0.0, 20.5)
    for i in (0, 3):
        axes[i].set_ylabel("skill value")
    for ax in axes[3:]:
        ax.set_xlabel("relative age")

    # One legend for the whole figure, placed to the right of the grid.
    handles, labels = axes[0].get_legend_handles_labels()
    fig.legend(handles, labels, loc="center right", fontsize=8, framealpha=0.9)
    fig.tight_layout(rect=(0, 0, 0.9, 0.97))
    fig.savefig(OUT_MAIN, dpi=120)
    print(f"wrote {OUT_MAIN}")


def plot_switching(dump: dict) -> None:
    """Dedicated view: category averages over time for the Switching series,
    with vertical lines at each focus transition."""
    switching = next((s for s in dump["series"] if s["focus"] == "Switching"), None)
    if switching is None:
        print("no Switching series in dump", file=sys.stderr)
        return

    snaps = switching["snapshots"]
    xs = [snap["rel_age"] for snap in snaps]

    fig, ax = plt.subplots(figsize=(11, 6))
    fig.suptitle(
        f"Switching scenario — {dump['player_name']} — potential {dump['potential']:.2f}",
        fontsize=12,
    )

    for cat_name, cat_range in CATEGORIES:
        ys = [category_avg(snap["skills"], cat_range) for snap in snaps]
        ax.plot(xs, ys, label=cat_name, linewidth=1.8)

    overall = [snap["overall"] for snap in snaps]
    ax.plot(xs, overall, label="Overall", color="black", linewidth=1.2, linestyle=":")

    # Switch markers + labels showing the new focus each segment trains.
    chain = ["Athletics", "Offense", "Defense", "Technical", "Mental"]
    transitions = switching["transitions"]
    ymax = 20.0
    # Segments are [0 .. t0], [t0 .. t1], ..., [tN .. 1.0]
    boundaries = [0.0] + transitions + [1.0]
    for i, (lo, hi) in enumerate(zip(boundaries[:-1], boundaries[1:])):
        if i < len(chain):
            ax.text(
                (lo + hi) / 2,
                ymax - 0.5,
                chain[i],
                ha="center",
                va="top",
                fontsize=9,
                color="black",
                bbox=dict(
                    boxstyle="round,pad=0.2",
                    facecolor="white",
                    alpha=0.7,
                    edgecolor="grey",
                ),
            )
    for t in transitions:
        ax.axvline(t, linestyle="--", color="black", alpha=0.5, linewidth=0.9)

    ax.axhline(
        dump["potential"], linestyle="--", color="grey", alpha=0.4, linewidth=0.8
    )
    ax.axvline(0.7, linestyle=":", color="grey", alpha=0.5, linewidth=0.8)
    ax.set_xlim(0.0, 1.0)
    ax.set_ylim(0.0, 20.5)
    ax.set_xlabel("relative age")
    ax.set_ylabel("category average")
    ax.grid(True, alpha=0.3)
    ax.legend(loc="lower right", fontsize=9)

    fig.tight_layout(rect=(0, 0, 1, 0.96))
    fig.savefig(OUT_SWITCH, dpi=120)
    print(f"wrote {OUT_SWITCH}")


def main() -> int:
    if not DATA_PATH.exists():
        print(f"missing {DATA_PATH}", file=sys.stderr)
        print("run: cargo test --test player_evolution", file=sys.stderr)
        return 1

    with DATA_PATH.open() as f:
        dump = json.load(f)

    plot_main(dump)
    plot_switching(dump)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
