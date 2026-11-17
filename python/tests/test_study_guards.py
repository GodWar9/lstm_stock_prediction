"""Offline safety tests; no provider access or reported strategy performance."""
import hashlib
import json
from types import SimpleNamespace

import numpy as np
import pytest

from scripts.run_daily_study import approval, reserve_holdout, folds, report


def test_review_hash_and_one_use_holdout(tmp_path):
    config = tmp_path / 'study.yaml'
    config.write_text('frozen: true\n')
    digest = hashlib.sha256(config.read_bytes()).hexdigest()
    with pytest.raises(ValueError, match='Human review'):
        approval(config, None)
    assert approval(config, digest) == digest
    config.write_text('frozen: changed\n')
    with pytest.raises(ValueError, match='Human review'):
        approval(config, digest)
    with pytest.raises(ValueError, match='Separate'):
        reserve_holdout(tmp_path, digest, None)
    reserve_holdout(tmp_path, digest, digest)
    with pytest.raises(FileExistsError):
        reserve_holdout(tmp_path, digest, digest)


def test_fold_boundaries_exclude_future_labels():
    timestamps = np.arange(np.datetime64('2018-01-01'), np.datetime64('2022-01-01')).astype('datetime64[ns]').astype(np.int64)
    dataset = SimpleNamespace(timestamps=timestamps, target_timestamps=timestamps+86_400_000_000_000)
    config = {'folds': {'purge_bars':5,'embargo_bars':60}, 'features':{'lookback':60}, 'metrics':{'bootstrap':{'block_length':20}}}
    train, val, test = folds(dataset, {'train_end':'2019-01-01','validation_end':'2020-01-01','test_end':'2021-01-01'}, config)
    assert val[0]-train[-1] > 65 and test[0]-val[-1] > 65
    assert dataset.target_timestamps[test[-1]] < np.datetime64('2021-01-01','ns').astype(np.int64)


def test_report_uses_saved_values(tmp_path):
    metrics = dict(after_cost_sharpe=None,sharpe_ci=None,max_drawdown=-0.123456,turnover_mean=1.234567,hit_rate=0.456789)
    candidate = dict(metrics=metrics,ic_mean=None,rank_ic_mean=None,ic_hac_t=None,rank_ic_hac_t=None,dsr={'value':None})
    result = dict(stage='test fixture',config_sha256='fixture',conclusion='Synthetic test only',candidates={'zero':candidate},lstm_seed_summary={},paired_comparisons={},results_path='fixture.json')
    path = tmp_path/'report.md'
    report(json.loads(json.dumps(result)),path)
    text = path.read_text()
    for value in metrics.values():
        assert json.dumps(value) in text
