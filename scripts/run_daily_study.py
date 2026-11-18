"""Frozen real-data study. No acquisition/fitting without a matching approval hash.

Final holdout has a separate one-use marker. Failed trials are retained and never
silently retried. Copying/deleting the local ledger defeats local reuse guards.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import numpy as np
import yaml
from python.ml.study_metrics import correlation, execution, summary, sharpe, hac_mean_t, deflated_sharpe, bootstrap_indices


def write_json(path, value):
    with path.open('x', encoding='utf-8') as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.flush(); os.fsync(stream.fileno())


def approval(path, approved):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if approved != digest:
        raise ValueError(f'Human review required. Supply the approved configuration SHA256: {digest}')
    return digest


def reserve_holdout(root, digest, signoff):
    if signoff != digest:
        raise ValueError('Separate final holdout sign-off required')
    write_json(root / 'holdout.started.json', {'config_sha256': digest, 'policy': 'one evaluation invocation; failure retains marker'})


def folds(dataset, boundaries, cfg):
    ts = dataset.timestamps
    cuts = [int(np.searchsorted(ts, np.datetime64(boundaries[key], 'ns').astype(np.int64)))
            for key in ('train_end','validation_end','test_end')]
    p, e = cfg['folds']['purge_bars'], cfg['folds']['embargo_bars']
    indices = (np.arange(max(0,cuts[0]-p)), np.arange(cuts[0]+e,max(cuts[0]+e,cuts[1]-p)), np.arange(cuts[1]+e,cuts[2]))
    from python.ml.dataset import validate_split_indices
    validate_split_indices(indices,len(ts),p,e)
    if any(len(i) < cfg['features']['lookback']+cfg['metrics']['bootstrap']['block_length'] for i in indices):
        raise ValueError('Insufficient fold coverage; no symbol/fold substitution allowed')
    if dataset.target_timestamps[indices[0][-1]] >= ts[indices[1][0]] or dataset.target_timestamps[indices[1][-1]] >= ts[indices[2][0]]:
        raise ValueError('Target overlaps a later split')
    if dataset.target_timestamps[indices[2][-1]] >= np.datetime64(boundaries['test_end'],'ns').astype(np.int64):
        indices = (*indices[:2], indices[2][dataset.target_timestamps[indices[2]] < np.datetime64(boundaries['test_end'],'ns').astype(np.int64)])
    if len(indices[2]) < cfg['features']['lookback']+cfg['metrics']['bootstrap']['block_length']:
        raise ValueError('Insufficient evaluation coverage after target boundary filtering')
    return indices


def ledger(path, event):
    with path.open('a', encoding='utf-8') as out:
        out.write(json.dumps(event,allow_nan=False)+'\n'); out.flush(); os.fsync(out.fileno())


def snapshot(symbol, cfg, digest, folder, binary):
    version=f'study_{digest[:12]}_{symbol}'
    config=yaml.safe_load((ROOT/'configs/default.yaml').read_text())
    config['data'].update(symbols=[symbol],provider=cfg['data']['provider'],start_date=cfg['data']['start_date'],
                          end_date=cfg['data']['end_date_exclusive'],dataset_version=version)
    config['features'].update(lookback=cfg['features']['lookback'],target_horizon=cfg['features']['target_horizon'])
    config['training'].update(purge_gap=cfg['folds']['purge_bars'],embargo_gap=cfg['folds']['embargo_bars'])
    config_path=folder/f'{symbol}.yaml'
    config_path.write_text(yaml.safe_dump(config),encoding='utf-8')
    market=ROOT/f'datasets/market/{version}/{symbol}.json'
    train=ROOT/f'datasets/training/{version}/{symbol}.arrow'
    if (folder/f'{symbol}.snapshot.json').exists() and (not market.exists() or not train.exists()):
        raise ValueError('Frozen snapshot missing; restore original bytes rather than reacquire')
    env=dict(os.environ,QUANTCTL_ALLOW_NETWORK='1',QUANTCTL_PYTHON=sys.executable)
    for command, needed in [(['data','ingest'],not market.exists()),(['data','validate'],True),(['features','build'],not train.exists())]:
        if needed:
            args=[str(binary),'--config',str(config_path),*command]
            print('COMMAND',args,flush=True)
            subprocess.run(args,cwd=ROOT,env=env,check=True)
    from python.ml.arrow_dataset import load_arrow_training_dataset
    data=load_arrow_training_dataset(train,train.with_suffix('.manifest.json'))
    if hashlib.sha256(market.read_bytes()).hexdigest() != data.integrity['market_sha256']:
        raise ValueError('Training features do not match the frozen market snapshot')
    source=data.integrity['source'].lower()
    if not source or source=='unknown' or any(t in source for t in ('synthetic','fixture','mock')):
        raise ValueError('Study requires real data; refusing substitute provenance')
    identity={'symbol':symbol,'config_sha256':digest,'training_sha256':data.integrity['training_sha256'],
              'market_sha256':data.integrity['market_sha256'],'source':source}
    lock=folder/f'{symbol}.snapshot.json'
    if lock.exists():
        if json.loads(lock.read_text()) != identity: raise ValueError('Frozen data snapshot changed')
    else: write_json(lock,identity)
    return data,config,json.loads(market.read_text())


def report(result, destination):
    rows=['# Real data baseline study','',f"Stage: {result['stage']}. Configuration SHA256: `{result['config_sha256']}`.",'',
          result['conclusion'],'','This is a fixed present-day stock sample with survivorship bias, provider revisions and unverified corporate-action treatment. Simulated next-open fills are not executable quotes. No alpha claim follows from development folds.','',
          '| Candidate | After cost Sharpe | Sharpe interval | Drawdown | Turnover | IC mean | Rank IC mean | IC HAC t | Rank IC HAC t | Hit rate | DSR |',
          '|---|---|---|---|---|---|---|---|---|---|---|']
    for name,item in result['candidates'].items():
        m=item['metrics']
        values=[name,m['after_cost_sharpe'],m['sharpe_ci'],m['max_drawdown'],m['turnover_mean'],
                item['ic_mean'],item['rank_ic_mean'],item['ic_hac_t'],item['rank_ic_hac_t'],m['hit_rate'],item['dsr']['value']]
        rows.append('| '+' | '.join(json.dumps(v,allow_nan=False) if not isinstance(v,str) else v for v in values)+' |')
    rows += ['', 'LSTM seed summaries and paired comparisons are retained verbatim in results JSON:', '',
             '```json',json.dumps({'lstm_seed_summary':result['lstm_seed_summary'],'paired_comparisons':result['paired_comparisons']},indent=2,allow_nan=False),'```','',
             'Undefined statistics remain null. DSR uses all logged attempts as a conservative count; independence assumptions are not certified. PBO is unavailable: CSCV is not implemented in this walk-forward study.','',
             '[Deflated Sharpe method](https://davidhbailey.com/dhbpapers/deflated-sharpe.pdf).', '',
             f"Results source: `{result['results_path']}`. The report is generated from that JSON, not manually transcribed."]
    if result.get('plot_path'):
        published = Path('evidence/phase5')/f"{result['config_sha256']}-{result['stage']}-equity.png"
        image_path = published.as_posix() if (ROOT/'Docs'/published).exists() else '../'+result['plot_path']
        rows += ['', f"![Simulated equity]({image_path})"]
    published_results = Path('evidence/phase5')/f"{result['config_sha256']}-{result['stage']}-results.json"
    if (ROOT/'Docs'/published_results).exists():
        rows += ['', f"[Retained results JSON]({published_results.as_posix()})."]
    rows += ['', '## Reproduce the saved report', '',
             'Install the locked study dependencies with `uv sync --locked --group study` from `python/`. From the repository root:', '',
             '```powershell',
             f"python/.venv/Scripts/python.exe scripts/run_daily_study.py --approved-sha256 {result['config_sha256']} --stage report",
             '```', '',
             'This reads saved results without acquisition or fitting. The original authorized development command uses `--stage development`. A completed or partial stage refuses automatic retrial. Final holdout requires a separate human sign-off and `--stage holdout --holdout-signoff REVIEWED_SHA256`; do not remove its one-use marker.', '',
             f"Trial ledger, predictions, snapshot hashes and plot: `reports/studies/{result['config_sha256']}/`. Retain these local artifacts together with the evidence log; the source data and trained packages are not automatically published to Git."]
    destination.write_text('\n'.join(rows)+'\n',encoding='utf-8')


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--config',type=Path,default=ROOT/'configs/study_daily.yaml')
    parser.add_argument('--approved-sha256')
    parser.add_argument('--stage',choices=['development','holdout','report'],default='development')
    parser.add_argument('--holdout-signoff')
    args=parser.parse_args()
    digest=approval(args.config,args.approved_sha256)
    cfg=yaml.safe_load(args.config.read_text())
    folder=ROOT/'reports/studies'/digest
    folder.mkdir(parents=True,exist_ok=True)
    if args.stage=='report':
        path=folder/('holdout-results.json' if (folder/'holdout-results.json').exists() else 'development-results.json')
        report(json.loads(path.read_text()),ROOT/'Docs/STUDY-REPORT.md'); return
    if cfg['protocol_version'] != 1 or cfg['data']['provider'] != 'yfinance' or cfg['features']['target_horizon'] != 1:
        raise ValueError('Unsupported protocol; implementation must be reviewed before changing it')
    os.environ['CUDA_VISIBLE_DEVICES']=''
    import torch
    from sklearn.ensemble import HistGradientBoostingRegressor
    from sklearn.linear_model import Ridge
    from python.ml.dataset import FeatureScaler,create_sliding_windows
    from python.ml.publication import staged_model
    from python.ml.train_orchestrator import train_and_export
    os.chdir(ROOT)
    torch.set_num_threads(1); torch.use_deterministic_algorithms(True)
    binary=ROOT/('rust/target/debug/quantctl.exe' if os.name=='nt' else 'rust/target/debug/quantctl')
    os.environ['QUANTCTL_EXECUTABLE']=str(binary)
    result_path=folder/f'{args.stage}-results.json'
    if result_path.exists(): raise ValueError('Stage already completed; use --stage report to reproduce its report')
    if args.stage=='holdout':
        if not (folder/'development-results.json').exists(): raise ValueError('Finish development before final holdout sign-off')
        reserve_holdout(folder,digest,args.holdout_signoff)
    ledger_path=folder/'trials.ndjson'
    old=[json.loads(line) for line in ledger_path.read_text().splitlines()] if ledger_path.exists() else []
    if any(row.get('stage')==args.stage for row in old): raise ValueError('Partial/failed stage retained; no automatic retrial')
    lock=folder/'running.lock'
    with lock.open('x') as out: out.write(str(os.getpid()))
    trials=[]
    try:
        schedule=cfg['folds']['development'] if args.stage=='development' else [cfg['folds']['final_holdout']]
        if len(cfg['models'])>cfg['trials']['max_configurations']: raise ValueError('Configuration cap exceeded')
        attempts=sum(row.get('event')=='started' for row in old)
        for symbol in cfg['data']['symbols']:
            data,base,bars=snapshot(symbol,cfg,digest,folder,binary)
            market={int(bar['timestamp']):bar for bar in bars}
            for fold,boundaries in enumerate(schedule):
                indices=folds(data,boundaries,cfg); train,val,test=indices
                lookback=cfg['features']['lookback']
                scaler=FeatureScaler().fit(data.features[train])
                xtrain,ytrain=create_sliding_windows(scaler.transform(data.features[train]),data.targets[train],lookback)
                xtest,ytest=create_sliding_windows(scaler.transform(data.features[test]),data.targets[test],lookback)
                selected=test[lookback-1:]
                timestamps=data.target_timestamps[selected]
                opens=np.array([market[int(t)]['open'] for t in timestamps]); closes=np.array([market[int(t)]['close'] for t in timestamps])
                for model,parameters in cfg['models'].items():
                    for seed in cfg['seeds'] if model=='lstm' else [42]:
                        attempts+=1
                        if attempts>cfg['trials']['max_attempts']: raise ValueError('Trial cap exceeded')
                        trial=dict(stage=args.stage,symbol=symbol,fold=fold,model=model,seed=seed,attempt=attempts)
                        ledger(ledger_path,dict(trial,event='started'))
                        try:
                            if model=='zero': prediction=np.zeros(len(ytest))
                            elif model=='momentum':
                                feature=data.feature_names.index('log_return'); width=parameters['trailing_bars']
                                prediction=np.array([data.features[i-width+1:i+1,feature].mean() for i in selected])
                            elif model in ('ridge','gbm'):
                                estimator=Ridge(**parameters) if model=='ridge' else HistGradientBoostingRegressor(**parameters)
                                estimator.fit(xtrain.reshape(len(xtrain),-1),ytrain)
                                prediction=estimator.predict(xtest.reshape(len(xtest),-1))
                            elif model=='lstm':
                                local=copy.deepcopy(base); local['training'].update(parameters,random_seed=seed)
                                model_id=f'study_{digest[:12]}_{symbol}_{args.stage}_{fold}_{seed}'
                                train_path=ROOT/f"datasets/training/{base['data']['dataset_version']}/{symbol}.arrow"
                                train_args=SimpleNamespace(dataset=str(train_path),manifest=str(train_path.with_suffix('.manifest.json')),synthetic=False,walk_forward=False,_indices=indices)
                                with staged_model(ROOT/'models',model_id) as stage:
                                    train_and_export(train_args,local,model_id,stage)
                                saved=json.loads((ROOT/'models'/model_id/'oos_predictions.json').read_text())
                                prediction=np.asarray(saved['predictions']).ravel()
                                np.testing.assert_allclose(saved['targets'],ytest,rtol=0,atol=0)
                                trial['model_id']=model_id
                            else:
                                raise ValueError(f'Unsupported model: {model}')
                            if len(prediction)!=len(ytest) or not np.isfinite(prediction).all(): raise ValueError('Invalid predictions')
                            cost=sum(cfg['execution'][key] for key in ('commission_bps_per_side','half_spread_bps_per_side','slippage_bps_per_side'))
                            returns,turnover=execution(prediction,opens,closes,cost)
                            trial.update(timestamps=timestamps.tolist(),prediction=prediction.tolist(),target=ytest.tolist(),returns=returns.tolist(),turnover=turnover.tolist())
                            write_json(folder/f"trial-{attempts}.json",trial)
                            ledger(ledger_path,dict(event='completed',stage=args.stage,attempt=attempts,result=f'trial-{attempts}.json'))
                            trials.append(trial)
                        except Exception as error:
                            ledger(ledger_path,dict(event='failed',stage=args.stage,attempt=attempts,error=str(error))); raise
        candidates={}; paths={}
        for model in cfg['models']:
            for seed in cfg['seeds'] if model=='lstm' else [42]:
                chosen=[t for t in trials if t['model']==model and t['seed']==seed]
                by_symbol={symbol:{} for symbol in cfg['data']['symbols']}
                for trial in chosen:
                    for i,t in enumerate(trial['timestamps']):
                        if t in by_symbol[trial['symbol']]: raise ValueError('Overlapping evaluation folds')
                        by_symbol[trial['symbol']][t]=tuple(trial[k][i] for k in ('prediction','target','returns','turnover'))
                dates=sorted(set.intersection(*(set(rows) for rows in by_symbol.values())))
                if len(dates) < cfg['metrics']['bootstrap']['block_length']:
                    raise ValueError('Insufficient common evaluation dates')
                panel=np.array([[by_symbol[s][t] for s in cfg['data']['symbols']] for t in dates])
                returns=panel[:,:,2].mean(axis=1)
                item=summary(returns,panel[:,:,3].mean(axis=1),panel[:,:,0].ravel(),panel[:,:,1].ravel(),cfg['metrics']['bootstrap'],cfg['execution']['annualization'])
                ics=[correlation(panel[:,i,0],panel[:,i,1]) for i in range(panel.shape[1])]
                ranks=[correlation(panel[:,i,0],panel[:,i,1],True) for i in range(panel.shape[1])]
                mean=lambda values: float(np.mean([v for v in values if v is not None])) if any(v is not None for v in values) else None
                name=f'{model}:{seed}'
                candidates[name]={'metrics':item,'ic_mean':mean(ics),'rank_ic_mean':mean(ranks),
                    'ic_hac_t':hac_mean_t([correlation(row[:,0],row[:,1]) for row in panel]),
                    'rank_ic_hac_t':hac_mean_t([correlation(row[:,0],row[:,1],True) for row in panel]),'dates':dates,
                    'excluded_dates_by_symbol':{symbol:len(rows)-len(dates) for symbol,rows in by_symbol.items()}}
                paths[name]=returns
        sr=[sharpe(values,1) for values in paths.values()]; sr=[v for v in sr if v is not None]
        variance=float(np.var(sr,ddof=1)) if len(sr)>1 else None
        for name,item in candidates.items(): item['dsr']=deflated_sharpe(paths[name],attempts,variance)
        seed_values=[candidates[f'lstm:{seed}']['metrics']['after_cost_sharpe'] for seed in cfg['seeds']]
        seed_summary={'after_cost_sharpe_mean':float(np.mean(seed_values)) if all(v is not None for v in seed_values) else None,
                      'after_cost_sharpe_std':float(np.std(seed_values,ddof=1)) if all(v is not None for v in seed_values) else None}
        comparison={}
        if args.stage=='holdout':
            boot=cfg['metrics']['bootstrap']; first=paths[f'lstm:{cfg["seeds"][0]}']
            draws=bootstrap_indices(len(first),boot['block_length'],boot['resamples'],boot['seed'])
            for baseline in ('ridge:42','gbm:42'):
                if any(candidates[k]['dates']!=candidates[baseline]['dates'] for k in paths): raise ValueError('Candidate date mismatch')
                differences=[]
                for ix in draws:
                    scores=[sharpe(paths[f'lstm:{seed}'][ix]) for seed in cfg['seeds']]; b=sharpe(paths[baseline][ix])
                    if b is not None and all(v is not None for v in scores): differences.append(float(np.mean(scores)-b))
                alpha=(1-boot['confidence'])/2
                comparison[baseline]=list(map(float,np.quantile(differences,[alpha,1-alpha]))) if differences else None
        conclusion='Development results only; final holdout remains unevaluated. No LSTM advantage claim is justified.'
        if args.stage=='holdout':
            conclusion='LSTM superiority is not established against both baselines.'
            if comparison and all(v is not None and v[0]>0 for v in comparison.values()):
                conclusion='Under this frozen protocol, the mean across LSTM seeds exceeds both baselines on holdout after-cost Sharpe with positive paired bootstrap difference intervals. This is not proof of deployable alpha.'
        result={'stage':args.stage,'config_sha256':digest,'results_path':str(result_path.relative_to(ROOT)),
                'plot_path':(folder/f'{args.stage}-equity.png').relative_to(ROOT).as_posix(),
                'trial_count':attempts,'candidates':candidates,'lstm_seed_summary':seed_summary,'paired_comparisons':comparison,
                'conclusion':conclusion,'pbo':None,'pbo_reason':'CSCV not implemented; no substitute estimate'}
        write_json(result_path,result)
        report(result,ROOT/'Docs/STUDY-REPORT.md')
        import matplotlib; matplotlib.use('Agg')
        import matplotlib.pyplot as plt
        for name,values in paths.items(): plt.plot(np.cumprod(1+values),label=name)
        plt.legend(fontsize=6); plt.xlabel('Aligned held-out observation'); plt.ylabel('Simulated wealth'); plt.tight_layout()
        plt.savefig(folder/f'{args.stage}-equity.png'); plt.close()
        print(json.dumps(result,indent=2,allow_nan=False))
    finally:
        lock.unlink()


if __name__=='__main__':
    main()
