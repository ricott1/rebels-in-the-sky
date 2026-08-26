#!/usr/bin/env python3
"""Regenerate the primary-attacker and final-shooter tables from the game's weights.

Standalone helper (not a cargo test). Edit the weight arrays below to match
src/game_engine/*.rs, then run:  python3 tests/tactic_tables.py
"""

roles = ["PG", "SG", "SF", "PF", "C"]

# tactic.rs pick_action: [Isolation, OffTheScreen, PickAndRoll, Post]
tactics = {
    "Balanced":    [2, 2, 2, 2],
    "BigPirates":  [1, 1, 2, 4],
    "Arrembaggio": [3, 1, 3, 1],
    "Shooters":    [1, 4, 2, 1],
}
actions = ["Isolation", "OffTheScreen", "PickAndRoll", "Post"]

# primary attacker (initiator) weights per action
primary = {
    "Isolation":    [4, 5, 3, 2, 1],     # isolation.rs:20
    "OffTheScreen": [50, 30, 25, 2, 1],  # off_the_screen.rs:22 (ball handler)
    "PickAndRoll":  [70, 15, 25, 2, 1],  # pick_and_roll.rs:20 (ball handler)
    "Post":         [1, 2, 10, 30, 45],  # post.rs:23
}
# the man who takes the shot when the sampled target differs from the initiator
off_screen_target = [4, 5, 3, 2, 1]      # off_the_screen.rs:47 (excludes ball handler)
pnr_roller        = [1, 2, 4, 3, 2]      # pick_and_roll.rs:44 (sampled unconditionally)


def norm(v):
    s = float(sum(v))
    return [x / s for x in v]


# OffTheScreen shooter = target marginal: ball handler sampled first, then excluded.
def off_target_marginal(play_w, target_w):
    p = norm(play_w)
    total = sum(target_w)
    out = [0.0] * 5
    for j in range(5):
        for k in range(5):
            if k != j:
                out[k] += p[j] * target_w[k] / (total - target_w[j])
    return out


shooter = {
    "Isolation":    norm(primary["Isolation"]),   # iso player shoots
    "OffTheScreen": off_target_marginal(primary["OffTheScreen"], off_screen_target),
    "PickAndRoll":  norm(pnr_roller),              # roller/target shoots
    "Post":         norm(primary["Post"]),         # poster shoots
}
primaryN = {a: norm(primary[a]) for a in actions}
pact = {t: norm(w) for t, w in tactics.items()}


def pct(x):
    return f"{100 * x:5.1f}%"


def table(dist, title):
    print(f"\n=== {title} ===")
    print(f"{'tactic':12} " + " ".join(f"{r:>7}" for r in roles))
    for t in tactics:
        row = [sum(pact[t][ai] * dist[a][ri] for ai, a in enumerate(actions)) for ri in range(5)]
        print(f"{t:12} " + " ".join(f"{pct(x):>7}" for x in row))


def main():
    print("Per-action shooter distribution (who shoots | action):")
    print(f"{'action':13} " + " ".join(f"{r:>7}" for r in roles))
    for a in actions:
        print(f"{a:13} " + " ".join(f"{pct(x):>7}" for x in shooter[a]))
    table(primaryN, "TABLE 1 - PRIMARY ATTACKER  P(role|tactic)")
    table(shooter,  "TABLE 2 - FINAL SHOOTER (approx)  P(role|tactic)")


if __name__ == "__main__":
    main()
