"""Hand-translated candidate VCs. No parsing/elaboration of .lc happens here.

Each check asks whether the stated premises AND the negated obligation have a
model. UNSAT proves only this SMT formula; SAT records a counterexample. This is
not a Lean certificate, a proof of Rust memory safety, or a Locus acceptance test.
"""
import json
from pathlib import Path
from z3 import *

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'results' / 'vcs'
OUT.mkdir(parents=True, exist_ok=True)
results = []

def check(name, premises, goal, expected, explain, witnesses=()):
    solver = Solver()
    solver.set(timeout=10000)
    solver.add(*premises, Not(goal))
    (OUT / (name + '.smt2')).write_text(solver.to_smt2())
    actual = str(solver.check())
    record = dict(name=name, expected=expected, actual=actual,
                  interpretation=explain, passed=actual == expected)
    if actual == 'sat':
        model = solver.model()
        record['witness'] = {str(x): str(model.eval(x, model_completion=True)) for x in witnesses}
        record['model'] = str(model)
    if actual == 'unknown':
        record['reason'] = solver.reason_unknown()
    results.append(record)
    print(f'{name}: {actual} (expected {expected})')

cap, n, i, j = Ints('capacity length index probe')
init = Array('entry_initialized', IntSort(), BoolSort())
contents = Array('entry_contents', IntSort(), IntSort())
final_contents = Array('final_contents', IntSort(), IntSort())
value, after_slot = Ints('arbitrary_final_element slot_at_acquisition_return')
new_init = Array('final_initialized', IntSort(), BoolSort())

def prefix(a, length):
    return And(0 <= length, length <= cap,
               ForAll([j], Implies(And(0 <= j, j < cap), a[j] == (j < length))))

entry = [prefix(init,n), 0 <= i, i < n]
element_relation = [new_init == Store(init,i,True), final_contents == Store(contents,i,value)]

check('vc01_prefix_arbitrary_element', entry + element_relation,
      prefix(new_init,n), 'unsat',
      'Any final element value preserves initialized-prefix shape, for arbitrary capacity and length.')
check('vc02_capacity_preserved', entry + element_relation,
      And(0 <= n, n <= cap), 'unsat', 'Shape borrowing does not change capacity or length.')
check('vc03_frame_outside_element', entry + element_relation + [j != i],
      final_contents[j] == contents[j], 'unsat', 'Loan relation frames every different index.')

cleared = Store(init,i,False)
check('vc04_lending_optional_slot_is_too_strong', entry,
      prefix(cleared,n), 'sat', 'Giving out &mut Option<T> permits deinitialization.', [cap,n,i])

a,b,x = Ints('entry_left entry_right final_left')
check('vc05_sorted_arbitrary_element', [a <= b], x <= b, 'sat',
      'SortedVec cannot give out arbitrary &mut element with no pledge.', [a,b,x])
def sorted_array(arr):
    return ForAll([j], Implies(And(0 <= j, j+1 < n), arr[j] <= arr[j+1]))
check('vc06_sorted_with_constrained_final',
      [sorted_array(contents), 0 <= i, i < n,
       Implies(i > 0, contents[i-1] <= value),
       Implies(i+1 < n, value <= contents[i+1])],
      sorted_array(Store(contents,i,value)), 'unsat',
      'Neighbor bounds on the final value preserve arbitrary-length sortedness; an unrestricted loan supplies no such bounds.')

check('vc07_no_circular_invariant_assumption', entry + [new_init == cleared],
      prefix(new_init,n), 'sat', 'Entry proof alone cannot certify a clearing mutation.', [cap,n,i])
check('vc08_circular_premise_vacuously_hides_bug', entry + [new_init == cleared, prefix(new_init,n)],
      prefix(new_init,n), 'unsat', 'Negative control: assuming the suspended invariant hides the error.')

normal = Bool('normal_return')
check('vc09_normal_contract_does_not_prove_unwind_safety',
      entry + [Implies(normal, new_init == init)], prefix(new_init,n), 'sat',
      'Normal-return evidence supplies no fact on the unwind edge.', [normal,cap,n,i,new_init[i]])
check('vc10_initialized_does_not_imply_no_panic', entry,
      normal, 'sat', 'An initialized input is compatible with a panicking callee.', [normal,cap,n,i])

y = Int('final_right')
check('vc11_enclosing_invariant', [0 <= a, 0 <= b, a+b <= 10, 0 <= x],
      x+b <= 10, 'sat', 'Preserving the child positivity invariant can break its parent bound.', [a,b,x])
check('vc12_two_loans_joint_obligation', [a==0,b==0,x+b<=10,a+y<=10],
      x+y<=10, 'sat', 'Two separately valid entry-based checks do not imply joint restoration.', [x,y])

inner_end, outer_end = Ints('inner_end outer_end')
check('vc13_distinct_reborrow_final_values', [inner_end==1, outer_end==2],
      inner_end==outer_end, 'sat', 'The two loan IDs cannot share one final-value symbol.', [inner_end,outer_end])

check('vc14_current_element_is_not_final_element', entry + element_relation + [after_slot != value],
      final_contents[i] == after_slot, 'sat',
      'An acquisition-return snapshot cannot replace the returned reference final term.', [after_slot,value])

future = Int('future_data')
check('vc15_future_data_liar', [Or(future==0,future==1), future==1-future],
      False, 'unsat', 'Negative control: allowing future-dependent writes admits an impossible fixed point.')

# A finite ghost-cache model of the wrapped Vec problem, separate from I01.
ghost, memory = Ints('cached_element memory_at_loan_end')
check('vc16_cache_needs_actual_loan_end_update', [ghost==after_slot, memory==value],
      ghost==memory, 'sat', 'Native in-place shape preservation does not update a separate wrapper cache.',
      [ghost,memory])

new_n = Int('changed_sibling_length')
check('vc17_other_proof_dependencies_must_stay_fixed', entry + element_relation,
      prefix(new_init,new_n), 'sat', 'Freezing proof reads does not alone freeze other dependencies such as length.',
      [cap,n,new_n])

(ROOT/'results'/'vcs.json').write_text(json.dumps(results, indent=2)+'\n')
assert all(r['passed'] for r in results), 'Unexpected SMT result; see results/vcs.json'
print(f'VC SUMMARY: {len(results)} expected outcomes; '+
      f"{sum(r['actual']=='unsat' for r in results)} unsat, "+
      f"{sum(r['actual']=='sat' for r in results)} sat")
