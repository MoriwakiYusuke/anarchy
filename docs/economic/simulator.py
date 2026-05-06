"""
Anarchy economic model simulator (v2 — corrected accounting).

Accounting rules (matches Substrate fungible semantics):
  - Pool balances are NOT part of total_issuance. They are deferred-mint authority
    (a pool deposit is a counter; a payout to a node mints from that authority).
  - total_issuance = cumulative mint − cumulative burn.
  - Mints: block reward (full), faucet, pool payouts (when realized).
  - Burns: base-fee burn, post burn share, DM burn share, slashing burn.

Compares:
  M0 = current model (post 80/10/10, fixed reaction reward, no tail emission, fee=0)
  M1 = TSTS proposed model (block reward fan-out 50/30/20, EIP-1559, dynamic γ,
       tail emission 0.5, storage stake)

Block time = 30s. 1 day = 2880 blocks.
"""

import math
from dataclasses import dataclass
from typing import List

BLOCKS_PER_DAY = 2880
BLOCKS_PER_YEAR = 1_051_200
DAYS_PER_YEAR = 365


@dataclass
class WorldParams:
    dau_curve: List[int]
    posts_per_dau_per_day: float = 3.0
    avg_post_kb: float = 4.0
    reactions_per_dau_per_day: float = 30.0
    dms_per_dau_per_day: float = 5.0
    avg_dm_kb: float = 1.0
    storage_node_count: int = 100
    miner_count: int = 50
    spam_posts_per_day: int = 0
    sybil_reactors: int = 0


# --- M0: current model ---------------------------------------------------

@dataclass
class M0State:
    block_height: int = 0
    total_issuance: float = 11_000_000.0  # genesis: 1M storage + 10M reaction
    storage_pool: float = 1_000_000.0
    reaction_pool: float = 10_000_000.0
    total_minted: float = 11_000_000.0    # genesis
    total_burned: float = 0.0
    miner_revenue: float = 0.0
    storage_paid: float = 0.0
    reaction_paid: float = 0.0


def simulate_m0(world: WorldParams, days: int) -> M0State:
    s = M0State()
    INITIAL_REWARD = 5.0
    HALVING = 4_204_800
    POST_BASE = 100.0
    POST_BYTE = 0.001
    DM_BASE = 1.0
    DM_BYTE = 0.05
    REACTION_REWARD = 1.0
    STORAGE_REWARD_PER_BYTE = 1e-12

    for d in range(days):
        dau = world.dau_curve[min(d, len(world.dau_curve) - 1)]

        # Block rewards (all to miner, halving only — no tail)
        for b in range(BLOCKS_PER_DAY):
            h = s.block_height + b
            halvings = h // HALVING
            reward = INITIAL_REWARD / (2 ** halvings) if halvings < 64 else 0
            s.miner_revenue += reward
            s.total_issuance += reward
            s.total_minted += reward
        s.block_height += BLOCKS_PER_DAY

        # Posts
        posts = int(dau * world.posts_per_dau_per_day) + world.spam_posts_per_day
        avg_bytes = world.avg_post_kb * 1024
        post_cost_each = POST_BASE + POST_BYTE * avg_bytes
        post_total = posts * post_cost_each
        # burn_from(total) → −total
        s.total_issuance -= post_total
        s.total_burned += post_total
        # pool counter increments (deferred-mint authority)
        s.storage_pool += post_total * 0.8
        s.reaction_pool += post_total * 0.1
        # 10% remains as permanent burn (counter not credited)

        # DMs
        dms = int(dau * world.dms_per_dau_per_day)
        dm_avg_bytes = world.avg_dm_kb * 1024
        dm_cost = dms * (DM_BASE + DM_BYTE * dm_avg_bytes)
        s.total_issuance -= dm_cost
        s.total_burned += dm_cost
        s.storage_pool += dm_cost * 0.8
        # 10% stealth → () = effective burn (no counter); 10% residual burn

        # Storage rewards (per-byte × challenges/day)
        active_bytes = posts * avg_bytes * 30  # 30-day rolling window
        daily_storage_payout = active_bytes * STORAGE_REWARD_PER_BYTE * 24
        daily_storage_payout = min(daily_storage_payout, s.storage_pool)
        s.storage_pool -= daily_storage_payout
        s.storage_paid += daily_storage_payout
        s.total_issuance += daily_storage_payout  # deferred mint realized
        s.total_minted += daily_storage_payout

        # Reaction rewards (fixed 1 MORAL each)
        reactions = int(dau * world.reactions_per_dau_per_day) + world.sybil_reactors
        reaction_payout_target = reactions * REACTION_REWARD
        reaction_payout = min(reaction_payout_target, s.reaction_pool)
        s.reaction_pool -= reaction_payout
        s.reaction_paid += reaction_payout
        s.total_issuance += reaction_payout
        s.total_minted += reaction_payout

    return s


# --- M1: TSTS proposed model --------------------------------------------

@dataclass
class M1State:
    block_height: int = 0
    total_issuance: float = 0.0
    storage_pool: float = 0.0
    reaction_pool: float = 0.0
    stealth_pool: float = 0.0
    repair_pool: float = 0.0
    total_bond_locked: float = 0.0
    total_minted: float = 0.0
    total_burned: float = 0.0
    miner_revenue: float = 0.0
    storage_paid: float = 0.0
    reaction_paid: float = 0.0
    base_fee: float = 1e-5  # MORAL/byte initially (~0.04 MORAL for 4KB)
    base_fee_burned: float = 0.0
    faucet_minted_total: float = 0.0


def simulate_m1(world: WorldParams, days: int) -> M1State:
    s = M1State()

    INITIAL_REWARD = 5.0
    HALVING = 4_204_800
    TAIL = 0.5
    MINER_SHARE = 0.50
    STORAGE_SHARE = 0.30
    REACTION_SHARE = 0.20

    POST_STORAGE_SHARE = 0.50
    POST_REACTION_SHARE = 0.20
    POST_BURN_SHARE = 0.30
    DM_STORAGE_SHARE = 0.50
    DM_STEALTH_SHARE = 0.20
    DM_BURN_SHARE = 0.30

    POST_BASE = 50.0
    POST_BYTE_TIP = 0.0008
    DM_BASE = 0.5
    DM_BYTE_TIP = 0.04

    GAS_TARGET_BYTES_PER_BLOCK = 50_000  # 50 KB target (= ~12 posts of 4KB) per block
    BASE_FEE_INIT = 1e-5
    BASE_FEE_MAX = 0.1  # cap at 0.1 MORAL/byte (= 100 MORAL/KB)
    BASE_FEE_MIN = 1e-7
    s.base_fee = BASE_FEE_INIT

    BOND_PER_GB = 10.0
    NODE_AVG_CAPACITY_GB = 100.0
    s.total_bond_locked = world.storage_node_count * NODE_AVG_CAPACITY_GB * BOND_PER_GB

    BASE_REWARD_PER_BYTE = 5e-9   # 5 nano-MORAL/byte
    SIGMA_TARGET_STORAGE = 500_000.0

    REACTOR_DECAY_K = 100

    FAUCET_HARD_CAP_MORAL = 100_000.0
    FAUCET_REWARD = 100.0

    # Genesis: 100k endowed accounts + 100k storage pool seed + 100k reaction pool seed
    genesis_endowed = 100_000.0
    genesis_storage_seed = 100_000.0
    genesis_reaction_seed = 100_000.0
    s.total_issuance = genesis_endowed
    s.storage_pool = genesis_storage_seed
    s.reaction_pool = genesis_reaction_seed
    s.total_minted = genesis_endowed
    # genesis seeds are not counted as issued until paid out

    for d in range(days):
        dau = world.dau_curve[min(d, len(world.dau_curve) - 1)]
        posts = int(dau * world.posts_per_dau_per_day) + world.spam_posts_per_day
        avg_bytes = world.avg_post_kb * 1024
        dms = int(dau * world.dms_per_dau_per_day)
        dm_avg_bytes = world.avg_dm_kb * 1024
        bytes_per_block = (posts * avg_bytes + dms * dm_avg_bytes) / BLOCKS_PER_DAY

        # Per-block loop: block reward + EIP-1559 base fee
        for b in range(BLOCKS_PER_DAY):
            h = s.block_height + b
            halvings = h // HALVING
            base = INITIAL_REWARD / (2 ** halvings) if halvings < 64 else 0
            reward = max(base, TAIL)
            # 3-way fan-out
            s.miner_revenue += reward * MINER_SHARE
            s.total_issuance += reward * MINER_SHARE  # miner mint
            s.total_minted += reward * MINER_SHARE
            s.storage_pool += reward * STORAGE_SHARE  # pool deposit (deferred mint)
            s.reaction_pool += reward * REACTION_SHARE

            # EIP-1559 base fee adjustment per block
            utilization = bytes_per_block / GAS_TARGET_BYTES_PER_BLOCK
            # b = b * (1 + 1/8 * (u - 1)), clamped
            adj = 1 + (utilization - 1) / 8
            adj = max(min(adj, 1.125), 0.875)
            s.base_fee = max(min(s.base_fee * adj, BASE_FEE_MAX), BASE_FEE_MIN)
        s.block_height += BLOCKS_PER_DAY

        # Posts (apply current base_fee)
        post_cost_each = POST_BASE + (POST_BYTE_TIP + s.base_fee) * avg_bytes
        post_total = posts * post_cost_each
        post_base_fee_burn = posts * s.base_fee * avg_bytes
        post_remaining = post_total - post_base_fee_burn
        s.total_issuance -= post_total
        s.total_burned += post_base_fee_burn + post_remaining * POST_BURN_SHARE
        s.base_fee_burned += post_base_fee_burn
        s.storage_pool += post_remaining * POST_STORAGE_SHARE
        s.reaction_pool += post_remaining * POST_REACTION_SHARE

        # DMs
        dm_cost_each = DM_BASE + (DM_BYTE_TIP + s.base_fee) * dm_avg_bytes
        dm_total = dms * dm_cost_each
        dm_base_fee_burn = dms * s.base_fee * dm_avg_bytes
        dm_remaining = dm_total - dm_base_fee_burn
        s.total_issuance -= dm_total
        s.total_burned += dm_base_fee_burn + dm_remaining * DM_BURN_SHARE
        s.storage_pool += dm_remaining * DM_STORAGE_SHARE
        s.stealth_pool += dm_remaining * DM_STEALTH_SHARE

        # Faucet (until cap)
        if s.faucet_minted_total < FAUCET_HARD_CAP_MORAL:
            daily_claims = min(50, int((FAUCET_HARD_CAP_MORAL - s.faucet_minted_total) / FAUCET_REWARD))
            faucet_mint = daily_claims * FAUCET_REWARD
            s.faucet_minted_total += faucet_mint
            s.total_issuance += faucet_mint
            s.total_minted += faucet_mint

        # Storage rewards (dynamic)
        active_bytes = posts * avg_bytes * 30
        sqrt_bond_factor = math.sqrt(min(1.0, s.total_bond_locked / 1_000_000.0))
        pool_ratio = min(1.0, s.storage_pool / SIGMA_TARGET_STORAGE)
        daily_storage_payout = active_bytes * BASE_REWARD_PER_BYTE * 24 * sqrt_bond_factor * pool_ratio
        daily_storage_payout = min(daily_storage_payout, s.storage_pool)
        s.storage_pool -= daily_storage_payout
        s.storage_paid += daily_storage_payout
        s.total_issuance += daily_storage_payout
        s.total_minted += daily_storage_payout

        # Reactions (dynamic γ + reactor decay)
        legitimate_reactions = int(dau * world.reactions_per_dau_per_day)
        sybil_reactions = world.sybil_reactors

        if world.reactions_per_dau_per_day > 0:
            n_per = int(world.reactions_per_dau_per_day)
            decay_legit_avg = sum(1.0 / math.sqrt(1 + n / REACTOR_DECAY_K) for n in range(n_per)) / max(n_per, 1)
        else:
            decay_legit_avg = 1.0

        gamma = s.reaction_pool / max(s.total_issuance, 1e-9)
        legit_payout = legitimate_reactions * gamma * decay_legit_avg
        sybil_payout = sybil_reactions * gamma * 1.0  # Sybil identity has 1 reaction → no decay
        # cap daily payout to 5% of pool to prevent farm draining
        max_daily = s.reaction_pool * 0.05
        total_payout = min(legit_payout + sybil_payout, max_daily)
        s.reaction_pool -= total_payout
        s.reaction_paid += total_payout
        s.total_issuance += total_payout
        s.total_minted += total_payout

    return s


def report(name: str, st, world: WorldParams, days: int):
    print(f"=== {name} (days={days}, end DAU={world.dau_curve[min(days-1, len(world.dau_curve)-1)]}) ===")
    print(f"  Total issuance:       {st.total_issuance:>16,.0f} MORAL")
    print(f"  Total minted:         {st.total_minted:>16,.0f} MORAL")
    print(f"  Total burned:         {st.total_burned:>16,.0f} MORAL")
    print(f"  Storage pool:         {st.storage_pool:>16,.0f} MORAL")
    print(f"  Reaction pool:        {st.reaction_pool:>16,.0f} MORAL")
    if hasattr(st, 'stealth_pool'):
        print(f"  Stealth pool:         {st.stealth_pool:>16,.0f} MORAL")
    print(f"  Miner revenue (5y):   {st.miner_revenue:>16,.0f} MORAL")
    print(f"  Storage paid (5y):    {st.storage_paid:>16,.0f} MORAL")
    print(f"  Reaction paid (5y):   {st.reaction_paid:>16,.0f} MORAL")
    if hasattr(st, 'base_fee'):
        print(f"  Final base_fee:       {st.base_fee:>16.6f} MORAL/byte")
        print(f"  Base-fee burned (5y): {st.base_fee_burned:>16,.0f} MORAL")
    inflation_5y = (st.total_minted - st.total_burned) / max(st.total_minted, 1)
    print(f"  Net supply Δ (mint−burn): {(st.total_minted - st.total_burned):>13,.0f} MORAL")
    print()


def linear_dau(start: int, end: int, days: int) -> List[int]:
    return [int(start + (end - start) * d / max(1, days - 1)) for d in range(days)]


print("=" * 78)
print("ANARCHY ECONOMIC MODEL SIMULATION v2")
print("Time horizon: 5 years × 365 days = 1825 days")
print("=" * 78)

DAYS = 1825

scenarios = [
    ("S1 organic 1k→100k DAU", WorldParams(dau_curve=linear_dau(1_000, 100_000, DAYS))),
    ("S2 stagnation 1k DAU", WorldParams(dau_curve=[1_000] * DAYS)),
    ("S3 spam 100k posts/day on 10k DAU", WorldParams(dau_curve=[10_000] * DAYS, spam_posts_per_day=100_000)),
    ("S4 sybil 1M reactors on 10k DAU", WorldParams(dau_curve=[10_000] * DAYS, sybil_reactors=1_000_000)),
    ("S5 small but loyal: 5k DAU flat", WorldParams(dau_curve=[5_000] * DAYS)),
]

results = {}
for name, world in scenarios:
    print(f"\n### {name} ###\n")
    m0 = simulate_m0(world, DAYS)
    m1 = simulate_m1(world, DAYS)
    results[name] = (m0, m1)
    report("M0 (current)", m0, world, DAYS)
    report("M1 (TSTS proposed)", m1, world, DAYS)

print("=" * 78)
print("KEY METRIC COMPARISON")
print("=" * 78)

print(f"\n{'Scenario':<40} {'M0 storage':>14} {'M1 storage':>14}")
for name, (m0, m1) in results.items():
    print(f"{name:<40} {m0.storage_pool:>14,.0f} {m1.storage_pool:>14,.0f}")

print(f"\n{'Scenario':<40} {'M0 reaction':>14} {'M1 reaction':>14}")
for name, (m0, m1) in results.items():
    print(f"{name:<40} {m0.reaction_pool:>14,.0f} {m1.reaction_pool:>14,.0f}")

print(f"\n{'Scenario':<40} {'M0 issuance':>16} {'M1 issuance':>16}")
for name, (m0, m1) in results.items():
    print(f"{name:<40} {m0.total_issuance:>16,.0f} {m1.total_issuance:>16,.0f}")

print(f"\n{'Scenario':<40} {'M0 miner':>14} {'M1 miner':>14}")
for name, (m0, m1) in results.items():
    print(f"{name:<40} {m0.miner_revenue:>14,.0f} {m1.miner_revenue:>14,.0f}")

print(f"\n{'Scenario':<40} {'M1 base_fee':>14} {'M1 fee burn':>16}")
for name, (m0, m1) in results.items():
    print(f"{name:<40} {m1.base_fee:>14.6f} {m1.base_fee_burned:>16,.0f}")

# Sybil ROI analysis (S4)
print()
print("=" * 78)
print("SYBIL ATTACK ECONOMICS (S4: 1M Sybil reactors vs 10k DAU)")
print("=" * 78)
m0_s4, m1_s4 = results["S4 sybil 1M reactors on 10k DAU"]
total_reactors_legit = 10_000
total_reactors_sybil = 1_000_000

# M0: each reaction = 1 MORAL. Sybil 1M reactions/day × 1825 = 1.825B; capped at pool
# But pool drains; sybils get most of it because they outnumber legits
# Crude: payout share = sybil_count / (sybil + legit*30)
m0_sybil_share = total_reactors_sybil / (total_reactors_sybil + total_reactors_legit * 30)
m0_sybil_take = m0_s4.reaction_paid * m0_sybil_share
print(f"\nM0 Sybil takes ~{m0_sybil_share*100:.1f}% of reaction payouts = {m0_sybil_take:,.0f} MORAL over 5y")

# M1: γ × decay. Sybil identity does 1 reaction (no decay benefit), legit does 30/day
# Sybil per-reaction reward = γ × 1
# Legit per-reaction reward = γ × decay_avg ≈ γ × 0.7 (avg of 1/sqrt(1+n/100) for n=0..29)
# Total Sybil reward share = (1M × 1) / (1M × 1 + 10k × 30 × 0.7) = 1M / (1M + 210k) ≈ 82.6%
# But total payout is capped at 5% pool/day, so Sybils get most of a smaller pie
# AND: PoW cost. Each Sybil reaction needs 16-bit PoW ≈ 0.7s CPU. 1M/day × 0.7s = 700k CPU-sec/day
m1_sybil_share = (total_reactors_sybil * 1.0) / (total_reactors_sybil * 1.0 + total_reactors_legit * 30 * 0.7)
m1_sybil_take = m1_s4.reaction_paid * m1_sybil_share
print(f"M1 Sybil takes ~{m1_sybil_share*100:.1f}% of reaction payouts = {m1_sybil_take:,.0f} MORAL over 5y")
print(f"   (cap: 5% pool/day prevents pool from draining beyond 95% per epoch)")

# Sybil PoW cost: 16-bit PoW = 65k hashes; @ 1MH/s typical CPU = 0.065s/reaction
sybil_pow_seconds = total_reactors_sybil * 0.065 * 365 * 5
sybil_cpu_years = sybil_pow_seconds / (365 * 24 * 3600)
print(f"\nSybil PoW cost: ~{sybil_cpu_years:.0f} CPU-years over 5y")
print(f"   M1 reward / CPU-year = {m1_sybil_take / max(sybil_cpu_years, 1):,.0f} MORAL")
print(f"   M0 reward / CPU-year = {m0_sybil_take / max(sybil_cpu_years, 1):,.0f} MORAL")
print(f"   Cloud CPU cost ~$300/yr × {sybil_cpu_years:.0f} years = ${sybil_cpu_years*300:,.0f}")
print(f"   Required token price for Sybil profitability:")
print(f"     M0: ${sybil_cpu_years*300/max(m0_sybil_take, 1):.4f}/MORAL")
print(f"     M1: ${sybil_cpu_years*300/max(m1_sybil_take, 1):.4f}/MORAL  ← higher = more secure")

print()
print("=" * 78)
print("SUMMARY: USER POSTING COST (M1, end of 5y)")
print("=" * 78)
for name, (m0, m1) in results.items():
    avg_bytes = 4 * 1024
    m1_post_cost = 50 + (0.0008 + m1.base_fee) * avg_bytes
    m0_post_cost = 100 + 0.001 * avg_bytes
    print(f"{name:<40}: M0=${m0_post_cost:>8.2f}  M1=${m1_post_cost:>10.2f} MORAL/4KB-post")

print()
print("=" * 78)
print("DONE")
print("=" * 78)
