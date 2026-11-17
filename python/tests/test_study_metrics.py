"""Hand-calculated synthetic controls, never reported as strategy performance."""
import numpy as np
from python.ml.study_metrics import correlation, execution, sharpe, bootstrap_indices, deflated_sharpe


def test_undefined_metrics_are_null_and_no_same_close_execution():
    assert correlation([0,0,0],[1,2,3]) is None
    assert sharpe([0,0,0]) is None
    returns, turnover = execution([1,-1], [100,100], [110,90], 10)
    np.testing.assert_allclose(returns, [1.1*.999/1.001-1,0])
    np.testing.assert_allclose(turnover,[2.1,0])


def test_bootstrap_is_repeatable_and_bounded():
    a=bootstrap_indices(40,5,20,7)
    np.testing.assert_array_equal(a,bootstrap_indices(40,5,20,7))
    assert a.shape == (20,40) and a.min() >= 0 and a.max() < 40


def test_more_trials_reduce_dsr_without_changing_returns():
    values=np.array([-.02,.03,.01,-.01,.02,.005]*20)
    low=deflated_sharpe(values,5,.01)
    high=deflated_sharpe(values,500,.01)
    assert 0 <= high['value'] < low['value'] <= 1
    assert deflated_sharpe(np.zeros(20),5,.01)['value'] is None
