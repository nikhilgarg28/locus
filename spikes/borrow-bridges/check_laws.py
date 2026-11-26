#!/usr/bin/env python3
"""Small hand-written VCs, NOT translation of Locus and NOT kernel proofs.

Unsat proves only the implication encoded by that query. Sat supplies a model
of weakened/missing assumptions. Each VC uses arbitrary symbolic observations;
the local law instances used in a derivation are supplied explicitly.
"""
import json
from pathlib import Path
import z3 as z

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "results"
OUT.mkdir(exist_ok=True)
(OUT / "vcs").mkdir(exist_ok=True)
K, Q, Trace = z.DeclareSort("K"), z.DeclareSort("Q"), z.DeclareSort("Trace")
k, l = z.Consts("k l", K)
q = z.Const("q", Q)
p = z.Function("projection", K, Q)
actual = z.Function("actual_borrow", K, Q)
ek = z.Function("key_equiv", K, K, z.BoolSort())
eq = z.Function("query_equiv", Q, Q, z.BoolSort())
hk, hq = z.Function("key_trace", K, Trace), z.Function("query_trace", Q, Trace)
ck = z.Function("key_cmp", K, K, z.IntSort())
cq = z.Function("query_cmp", Q, Q, z.IntSort())
results = []


def check(name, assumptions, denied_goal, expected):
    solver = z.Solver()
    solver.set(timeout=10000)
    solver.add(*assumptions, denied_goal)
    (OUT / "vcs" / f"{name}.smt2").write_text(solver.to_smt2())
    outcome = str(solver.check())
    result = {"name": name, "expected": expected, "actual": outcome,
              "passed": outcome == expected}
    if outcome == "sat":
        result["model"] = str(solver.model())
    results.append(result)
    print(f"{name}: {outcome} (expected {expected})")
    if outcome != expected:
        raise RuntimeError(result)


# Matching an arbitrary valid query, not just a query syntactically p(other).
matching = eq(p(k), q)
trace_bridge = hk(k) == hq(p(k))
query_hash_law = z.Implies(matching, hq(p(k)) == hq(q))
check("01_matching_query_equal_trace",
      [matching, trace_bridge, query_hash_law], hk(k) != hq(q), "unsat")
check("02_missing_borrow_hash",
      [matching, query_hash_law], hk(k) != hq(q), "sat")
check("03_missing_query_equiv_hash",
      [matching, trace_bridge], hk(k) != hq(q), "sat")

# Two matched stored keys must be the same class, so map lookup is unambiguous.
both_match = [eq(p(k), q), eq(p(l), q)]
symmetry_instance = z.Implies(eq(p(l), q), eq(q, p(l)))
transitivity_instance = z.Implies(z.And(eq(p(k), q), eq(q, p(l))), eq(p(k), p(l)))
reflection_instance = z.Implies(eq(p(k), p(l)), ek(k, l))
check("04_matching_keys_one_class", both_match +
      [symmetry_instance, transitivity_instance, reflection_instance],
      z.Not(ek(k, l)), "unsat")
check("05_missing_reflection", both_match +
      [symmetry_instance, transitivity_instance], z.Not(ek(k, l)), "sat")

# Replacing a key with an equivalent representative preserves all query matches.
preservation = z.Implies(ek(k, l), eq(p(k), p(l)))
swap_symmetry = z.Implies(eq(p(k), p(l)), eq(p(l), p(k)))
swap_transitivity = z.Implies(z.And(eq(p(l), p(k)), eq(p(k), q)), eq(p(l), q))
check("06_representative_independent_match",
      [ek(k, l), matching, preservation, swap_symmetry, swap_transitivity],
      z.Not(eq(p(l), q)), "unsat")
check("07_missing_preservation", [ek(k, l), matching],
      z.Not(eq(p(l), q)), "sat")

# Correctness of the independent operation does not constrain native Borrow.
check("08_unbound_native_borrow",
      [matching, trace_bridge, query_hash_law], actual(k) != p(k), "sat")
check("09_bound_native_borrow",
      [actual(k) == p(k), matching], z.Not(eq(actual(k), q)), "unsat")

# Checking only comparison==Equal does not preserve branch direction.
valid_cmp = [ck(k, l) >= -1, ck(k, l) <= 1,
             cq(p(k), p(l)) >= -1, cq(p(k), p(l)) <= 1]
check("10_equal_only_order_is_insufficient", valid_cmp +
      [(ck(k, l) == 0) == (cq(p(k), p(l)) == 0)],
      ck(k, l) != cq(p(k), p(l)), "sat")
check("11_full_order_preserves_branch", valid_cmp +
      [ck(k, l) == cq(p(k), p(l))],
      (ck(k, l) < 0) != (cq(p(k), p(l)) < 0), "unsat")

# Validity must transport through projection before applying query-side laws.
vk = z.Function("valid_key", K, z.BoolSort())
vq = z.Function("valid_query", Q, z.BoolSort())
guarded_hash = z.Implies(z.And(vq(p(k)), vq(q), matching), hq(p(k)) == hq(q))
check("12_missing_projection_validity",
      [vk(k), vq(q), matching, trace_bridge, guarded_hash], hk(k) != hq(q), "sat")
check("13_with_projection_validity",
      [vk(k), vq(q), matching, trace_bridge, guarded_hash,
       z.Implies(vk(k), vq(p(k)))], hk(k) != hq(q), "unsat")

# Trace equality gives digest equality only with the compatible state/interpreter.
digest = z.Function("digest", z.IntSort(), Trace, z.IntSort())
s1, s2 = z.Ints("state1 state2")
check("14_equal_trace_different_builder_state", [hk(k) == hq(q)],
      digest(s1, hk(k)) != digest(s2, hq(q)), "sat")
check("15_equal_trace_same_builder_state", [hk(k) == hq(q), s1 == s2],
      digest(s1, hk(k)) != digest(s2, hq(q)), "unsat")

# A key left of the pivot cannot match a query right of the pivot. The query
# order's own transitivity is essential, in addition to cross-type alignment.
left_partition = [ck(k, l) < 0, ck(k, l) == cq(p(k), p(l)), cq(p(l), q) < 0]
query_order_transitivity = z.Implies(
    z.And(cq(p(k), p(l)) < 0, cq(p(l), q) < 0), cq(p(k), q) < 0)
query_equal_agreement = eq(p(k), q) == (cq(p(k), q) == 0)
check("16_tree_pruning_arbitrary_query", left_partition +
      [query_order_transitivity, query_equal_agreement], eq(p(k), q), "unsat")
check("17_missing_query_order_transitivity", left_partition + [query_equal_agreement],
      eq(p(k), q), "sat")

(OUT / "laws.json").write_text(json.dumps({"z3": z.get_version_string(),
                                          "queries": results}, indent=2) + "\n")
print(f"LAW QUERIES: {len(results)} matched; not Locus/kernel proofs")
