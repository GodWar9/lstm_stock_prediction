"""Run a command without a shell and retain dated, source-bound evidence."""
import argparse
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--phase', required=True)
    parser.add_argument('--name', required=True)
    parser.add_argument('--cwd', default='.')
    parser.add_argument('--timeout', type=int, default=900)
    parser.add_argument('command', nargs=argparse.REMAINDER)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
    out = root / 'Docs' / 'evidence' / args.phase
    out.mkdir(parents=True, exist_ok=True)
    metadata = dict(command=command, cwd=str(root / args.cwd), started_utc=stamp,
                    commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
                    status=subprocess.check_output(['git', 'status', '--short'], cwd=root, text=True))
    log = out / f'{args.name}-{stamp}.txt'
    env = os.environ.copy()
    env['QUANTCTL_PYTHON'] = str(root / 'python/.venv/Scripts/python.exe') if os.name == 'nt' else str(root / 'python/.venv/bin/python')
    with log.open('w', encoding='utf-8') as stream:
        stream.write(json.dumps(metadata, indent=2) + '\n\n')
        stream.flush()
        try:
            result = subprocess.run(command, cwd=root / args.cwd, env=env, stdout=stream,
                                    stderr=subprocess.STDOUT, timeout=args.timeout)
            code = result.returncode
        except (OSError, subprocess.TimeoutExpired) as error:
            stream.write(f'\nRunner error: {error}\n')
            code = 124
        stream.write(f'\nEXIT_CODE={code}\n')
    print(f'{log.relative_to(root)} exit={code}', flush=True)
    return code


if __name__ == '__main__':
    sys.exit(main())
