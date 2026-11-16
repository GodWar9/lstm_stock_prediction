"""Serial, local mutation acceptance. Restores exact bytes after every mutation.

Run only with no concurrent tests/builds or source edits. Never commit mutations.
The shuffle mutation removes the negative-control intervention, not a production
training feature. All fixtures are synthetic and establish no investment result.
"""
import hashlib
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PYTEST = [sys.executable, '-m', 'pytest', '-v', '-s']
TEST = 'python/tests/test_validity_canaries.py::'
MUTATIONS = [
    ('future-feature', 'rust/data/src/validate.rs',
     [('if feature_availability > target_observation_start {',
       'if false && feature_availability > target_observation_start {')],
     ['cargo', 'test', '--manifest-path', 'rust/Cargo.toml', '-p', 'quant_data',
      '--locked', 'lookahead_feature_canary']),
    ('purge-embargo', 'python/ml/dataset.py',
     [('if right[0] - left[-1] - 1 < purge_bars + embargo_bars:',
       'if False and right[0] - left[-1] - 1 < purge_bars + embargo_bars:')],
     PYTEST + [TEST + 'test_orchestrator_rejects_purge_embargo_violation']),
    ('train-only-scaler', 'python/ml/train_orchestrator.py',
     [('scaler.fit_transform(raw_features[train_idx])', 'scaler.fit_transform(raw_features)')],
     PYTEST + [TEST + 'test_orchestrator_scaler_sees_only_train_fold']),
    ('shuffled-label-control', 'python/tests/test_validity_canaries.py',
     [('train_y = rng.permutation(target[:1024])', 'train_y = target[:1024].copy()'),
      ('test_y = rng.permutation(target[1024:])', 'test_y = target[1024:].copy()')],
     PYTEST + [TEST + 'test_shuffled_labels_have_no_heldout_ic']),
]


def main():
    for name, relative, replacements, command in MUTATIONS:
        path = ROOT / relative
        original = path.read_bytes()
        mutated = original
        for before, after in replacements:
            before, after = before.encode(), after.encode()
            if mutated.count(before) != 1:
                raise RuntimeError(f'{name}: expected exactly one mutation site')
            mutated = mutated.replace(before, after)
        print(f'\nMUTATION {name}: {relative}\nCOMMAND {command}', flush=True)
        print(f'ORIGINAL_SHA256 {hashlib.sha256(original).hexdigest()}', flush=True)
        try:
            path.write_bytes(mutated)
            result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, text=True, encoding='utf-8',
                                    errors='replace', timeout=300)
            print(result.stdout, flush=True)
            # Reject compiler/import/tool failures as mutation evidence.
            if result.returncode == 0 or 'FAILED' not in result.stdout or 'AssertionError' not in result.stdout and 'panicked at' not in result.stdout and 'Invalid fold reached scaler fitting' not in result.stdout:
                raise RuntimeError(f'{name}: no expected assertion failure observed')
            print(f'MUTATION DETECTED exit={result.returncode}', flush=True)
        finally:
            path.write_bytes(original)
            if path.read_bytes() != original:
                raise RuntimeError(f'{name}: source restoration failed')
            print(f'RESTORED {relative}', flush=True)


if __name__ == '__main__':
    main()
