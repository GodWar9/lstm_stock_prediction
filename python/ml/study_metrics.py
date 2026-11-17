"""Study metrics with explicit undefined values, separate from legacy demo metrics."""
import numpy as np
from scipy import stats


def correlation(prediction, target, rank=False):
    x, y = np.asarray(prediction), np.asarray(target)
    if len(x) < 3 or np.std(x) == 0 or np.std(y) == 0:
        return None
    return float((stats.spearmanr if rank else stats.pearsonr)(x, y).statistic)


def sharpe(returns, annualization=252):
    values = np.asarray(returns, dtype=float)
    if len(values) < 2 or np.std(values, ddof=1) == 0:
        return None
    return float(values.mean() / values.std(ddof=1) * np.sqrt(annualization))


def execution(prediction, next_open, next_close, cost_bps):
    position = (np.asarray(prediction) > 0).astype(float)
    ratio = np.asarray(next_close) / np.asarray(next_open)
    cost = cost_bps / 10000
    # Entry consumes capital including cost; exit pays cost on exit notional.
    returns = position * (ratio * (1-cost)/(1+cost) - 1)
    turnover = position * (1+ratio)
    return returns, turnover


def hac_mean_t(values, lag=5):
    x = np.asarray([v for v in values if v is not None], dtype=float)
    if len(x) <= lag + 2:
        return None
    centered = x - x.mean()
    variance = np.dot(centered, centered) / len(x)
    for shift in range(1, lag+1):
        variance += 2*(1-shift/(lag+1))*np.dot(centered[shift:], centered[:-shift])/len(x)
    return None if variance <= 0 else float(x.mean()/np.sqrt(variance/len(x)))


def bootstrap_indices(size, block, count, seed):
    if size < block or count < 1:
        raise ValueError('Insufficient observations for configured block bootstrap')
    rng = np.random.default_rng(seed)
    # Moving blocks without wraparound, truncated only at the end of each draw.
    starts = rng.integers(0, size-block+1, size=(count, int(np.ceil(size/block))))
    return (starts[..., None]+np.arange(block)).reshape(count, -1)[:, :size]


def summary(returns, turnover, prediction, target, bootstrap, annualization=252):
    values = np.asarray(returns, dtype=float)
    if not np.isfinite(values).all() or np.any(values <= -1):
        raise ValueError('Invalid simple returns')
    equity = np.r_[1., np.cumprod(1+values)]
    drawdown = float(np.min(equity/np.maximum.accumulate(equity)-1))
    draws = bootstrap_indices(len(values), bootstrap['block_length'], bootstrap['resamples'], bootstrap['seed'])
    estimates = [sharpe(values[indices], annualization) for indices in draws]
    finite = [v for v in estimates if v is not None]
    alpha = (1-bootstrap['confidence'])/2
    interval = list(map(float, np.quantile(finite, [alpha, 1-alpha]))) if finite else None
    target = np.asarray(target); prediction = np.asarray(prediction)
    valid = target != 0
    return {'ic': correlation(prediction, target), 'rank_ic': correlation(prediction, target, True),
            'hit_rate': float(np.mean(np.sign(prediction[valid]) == np.sign(target[valid]))) if valid.any() else None,
            'after_cost_sharpe': sharpe(values, annualization), 'turnover_mean': float(np.mean(turnover)),
            'max_drawdown': drawdown, 'sharpe_ci': interval,
            'bootstrap_defined_draws': len(finite), 'observations': len(values),
            'undefined_policy': 'Constant predictions or zero return variance yield null, not fabricated zero metrics.'}


def deflated_sharpe(returns, trial_count, variance_sr):
    """Bailey/Lopez de Prado approximation; daily SR and Pearson kurtosis.

    Counts all ledger attempts conservatively; correlated trials and serially
    dependent returns mean this is an assumption-dependent diagnostic.
    """
    x = np.asarray(returns, dtype=float)
    sr = sharpe(x, 1)
    if sr is None or len(x) < 4 or trial_count < 2 or variance_sr is None or variance_sr <= 0:
        return {'value': None, 'reason': 'Insufficient nonconstant trials/returns for DSR', 'trial_count': trial_count}
    skew = float(stats.skew(x, bias=False))
    kurtosis = float(stats.kurtosis(x, fisher=False, bias=False))
    gamma = np.euler_gamma
    benchmark = np.sqrt(variance_sr)*((1-gamma)*stats.norm.ppf(1-1/trial_count)
                                    + gamma*stats.norm.ppf(1-1/(trial_count*np.e)))
    denominator = 1-skew*sr+(kurtosis-1)*sr*sr/4
    value = float(stats.norm.cdf((sr-benchmark)*np.sqrt(len(x)-1)/np.sqrt(denominator))) if denominator > 0 else None
    return {'value': value, 'trial_count': trial_count, 'daily_sr': sr, 'variance_sr': variance_sr,
            'skewness': skew, 'pearson_kurtosis': kurtosis, 'benchmark_daily_sr': float(benchmark),
            'caveat': 'Approximation assumes independent trials and suitable sampling; raw attempt count is conservative, not an estimated effective independent count.'}
